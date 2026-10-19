//! Apply Ozzit's presentation rules to an existing workbook.
//!
//! Targeted OOXML surgery: formula text, cached values, drawings, namespace
//! prefixes and the Advanced Formula Environment store are preserved. The pass
//! is idempotent and writes through the canonical archive writer.
use crate::{archive::*, common::*, py, sanitise::thousands, workbook::xml_part};
use std::path::Path;

pub const MAIN_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
pub const DOC_REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const SHARED: &str = "xl/sharedStrings.xml";
const STYLES: &str = "xl/styles.xml";
const RATIOS: &str = "oz.FinancialRatios";

pub const EXPECTED_STRAPLINES: [(&str, &str); 8] = [
    (
        "oz.CountCλ",
        "Count the number of times one or more characters appear in a string",
    ),
    (
        "oz.CountRowsλ",
        "Count the number of numbers in each row of an array",
    ),
    (
        "oz.CountColsλ",
        "Count the number of numbers in each column of an array",
    ),
    (
        "oz.CountARowsλ",
        "Count all non-empty cells in each row of an array",
    ),
    (
        "oz.CountAColsλ",
        "Count all non-empty cells in each column of an array",
    ),
    (
        "oz.IsBetweenEλ",
        "Determine if a value is between a lower and upper limit",
    ),
    (
        "oz.RangeToDAEλ",
        "Convert a static range into a dynamic array",
    ),
    (RATIOS, "Three dozen financial ratios"),
];
pub const COVER_OLD: &str = "Each function worksheet has green shaded cells. These cells contain the function whose name appears in the worksheet's title.";
pub const COVER_NEW: &str = "Each function worksheet has pale purple shaded cells. These cells contain the function whose name appears in the worksheet's title.";

/// `parts[name]`, failing as Python's `KeyError` does.
fn bytes<'a>(parts: &'a Parts, name: &str) -> Result<&'a [u8]> {
    parts
        .get(name)
        .map(Vec::as_slice)
        .ok_or_else(|| py::repr_str(name))
}

/// Sheet names and the archive parts their relationships name, in sheet order.
pub fn sheet_paths(parts: &Parts) -> Result<Vec<(String, String)>> {
    let book = xml_part(BOOK, bytes(parts, BOOK)?)?;
    let rels_name = "xl/_rels/workbook.xml.rels";
    let rels = xml_part(rels_name, bytes(parts, rels_name)?)?;
    let mut targets = std::collections::HashMap::new();
    for rel in rels.root_element().children().filter(|n| n.is_element()) {
        let id = rel.attribute("Id").ok_or_else(|| py::repr_str("Id"))?;
        let target = rel
            .attribute("Target")
            .ok_or_else(|| py::repr_str("Target"))?;
        targets.insert(id, target);
    }
    let sheets = book
        .root_element()
        .children()
        .find(|n| n.has_tag_name((MAIN_NS, "sheets")))
        .ok_or("workbook has no sheets collection")?;
    let mut out: Vec<(String, String)> = Vec::new();
    for sheet in sheets.children().filter(|n| n.is_element()) {
        let name = sheet
            .attribute("name")
            .ok_or_else(|| py::repr_str("name"))?;
        let rid = sheet
            .attribute((DOC_REL_NS, "id"))
            .ok_or_else(|| py::repr_str(&format!("{{{DOC_REL_NS}}}id")))?;
        let target = targets.get(rid).ok_or_else(|| py::repr_str(rid))?;
        let path = resolve_target(BOOK, target);
        match out.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => entry.1 = path,
            None => out.push((name.into(), path)),
        }
    }
    Ok(out)
}

/// The text of every shared string, as ElementTree's `itertext` joins it.
pub fn shared_values(text: &str) -> Result<Vec<String>> {
    let doc = xml_part(SHARED, text.as_bytes())?;
    Ok(doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name((MAIN_NS, "si")))
        .map(|si| {
            si.descendants()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .collect()
        })
        .collect())
}

fn ensure_shared(text: String, values: &mut Vec<String>, value: &str) -> (String, usize) {
    if let Some(index) = values.iter().position(|v| v == value) {
        return (text, index);
    }
    let text = text.replace(
        "</sst>",
        &format!("<si><t>{}</t></si></sst>", escape(value)),
    );
    values.push(value.into());
    let unique = values.len();
    let text = replace_first(
        &bounded(r#"(<sst\b[^>]*\buniqueCount=")\d+("[^>]*>)"#),
        &text,
        |m| format!("{}{unique}{}", &m[1], &m[2]),
    );
    (text, unique - 1)
}

/// `re.sub(pattern, f, text, count=1)`.
fn replace_first(
    pattern: &str,
    text: &str,
    f: impl FnOnce(&fancy_regex::Captures<'_, str>) -> String,
) -> String {
    match re(pattern).captures(text).ok().flatten() {
        Some(m) => {
            let whole = m.get(0).expect("whole match");
            format!(
                "{}{}{}",
                &text[..whole.start()],
                f(&m),
                &text[whole.end()..]
            )
        }
        None => text.into(),
    }
}

pub fn set_existing_shared_cell(text: &str, address: &str, index: usize) -> Result<(String, bool)> {
    let pattern = format!(
        r#"(<c r="{}"[^>]*\bt="s"[^>]*>\s*<v>)\d+(</v>\s*</c>)"#,
        fancy_regex::escape(address)
    );
    let pattern = bounded(&pattern);
    let found = captures(&pattern, text);
    let Some(found) = found.first() else {
        return Err(format!("shared-string cell {address} not found"));
    };
    let after = found[0].split_once("<v>").map_or("", |x| x.1);
    let current = captures(r"\d+", after)
        .first()
        .map(|d| py::parse_int(&d[0], 10))
        .transpose()?
        .ok_or_else(|| format!("shared-string cell {address} has no value index"))?;
    if current == index.into() {
        return Ok((text.into(), false));
    }
    Ok((
        replace_first(&pattern, text, |m| format!("{}{index}{}", &m[1], &m[2])),
        true,
    ))
}

/// Returns (text, changed, inserted). A repoint changes which shared string A2
/// points at and adds no reference, so only an insert raises the shared-string count.
pub fn add_financial_ratios_strapline(text: &str, index: usize) -> Result<(String, bool, bool)> {
    if matches(r#"<c r="A2"[^>]*>"#, text) {
        let (text, changed) = set_existing_shared_cell(text, "A2", index)?;
        return Ok((text, changed, false));
    }
    // Financial Ratios' sheet begins at row 1, so the first closing row is row 1.
    let Some(at) = text.find("</row>") else {
        return Err(format!("could not insert {RATIOS} row 2"));
    };
    let at = at + "</row>".len();
    let mut text = format!(
        r#"{}<row r="2" spans="1:11" x14ac:dyDescent="0.25"><c r="A2" s="288" t="s"><v>{index}</v></c></row>{}"#,
        &text[..at],
        &text[at..]
    );
    if !text.contains(r#"<mergeCell ref="A2:K2""#) {
        let merges = re(r#"(?s)<mergeCells count="(\d+)">(.*?)</mergeCells>"#);
        let Some(m) = merges.captures(text.as_str()).ok().flatten() else {
            return Err(format!("{RATIOS} mergeCells not found"));
        };
        let count = py::parse_int(&m[1], 10)? + 1;
        let whole = m.get(0).expect("whole match").range();
        let merged = format!(
            r#"<mergeCells count="{count}">{}<mergeCell ref="A2:K2"/></mergeCells>"#,
            &m[2]
        );
        text.replace_range(whole, &merged);
    }
    Ok((text, true, true))
}

fn update_shared_and_sheets(parts: &mut Parts, paths: &[(String, String)]) -> Result<Vec<String>> {
    let mut shared = decode(bytes(parts, SHARED)?)?;
    let mut values = shared_values(&shared)?;
    let mut changes = Vec::new();

    if shared.contains(COVER_OLD) {
        shared = shared.replace(COVER_OLD, COVER_NEW);
        let at = values
            .iter()
            .position(|v| v == COVER_OLD)
            .ok_or("list.index(x): x not in list")?;
        values[at] = COVER_NEW.into();
        changes.push("updated Cover formula-highlight wording".into());
    }

    let mut added = 0;
    for (sheet_name, strapline) in EXPECTED_STRAPLINES {
        let index;
        (shared, index) = ensure_shared(shared, &mut values, strapline);
        let path = &paths
            .iter()
            .find(|(n, _)| n == sheet_name)
            .ok_or_else(|| py::repr_str(sheet_name))?
            .1;
        let sheet = decode(bytes(parts, path)?)?;
        let (sheet, changed) = if sheet_name == RATIOS {
            let (sheet, changed, inserted) = add_financial_ratios_strapline(&sheet, index)?;
            if inserted {
                added += 1;
            }
            (sheet, changed)
        } else {
            set_existing_shared_cell(&sheet, "A2", index)?
        };
        if changed {
            parts.insert(path.clone(), sheet.into_bytes());
            changes.push(format!("set {sheet_name}!A2"));
        }
    }

    if added > 0 {
        shared = replace_first(
            &bounded(r#"(<sst\b[^>]*\bcount=")(\d+)(")"#),
            &shared,
            |m| {
                let count = py::parse_int(&m[2], 10).expect("digits") + added;
                format!("{}{count}{}", &m[1], &m[3])
            },
        );
    }
    parts.insert(SHARED.into(), shared.into_bytes());
    Ok(changes)
}

pub fn update_styles(parts: &mut Parts) -> Result<Vec<String>> {
    let original = decode(bytes(parts, STYLES)?)?;
    let mut styles = original.clone();
    let mut changes = Vec::new();

    let mint = styles.matches(r#"<fgColor indexed="42"/>"#).count();
    if mint > 0 {
        styles = styles.replace(r#"<fgColor indexed="42"/>"#, r#"<fgColor rgb="FFE8D9E2"/>"#);
        changes.push(format!("replaced {mint} mint fill definition"));
    }

    let fonts = re(&bounded(r"(?s)<fonts\b.*?</fonts>"))
        .find(&styles)
        .ok()
        .flatten()
        .map(|m| m.range())
        .ok_or("styles.xml fonts section missing")?;
    let grey = styles[fonts.clone()]
        .matches(r#"<color rgb="FF808080"/>"#)
        .count();
    if grey > 0 {
        let new = styles[fonts.clone()]
            .replace(r#"<color rgb="FF808080"/>"#, r#"<color rgb="FF6E6862"/>"#);
        styles.replace_range(fonts, &new);
        changes.push(format!("raised {grey} strapline font colour"));
    }

    let xfs = re(&bounded(r"(?s)<cellXfs\b.*?</cellXfs>"))
        .find(&styles)
        .ok()
        .flatten()
        .map(|m| m.range())
        .ok_or("styles.xml cellXfs section missing")?;
    let dates = styles[xfs.clone()].matches(r#"numFmtId="14""#).count();
    if dates > 0 {
        let new = styles[xfs.clone()].replace(r#"numFmtId="14""#, r#"numFmtId="169""#);
        styles.replace_range(xfs, &new);
        changes.push(format!("made {dates} date styles locale-safe"));
    }

    if styles != original {
        parts.insert(STYLES.into(), styles.into_bytes());
    }
    Ok(changes)
}

pub fn polish(workbook: &Path) -> Result<Vec<String>> {
    let mut parts = read_parts(workbook)?;
    let before = parts.clone();
    let paths = sheet_paths(&parts)?;
    let mut changes = update_shared_and_sheets(&mut parts, &paths)?;
    changes.extend(update_styles(&mut parts)?);
    if parts == before {
        return Ok(vec!["already polished".into()]);
    }
    write_deterministic(workbook, &parts)?;
    Ok(changes)
}

pub fn command(args: &[String]) -> Output {
    if args.len() > 1 {
        return Output::stderr("FAIL: usage: ozzit polish_workbook [workbook]");
    }
    let shown = py::PyPath::new(args.first().map_or("ozzit.xlsx", String::as_str));
    let path = shown.fs();
    if !path.is_file() {
        return Output::stderr(format!("FAIL: no such workbook: {shown}"));
    }
    let mut out = match polish(&path) {
        Ok(changes) => changes,
        Err(e) => return Output::stderr(format!("FAIL: cannot polish {shown}: {e}")),
    };
    match std::fs::metadata(&path) {
        Ok(meta) => out.push(format!(
            "OK: {shown} polished, {} bytes",
            thousands(meta.len())
        )),
        Err(e) => return Output::failure(format!("FAIL: {}", io_error(&e, &path))),
    }
    Output::ok(out)
}
