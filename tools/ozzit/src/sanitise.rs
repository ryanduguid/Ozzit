//! Strip the parts Excel adds on save and rebuild the archive deterministically.
//!
//! An Excel save adds printer settings, the save path, always-calculate flags,
//! empty relationship parts, rewritten content types, session state and
//! `_xlfn._LONGTEXT` splits of long literals. All of it is removed or pinned and
//! the archive is rewritten sorted at a fixed timestamp, so 2 saves of the same
//! content produce the same bytes. No formula or cached value is evaluated.
use crate::{archive::*, common::*, py};
use std::path::Path;

const CUSTOMPROP_CT: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.customProperty";
const FIXED_STAMP: &str = "2026-01-01T00:00:00Z";
/// The Excel window every save records: maximised at the origin, a common size.
const FIXED_WINDOW: &str = r#"xWindow="0" yWindow="0" windowWidth="28800" windowHeight="16000""#;
/// Excel's own split of a string literal over 255 characters.
const LONGTEXT: &str = r#"_xlfn\._LONGTEXT\(("(?:[^"]|"")*"(?:,"(?:[^"]|"")*")+)\)"#;

/// A pattern with Python's `\b`, which follows a word character in every use here.
fn bounded(pattern: &str) -> String {
    pattern.replace(r"\b", &py::word_boundary())
}

/// `parts[name].decode("utf-8")`; a missing part fails as Python's `KeyError`.
fn part(parts: &Parts, name: &str) -> Result<String> {
    decode(parts.get(name).ok_or_else(|| py::repr_str(name))?)
}

fn count(pattern: &str, text: &str) -> usize {
    re(pattern).find_iter(text).count()
}

/// Join each `_xlfn._LONGTEXT("a","b")` back into `"ab"`; return the text and the count.
pub fn fold_longtext(text: &str) -> (String, usize) {
    let folded = count(LONGTEXT, text);
    let text = sub_fn(LONGTEXT, text, |m| {
        let joined = captures(r#""((?:[^"]|"")*)""#, &m[1])
            .into_iter()
            .map(|c| c[1].clone())
            .collect::<String>();
        format!("\"{joined}\"")
    });
    (text, folded)
}

/// `rb"<Relationships[^>]*>\s*</Relationships>"`, searched in the raw bytes.
fn empty_relationships(bytes: &[u8]) -> bool {
    const OPEN: &[u8] = b"<Relationships";
    const CLOSE: &[u8] = b"</Relationships>";
    (0..bytes.len()).any(|i| {
        if !bytes[i..].starts_with(OPEN) {
            return false;
        }
        let Some(end) = bytes[i + OPEN.len()..].iter().position(|&b| b == b'>') else {
            return false;
        };
        let rest = &bytes[i + OPEN.len() + end + 1..];
        let body = rest
            .iter()
            .position(|b| !b" \t\n\r\x0b\x0c".contains(b))
            .unwrap_or(rest.len());
        rest[body..].starts_with(CLOSE)
    })
}

/// Sanitise the workbook in place and return the log of what changed.
pub fn sanitise(workbook: &Path) -> Result<Vec<String>> {
    let mut parts = read_parts(workbook)?;
    let mut log = Vec::new();

    // Printer settings: parts, then the relationships, then the pageSetup
    // attributes that referenced them.
    let printers = parts
        .keys()
        .filter(|n| n.starts_with("xl/printerSettings/"))
        .cloned()
        .collect::<Vec<_>>();
    for name in &printers {
        parts.shift_remove(name);
    }
    if !printers.is_empty() {
        log.push(format!("removed {} printerSettings parts", printers.len()));
    }
    for name in parts
        .keys()
        .filter(|n| n.ends_with(".rels"))
        .cloned()
        .collect::<Vec<_>>()
    {
        let text = part(&parts, &name)?;
        let new = sub(
            r#"<Relationship[^>]*Target="[^"]*printerSettings[^"]*"[^>]*/>"#,
            &text,
            "",
        );
        if new != text {
            parts.insert(name, new.into_bytes());
        }
    }

    let sheets = parts
        .keys()
        .filter(|n| n.starts_with("xl/worksheets/sheet"))
        .cloned()
        .collect::<Vec<_>>();
    let mut cleared = 0;
    for name in &sheets {
        let text = part(&parts, name)?;
        if !text.contains("r:id=") {
            continue;
        }
        let new = sub_fn(
            &bounded(r#"(<pageSetup\b[^>]*?)\s+r:id="[^"]*""#),
            &text,
            |m| m[1].to_string(),
        );
        if new != text {
            parts.insert(name.clone(), new.into_bytes());
            cleared += 1;
        }
    }
    if cleared > 0 {
        log.push(format!("cleared pageSetup r:id on {cleared} sheets"));
    }

    // Content types: printer Overrides gone, .bin default back at custom
    // properties, per-part customProperty Overrides dropped.
    let types = "[Content_Types].xml";
    let ct = part(&parts, types)?;
    let before = py::char_len(&ct);
    let mut ct = sub(
        r#"<Override[^>]*PartName="/xl/printerSettings/[^"]*"[^>]*/>"#,
        &ct,
        "",
    );
    if ct.contains(r#"<Default Extension="bin""#) {
        ct = sub(
            r#"<Default Extension="bin"[^>]*/>"#,
            &ct,
            &format!(r#"<Default Extension="bin" ContentType="{CUSTOMPROP_CT}"/>"#),
        );
        ct = sub(
            r#"<Override[^>]*PartName="/xl/customProperty\d+\.bin"[^>]*/>"#,
            &ct,
            "",
        );
    }
    let after = py::char_len(&ct);
    parts.insert(types.into(), ct.into_bytes());
    if after != before {
        log.push(format!("{types} {before} -> {after} bytes"));
    }

    // The save path.
    let wb = part(&parts, BOOK)?;
    let absolute = count(r"<x15ac:absPath[^>]*/>", &wb);
    if absolute > 0 {
        let wb = sub(r"<x15ac:absPath[^>]*/>", &wb, "");
        let wb = sub(
            r"<mc:AlternateContent[^>]*>\s*<mc:Choice[^>]*>\s*</mc:Choice>\s*</mc:AlternateContent>",
            &wb,
            "",
        );
        parts.insert(BOOK.into(), wb.into_bytes());
        log.push(format!("removed {absolute} x15ac:absPath"));
    }

    // Long string literals Excel split on save.
    let (wb, folded) = fold_longtext(&part(&parts, BOOK)?);
    if folded > 0 {
        parts.insert(BOOK.into(), wb.into_bytes());
        log.push(format!(
            "folded {folded} _LONGTEXT literal(s) in defined names"
        ));
    }

    // Always-calculate flags belong on CELL() formulas and nowhere else.
    let cell = bounded(r"(?s)<c\b(?:(?!</c>|<c\b).)*?</c>|<c\b[^>]*/>");
    let mut flags = 0;
    for name in &sheets {
        let text = part(&parts, name)?;
        if !text.contains(r#"ca="1""#) && !text.contains(r#"aca="1""#) {
            continue;
        }
        let mut hits = 0;
        let new = sub_fn(&cell, &text, |m| {
            let cell = &m[0];
            if cell.contains("CELL(") || !cell.contains(r#"ca="1""#) && !cell.contains(r#"aca="1""#)
            {
                return cell.to_string();
            }
            hits += 1;
            cell.replace(r#" aca="1""#, "").replace(r#" ca="1""#, "")
        });
        if hits > 0 {
            parts.insert(name.clone(), new.into_bytes());
            flags += hits;
        }
    }
    if flags > 0 {
        log.push(format!("cleared always-calculate flags on {flags} cells"));
    }

    // Session state: who saved, when, from what window, with which build.
    let mut session = 0;
    let core = part(&parts, "docProps/core.xml")?;
    // The last editor becomes the creator when the file names one and is dropped
    // when it does not; the save time is pinned either way.
    let editor = captures(r"<dc:creator>([^<]*)</dc:creator>", &core)
        .first()
        .map_or(String::new(), |c| {
            format!("<cp:lastModifiedBy>{}</cp:lastModifiedBy>", c[1])
        });
    let new_core = sub(
        r"<cp:lastModifiedBy>[^<]*</cp:lastModifiedBy>",
        &core,
        &editor,
    );
    let new_core = sub_fn(
        r#"(<dcterms:modified xsi:type="dcterms:W3CDTF">)[^<]*(</dcterms:modified>)"#,
        &new_core,
        |m| format!("{}{FIXED_STAMP}{}", &m[1], &m[2]),
    );
    if new_core != core {
        parts.insert("docProps/core.xml".into(), new_core.into_bytes());
        session += 1;
    }
    let wb = part(&parts, BOOK)?;
    let mut new_wb = sub(&bounded(r"<xr:revisionPtr\b[^>]*/>"), &wb, "");
    let view = re(&bounded(r"<workbookView\b[^>]*>"));
    if let Some(found) = view.find(&new_wb).ok().flatten() {
        let tag = sub(
            r#"\s+(?:xWindow|yWindow|windowWidth|windowHeight)="[^"]*""#,
            found.as_str(),
            "",
        )
        .replacen("<workbookView", &format!("<workbookView {FIXED_WINDOW}"), 1);
        new_wb.replace_range(found.range(), &tag);
    }
    let new_wb = sub_fn(
        &bounded(r#"(<fileVersion\b[^>]*?)\s+rupBuild="[^"]*""#),
        &new_wb,
        |m| m[1].to_string(),
    );
    if new_wb != wb {
        parts.insert(BOOK.into(), new_wb.into_bytes());
        session += 1;
    }
    if session > 0 {
        log.push(format!("pinned session state in {session} part(s)"));
    }

    // Rels parts with nothing left in them.
    let empty = parts
        .iter()
        .filter(|(n, b)| n.starts_with("xl/worksheets/_rels/") && empty_relationships(b))
        .map(|(n, _)| n.clone())
        .collect::<Vec<_>>();
    for name in &empty {
        parts.shift_remove(name);
    }
    if !empty.is_empty() {
        log.push(format!("dropped {} empty worksheet rels", empty.len()));
    }

    let canonical = deterministic_bytes(&parts)?;
    if log.is_empty() && read_bytes(workbook)? == canonical {
        return Ok(vec!["already clean".into()]);
    }
    if log.is_empty() {
        log.push("canonicalised archive metadata and compression".into());
    }
    replace_atomically(workbook, &canonical)?;
    Ok(log)
}

/// `f"{n:,}"`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn command(args: &[String]) -> Output {
    let [workbook] = args else {
        return Output::stderr("FAIL: usage: ozzit sanitise_workbook ozzit.xlsx");
    };
    let shown = py::PyPath::new(workbook);
    let path = shown.fs();
    if !path.is_file() {
        return Output::stderr(format!("FAIL: no such workbook: {shown}"));
    }
    let log = match sanitise(&path) {
        Ok(log) => log,
        Err(e) => return Output::stderr(format!("FAIL: cannot sanitise {shown}: {e}")),
    };
    let size = match std::fs::metadata(&path) {
        Ok(meta) => meta.len(),
        Err(e) => return Output::failure(format!("FAIL: {}", io_error(&e, &path))),
    };
    let mut out = log;
    out.push(format!("OK: {shown} sanitised, {} bytes", thousands(size)));
    Output::ok(out)
}
