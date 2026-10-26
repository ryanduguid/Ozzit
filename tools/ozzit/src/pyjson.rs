//! JSON read and written as the original builder's `json` module did.
//!
//! Records are read with Python's C scanner semantics, so a refusal names the
//! same position. A repeated key keeps its first position and last value, as
//! Python's default dictionary constructor does. Output matches
//! `json.dumps(..., indent=2, ensure_ascii=False)`.

use num_bigint::BigInt;

use crate::py;

/// A decoded JSON value, keeping object key order.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(BigInt),
    Float(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn type_name(&self) -> &'static str {
        match self {
            Json::Null => "NoneType",
            Json::Bool(_) => "bool",
            Json::Int(_) => "int",
            Json::Float(_) => "float",
            Json::Str(_) => "str",
            Json::Array(_) => "list",
            Json::Object(_) => "dict",
        }
    }
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        match self {
            Json::Object(pairs) => pairs.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn str_obj(value: &str) -> Json {
        Json::Str(value.to_string())
    }

    /// `repr(value)` of the Python object.
    pub fn repr(&self) -> String {
        match self {
            Json::Null => "None".into(),
            Json::Bool(true) => "True".into(),
            Json::Bool(false) => "False".into(),
            Json::Int(i) => py::int_repr(i).unwrap_or_else(|e| e),
            Json::Float(f) => py::float_repr(*f),
            Json::Str(s) => py::repr_str(s),
            Json::Array(items) => format!(
                "[{}]",
                items.iter().map(Json::repr).collect::<Vec<_>>().join(", ")
            ),
            Json::Object(pairs) => format!(
                "{{{}}}",
                pairs
                    .iter()
                    .map(|(k, v)| format!("{}: {}", py::repr_str(k), v.repr()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    /// `str(value)` of the Python object.
    pub fn py_str(&self) -> String {
        match self {
            Json::Str(s) => s.clone(),
            other => other.repr(),
        }
    }

    /// Python truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            Json::Null => false,
            Json::Bool(b) => *b,
            Json::Int(i) => i != &BigInt::from(0),
            Json::Float(f) => *f != 0.0,
            Json::Str(s) => !s.is_empty(),
            Json::Array(items) => !items.is_empty(),
            Json::Object(pairs) => !pairs.is_empty(),
        }
    }
}

/// Why a JSON text could not be read, rendered as Python's exception.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodeError(pub String);

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn decode_error(msg: &str, doc: &[char], pos: usize) -> DecodeError {
    let lineno = doc[..pos].iter().filter(|c| **c == '\n').count() + 1;
    let colno = match doc[..pos].iter().rposition(|c| *c == '\n') {
        Some(i) => pos - i,
        None => pos + 1,
    };
    DecodeError(format!("{msg}: line {lineno} column {colno} (char {pos})"))
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

enum Frame {
    Array(Vec<Json>),
    Object(Vec<(String, Json)>, String),
}

/// `json.loads(text)` with the default dictionary constructor.
pub fn loads(text: &str) -> Result<Json, DecodeError> {
    load(text, None)
}

/// `json.loads` with the retained evidence's unique-object-key hook.
pub fn loads_unique(text: &str) -> Result<Json, DecodeError> {
    load(
        text,
        Some(&|_| DecodeError("duplicate evidence key".into())),
    )
}

/// Why a JSON text with unique object keys could not be read.
#[derive(Debug, Clone, PartialEq)]
pub enum UniqueError {
    Decode(DecodeError),
    /// The first repeated member name of the first object that repeats one.
    Duplicate(String),
}

/// `json.loads` with a hook that raises on the first repeated member name.
pub fn loads_unique_keys(text: &str) -> Result<Json, UniqueError> {
    let repeated = std::cell::RefCell::new(None);
    load(
        text,
        Some(&|key| {
            *repeated.borrow_mut() = Some(key.to_string());
            DecodeError(String::new())
        }),
    )
    .map_err(|e| match repeated.take() {
        Some(key) => UniqueError::Duplicate(key),
        None => UniqueError::Decode(e),
    })
}

/// What follows a finished value: the whole document, or another value to scan.
enum Attached {
    Done(Json),
    Next,
}

/// The decoder's state: the text, the scan position and the open containers.
struct Parser<'a> {
    s: Vec<char>,
    idx: usize,
    stack: Vec<Frame>,
    unique: Option<&'a dyn Fn(&str) -> DecodeError>,
}

impl Parser<'_> {
    fn skip(&self, mut i: usize) -> usize {
        while i < self.s.len() && is_ws(self.s[i]) {
            i += 1;
        }
        i
    }

    fn at(&self, c: char) -> bool {
        self.idx < self.s.len() && self.s[self.idx] == c
    }

    fn error(&self, msg: &str, pos: usize) -> DecodeError {
        decode_error(msg, &self.s, pos)
    }

    /// Reads a key and its delimiter, leaving idx at the value.
    fn read_key(&mut self) -> Result<String, DecodeError> {
        if !self.at('"') {
            return Err(self.error(
                "Expecting property name enclosed in double quotes",
                self.idx,
            ));
        }
        let (key, next) = scanstring(&self.s, self.idx + 1)?;
        self.idx = self.skip(next);
        if !self.at(':') {
            return Err(self.error("Expecting ':' delimiter", self.idx));
        }
        self.idx = self.skip(self.idx + 1);
        Ok(key)
    }

    /// Scans one value starting at idx; `None` when it opened a container
    /// whose members follow.
    fn value(&mut self) -> Result<Option<Json>, DecodeError> {
        if self.idx >= self.s.len() {
            return Err(self.error("Expecting value", self.idx));
        }
        // shortcut: cap input at 1,000 containers while formatting and value
        // destruction are recursive. Replace those consumers before raising it.
        if matches!(self.s[self.idx], '{' | '[') && self.stack.len() >= 1000 {
            return Err(DecodeError(
                "JSON nesting exceeds the native limit of 1000".into(),
            ));
        }
        Ok(Some(match self.s[self.idx] {
            '"' => {
                let (string, next) = scanstring(&self.s, self.idx + 1)?;
                self.idx = next;
                Json::Str(string)
            }
            '{' => {
                self.idx = self.skip(self.idx + 1);
                if !self.at('}') {
                    let key = self.read_key()?;
                    self.stack.push(Frame::Object(Vec::new(), key));
                    return Ok(None);
                }
                self.idx += 1;
                Json::Object(Vec::new())
            }
            '[' => {
                self.idx = self.skip(self.idx + 1);
                if !self.at(']') {
                    self.stack.push(Frame::Array(Vec::new()));
                    return Ok(None);
                }
                self.idx += 1;
                Json::Array(Vec::new())
            }
            _ => self.literal()?,
        }))
    }

    /// A constant or a number at idx.
    fn literal(&mut self) -> Result<Json, DecodeError> {
        let idx = self.idx;
        let literal = |word: &str| {
            self.s[idx..]
                .iter()
                .take(word.chars().count())
                .copied()
                .eq(word.chars())
        };
        let (value, width) = if literal("null") {
            (Json::Null, 4)
        } else if literal("true") {
            (Json::Bool(true), 4)
        } else if literal("false") {
            (Json::Bool(false), 5)
        } else if literal("NaN") {
            (Json::Float(f64::NAN), 3)
        } else if literal("Infinity") {
            (Json::Float(f64::INFINITY), 8)
        } else if literal("-Infinity") {
            (Json::Float(f64::NEG_INFINITY), 9)
        } else {
            let (number, next) =
                match_number(&self.s, idx).ok_or_else(|| self.error("Expecting value", idx))??;
            self.idx = next;
            return Ok(number);
        };
        self.idx += width;
        Ok(value)
    }

    /// After a container member: true when `close` ends the container, false
    /// after a comma that another member follows.
    fn after_member(&mut self, close: char, what: &str) -> Result<bool, DecodeError> {
        self.idx = self.skip(self.idx);
        if self.at(close) {
            self.idx += 1;
            return Ok(true);
        }
        if !self.at(',') {
            return Err(self.error("Expecting ',' delimiter", self.idx));
        }
        let comma = self.idx;
        self.idx = self.skip(self.idx + 1);
        if self.at(close) {
            return Err(self.error(
                &format!("Illegal trailing comma before end of {what}"),
                comma,
            ));
        }
        Ok(false)
    }

    /// Attaches a finished value to its container, closing any that end here.
    fn attach(&mut self, mut value: Json) -> Result<Attached, DecodeError> {
        loop {
            let Some(frame) = self.stack.last_mut() else {
                let end = self.skip(self.idx);
                if end != self.s.len() {
                    return Err(self.error("Extra data", end));
                }
                return Ok(Attached::Done(value));
            };
            match frame {
                Frame::Array(items) => {
                    items.push(value);
                    if !self.after_member(']', "array")? {
                        return Ok(Attached::Next);
                    }
                    let Some(Frame::Array(items)) = self.stack.pop() else {
                        unreachable!()
                    };
                    value = Json::Array(items);
                }
                Frame::Object(pairs, key) => {
                    if self.unique.is_some() {
                        pairs.push((std::mem::take(key), value));
                    } else if let Some((_, held)) = pairs.iter_mut().find(|(k, _)| k == key) {
                        *held = value;
                    } else {
                        pairs.push((std::mem::take(key), value));
                    }
                    if !self.after_member('}', "object")? {
                        let next_key = self.read_key()?;
                        if let Some(Frame::Object(_, key)) = self.stack.last_mut() {
                            *key = next_key;
                        }
                        return Ok(Attached::Next);
                    }
                    let Some(Frame::Object(pairs, _)) = self.stack.pop() else {
                        unreachable!()
                    };
                    if let Some(duplicate) = self.unique {
                        let mut keys = std::collections::HashSet::new();
                        if let Some((key, _)) = pairs.iter().find(|(key, _)| !keys.insert(key)) {
                            return Err(duplicate(key));
                        }
                    }
                    value = Json::Object(pairs);
                }
            }
        }
    }
}

fn load(text: &str, unique: Option<&dyn Fn(&str) -> DecodeError>) -> Result<Json, DecodeError> {
    let s: Vec<char> = text.chars().collect();
    if s.first() == Some(&'\u{feff}') {
        return Err(decode_error(
            "Unexpected UTF-8 BOM (decode using utf-8-sig)",
            &s,
            0,
        ));
    }
    let mut parser = Parser {
        s,
        idx: 0,
        stack: Vec::new(),
        unique,
    };
    parser.idx = parser.skip(0);
    loop {
        if let Some(value) = parser.value()?
            && let Attached::Done(json) = parser.attach(value)?
        {
            return Ok(json);
        }
    }
}

/// The end of an exponent starting at `idx`, when one does.
fn exponent_end(s: &[char], mut idx: usize) -> Option<usize> {
    let len = s.len();
    if !(idx + 1 < len && (s[idx] == 'e' || s[idx] == 'E')) {
        return None;
    }
    idx += 1;
    if idx + 1 < len && (s[idx] == '-' || s[idx] == '+') {
        idx += 1;
    }
    while idx < len && s[idx].is_ascii_digit() {
        idx += 1;
    }
    s[idx - 1].is_ascii_digit().then_some(idx)
}

/// The C scanner's number match; None where it raises StopIteration.
fn match_number(s: &[char], start: usize) -> Option<Result<(Json, usize), DecodeError>> {
    let len = s.len();
    let digit = |i: usize| i < len && s[i].is_ascii_digit();
    let mut idx = start;
    if s[idx] == '-' {
        idx += 1;
        if idx >= len {
            return None;
        }
    }
    if ('1'..='9').contains(&s[idx]) {
        idx += 1;
        while digit(idx) {
            idx += 1;
        }
    } else if s[idx] == '0' {
        idx += 1;
    } else {
        return None;
    }
    let mut is_float = false;
    if idx + 1 < len && s[idx] == '.' && s[idx + 1].is_ascii_digit() {
        is_float = true;
        idx += 2;
        while digit(idx) {
            idx += 1;
        }
    }
    if let Some(end) = exponent_end(s, idx) {
        is_float = true;
        idx = end;
    }
    let text: String = s[start..idx].iter().collect();
    if is_float {
        let value: f64 = text.parse().expect("a JSON float literal");
        Some(Ok((Json::Float(value), idx)))
    } else {
        Some(
            py::parse_int(&text, 10)
                .map(|i| (Json::Int(i), idx))
                .map_err(DecodeError),
        )
    }
}

/// The character a `\uXXXX` escape names, joining a surrogate pair, and the
/// index after it; `next` is the index of the first hex digit.
fn unicode_escape(s: &[char], next: usize) -> Result<(char, usize), DecodeError> {
    let len = s.len();
    let mut end = next + 4;
    if end >= len {
        return Err(decode_error("Invalid \\uXXXX escape", s, next - 1));
    }
    let hex = |from: usize, end: usize| -> Result<u32, DecodeError> {
        let mut value = 0;
        for &digit in &s[from..from + 4] {
            value = value << 4
                | digit
                    .to_digit(16)
                    .ok_or_else(|| decode_error("Invalid \\uXXXX escape", s, end - 5))?;
        }
        Ok(value)
    };
    let mut code = hex(next, end)?;
    if (0xd800..=0xdbff).contains(&code) && end + 6 < len && s[end] == '\\' && s[end + 1] == 'u' {
        let low = hex(end + 2, end + 6)?;
        if (0xdc00..=0xdfff).contains(&low) {
            code = 0x10000 + (((code - 0xd800) << 10) | (low - 0xdc00));
            end += 6;
        }
    }
    match char::from_u32(code) {
        Some(ch) => Ok((ch, end)),
        // shortcut: Python keeps a lone surrogate in the string, and the
        // builder could never write it out; it is refused at the escape.
        // Use a surrogate-preserving string type if a real input carries one.
        None => Err(decode_error(
            "Unpaired surrogate in \\uXXXX escape",
            s,
            next - 2,
        )),
    }
}

/// The C `scanstring`: `end` is the index after the opening quote.
fn scanstring(s: &[char], end: usize) -> Result<(String, usize), DecodeError> {
    let len = s.len();
    let begin = end - 1;
    let mut out = String::new();
    let mut end = end;
    loop {
        let mut next = end;
        let mut c = '\0';
        while next < len {
            c = s[next];
            if c == '"' || c == '\\' {
                break;
            }
            if (c as u32) <= 0x1f {
                return Err(decode_error("Invalid control character at", s, next));
            }
            next += 1;
        }
        if next >= len || c != '"' && c != '\\' {
            return Err(decode_error("Unterminated string starting at", s, begin));
        }
        out.extend(&s[end..next]);
        next += 1;
        if c == '"' {
            return Ok((out, next));
        }
        if next == len {
            return Err(decode_error("Unterminated string starting at", s, begin));
        }
        let c = s[next];
        if c != 'u' {
            end = next + 1;
            let replacement = match c {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\x08',
                'f' => '\x0c',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => return Err(decode_error("Invalid \\escape", s, end - 2)),
            };
            out.push(replacement);
            continue;
        }
        let (ch, after) = unicode_escape(s, next + 1)?;
        out.push(ch);
        end = after;
    }
}

fn dump_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            _ if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out.push('"');
}

/// A line break and indent before an item or a closing bracket, unless compact.
fn break_line(level: usize, compact: bool, out: &mut String) {
    if !compact {
        out.push('\n');
        out.push_str(&"  ".repeat(level));
    }
}

fn dump_array(items: &[Json], level: usize, sort_keys: bool, compact: bool, out: &mut String) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        break_line(level + 1, compact, out);
        dump_into(item, level + 1, sort_keys, compact, out);
    }
    break_line(level, compact, out);
    out.push(']');
}

fn dump_object(
    pairs: &[(String, Json)],
    level: usize,
    sort_keys: bool,
    compact: bool,
    out: &mut String,
) {
    let mut order: Vec<&(String, Json)> = pairs.iter().collect();
    if sort_keys {
        order.sort_by(|a, b| a.0.cmp(&b.0));
    }
    out.push('{');
    for (i, (k, v)) in order.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        break_line(level + 1, compact, out);
        dump_str(k, out);
        out.push(':');
        if !compact {
            out.push(' ');
        }
        dump_into(v, level + 1, sort_keys, compact, out);
    }
    break_line(level, compact, out);
    out.push('}');
}

fn dump_into(value: &Json, level: usize, sort_keys: bool, compact: bool, out: &mut String) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Int(i) => out.push_str(&i.to_string()),
        Json::Float(f) if f.is_nan() => out.push_str("NaN"),
        Json::Float(f) if f.is_infinite() => {
            out.push_str(if *f > 0.0 { "Infinity" } else { "-Infinity" })
        }
        Json::Float(f) => out.push_str(&py::float_repr(*f)),
        Json::Str(s) => dump_str(s, out),
        Json::Array(items) if items.is_empty() => out.push_str("[]"),
        Json::Array(items) => dump_array(items, level, sort_keys, compact, out),
        Json::Object(pairs) if pairs.is_empty() => out.push_str("{}"),
        Json::Object(pairs) => dump_object(pairs, level, sort_keys, compact, out),
    }
}

/// `json.dumps(value, indent=2, ensure_ascii=False, sort_keys=...)`.
pub fn dumps(value: &Json, sort_keys: bool) -> String {
    let mut out = String::new();
    dump_into(value, 0, sort_keys, false, &mut out);
    out
}

/// `json.dumps(value, ensure_ascii=False, separators=(",", ":"))`.
pub fn dumps_compact(value: &Json) -> String {
    let mut out = String::new();
    dump_into(value, 0, false, true, &mut out);
    out
}

/// `json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)`.
pub fn dumps_sorted_compact(value: &Json) -> String {
    let mut out = String::new();
    dump_into(value, 0, true, true, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(text: &str) -> String {
        loads(text).unwrap_err().0
    }

    #[test]
    fn errors_match_the_c_scanner() {
        assert_eq!(
            err("{\"a\":1,}"),
            "Illegal trailing comma before end of object: line 1 column 7 (char 6)"
        );
        assert_eq!(
            err("[1,]"),
            "Illegal trailing comma before end of array: line 1 column 3 (char 2)"
        );
        assert_eq!(
            err("\"\u{1}\""),
            "Invalid control character at: line 1 column 2 (char 1)"
        );
        assert_eq!(err("\"\\q\""), "Invalid \\escape: line 1 column 2 (char 1)");
        assert_eq!(
            err("\"\\u12\""),
            "Invalid \\uXXXX escape: line 1 column 3 (char 2)"
        );
        assert_eq!(
            err("\"abc"),
            "Unterminated string starting at: line 1 column 1 (char 0)"
        );
        assert_eq!(
            err("{\"a\" 1}"),
            "Expecting ':' delimiter: line 1 column 6 (char 5)"
        );
    }

    #[test]
    fn structure_errors_match_the_c_scanner() {
        assert_eq!(err("1 2"), "Extra data: line 1 column 3 (char 2)");
        assert_eq!(err(""), "Expecting value: line 1 column 1 (char 0)");
        assert_eq!(
            err("\u{feff}{}"),
            "Unexpected UTF-8 BOM (decode using utf-8-sig): line 1 column 1 (char 0)"
        );
        assert_eq!(
            err("[1 2]"),
            "Expecting ',' delimiter: line 1 column 4 (char 3)"
        );
        assert_eq!(
            err("{1:2}"),
            "Expecting property name enclosed in double quotes: line 1 column 2 (char 1)"
        );
        assert_eq!(err("-"), "Expecting value: line 1 column 1 (char 0)");
        assert_eq!(err("01"), "Extra data: line 1 column 2 (char 1)");
        assert_eq!(
            err("\"\\"),
            "Unterminated string starting at: line 1 column 1 (char 0)"
        );
        assert_eq!(err("{\"a\":"), "Expecting value: line 1 column 6 (char 5)");
        assert_eq!(
            err("{\"a\":1"),
            "Expecting ',' delimiter: line 1 column 7 (char 6)"
        );
        assert_eq!(err("["), "Expecting value: line 1 column 2 (char 1)");
    }

    #[test]
    fn deeply_nested_input_is_refused_before_recursive_consumers_run() {
        assert_eq!(
            err(&format!("{}0{}", "[".repeat(200_000), "]".repeat(200_000))),
            "JSON nesting exceeds the native limit of 1000"
        );
        let text = format!("{}0{}", "[".repeat(1000), "]".repeat(1000));
        let value = loads(&text).unwrap();
        assert_eq!(dumps_compact(&value), text);
    }

    #[test]
    fn duplicate_keys_keep_the_last_value_in_the_first_position() {
        let value = loads(r#"{"a":1,"b":2,"a":3}"#).unwrap();
        assert_eq!(dumps_compact(&value), r#"{"a":3,"b":2}"#);
    }

    #[test]
    fn dumps_matches_python() {
        let value = loads("{\"b\": [1, 2.5, \"\u{e9}\\n\\u0001\"], \"a\": {}, \"c\": []}").unwrap();
        assert_eq!(
            dumps(&value, false),
            "{\n  \"b\": [\n    1,\n    2.5,\n    \"\u{e9}\\n\\u0001\"\n  ],\n  \"a\": {},\n  \"c\": []\n}"
        );
        assert!(dumps(&value, true).starts_with("{\n  \"a\": {}"));
    }
}
