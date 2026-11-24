// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! `ozzit audit`: a read-only review of any workbook for hard-coded numbers that
//! break a formula run, external links whose target is missing, and hidden
//! defined names and sheets. It reads the archive and writes nothing.
use crate::archive::{BOOK, Parts, read_selected_inventory};
use crate::common::{Output, Result, column_number, sub_fn};
use crate::formula::split_literals;
use crate::workbook::xml_part;
use roxmltree::{Document, Node};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// An A1 cell, whole-column or whole-row reference outside a name, a function
/// call or a structured reference: groups 1 to 4, 5 to 8 and 9 to 12.
const REFERENCE: &str = r"(?<![A-Za-z0-9_.$\[\]])(?:(\$?)([A-Z]{1,3})(\$?)([0-9]{1,7})|(\$?)([A-Z]{1,3}):(\$?)([A-Z]{1,3})|(\$?)([0-9]{1,7}):(\$?)([0-9]{1,7}))(?![A-Za-z0-9_.(\[])";
const MAX_COLUMN: usize = 16_384;
const MAX_ROW: usize = 1_048_576;

enum Cell {
    /// A formula's references in relative R1C1 form, equal for cells that
    /// follow one pattern.
    Formula(String),
    Number(String),
}

struct Sheet {
    name: String,
    state: String,
    part: Option<String>,
}

fn attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name() == name)
        .map(|a| a.value())
}

fn children<'a, 'i>(node: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(move |n| n.tag_name().name() == name)
}

fn letters(mut column: usize) -> String {
    let mut out = Vec::new();
    while column > 0 {
        column -= 1;
        out.push(b'A' + (column % 26) as u8);
        column /= 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

fn a1((row, column): (usize, usize)) -> String {
    format!("{}{row}", letters(column))
}

fn parse_a1(reference: &str) -> Option<(usize, usize)> {
    let split = reference.find(|c: char| c.is_ascii_digit())?;
    let (column, row) = reference.split_at(split);
    if column.is_empty() || !column.bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }
    Some((row.parse().ok()?, column_number(column)))
}

fn offset(absolute: bool, value: usize, host: usize, axis: char) -> String {
    if absolute {
        format!("{axis}{value}")
    } else {
        format!("{axis}[{}]", value as i64 - host as i64)
    }
}

fn row(dollar: &str, digits: &str, host: usize) -> Option<String> {
    let row = digits.parse().ok().filter(|r| (1..=MAX_ROW).contains(r))?;
    Some(offset(!dollar.is_empty(), row, host, 'R'))
}

fn column(dollar: &str, letters: &str, host: usize) -> Option<String> {
    let column = Some(column_number(letters)).filter(|&c| c <= MAX_COLUMN)?;
    Some(offset(!dollar.is_empty(), column, host, 'C'))
}

/// One matched reference in R1C1 form, or `None` past the sheet's edge.
fn reference(m: &fancy_regex::Captures<'_, str>, host: (usize, usize)) -> Option<String> {
    let group = |i: usize| m.get(i).map_or("", |g| g.as_str());
    if m.get(2).is_some() {
        Some(row(group(3), group(4), host.0)? + &column(group(1), group(2), host.1)?)
    } else if m.get(6).is_some() {
        Some(column(group(5), group(6), host.1)? + ":" + &column(group(7), group(8), host.1)?)
    } else {
        Some(row(group(9), group(10), host.0)? + ":" + &row(group(11), group(12), host.0)?)
    }
}

/// The formula with each A1 reference rewritten relative to the cell at `host`.
/// String literals and quoted sheet names stay as written.
fn relative(formula: &str, host: (usize, usize)) -> String {
    let mut out = String::new();
    for (literal, piece) in split_literals(formula) {
        if literal {
            out.push_str(piece);
            continue;
        }
        for (i, segment) in piece.split('\'').enumerate() {
            if i > 0 {
                out.push('\'');
            }
            if i % 2 == 1 {
                out.push_str(segment);
                continue;
            }
            out.push_str(&sub_fn(REFERENCE, segment, |m| {
                reference(m, host).unwrap_or_else(|| m[0].to_owned())
            }));
        }
    }
    out
}

/// Each formula and plain number cell by (row, column). A shared formula's
/// dependants take its anchor's pattern, which relative form keeps unchanged.
fn cells(doc: &Document<'_>) -> BTreeMap<(usize, usize), Cell> {
    let mut out = BTreeMap::new();
    let mut shared = HashMap::new();
    let mut row = 0;
    for row_node in doc.descendants().filter(|n| n.tag_name().name() == "row") {
        row = attr(row_node, "r")
            .and_then(|r| r.parse().ok())
            .unwrap_or(row + 1);
        let mut column = 0;
        for cell in children(row_node, "c") {
            let at = attr(cell, "r")
                .and_then(parse_a1)
                .unwrap_or((row, column + 1));
            column = at.1;
            if let Some(found) = classify(cell, at, &mut shared) {
                out.insert(at, found);
            }
        }
    }
    out
}

fn classify(
    cell: Node<'_, '_>,
    at: (usize, usize),
    shared: &mut HashMap<String, String>,
) -> Option<Cell> {
    if let Some(f) = children(cell, "f").next() {
        let text = f.text().unwrap_or_default();
        let index = (attr(f, "t") == Some("shared"))
            .then(|| attr(f, "si"))
            .flatten();
        let pattern = match index {
            Some(si) if text.is_empty() => shared.get(si)?.clone(),
            Some(si) => shared
                .entry(si.to_owned())
                .or_insert_with(|| relative(text, at))
                .clone(),
            None => relative(text, at),
        };
        return Some(Cell::Formula(pattern));
    }
    if !matches!(attr(cell, "t"), None | Some("n")) {
        return None;
    }
    let value = children(cell, "v").next()?.text()?;
    Some(Cell::Number(value.to_owned()))
}

fn quoted(sheet: &str) -> String {
    let plain = sheet
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        && !sheet.starts_with(|c: char| c.is_ascii_digit());
    if plain {
        sheet.to_owned()
    } else {
        format!("'{}'", sheet.replace('\'', "''"))
    }
}

fn overrides(sheet: &str, cells: &BTreeMap<(usize, usize), Cell>) -> Vec<String> {
    let pattern = |at: (usize, usize)| match cells.get(&at) {
        Some(Cell::Formula(p)) => Some(p),
        _ => None,
    };
    let mut out = Vec::new();
    for (&(row, column), cell) in cells {
        let Cell::Number(value) = cell else { continue };
        let sides = [
            ((row, column.wrapping_sub(1)), (row, column + 1)),
            ((row.wrapping_sub(1), column), (row + 1, column)),
        ];
        if let Some((before, after)) = sides
            .into_iter()
            .find(|&(b, a)| pattern(b).is_some() && pattern(b) == pattern(a))
        {
            out.push(format!(
                "hard-coded number: {}!{} holds {value} between {} and {}, formulas of one pattern",
                quoted(sheet),
                a1((row, column)),
                a1(before),
                a1(after)
            ));
        }
    }
    out
}

/// Each `(relationship id, target)` in a relationships part.
fn relationships(parts: &Parts, part: &str) -> Result<HashMap<String, (String, String)>> {
    let Some(bytes) = parts.get(part) else {
        return Ok(HashMap::new());
    };
    let doc = xml_part(part, bytes)?;
    Ok(doc
        .descendants()
        .filter(|n| n.tag_name().name() == "Relationship")
        .filter_map(|n| {
            let id = attr(n, "Id")?.to_owned();
            Some((
                id,
                (attr(n, "Type")?.to_owned(), attr(n, "Target")?.to_owned()),
            ))
        })
        .collect())
}

fn relationship_id<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name() == "id" && a.namespace().is_some())
        .map(|a| a.value())
}

/// A workbook relationship target as a part name.
fn part_name(target: &str) -> String {
    target
        .strip_prefix('/')
        .map_or_else(|| format!("xl/{target}"), str::to_owned)
}

fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A decoded link target as a path: file:///C:/x.xlsx names C:/x.xlsx,
/// file:///home/x.xlsx and file://localhost/home/x.xlsx name /home/x.xlsx, and
/// file://server/share/x.xlsx names //server/share/x.xlsx. Others are paths already.
fn local_path(target: &str) -> String {
    let Some(rest) = target.strip_prefix("file://") else {
        return target.to_owned();
    };
    let rest = rest
        .strip_prefix("localhost")
        .filter(|r| r.starts_with('/'))
        .unwrap_or(rest);
    if !rest.starts_with(['/', '\\']) {
        format!("//{rest}")
    } else if rest.as_bytes().get(2) == Some(&b':') || rest.starts_with(r"/\\") {
        rest[1..].to_owned()
    } else {
        rest.to_owned()
    }
}

/// The POSIX paths a Mac volume-colon path can name: `Macintosh HD:Users:a.xlsx`
/// is `/Volumes/Macintosh HD/Users/a.xlsx`, or `/Users/a.xlsx` when that volume
/// is the startup disk. `None` for any other path, including a drive letter.
fn mac_volume_paths(path: &str) -> Option<[String; 2]> {
    let path = path.strip_prefix('/').unwrap_or(path);
    let (volume, rest) = path.split_once(':')?;
    let plain = |s: &str| !s.is_empty() && !s.contains(['/', '\\']);
    (volume.chars().count() > 1 && plain(volume) && plain(rest)).then(|| {
        let rest = rest.replace(':', "/");
        [format!("/Volumes/{volume}/{rest}"), format!("/{rest}")]
    })
}

/// Why an external link's target is not usable, or `None` when its file exists.
fn missing_target(kind: &str, target: &str, base: &Path) -> Option<String> {
    if kind.ends_with("/xlPathMissing") {
        return Some(format!("targets {target}, which Excel marked missing"));
    }
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(format!("targets {target}, a remote file not checked"));
    }
    let path = local_path(&percent_decoded(target));
    if let Some(paths) = mac_volume_paths(&path) {
        return (!paths.iter().any(|p| Path::new(p).exists()))
            .then(|| format!("targets {target}, a Mac volume path not found on this computer"));
    }
    (!base.join(path).exists()).then(|| format!("targets {target}, which does not exist"))
}

fn external_links(parts: &Parts, book: &Document<'_>, base: &Path) -> Result<Vec<String>> {
    let rels = relationships(parts, "xl/_rels/workbook.xml.rels")?;
    let mut out = Vec::new();
    for reference in book
        .descendants()
        .filter(|n| n.tag_name().name() == "externalReference")
    {
        let Some((_, target)) = relationship_id(reference).and_then(|id| rels.get(id)) else {
            continue;
        };
        let part = part_name(target);
        let Some(bytes) = parts.get(&part) else {
            out.push(format!(
                "external link: {part} is missing from the workbook"
            ));
            continue;
        };
        let doc = xml_part(&part, bytes)?;
        let Some(book_ref) = doc
            .descendants()
            .find(|n| n.tag_name().name() == "externalBook")
        else {
            continue;
        };
        let (dir, file) = part.rsplit_once('/').unwrap_or(("", &part));
        let link_rels = relationships(parts, &format!("{dir}/_rels/{file}.rels"))?;
        let problem = match relationship_id(book_ref).and_then(|id| link_rels.get(id)) {
            Some((kind, target)) => missing_target(kind, target, base),
            None => Some("has no target".to_owned()),
        };
        if let Some(problem) = problem {
            out.push(format!("external link: {part} {problem}"));
        }
    }
    Ok(out)
}

fn sheets(book: &Document<'_>, rels: &HashMap<String, (String, String)>) -> Vec<Sheet> {
    book.descendants()
        .filter(|n| n.tag_name().name() == "sheet")
        .map(|n| Sheet {
            name: attr(n, "name").unwrap_or_default().to_owned(),
            state: attr(n, "state").unwrap_or("visible").to_owned(),
            part: relationship_id(n)
                .and_then(|id| rels.get(id))
                .map(|(_, target)| part_name(target)),
        })
        .collect()
}

fn hidden(book: &Document<'_>, sheets: &[Sheet]) -> Vec<String> {
    let mut out = Vec::new();
    for sheet in sheets {
        match sheet.state.as_str() {
            "hidden" => out.push(format!("hidden sheet: {}", sheet.name)),
            "veryHidden" => out.push(format!("very hidden sheet: {}", sheet.name)),
            _ => {}
        }
    }
    for name in book
        .descendants()
        .filter(|n| n.tag_name().name() == "definedName")
        .filter(|n| matches!(attr(*n, "hidden"), Some("1" | "true")))
    {
        let scope = attr(name, "localSheetId")
            .and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| sheets.get(i))
            .map(|s| format!(" (sheet {})", s.name))
            .unwrap_or_default();
        out.push(format!(
            "hidden name: {}{scope}",
            attr(name, "name").unwrap_or_default()
        ));
    }
    out
}

/// Every finding for the workbook at `path`, in sheet, link and name order.
pub fn audit(path: &Path) -> Result<Vec<String>> {
    let (parts, _) = read_selected_inventory(path, |name| {
        name.ends_with(".xml") || name.ends_with(".rels")
    })?;
    let book = xml_part(
        BOOK,
        parts
            .get(BOOK)
            .ok_or("there is no xl/workbook.xml in the archive")?,
    )?;
    let rels = relationships(&parts, "xl/_rels/workbook.xml.rels")?;
    let sheets = sheets(&book, &rels);
    let mut out = Vec::new();
    for sheet in &sheets {
        let Some(bytes) = sheet.part.as_ref().and_then(|p| parts.get(p)) else {
            continue;
        };
        let doc = xml_part(sheet.part.as_deref().unwrap_or_default(), bytes)?;
        out.extend(overrides(&sheet.name, &cells(&doc)));
    }
    let base = path.parent().unwrap_or(Path::new(""));
    out.extend(external_links(&parts, &book, base)?);
    out.extend(hidden(&book, &sheets));
    Ok(out)
}

/// `ozzit audit [workbook]`.
pub fn command(args: &[String]) -> Output {
    if args.len() > 1 {
        return Output::stderr("FAIL: usage: ozzit audit [workbook]");
    }
    let workbook = args.first().map_or("ozzit.xlsx", String::as_str);
    match audit(Path::new(workbook)) {
        Ok(findings) => {
            let bad = format!("FOUND: {} finding(s) in {workbook}", findings.len());
            Output::report(findings, bad, format!("OK: {workbook}, no findings"))
        }
        Err(e) => Output::failure(format!("FAIL: cannot read {workbook}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_become_relative_or_stay_absolute() {
        assert_eq!(
            relative("A1+$B$2+C$3+$D4", (2, 2)),
            "R[-1]C[-1]+R2C2+R3C[1]+R[2]C4"
        );
        assert_eq!(relative("SUM(A1:B2)", (1, 1)), "SUM(R[0]C[0]:R[1]C[1])");
        assert_eq!(relative("Sheet2!A1", (1, 1)), "Sheet2!R[0]C[0]");
        assert_eq!(
            relative("SUM(B:$C)+SUM(2:$3)", (1, 1)),
            "SUM(C[1]:C3)+SUM(R[1]:R3)"
        );
        assert_eq!(relative("SUM(A:A)", (1, 2)), relative("SUM(B:B)", (1, 3)));
    }

    #[test]
    fn names_functions_literals_and_quoted_sheets_are_not_references() {
        for text in [
            "LOG10(2)",
            "oz.Rate1",
            "\"A1\"",
            "'Q1 A1'!B",
            "Table1[A1]",
            "XFE1",
            "A0",
        ] {
            assert_eq!(relative(text, (5, 5)), text);
        }
    }

    #[test]
    fn column_letters_round_trip() {
        for column in [1, 26, 27, 702, 703, MAX_COLUMN] {
            assert_eq!(column_number(&letters(column)), column);
        }
    }

    #[test]
    fn file_urls_name_local_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a b.xlsx");
        std::fs::write(&file, b"").unwrap();
        let path = file
            .display()
            .to_string()
            .replace('\\', "/")
            .replace(' ', "%20");
        let url = format!(
            "file://{}{path}",
            if path.starts_with('/') { "" } else { "/" }
        );
        assert_eq!(
            missing_target("x/externalLinkPath", &url, Path::new("")),
            None
        );
        assert_eq!(percent_decoded("%zz%4"), "%zz%4");
        for (url, path) in [
            ("file:///C:/a.xlsx", "C:/a.xlsx"),
            (r"file:///C:\a.xlsx", r"C:\a.xlsx"),
            ("file:///Users/a.xlsx", "/Users/a.xlsx"),
            ("file://localhost/Users/a.xlsx", "/Users/a.xlsx"),
            ("file://localhost/C:/a.xlsx", "C:/a.xlsx"),
            ("file://server/share/a.xlsx", "//server/share/a.xlsx"),
            (r"file:///\\server\share\a.xlsx", r"\\server\share\a.xlsx"),
            ("../a.xlsx", "../a.xlsx"),
        ] {
            assert_eq!(local_path(url), path, "{url}");
        }
    }

    #[test]
    fn mac_volume_paths_name_the_volume_or_the_startup_disk() {
        assert_eq!(
            mac_volume_paths("Macintosh HD:Users:ryan:a b.xlsx"),
            Some([
                "/Volumes/Macintosh HD/Users/ryan/a b.xlsx".to_owned(),
                "/Users/ryan/a b.xlsx".to_owned()
            ])
        );
        assert_eq!(
            mac_volume_paths(&local_path("file:///Data:Q1.xlsx")),
            Some(["/Volumes/Data/Q1.xlsx".to_owned(), "/Q1.xlsx".to_owned()])
        );
        for path in [
            "C:/a.xlsx",
            r"C:\a.xlsx",
            "/Users/a.xlsx",
            "//server/share/a.xlsx",
            "../a.xlsx",
            "a.xlsx",
            "Disk:",
            ":a.xlsx",
            "Disk:dir/a.xlsx",
        ] {
            assert_eq!(mac_volume_paths(path), None, "{path}");
        }
    }
}
