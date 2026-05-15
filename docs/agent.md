# Cai Agent

Config options:
- `askForPermission: always | edit | never` (default `always`) -
    Ask for permission to use tools


## List tool

List files in a directory including their modification date and size.
Results are sorted by modification time and capped at 100 files.
If the cap is hit, the agent sees a truncation flag in the result
and can use the glob tool to list a narrowed down selection of files


## Glob tool

The Glob tool finds files by name pattern.
It supports standard glob syntax including ** for recursive directory matching:
**/*.js matches all .js files at any depth
src/**/*.ts matches all .ts files under src/
*.{json,yaml} matches .json and .yaml files in the current directory
Results are sorted by modification time and capped at 100 files.
If the cap is hit, the agent sees a truncation flag in the result
and can narrow the pattern.


## Grep tool

The Grep tool searches file contents for patterns.
Where Glob finds files by name, Grep finds lines inside them.
Grep is built on ripgrep and uses ripgrep’s regex syntax, not POSIX grep.
Patterns that include regex metacharacters need escaping.
For example, finding interface{} in Go code takes the pattern interface\{\}.
Three output modes control what comes back:
files_with_matches: file paths only, no line content.
This is the default.
content: matching lines with file and line number.
count: match count per file.
the agent can scope results by file with the glob parameter,
such as **/*.tsx, or by language with the type parameter, such as py or rust.
By default, patterns match within a single line.
the agent can set multiline: true to match across line boundaries.
Grep respects .gitignore, so gitignored files are skipped.
To search a gitignored file, the agent passes its path directly.


## Read tool

The Read tool takes a file path and returns the contents with line numbers.
the agent is instructed to always pass absolute paths.
By default, Read returns the file from the start.
Files over a size threshold return an error rather than partial content,
prompting the agent to retry with offset and limit to read a specific range.
Read handles several file types beyond plain text:
PDFs: the agent reads short .pdf files whole.
For PDFs longer than 10 pages, it reads in ranges with a pages parameter,
such as "1-5", up to 20 pages at a time.
Jupyter notebooks: .ipynb files return all cells with their outputs,
including code, markdown, and visualizations.
Read only reads files, not directories.
the agent uses the list tool to list directory contents.


## WebFetch tool

WebFetch takes a URL and a prompt describing what to extract.
It fetches the page, converts the response to Markdown when the server returns HTML,
and runs the prompt against the content using a small, fast model.
For most fetches, the agend receives that model’s answer, not the raw page.
The conversion step is not configurable.
This makes WebFetch lossy by design.
The extraction prompt determines what reaches the agent,
so a result that says a page does not mention something
may only mean the prompt did not ask about it.

A few behaviors shape the response the agent receives:
- HTTP URLs are automatically upgraded to HTTPS.
- Large pages are truncated to a fixed character limit before processing.
- Responses are cached for 15 minutes, so repeated fetches of the same URL return quickly.
- When a URL redirects to a different host, WebFetch returns a text result
    that names the original URL and the redirect target instead of following it.
    The agent then fetches the new URL with a second WebFetch call.

WebFetch sets an Accept header that prefers Markdown over HTML
so servers that support content negotiation can return Markdown directly.


## WebSearch tool

WebSearch returns result titles and URLs.
It does not fetch the result pages.
To read a page the agent finds in search results, it follows up with WebFetch.
The tool may issue up to eight searches per call,
refining the search internally before returning results.


## Edit tool

The Edit tool performs exact string replacement.
It takes an old_string and a new_string and replaces the first with the second.
It does not use regex or fuzzy matching.
Three checks must pass for an edit to apply:
Read-before-edit: the agent must have read the file in the current conversation,
and the file must not have changed on disk since that read.
This check runs first, before any string matching.
Match: old_string must appear in the file exactly as written.
A single character of whitespace or indentation difference is enough to miss.
Uniqueness: old_string must appear exactly once.
When it appears more than once, the agent either supplies a longer string
with enough surrounding context to pin down one occurrence,
or sets `replace_all: true` to replace them all.

It can only edit files that are in or below the current directory.

The agent can not create new files or directories - only edit existing ones.


## UserInstruct tool

If there is something the agent can't do on its own (e.g. create a new file),
it can instruct the user to do it and wait until the user
performed the instruction and signified it by instructing the agent to continue.
