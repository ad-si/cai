use bat::PrettyPrinter;
use textwrap::termwidth;

pub fn wrap_text(text: &str) -> String {
  if termwidth() > 100 {
    textwrap::wrap(text, 80).join("\n")
  } else {
    text.to_string()
  }
}

pub fn text_via_bat(text: &str) {
  let text_wrapped = wrap_text(text);
  PrettyPrinter::new()
    .input_from_bytes(text_wrapped.as_bytes())
    .language("markdown")
    .print()
    .unwrap();
}

// TODO: This doesn't syntax highlight code blocks
//
// use textwrap::termwidth;
// use syntect::easy::HighlightLines;
// use syntect::highlighting::{Style, ThemeSet};
// use syntect::parsing::SyntaxSet;
// use syntect::util::as_24_bit_terminal_escaped;
//
// pub fn lines_via_syntect(msg: &String) {
//   let lines = if termwidth() > 100 {
//     textwrap::wrap(&msg, 60)
//       .iter()
//       .map(|s| s.to_string())
//       .collect::<Vec<String>>()
//   } else {
//     msg
//       .split("\n")
//       .map(|s| s.to_string())
//       .collect::<Vec<String>>()
//   };
//
//   let syn_set = SyntaxSet::load_defaults_newlines();
//   let theme_set = ThemeSet::load_defaults();
//   let syntax = syn_set.find_syntax_by_extension("md").unwrap();
//   let mode = dark_light::detect();
//   let mut highlighter = HighlightLines::new(
//     syntax,
//     match mode {
//       dark_light::Mode::Default => &theme_set.themes["base16-mocha.dark"],
//       dark_light::Mode::Dark => &theme_set.themes["base16-mocha.dark"],
//       dark_light::Mode::Light => &theme_set.themes["InspiredGitHub"],
//     },
//   );
//
//   for line in lines {
//     let ranges: Vec<(Style, &str)> = highlighter //
//       .highlight_line(line, &syn_set)
//       .unwrap();
//     let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
//     print!("{}\n", escaped);
//   }
// }
