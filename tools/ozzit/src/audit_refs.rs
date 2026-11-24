// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! The reference checks of `ozzit audit`: defined names that nothing uses, check
//! rows that nothing reads, rows a SUM total leaves out, and circular references.
//! Every check reads the formulas the file records; a reference built at run
//! time, through `INDIRECT` or `OFFSET`, cannot be followed, so a workbook whose
//! cells use either is not checked for buried checks or unlinked rows.
use crate::archive::Parts;
use crate::audit::{MAX_COLUMN, MAX_ROW, Sheet, a1, attr, children, parse_a1, quoted};
use crate::common::{column_number, re, resolve_target};
use crate::workbook::xml_part;
use roxmltree::{Document, Node};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// An optional sheet or sheet-range prefix, then a cell, a cell range, whole
/// columns or whole rows, then an optional spill `#`. A reference inside a name
/// or function name does not match; [`scrub`] has already removed references to
/// other workbooks and structured references.
const REFERENCE: &str = r"(?i)(?<![\p{L}\p{N}_.$\[\]'!])(?:(?:'((?:[^']|'')+)'|([\p{L}_\\][\p{L}\p{N}_.]*(?::[\p{L}_\\][\p{L}\p{N}_.]*)?))!)?(\$?[A-Z]{1,3}\$?[0-9]{1,7}(?::\$?[A-Z]{1,3}\$?[0-9]{1,7})?|\$?[A-Z]{1,3}:\$?[A-Z]{1,3}|\$?[0-9]{1,7}:\$?[0-9]{1,7})(#?)(?![\p{L}\p{N}_.(\[])";
/// A function whose reference or name argument is read for its position, not
/// its value, such as `ROW(A1)` or `CELL("filename", A1)`.
const POSITION_ONLY: &str = r#"(?i)(?<![\p{L}\p{N}_.])(?:(?:ROWS?|COLUMNS?|ISREF|_xlfn\.ISFORMULA|_xlfn\.FORMULATEXT)\(|CELL\(\s*"(?:filename|address|row|col|width)"\s*,)\s*(?:'(?:[^']|'')+'!|[\p{L}_\\][\p{L}\p{N}_.]*!)?(?:\$?[A-Z]{0,3}\$?[0-9]{0,7}(?::\$?[A-Z]{0,3}\$?[0-9]{0,7})?#?|[\p{L}_\\][\p{L}\p{N}_.]*)\s*\)"#;
const DYNAMIC: &str = r"(?i)(?<![\p{L}\p{N}_.])(?:INDIRECT|OFFSET)\(";
const CALL: &str = r"[\p{L}_][\p{L}\p{N}_.]*\(";
const NAME_TOKEN: &str = r"[\p{L}\p{N}_.\\?]+";
const CHECK_LABEL: &str = r"(?i)\bchecks?\b";
/// A reference to another workbook: `[1]Sheet!A1` or `'C:\[Book.xlsx]Sheet'!A1:B2`.
const EXTERNAL: &str = r"(?i)(?:'(?:[^']|'')*\[(?:[^']|'')*'|\[[^\]]*\][\p{L}\p{N}_.]*(?::[\p{L}\p{N}_.]+)?)!\$?[A-Z]{0,3}\$?[0-9]{0,7}(?::\$?[A-Z]{0,3}\$?[0-9]{0,7})?";

/// A name's sheet scope (`None` for the workbook) and its lower-case text.
type Scoped = (Option<usize>, String);
/// A formula cell as (sheet, row, column).
type Place = (usize, usize, usize);
/// Unread gap cells by (sheet, vertical, gap line): each with its position
/// along the line, its total's cell and the summed range.
type Gaps = BTreeMap<(usize, bool, usize), Vec<(usize, (usize, usize), Area)>>;

/// A block of cells on one sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Area {
    sheet: usize,
    top: usize,
    left: usize,
    bottom: usize,
    right: usize,
}

impl Area {
    fn holds(&self, sheet: usize, (row, column): (usize, usize)) -> bool {
        self.sheet == sheet
            && (self.top..=self.bottom).contains(&row)
            && (self.left..=self.right).contains(&column)
    }
    fn meets(&self, other: &Self) -> bool {
        self.sheet == other.sheet
            && self.top <= other.bottom
            && other.top <= self.bottom
            && self.left <= other.right
            && other.left <= self.right
    }
}

/// One reference: where it points and whether it ends in a spill `#`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ref {
    area: Area,
    spill: bool,
}

struct Formula {
    /// The anchor's text: a shared formula's dependants read it from `origin`.
    text: String,
    origin: (usize, usize),
    /// The bottom-right cell of the array the formula fills, if any.
    spill: Option<(usize, usize)>,
}

#[derive(Default)]
struct Grid {
    formulas: BTreeMap<(usize, usize), Formula>,
    numbers: BTreeSet<(usize, usize)>,
    labels: BTreeMap<(usize, usize), String>,
    spills: Vec<Area>,
}

/// Something that reads an area: a formula cell, or a formatting or validation
/// rule over `reader`, or a name or chart (no reader cells).
struct Read {
    area: Area,
    reader: Vec<Area>,
}

struct Book {
    names: Vec<String>,
    grids: Vec<Grid>,
    reads: Vec<Read>,
    /// (scope, lower-case name or table) → the areas it names, aliases
    /// followed; a workbook name or table has no scope.
    named: HashMap<Scoped, Vec<Area>>,
    /// The same, for names that are references alone.
    plain_names: HashMap<Scoped, Vec<Area>>,
    tables: Vec<Area>,
    dynamic: Option<String>,
}

fn sheet_index(names: &[String], name: &str) -> Option<usize> {
    names.iter().position(|n| n.eq_ignore_ascii_case(name))
}

/// One end of a reference: (row, absolute) and (column, absolute), either
/// missing for a whole column or row.
type End = (Option<(usize, bool)>, Option<(usize, bool)>);

fn end(text: &str) -> End {
    let text = text.to_ascii_uppercase();
    let caps = re(r"^(\$?)([A-Z]*)(\$?)([0-9]*)$");
    let Ok(Some(m)) = caps.captures(&text) else {
        return (None, None);
    };
    let group = |i: usize| m.get(i).map_or("", |g| g.as_str());
    let column = (!group(2).is_empty()).then(|| (column_number(group(2)), !group(1).is_empty()));
    // `$3` alone matches its dollar in the first group.
    let absolute = !group(3).is_empty() || (column.is_none() && !group(1).is_empty());
    let row = group(4).parse().ok().map(|r| (r, absolute));
    (row, column)
}

fn shift((value, absolute): (usize, bool), by: i64, max: usize) -> Option<usize> {
    // A coordinate off the sheet as written, such as the name `A0`, is no reference.
    if !(1..=max).contains(&value) {
        return None;
    }
    let value = if absolute {
        value as i64
    } else {
        value as i64 + by
    };
    usize::try_from(value)
        .ok()
        .filter(|v| (1..=max).contains(v))
}

/// The formula split into code and string literals. Unlike `split_literals`,
/// a double quote inside an apostrophe-quoted sheet name stays in the code.
fn split(formula: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut code = String::new();
    let mut chars = formula.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\'' && c != '"' {
            code.push(c);
            continue;
        }
        let mut quoted = String::from(c);
        while let Some(d) = chars.next() {
            quoted.push(d);
            if d == c {
                if chars.peek() != Some(&c) {
                    break;
                }
                quoted.push(c);
                chars.next();
            }
        }
        if c == '"' {
            out.push((false, std::mem::take(&mut code)));
            out.push((true, quoted));
        } else {
            code.push_str(&quoted);
        }
    }
    out.push((false, code));
    out
}

/// Code with references to other workbooks and the bracketed parts of
/// structured references blanked, so neither is read as a local reference.
fn scrub(code: &str) -> String {
    let mut out = re(EXTERNAL).replace_all(code, " ").into_owned();
    while out.contains('[') {
        let next = re(r"\[[^\[\]]*\]").replace_all(&out, " ").into_owned();
        if next == out {
            break;
        }
        out = next;
    }
    out
}

/// The sheets a prefix names: one sheet, or each sheet of a 3-D range in
/// workbook order. Empty for an unknown sheet.
fn prefix_sheets(prefix: &str, sheets: &[String]) -> Vec<usize> {
    match prefix.split_once(':') {
        Some((a, b)) => match (sheet_index(sheets, a), sheet_index(sheets, b)) {
            (Some(a), Some(b)) => (a.min(b)..=a.max(b)).collect(),
            _ => Vec::new(),
        },
        None => sheet_index(sheets, prefix).into_iter().collect(),
    }
}

fn references(
    formula: &str,
    origin: (usize, usize),
    at: (usize, usize),
    own: Option<usize>,
    sheets: &[String],
) -> Vec<Ref> {
    let by = (at.0 as i64 - origin.0 as i64, at.1 as i64 - origin.1 as i64);
    let pattern = re(REFERENCE);
    let mut out = Vec::new();
    for (literal, piece) in split(formula) {
        if literal {
            continue;
        }
        let piece = scrub(&piece);
        for m in pattern.captures_iter(piece.as_str()).flatten() {
            let targets = match (m.get(1), m.get(2)) {
                (Some(q), _) => prefix_sheets(&q.as_str().replace("''", "'"), sheets),
                (None, Some(p)) => prefix_sheets(p.as_str(), sheets),
                (None, None) => own.into_iter().collect(),
            };
            let text = &m[3];
            let (first, last) = text.split_once(':').unwrap_or((text, text));
            let (a, b) = (end(first), end(last));
            let rows = |e: End| {
                e.0.map_or(Some(None), |r| shift(r, by.0, MAX_ROW).map(Some))
            };
            let columns = |e: End| {
                e.1.map_or(Some(None), |c| shift(c, by.1, MAX_COLUMN).map(Some))
            };
            let (Some(r1), Some(r2), Some(c1), Some(c2)) =
                (rows(a), rows(b), columns(a), columns(b))
            else {
                continue;
            };
            let (top, bottom) = (r1.unwrap_or(1), r2.unwrap_or(MAX_ROW));
            let (left, right) = (c1.unwrap_or(1), c2.unwrap_or(MAX_COLUMN));
            for sheet in targets {
                out.push(Ref {
                    area: line(
                        sheet,
                        top.min(bottom),
                        left.min(right),
                        top.max(bottom),
                        left.max(right),
                    ),
                    spill: !m[4].is_empty(),
                });
            }
        }
    }
    out
}

fn name_tokens(formula: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (literal, piece) in split(formula) {
        if literal {
            continue;
        }
        let piece = scrub(&re(r"'(?:[^']|'')*'").replace_all(&piece, " "));
        for m in re(NAME_TOKEN).find_iter(&piece).flatten() {
            // A sheet qualifier is not a name.
            if piece[m.end()..].starts_with('!') {
                continue;
            }
            out.push(m.as_str().to_lowercase());
        }
    }
    out
}

fn text_of(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|n| n.tag_name().name() == "t")
        .filter_map(|n| n.text())
        .collect()
}

fn shared_strings(parts: &Parts) -> Vec<String> {
    let Some(doc) = parts
        .get("xl/sharedStrings.xml")
        .and_then(|b| xml_part("xl/sharedStrings.xml", b).ok())
    else {
        return Vec::new();
    };
    children(doc.root_element(), "si").map(text_of).collect()
}

fn parse_area(sheet: usize, text: &str) -> Option<Area> {
    let (first, last) = text.split_once(':').unwrap_or((text, text));
    let (top, left) = parse_a1(&first.replace('$', ""))?;
    let (bottom, right) = parse_a1(&last.replace('$', ""))?;
    Some(Area {
        sheet,
        top,
        left,
        bottom,
        right,
    })
}

fn grid(doc: &Document<'_>, sheet: usize, strings: &[String]) -> Grid {
    let mut grid = Grid::default();
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
            match children(cell, "f").next() {
                Some(f) => add_formula(&mut grid, &mut shared, f, sheet, at),
                None => add_value(&mut grid, cell, at, strings),
            }
        }
    }
    grid
}

/// A formula cell; a shared formula's dependants take their anchor's text.
fn add_formula(
    grid: &mut Grid,
    shared: &mut HashMap<String, (String, (usize, usize))>,
    f: Node<'_, '_>,
    sheet: usize,
    at: (usize, usize),
) {
    let text = f.text().unwrap_or_default().to_owned();
    let spill = (attr(f, "t") == Some("array"))
        .then(|| attr(f, "ref").and_then(|r| parse_area(sheet, r)))
        .flatten();
    grid.spills.extend(spill);
    let si = (attr(f, "t") == Some("shared"))
        .then(|| attr(f, "si"))
        .flatten();
    let found = match si {
        Some(si) if text.is_empty() => shared.get(si).cloned(),
        Some(si) => Some(shared.entry(si.to_owned()).or_insert((text, at)).clone()),
        None => Some((text, at)),
    };
    if let Some((text, origin)) = found {
        let spill = spill.map(|a| (a.bottom, a.right));
        grid.formulas.insert(
            at,
            Formula {
                text,
                origin,
                spill,
            },
        );
    }
}

fn add_value(grid: &mut Grid, cell: Node<'_, '_>, at: (usize, usize), strings: &[String]) {
    match attr(cell, "t") {
        None | Some("n") if children(cell, "v").next().is_some() => {
            grid.numbers.insert(at);
        }
        Some("s") => {
            let label = children(cell, "v")
                .next()
                .and_then(|v| v.text()?.parse::<usize>().ok())
                .and_then(|i| strings.get(i));
            if let Some(label) = label {
                grid.labels.insert(at, label.clone());
            }
        }
        Some("inlineStr") => {
            grid.labels
                .insert(at, children(cell, "is").map(text_of).collect());
        }
        _ => {}
    }
}

/// The formatting and validation rules of a sheet as reads over their ranges.
fn rule_reads(doc: &Document<'_>, sheet: usize, sheets: &[String], out: &mut Vec<Read>) {
    for rule in doc.descendants().filter(|n| {
        matches!(
            n.tag_name().name(),
            "conditionalFormatting" | "dataValidation"
        )
    }) {
        let sqref = attr(rule, "sqref")
            .map(str::to_owned)
            .or_else(|| {
                rule.descendants()
                    .find(|n| n.tag_name().name() == "sqref")
                    .and_then(|n| n.text().map(str::to_owned))
            })
            .unwrap_or_default();
        let reader: Vec<Area> = sqref
            .split_whitespace()
            .filter_map(|r| parse_area(sheet, r))
            .collect();
        let origin = reader.first().map_or((1, 1), |a| (a.top, a.left));
        for f in rule.descendants().filter(|n| {
            matches!(
                n.tag_name().name(),
                "formula" | "formula1" | "formula2" | "f"
            )
        }) {
            for r in references(
                f.text().unwrap_or_default(),
                origin,
                origin,
                Some(sheet),
                sheets,
            ) {
                out.push(Read {
                    area: r.area,
                    reader: reader.clone(),
                });
            }
        }
    }
}

fn tables(parts: &Parts, sheet: usize, part: &str, out: &mut Book) {
    let (dir, file) = part.rsplit_once('/').unwrap_or(("", part));
    let rels = format!("{dir}/_rels/{file}.rels");
    let Some(doc) = parts.get(&rels).and_then(|b| xml_part(&rels, b).ok()) else {
        return;
    };
    for rel in doc
        .descendants()
        .filter(|n| attr(*n, "Type").is_some_and(|t| t.ends_with("/table")))
    {
        let target = resolve_target(part, attr(rel, "Target").unwrap_or_default());
        let target = target.trim_start_matches('/');
        let Some(table) = parts.get(target).and_then(|b| xml_part(target, b).ok()) else {
            continue;
        };
        let root = table.root_element();
        let area = attr(root, "ref").and_then(|r| parse_area(sheet, r));
        out.tables.extend(area);
        for key in ["name", "displayName"].iter().filter_map(|k| attr(root, k)) {
            out.named
                .entry((None, key.to_lowercase()))
                .or_default()
                .extend(area);
        }
    }
}

fn read_book(parts: &Parts, book: &Document<'_>, sheets: &[Sheet]) -> Book {
    let names: Vec<String> = sheets.iter().map(|s| s.name.clone()).collect();
    let strings = shared_strings(parts);
    let mut out = Book {
        names: names.clone(),
        grids: Vec::new(),
        reads: Vec::new(),
        named: HashMap::new(),
        plain_names: HashMap::new(),
        tables: Vec::new(),
        dynamic: None,
    };
    for (i, sheet) in sheets.iter().enumerate() {
        let doc = sheet
            .part
            .as_ref()
            .and_then(|p| parts.get(p).and_then(|b| xml_part(p, b).ok()));
        let Some(doc) = doc else {
            out.grids.push(Grid::default());
            continue;
        };
        let grid = grid(&doc, i, &strings);
        if out.dynamic.is_none() {
            out.dynamic = grid
                .formulas
                .iter()
                .find(|(_, f)| has_dynamic(&f.text))
                .map(|(&at, _)| format!("{}!{}", quoted(&sheet.name), a1(at)));
        }
        rule_reads(&doc, i, &names, &mut out.reads);
        if let Some(part) = &sheet.part {
            tables(parts, i, part, &mut out);
        }
        out.grids.push(grid);
    }
    read_names(&mut out, book);
    chart_reads(&mut out, parts);
    let reads = formula_reads(&out);
    out.reads.extend(reads);
    out
}

fn read_names(out: &mut Book, book: &Document<'_>) {
    let mut raw: Vec<(Scoped, String)> = Vec::new();
    for name in book
        .descendants()
        .filter(|n| n.tag_name().name() == "definedName")
    {
        let scope = attr(name, "localSheetId").and_then(|i| i.parse::<usize>().ok());
        let key = attr(name, "name").unwrap_or_default().to_lowercase();
        raw.push(((scope, key), name.text().unwrap_or_default().to_owned()));
    }
    let direct: HashMap<Scoped, (Vec<Area>, bool, Vec<String>)> = raw
        .iter()
        .map(|(key, text)| {
            let areas: Vec<Area> = references(text, (1, 1), (1, 1), None, &out.names)
                .iter()
                .map(|r| r.area)
                .collect();
            let rest = re(REFERENCE).replace_all(text, "");
            let plain = !areas.is_empty() && rest.trim_matches([',', ' ']).is_empty();
            (key.clone(), (areas, plain, name_tokens(text)))
        })
        .collect();
    for (key, (areas, plain, _)) in &direct {
        if *plain {
            out.plain_names.insert(key.clone(), areas.clone());
        }
        let mut all: Vec<Area> = Vec::new();
        let mut seen = BTreeSet::new();
        let mut queue = vec![key.clone()];
        // Follow aliases, a name whose definition names another, once each.
        while let Some(k) = queue.pop() {
            if !seen.insert(k.clone()) {
                continue;
            }
            let Some((areas, _, tokens)) = direct.get(&k) else {
                continue;
            };
            all.extend(areas);
            for t in tokens {
                for scope in [k.0, None] {
                    if direct.contains_key(&(scope, t.clone())) {
                        queue.push((scope, t.clone()));
                        break;
                    }
                }
            }
        }
        out.named.entry(key.clone()).or_default().extend(all);
    }
}

fn chart_reads(out: &mut Book, parts: &Parts) {
    for (part, bytes) in parts {
        if !(part.contains("/charts/") && part.ends_with(".xml")) {
            continue;
        }
        let Ok(doc) = xml_part(part, bytes) else {
            continue;
        };
        for f in doc.descendants().filter(|n| n.tag_name().name() == "f") {
            let text = f.text().unwrap_or_default();
            for r in references(text, (1, 1), (1, 1), None, &out.names) {
                out.reads.push(Read {
                    area: r.area,
                    reader: Vec::new(),
                });
            }
        }
    }
}

fn formula_reads(book: &Book) -> Vec<Read> {
    let mut reads = Vec::new();
    for (sheet, grid) in book.grids.iter().enumerate() {
        for (&at, f) in &grid.formulas {
            let reader = vec![book.formula_area(sheet, at)];
            let mut areas: Vec<Area> = references(&f.text, f.origin, at, Some(sheet), &book.names)
                .into_iter()
                .map(|r| book.resolve(r))
                .collect();
            for token in name_tokens(&f.text) {
                areas.extend(book.name_areas(&book.named, sheet, &token));
            }
            // A structured reference without a table name reads its own table.
            let bare = split(&f.text).into_iter().any(|(literal, piece)| {
                !literal && re(EXTERNAL).replace_all(&piece, "").contains('[')
            });
            if bare {
                areas.extend(book.tables.iter().filter(|t| t.holds(sheet, at)));
            }
            reads.extend(areas.into_iter().map(|area| Read {
                area,
                reader: reader.clone(),
            }));
        }
    }
    reads
}

fn has_dynamic(formula: &str) -> bool {
    split(formula)
        .into_iter()
        .any(|(literal, piece)| !literal && re(DYNAMIC).is_match(&piece).unwrap_or(false))
}

impl Book {
    fn formula_area(&self, sheet: usize, at: (usize, usize)) -> Area {
        let spill = self.grids[sheet].formulas.get(&at).and_then(|f| f.spill);
        let (bottom, right) = spill.unwrap_or(at);
        Area {
            sheet,
            top: at.0,
            left: at.1,
            bottom: bottom.max(at.0),
            right: right.max(at.1),
        }
    }
    /// A spill reference reads the whole array its anchor fills.
    fn resolve(&self, r: Ref) -> Area {
        let anchor = (r.area.top, r.area.left);
        if r.spill && self.grids[r.area.sheet].formulas.contains_key(&anchor) {
            self.formula_area(r.area.sheet, anchor)
        } else {
            r.area
        }
    }
    /// Whether anything outside `group` reads part of `area`.
    fn is_read(&self, area: &Area, group: &Area) -> bool {
        self.reads
            .iter()
            .any(|read| read.area.meets(area) && !read.reader.iter().any(|r| r.meets(group)))
    }
    /// The areas a name or table names, a sheet's own name first.
    fn name_areas(&self, map: &HashMap<Scoped, Vec<Area>>, sheet: usize, token: &str) -> Vec<Area> {
        map.get(&(Some(sheet), token.to_owned()))
            .or_else(|| map.get(&(None, token.to_owned())))
            .cloned()
            .unwrap_or_default()
    }
    fn label(&self, sheet: usize, (row, column): (usize, usize)) -> Option<&String> {
        self.grids[sheet]
            .labels
            .range((row, 0)..(row, column))
            .next_back()
            .map(|(_, l)| l)
    }
    fn place(&self, sheet: usize, area: &Area) -> String {
        let (start, finish) = ((area.top, area.left), (area.bottom, area.right));
        let cells = if start == finish {
            a1(start)
        } else {
            format!("{}:{}", a1(start), a1(finish))
        };
        format!("{}!{cells}", quoted(&self.names[sheet]))
    }
}

/// Names of a range or constant that no formula, rule, chart or other part
/// mentions. Built-in and hidden names are left out, as hidden names are
/// reported already, and so is a name holding a formula that calls a function,
/// `LAMBDA` included, which a user may call. A name that is the whole value
/// of an attribute or element in a part other than the workbook, a sheet or the
/// shared strings, such as a slicer's cache name, counts as used.
fn unused_names(parts: &Parts, book: &Document<'_>, sheets: &[Sheet]) -> Vec<String> {
    let mut used: BTreeSet<String> = BTreeSet::new();
    for (part, bytes) in parts {
        let Ok(doc) = xml_part(part, bytes) else {
            continue;
        };
        let other = part != crate::archive::BOOK
            && !part.starts_with("xl/worksheets/")
            && part != "xl/sharedStrings.xml";
        for node in doc.descendants().filter(|n| n.is_element()) {
            let name = node.tag_name().name();
            if matches!(
                name,
                "f" | "formula" | "formula1" | "formula2" | "definedName"
            ) {
                used.extend(name_tokens(node.text().unwrap_or_default()));
            } else if other {
                used.extend(node.attributes().map(|a| a.value().to_lowercase()));
                used.extend(node.text().map(str::to_lowercase));
            }
        }
    }
    let mut out = Vec::new();
    for name in book
        .descendants()
        .filter(|n| n.tag_name().name() == "definedName")
    {
        let key = attr(name, "name").unwrap_or_default();
        let text = name.text().unwrap_or_default().trim_start_matches('=');
        let skip = key.to_lowercase().starts_with("_xl")
            || matches!(attr(name, "hidden"), Some("1" | "true"))
            || split(text)
                .into_iter()
                .any(|(literal, piece)| !literal && re(CALL).is_match(&piece).unwrap_or(false));
        if skip || used.contains(&key.to_lowercase()) {
            continue;
        }
        let scope = attr(name, "localSheetId")
            .and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| sheets.get(i))
            .map(|s| format!(" (sheet {})", s.name))
            .unwrap_or_default();
        out.push(format!("unused name: {key}{scope}, which no formula uses"));
    }
    out
}

/// Rows labelled as a check whose formulas nothing outside the row reads, and
/// which read no other check: a check that no roll-up or summary shows.
fn buried_checks(book: &Book) -> Vec<String> {
    let mut rows: BTreeMap<(usize, usize), Vec<(usize, usize)>> = BTreeMap::new();
    for (sheet, grid) in book.grids.iter().enumerate() {
        for &at in grid.formulas.keys() {
            if book
                .label(sheet, at)
                .is_some_and(|l| re(CHECK_LABEL).is_match(l).unwrap_or(false))
            {
                rows.entry((sheet, at.0)).or_default().push(at);
            }
        }
    }
    let checks: Vec<Area> = rows
        .iter()
        .map(|(&(sheet, row), cells)| line(sheet, row, cells[0].1, row, cells[cells.len() - 1].1))
        .collect();
    let mut out = Vec::new();
    for ((&(sheet, _), cells), group) in rows.iter().zip(&checks) {
        let read = cells
            .iter()
            .any(|&at| book.is_read(&book.formula_area(sheet, at), group));
        let rolls_up = book.reads.iter().any(|r| {
            r.reader.iter().any(|x| x.meets(group))
                && checks.iter().any(|c| c != group && r.area.meets(c))
        });
        if read || rolls_up {
            continue;
        }
        let label = book.label(sheet, cells[0]).cloned().unwrap_or_default();
        out.push(format!(
            "buried check: {} ('{label}') is read by no other formula, so no summary shows it",
            book.place(sheet, group)
        ));
    }
    out
}

/// The range a total cell sums, when its formula is `SUM` of one range on its
/// own sheet in the total's own column or row, with the total outside it.
fn summed(book: &Book, sheet: usize, at: (usize, usize), f: &Formula) -> Option<Area> {
    let text = f.text.trim_start_matches('=').trim();
    let inner = text.strip_prefix("SUM(")?.strip_suffix(')')?;
    if !re(r"^\$?[A-Z]{1,3}\$?[0-9]{1,7}:\$?[A-Z]{1,3}\$?[0-9]{1,7}$")
        .is_match(inner.trim())
        .unwrap_or(false)
    {
        return None;
    }
    let area = references(inner, f.origin, at, Some(sheet), &book.names)
        .first()?
        .area;
    let column = area.left == area.right && area.left == at.1;
    let row = area.top == area.bottom && area.top == at.0;
    ((column && !(area.top..=area.bottom).contains(&at.0))
        || (row && !(area.left..=area.right).contains(&at.1)))
    .then_some(area)
}

fn unlinked_rows(book: &Book) -> Vec<String> {
    let mut out = Vec::new();
    for ((sheet, vertical, step), cells) in gaps(book) {
        let (first, total, area) = cells[0];
        let last = cells[cells.len() - 1].0;
        let gap = if vertical {
            line(sheet, step, first, step, last)
        } else {
            line(sheet, first, step, last, step)
        };
        let label = book
            .label(sheet, (gap.top, gap.left))
            .map(|l| format!(" ('{l}')"))
            .unwrap_or_default();
        let range = format!(
            "{}:{}",
            a1((area.top, area.left)),
            a1((area.bottom, area.right))
        );
        out.push(format!(
            "unlinked row: {}{label} lies between SUM({range}) and its total {}, which leaves it out, and no formula reads it",
            book.place(sheet, &gap),
            a1(total)
        ));
    }
    out
}

fn line(sheet: usize, top: usize, left: usize, bottom: usize, right: usize) -> Area {
    Area {
        sheet,
        top,
        left,
        bottom,
        right,
    }
}

/// The unread cells between each SUM total and its range, by sheet, direction
/// and gap row (or column): each with its position along the gap, the total
/// and the summed range.
fn gaps(book: &Book) -> Gaps {
    let mut found = Gaps::new();
    for (sheet, grid) in book.grids.iter().enumerate() {
        for (&at, f) in &grid.formulas {
            let Some(area) = summed(book, sheet, at, f) else {
                continue;
            };
            let vertical = area.left == area.right && area.left == at.1;
            let (from, to) = match (vertical, at.0 > area.bottom, at.1 > area.right) {
                (true, true, _) => (area.bottom + 1, at.0),
                (true, false, _) => (at.0 + 1, area.top),
                (false, _, true) => (area.right + 1, at.1),
                (false, _, false) => (at.1 + 1, area.left),
            };
            for step in from..to {
                let gap = if vertical { (step, at.1) } else { (at.0, step) };
                // Reads from the gap's own row do not link it.
                let own = line(sheet, gap.0, 1, gap.0, MAX_COLUMN);
                let in_spill = grid
                    .spills
                    .iter()
                    .any(|s| s.holds(sheet, gap) && (s.top, s.left) != gap);
                let holds = grid.numbers.contains(&gap) || grid.formulas.contains_key(&gap);
                let cell = line(sheet, gap.0, gap.1, gap.0, gap.1);
                if holds && !in_spill && !book.is_read(&cell, &own) {
                    let along = if vertical { gap.1 } else { gap.0 };
                    found
                        .entry((sheet, vertical, step))
                        .or_default()
                        .push((along, at, area));
                }
            }
        }
    }
    found
}

fn edges(book: &Book) -> BTreeMap<Place, Vec<Place>> {
    let mut out = BTreeMap::new();
    for (sheet, grid) in book.grids.iter().enumerate() {
        for (&at, f) in &grid.formulas {
            let text = re(POSITION_ONLY).replace_all(&f.text, "0").into_owned();
            let mut areas: Vec<Area> = references(&text, f.origin, at, Some(sheet), &book.names)
                .into_iter()
                .map(|r| book.resolve(r))
                .collect();
            for token in name_tokens(&text) {
                areas.extend(book.name_areas(&book.plain_names, sheet, &token));
            }
            let mut to = BTreeSet::new();
            for area in areas {
                read_cells(book, area, &mut to);
            }
            out.insert((sheet, at.0, at.1), to.into_iter().collect());
        }
    }
    out
}

fn read_cells(book: &Book, area: Area, to: &mut BTreeSet<Place>) {
    let s = area.sheet;
    let g = &book.grids[s];
    for (&cell, _) in g.formulas.range((area.top, 0)..=(area.bottom, MAX_COLUMN)) {
        if (area.left..=area.right).contains(&cell.1) {
            to.insert((s, cell.0, cell.1));
        }
    }
    for spill in g.spills.iter().filter(|x| x.meets(&area)) {
        to.insert((s, spill.top, spill.left));
    }
}

fn circular(book: &Book) -> Vec<String> {
    let graph = edges(book);
    let nodes: Vec<Place> = graph.keys().copied().collect();
    let index_of: HashMap<Place, usize> = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let next: Vec<Vec<usize>> = nodes
        .iter()
        .map(|n| {
            graph[n]
                .iter()
                .filter_map(|t| index_of.get(t).copied())
                .collect()
        })
        .collect();
    let mut groups = components(&next);
    groups.sort();
    groups
        .into_iter()
        .map(|group| {
            let cells: Vec<String> = group
                .iter()
                .take(10)
                .map(|&i| {
                    let (s, r, c) = nodes[i];
                    format!("{}!{}", quoted(&book.names[s]), a1((r, c)))
                })
                .collect();
            let more = match group.len().saturating_sub(10) {
                0 => String::new(),
                n => format!(" and {n} more"),
            };
            format!(
                "circular reference: {}{more} read one another",
                cells.join(", ")
            )
        })
        .collect()
}

/// Tarjan's strongly connected components, iteratively: each group of 2 or
/// more nodes that reach one another, and each node that reads itself.
fn components(next: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = next.len();
    let (mut index, mut low, mut on) = (vec![usize::MAX; n], vec![0; n], vec![false; n]);
    let (mut stack, mut counter, mut groups) = (Vec::new(), 0, Vec::new());
    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        let mut work = vec![(root, 0)];
        while let Some((v, i)) = work.pop() {
            if i == 0 {
                (index[v], low[v], on[v]) = (counter, counter, true);
                counter += 1;
                stack.push(v);
            }
            if let Some(&w) = next[v].get(i) {
                work.push((v, i + 1));
                if index[w] == usize::MAX {
                    work.push((w, 0));
                } else if on[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            if low[v] == index[v] {
                let mut group = Vec::new();
                while let Some(w) = stack.pop() {
                    on[w] = false;
                    group.push(w);
                    if w == v {
                        break;
                    }
                }
                if group.len() > 1 || next[v].contains(&v) {
                    group.sort_unstable();
                    groups.push(group);
                }
            }
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[v]);
            }
        }
    }
    groups
}

/// Every reference-check finding, in names, checks, rows and cycles order.
pub(crate) fn findings(parts: &Parts, book: &Document<'_>, sheets: &[Sheet]) -> Vec<String> {
    let model = read_book(parts, book, sheets);
    let mut out = unused_names(parts, book, sheets);
    match &model.dynamic {
        Some(cell) => out.push(format!(
            "not checked for buried checks or unlinked rows: {cell} uses INDIRECT or OFFSET, so what reads a cell is not known"
        )),
        None => {
            out.extend(buried_checks(&model));
            out.extend(unlinked_rows(&model));
        }
    }
    out.extend(circular(&model));
    let iterative = book
        .descendants()
        .find(|n| n.tag_name().name() == "calcPr")
        .and_then(|n| attr(n, "iterate"))
        .is_some_and(|v| matches!(v, "1" | "true"));
    if iterative {
        out.push(
            "iterative calculation is on, so Excel does not warn of a circular reference".into(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheets() -> Vec<String> {
        vec!["Model".into(), "Cash flow".into(), "oz.Dλ".into()]
    }

    #[test]
    fn references_name_their_sheet_and_shift_when_relative() {
        let r = |f: &str, at| references(f, (2, 2), at, Some(0), &sheets());
        let area = |sheet, top, left, bottom, right| Area {
            sheet,
            top,
            left,
            bottom,
            right,
        };
        assert_eq!(r("A1", (2, 2))[0].area, area(0, 1, 1, 1, 1));
        assert_eq!(
            r("A1+$B$1", (4, 3))
                .iter()
                .map(|x| x.area)
                .collect::<Vec<_>>(),
            [area(0, 3, 2, 3, 2), area(0, 1, 2, 1, 2)]
        );
        assert_eq!(r("'Cash flow'!B2:C9", (2, 2))[0].area, area(1, 2, 2, 9, 3));
        assert_eq!(r("oz.Dλ!$A$1", (2, 2))[0].area, area(2, 1, 1, 1, 1));
        assert_eq!(r("SUM(C:C)", (2, 2))[0].area, area(0, 1, 3, MAX_ROW, 3));
        assert_eq!(r("SUM(3:4)", (2, 2))[0].area, area(0, 3, 1, 4, MAX_COLUMN));
        let sheets3 = |f: &str| {
            r(f, (2, 2))
                .iter()
                .map(|x| x.area.sheet)
                .collect::<Vec<_>>()
        };
        assert_eq!(sheets3("Model:oz.Dλ!A1"), [0, 1, 2]);
        assert_eq!(sheets3("'Model:Cash flow'!a1"), [0, 1]);
        assert_eq!(
            sheets3("SUM([1]Model!A1:A2)+Sales[Cost A1]"),
            Vec::<usize>::new()
        );
        assert!(r("A0", (3, 3)).is_empty());
        assert!(r("A1#", (2, 2))[0].spill);
        for text in [
            "[1]Model!A1",
            "'[Book.xlsx]Model'!A1",
            "\"A1\"",
            "LOG10(2)",
            "Gone!A1",
            "A1",
            "oz.Rate1",
        ] {
            let at = if text == "A1" { (1, 2) } else { (2, 2) };
            let found = r(text, at);
            assert!(found.is_empty() || text == "A1", "{text}: {found:?}");
        }
        // A1 written at B2, read at A2, would be column 0: off the sheet.
        assert!(r("A1", (2, 1)).is_empty());
    }

    #[test]
    fn name_tokens_skip_literals_and_structured_columns() {
        assert_eq!(
            name_tokens("Rate*\"Tax\"+Sales[[#This Row],[Tax]]+Model!Fee"),
            ["rate", "sales", "model", "fee"]
        );
    }

    #[test]
    fn position_only_arguments_are_not_dependencies() {
        assert_eq!(
            re(POSITION_ONLY).replace_all(
                r#"ROW(A5)+COLUMNS(Model!A1:C1)+CELL("filename",A1)+CELL("contents",A1)+A5"#,
                "0"
            ),
            r#"0+0+0+CELL("contents",A1)+A5"#
        );
    }
}
