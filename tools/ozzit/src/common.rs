//! Compatibility at the text and filesystem boundaries of the Python tools.
use crate::py;
use fancy_regex::{Captures, Regex, RegexBuilder};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

pub type Result<T> = std::result::Result<T, String>;
static REGEXES: LazyLock<Mutex<HashMap<String, Arc<Regex>>>> = LazyLock::new(Default::default);

pub fn re(pattern: &str) -> Arc<Regex> {
    let mut cache = REGEXES.lock().expect("regex cache poisoned");
    cache
        .entry(pattern.into())
        .or_insert_with(|| {
            let pattern = pattern
                .replace(r"\s", &py::S)
                .replace(r"\w", &py::W)
                .replace(r"\d", &py::D);
            // Python's `re` has no backtrack limit; a finite one makes large parts panic.
            Arc::new(
                RegexBuilder::new(&pattern)
                    .backtrack_limit(usize::MAX)
                    .build()
                    .expect("constant regular expression"),
            )
        })
        .clone()
}

/// A pattern with Python's `\b` in place of the engine's own word boundary.
pub fn bounded(pattern: &str) -> String {
    pattern.replace(r"\b", &py::word_boundary())
}

/// `re.fullmatch(pattern, text)`; `$` here is the end of the text, as in a full match.
pub fn fullmatch(pattern: &str, text: &str) -> bool {
    matches(&format!("^(?:{pattern})$"), text)
}

pub fn matches(pattern: &str, text: &str) -> bool {
    re(pattern)
        .is_match(text)
        .expect("regular expression execution")
}

pub fn captures(pattern: &str, text: &str) -> Vec<Vec<String>> {
    re(pattern)
        .captures_iter(text)
        .map(|m| {
            let m = m.expect("regular expression execution");
            (0..m.len())
                .map(|i| m.get(i).map_or("", |v| v.as_str()).to_owned())
                .collect()
        })
        .collect()
}

pub fn sub(pattern: &str, text: &str, replacement: &str) -> String {
    re(pattern)
        .replace_all(text, fancy_regex::NoExpand(replacement))
        .into_owned()
}

pub fn sub_fn(pattern: &str, text: &str, f: impl FnMut(&Captures<'_, str>) -> String) -> String {
    re(pattern).replace_all(text, f).into_owned()
}

pub fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
pub fn basename(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
pub fn io_error(error: &std::io::Error, path: &Path) -> String {
    py::oserror_str(error, &display(path), py::OsStyle::Errno)
}
pub fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| io_error(&e, path))
}
pub fn decode(bytes: &[u8]) -> Result<String> {
    py::decode_utf8(bytes).map_err(|e| e.to_string())
}
pub fn read_raw(path: &Path) -> Result<String> {
    decode(&read_bytes(path)?)
}
pub fn read_sig(path: &Path) -> Result<String> {
    let text = read_raw(path)?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}
pub fn read_text(path: &Path) -> Result<String> {
    Ok(py::universal_newlines(&read_raw(path)?))
}
pub fn txt_files(src: &Path) -> Result<Vec<PathBuf>> {
    if !src.is_dir() {
        return Ok(vec![]);
    }
    let mut paths = std::fs::read_dir(src)
        .map_err(|e| io_error(&e, src))?
        .map(|entry| entry.map(|e| e.path()).map_err(|e| io_error(&e, src)))
        .collect::<Result<Vec<_>>>()?;
    paths.retain(|p| p.extension().is_some_and(|e| e == "txt"));
    paths.sort();
    Ok(paths)
}
pub fn prefix(text: &str, n: usize) -> &str {
    py::prefix_chars(text, n)
}
pub fn char_slice(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}
pub fn count(text: &str, needle: &str) -> usize {
    text.matches(needle).count()
}
pub fn column_number(letters: &str) -> usize {
    letters.bytes().fold(0, |n, c| n * 26 + usize::from(c) - 64)
}
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
pub fn escape_attr(text: &str) -> String {
    escape(text).replace('"', "&quot;")
}

static ENTITIES: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("data/html5.json")).expect("HTML entity table")
});
pub fn unescape(text: &str) -> Result<String> {
    // html.unescape reads a decimal reference with int(), which refuses more
    // than 4,300 digits, leading zeros included.
    if let Some(m) = captures(r"&#([0-9]{4301,})", text).first() {
        return Err(py::int_digits_error(m[1].len()));
    }
    Ok(sub_fn(
        r"&(#(?:[0-9]+|[xX][0-9a-fA-F]+);?|[^\t\n\f <&#;]{1,32};?)",
        text,
        |m| {
            let s = &m[1];
            if let Some(numeric) = s.strip_prefix('#') {
                let numeric = numeric.trim_end_matches(';');
                let n = if numeric.starts_with(['x', 'X']) {
                    u32::from_str_radix(&numeric[1..], 16)
                } else {
                    numeric.parse()
                }
                .unwrap_or(u32::MAX);
                if n == 0 || n > 0x10ffff || (0xd800..=0xdfff).contains(&n) {
                    return "�".into();
                }
                if (128..=159).contains(&n) {
                    return "€\u{81}‚ƒ„…†‡ˆ‰Š‹Œ\u{8d}Ž\u{8f}\u{90}‘’“”•–—˜™š›œ\u{9d}žŸ"
                        .chars()
                        .nth((n - 128) as usize)
                        .expect("C1 table")
                        .to_string();
                }
                if matches!(n,1..=8|11|14..=31|127)
                    || (0xfdd0..=0xfdef).contains(&n)
                    || n & 0xffff >= 0xfffe
                {
                    return String::new();
                }
                return char::from_u32(n).expect("valid Unicode scalar").to_string();
            }
            if let Some(value) = ENTITIES.get(s) {
                return value.clone();
            }
            for (i, _) in s.char_indices().rev().filter(|(i, _)| *i > 0) {
                if let Some(value) = ENTITIES.get(&s[..i]) {
                    return format!("{value}{}", &s[i..]);
                }
            }
            format!("&{s}")
        },
    ))
}

/// DictReader preserves repeated columns, short rows and blank lines as Python did.
pub struct Csv {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}
/// The csv module's reader state, fed one character at a time.
#[derive(Default)]
struct CsvState {
    records: Vec<Vec<String>>,
    row: Vec<String>,
    field: String,
    quoted: bool,
    start: bool,
    touched: bool,
    length: usize,
}
impl CsvState {
    fn push_char(&mut self, c: char) {
        self.field.push(c);
        self.length += 1;
    }
    /// Ends a record; a blank line is kept as an empty record, as csv.reader yields [].
    fn end_row(&mut self, at_end: bool) {
        if self.touched || !self.row.is_empty() || !self.field.is_empty() {
            self.row.push(std::mem::take(&mut self.field));
            self.records.push(std::mem::take(&mut self.row));
        } else if !at_end {
            self.records.push(vec![]);
        }
    }
    fn step(&mut self, c: char, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
        if self.quoted {
            if c != '"' {
                self.push_char(c);
            } else if chars.next_if_eq(&'"').is_some() {
                self.push_char('"');
            } else {
                self.quoted = false;
            }
        } else if c == '"' && self.start {
            self.quoted = true;
            self.start = false;
            self.touched = true;
        } else if c == ',' {
            self.row.push(std::mem::take(&mut self.field));
            self.start = true;
            self.touched = true;
            self.length = 0;
        } else if c == '\r' || c == '\n' {
            if c == '\r' {
                chars.next_if_eq(&'\n');
            }
            self.end_row(false);
            self.touched = false;
            self.start = true;
            self.length = 0;
        } else {
            self.push_char(c);
            self.start = false;
            self.touched = true;
        }
    }
}
impl Csv {
    pub fn read(path: &Path) -> Result<Self> {
        Self::parse(&read_sig(path)?)
    }
    pub fn parse(text: &str) -> Result<Self> {
        let mut state = CsvState {
            start: true,
            ..CsvState::default()
        };
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            state.step(c, &mut chars);
            if state.length > 131072 {
                return Err("field larger than field limit (131072)".into());
            }
        }
        state.end_row(true);
        // DictReader takes the first record as the header even when it is blank,
        // then skips blank records.
        let mut records = state.records.into_iter();
        let headers = records.next().unwrap_or_default();
        Ok(Self {
            headers,
            rows: records.filter(|r| !r.is_empty()).collect(),
        })
    }
    pub fn get<'a>(&self, row: &'a [String], key: &str) -> Option<&'a str> {
        self.headers
            .iter()
            .rposition(|h| h == key)
            .and_then(|i| row.get(i))
            .map(String::as_str)
    }
    pub fn duplicates(&self, columns: &[&str]) -> Vec<String> {
        columns
            .iter()
            .filter(|&&f| self.headers.iter().filter(|h| h.as_str() == f).count() > 1)
            .map(|f| (*f).into())
            .collect()
    }
}
pub fn csv_row(fields: &[String]) -> String {
    let mut out = fields
        .iter()
        .map(|s| {
            if s.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    out.push('\n');
    out
}

pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).map_err(|e| io_error(&e, path))
}
pub fn stage_bytes(path: &Path, bytes: &[u8]) -> Result<tempfile::NamedTempFile> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::Builder::new()
        .prefix(".ozzit-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|e| io_error(&e, path))?;
    file.write_all(bytes).map_err(|e| io_error(&e, path))?;
    #[cfg(unix)]
    if let Ok(meta) = std::fs::metadata(path) {
        file.as_file()
            .set_permissions(meta.permissions())
            .map_err(|e| io_error(&e, path))?;
    }
    Ok(file)
}
pub fn replace_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    stage_bytes(path, bytes)?
        .persist(path)
        .map_err(|e| io_error(&e.error, path))?;
    Ok(())
}

pub fn resolve_target(part: &str, target: &str) -> String {
    let joined = if target.starts_with('/') {
        target.into()
    } else {
        format!("{}/{target}", part.rsplit_once('/').map_or("", |x| x.0))
    };
    let mut stack = Vec::new();
    for p in joined.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                if stack.last().is_some_and(|s| *s != "..") {
                    stack.pop();
                } else if !joined.starts_with('/') {
                    stack.push("..");
                }
            }
            _ => stack.push(p),
        }
    }
    stack.join("/")
}

#[derive(Default, Debug)]
pub struct Output {
    pub code: u8,
    pub out: Vec<String>,
    pub err: Vec<String>,
}
impl Output {
    pub fn ok(lines: Vec<String>) -> Self {
        Self {
            out: lines,
            ..Self::default()
        }
    }
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            code: 1,
            out: vec![message.into()],
            err: vec![],
        }
    }
    pub fn stderr(message: impl Into<String>) -> Self {
        Self {
            code: 1,
            out: vec![],
            err: vec![message.into()],
        }
    }
    pub fn report(failures: Vec<String>, bad: String, good: String) -> Self {
        if failures.is_empty() {
            Self::ok(vec![good])
        } else {
            let mut out = vec![bad];
            out.extend(failures.into_iter().map(|f| format!("  - {f}")));
            Self {
                code: 1,
                out,
                err: vec![],
            }
        }
    }
}
