// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! The 13-week cash-flow forecast template: its package, sheets, formulas,
//! styles, charts and documentation.
//!
//! The helpers restate the ElementTree lookups the Python module made. Cell
//! values and formulas keep ElementTree's distinction between an absent element
//! and one with no text, and paths resolve with `posixpath` semantics.
use super::xml::{self, MAIN, PACKAGE_REL, REL};
use super::{index, part, parts, read, root};
use chrono::{Days, NaiveDate};
use ozzit::archive::Parts;
use ozzit::{common, py};
use roxmltree::Node;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;
use std::sync::LazyLock;

const WORKBOOK: &str = "templates/13-week-cash-flow-forecast.xlsx";
const DRAWING: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
const CHART: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const DRAWING_MAIN: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const EXPECTED_SHEETS: [&str; 6] = [
    "Start Here",
    "Dashboard",
    "Assumptions",
    "13-Week Forecast",
    "Weekly Review",
    "Checks & Sources",
];
const FORECAST_LABELS: [(usize, &str); 10] = [
    (5, "Week commencing"),
    (6, "Week ending"),
    (7, "Actual / Forecast"),
    (9, "CASH RECEIPTS"),
    (19, "Total cash receipts"),
    (21, "CASH PAYMENTS"),
    (43, "Total cash payments"),
    (45, "CASH POSITION"),
    (48, "Closing cash balance"),
    (51, "Liquidity status"),
];
const FORECAST_ROW_MAP: &[(usize, &str)] = &[
    (5, "Week commencing"),
    (6, "Week ending"),
    (7, "Actual / Forecast"),
    (9, "CASH RECEIPTS"),
    (10, "Customer receipts"),
    (11, "Overdue receipts"),
    (12, "Cash / EFTPOS sales"),
    (13, "GST refunds / credits"),
    (14, "Other operating"),
    (15, "Asset sale proceeds"),
    (16, "Equity / owner funding"),
    (17, "Loan proceeds"),
    (18, "Scenario receipt adjustment"),
    (19, "Total cash receipts"),
    (21, "CASH PAYMENTS"),
    (22, "Suppliers and inventory"),
    (23, "Net wages"),
    (24, "PAYG withholding"),
    (25, "Superannuation"),
    (26, "Payroll tax / workers compensation"),
    (27, "Rent"),
    (28, "Utilities"),
    (29, "Insurance"),
    (30, "Software and subscriptions"),
    (31, "Marketing"),
    (32, "Professional services"),
    (33, "Freight, vehicles and travel"),
    (34, "GST / BAS payments"),
    (35, "PAYG income tax instalments"),
    (36, "FBT / other tax"),
    (37, "Interest and bank fees"),
    (38, "Loan principal"),
    (39, "Capital expenditure"),
    (40, "Dividends / distributions"),
    (41, "Other operating"),
    (42, "Scenario payment adjustment"),
    (43, "Total cash payments"),
    (45, "CASH POSITION"),
    (46, "Opening cash balance"),
    (47, "Net cash movement"),
    (48, "Closing cash balance"),
    (49, "Minimum cash buffer"),
    (50, "Headroom / (gap)"),
    (51, "Liquidity status"),
];
const BASE_CLOSING_CASH: [i64; 13] = [
    358_000, 349_000, 325_000, 293_000, 201_000, 133_000, 217_000, 160_000, 117_000, 137_000,
    99_000, 82_000, 31_000,
];
const UPSIDE_CLOSING_CASH: [i64; 13] = [
    358_000, 364_380, 354_920, 336_680, 257_940, 203_320, 300_000, 256_060, 226_480, 260_600,
    237_020, 234_980, 199_320,
];
const DOWNSIDE_CLOSING_CASH: [i64; 13] = [
    358_000, 318_800, 266_150, 206_900, 88_550, -6_050, 52_600, -30_500, -100_300, -108_350,
    -174_900, -221_400, -302_550,
];
const FORECAST_COLUMNS: &str = "BCDEFGHIJKLMN";
const TERMINAL_ROWS: [usize; 5] = [46, 48, 49, 50, 51];
const PALETTE_ANCHORS: [&str; 9] = [
    "66023C", "000000", "FFFFF0", "E5E4E2", "E8D9E2", "990024", "F0D9DE", "0F4B32", "DBE4E0",
];
const BANNED_SOURCE_TEXT: [&str; 5] = [
    "tradepoint",
    "1099",
    "federal tax",
    "state tax",
    "tradepointcfo",
];
const AUD_FORMAT: &str = "\"$\"#,##0;[Red](\"$\"#,##0);-";
const DATE_FORMAT: &str = "dd mmm yyyy";
const PERCENT_FORMAT: &str = "0.0%";
const ERROR_TOKENS: [&str; 7] = [
    "#REF!", "#DIV/0!", "#VALUE!", "#NAME?", "#N/A", "#NUM!", "#NULL!",
];
const PROHIBITED_PART_MARKERS: [&str; 8] = [
    "vbaproject",
    "/activex/",
    "/oleobjects/",
    "/ctrlprops/",
    "/embeddings/",
    "customui/",
    "attachedtemplate",
    "/externallinks/",
];
const PROHIBITED_CONTENT_TYPE_MARKERS: [&str; 9] = [
    "vba",
    "activex",
    "oleobject",
    "macroenabled",
    "attachedtemplate",
    "control",
    "customui",
    "embedded",
    "externallink",
];
const PROHIBITED_RELATIONSHIP_MARKERS: [&str; 7] = [
    "/vbaproject",
    "/activex",
    "/oleobject",
    "/control",
    "/customui",
    "/attachedtemplate",
    "/externallink",
];
const FORMULA_ERROR_COUNT_FORMULA: &str = concat!(
    "SUMPRODUCT(--ISERROR('Assumptions'!$B$5:$E$24))",
    "+SUMPRODUCT(--ISERROR('13-Week Forecast'!$B$5:$O$51))",
    "+SUMPRODUCT(--ISERROR('Weekly Review'!$A$4:$O$18))",
    "+SUMPRODUCT(--ISERROR('Dashboard'!$A$6:$J$36))",
    "+SUMPRODUCT(--ISERROR('Dashboard'!$P$4:$S$49))",
);
const STRING_LITERAL: &str = r#""(?:[^"]|"")*""#;
const PASSING: (&str, &str) = ("FF0F4B32", "FFDBE4E0");
const FAILING: (&str, &str) = ("FF990024", "FFF0D9DE");
const INPUT: (&str, &str) = ("FF0000FF", "FFFFFFF0");

/// A volatile call, with the class and names matched as `re.IGNORECASE` matches them.
static VOLATILE_PATTERN: LazyLock<String> = LazyLock::new(|| {
    let letters: String = ('A'..='Z')
        .map(|letter| {
            let class = py::ignorecase_letter(letter);
            class[1..class.len() - 1].to_owned()
        })
        .collect();
    let names = [
        "TODAY",
        "NOW",
        "RAND",
        "RANDARRAY",
        "RANDBETWEEN",
        "OFFSET",
        "INDIRECT",
        "CELL",
        "INFO",
        "RTD",
    ]
    .map(py::ignorecase_literal)
    .join("|");
    format!(
        r"(?<![0-9_.{letters}])(?:{})?(?:{names})\s*\(",
        py::ignorecase_literal("_xlfn.")
    )
});

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("a valid date")
}

fn start_date() -> NaiveDate {
    date(2026, 8, 31)
}

fn attr<'a>(node: Node<'a, '_>, name: &str) -> &'a str {
    node.attribute(name)
        .unwrap_or_else(|| panic!("<{}> has no {name}", node.tag_name().name()))
}

fn relationship_id<'a>(node: Node<'a, '_>) -> &'a str {
    node.attribute((REL, "id"))
        .unwrap_or_else(|| panic!("<{}> has no r:id", node.tag_name().name()))
}

/// `int(text)`.
fn int(text: &str) -> i64 {
    text.parse()
        .unwrap_or_else(|_| panic!("{text:?} is not an integer"))
}

/// `float(text)`.
fn float(text: &str) -> f64 {
    py::parse_float(text).unwrap_or_else(|e| panic!("{e}"))
}

/// `posixpath.join(a, b)`.
fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        b.to_owned()
    } else if a.is_empty() || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

/// `posixpath.dirname(path)`.
fn dirname(path: &str) -> String {
    let head = &path[..path.rfind('/').map_or(0, |i| i + 1)];
    if head.is_empty() || head.chars().all(|c| c == '/') {
        head.to_owned()
    } else {
        head.trim_end_matches('/').to_owned()
    }
}

/// `posixpath.normpath(path)`.
fn normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let initial = match path {
        p if p.starts_with("//") && !p.starts_with("///") => 2,
        p if p.starts_with('/') => 1,
        _ => 0,
    };
    let mut parts: Vec<&str> = vec![];
    for part in path.split('/').filter(|part| !matches!(*part, "" | ".")) {
        if part != ".." || (initial == 0 && parts.is_empty()) || parts.last() == Some(&"..") {
            parts.push(part);
        } else {
            parts.pop();
        }
    }
    let normal = "/".repeat(initial) + &parts.join("/");
    if normal.is_empty() {
        ".".to_owned()
    } else {
        normal
    }
}

/// Each sheet's name and part, in workbook order.
fn sheet_paths(parts: &Parts) -> Vec<(String, String)> {
    let (book, rels) = (
        part(parts, "xl/workbook.xml"),
        part(parts, "xl/_rels/workbook.xml.rels"),
    );
    let (book, rels) = (xml::parse(&book), xml::parse(&rels));
    let targets: HashMap<&str, &str> = xml::elements(rels.root_element())
        .map(|item| (attr(item, "Id"), attr(item, "Target")))
        .collect();
    let sheets = xml::child(book.root_element(), MAIN, "sheets").expect("a sheets element");
    xml::elements(sheets)
        .map(|item| {
            let target = targets[relationship_id(item)].trim_start_matches('/');
            let path = if target.starts_with("xl/") {
                target.to_owned()
            } else {
                format!("xl/{target}")
            };
            (attr(item, "name").to_owned(), path)
        })
        .collect()
}

/// `dict(sheet_paths(parts))`.
fn sheet_map(parts: &Parts) -> HashMap<String, String> {
    sheet_paths(parts).into_iter().collect()
}

fn shared_strings(parts: &Parts) -> Vec<String> {
    if !parts.contains_key("xl/sharedStrings.xml") {
        return vec![];
    }
    let text = part(parts, "xl/sharedStrings.xml");
    xml::children(xml::parse(&text).root_element(), MAIN, "si")
        .map(xml::text)
        .collect()
}

/// A sheet's cell values by address; an empty `<v/>` holds None, as `.text` did.
#[derive(Debug, PartialEq)]
struct Values(HashMap<String, Option<String>>);

impl Values {
    /// `values.get(address)`.
    fn get(&self, address: &str) -> Option<&str> {
        self.0.get(address).and_then(Option::as_deref)
    }

    /// `values[address]`, used as text.
    fn at(&self, address: &str) -> &str {
        self.get(address)
            .unwrap_or_else(|| panic!("no value at {address}"))
    }

    /// `values.get(address, "")`, used as text.
    fn or_empty(&self, address: &str) -> &str {
        self.0.get(address).map_or("", |value| {
            value
                .as_deref()
                .unwrap_or_else(|| panic!("{address} holds no text"))
        })
    }

    /// `int(float(values[address]))`.
    fn whole(&self, address: &str) -> i64 {
        let value = float(self.at(address));
        assert!(value.is_finite(), "{address}={value}");
        value.trunc() as i64
    }

    /// `[values.get(f"{column}{row}") for column in columns]`.
    fn row(&self, columns: &str, row: usize) -> Vec<Option<&str>> {
        columns
            .chars()
            .map(|column| self.get(&format!("{column}{row}")))
            .collect()
    }

    /// `[values.get(f"{column}{row}") for row in rows]`.
    fn column(&self, column: char, rows: Range<usize>) -> Vec<Option<&str>> {
        rows.map(|row| self.get(&format!("{column}{row}")))
            .collect()
    }
}

fn texts<S: AsRef<str>>(items: &[S]) -> Vec<Option<&str>> {
    items.iter().map(|item| Some(item.as_ref())).collect()
}

fn values_by_address(parts: &Parts, path: &str) -> Values {
    let strings = shared_strings(parts);
    let text = part(parts, path);
    let sheet = xml::parse(&text);
    let mut values = HashMap::new();
    for cell in xml::descendants(sheet.root_element(), MAIN, "c") {
        let reference = attr(cell, "r").to_owned();
        let value = match xml::child(cell, MAIN, "v") {
            None => Some(xml::text(cell)),
            Some(v) if cell.attribute("t") == Some("s") => {
                let index: usize = v
                    .text()
                    .and_then(|i| i.parse().ok())
                    .expect("a shared string index");
                Some(strings[index].clone())
            }
            Some(v) => v.text().map(str::to_owned),
        };
        values.insert(reference, value);
    }
    Values(values)
}

/// A sheet's formulas by address; a formula element with no text holds None.
struct Formulas(HashMap<String, Option<String>>);

impl Formulas {
    fn get(&self, address: &str) -> Option<&str> {
        self.0.get(address).and_then(Option::as_deref)
    }

    fn has(&self, address: &str) -> bool {
        self.0.contains_key(address)
    }
}

fn formulas_by_address(parts: &Parts, path: &str) -> Formulas {
    let text = part(parts, path);
    let sheet = xml::parse(&text);
    Formulas(
        xml::descendants(sheet.root_element(), MAIN, "c")
            .filter_map(|cell| {
                let formula = xml::child(cell, MAIN, "f")?;
                Some((
                    attr(cell, "r").to_owned(),
                    formula.text().map(str::to_owned),
                ))
            })
            .collect(),
    )
}

fn find_cell<'a, 'i>(root: Node<'a, 'i>, address: &str) -> Node<'a, 'i> {
    xml::cell(root, address).unwrap_or_else(|| panic!("no cell {address}"))
}

fn cached_value(parts: &Parts, path: &str, address: &str) -> Option<String> {
    let text = part(parts, path);
    let sheet = xml::parse(&text);
    let cell = find_cell(sheet.root_element(), address);
    xml::child(cell, MAIN, "v").map(|v| v.text().unwrap_or_default().to_owned())
}

/// The Excel date serial: days since 30 December 1899, as text.
fn excel_serial(value: NaiveDate) -> String {
    (value - date(1899, 12, 30)).num_days().to_string()
}

fn markdown_section<'a>(markdown: &'a str, heading: &str, next_heading: Option<&str>) -> &'a str {
    let marker = format!("## {heading}\n");
    let start = index(markdown, &marker, 0) + marker.len();
    let end = next_heading.map_or(markdown.len(), |next| {
        index(markdown, &format!("## {next}\n"), start)
    });
    &markdown[start..end]
}

fn style_list<'a, 'i>(styles: &'a roxmltree::Document<'i>, name: &'static str) -> Node<'a, 'i> {
    xml::child(styles.root_element(), MAIN, name).unwrap_or_else(|| panic!("no {name}"))
}

fn rgb(node: Option<Node<'_, '_>>) -> Option<String> {
    node.and_then(|node| node.attribute("rgb"))
        .map(str::to_owned)
}

/// The cell's font colour and its fill's first foreground colour.
fn style_for_cell(parts: &Parts, path: &str, address: &str) -> (Option<String>, Option<String>) {
    let (sheet, styles) = (part(parts, path), part(parts, "xl/styles.xml"));
    let (sheet, styles) = (xml::parse(&sheet), xml::parse(&styles));
    let cell = find_cell(sheet.root_element(), address);
    let style = xml::nth(style_list(&styles, "cellXfs"), xml::index(cell, "s"));
    let font = xml::nth(style_list(&styles, "fonts"), xml::index(style, "fontId"));
    let fill = xml::nth(style_list(&styles, "fills"), xml::index(style, "fillId"));
    (
        rgb(xml::child(font, MAIN, "color")),
        rgb(xml::descendants(fill, MAIN, "fgColor").next()),
    )
}

fn colours(font: &str, fill: Option<&str>) -> (Option<String>, Option<String>) {
    (Some(font.to_owned()), fill.map(str::to_owned))
}

/// The cell's font name and its number format code.
fn font_and_number_format(parts: &Parts, path: &str, address: &str) -> (Option<String>, String) {
    let (sheet, styles) = (part(parts, path), part(parts, "xl/styles.xml"));
    let (sheet, styles) = (xml::parse(&sheet), xml::parse(&styles));
    let cell = find_cell(sheet.root_element(), address);
    let style = xml::nth(style_list(&styles, "cellXfs"), xml::index(cell, "s"));
    let font = xml::nth(style_list(&styles, "fonts"), xml::index(style, "fontId"));
    let name = xml::child(font, MAIN, "name").and_then(|name| name.attribute("val"));
    let id = int(style.attribute("numFmtId").unwrap_or("0"));
    let custom: HashMap<i64, &str> = xml::path(styles.root_element(), MAIN, &["numFmts", "numFmt"])
        .into_iter()
        .map(|item| (int(attr(item, "numFmtId")), attr(item, "formatCode")))
        .collect();
    (
        name.map(str::to_owned),
        custom.get(&id).copied().unwrap_or("General").to_owned(),
    )
}

fn used_cell_font_names(parts: &Parts) -> HashSet<Option<String>> {
    let styles = part(parts, "xl/styles.xml");
    let styles = xml::parse(&styles);
    let cell_xfs = style_list(&styles, "cellXfs");
    let font_names: Vec<Option<&str>> = xml::elements(style_list(&styles, "fonts"))
        .map(|font| xml::child(font, MAIN, "name").and_then(|name| name.attribute("val")))
        .collect();
    let mut names = HashSet::new();
    for (_, path) in sheet_paths(parts) {
        let text = part(parts, &path);
        for cell in xml::descendants(xml::parse(&text).root_element(), MAIN, "c") {
            let style = xml::nth(cell_xfs, xml::index(cell, "s"));
            names.insert(font_names[xml::index(style, "fontId")].map(str::to_owned));
        }
    }
    names
}

#[derive(Debug, PartialEq)]
struct Relationship {
    source: String,
    kind: String,
    target: String,
    target_mode: String,
    resolved_target: String,
}

fn relationship_records(parts: &Parts) -> Vec<Relationship> {
    let mut records = vec![];
    for name in parts.keys().filter(|name| name.ends_with(".rels")) {
        let text = part(parts, name);
        let rels = xml::parse(&text);
        let source = if name == "_rels/.rels" {
            String::new()
        } else {
            let (prefix, file) = name
                .rsplit_once("/_rels/")
                .unwrap_or_else(|| panic!("{name} is not in a _rels folder"));
            join(prefix, &file[..file.len() - ".rels".len()])
        };
        for relationship in xml::children(rels.root_element(), PACKAGE_REL, "Relationship") {
            let target = relationship.attribute("Target").unwrap_or("");
            let resolved = if target.starts_with('/') {
                normpath(target.trim_start_matches('/'))
            } else {
                normpath(&join(&dirname(&source), target))
                    .trim_start_matches('/')
                    .to_owned()
            };
            records.push(Relationship {
                source: source.clone(),
                kind: relationship.attribute("Type").unwrap_or("").to_owned(),
                target: target.to_owned(),
                target_mode: relationship
                    .attribute("TargetMode")
                    .unwrap_or("Internal")
                    .to_owned(),
                resolved_target: resolved,
            });
        }
    }
    records
}

fn worksheet_references(formula: &str) -> BTreeSet<String> {
    let unquoted = common::sub(STRING_LITERAL, formula, r#""""#);
    common::captures(r"(?:'((?:[^']|'')+)'|([A-Za-z0-9 _&-]+))!", &unquoted)
        .into_iter()
        .map(|groups| {
            let name = if groups[1].is_empty() {
                groups[2].clone()
            } else {
                groups[1].replace("''", "'")
            };
            py::strip(&name).to_owned()
        })
        .collect()
}

fn marked(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn package_failures(parts: &Parts) -> Result<(), String> {
    let names: Vec<String> = parts
        .keys()
        .map(|name| name.to_lowercase())
        .filter(|name| marked(name, &PROHIBITED_PART_MARKERS))
        .collect();
    if !names.is_empty() {
        return Err(format!("prohibited parts: {names:?}"));
    }
    let types = part(parts, "[Content_Types].xml");
    let declared: Vec<String> = xml::elements(xml::parse(&types).root_element())
        .map(|item| item.attribute("ContentType").unwrap_or("").to_lowercase())
        .filter(|kind| marked(kind, &PROHIBITED_CONTENT_TYPE_MARKERS))
        .collect();
    if !declared.is_empty() {
        return Err(format!("prohibited content types: {declared:?}"));
    }
    let relationships = relationship_records(parts);
    let external = |item: &&Relationship| item.target_mode.to_lowercase() == "external";
    let found: Vec<&Relationship> = relationships.iter().filter(external).collect();
    if !found.is_empty() {
        return Err(format!("external relationships: {found:?}"));
    }
    let prohibited: Vec<&Relationship> = relationships
        .iter()
        .filter(|item| {
            let kind = item.kind.to_lowercase();
            marked(&kind, &PROHIBITED_RELATIONSHIP_MARKERS) || kind.ends_with("/package")
        })
        .collect();
    if !prohibited.is_empty() {
        return Err(format!("prohibited relationships: {prohibited:?}"));
    }
    let unresolved: Vec<&Relationship> = relationships
        .iter()
        .filter(|item| !external(item) && !parts.contains_key(&item.resolved_target))
        .collect();
    if !unresolved.is_empty() {
        return Err(format!("unresolved relationships: {unresolved:?}"));
    }
    Ok(())
}

fn formula_failures(parts: &Parts) -> Result<(), String> {
    let sheets: BTreeSet<String> = sheet_paths(parts)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    for (sheet, path) in sheet_paths(parts) {
        let text = part(parts, &path);
        for cell in xml::descendants(xml::parse(&text).root_element(), MAIN, "c") {
            let location = format!("{sheet}!{}", attr(cell, "r"));
            let formula = xml::child(cell, MAIN, "f").map_or("", |f| f.text().unwrap_or(""));
            let cached = xml::child(cell, MAIN, "v").map_or("", |v| v.text().unwrap_or(""));
            if cell.attribute("t") == Some("e") {
                return Err(format!("{location} caches an error value"));
            }
            let tokens: Vec<&str> = ERROR_TOKENS
                .into_iter()
                .filter(|token| formula.contains(token) || cached.contains(token))
                .collect();
            if !tokens.is_empty() {
                return Err(format!("{location} holds {tokens:?}"));
            }
            let unknown: Vec<String> = worksheet_references(formula)
                .into_iter()
                .filter(|name| !sheets.contains(name))
                .collect();
            if !unknown.is_empty() {
                return Err(format!("{location} refers to {unknown:?}"));
            }
            if common::matches(
                &VOLATILE_PATTERN,
                &common::sub(STRING_LITERAL, formula, r#""""#),
            ) {
                return Err(format!("{location} calls a volatile function: {formula}"));
            }
        }
    }
    Ok(())
}

/// The package and formula contract: the first failure, as the Python assertions raised it.
fn assert_package_and_formula_safe(parts: &Parts) -> Result<(), String> {
    package_failures(parts)?;
    formula_failures(parts)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Every comments part, ASCII-lowercased and joined with spaces.
fn comments_text(parts: &Parts) -> Vec<u8> {
    let payloads: Vec<Vec<u8>> = parts
        .iter()
        .filter(|(name, _)| name.contains("comments") && name.ends_with(".xml"))
        .map(|(_, payload)| payload.to_ascii_lowercase())
        .collect();
    payloads.join(&b' ')
}

/// The lowercased text of the comment on the cell, or "" when it has none.
fn comment_text(parts: &Parts, address: &str) -> String {
    let address = address.to_uppercase();
    for name in parts
        .keys()
        .filter(|name| name.starts_with("xl/comments") && name.ends_with(".xml"))
    {
        let text = part(parts, name);
        let comments = xml::parse(&text);
        let found = xml::path(comments.root_element(), MAIN, &["commentList", "comment"])
            .into_iter()
            .find(|comment| comment.attribute("ref").unwrap_or("").to_uppercase() == address);
        if let Some(comment) = found {
            return xml::text(comment).to_lowercase();
        }
    }
    String::new()
}

/// Whether a conditional-format formula on the sheet holds `needle`, read in order as `any()` reads them.
fn any_conditional_formula(parts: &Parts, path: &str, needle: &str) -> bool {
    let text = part(parts, path);
    let sheet = xml::parse(&text);
    xml::descendants(sheet.root_element(), MAIN, "conditionalFormatting")
        .flat_map(|formatting| xml::path(formatting, MAIN, &["cfRule", "formula"]))
        .any(|formula| {
            formula
                .text()
                .expect("a conditional formula with text")
                .contains(needle)
        })
}

fn forecast_formula_contract(column: char, previous: Option<char>) -> Vec<(usize, String)> {
    let c = column;
    vec![
        (
            7,
            format!(r#"IF({c}6<='Assumptions'!$B$9,"Actual","Forecast")"#),
        ),
        (
            18,
            format!(
                r#"IF({c}$7="Actual",0,SUM({c}10:{c}12)*(INDEX(Assumptions!$C$15:$C$17,MATCH(Assumptions!$B$12,Assumptions!$B$15:$B$17,0))-1))"#
            ),
        ),
        (19, format!("SUM({c}10:{c}18)")),
        (
            42,
            format!(
                r#"IF({c}$7="Actual",0,SUM({c}22,{c}31,{c}33,{c}41)*(INDEX(Assumptions!$D$15:$D$17,MATCH(Assumptions!$B$12,Assumptions!$B$15:$B$17,0))-1))"#
            ),
        ),
        (43, format!("SUM({c}22:{c}42)")),
        (
            46,
            previous.map_or("'Assumptions'!$B$10".to_owned(), |p| format!("{p}48")),
        ),
        (47, format!("{c}19-{c}43")),
        (48, format!("{c}46+{c}47")),
        (49, "'Assumptions'!$B$11".to_owned()),
        (50, format!("{c}48-{c}49")),
        (
            51,
            format!(r#"IF({c}48<{c}49,"BELOW BUFFER",IF({c}48<={c}49*1.25,"WATCH","OK"))"#),
        ),
    ]
}

fn cell_row(reference: &str) -> usize {
    let digits: String = reference.chars().filter(char::is_ascii_digit).collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("{reference:?} has no row"))
}

fn row_bounds(reference: &str) -> (usize, usize) {
    let rows: Vec<usize> = reference.split(':').map(cell_row).collect();
    (
        *rows.iter().min().expect("a row"),
        *rows.iter().max().expect("a row"),
    )
}

fn column_number(reference: &str) -> usize {
    reference
        .chars()
        .filter(char::is_ascii_alphabetic)
        .fold(0, |value, c| {
            value * 26 + usize::from(c.to_ascii_uppercase() as u8 - b'A') + 1
        })
}

fn column_width(parts: &Parts, path: &str, reference: &str) -> Option<f64> {
    let target = column_number(reference) as i64;
    let text = part(parts, path);
    let sheet = xml::parse(&text);
    xml::path(sheet.root_element(), MAIN, &["cols", "col"])
        .into_iter()
        .find(|column| int(attr(*column, "min")) <= target && target <= int(attr(*column, "max")))
        .map(|column| float(attr(column, "width")))
}

fn relationship_target(parts: &Parts, source: &str, id: &str) -> String {
    let name = source.rsplit('/').next().unwrap_or(source);
    let rels = part(
        parts,
        &join(&join(&dirname(source), "_rels"), &format!("{name}.rels")),
    );
    let rels = xml::parse(&rels);
    let target = xml::elements(rels.root_element())
        .find(|item| attr(*item, "Id") == id)
        .map(|item| attr(item, "Target"))
        .unwrap_or_else(|| panic!("{source} has no relationship {id}"));
    normpath(&join(&dirname(source), target))
        .trim_start_matches('/')
        .to_owned()
}

#[derive(Debug)]
struct Chart {
    kind: String,
    bar_direction: Option<String>,
    grouping: Option<String>,
    title: String,
    categories: Vec<Option<String>>,
    series: Vec<Option<String>>,
    anchor: [i64; 4],
}

fn chart(parts: &Parts, path: &str, anchor: Node<'_, '_>) -> Chart {
    let text = part(parts, path);
    let root = xml::parse(&text);
    let root = root.root_element();
    let plot_area = xml::path(root, CHART, &["chart", "plotArea"])
        .into_iter()
        .next()
        .expect("a plot area");
    let element = xml::elements(plot_area)
        .next()
        .expect("a chart in the plot area");
    let val = |name: &'static str| {
        xml::child(element, CHART, name)
            .and_then(|node| node.attribute("val"))
            .map(str::to_owned)
    };
    let formulas = |name: &'static str| {
        xml::descendants(root, CHART, name)
            .flat_map(|node| xml::descendants(node, CHART, "f"))
            .map(|formula| formula.text().map(str::to_owned))
            .collect::<Vec<_>>()
    };
    let coordinate = |edge: &'static str, coordinate: &'static str| {
        let node = xml::path(anchor, DRAWING, &[edge, coordinate])
            .into_iter()
            .next();
        int(node
            .unwrap_or_else(|| panic!("no {edge}/{coordinate}"))
            .text()
            .unwrap_or(""))
    };
    Chart {
        kind: element.tag_name().name().to_owned(),
        bar_direction: val("barDir"),
        grouping: val("grouping"),
        title: xml::descendants(root, DRAWING_MAIN, "t")
            .map(|t| t.text().unwrap_or(""))
            .collect(),
        categories: formulas("cat"),
        series: formulas("val"),
        anchor: [
            ("from", "col"),
            ("from", "row"),
            ("to", "col"),
            ("to", "row"),
        ]
        .map(|(e, c)| coordinate(e, c)),
    }
}

fn dashboard_charts(parts: &Parts, dashboard_path: &str) -> Vec<Chart> {
    let dashboard = part(parts, dashboard_path);
    let dashboard = xml::parse(&dashboard);
    let drawing = xml::child(dashboard.root_element(), MAIN, "drawing").expect("a drawing");
    let drawing_path = relationship_target(parts, dashboard_path, relationship_id(drawing));
    let text = part(parts, &drawing_path);
    let drawing = xml::parse(&text);
    xml::children(drawing.root_element(), DRAWING, "twoCellAnchor")
        .filter_map(|anchor| {
            let chart_ref = xml::descendants(anchor, CHART, "chart").next()?;
            let chart_path = relationship_target(parts, &drawing_path, relationship_id(chart_ref));
            Some(chart(parts, &chart_path, anchor))
        })
        .collect()
}

fn max_abs_formula(expressions: &[String]) -> String {
    let terms: Vec<String> = expressions.iter().map(|e| format!("ABS({e})")).collect();
    format!("MAX({})", terms.join(","))
}

fn cash_roll_forward_check_formula() -> String {
    let columns: Vec<char> = FORECAST_COLUMNS.chars().collect();
    let equations = columns.iter().map(|c| {
        format!("'13-Week Forecast'!${c}$48-'13-Week Forecast'!${c}$46-'13-Week Forecast'!${c}$47")
    });
    let continuity = columns.windows(2).map(|pair| {
        format!(
            "'13-Week Forecast'!${}$46-'13-Week Forecast'!${}$48",
            pair[1], pair[0]
        )
    });
    max_abs_formula(&equations.chain(continuity).collect::<Vec<_>>())
}

fn weekly_total_check_formula(total: usize, start: usize, end: usize) -> String {
    let terms: Vec<String> = FORECAST_COLUMNS
        .chars()
        .map(|c| {
            format!(
                "'13-Week Forecast'!${c}${total}-SUM('13-Week Forecast'!${c}${start}:${c}${end})"
            )
        })
        .collect();
    max_abs_formula(&terms)
}

/// Each conditional rule's formula with its style's font colour and fill background.
fn conditional_rule_styles(
    parts: &Parts,
    path: &str,
) -> HashMap<String, (Option<String>, Option<String>)> {
    let (sheet, styles) = (part(parts, path), part(parts, "xl/styles.xml"));
    let (sheet, styles) = (xml::parse(&sheet), xml::parse(&styles));
    let dxfs = style_list(&styles, "dxfs");
    let mut rules = HashMap::new();
    for formatting in xml::descendants(sheet.root_element(), MAIN, "conditionalFormatting") {
        for rule in xml::children(formatting, MAIN, "cfRule") {
            let dxf = xml::nth(dxfs, attr(rule, "dxfId").parse().expect("a dxf index"));
            let first =
                |steps: &[&'static str]| rgb(xml::path(dxf, MAIN, steps).into_iter().next());
            let styled = (
                first(&["font", "color"]),
                first(&["fill", "patternFill", "bgColor"]),
            );
            // A rule without a formula is keyed None in Python, which no lookup below uses.
            if let Some(formula) = xml::child(rule, MAIN, "formula") {
                rules.insert(formula.text().unwrap_or("").to_owned(), styled);
            }
        }
    }
    rules
}

fn rule(font: &str, fill: &str) -> (Option<String>, Option<String>) {
    (Some(font.to_owned()), Some(fill.to_owned()))
}

/// Add an element as the last child of the part's root, before its closing tag.
fn append_child(parts: &mut Parts, name: &str, closing: &str, element: &str) {
    let mut text = part(parts, name);
    let at = text
        .rfind(closing)
        .unwrap_or_else(|| panic!("{name} has no {closing}"));
    text.insert_str(at, element);
    parts.insert(name.to_owned(), text.into_bytes());
}

#[test]
fn package_contract_rejects_internal_external_link_parts() {
    // An internal externalLink package must fail even without TargetMode=External.
    let mut parts = parts(WORKBOOK);
    let external_link = "xl/externalLinks/externalLink1.xml";
    parts.insert(external_link.to_owned(), b"<externalLink/>".to_vec());
    append_child(
        &mut parts,
        "[Content_Types].xml",
        "</Types>",
        &format!(
            r#"<Override PartName="/{external_link}" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.externalLink+xml"/>"#
        ),
    );
    append_child(
        &mut parts,
        "xl/_rels/workbook.xml.rels",
        "</Relationships>",
        r#"<Relationship Id="rIdSyntheticExternalLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLink" Target="/xl/externalLinks/externalLink1.xml"/>"#,
    );
    assert!(assert_package_and_formula_safe(&parts).is_err());
}

#[test]
fn scaffold_sheets_and_forecast_headers() {
    // The artifact-tool scaffold is the production change that makes this pass.
    assert!(
        root().join(WORKBOOK).is_file(),
        "Production change needed: run the artifact-tool scaffold generator to create {WORKBOOK}"
    );
    let parts = parts(WORKBOOK);
    let names: Vec<String> = sheet_paths(&parts)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, EXPECTED_SHEETS);
    let forecast_path = &sheet_map(&parts)["13-Week Forecast"];
    let forecast = values_by_address(&parts, forecast_path);
    for (row, label) in FORECAST_LABELS {
        assert_eq!(forecast.get(&format!("A{row}")), Some(label), "A{row}");
    }
    let week_starts: Vec<String> = (0..13)
        .map(|week| excel_serial(start_date() + Days::new(7 * week)))
        .collect();
    assert_eq!(forecast.row(FORECAST_COLUMNS, 5), texts(&week_starts));
    assert_eq!(forecast.get("O5"), Some("13-week total / terminal"));
    assert_eq!(forecast.get("O6"), Some("Week 13 value"));
    assert_eq!(forecast.get("O7"), Some(""));
    let text = part(&parts, forecast_path);
    let sheet = xml::parse(&text);
    let after_terminal: Vec<&str> = xml::descendants(sheet.root_element(), MAIN, "c")
        .filter(|cell| {
            let reference = attr(*cell, "r");
            [5, 6, 7].contains(&cell_row(reference))
                && column_number(reference) > 15
                && ["v", "f", "is"]
                    .into_iter()
                    .any(|name| xml::child(*cell, MAIN, name).is_some())
        })
        .map(|cell| attr(cell, "r"))
        .collect();
    assert_eq!(after_terminal, Vec::<&str>::new());
}

#[test]
fn start_here_title_and_version() {
    let parts = parts(WORKBOOK);
    let start = values_by_address(&parts, &sheet_map(&parts)["Start Here"]);
    assert_eq!(start.get("A1"), Some("Ozzit | 13-Week Cash-Flow Forecast"));
    assert_eq!(
        start.get("A2"),
        Some(
            "A practical weekly liquidity model for Australian finance teams and small-business operators."
        )
    );
    assert!(start.or_empty("A4").contains("ILLUSTRATIVE DATA"));
    assert_eq!(start.get("D6"), Some("Workbook sheets"));
    assert_eq!(start.get("A6"), Some("Version"));
    assert_eq!(start.get("B6"), Some("Release 1.2"));
    assert_eq!(start.get("A7"), Some("Prepared date"));
    assert_eq!(
        start.get("B7"),
        Some(excel_serial(date(2026, 8, 27)).as_str())
    );
}

#[test]
fn start_here_process_legend_and_sheet_list() {
    let parts = parts(WORKBOOK);
    let start = values_by_address(&parts, &sheet_map(&parts)["Start Here"]);
    assert_eq!(start.get("A16"), Some("Five-step weekly update process"));
    assert_eq!(start.column('A', 17..22), texts(&["1", "2", "3", "4", "5"]));
    assert_eq!(
        start.column('B', 17..22),
        texts(&[
            "Update the Assumptions control panel and selected scenario.",
            "Replace blue cash-receipt and cash-payment inputs with the latest weekly view.",
            "Review closing cash, headroom and liquidity status in the 13-Week Forecast.",
            "Snapshot the original forecast as values, then record actuals and owner commentary in Weekly Review.",
            "Resolve any control failure in Checks & Sources before management review.",
        ])
    );
    assert_eq!(start.get("A23"), Some("Colour legend"));
    assert_eq!(
        start.column('A', 24..29),
        texts(&[
            "Blue text / ivory fill",
            "Black text",
            "Green text",
            "Red text / pale-red fill",
            "Dark green / pale-green fill",
        ])
    );
    assert_eq!(
        start.column('B', 24..29),
        texts(&[
            "Editable user input",
            "Formula or calculation",
            "Cross-sheet link",
            "Warning or exception",
            "Passing check",
        ])
    );
    assert!(
        start
            .or_empty("A31")
            .contains("not tax, payroll, legal or financial advice")
    );
    for (row, sheet) in (7..).zip(&EXPECTED_SHEETS[1..]) {
        assert_eq!(start.get(&format!("D{row}")), Some(*sheet), "D{row}");
    }
}

#[test]
fn defined_names() {
    let parts = parts(WORKBOOK);
    let text = part(&parts, "xl/workbook.xml");
    let book = xml::parse(&text);
    let defined: HashMap<&str, Option<&str>> =
        xml::path(book.root_element(), MAIN, &["definedNames", "definedName"])
            .into_iter()
            .map(|item| (attr(item, "name"), item.text()))
            .collect();
    assert_eq!(
        defined,
        HashMap::from([
            ("CashFlowScenario", Some("'Assumptions'!$B$12")),
            ("CashFlowSelectedScenario", Some("'Assumptions'!$B$12")),
            ("CashFlowScenarioList", Some("'Assumptions'!$B$15:$B$17")),
        ])
    );
}

#[test]
fn assumptions_inputs_and_scenarios() {
    let parts = parts(WORKBOOK);
    let assumptions = values_by_address(&parts, &sheet_map(&parts)["Assumptions"]);
    let (start, as_at) = (excel_serial(start_date()), excel_serial(date(2026, 9, 6)));
    for (address, expected) in [
        ("B5", "Illustrative Australian business"),
        ("B6", "AUD"),
        ("B7", "Whole dollars"),
        ("B8", start.as_str()),
        ("B9", as_at.as_str()),
        ("B10", "400000"),
        ("B11", "100000"),
        ("B12", "Base"),
    ] {
        assert_eq!(assumptions.get(address), Some(expected), "{address}");
    }
    assert_eq!(
        assumptions.column('B', 15..18),
        texts(&["Base", "Upside", "Downside"])
    );
    assert_eq!(
        assumptions.column('C', 15..18),
        texts(&["1", "1.08", "0.85"])
    );
    assert_eq!(
        assumptions.column('D', 15..18),
        texts(&["1", "0.98", "1.05"])
    );
    assert_eq!(
        assumptions.get("A19"),
        Some("Liquidity action lead time (weeks)")
    );
    assert_eq!(assumptions.get("B19"), Some("2"));
    assert_eq!(assumptions.get("A26"), Some("Data-quality notes"));
    assert!(
        assumptions
            .or_empty("A27")
            .contains("Monday forecast start")
    );
}

#[test]
fn assumptions_styles_widths_and_validations() {
    let parts = parts(WORKBOOK);
    let map = sheet_map(&parts);
    let input_style = style_for_cell(&parts, &map["Assumptions"], "B10");
    let link_style = style_for_cell(&parts, &map["Start Here"], "D7");
    assert_eq!(input_style, colours("FF0000FF", Some("FFE5E4E2")));
    assert_eq!(link_style, colours("FF000000", None));
    assert_ne!(link_style, input_style);
    let width = column_width(&parts, &map["Assumptions"], "D").expect("a width for column D");
    assert!(
        width >= 22.0,
        "Assumptions!D14 must show 'Variable-payment factor' without clipping into Description."
    );
    let text = part(&parts, &map["Assumptions"]);
    let sheet = xml::parse(&text);
    let validations = xml::path(
        sheet.root_element(),
        MAIN,
        &["dataValidations", "dataValidation"],
    );
    let ranges: HashSet<Option<&str>> = validations.iter().map(|v| v.attribute("sqref")).collect();
    assert_eq!(ranges, HashSet::from([Some("B12"), Some("B19")]));
    let validation = |range| {
        *validations
            .iter()
            .find(|v| v.attribute("sqref") == Some(range))
            .expect("a validation")
    };
    let formula = |node: Node<'_, '_>, name| {
        xml::child(node, MAIN, name).map(|f| f.text().unwrap_or("").to_owned())
    };
    let (scenario, lead_time) = (validation("B12"), validation("B19"));
    assert_eq!(
        formula(scenario, "formula1").as_deref(),
        Some("CashFlowScenarioList")
    );
    assert_eq!(lead_time.attribute("type"), Some("whole"));
    assert_eq!(lead_time.attribute("operator"), Some("between"));
    assert_eq!(formula(lead_time, "formula1").as_deref(), Some("0"));
    assert_eq!(formula(lead_time, "formula2").as_deref(), Some("13"));
}

#[test]
fn sheet_views_margins_and_merges() {
    let parts = parts(WORKBOOK);
    for (sheet, path) in sheet_paths(&parts) {
        let text = part(&parts, &path);
        let root = xml::parse(&text);
        let view = xml::path(root.root_element(), MAIN, &["sheetViews", "sheetView"]);
        let view = view
            .first()
            .unwrap_or_else(|| panic!("{sheet} has no sheet view"));
        assert_eq!(view.attribute("showGridLines"), Some("0"), "{sheet}");
        let margins = xml::child(root.root_element(), MAIN, "pageMargins");
        let margins = margins.unwrap_or_else(|| panic!("{sheet} has no page margins"));
        let positive = ["left", "right", "top", "bottom"]
            .into_iter()
            .all(|key| float(attr(margins, key)) > 0.0);
        assert!(positive, "{sheet}");
    }
    let text = part(&parts, &sheet_map(&parts)["13-Week Forecast"]);
    let forecast = xml::parse(&text);
    let calculation_merges: Vec<&str> =
        xml::path(forecast.root_element(), MAIN, &["mergeCells", "mergeCell"])
            .into_iter()
            .map(|merge| attr(merge, "ref"))
            .filter(|reference| row_bounds(reference).0 <= 51 && row_bounds(reference).1 >= 5)
            .collect();
    assert_eq!(calculation_merges, Vec::<&str>::new());
}

#[test]
fn package_safety_banned_text_and_palette() {
    let parts = parts(WORKBOOK);
    assert_package_and_formula_safe(&parts).unwrap_or_else(|e| panic!("{e}"));
    assert!(common::matches(&VOLATILE_PATTERN, "=_xlfn.RANDARRAY(2,2)"));
    let payloads: Vec<Vec<u8>> = parts
        .iter()
        .filter(|(name, _)| name.ends_with(".xml") || name.ends_with(".rels"))
        .map(|(_, payload)| payload.to_ascii_lowercase())
        .collect();
    let all_xml = payloads.join(&b' ');
    for banned in BANNED_SOURCE_TEXT {
        assert!(!contains(&all_xml, banned.as_bytes()), "{banned}");
    }
    let styles = parts["xl/styles.xml"].to_ascii_uppercase();
    for colour in PALETTE_ANCHORS {
        assert!(contains(&styles, colour.as_bytes()), "{colour}");
    }
}

#[test]
fn fonts_and_number_formats() {
    let parts = parts(WORKBOOK);
    for (sheet, path) in sheet_paths(&parts) {
        for address in ["A1", "A2"] {
            let font = font_and_number_format(&parts, &path, address).0;
            assert_eq!(font.as_deref(), Some("Aptos"), "{sheet}!{address}");
        }
    }
    assert_eq!(
        used_cell_font_names(&parts),
        HashSet::from([Some("Aptos".to_owned())])
    );
    let map = sheet_map(&parts);
    for (sheet, address, format) in [
        ("Start Here", "B7", DATE_FORMAT),
        ("Assumptions", "B8", DATE_FORMAT),
        ("Assumptions", "B10", AUD_FORMAT),
        ("Assumptions", "C15", PERCENT_FORMAT),
    ] {
        assert_eq!(
            font_and_number_format(&parts, &map[sheet], address).1,
            format,
            "{sheet}!{address}"
        );
    }
}

#[test]
fn forecast_labels_closing_cash_and_formulas() {
    // The formula-driven forecast generator is the production change that makes this pass.
    let parts = parts(WORKBOOK);
    let forecast_path = &sheet_map(&parts)["13-Week Forecast"];
    let values = values_by_address(&parts, forecast_path);
    let formulas = formulas_by_address(&parts, forecast_path);
    for (row, label) in FORECAST_ROW_MAP {
        assert_eq!(values.get(&format!("A{row}")), Some(*label), "A{row}");
    }
    let mut status = vec![Some("Actual")];
    status.extend([Some("Forecast"); 12]);
    assert_eq!(values.row(FORECAST_COLUMNS, 7), status);
    let closing: Vec<i64> = FORECAST_COLUMNS
        .chars()
        .map(|c| values.whole(&format!("{c}48")))
        .collect();
    assert_eq!(closing, BASE_CLOSING_CASH);
    let mut previous = None;
    for column in FORECAST_COLUMNS.chars() {
        for (row, expected) in forecast_formula_contract(column, previous) {
            assert_eq!(
                formulas.get(&format!("{column}{row}")),
                Some(expected.as_str()),
                "{column}{row}"
            );
        }
        previous = Some(column);
    }
}

#[test]
fn forecast_totals_and_terminal_values() {
    let parts = parts(WORKBOOK);
    let forecast_path = &sheet_map(&parts)["13-Week Forecast"];
    let values = values_by_address(&parts, forecast_path);
    let formulas = formulas_by_address(&parts, forecast_path);
    for row in (10..20).chain(22..44).chain([47]) {
        let total = format!("O{row}");
        assert_eq!(
            formulas.get(&total),
            Some(format!("SUM(B{row}:N{row})").as_str()),
            "{total}"
        );
        let weeks: i64 = FORECAST_COLUMNS
            .chars()
            .map(|c| values.whole(&format!("{c}{row}")))
            .sum();
        assert_eq!(values.whole(&total), weeks, "{total}");
    }
    for row in TERMINAL_ROWS {
        assert_eq!(
            formulas.get(&format!("O{row}")),
            Some(format!("N{row}").as_str()),
            "O{row}"
        );
        assert_eq!(
            values.get(&format!("O{row}")),
            values.get(&format!("N{row}")),
            "O{row}"
        );
    }
    assert_eq!(formulas.get("O47"), Some("SUM(B47:N47)"));
    assert_eq!(values.get("O47"), Some("-369000"));
}

#[test]
fn forecast_styles_rules_and_comments() {
    let parts = parts(WORKBOOK);
    let forecast_path = &sheet_map(&parts)["13-Week Forecast"];
    assert_eq!(
        style_for_cell(&parts, forecast_path, "B10"),
        colours(INPUT.0, Some(INPUT.1))
    );
    assert_eq!(
        style_for_cell(&parts, forecast_path, "B18").0.as_deref(),
        Some("FF000000")
    );
    assert_eq!(
        style_for_cell(&parts, forecast_path, "B46").0.as_deref(),
        Some("FF008000")
    );
    assert_eq!(
        font_and_number_format(&parts, forecast_path, "B5").1,
        DATE_FORMAT
    );
    assert_eq!(
        font_and_number_format(&parts, forecast_path, "B47").1,
        AUD_FORMAT
    );
    for status in ["BELOW BUFFER", "WATCH", "OK"] {
        assert!(
            any_conditional_formula(&parts, forecast_path, status),
            "{status}"
        );
    }
    let comments = comments_text(&parts);
    for phrase in [
        "scenario receipt",
        "scenario payment",
        "gst / bas",
        "payg income-tax",
    ] {
        assert!(contains(&comments, phrase.as_bytes()), "{phrase}");
    }
    assert!(
        comment_text(&parts, "B18")
            .contains("current customer receipts, overdue receipts and cash / eftpos sales")
    );
}

fn review_formulas(row: usize) -> [(char, String); 5] {
    [
        ('E', format!(r#"IF(D{row}="","",D{row}-C{row})"#)),
        (
            'F',
            format!(r#"IF(OR(D{row}="",C{row}=0),"",(D{row}-C{row})/C{row})"#),
        ),
        ('I', format!(r#"IF(H{row}="","",G{row}-H{row})"#)),
        (
            'J',
            format!(r#"IF(OR(H{row}="",G{row}=0),"",(G{row}-H{row})/G{row})"#),
        ),
        ('M', format!(r#"IF(L{row}="","",L{row}-K{row})"#)),
    ]
}

#[test]
fn weekly_review_headers_and_note() {
    let parts = parts(WORKBOOK);
    let values = values_by_address(&parts, &sheet_map(&parts)["Weekly Review"]);
    assert_eq!(
        values.row("ABCDEFGHIJKLMN", 5),
        texts(&[
            "Snapshot week start",
            "Snapshot week end",
            "Forecast snapshot receipts",
            "Actual receipts",
            "Receipt variance $",
            "Receipt variance %",
            "Forecast snapshot payments",
            "Actual payments",
            "Payment variance $",
            "Payment variance %",
            "Forecast snapshot closing cash",
            "Actual closing cash",
            "Closing-cash variance",
            "Owner commentary",
        ])
    );
    let note = values.or_empty("A4").to_lowercase();
    assert!(
        note.contains("as values") && note.contains("must not link"),
        "{note}"
    );
}

#[test]
fn weekly_review_snapshot_and_variance_formulas() {
    let parts = parts(WORKBOOK);
    let map = sheet_map(&parts);
    let forecast = values_by_address(&parts, &map["13-Week Forecast"]);
    let review_path = &map["Weekly Review"];
    let (values, formulas) = (
        values_by_address(&parts, review_path),
        formulas_by_address(&parts, review_path),
    );
    for (row, column) in (6..).zip(FORECAST_COLUMNS.chars()) {
        for (review_column, expected) in review_formulas(row) {
            assert_eq!(
                formulas.get(&format!("{review_column}{row}")),
                Some(expected.as_str())
            );
        }
        for (snapshot, source) in [('A', 5), ('B', 6), ('C', 19), ('G', 43), ('K', 48)] {
            let (at, from) = (format!("{snapshot}{row}"), format!("{column}{source}"));
            assert_eq!(values.get(&at), forecast.get(&from), "{at}");
            assert!(!formulas.has(&at), "{at} is a formula");
        }
        for input in "DHLN".chars() {
            assert_eq!(
                values.get(&format!("{input}{row}")),
                Some(""),
                "{input}{row}"
            );
        }
        for variance in "EFIJM".chars() {
            assert_eq!(
                cached_value(&parts, review_path, &format!("{variance}{row}")),
                None,
                "{variance}{row}"
            );
        }
    }
}

#[test]
fn weekly_review_styles_and_rules() {
    let parts = parts(WORKBOOK);
    let review_path = &sheet_map(&parts)["Weekly Review"];
    for address in ["A6", "B6", "C6", "G6", "K6", "D6", "H6", "L6", "N6"] {
        assert_eq!(
            style_for_cell(&parts, review_path, address),
            colours(INPUT.0, Some(INPUT.1)),
            "{address}"
        );
    }
    assert_eq!(
        font_and_number_format(&parts, review_path, "A6").1,
        DATE_FORMAT
    );
    assert_eq!(
        font_and_number_format(&parts, review_path, "C6").1,
        AUD_FORMAT
    );
    assert_eq!(
        font_and_number_format(&parts, review_path, "F6").1,
        PERCENT_FORMAT
    );
    let styles = conditional_rule_styles(&parts, review_path);
    for formula in ["E6>0", "I6>0", "M6>0"] {
        assert_eq!(styles[formula], rule(PASSING.0, PASSING.1), "{formula}");
    }
    for formula in ["E6<0", "I6<0", "M6<0"] {
        assert_eq!(styles[formula], rule(FAILING.0, FAILING.1), "{formula}");
    }
    let text = part(&parts, review_path);
    let review = xml::parse(&text);
    assert!(
        xml::path(
            review.root_element(),
            MAIN,
            &["dataValidations", "dataValidation"]
        )
        .is_empty()
    );
}

#[test]
fn dashboard_kpi_headers_and_formulas() {
    // The dashboard-and-controls generator change is required for this contract.
    let parts = parts(WORKBOOK);
    let dashboard_path = &sheet_map(&parts)["Dashboard"];
    let (values, formulas) = (
        values_by_address(&parts, dashboard_path),
        formulas_by_address(&parts, dashboard_path),
    );
    assert_eq!(
        values.row("ABCDEFGHIJ", 6),
        texts(&[
            "Selected scenario",
            "Week 13 closing cash",
            "Lowest closing cash",
            "Minimum cash buffer",
            "Minimum headroom",
            "Weeks below buffer",
            "13-week net cash change",
            "Week of lowest cash",
            "MODEL STATUS",
            "LIQUIDITY STATUS",
        ])
    );
    for (address, expected) in [
        ("A7", "'Assumptions'!$B$12"),
        ("B7", "'13-Week Forecast'!$N$48"),
        ("C7", "MIN('13-Week Forecast'!$B$48:$N$48)"),
        ("D7", "'Assumptions'!$B$11"),
        ("E7", "MIN('13-Week Forecast'!$B$50:$N$50)"),
        ("F7", r#"COUNTIF('13-Week Forecast'!$B$50:$N$50,"<0")"#),
        ("G7", "SUM('13-Week Forecast'!$B$47:$N$47)"),
        (
            "H7",
            "INDEX('13-Week Forecast'!$B$6:$N$6,1,MATCH(C7,'13-Week Forecast'!$B$48:$N$48,0))",
        ),
        ("I7", "'Checks & Sources'!$B$4"),
        ("J7", "'Checks & Sources'!$E$4"),
    ] {
        assert_eq!(formulas.get(address), Some(expected), "{address}");
    }
}

#[test]
fn dashboard_kpi_values_and_formats() {
    let parts = parts(WORKBOOK);
    let dashboard_path = &sheet_map(&parts)["Dashboard"];
    let values = values_by_address(&parts, dashboard_path);
    let lowest_week = excel_serial(date(2026, 11, 29));
    let kpis = [
        "Base",
        "31000",
        "31000",
        "100000",
        "-69000",
        "3",
        "-369000",
        lowest_week.as_str(),
        "PASS",
        "ACTION REQUIRED",
    ];
    assert_eq!(values.row("ABCDEFGHIJ", 7), texts(&kpis));
    for address in ["B7", "C7", "D7", "E7", "G7"] {
        assert_eq!(
            font_and_number_format(&parts, dashboard_path, address).1,
            AUD_FORMAT,
            "{address}"
        );
    }
    assert_eq!(
        font_and_number_format(&parts, dashboard_path, "H7").1,
        DATE_FORMAT
    );
}

#[test]
fn dashboard_liquidity_actions() {
    let parts = parts(WORKBOOK);
    let dashboard_path = &sheet_map(&parts)["Dashboard"];
    let (values, formulas) = (
        values_by_address(&parts, dashboard_path),
        formulas_by_address(&parts, dashboard_path),
    );
    let actions = [
        "First buffer breach",
        "Weeks until breach",
        "Funding required",
        "Action deadline",
    ];
    assert_eq!(values.row("ABCD", 9), texts(&actions));
    for (address, expected) in [
        (
            "A10",
            r#"IF(COUNTIF('13-Week Forecast'!$B$51:$N$51,"BELOW BUFFER")=0,"None",INDEX('13-Week Forecast'!$B$6:$N$6,1,MATCH("BELOW BUFFER",'13-Week Forecast'!$B$51:$N$51,0)))"#,
        ),
        (
            "B10",
            r#"IF(A10="None","13+",MAX(0,ROUNDUP((A10-'Assumptions'!$B$9)/7,0)))"#,
        ),
        ("C10", "MAX(0,-MIN('13-Week Forecast'!$B$50:$N$50))"),
        ("D10", r#"IF(A10="None","None",A10-7*'Assumptions'!$B$19)"#),
    ] {
        assert_eq!(formulas.get(address), Some(expected), "{address}");
    }
    let (breach, deadline) = (
        excel_serial(date(2026, 11, 15)),
        excel_serial(date(2026, 11, 1)),
    );
    assert_eq!(
        values.row("ABCD", 10),
        texts(&[breach.as_str(), "10", "69000", deadline.as_str()])
    );
    for (address, format) in [
        ("A10", DATE_FORMAT),
        ("C10", AUD_FORMAT),
        ("D10", DATE_FORMAT),
    ] {
        assert_eq!(
            font_and_number_format(&parts, dashboard_path, address).1,
            format,
            "{address}"
        );
    }
}

#[test]
fn dashboard_weekly_tables() {
    let parts = parts(WORKBOOK);
    let dashboard_path = &sheet_map(&parts)["Dashboard"];
    let (values, formulas) = (
        values_by_address(&parts, dashboard_path),
        formulas_by_address(&parts, dashboard_path),
    );
    let headers = [
        "Week",
        "Week ending",
        "Closing cash",
        "Headroom",
        "Liquidity status",
    ];
    assert_eq!(values.row("ABCDE", 11), texts(&headers));
    let check = |address: String, expected: String| {
        assert_eq!(formulas.get(&address), Some(expected.as_str()), "{address}");
    };
    for (index, column) in (12..).zip(FORECAST_COLUMNS.chars()) {
        check(format!("A{index}"), "ROW()-11".to_owned());
        for (dashboard, row) in [('B', 6), ('C', 48), ('D', 50), ('E', 51)] {
            check(
                format!("{dashboard}{index}"),
                format!("'13-Week Forecast'!{column}${row}"),
            );
        }
    }
    assert_eq!(
        values.row("PQR", 4),
        texts(&["Week ending", "Closing cash", "Minimum buffer"])
    );
    assert_eq!(
        values.row("PQR", 21),
        texts(&["Week ending", "Total cash receipts", "Total cash payments"])
    );
    for (first, rows) in [(5, [48, 49]), (22, [19, 43])] {
        for (index, column) in (first..).zip(FORECAST_COLUMNS.chars()) {
            check(
                format!("P{index}"),
                format!(r#"TEXT('13-Week Forecast'!{column}$6,"dd mmm")"#),
            );
            check(
                format!("Q{index}"),
                format!("'13-Week Forecast'!{column}${}", rows[0]),
            );
            check(
                format!("R{index}"),
                format!("'13-Week Forecast'!{column}${}", rows[1]),
            );
        }
    }
}

#[test]
fn dashboard_scenario_comparison() {
    let parts = parts(WORKBOOK);
    let dashboard_path = &sheet_map(&parts)["Dashboard"];
    let (values, formulas) = (
        values_by_address(&parts, dashboard_path),
        formulas_by_address(&parts, dashboard_path),
    );
    let headers = [
        "Scenario",
        "Week 13 cash",
        "Minimum cash",
        "First buffer breach",
        "Funding required",
    ];
    assert_eq!(values.row("ABCDE", 27), texts(&headers));
    let base_breach = excel_serial(date(2026, 11, 15));
    let downside_breach = excel_serial(date(2026, 10, 4));
    for (row, name, week_13, minimum, breach, funding) in [
        (28, "Base", 31_000, 31_000, base_breach.as_str(), 69_000),
        (29, "Upside", 199_320, 199_320, "None", 0),
        (
            30,
            "Downside",
            -302_550,
            -302_550,
            downside_breach.as_str(),
            402_550,
        ),
    ] {
        assert_eq!(values.get(&format!("A{row}")), Some(name));
        assert!(formulas.has(&format!("A{row}")), "A{row}");
        assert_eq!(values.whole(&format!("B{row}")), week_13, "B{row}");
        assert_eq!(values.whole(&format!("C{row}")), minimum, "C{row}");
        assert_eq!(values.get(&format!("D{row}")), Some(breach), "D{row}");
        assert_eq!(values.whole(&format!("E{row}")), funding, "E{row}");
        for column in "BCDE".chars() {
            assert!(formulas.has(&format!("{column}{row}")), "{column}{row}");
        }
    }
}

#[test]
fn dashboard_scenario_closing_cash_series() {
    let parts = parts(WORKBOOK);
    let dashboard_path = &sheet_map(&parts)["Dashboard"];
    let (values, formulas) = (
        values_by_address(&parts, dashboard_path),
        formulas_by_address(&parts, dashboard_path),
    );
    let headers = [
        "Week ending",
        "Base closing cash",
        "Upside closing cash",
        "Downside closing cash",
    ];
    assert_eq!(values.row("PQRS", 36), texts(&headers));
    for (column, expected) in [
        ('Q', BASE_CLOSING_CASH),
        ('R', UPSIDE_CLOSING_CASH),
        ('S', DOWNSIDE_CLOSING_CASH),
    ] {
        let actual: Vec<i64> = (37..50)
            .map(|row| values.whole(&format!("{column}{row}")))
            .collect();
        assert_eq!(actual, expected, "{column}");
        assert!(
            (37..50).all(|row| formulas.has(&format!("{column}{row}"))),
            "{column}"
        );
    }
    assert_eq!(
        values.row("QRS", 37),
        texts(&["358000", "358000", "358000"])
    );
}

#[test]
fn dashboard_charts_contract() {
    let parts = parts(WORKBOOK);
    let charts = dashboard_charts(&parts, &sheet_map(&parts)["Dashboard"]);
    assert_eq!(charts.len(), 2);
    let titled = |title| {
        charts
            .iter()
            .find(|chart| chart.title == title)
            .unwrap_or_else(|| panic!("no {title}"))
    };
    let line = titled("Closing cash vs minimum buffer (AUD)");
    let receipts = titled("Weekly receipts vs payments (AUD)");
    let formulas = |items: [&str; 2]| items.map(|item| Some(item.to_owned())).to_vec();
    assert_eq!(line.kind, "lineChart");
    assert_eq!(
        line.categories,
        formulas(["'Dashboard'!$P$5:$P$17", "'Dashboard'!$P$5:$P$17"])
    );
    assert_eq!(
        line.series,
        formulas(["'Dashboard'!$Q$5:$Q$17", "'Dashboard'!$R$5:$R$17"])
    );
    assert_eq!(line.anchor, [5, 9, 15, 25]);
    assert_eq!(receipts.kind, "barChart");
    assert_eq!(receipts.bar_direction.as_deref(), Some("col"));
    assert_eq!(receipts.grouping.as_deref(), Some("clustered"));
    assert_eq!(
        receipts.categories,
        formulas(["'Dashboard'!$P$22:$P$34", "'Dashboard'!$P$22:$P$34"])
    );
    assert_eq!(
        receipts.series,
        formulas(["'Dashboard'!$Q$22:$Q$34", "'Dashboard'!$R$22:$R$34"])
    );
    assert_eq!(receipts.anchor, [5, 26, 15, 42]);
}

#[expect(
    clippy::too_many_lines,
    reason = "Retains the 9 control rows of the Checks & Sources table together."
)]
fn expected_checks() -> [(
    &'static str,
    String,
    &'static str,
    &'static str,
    &'static str,
); 9] {
    [
        (
            "Exactly 13 forecast weeks",
            "COLUMNS('13-Week Forecast'!$B$5:$N$5)".into(),
            "13",
            "B8-C8",
            "13-Week Forecast!B5:N5",
        ),
        (
            "Forecast starts Monday",
            "WEEKDAY('Assumptions'!$B$8,2)".into(),
            "1",
            "B9-C9",
            "Assumptions!B8",
        ),
        (
            "Selected scenario recognised",
            "COUNTIF('Assumptions'!$B$15:$B$17,'Assumptions'!$B$12)".into(),
            "1",
            "B10-C10",
            "Assumptions!B12",
        ),
        (
            "Week 1 opening cash ties",
            "'13-Week Forecast'!$B$46".into(),
            "'Assumptions'!$B$10",
            "B11-C11",
            "13-Week Forecast!B46",
        ),
        (
            "Weekly cash roll-forward equations",
            cash_roll_forward_check_formula(),
            "0",
            "B12-C12",
            "13-Week Forecast!B46:N48",
        ),
        (
            "Cash receipt totals reconcile",
            weekly_total_check_formula(19, 10, 18),
            "0",
            "B13-C13",
            "13-Week Forecast!B19:N19",
        ),
        (
            "Cash payment totals reconcile",
            weekly_total_check_formula(43, 22, 42),
            "0",
            "B14-C14",
            "13-Week Forecast!B43:N43",
        ),
        (
            "Week 13 closing cash equation",
            "'13-Week Forecast'!$N$48".into(),
            "'Assumptions'!$B$10+SUM('13-Week Forecast'!$B$47:$N$47)",
            "B15-C15",
            "13-Week Forecast!N48",
        ),
        (
            "Formula-error count",
            FORMULA_ERROR_COUNT_FORMULA.into(),
            "0",
            "B16-C16",
            "Assumptions, Forecast, Weekly Review and Dashboard",
        ),
    ]
}

#[test]
fn checks_status_and_controls() {
    let parts = parts(WORKBOOK);
    let checks_path = &sheet_map(&parts)["Checks & Sources"];
    let (values, formulas) = (
        values_by_address(&parts, checks_path),
        formulas_by_address(&parts, checks_path),
    );
    assert_eq!(values.get("A4"), Some("MODEL STATUS"));
    assert_eq!(
        formulas.get("B4"),
        Some(r#"IF(COUNTIF(F8:F16,"PASS")=9,"PASS","FAIL")"#)
    );
    assert_eq!(values.get("B4"), Some("PASS"));
    assert_eq!(values.get("D4"), Some("LIQUIDITY STATUS"));
    assert_eq!(
        formulas.get("E4"),
        Some(
            r#"IF(COUNTIF('13-Week Forecast'!$B$51:$N$51,"BELOW BUFFER")>0,"ACTION REQUIRED",IF(COUNTIF('13-Week Forecast'!$B$51:$N$51,"WATCH")>0,"WATCH","OK"))"#
        )
    );
    assert_eq!(values.get("E4"), Some("ACTION REQUIRED"));
    let headers = [
        "Check",
        "Actual",
        "Expected",
        "Difference",
        "Tolerance",
        "Status",
        "Where to fix",
        "Notes",
    ];
    assert_eq!(values.row("ABCDEFGH", 7), texts(&headers));
    for (row, (label, actual, expected, difference, where_to_fix)) in (8..).zip(expected_checks()) {
        assert_eq!(values.get(&format!("A{row}")), Some(label));
        assert_eq!(
            formulas.get(&format!("B{row}")),
            Some(actual.as_str()),
            "B{row}"
        );
        assert_eq!(formulas.get(&format!("C{row}")), Some(expected), "C{row}");
        assert_eq!(formulas.get(&format!("D{row}")), Some(difference), "D{row}");
        assert_eq!(values.get(&format!("E{row}")), Some("0"), "E{row}");
        let status = format!(r#"IF(ABS(D{row})<=E{row},"PASS","FAIL")"#);
        assert_eq!(
            formulas.get(&format!("F{row}")),
            Some(status.as_str()),
            "F{row}"
        );
        assert_eq!(values.get(&format!("F{row}")), Some("PASS"), "F{row}");
        assert_eq!(values.get(&format!("G{row}")), Some(where_to_fix), "G{row}");
    }
}

/// The Checks & Sources source table: each row's columns B to F, by its column A label.
fn source_rows(values: &Values) -> HashMap<&str, [&str; 5]> {
    (22..27)
        .map(|row| {
            let columns =
                ['B', 'C', 'D', 'E', 'F'].map(|column| values.or_empty(&format!("{column}{row}")));
            (values.at(&format!("A{row}")), columns)
        })
        .collect()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Retains the 5 source rows and the disclaimer of one sheet together."
)]
fn checks_sources_and_disclaimer() {
    let parts = parts(WORKBOOK);
    let values = values_by_address(&parts, &sheet_map(&parts)["Checks & Sources"]);
    let rows = source_rows(&values);
    let (b, c, d, e) = (0, 1, 2, 3);
    let cell = |name: &str, column: usize| {
        rows.get(name).unwrap_or_else(|| panic!("no {name} source"))[column]
    };
    let joined = |name: &str| {
        rows.get(name)
            .unwrap_or_else(|| panic!("no {name} source"))
            .join(" ")
            .to_lowercase()
    };
    assert_eq!(
        cell("GST standard rate", b),
        "https://www.legislation.gov.au/C2004A00446/latest/text"
    );
    assert!(cell("GST standard rate", c).contains("GST default: 10%"));
    assert!(cell("GST standard rate", c).contains("taxable supply"));
    assert!(cell("GST standard rate", e).contains("does not decide"));
    assert_eq!(
        cell("BAS due dates", b),
        "https://www.ato.gov.au/businesses-and-organisations/preparing-lodging-and-paying/business-activity-statements-bas/due-dates-for-lodging-and-paying-your-bas"
    );
    assert!(
        cell("BAS due dates", c)
            .to_lowercase()
            .contains("planning pattern")
    );
    assert!(cell("BAS due dates", e).contains("issued or entity-specific BAS due date controls"));
    assert!(!joined("BAS due dates").contains("all entities"));
    assert_eq!(
        cell("PAYG withholding", b),
        "https://www.ato.gov.au/businesses-and-organisations/hiring-and-paying-your-workers/payg-withholding"
    );
    let payg = joined("PAYG withholding");
    for phrase in [
        "business size",
        "circumstances",
        "user-entered",
        "confirmed",
    ] {
        assert!(payg.contains(phrase), "{phrase}");
    }
    assert_eq!(
        cell("Super guarantee", b),
        "https://www.ato.gov.au/tax-rates-and-codes/key-superannuation-rates-and-thresholds/super-guarantee"
    );
    assert!(cell("Super guarantee", c).contains("12.0%"));
    assert!(
        cell("Super guarantee", e)
            .to_lowercase()
            .contains("worker eligibility")
    );
    assert_eq!(
        cell("Payday Super", b),
        "https://softwaredevelopers.ato.gov.au/PaydaySuper"
    );
    let payday = joined("Payday Super");
    assert!(payday.contains("from 1 july 2026"));
    assert!(payday.contains("generally receive it by the seventh business day after payday"));
    assert!(payday.contains("eligible longer period"));
    assert!(!payday.contains("must be received by the seventh business day after payday"));
    assert!(rows.values().all(|row| row[d] == "26 August 2026"));
    let disclaimer = values.or_empty("A28").to_lowercase();
    for phrase in [
        "illustrative fp&a cash-planning model only",
        "not tax, bas, payroll, superannuation or legal advice",
        "entity-specific lodgement dates and tax classifications must be confirmed by the user or adviser",
    ] {
        assert!(disclaimer.contains(phrase), "{phrase}");
    }
}

#[test]
fn dashboard_and_checks_styles() {
    let parts = parts(WORKBOOK);
    let map = sheet_map(&parts);
    let (dashboard_path, checks_path) = (&map["Dashboard"], &map["Checks & Sources"]);
    assert_eq!(
        style_for_cell(&parts, dashboard_path, "A1"),
        colours("FFFFFFF0", Some("FF66023C"))
    );
    assert_eq!(
        style_for_cell(&parts, dashboard_path, "A6"),
        colours("FFFFFFF0", Some("FF000000"))
    );
    assert_eq!(
        style_for_cell(&parts, dashboard_path, "B7"),
        colours("FF000000", Some("FFFFFFFF"))
    );
    assert_eq!(
        style_for_cell(&parts, checks_path, "A21"),
        colours("FFFFFFF0", Some("FF000000"))
    );
    let dashboard = conditional_rule_styles(&parts, dashboard_path);
    let checks = conditional_rule_styles(&parts, checks_path);
    assert_eq!(dashboard[r#"I7="PASS""#], rule(PASSING.0, PASSING.1));
    assert_eq!(
        dashboard[r#"J7="ACTION REQUIRED""#],
        rule(FAILING.0, FAILING.1)
    );
    assert_eq!(checks[r#"B4="PASS""#], rule(PASSING.0, PASSING.1));
    assert_eq!(
        checks[r#"E4="ACTION REQUIRED""#],
        rule(FAILING.0, FAILING.1)
    );
    assert_eq!(checks[r#"E4="OK""#], rule(PASSING.0, PASSING.1));
    assert_eq!(
        font_and_number_format(&parts, checks_path, "B11").1,
        AUD_FORMAT
    );
}

/// The governance roles Start Here shows and Assumptions records.
const GOVERNANCE_ROLES: [&str; 4] = [
    "Prepared by",
    "Reviewed by",
    "Forecast owner",
    "Next review date",
];

#[test]
fn governance_start_here_and_assumptions() {
    // Removing the V1.2 management controls must break this contract.
    let parts = parts(WORKBOOK);
    let map = sheet_map(&parts);
    let start_path = &map["Start Here"];
    let (start, start_formulas) = (
        values_by_address(&parts, start_path),
        formulas_by_address(&parts, start_path),
    );
    assert_eq!(start.get("B6"), Some("Release 1.2"));
    assert_eq!(start.column('A', 8..12), texts(&GOVERNANCE_ROLES));
    for (address, expected) in [
        ("B8", "'Assumptions'!$B$21"),
        ("B9", "'Assumptions'!$B$22"),
        ("B10", "'Assumptions'!$B$23"),
        ("B11", "'Assumptions'!$B$24"),
        ("B14", "'Assumptions'!$B$12"),
    ] {
        assert_eq!(start_formulas.get(address), Some(expected), "{address}");
    }
    assert_eq!(
        font_and_number_format(&parts, start_path, "B11").1,
        DATE_FORMAT
    );
    let assumptions_path = &map["Assumptions"];
    let assumptions = values_by_address(&parts, assumptions_path);
    assert_eq!(assumptions.get("A20"), Some("Forecast governance"));
    assert_eq!(assumptions.column('A', 21..25), texts(&GOVERNANCE_ROLES));
    let review_date = excel_serial(date(2026, 9, 13));
    let owners = [
        "Finance Manager",
        "CFO / Business Owner",
        "Finance Manager",
        review_date.as_str(),
    ];
    assert_eq!(assumptions.column('B', 21..25), texts(&owners));
    for address in ["B21", "B22", "B23", "B24"] {
        assert_eq!(
            style_for_cell(&parts, assumptions_path, address),
            colours(INPUT.0, Some(INPUT.1)),
            "{address}"
        );
    }
    assert_eq!(
        font_and_number_format(&parts, assumptions_path, "B24").1,
        DATE_FORMAT
    );
}

#[test]
fn governance_weekly_review_status() {
    let parts = parts(WORKBOOK);
    let review_path = &sheet_map(&parts)["Weekly Review"];
    let (values, formulas) = (
        values_by_address(&parts, review_path),
        formulas_by_address(&parts, review_path),
    );
    assert_eq!(values.get("O5"), Some("Review status"));
    for row in 6..19 {
        let expected = format!(
            r#"IF(COUNTA(D{row},H{row},L{row},N{row})=0,"NOT STARTED",IF(AND(A{row}<>"",B{row}<>"",C{row}<>"",D{row}<>"",G{row}<>"",H{row}<>"",K{row}<>"",L{row}<>"",N{row}<>""),"COMPLETE","INCOMPLETE"))"#
        );
        assert_eq!(
            formulas.get(&format!("O{row}")),
            Some(expected.as_str()),
            "O{row}"
        );
        assert_eq!(
            values.get(&format!("O{row}")),
            Some("NOT STARTED"),
            "O{row}"
        );
    }
    let styles = conditional_rule_styles(&parts, review_path);
    assert_eq!(styles[r#"O6="COMPLETE""#], rule(PASSING.0, PASSING.1));
    assert_eq!(styles[r#"O6="INCOMPLETE""#], rule(FAILING.0, FAILING.1));
}

#[test]
fn governance_dashboard_accuracy_formulas() {
    let parts = parts(WORKBOOK);
    let map = sheet_map(&parts);
    let dashboard_path = &map["Dashboard"];
    let (values, formulas) = (
        values_by_address(&parts, dashboard_path),
        formulas_by_address(&parts, dashboard_path),
    );
    let headers = [
        "Weeks actualised",
        "Receipt variance %",
        "Payment variance %",
        "Closing-cash bias",
        "Review completeness",
    ];
    assert_eq!(values.row("ABCDE", 32), texts(&headers));
    for (address, expected) in [
        (
            "A33",
            r#"COUNTIFS('Weekly Review'!$D$6:$D$18,"<>",'Weekly Review'!$H$6:$H$18,"<>",'Weekly Review'!$L$6:$L$18,"<>")"#,
        ),
        (
            "B33",
            r#"IF(A33=0,"",IF(SUMIF('Weekly Review'!$D$6:$D$18,"<>",'Weekly Review'!$C$6:$C$18)=0,"",SUM('Weekly Review'!$E$6:$E$18)/SUMIF('Weekly Review'!$D$6:$D$18,"<>",'Weekly Review'!$C$6:$C$18)))"#,
        ),
        (
            "C33",
            r#"IF(A33=0,"",IF(SUMIF('Weekly Review'!$H$6:$H$18,"<>",'Weekly Review'!$G$6:$G$18)=0,"",SUM('Weekly Review'!$I$6:$I$18)/SUMIF('Weekly Review'!$H$6:$H$18,"<>",'Weekly Review'!$G$6:$G$18)))"#,
        ),
        (
            "D33",
            r#"IF(A33=0,"",SUMIFS('Weekly Review'!$M$6:$M$18,'Weekly Review'!$D$6:$D$18,"<>",'Weekly Review'!$H$6:$H$18,"<>",'Weekly Review'!$L$6:$L$18,"<>")/A33)"#,
        ),
        (
            "E33",
            r#"IF(COUNTIF('Weekly Review'!$O$6:$O$18,"INCOMPLETE")>0,"INCOMPLETE",IF(COUNTIF('Weekly Review'!$O$6:$O$18,"COMPLETE")>0,"COMPLETE","NOT STARTED"))"#,
        ),
    ] {
        assert_eq!(formulas.get(address), Some(expected), "{address}");
    }
}

#[test]
fn governance_dashboard_accuracy_values_and_styles() {
    let parts = parts(WORKBOOK);
    let map = sheet_map(&parts);
    let dashboard_path = &map["Dashboard"];
    let values = values_by_address(&parts, dashboard_path);
    assert_eq!(values.get("A33"), Some("0"));
    for address in ["B33", "C33", "D33"] {
        assert_eq!(
            cached_value(&parts, dashboard_path, address),
            None,
            "{address}"
        );
    }
    assert_eq!(values.get("E33"), Some("NOT STARTED"));
    assert_eq!(
        font_and_number_format(&parts, dashboard_path, "B33").1,
        PERCENT_FORMAT
    );
    assert_eq!(
        font_and_number_format(&parts, dashboard_path, "D33").1,
        AUD_FORMAT
    );
    let styles = conditional_rule_styles(&parts, dashboard_path);
    assert_eq!(styles[r#"E33="COMPLETE""#], rule(PASSING.0, PASSING.1));
    assert_eq!(styles[r#"E33="INCOMPLETE""#], rule(FAILING.0, FAILING.1));
    let checks = formulas_by_address(&parts, &map["Checks & Sources"]);
    assert_eq!(checks.get("B16"), Some(FORMULA_ERROR_COUNT_FORMULA));
}

#[test]
fn template_documentation_workflow() {
    // The repository documentation change is the production change that makes this pass.
    let template_readme = read("templates/README.md");
    let workflow = markdown_section(
        &template_readme,
        "Weekly workflow",
        Some("Scenario behaviour"),
    );
    assert!(template_readme.contains("13-week-cash-flow-forecast.xlsx"));
    assert!(template_readme.contains("Excel 2024 and later"));
    assert!(template_readme.to_lowercase().contains("illustrative data"));
    assert_eq!(
        common::captures(r"(?m)^\d+\. \*\*[^*]+\*\*", workflow).len(),
        5
    );
    let workflow = workflow.to_lowercase();
    for action in [
        "replace the blue input cells",
        "enter the business name, forecast start date, as-at date, opening cash and minimum cash buffer",
        "select `Base`, `Upside` or `Downside`",
        "refresh the forecast inputs and enter available actual receipts, actual payments and actual closing cash",
        "before refreshing the live forecast",
        "paste the original period dates, forecast receipts, forecast payments and forecast closing cash into the blue snapshot cells",
        "as values",
        "must not link to the live forecast",
        "use `Dashboard`",
        "action deadline",
        "3-scenario comparison",
        "use `Weekly Review`",
        "compare receipt, payment and closing-cash variances",
        "record owner commentary",
        "review `Checks & Sources`",
        "save the reviewed file as the period's archive",
        "copy it for the next cycle",
    ] {
        assert!(workflow.contains(&action.to_lowercase()), "{action}");
    }
}

#[test]
fn template_documentation_scenarios_and_limitations() {
    let template_readme = read("templates/README.md");
    let scenario = markdown_section(
        &template_readme,
        "Scenario behaviour",
        Some("Checks and limitations"),
    );
    let limitations =
        markdown_section(&template_readme, "Checks and limitations", None).to_lowercase();
    let workbook_use = read("docs/workbook-use.md");
    let getting_started = markdown_section(&workbook_use, "Getting started", Some("Modern Excel"));
    assert!(scenario.contains("Scenario factors apply only to weeks labelled `Forecast`."));
    assert!(scenario.contains("Weeks labelled `Actual` remain unchanged."));
    for (name, receipts, payments) in [
        ("Base", "100%", "100%"),
        ("Upside", "108%", "98%"),
        ("Downside", "85%", "105%"),
    ] {
        let line =
            format!("`{name}` uses {receipts} receipts and {payments} selected variable payments.");
        assert!(scenario.contains(&line), "{line}");
    }
    for text in [
        "The scenario receipt adjustment is calculated from customer receipts, overdue receipts and cash / EFTPOS sales.",
        "The scenario variable-payment adjustment is calculated from suppliers and inventory, marketing, freight / vehicles / travel and other operating payments.",
        "GST refunds or credits, other operating receipts, asset sale proceeds, equity or owner funding, or loan proceeds",
        "wages, PAYG withholding, superannuation, payroll tax",
        "GST / BAS payments, income-tax instalments",
        "FBT, interest, loan principal, capital expenditure or dividends",
    ] {
        assert!(scenario.contains(text), "{text}");
    }
    for phrase in [
        "not tax advice",
        "statutory",
        "exactly 13 weeks",
        "formula errors",
    ] {
        assert!(limitations.contains(phrase), "{phrase}");
    }
    assert!(
        getting_started
            .contains("[Ozzit 13-week cash-flow forecast template](../templates/README.md)")
    );
    assert!(
        read("docs/cash-flow-template.md").contains("templates/13-week-cash-flow-forecast.xlsx")
    );
    assert!(read("README.md").contains("docs/cash-flow-template.md"));
}

// The helpers above, run on small synthetic input that takes their other paths:
// against the committed workbook each helper only sees the shapes it was written for.

fn worksheet(body: &str) -> Vec<u8> {
    format!("<worksheet xmlns='{MAIN}' xmlns:r='{REL}'>{body}</worksheet>").into_bytes()
}

fn synthetic(members: [(&str, Vec<u8>); 2]) -> Parts {
    members
        .into_iter()
        .map(|(name, payload)| (name.to_owned(), payload))
        .collect()
}

#[test]
fn values_by_address_reads_shared_inline_and_number_cells() {
    let parts = synthetic([
        (
            "xl/sharedStrings.xml",
            format!("<sst xmlns='{MAIN}'><si><t>Opening</t></si></sst>").into_bytes(),
        ),
        (
            "s.xml",
            worksheet(
                "<sheetData><row><c r='A1' t='s'><v>0</v></c><c r='B1'><v>5</v></c>\
                 <c r='C1' t='inlineStr'><is><t>Note</t></is></c></row></sheetData>",
            ),
        ),
    ]);
    let expected = [("A1", "Opening"), ("B1", "5"), ("C1", "Note")]
        .map(|(address, value)| (address.to_owned(), Some(value.to_owned())));
    assert_eq!(
        values_by_address(&parts, "s.xml"),
        Values(HashMap::from(expected))
    );
}

#[test]
fn relationship_records_resolve_a_relative_target() {
    let rels = format!(
        "<Relationships xmlns='{PACKAGE_REL}'>\
         <Relationship Id='rId1' Type='t' Target='../drawings/drawing1.xml'/></Relationships>"
    );
    let parts = synthetic([
        ("xl/workbook.xml", b"<x/>".to_vec()),
        ("xl/worksheets/_rels/sheet1.xml.rels", rels.into_bytes()),
    ]);
    assert_eq!(
        relationship_records(&parts),
        [Relationship {
            source: "xl/worksheets/sheet1.xml".into(),
            kind: "t".into(),
            target: "../drawings/drawing1.xml".into(),
            target_mode: "Internal".into(),
            resolved_target: "xl/drawings/drawing1.xml".into(),
        }]
    );
}

#[test]
fn comment_text_matches_the_address_and_is_empty_without_one() {
    let comments = format!(
        "<comments xmlns='{MAIN}'><commentList>\
         <comment ref='B2'><text><t>Read Me</t></text></comment></commentList></comments>"
    );
    let parts = synthetic([
        ("xl/styles.xml", b"<x/>".to_vec()),
        ("xl/comments1.xml", comments.into_bytes()),
    ]);
    assert_eq!(comment_text(&parts, "b2"), "read me");
    assert_eq!(comment_text(&parts, "C3"), "");
}

#[test]
fn column_width_is_none_outside_every_declared_column() {
    let parts: Parts = [(
        "s.xml".to_owned(),
        worksheet("<cols><col min='1' max='2' width='12.5'/></cols>"),
    )]
    .into();
    assert_eq!(column_width(&parts, "s.xml", "B"), Some(12.5));
    assert_eq!(column_width(&parts, "s.xml", "D"), None);
}

#[test]
fn dashboard_charts_skip_an_anchor_without_a_chart() {
    let drawing = format!(
        "<xdr:wsDr xmlns:xdr='{DRAWING}'><xdr:twoCellAnchor><xdr:pic/></xdr:twoCellAnchor></xdr:wsDr>"
    );
    let rels = format!(
        "<Relationships xmlns='{PACKAGE_REL}'>\
         <Relationship Id='rId1' Type='d' Target='../drawings/drawing1.xml'/></Relationships>"
    );
    let parts: Parts = [
        (
            "xl/worksheets/sheet2.xml".to_owned(),
            worksheet("<drawing r:id='rId1'/>"),
        ),
        (
            "xl/worksheets/_rels/sheet2.xml.rels".to_owned(),
            rels.into_bytes(),
        ),
        ("xl/drawings/drawing1.xml".to_owned(), drawing.into_bytes()),
    ]
    .into();
    let charts = dashboard_charts(&parts, "xl/worksheets/sheet2.xml");
    assert!(charts.is_empty(), "{charts:?}");
}
