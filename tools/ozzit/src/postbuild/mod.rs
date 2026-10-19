//! The postbuild passes: deterministic, idempotent text surgery on the workbook
//! and `src/`. No COM, no recalculation; every pass writes through the
//! deterministic archive writer and reports "already applied" on a current
//! workbook.
//!
//! The passes' text tables live in `src/data/postbuild/*.json`, exported once
//! from the Python passes so that no anchor is retyped.
use crate::{archive::*, common::*, py};
use serde_json::Value;
use std::path::Path;

pub mod aasb16_leases;
pub mod cover_label;
pub mod function_pass;
pub mod fy27_help_text;
pub mod gst_help_text;
pub mod help_corrections;
pub mod help_links;
pub mod remove_residue;
pub mod sheet_names;
pub mod strip_revision_history;
pub mod workbook_palette;

pub const MODULES: [&str; 6] = crate::formula::MODULES;

/// One pass's exported tables.
pub fn data(json: &str) -> Value {
    serde_json::from_str(json).expect("embedded postbuild data")
}

pub fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("string array")
        .iter()
        .map(|v| v.as_str().expect("string").to_string())
        .collect()
}

/// `parts[name]`, failing as Python's `KeyError` does.
pub fn part<'a>(parts: &'a Parts, name: &str) -> Result<&'a [u8]> {
    parts
        .get(name)
        .map(Vec::as_slice)
        .ok_or_else(|| py::repr_str(name))
}

pub fn part_text(parts: &Parts, name: &str) -> Result<String> {
    decode(part(parts, name)?)
}

/// `str.count`.
pub fn count(text: &str, needle: &str) -> usize {
    text.matches(needle).count()
}

/// `repr(text[:n])` and `repr(text[-n:])`, as the passes quote anchors.
pub fn head_repr(text: &str, n: usize) -> String {
    py::repr_str(py::prefix_chars(text, n))
}
pub fn tail_repr(text: &str, n: usize) -> String {
    let skip = py::char_len(text).saturating_sub(n);
    py::repr_str(&text.chars().skip(skip).collect::<String>())
}

/// Each (old, new) replaced in turn, in the order listed.
pub fn apply_swaps(text: &str, pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .fold(text.to_string(), |text, (old, new)| text.replace(old, new))
}

/// `read_text` of the shared writer: UTF-8 with line endings kept.
pub fn read_text(path: &Path) -> Result<String> {
    read_raw(path)
}

pub fn write_text(path: &Path, text: &str) -> Result<()> {
    replace_atomically(path, text.as_bytes())
}

/// The standard command line of a workbook pass: `[workbook]` or `[workbook] [src dir]`.
pub struct PassArgs {
    pub workbook: py::PyPath,
    pub src: py::PyPath,
}

pub fn pass_args(
    args: &[String],
    usage: &str,
    with_src: bool,
) -> std::result::Result<PassArgs, Output> {
    if args.len() > if with_src { 2 } else { 1 } {
        return Err(Output::stderr(format!("FAIL: usage: {usage}")));
    }
    let workbook = py::PyPath::new(args.first().map_or("ozzit.xlsx", String::as_str));
    if !workbook.fs().is_file() {
        return Err(Output::stderr(format!(
            "FAIL: no such workbook: {workbook}"
        )));
    }
    Ok(PassArgs {
        workbook,
        src: py::PyPath::new(args.get(1).map_or("src", String::as_str)),
    })
}

/// The usual ending: what the pass changed, or that it was already applied.
pub fn report(
    result: Result<Vec<String>>,
    workbook: &py::PyPath,
    applied: &str,
    current: &str,
) -> Output {
    match result {
        Err(e) => Output::stderr(format!("FAIL: {e}")),
        Ok(changed) => {
            let first = if changed.is_empty() {
                format!("{current} already applied; no changes")
            } else {
                format!("applied {applied} to: {}", changed.join(", "))
            };
            Output::ok(vec![first, format!("OK: {workbook}")])
        }
    }
}

/// Sheet name to worksheet part, from the workbook and its relationships.
pub fn sheet_targets(parts: &Parts) -> Result<Vec<(String, String)>> {
    let book = part_text(parts, BOOK)?;
    let rels = part_text(parts, "xl/_rels/workbook.xml.rels")?;
    let mut targets = std::collections::HashMap::new();
    for m in captures(r#"<Relationship Id="([^"]+)"[^>]*Target="([^"]+)""#, &rels) {
        targets.insert(m[1].clone(), m[2].clone());
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for m in captures(r#"<sheet name="([^"]+)"[^>]*r:id="([^"]+)""#, &book) {
        let target = targets.get(&m[2]).ok_or_else(|| py::repr_str(&m[2]))?;
        let path = resolve_target(BOOK, target);
        match out.iter_mut().find(|(n, _)| *n == m[1]) {
            Some(entry) => entry.1 = path,
            None => out.push((m[1].clone(), path)),
        }
    }
    Ok(out)
}

pub fn lookup<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a String> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Python's `f"{text:<{width}}"`: left-aligned, padded with spaces by character count.
pub fn pad(text: &str, width: usize) -> String {
    format!(
        "{text}{}",
        " ".repeat(width.saturating_sub(py::char_len(text)))
    )
}

/// "absent" or "applied" for a pass that adds `names` (and an About `heading`).
pub fn workbook_state(
    book: &str,
    names: &[String],
    heading: Option<&str>,
    noun: &str,
) -> Result<&'static str> {
    let present = names
        .iter()
        .filter(|n| book.contains(&format!("<definedName name=\"{n}\"")))
        .count();
    let about = heading.map(|h| count(book, h));
    if present == 0 && about.is_none_or(|a| a == 0) {
        return Ok("absent");
    }
    if present == names.len() && about.is_none_or(|a| a == 1) {
        return Ok("applied");
    }
    let mut detail = format!("{present} of {} {noun}", names.len());
    if let Some(about) = about {
        detail.push_str(&format!(" and {about} About heading(s)"));
    }
    Err(format!(
        "{BOOK} holds {detail}; it is neither state this pass recognises"
    ))
}

/// "absent" or "applied" for a src module that a pass adds functions and About rows to.
pub fn source_state(
    place: &str,
    text: &str,
    functions: &[(String, String)],
) -> Result<&'static str> {
    let present = functions
        .iter()
        .filter(|(name, _)| text.contains(&format!("\n{name} = LAMBDA(")))
        .count();
    let rows = functions
        .iter()
        .filter(|(_, row)| text.contains(row.as_str()))
        .count();
    if present == 0 && rows == 0 {
        return Ok("absent");
    }
    if present == functions.len() && rows == functions.len() {
        return Ok("applied");
    }
    Err(format!(
        "{place} holds {present} of {} functions and {rows} About rows; it is neither state this pass recognises",
        functions.len()
    ))
}

/// "absent", "applied", or None when no index is supplied, raising on a partial one.
pub fn index_state(
    index: Option<&py::PyPath>,
    names: &[String],
    rows: &str,
) -> Result<Option<&'static str>> {
    let Some(index) = index else {
        return Ok(None);
    };
    if !index.fs().is_file() {
        return Err(format!(
            "{index}: no such index; supply an existing functions.csv or none"
        ));
    }
    let csv = Csv::read(&index.fs())?;
    let listed = csv
        .rows
        .iter()
        .map(|r| csv.get(r, "function").unwrap_or_default())
        .collect::<std::collections::BTreeSet<_>>();
    let present = names.iter().filter(|n| listed.contains(n.as_str())).count();
    if present == 0 {
        return Ok(Some("absent"));
    }
    if present == names.len() {
        return Ok(Some("applied"));
    }
    Err(format!(
        "{} holds {present} of {} {rows} rows; it is neither state this pass recognises",
        index.name(),
        names.len()
    ))
}

/// The module with `rows` inserted after the About row `anchor`, at its indent.
pub fn insert_about_rows(
    text: &str,
    place: &str,
    anchor: &str,
    rows: &[String],
    heading: Option<&str>,
) -> Result<String> {
    let hits = count(text, anchor);
    if hits != 1 {
        return Err(format!(
            "{place}: About anchor found {hits} times, expected 1"
        ));
    }
    let at = text.find(anchor).expect("one anchor");
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let indent = &text[line_start..at];
    let mut lines = match heading {
        Some(heading) => vec![
            format!("{indent}\"→¶\" &"),
            format!("{indent}\"{heading}¶\" &"),
        ],
        None => vec![],
    };
    lines.extend(rows.iter().map(|row| format!("{indent}{row} &")));
    Ok(text.replacen(
        &format!("{anchor} &"),
        &format!("{anchor} &\n{}", lines.join("\n")),
        1,
    ))
}
