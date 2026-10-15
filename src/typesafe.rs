//! Support for TypeSafe's System One models (Jev).
//!
//! Unlike chat models, System One models don't generate text.
//! They evaluate a `state` against typed questions and return
//! calibrated probabilities (https://docs.typesafe.ai/api).

use std::error::Error;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{error::ErrorKind, Arg, ArgAction, ArgMatches};
use color_print::{cformat, cprintln};
use image::{
  codecs::jpeg::JpegEncoder, imageops::FilterType, metadata::Orientation,
  DynamicImage, ImageDecoder, ImageFormat, ImageReader, Rgba, RgbaImage,
};
use reqwest::multipart::{Form, Part};
use serde_derive::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::{
  format_elapsed_time, get_base_url, get_full_config, get_secrets_path_str,
  ExecOptions,
};

pub const DEFAULT_JEV_MODEL: &str = "jev-latest";

/// Box questions need Jev 1.14, which is only available as preview
pub const DEFAULT_JEV_BOX_MODEL: &str = "jev-preview";

/// Key under which a single question is sent and its answer returned
const QUESTION_ID: &str = "answer";

/// Key of the piped text in the state when images are added to it
const TEXT_STATE_KEY: &str = "text";

/// Header that turns on the experimental image and Box support
const IMAGES_HEADER: &str = "TypeSafe-Danger-Zone-Images";

/// The question types supported by System One models
#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum JevQuestion {
  /// Yes/no question, answered with the probability of "yes"
  Noul,
  /// Pick one of the options, given as `name` or `name=description`
  Choice(Vec<String>),
  /// Rate along the ordered levels
  Score(Vec<String>),
  /// Locate what the instructions describe in an image
  Box,
}

impl JevQuestion {
  fn to_json(&self, instructions: &str) -> Value {
    match self {
      JevQuestion::Noul => json!({
        "type": "noul",
        "instructions": instructions,
      }),
      JevQuestion::Choice(options) => {
        let criteria = options
          .iter()
          .map(|option| match option.split_once('=') {
            Some((name, description)) => (
              name.trim().to_string(),
              Value::String(description.trim().to_string()),
            ),
            None => (option.trim().to_string(), Value::Null),
          })
          .collect::<Map<String, Value>>();
        json!({
          "type": "choice",
          "instructions": instructions,
          "criteria": criteria,
        })
      }
      JevQuestion::Score(levels) => json!({
        "type": "score",
        "instructions": instructions,
        "criteria": levels,
      }),
      JevQuestion::Box => json!({
        "type": "box",
        "instructions": instructions,
      }),
    }
  }
}

/// The default model for the questions:
/// Box questions need a newer model than the other question types
pub fn default_jev_model(questions: &[NamedJevQuestion]) -> &'static str {
  if questions
    .iter()
    .any(|named| named.question == JevQuestion::Box)
  {
    DEFAULT_JEV_BOX_MODEL
  } else {
    DEFAULT_JEV_MODEL
  }
}

/// An image file added to the state under `name`.
/// The model receives the image wherever `reference` appears in the request.
#[derive(Debug, PartialEq, Clone)]
struct ImageAsset {
  name: String,
  reference: String,
  file_name: String,
  mime_type: &'static str,
  bytes: Vec<u8>,
}

/// Smallest width and height an image may have
const MIN_IMAGE_SIDE: u32 = 256;
/// Largest width and height an image may have
const MAX_IMAGE_SIDE: u32 = 1568;
/// Smallest number of pixels an image may have (280 × 280)
const MIN_IMAGE_PIXELS: u64 = 78_400;
/// Quality of re-encoded JPEG images
const JPEG_QUALITY: u8 = 90;

/// The size an image of `width` × `height` must be scaled to
/// in order to fit the API's limits, or `None` if it already fits.
/// Scaling keeps the aspect ratio, so Box answers (fractions of the size)
/// are valid for the original image as well.
fn fitting_image_size(
  width: u32,
  height: u32,
) -> Result<Option<(u32, u32)>, String> {
  let (w, h) = (f64::from(width), f64::from(height));
  let long_side = w.max(h);
  let short_side = w.min(h);
  let pixels = u64::from(width) * u64::from(height);

  let scaled = if long_side > f64::from(MAX_IMAGE_SIDE) {
    let scale = f64::from(MAX_IMAGE_SIDE) / long_side;
    // Round down to never exceed the maximum
    Some(((w * scale).floor() as u32, (h * scale).floor() as u32))
  } else if short_side < f64::from(MIN_IMAGE_SIDE) || pixels < MIN_IMAGE_PIXELS
  {
    let scale = (f64::from(MIN_IMAGE_SIDE) / short_side)
      .max((MIN_IMAGE_PIXELS as f64 / (w * h)).sqrt());
    // Round up to never fall below the minimum
    Some(((w * scale).ceil() as u32, (h * scale).ceil() as u32))
  } else {
    None
  };

  if let Some((new_width, new_height)) = scaled {
    let fits = new_width.min(new_height) >= MIN_IMAGE_SIDE
      && new_width.max(new_height) <= MAX_IMAGE_SIDE
      && u64::from(new_width) * u64::from(new_height) >= MIN_IMAGE_PIXELS;
    if !fits {
      Err(format!(
        "Its aspect ratio of {width} × {height} px is too extreme \
        to scale it into {MIN_IMAGE_SIDE} to {MAX_IMAGE_SIDE} px per side. \
        Crop it first."
      ))?
    }
  }
  Ok(scaled)
}

/// An image file prepared for the API
#[derive(Debug, PartialEq, Clone)]
struct PreparedImage {
  bytes: Vec<u8>,
  mime_type: &'static str,
  extension: &'static str,
}

/// Prepare an image as the API requires it:
/// rotated upright according to its EXIF orientation
/// (the API strips all metadata), scaled into the size limits,
/// and encoded as PNG or JPEG.
/// PNG and JPEG images that already meet these requirements
/// are passed through unchanged.
/// A decoded image, rotated upright according to its EXIF orientation
struct UprightImage {
  format: Option<ImageFormat>,
  /// Whether the pixels were already upright before applying the orientation
  was_upright: bool,
  image: DynamicImage,
}

fn decode_upright(bytes: &[u8]) -> Result<UprightImage, String> {
  let decode = || -> image::ImageResult<_> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let format = reader.format();
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(UprightImage {
      format,
      was_upright: orientation == Orientation::NoTransforms,
      image,
    })
  };
  decode().map_err(|err| format!("It isn't a supported image: {err}"))
}

/// Encode as JPEG for photos, otherwise as lossless PNG
fn encode_image(
  image: &DynamicImage,
  as_jpeg: bool,
) -> Result<Vec<u8>, String> {
  let mut encoded = Vec::new();
  let encoding = if as_jpeg {
    JpegEncoder::new_with_quality(&mut encoded, JPEG_QUALITY)
      .encode_image(&image.to_rgb8())
  } else {
    image.write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
  };
  encoding.map_err(|err| format!("Failed to encode the image: {err}"))?;
  Ok(encoded)
}

fn prepare_image(bytes: Vec<u8>) -> Result<PreparedImage, String> {
  let UprightImage {
    format,
    was_upright: is_upright,
    mut image,
  } = decode_upright(&bytes)?;
  let is_jpeg = format == Some(ImageFormat::Jpeg);
  let new_size = fitting_image_size(image.width(), image.height())?;

  if (is_jpeg || format == Some(ImageFormat::Png))
    && is_upright
    && new_size.is_none()
  {
    return Ok(if is_jpeg {
      PreparedImage {
        bytes,
        mime_type: "image/jpeg",
        extension: "jpg",
      }
    } else {
      PreparedImage {
        bytes,
        mime_type: "image/png",
        extension: "png",
      }
    });
  }

  if let Some((width, height)) = new_size {
    image = image.resize_exact(width, height, FilterType::Lanczos3);
  }

  // Keep JPEG photos as JPEG, everything else becomes a lossless PNG
  let encoded = encode_image(&image, is_jpeg)?;

  Ok(if is_jpeg {
    PreparedImage {
      bytes: encoded,
      mime_type: "image/jpeg",
      extension: "jpg",
    }
  } else {
    PreparedImage {
      bytes: encoded,
      mime_type: "image/png",
      extension: "png",
    }
  })
}

/// Turn a file stem into a name usable as state key, e.g. `IMG 1` → `IMG_1`
fn image_name_from_path(path: &str) -> String {
  let name = Path::new(path)
    .file_stem()
    .map(|stem| {
      stem
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
    })
    .unwrap_or_default();
  if name.is_empty() {
    "image".to_string()
  } else {
    name
  }
}

/// Split `[NAME=]PATH` into name and path.
/// Without a name (or if the whole value is an existing file),
/// the name is derived from the file name.
fn parse_named_image(value: &str) -> (String, String) {
  match value.split_once('=') {
    Some((name, path))
      if !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !Path::new(value).exists() =>
    {
      (name.to_string(), path.to_string())
    }
    _ => (image_name_from_path(value), value.to_string()),
  }
}

/// A reference derived from the image's content,
/// so it can't occur in the text by accident
fn image_reference(bytes: &[u8]) -> String {
  let hash = Sha256::digest(bytes);
  let hex = hash[..8]
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();
  format!("image-{hex}")
}

/// Build an image asset from `[NAME=]PATH` and the file's content
fn image_asset(value: &str, bytes: Vec<u8>) -> Result<ImageAsset, String> {
  let (name, path) = parse_named_image(value);
  let prepared = prepare_image(bytes)
    .map_err(|err| format!("Can't send image `{path}`. {err}"))?;
  let file_name = Path::new(&path)
    .with_extension(prepared.extension)
    .file_name()
    .map(|file_name| file_name.to_string_lossy().to_string())
    .unwrap_or_else(|| format!("image.{}", prepared.extension));
  Ok(ImageAsset {
    name,
    reference: image_reference(&prepared.bytes),
    file_name,
    mime_type: prepared.mime_type,
    bytes: prepared.bytes,
  })
}

/// Color of the box drawn onto the image
const BOX_COLOR: Rgba<u8> = Rgba([0, 255, 0, 255]);

/// The edges of a box answer as fractions `(left, top, right, bottom)`
fn box_edges(edges: &Value) -> (f64, f64, f64, f64) {
  let edge = |name: &str| edges[name].as_f64().unwrap_or_default();
  (edge("left"), edge("top"), edge("right"), edge("bottom"))
}

/// Draw the outline of the box onto the image, inside the box's edges
fn draw_box(image: &mut RgbaImage, edges: &Value) {
  let (width, height) = image.dimensions();
  if width == 0 || height == 0 {
    return;
  }
  // The box covers the pixels from the start edge up to,
  // but excluding, the end edge
  let to_px = |fraction: f64, size: u32| {
    (fraction.clamp(0.0, 1.0) * f64::from(size)).round() as u32
  };
  let start_px = |fraction, size| to_px(fraction, size).min(size - 1);
  let end_px = |fraction, size| to_px(fraction, size).clamp(1, size) - 1;
  let (left, top, right, bottom) = box_edges(edges);
  let (x0, x1) = (start_px(left, width), end_px(right, width));
  let (y0, y1) = (start_px(top, height), end_px(bottom, height));
  let (x0, x1, y0, y1) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
  let thickness = (width.min(height) / 200).max(2);

  for y in y0..=y1 {
    for x in x0..=x1 {
      let is_outline = x - x0 < thickness
        || x1 - x < thickness
        || y - y0 < thickness
        || y1 - y < thickness;
      if is_outline {
        image.put_pixel(x, y, BOX_COLOR);
      }
    }
  }
}

/// Write the image with the box drawn onto it to a temporary file
/// and return the file's path
fn write_box_overlay(
  upright: UprightImage,
  edges: &Value,
  image_path: &str,
) -> Result<PathBuf, String> {
  let mut overlay = upright.image.to_rgba8();
  draw_box(&mut overlay, edges);

  let as_jpeg = upright.format == Some(ImageFormat::Jpeg);
  let encoded = encode_image(&DynamicImage::ImageRgba8(overlay), as_jpeg)?;
  let stem = image_name_from_path(image_path);
  let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S%.3f");
  let extension = if as_jpeg { "jpg" } else { "png" };
  let overlay_path = std::env::temp_dir()
    .join(format!("cai-box-{stem}-{timestamp}.{extension}"));
  std::fs::write(&overlay_path, encoded).map_err(|err| {
    format!("Failed to write `{}`: {err}", overlay_path.display())
  })?;
  Ok(overlay_path)
}

/// Read the images passed as `-i/--image [NAME=]PATH`
fn load_images(values: &[String]) -> Result<Vec<ImageAsset>, String> {
  let mut images: Vec<ImageAsset> = Vec::new();
  for value in values {
    let (_, path) = parse_named_image(value);
    let bytes = std::fs::read(&path)
      .map_err(|err| format!("Failed to read image `{path}`: {err}"))?;
    let image = image_asset(value, bytes)?;
    if images.iter().any(|other| other.name == image.name) {
      Err(format!(
        "Image name `{}` is used more than once. \
        Name the images explicitly with `-i NAME=PATH`.",
        image.name
      ))?
    }
    images.push(image);
  }
  Ok(images)
}

/// A question with the id its answer is returned under
#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct NamedJevQuestion {
  pub name: String,
  pub instructions: String,
  pub question: JevQuestion,
}

/// Several questions passed as order-dependent CLI flags, e.g.
/// `--noul a='Is it urgent?' --choice b='Which team?' -o billing -o sales`.
/// Each `-o/--option` and `-l/--level` belongs to the preceding
/// `--choice` or `--score` question.
#[derive(Debug, PartialEq, Clone, Default, Serialize)]
pub struct JevQuestions(pub Vec<NamedJevQuestion>);

/// Split `name=question` into its parts.
/// Values whose part before the first `=` isn't an identifier
/// (e.g. `Is 1+1=2?`) are used as question and get an auto-generated name.
fn parse_named_question(value: &str, position: usize) -> (String, String) {
  match value.split_once('=') {
    Some((name, question))
      if !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') =>
    {
      (name.to_string(), question.trim().to_string())
    }
    _ => (format!("q{position}"), value.trim().to_string()),
  }
}

impl JevQuestions {
  const ARG_IDS: [&'static str; 6] =
    ["noul", "choice", "option", "score", "level", "box"];

  /// Build the questions from `(arg id, value)` pairs in command line order
  fn from_ordered_args(args: &[(&str, String)]) -> Result<Self, String> {
    let mut questions: Vec<NamedJevQuestion> = Vec::new();

    for (arg_id, value) in args {
      match *arg_id {
        "noul" | "choice" | "score" | "box" => {
          let (name, instructions) =
            parse_named_question(value, questions.len() + 1);
          if questions.iter().any(|question| question.name == name) {
            Err(format!("Question name `{name}` is used more than once"))?
          }
          let question = match *arg_id {
            "noul" => JevQuestion::Noul,
            "choice" => JevQuestion::Choice(Vec::new()),
            "score" => JevQuestion::Score(Vec::new()),
            _ => JevQuestion::Box,
          };
          questions.push(NamedJevQuestion {
            name,
            instructions,
            question,
          });
        }
        "option" => match questions.last_mut().map(|q| &mut q.question) {
          Some(JevQuestion::Choice(options)) => options.push(value.clone()),
          _ => Err(format!(
            "`-o/--option {value}` must follow a `--choice` question"
          ))?,
        },
        "level" => match questions.last_mut().map(|q| &mut q.question) {
          Some(JevQuestion::Score(levels)) => levels.push(value.clone()),
          _ => Err(format!(
            "`-l/--level {value}` must follow a `--score` question"
          ))?,
        },
        _ => unreachable!("Unknown jev argument `{arg_id}`"),
      }
    }

    for NamedJevQuestion { name, question, .. } in &questions {
      match question {
        JevQuestion::Choice(options) if options.is_empty() => Err(format!(
          "`--choice {name}` needs at least one `-o/--option`"
        ))?,
        JevQuestion::Score(levels) if levels.is_empty() => {
          Err(format!("`--score {name}` needs at least one `-l/--level`"))?
        }
        _ => {}
      }
    }

    if questions.is_empty() {
      Err(
        "Provide at least one `--noul`, `--choice`, `--score`, \
        or `--box` question",
      )?
    }

    Ok(JevQuestions(questions))
  }
}

impl clap::Args for JevQuestions {
  fn augment_args(cmd: clap::Command) -> clap::Command {
    cmd
      .arg(
        Arg::new("noul")
          .long("noul")
          .value_name("[NAME=]QUESTION")
          .action(ArgAction::Append)
          .help("Yes/no question, answered with the probability of yes"),
      )
      .arg(
        Arg::new("choice")
          .long("choice")
          .value_name("[NAME=]QUESTION")
          .action(ArgAction::Append)
          .help("Pick one of the subsequent `-o/--option`s"),
      )
      .arg(
        Arg::new("option")
          .long("option")
          .short('o')
          .value_name("NAME[=DESCRIPTION]")
          .action(ArgAction::Append)
          .help("An option of the preceding `--choice` question"),
      )
      .arg(
        Arg::new("score")
          .long("score")
          .value_name("[NAME=]QUESTION")
          .action(ArgAction::Append)
          .help("Rate along the subsequent `-l/--level`s"),
      )
      .arg(
        Arg::new("level")
          .long("level")
          .short('l')
          .value_name("DESCRIPTION")
          .action(ArgAction::Append)
          .help(
            "A level of the preceding `--score` question, \
            from lowest to highest",
          ),
      )
      .arg(
        Arg::new("box")
          .long("box")
          .value_name("[NAME=]TARGET")
          .action(ArgAction::Append)
          .help(
            "Locate the target in an image \
            (needs an `-i/--image` and Jev 1.14+)",
          ),
      )
  }

  fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
    Self::augment_args(cmd)
  }
}

impl clap::FromArgMatches for JevQuestions {
  fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
    // Restore the command line order, which clap doesn't keep across args
    let mut indexed_args: Vec<(usize, &str, String)> = Vec::new();
    for arg_id in Self::ARG_IDS {
      if let (Some(indices), Some(values)) = (
        matches.indices_of(arg_id),
        matches.get_many::<String>(arg_id),
      ) {
        indexed_args.extend(
          indices
            .zip(values)
            .map(|(index, value)| (index, arg_id, value.clone())),
        );
      }
    }
    indexed_args.sort_by_key(|(index, ..)| *index);
    let args = indexed_args
      .into_iter()
      .map(|(_, arg_id, value)| (arg_id, value))
      .collect::<Vec<_>>();

    Self::from_ordered_args(&args)
      .map_err(|msg| clap::Error::raw(ErrorKind::ArgumentConflict, msg + "\n"))
  }

  fn update_from_arg_matches(
    &mut self,
    matches: &ArgMatches,
  ) -> Result<(), clap::Error> {
    *self = Self::from_arg_matches(matches)?;
    Ok(())
  }
}

/// Use structured JSON input (object or array) as structured state,
/// everything else as plain text
fn parse_state(state: &str) -> Value {
  match serde_json::from_str::<Value>(state) {
    Ok(value @ (Value::Object(_) | Value::Array(_))) => value,
    _ => Value::String(state.to_string()),
  }
}

/// Build the state from the piped input and the images.
/// Images are added under their names to a piped JSON object,
/// other piped input is put under the key `text`.
fn build_state(state: &str, images: &[ImageAsset]) -> Result<Value, String> {
  if images.is_empty() {
    return Ok(parse_state(state));
  }

  let mut state_obj = match state.trim() {
    "" => Map::new(),
    state => match parse_state(state) {
      Value::Object(obj) => obj,
      value => Map::from_iter([(TEXT_STATE_KEY.to_string(), value)]),
    },
  };
  for image in images {
    if state_obj.contains_key(&image.name) {
      Err(format!(
        "Image name `{}` is already a key of the state. \
        Choose another name with `-i NAME=PATH`.",
        image.name
      ))?
    }
    state_obj
      .insert(image.name.clone(), Value::String(image.reference.clone()));
  }
  Ok(Value::Object(state_obj))
}

fn jev_request_body(
  model: &str,
  state: Value,
  questions: &[NamedJevQuestion],
  images: &[ImageAsset],
) -> Value {
  let questions_obj = questions
    .iter()
    .map(|named| {
      (
        named.name.clone(),
        named.question.to_json(&named.instructions),
      )
    })
    .collect::<Map<String, Value>>();

  let mut body = json!({
    "state": state,
    "model": model,
    "questions": questions_obj,
  });

  if !images.is_empty() {
    // Each image is uploaded as the multipart file part named by its reference
    let assets = images
      .iter()
      .map(|image| {
        (
          image.reference.clone(),
          json!({ "type": "image", "part": image.reference }),
        )
      })
      .collect::<Map<String, Value>>();
    body["assets"] = Value::Object(assets);
  }

  body
}

/// Images and Box questions are experimental and must be turned on explicitly
fn needs_images_header(
  questions: &[NamedJevQuestion],
  images: &[ImageAsset],
) -> bool {
  !images.is_empty()
    || questions
      .iter()
      .any(|named| named.question == JevQuestion::Box)
}

/// Multipart form with the JSON body as first part, followed by the images.
/// Identical images are only uploaded once.
fn multipart_form(
  body: &Value,
  images: &[ImageAsset],
) -> Result<Form, Box<dyn Error + Send + Sync>> {
  let mut form = Form::new().part(
    "request",
    Part::text(body.to_string()).mime_str("application/json")?,
  );
  let mut uploaded: Vec<&str> = Vec::new();
  for image in images {
    if uploaded.contains(&image.reference.as_str()) {
      continue;
    }
    uploaded.push(&image.reference);
    form = form.part(
      image.reference.clone(),
      Part::bytes(image.bytes.clone())
        .file_name(image.file_name.clone())
        .mime_str(image.mime_type)?,
    );
  }
  Ok(form)
}

/// Probabilities of a choice or score answer as `(key, probability)` pairs
fn answer_probabilities(answer: &Value) -> Vec<(String, f64)> {
  answer["probabilities"]
    .as_object()
    .map(|probs| {
      probs
        .iter()
        .map(|(key, prob)| (key.clone(), prob.as_f64().unwrap_or_default()))
        .collect()
    })
    .unwrap_or_default()
}

/// One line per edge and the confidence of a box answer.
/// With the image's size, also the edges' pixel positions.
fn format_box_answer(answer: &Value, size: Option<(u32, u32)>) -> String {
  let (left, top, right, bottom) = box_edges(&answer["box"]);
  let mut rows = [
    ("left", left, size.map(|(width, _)| width)),
    ("top", top, size.map(|(_, height)| height)),
    ("right", right, size.map(|(width, _)| width)),
    ("bottom", bottom, size.map(|(_, height)| height)),
  ]
  .iter()
  .map(|(edge, fraction, pixels)| {
    let pixels = pixels
      .map(|pixels| {
        format!("  {:>5} px", (fraction * f64::from(pixels)).round())
      })
      .unwrap_or_default();
    format!("{edge:<10}  {fraction:.2}{pixels}")
  })
  .collect::<Vec<_>>();
  if let Some(conf) = answer["confidence"].as_f64() {
    rows.push(cformat!("<dim>{:<10}  {conf:.2}</dim>", "confidence"));
  }
  rows.join("\n")
}

/// The edges of a box answer in the order left, top, right, bottom.
/// Raw mode only prints the space separated fractions.
fn format_box(edges: &Value, is_raw: bool) -> String {
  ["left", "top", "right", "bottom"]
    .iter()
    .map(|edge| {
      let fraction = edges[edge].as_f64().unwrap_or_default();
      if is_raw {
        fraction.to_string()
      } else {
        format!("{edge} {fraction:.2}")
      }
    })
    .collect::<Vec<_>>()
    .join(if is_raw { " " } else { ", " })
}

/// The bare value of an answer (probability, option, score, or box)
fn answer_value(answer: &Value, is_raw: bool) -> String {
  let answer_type = answer["type"].as_str().unwrap_or_default();
  match &answer[answer_type] {
    edges @ Value::Object(_) if answer_type == "box" => {
      format_box(edges, is_raw)
    }
    Value::String(string) => string.clone(),
    Value::Number(number) if !is_raw => {
      format!("{:.2}", number.as_f64().unwrap_or_default())
    }
    value => value.to_string(),
  }
}

fn answer_confidence(answer: &Value) -> Option<String> {
  answer["confidence"]
    .as_f64()
    .map(|conf| cformat!("<dim>(confidence: {conf:.2})</dim>"))
}

/// Format an answer with all its probabilities for display.
/// Raw mode only prints the bare value.
fn format_answer(answer: &Value, is_raw: bool) -> String {
  let value = answer_value(answer, is_raw);
  if is_raw {
    return value;
  }

  let confidence = answer_confidence(answer)
    .map(|conf| format!(" {conf}"))
    .unwrap_or_default();

  match answer["type"].as_str().unwrap_or_default() {
    "noul" => value,
    "box" => format_box_answer(answer, None),
    "choice" => {
      let mut probs = answer_probabilities(answer);
      probs.sort_by(|a, b| b.1.total_cmp(&a.1));
      let width = probs.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
      let rows = probs
        .iter()
        .map(|(key, prob)| format!("  {key:<width$}  {prob:.2}"))
        .collect::<Vec<_>>()
        .join("\n");
      format!("{value}{confidence}\n\n{rows}")
    }
    "score" => {
      let mut probs = answer_probabilities(answer);
      probs.sort_by_key(|(level, _)| level.parse::<usize>().unwrap_or(0));
      let legends = probs
        .iter()
        .map(|(level, _)| answer["legend"][level].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
      let width = legends.iter().map(|legend| legend.len()).max().unwrap_or(0);
      let rows = probs
        .iter()
        .zip(legends)
        .map(|((level, prob), legend)| {
          format!("  {level}  {legend:<width$}  {prob:.2}")
        })
        .collect::<Vec<_>>()
        .join("\n");
      format!("{value}{confidence}\n\n{rows}")
    }
    _ => serde_json::to_string_pretty(answer).unwrap_or_default(),
  }
}

/// Format the answers to several questions as one line per question,
/// in the order the questions were asked.
/// Raw mode prints tab separated `name` and `value` pairs.
fn format_answers_table(
  questions: &[NamedJevQuestion],
  answers: &Value,
  is_raw: bool,
) -> String {
  let rows = questions
    .iter()
    .map(|named| {
      let answer = &answers[&named.name];
      (
        named.name.as_str(),
        answer_value(answer, is_raw),
        answer_confidence(answer),
      )
    })
    .collect::<Vec<_>>();

  if is_raw {
    return rows
      .iter()
      .map(|(name, value, _)| format!("{name}\t{value}"))
      .collect::<Vec<_>>()
      .join("\n");
  }

  let name_width = rows.iter().map(|(name, ..)| name.len()).max().unwrap_or(0);
  let value_width = rows
    .iter()
    .filter(|(.., confidence)| confidence.is_some())
    .map(|(_, value, _)| value.chars().count())
    .max()
    .unwrap_or(0);

  rows
    .iter()
    .map(|(name, value, confidence)| match confidence {
      Some(conf) => {
        format!("{name:<name_width$}  {value:<value_width$}  {conf}")
      }
      None => format!("{name:<name_width$}  {value}"),
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// Send the questions to a System One model.
/// Prints the full response in JSON mode and returns `None`,
/// otherwise prints the header line (unless in raw mode)
/// and returns the answers.
async fn request_answers(
  opts: &ExecOptions,
  model: &str,
  state: &str,
  image_args: &[String],
  questions: &[NamedJevQuestion],
) -> Result<Option<Value>, Box<dyn Error + Send + Sync>> {
  if state.trim().is_empty() && image_args.is_empty() {
    Err(
      "Pipe the text to evaluate into cai via stdin \
      or pass an image with `-i/--image`, \
      e.g. `echo 'Help, my payouts fail!' | cai noul Is this urgent`",
    )?
  }
  if let Some(named) = questions
    .iter()
    .find(|named| named.instructions.trim().is_empty())
  {
    Err(format!("Question `{}` is empty", named.name))?
  }
  if image_args.is_empty()
    && questions
      .iter()
      .any(|named| named.question == JevQuestion::Box)
  {
    Err("Box questions need an image. Pass one with `-i/--image`.")?
  }
  let images = load_images(image_args)?;
  let body =
    jev_request_body(model, build_state(state, &images)?, questions, &images);

  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let api_key = full_config
    .get("typesafe_api_key")
    .filter(|key| !key.is_empty())
    .ok_or(format!(
      "A TypeSafe API key must be provided. Use one of the following options:\n\
      \n\
      1. Set `typesafe_api_key` in {secrets_path_str}\n\
      2. Set the env variable CAI_TYPESAFE_API_KEY\n\
      3. Set the env variable TYPESAFE_API_KEY\n\
      \n\
      Create a new API key at https://console.typesafe.ai/keys\n"
    ))?;
  let base_url = get_base_url(
    &full_config,
    "typesafe_base_url",
    "https://api.typesafe.ai/v1",
  );

  let mut request = reqwest::Client::new()
    .post(format!("{base_url}/systemone"))
    .bearer_auth(api_key);
  if needs_images_header(questions, &images) {
    request = request.header(IMAGES_HEADER, "true");
  }
  // Multipart sends the images as they are,
  // while base64 in JSON would make them a third larger
  request = if images.is_empty() {
    request.json(&body)
  } else {
    request.multipart(multipart_form(&body, &images)?)
  };

  let start = Instant::now();
  let resp = request.send().await?;
  let (elapsed_time, time_unit) =
    format_elapsed_time(start.elapsed().as_millis());

  let status = resp.status();
  let resp_text = resp.text().await?;
  let resp_json = serde_json::from_str::<Value>(&resp_text);

  if !status.is_success() {
    let details = resp_json
      .ok()
      .and_then(|json| serde_json::to_string_pretty(&json).ok())
      .unwrap_or(resp_text);
    return Err(format!("TypeSafe API returned {status}\n\n{details}").into());
  }
  let mut resp_json = resp_json?;

  if opts.is_json {
    println!("{}", serde_json::to_string_pretty(&resp_json)?);
    return Ok(None);
  }

  if let Some(named) = questions
    .iter()
    .find(|named| resp_json["answers"][&named.name].is_null())
  {
    Err(format!(
      "Response contains no answer for `{}`:\n{resp_json}",
      named.name
    ))?
  }

  if !opts.is_raw {
    let subcommand = opts
      .subcommand
      .as_ref()
      .and_then(|x| x.to_string_pretty())
      .map(|subcom| format!("➡️ {subcom} | "))
      .unwrap_or_default();
    let used_model = resp_json["model"].as_str().unwrap_or(model);
    cprintln!(
      "<bold>{subcommand}🧠 TypeSafe {used_model} | ⏱️ {elapsed_time} {time_unit}</bold>\n",
    );
  }

  Ok(Some(resp_json["answers"].take()))
}

/// Ask a System One model a question about the given state and images
/// (passed as `[NAME=]PATH`) and print its answer with all probabilities
pub async fn ask_jev(
  opts: &ExecOptions,
  model: &str,
  state: &str,
  images: &[String],
  instructions: &str,
  question: &JevQuestion,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let questions = [NamedJevQuestion {
    name: QUESTION_ID.to_string(),
    instructions: instructions.to_string(),
    question: question.clone(),
  }];
  if let Some(answers) =
    request_answers(opts, model, state, images, &questions).await?
  {
    println!("{}", format_answer(&answers[QUESTION_ID], opts.is_raw));
  }
  Ok(())
}

/// Locate what the instructions describe in the image (`[NAME=]PATH`),
/// print the box's edges, and write a copy of the image with the box
/// drawn onto it to a temporary file.
/// Further images (e.g. a picture of the target) can be passed as `images`.
pub async fn ask_jev_box(
  opts: &ExecOptions,
  model: &str,
  state: &str,
  image: &str,
  images: &[String],
  instructions: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let image_args = [&[image.to_string()], images].concat();
  let questions = [NamedJevQuestion {
    name: QUESTION_ID.to_string(),
    instructions: instructions.to_string(),
    question: JevQuestion::Box,
  }];
  let Some(answers) =
    request_answers(opts, model, state, &image_args, &questions).await?
  else {
    return Ok(());
  };
  let answer = &answers[QUESTION_ID];
  if opts.is_raw {
    println!("{}", format_answer(answer, true));
    return Ok(());
  }

  let (_, path) = parse_named_image(image);
  let upright = decode_upright(&std::fs::read(&path)?)?;
  println!(
    "{}\n",
    format_box_answer(
      answer,
      Some((upright.image.width(), upright.image.height())),
    )
  );
  let overlay_path = write_box_overlay(upright, &answer["box"], &path)?;
  println!("Overlay: {}", overlay_path.display());
  Ok(())
}

/// Ask a System One model several questions about the given state
/// and images (passed as `[NAME=]PATH`) in one request
/// and print one answer per line
pub async fn ask_jev_many(
  opts: &ExecOptions,
  model: &str,
  state: &str,
  images: &[String],
  questions: &[NamedJevQuestion],
) -> Result<(), Box<dyn Error + Send + Sync>> {
  if let Some(answers) =
    request_answers(opts, model, state, images, questions).await?
  {
    println!("{}", format_answers_table(questions, &answers, opts.is_raw));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn single(
    instructions: &str,
    question: JevQuestion,
  ) -> Vec<NamedJevQuestion> {
    vec![NamedJevQuestion {
      name: QUESTION_ID.to_string(),
      instructions: instructions.to_string(),
      question,
    }]
  }

  fn args(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    pairs
      .iter()
      .map(|(arg_id, value)| (*arg_id, value.to_string()))
      .collect()
  }

  #[test]
  fn test_noul_request_body() {
    let body = jev_request_body(
      "jev-latest",
      parse_state("Help! My payouts fail."),
      &single("Is this urgent?", JevQuestion::Noul),
      &[],
    );
    assert_eq!(
      body,
      json!({
        "state": "Help! My payouts fail.",
        "model": "jev-latest",
        "questions": {
          "answer": { "type": "noul", "instructions": "Is this urgent?" }
        }
      })
    );
  }

  #[test]
  fn test_choice_request_body_parses_descriptions() {
    let options = vec![
      "billing = Payment issues".to_string(),
      "technical=Bugs, a=b".to_string(),
      "sales".to_string(),
    ];
    let questions = single("Which team?", JevQuestion::Choice(options));
    let body = jev_request_body("jev-latest", json!("x"), &questions, &[]);
    assert_eq!(
      body["questions"]["answer"]["criteria"],
      json!({
        "billing": "Payment issues",
        "technical": "Bugs, a=b",
        "sales": null,
      })
    );
  }

  #[test]
  fn test_score_request_body_keeps_level_order() {
    let levels = vec!["Calm".to_string(), "Angry".to_string()];
    let questions = single("How angry?", JevQuestion::Score(levels));
    let body = jev_request_body("jev-latest", json!("x"), &questions, &[]);
    assert_eq!(body["questions"]["answer"]["type"], "score");
    assert_eq!(
      body["questions"]["answer"]["criteria"],
      json!(["Calm", "Angry"])
    );
  }

  #[test]
  fn test_json_input_is_sent_as_structured_state() {
    assert_eq!(build_state(r#"{"a": 1}"#, &[]).unwrap(), json!({ "a": 1 }));

    // JSON scalars stay plain text
    assert_eq!(build_state("42", &[]).unwrap(), json!("42"));
  }

  #[test]
  fn test_parse_named_question() {
    let cases = [
      ("urgent=Is it urgent?", ("urgent", "Is it urgent?")),
      ("is_repeat_2=Again?", ("is_repeat_2", "Again?")),
      ("Is 1+1=2?", ("q3", "Is 1+1=2?")),
      ("no name", ("q3", "no name")),
      ("=Empty name?", ("q3", "=Empty name?")),
    ];
    for (value, (name, question)) in cases {
      assert_eq!(
        parse_named_question(value, 3),
        (name.to_string(), question.to_string()),
        "`{value}`"
      );
    }
  }

  #[test]
  fn test_questions_from_ordered_args() {
    let questions = JevQuestions::from_ordered_args(&args(&[
      ("noul", "human=Asks for a human?"),
      ("choice", "team=Which team?"),
      ("option", "billing=Payments"),
      ("option", "sales"),
      ("score", "How angry?"),
      ("level", "Calm"),
      ("level", "Angry"),
    ]))
    .unwrap();

    let summary = questions
      .0
      .iter()
      .map(|named| (named.name.as_str(), named.question.clone()))
      .collect::<Vec<_>>();
    assert_eq!(
      summary,
      vec![
        ("human", JevQuestion::Noul),
        (
          "team",
          JevQuestion::Choice(vec!["billing=Payments".into(), "sales".into()])
        ),
        (
          "q3",
          JevQuestion::Score(vec!["Calm".into(), "Angry".into()])
        ),
      ]
    );
  }

  #[test]
  fn test_questions_from_invalid_args() {
    let cases = [
      (vec![], "at least one"),
      (vec![("option", "sales")], "must follow a `--choice`"),
      (
        vec![("noul", "a=?"), ("option", "x")],
        "must follow a `--choice`",
      ),
      (
        vec![("choice", "a=?"), ("level", "x")],
        "must follow a `--score`",
      ),
      (vec![("choice", "a=?")], "`--choice a` needs at least one"),
      (vec![("score", "a=?")], "`--score a` needs at least one"),
      (
        vec![("noul", "a=?"), ("noul", "a=!")],
        "`a` is used more than once",
      ),
    ];
    for (pairs, expected) in cases {
      let err = JevQuestions::from_ordered_args(&args(&pairs)).unwrap_err();
      assert!(err.contains(expected), "{pairs:?}: {err}");
    }
  }

  #[test]
  fn test_cli_flags_keep_command_line_order() {
    use clap::{Args, FromArgMatches};
    let cmd = JevQuestions::augment_args(clap::Command::new("jev"));
    let matches = cmd
      .try_get_matches_from([
        "jev", "--score", "s=?", "-l", "low", "--choice", "c=?", "-o", "x",
        "--score", "t=?", "-l", "high", "--noul", "n=?",
      ])
      .unwrap();
    let questions = JevQuestions::from_arg_matches(&matches).unwrap();
    let summary = questions
      .0
      .iter()
      .map(|named| (named.name.as_str(), named.question.clone()))
      .collect::<Vec<_>>();
    assert_eq!(
      summary,
      vec![
        ("s", JevQuestion::Score(vec!["low".into()])),
        ("c", JevQuestion::Choice(vec!["x".into()])),
        ("t", JevQuestion::Score(vec!["high".into()])),
        ("n", JevQuestion::Noul),
      ]
    );
  }

  #[test]
  fn test_format_raw_answers() {
    let noul = json!({ "type": "noul", "noul": 0.95 });
    assert_eq!(format_answer(&noul, true), "0.95");

    let choice = json!({
      "type": "choice",
      "choice": "technical",
      "confidence": 0.78,
      "probabilities": { "technical": 0.85, "billing": 0.15 }
    });
    assert_eq!(format_answer(&choice, true), "technical");

    let score = json!({
      "type": "score",
      "score": 1.05,
      "confidence": 0.92,
      "legend": { "0": "Calm", "1": "Frustrated" },
      "probabilities": { "0": 0.0, "1": 1.0 }
    });
    assert_eq!(format_answer(&score, true), "1.05");
  }

  #[test]
  fn test_format_choice_sorts_by_probability() {
    let choice = json!({
      "type": "choice",
      "choice": "technical",
      "confidence": 0.78,
      "probabilities": { "billing": 0.15, "sales": 0.0, "technical": 0.85 }
    });
    let formatted = format_answer(&choice, false);
    let rows = formatted.lines().skip(2).collect::<Vec<_>>();
    assert_eq!(
      rows,
      vec![
        "  technical  0.85",
        "  billing    0.15",
        "  sales      0.00"
      ]
    );
  }

  #[test]
  fn test_format_score_sorts_levels_numerically() {
    let levels = (0..12)
      .map(|level| (level.to_string(), json!(format!("L{level}"))))
      .collect::<Map<String, Value>>();
    let probs = (0..12)
      .map(|level| (level.to_string(), json!(0.0)))
      .collect::<Map<String, Value>>();
    let score = json!({
      "type": "score",
      "score": 0.0,
      "confidence": 1.0,
      "legend": levels,
      "probabilities": probs,
    });
    let formatted = format_answer(&score, false);
    let first_cols = formatted
      .lines()
      .skip(2)
      .map(|row| row.split_whitespace().next().unwrap())
      .collect::<Vec<_>>();
    assert_eq!(
      first_cols,
      (0..12).map(|level| level.to_string()).collect::<Vec<_>>()
    );
  }

  #[test]
  fn test_format_answers_table_keeps_question_order() {
    let questions = JevQuestions::from_ordered_args(&args(&[
      ("noul", "zeta=?"),
      ("choice", "team=?"),
      ("option", "technical"),
      ("noul", "alpha=?"),
    ]))
    .unwrap()
    .0;
    let answers = json!({
      "alpha": { "type": "noul", "noul": 0.1 },
      "team": { "type": "choice", "choice": "technical", "confidence": 0.75 },
      "zeta": { "type": "noul", "noul": 0.97 },
    });

    assert_eq!(
      format_answers_table(&questions, &answers, true),
      "zeta\t0.97\nteam\ttechnical\nalpha\t0.1"
    );

    let table = format_answers_table(&questions, &answers, false);
    let lines = table.lines().collect::<Vec<_>>();
    assert_eq!(lines[0], "zeta   0.97");
    assert!(lines[1].starts_with("team   technical  "), "{}", lines[1]);
    assert!(lines[1].contains("(confidence: 0.75)"), "{}", lines[1]);
    assert_eq!(lines[2], "alpha  0.10");
  }

  /// An encoded single-color image
  fn encode_image(
    width: u32,
    height: u32,
    format: image::ImageFormat,
  ) -> Vec<u8> {
    let image =
      image::RgbImage::from_pixel(width, height, [200, 30, 30].into());
    let mut bytes = Vec::new();
    image
      .write_to(&mut std::io::Cursor::new(&mut bytes), format)
      .unwrap();
    bytes
  }

  fn image_size(bytes: &[u8]) -> (u32, u32) {
    let image = image::load_from_memory(bytes).unwrap();
    (image.width(), image.height())
  }

  fn png(value: &str) -> ImageAsset {
    let bytes = encode_image(300, 300, image::ImageFormat::Png);
    image_asset(value, bytes).unwrap()
  }

  #[test]
  fn test_parse_named_image() {
    let cases = [
      ("door=photos/door.jpg", ("door", "photos/door.jpg")),
      (
        "photos/door-cam 1.jpg",
        ("door_cam_1", "photos/door-cam 1.jpg"),
      ),
      ("a b=c.png", ("a_b_c", "a b=c.png")),
      ("=c.png", ("_c", "=c.png")),
      (".png", ("_png", ".png")),
    ];
    for (value, (name, path)) in cases {
      assert_eq!(
        parse_named_image(value),
        (name.to_string(), path.to_string()),
        "`{value}`"
      );
    }
  }

  #[test]
  fn test_image_asset_detects_format() {
    let image = png("door=photos/door.png");
    assert_eq!(image.name, "door");
    assert_eq!(image.file_name, "door.png");
    assert_eq!(image.mime_type, "image/png");
    assert!(image.reference.starts_with("image-"), "{}", image.reference);
    assert_eq!(image.reference.len(), "image-".len() + 16);

    let jpg_bytes = encode_image(300, 300, image::ImageFormat::Jpeg);
    let jpg = image_asset("x.jpg", jpg_bytes).unwrap();
    assert_eq!(jpg.mime_type, "image/jpeg");
    assert_ne!(jpg.reference, image.reference);

    let err = image_asset("x.txt", b"Hello".to_vec()).unwrap_err();
    assert!(err.contains("`x.txt`. It isn't a supported image"), "{err}");
  }

  #[test]
  fn test_fitting_image_size() {
    let cases = [
      ((800, 600), Ok(None)),
      ((1568, 256 * 2), Ok(None)),
      ((4032, 3024), Ok(Some((1568, 1176)))),
      ((3024, 4032), Ok(Some((1176, 1568)))),
      ((100, 100), Ok(Some((280, 280)))),
      ((256, 256), Ok(Some((280, 280)))),
      ((200, 1000), Ok(Some((256, 1280)))),
      ((100, 1000), Err("too extreme")),
      ((5000, 500), Err("too extreme")),
    ];
    for ((width, height), expected) in cases {
      let result = fitting_image_size(width, height);
      match expected {
        Ok(size) => assert_eq!(result, Ok(size), "{width} × {height}"),
        Err(msg) => assert!(
          result.as_ref().is_err_and(|err| err.contains(msg)),
          "{width} × {height}: {result:?}"
        ),
      }
    }
  }

  #[test]
  fn test_prepare_image_passes_fitting_images_through() {
    let bytes = encode_image(800, 600, image::ImageFormat::Png);
    let prepared = prepare_image(bytes.clone()).unwrap();
    assert_eq!(prepared.bytes, bytes);
    assert_eq!(prepared.mime_type, "image/png");
  }

  #[test]
  fn test_prepare_image_scales_into_limits() {
    let large = encode_image(2000, 1000, image::ImageFormat::Jpeg);
    let prepared = prepare_image(large).unwrap();
    assert_eq!(prepared.mime_type, "image/jpeg");
    assert_eq!(image_size(&prepared.bytes), (1568, 784));

    let small = encode_image(100, 50, image::ImageFormat::Png);
    let prepared = prepare_image(small).unwrap();
    assert_eq!(prepared.mime_type, "image/png");
    assert_eq!(image_size(&prepared.bytes), (512, 256));
  }

  #[test]
  fn test_prepare_image_converts_other_formats_to_png() {
    let gif = encode_image(400, 300, image::ImageFormat::Gif);
    let prepared = prepare_image(gif).unwrap();
    assert_eq!(prepared.mime_type, "image/png");
    assert_eq!(prepared.extension, "png");
    assert_eq!(image_size(&prepared.bytes), (400, 300));

    let asset =
      image_asset("anim.gif", encode_image(400, 300, image::ImageFormat::Gif))
        .unwrap();
    assert_eq!(asset.file_name, "anim.png");
  }

  #[test]
  fn test_build_state_with_images() {
    let door = png("door=door.png");
    let reference = door.reference.clone();
    let images = [door];

    assert_eq!(
      build_state("", &images).unwrap(),
      json!({ "door": reference })
    );
    assert_eq!(
      build_state("Who rang?", &images).unwrap(),
      json!({ "text": "Who rang?", "door": reference })
    );
    assert_eq!(
      build_state("[1, 2]", &images).unwrap(),
      json!({ "text": [1, 2], "door": reference })
    );
    assert_eq!(
      build_state(r#"{"time": "9:00"}"#, &images).unwrap(),
      json!({ "time": "9:00", "door": reference })
    );

    let err = build_state(r#"{"door": 1}"#, &images).unwrap_err();
    assert!(err.contains("`door` is already a key"), "{err}");
    let err = build_state("x", &[png("text=a.png")]).unwrap_err();
    assert!(err.contains("`text` is already a key"), "{err}");
  }

  #[test]
  fn test_request_body_lists_each_image_once_as_asset() {
    let images = [png("before=a.png"), png("after=b.png")];
    let reference = images[0].reference.clone();
    let questions = single("Did it change?", JevQuestion::Noul);
    let state = build_state("", &images).unwrap();
    let body = jev_request_body("jev-latest", state, &questions, &images);

    assert_eq!(
      body["assets"],
      json!({ reference.clone(): { "type": "image", "part": reference } })
    );
    assert!(needs_images_header(&questions, &images));
    assert!(!needs_images_header(&questions, &[]));

    let body = jev_request_body("jev-latest", json!("x"), &questions, &[]);
    assert!(body.get("assets").is_none());
  }

  #[test]
  fn test_box_request_body_and_default_model() {
    let questions = single("the mug in `photo`", JevQuestion::Box);
    let body = jev_request_body("jev-preview", json!("x"), &questions, &[]);
    assert_eq!(
      body["questions"]["answer"],
      json!({ "type": "box", "instructions": "the mug in `photo`" })
    );
    assert!(needs_images_header(&questions, &[]));
    assert_eq!(default_jev_model(&questions), DEFAULT_JEV_BOX_MODEL);
    assert_eq!(
      default_jev_model(&single("?", JevQuestion::Noul)),
      DEFAULT_JEV_MODEL
    );
  }

  #[test]
  fn test_box_cli_flag() {
    use clap::{Args, FromArgMatches};
    let cmd = JevQuestions::augment_args(clap::Command::new("jev"));
    let matches = cmd
      .try_get_matches_from(["jev", "--noul", "n=?", "--box", "mug=the mug"])
      .unwrap();
    let questions = JevQuestions::from_arg_matches(&matches).unwrap();
    assert_eq!(questions.0[1].name, "mug");
    assert_eq!(questions.0[1].instructions, "the mug");
    assert_eq!(questions.0[1].question, JevQuestion::Box);
  }

  #[test]
  fn test_format_box_answer() {
    let answer = json!({
      "type": "box",
      "box": { "top": 0.25, "bottom": 0.82, "left": 0.44, "right": 0.84 },
      "confidence": 0.92
    });
    assert_eq!(format_answer(&answer, true), "0.44 0.25 0.84 0.82");

    // Without the image's size, e.g. when asked via `cai jev --box`
    let formatted = format_answer(&answer, false);
    let lines = formatted.lines().collect::<Vec<_>>();
    assert_eq!(
      lines[..4],
      [
        "left        0.44",
        "top         0.25",
        "right       0.84",
        "bottom      0.82"
      ]
    );
    assert!(lines[4].contains("confidence  0.92"), "{}", lines[4]);

    let formatted = format_box_answer(&answer, Some((800, 600)));
    let lines = formatted.lines().collect::<Vec<_>>();
    assert_eq!(
      lines[..4],
      [
        "left        0.44    352 px",
        "top         0.25    150 px",
        "right       0.84    672 px",
        "bottom      0.82    492 px"
      ]
    );

    // Single line in the table of several answers
    assert_eq!(
      answer_value(&answer, false),
      "left 0.44, top 0.25, right 0.84, bottom 0.82"
    );
  }

  #[test]
  fn test_draw_box_outlines_the_edges() {
    let mut image = RgbaImage::from_pixel(400, 400, Rgba([0, 0, 0, 255]));
    let edges =
      json!({ "left": 0.25, "top": 0.5, "right": 0.75, "bottom": 1.0 });
    draw_box(&mut image, &edges);

    // Thickness is 2 px for a 400 px image
    for (x, y) in [(100, 200), (101, 300), (299, 250), (200, 399), (200, 201)] {
      assert_eq!(*image.get_pixel(x, y), BOX_COLOR, "({x}, {y})");
    }
    for (x, y) in [(99, 200), (102, 300), (200, 300), (200, 199), (300, 250)] {
      assert_eq!(*image.get_pixel(x, y), Rgba([0, 0, 0, 255]), "({x}, {y})");
    }
  }
}
