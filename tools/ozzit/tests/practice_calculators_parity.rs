// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! Gate A semantic OOXML parity, structural invariants, contrast and CLI tests
//! for `build_practice_calculators`.
use ozzit::archive::{Parts, read_parts};
use ozzit::common::{column_letters, column_number, sub_fn};
use ozzit::formula::split_literals;
use ozzit::practice_calculators::{SHEET_NAMES, build_bytes, build_parts, write_workbook};
use ozzit::py;
use roxmltree::{Document, Node};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DRAW: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const REF_RE: &str = r"(?<![A-Za-z0-9_$.])(\$?)([A-Z]{1,3})(\$?)([0-9]{1,7})(?![A-Za-z0-9_(.])";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn part_text(parts: &Parts, name: &str) -> String {
    std::str::from_utf8(parts.get(name).unwrap_or_else(|| panic!("missing {name}")))
        .expect("utf-8 xml")
        .to_owned()
}

fn parse_xml(text: &str) -> Document<'_> {
    Document::parse(text).expect("valid xml")
}

fn child<'a, 'i>(node: Node<'a, 'i>, ns: &str, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|n| n.is_element() && n.has_tag_name((ns, name)))
}

fn elements<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(Node::is_element)
}

fn parse_a1(addr: &str) -> (usize, usize) {
    let split = addr.find(|c: char| c.is_ascii_digit()).expect("a1 row");
    (
        addr[split..].parse().expect("row"),
        column_number(&addr[..split]),
    )
}

fn expand_range(sqref: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for piece in sqref.split_whitespace() {
        let ((r1, c1), (r2, c2)) = match piece.split_once(':') {
            Some((a, b)) => (parse_a1(a), parse_a1(b)),
            None => {
                let p = parse_a1(piece);
                (p, p)
            }
        };
        for r in r1..=r2 {
            for c in c1..=c2 {
                out.push((r, c));
            }
        }
    }
    out
}

fn shift_formula(formula: &str, dr: isize, dc: isize) -> String {
    let mut out = String::new();
    for (literal, piece) in split_literals(formula) {
        if literal {
            out.push_str(piece);
            continue;
        }
        out.push_str(&sub_fn(REF_RE, piece, |m| {
            let c_abs = &m[1];
            let r_abs = &m[3];
            let col = column_number(&m[2]) as isize + if c_abs.is_empty() { dc } else { 0 };
            let row = m[4].parse::<isize>().expect("row") + if r_abs.is_empty() { dr } else { 0 };
            format!("{c_abs}{}{r_abs}{row}", column_letters(col as usize))
        }));
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedStyle {
    num_fmt: String,
    font_name: String,
    font_size: String,
    bold: bool,
    italic: bool,
    underline: bool,
    font_rgb: String,
    fill_rgb: String,
    bottom_border: Option<(String, String)>,
    h_align: String,
    v_align: String,
    wrap: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedDxf {
    bold: bool,
    font_rgb: String,
    fill_rgb: String,
}

struct StylesContext {
    xfs: Vec<ResolvedStyle>,
    dxfs: Vec<ResolvedDxf>,
}

fn norm_fmt(code: &str) -> String {
    code.replace(r"\(", "(")
        .replace(r"\)", ")")
        .replace(r"\-", "-")
}

fn builtin_fmt(id: &str) -> String {
    match id {
        "0" => "General".into(),
        "1" => "0".into(),
        "2" => "0.00".into(),
        "17" => "mmm-yy".into(),
        "49" => "@".into(),
        other => format!("builtin:{other}"),
    }
}

fn resolve_font(node: Node<'_, '_>) -> (String, String, bool, bool, bool, String) {
    let name = child(node, MAIN, "name")
        .and_then(|n| n.attribute("val"))
        .unwrap_or("")
        .to_owned();
    let size = child(node, MAIN, "sz")
        .and_then(|n| n.attribute("val"))
        .unwrap_or("11")
        .to_owned();
    let bold = child(node, MAIN, "b").is_some();
    let italic = child(node, MAIN, "i").is_some();
    let underline = child(node, MAIN, "u").is_some();
    let rgb = child(node, MAIN, "color")
        .and_then(|c| c.attribute("rgb").or_else(|| c.attribute("theme")))
        .unwrap_or("")
        .to_owned();
    (name, size, bold, italic, underline, rgb)
}

fn resolve_fill(node: Node<'_, '_>) -> String {
    let Some(pf) = child(node, MAIN, "patternFill") else {
        return String::new();
    };
    if pf.attribute("patternType") != Some("solid") {
        return pf.attribute("patternType").unwrap_or("").to_owned();
    }
    child(pf, MAIN, "fgColor")
        .or_else(|| child(pf, MAIN, "bgColor"))
        .and_then(|c| c.attribute("rgb"))
        .unwrap_or("")
        .to_owned()
}

fn resolve_border(node: Node<'_, '_>) -> Option<(String, String)> {
    let bottom = child(node, MAIN, "bottom")?;
    let style = bottom.attribute("style")?.to_owned();
    let rgb = child(bottom, MAIN, "color")
        .and_then(|c| c.attribute("rgb"))
        .unwrap_or("")
        .to_owned();
    Some((style, rgb))
}

fn parse_styles(xml: &str) -> StylesContext {
    let doc = parse_xml(xml);
    let root = doc.root_element();
    let mut num_fmts = HashMap::new();
    if let Some(nfs) = child(root, MAIN, "numFmts") {
        for nf in elements(nfs) {
            num_fmts.insert(
                nf.attribute("numFmtId").expect("id").to_owned(),
                norm_fmt(nf.attribute("formatCode").expect("code")),
            );
        }
    }
    let fonts: Vec<_> = elements(child(root, MAIN, "fonts").expect("fonts"))
        .map(resolve_font)
        .collect();
    let fills: Vec<_> = elements(child(root, MAIN, "fills").expect("fills"))
        .map(resolve_fill)
        .collect();
    let borders: Vec<_> = elements(child(root, MAIN, "borders").expect("borders"))
        .map(resolve_border)
        .collect();
    let xfs = elements(child(root, MAIN, "cellXfs").expect("cellXfs"))
        .map(|xf| build_resolved_xf(xf, &num_fmts, &fonts, &fills, &borders))
        .collect();
    let dxfs = child(root, MAIN, "dxfs")
        .map(|d| elements(d).map(parse_dxf).collect())
        .unwrap_or_default();
    StylesContext { xfs, dxfs }
}

fn build_resolved_xf(
    xf: Node<'_, '_>,
    num_fmts: &HashMap<String, String>,
    fonts: &[(String, String, bool, bool, bool, String)],
    fills: &[String],
    borders: &[Option<(String, String)>],
) -> ResolvedStyle {
    let nf_id = xf.attribute("numFmtId").unwrap_or("0");
    let num_fmt = num_fmts
        .get(nf_id)
        .cloned()
        .unwrap_or_else(|| builtin_fmt(nf_id));
    let fi: usize = xf.attribute("fontId").unwrap_or("0").parse().unwrap();
    let li: usize = xf.attribute("fillId").unwrap_or("0").parse().unwrap();
    let bi: usize = xf.attribute("borderId").unwrap_or("0").parse().unwrap();
    let al = child(xf, MAIN, "alignment");
    let font = &fonts[fi];
    ResolvedStyle {
        num_fmt,
        font_name: font.0.clone(),
        font_size: font.1.clone(),
        bold: font.2,
        italic: font.3,
        underline: font.4,
        font_rgb: font.5.clone(),
        fill_rgb: fills[li].clone(),
        bottom_border: borders[bi].clone(),
        h_align: al
            .and_then(|a| a.attribute("horizontal"))
            .unwrap_or("")
            .to_owned(),
        v_align: al
            .and_then(|a| a.attribute("vertical"))
            .unwrap_or("")
            .to_owned(),
        wrap: al.and_then(|a| a.attribute("wrapText")) == Some("1"),
    }
}

fn parse_dxf(node: Node<'_, '_>) -> ResolvedDxf {
    let font = child(node, MAIN, "font");
    let bold = font.and_then(|f| child(f, MAIN, "b")).is_some();
    let font_rgb = font
        .and_then(|f| child(f, MAIN, "color"))
        .and_then(|c| c.attribute("rgb"))
        .unwrap_or("")
        .to_owned();
    let fill_rgb = child(node, MAIN, "fill")
        .map(resolve_fill)
        .unwrap_or_default();
    ResolvedDxf {
        bold,
        font_rgb,
        fill_rgb,
    }
}

fn sheet_paths(parts: &Parts) -> Vec<(String, String)> {
    let book_xml = part_text(parts, "xl/workbook.xml");
    let rels_xml = part_text(parts, "xl/_rels/workbook.xml.rels");
    let book = parse_xml(&book_xml);
    let rels = parse_xml(&rels_xml);
    elements(child(book.root_element(), MAIN, "sheets").expect("sheets"))
        .map(|s| {
            let id = s.attribute((REL, "id")).expect("r:id");
            let target = elements(rels.root_element())
                .find(|r| r.attribute("Id") == Some(id))
                .and_then(|r| r.attribute("Target"))
                .expect("target");
            let path = if target.starts_with('/') {
                target.trim_start_matches('/').to_owned()
            } else {
                format!("xl/{target}")
            };
            (s.attribute("name").expect("name").to_owned(), path)
        })
        .collect()
}

fn parse_sst(parts: &Parts) -> Vec<String> {
    let xml = part_text(parts, "xl/sharedStrings.xml");
    let doc = parse_xml(&xml);
    elements(doc.root_element())
        .map(|si| {
            si.descendants()
                .filter(Node::is_text)
                .filter_map(|t| t.text())
                .collect()
        })
        .collect()
}

#[derive(Clone, Debug)]
enum CellContent {
    Empty,
    Formula(String),
    Text(String),
    Number(f64),
}

type CfRule = (String, String, String, ResolvedDxf);
type CfMap = BTreeMap<(usize, usize), Vec<CfRule>>;

struct ParsedSheet {
    tab_color: String,
    grid_lines: String,
    tab_selected: bool,
    pane: Option<(String, String, String, String, String)>,
    col_widths: BTreeMap<usize, String>,
    row_heights: BTreeMap<usize, f64>,
    cells: BTreeMap<(usize, usize), (ResolvedStyle, CellContent)>,
    cfs: CfMap,
    dvs: BTreeMap<(usize, usize), (String, String)>,
    hyperlinks: BTreeSet<(String, String, String)>,
}

fn parse_sheet(xml: &str, styles: &StylesContext, sst: &[String]) -> ParsedSheet {
    let doc = parse_xml(xml);
    let root = doc.root_element();
    let tab_color = child(root, MAIN, "sheetPr")
        .and_then(|p| child(p, MAIN, "tabColor"))
        .and_then(|c| c.attribute("rgb"))
        .unwrap_or("")
        .to_owned();
    let sv = child(root, MAIN, "sheetViews")
        .and_then(|v| child(v, MAIN, "sheetView"))
        .expect("sheetView");
    let grid_lines = sv.attribute("showGridLines").unwrap_or("1").to_owned();
    let tab_selected = sv.attribute("tabSelected") == Some("1");
    let pane = child(sv, MAIN, "pane").map(|p| {
        (
            p.attribute("xSplit").unwrap_or("0").to_owned(),
            p.attribute("ySplit").unwrap_or("0").to_owned(),
            p.attribute("topLeftCell").unwrap_or("").to_owned(),
            p.attribute("activePane").unwrap_or("").to_owned(),
            p.attribute("state").unwrap_or("").to_owned(),
        )
    });
    ParsedSheet {
        tab_color,
        grid_lines,
        tab_selected,
        pane,
        col_widths: read_cols(root),
        row_heights: read_row_heights(root),
        cells: read_cells(root, styles, sst),
        cfs: read_cfs(root, styles),
        dvs: read_dvs(root),
        hyperlinks: read_hyperlinks(root),
    }
}

fn read_cols(root: Node<'_, '_>) -> BTreeMap<usize, String> {
    let mut widths = BTreeMap::new();
    if let Some(cols) = child(root, MAIN, "cols") {
        for col in elements(cols) {
            let min: usize = col.attribute("min").unwrap().parse().unwrap();
            let max: usize = col.attribute("max").unwrap().parse().unwrap();
            let width = col.attribute("width").unwrap().to_owned();
            for c in min..=max {
                widths.insert(c, width.clone());
            }
        }
    }
    widths
}

fn read_row_heights(root: Node<'_, '_>) -> BTreeMap<usize, f64> {
    let mut heights = BTreeMap::new();
    for row in elements(child(root, MAIN, "sheetData").expect("sheetData")) {
        if let Some(ht) = row.attribute("ht") {
            let r: usize = row.attribute("r").unwrap().parse().unwrap();
            heights.insert(r, ht.parse().unwrap());
        }
    }
    heights
}

fn read_cells(
    root: Node<'_, '_>,
    styles: &StylesContext,
    sst: &[String],
) -> BTreeMap<(usize, usize), (ResolvedStyle, CellContent)> {
    let mut cells = BTreeMap::new();
    let mut shared: HashMap<String, ((usize, usize), String)> = HashMap::new();
    for row in elements(child(root, MAIN, "sheetData").expect("sheetData")) {
        for c in elements(row) {
            let at = parse_a1(c.attribute("r").expect("r"));
            let s_idx: usize = c.attribute("s").unwrap_or("0").parse().unwrap();
            let content = decode_cell(c, at, sst, &mut shared);
            cells.insert(at, (styles.xfs[s_idx].clone(), content));
        }
    }
    cells
}

fn decode_cell(
    c: Node<'_, '_>,
    at: (usize, usize),
    sst: &[String],
    shared: &mut HashMap<String, ((usize, usize), String)>,
) -> CellContent {
    if let Some(f) = child(c, MAIN, "f") {
        let text = f.text().unwrap_or("");
        let expr = if f.attribute("t") == Some("shared") {
            let si = f.attribute("si").expect("si").to_owned();
            if text.is_empty() {
                let (anchor, base) = &shared[&si];
                shift_formula(
                    base,
                    at.0 as isize - anchor.0 as isize,
                    at.1 as isize - anchor.1 as isize,
                )
            } else {
                shared.insert(si, (at, text.to_owned()));
                text.to_owned()
            }
        } else {
            text.to_owned()
        };
        return CellContent::Formula(expr);
    }
    let Some(v) = child(c, MAIN, "v").and_then(|v| v.text()) else {
        return CellContent::Empty;
    };
    if c.attribute("t") == Some("s") {
        CellContent::Text(sst[v.parse::<usize>().expect("sst")].clone())
    } else {
        CellContent::Number(v.parse().expect("number"))
    }
}

fn read_cfs(root: Node<'_, '_>, styles: &StylesContext) -> CfMap {
    let mut map: CfMap = BTreeMap::new();
    for cf in root
        .children()
        .filter(|n| n.has_tag_name((MAIN, "conditionalFormatting")))
    {
        let cells = expand_range(cf.attribute("sqref").expect("sqref"));
        let anchor = cells[0];
        for rule in elements(cf) {
            let kind = rule.attribute("type").unwrap_or("").to_owned();
            let op = rule.attribute("operator").unwrap_or("").to_owned();
            let dxf_id: usize = rule.attribute("dxfId").unwrap().parse().unwrap();
            let base = child(rule, MAIN, "formula")
                .and_then(|f| f.text())
                .unwrap_or("");
            for at in &cells {
                let norm = shift_formula(
                    base,
                    at.0 as isize - anchor.0 as isize,
                    at.1 as isize - anchor.1 as isize,
                );
                map.entry(*at).or_default().push((
                    kind.clone(),
                    op.clone(),
                    norm,
                    styles.dxfs[dxf_id].clone(),
                ));
            }
        }
    }
    map
}

fn read_dvs(root: Node<'_, '_>) -> BTreeMap<(usize, usize), (String, String)> {
    let mut map = BTreeMap::new();
    if let Some(dvs) = child(root, MAIN, "dataValidations") {
        for dv in elements(dvs) {
            let kind = dv.attribute("type").unwrap_or("").to_owned();
            let f1 = child(dv, MAIN, "formula1")
                .and_then(|f| f.text())
                .unwrap_or("")
                .to_owned();
            for at in expand_range(dv.attribute("sqref").expect("sqref")) {
                map.insert(at, (kind.clone(), f1.clone()));
            }
        }
    }
    map
}

fn read_hyperlinks(root: Node<'_, '_>) -> BTreeSet<(String, String, String)> {
    child(root, MAIN, "hyperlinks")
        .into_iter()
        .flat_map(elements)
        .map(|h| {
            (
                h.attribute("ref").unwrap_or("").to_owned(),
                h.attribute("location").unwrap_or("").to_owned(),
                h.attribute("display").unwrap_or("").to_owned(),
            )
        })
        .collect()
}

fn theme_scheme(parts: &Parts) -> Vec<(String, String)> {
    let xml = part_text(parts, "xl/theme/theme1.xml");
    let doc = parse_xml(&xml);
    let scheme = doc
        .descendants()
        .find(|n| n.has_tag_name((DRAW, "clrScheme")))
        .expect("clrScheme");
    elements(scheme)
        .map(|entry| {
            let val = elements(entry)
                .next()
                .and_then(|c| c.attribute("val"))
                .unwrap_or("")
                .to_owned();
            (entry.tag_name().name().to_owned(), val)
        })
        .collect()
}

fn assert_cell_equal(sheet: &str, at: (usize, usize), a: &CellContent, b: &CellContent) {
    match (a, b) {
        (CellContent::Empty, CellContent::Empty) => {}
        (CellContent::Formula(fa), CellContent::Formula(fb)) => {
            assert_eq!(fa, fb, "{sheet} {at:?}")
        }
        (CellContent::Text(ta), CellContent::Text(tb)) => assert_eq!(ta, tb, "{sheet} {at:?}"),
        (CellContent::Number(na), CellContent::Number(nb)) => {
            assert!((na - nb).abs() <= 1e-12, "{sheet} {at:?}: {na} != {nb}");
        }
        _ => panic!("{sheet} {at:?}: content mismatch {a:?} vs {b:?}"),
    }
}

fn assert_sheet_parity(name: &str, cand: &ParsedSheet, ship: &ParsedSheet) {
    assert_eq!(cand.tab_color, ship.tab_color, "{name} tabColor");
    assert_eq!(cand.grid_lines, ship.grid_lines, "{name} gridLines");
    assert_eq!(cand.tab_selected, ship.tab_selected, "{name} tabSelected");
    assert_eq!(cand.pane, ship.pane, "{name} pane");
    assert_eq!(cand.col_widths, ship.col_widths, "{name} col_widths");
    assert_eq!(
        cand.row_heights.len(),
        ship.row_heights.len(),
        "{name} rows"
    );
    for (r, h) in &cand.row_heights {
        assert!((h - ship.row_heights[r]).abs() <= 0.15, "{name} row {r}");
    }
    assert_eq!(cand.hyperlinks, ship.hyperlinks, "{name} hyperlinks");
    assert_eq!(cand.dvs, ship.dvs, "{name} dataValidations");
    assert_eq!(cand.cfs, ship.cfs, "{name} conditionalFormatting");
    assert_eq!(cand.cells.len(), ship.cells.len(), "{name} cell count");
    for (at, (cs, cv)) in &cand.cells {
        let (ss, sv) = &ship.cells[at];
        assert_eq!(cs, ss, "{name} {at:?} style");
        assert_cell_equal(name, *at, cv, sv);
    }
}

#[test]
fn candidate_matches_shipped_practice_template_semantically() {
    let cand_parts = build_parts().unwrap();
    let ship_parts = read_parts(&root().join("templates/practice-calculators.xlsx")).unwrap();
    assert_eq!(theme_scheme(&cand_parts), theme_scheme(&ship_parts));
    let (cand_sheets, ship_sheets) = (sheet_paths(&cand_parts), sheet_paths(&ship_parts));
    assert_eq!(cand_sheets, ship_sheets);
    let cand_styles = parse_styles(&part_text(&cand_parts, "xl/styles.xml"));
    let ship_styles = parse_styles(&part_text(&ship_parts, "xl/styles.xml"));
    let (cand_sst, ship_sst) = (parse_sst(&cand_parts), parse_sst(&ship_parts));
    let mut formula_count = 0usize;
    for (name, path) in &cand_sheets {
        let cand_xml = part_text(&cand_parts, path);
        assert_no_fabricated_formula_cache(&cand_xml, name);
        let cand = parse_sheet(&cand_xml, &cand_styles, &cand_sst);
        let ship = parse_sheet(&part_text(&ship_parts, path), &ship_styles, &ship_sst);
        formula_count += cand
            .cells
            .values()
            .filter(|(_, c)| matches!(c, CellContent::Formula(_)))
            .count();
        assert_sheet_parity(name, &cand, &ship);
    }
    assert_eq!(formula_count, 2766);
}

fn assert_no_fabricated_formula_cache(xml: &str, name: &str) {
    let doc = parse_xml(xml);
    for c in doc.descendants().filter(|n| n.has_tag_name((MAIN, "c"))) {
        if child(c, MAIN, "f").is_some() {
            assert!(
                child(c, MAIN, "v").is_none(),
                "{name}!{} must not fabricate cached <v>",
                c.attribute("r").unwrap_or_default()
            );
        }
    }
}

fn luminance(rgb: &str) -> f64 {
    let ch = |i: usize| f64::from(u8::from_str_radix(&rgb[i..i + 2], 16).unwrap()) / 255.0;
    let lin = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    [0, 2, 4]
        .into_iter()
        .zip([0.2126, 0.7152, 0.0722])
        .map(|(i, w)| lin(ch(i)) * w)
        .sum()
}

fn contrast(fg: &str, bg: &str) -> f64 {
    let (a, b) = (luminance(fg), luminance(bg));
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn candidate_passes_structural_navigation_and_contrast_gates() {
    let parts = build_parts().unwrap();
    assert!(
        !parts
            .keys()
            .any(|p| p.to_lowercase().contains("vbaproject"))
    );
    assert!(!parts.keys().any(|p| p.starts_with("xl/externalLinks/")));
    assert!(!part_text(&parts, "[Content_Types].xml").contains("macroEnabled"));
    assert!(
        part_text(&parts, "xl/workbook.xml")
            .contains(r#"<calcPr calcId="191029" fullCalcOnLoad="1"/>"#)
    );
    assert!(part_text(&parts, "xl/styles.xml").contains(r#"<cellStyles count="2"><cellStyle name="Hyperlink" xfId="1" builtinId="8"/><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>"#));
    for name in parts.keys().filter(|n| n.ends_with(".rels")) {
        let rels_xml = part_text(&parts, name);
        let doc = parse_xml(&rels_xml);
        assert!(
            !elements(doc.root_element()).any(|r| r.attribute("TargetMode") == Some("External"))
        );
    }
    let styles = parse_styles(&part_text(&parts, "xl/styles.xml"));
    let sst = parse_sst(&parts);
    let sheets = sheet_paths(&parts);
    assert_eq!(
        sheets.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        SHEET_NAMES
    );
    check_candidate_sheets(&parts, &sheets, &styles, &sst);
}

fn hex6(rgb: &str) -> &str {
    match rgb.len() {
        0 => "000000",
        8 => &rgb[2..],
        6 => rgb,
        _ => panic!("unexpected color value: {rgb}"),
    }
}

fn check_candidate_sheets(
    parts: &Parts,
    sheets: &[(String, String)],
    styles: &StylesContext,
    sst: &[String],
) {
    for (i, (name, path)) in sheets.iter().enumerate() {
        let parsed = parse_sheet(&part_text(parts, path), styles, sst);
        for ((r, c), (st, content)) in &parsed.cells {
            if matches!(content, CellContent::Text(_) | CellContent::Number(_)) {
                let fg = hex6(&st.font_rgb);
                let bg = match st.fill_rgb.as_str() {
                    "" | "none" | "gray125" => "FFFFFF",
                    s => hex6(s),
                };
                let pts = py::parse_float(&st.font_size).unwrap();
                let floor = if pts >= 18.0 || (pts >= 14.0 && st.bold) {
                    3.0
                } else {
                    4.5
                };
                assert!(contrast(fg, bg) >= floor, "{name}!({r},{c}) contrast");
            }
        }
        if i == 0 {
            assert_eq!(parsed.hyperlinks.len(), 21);
        } else {
            assert_eq!(
                parsed.hyperlinks,
                BTreeSet::from([("B1".into(), "'Read me'!B2".into(), "Contents".into())])
            );
        }
    }
    for d in &styles.dxfs {
        assert!(contrast(hex6(&d.font_rgb), hex6(&d.fill_rgb)) >= 4.5);
    }
}

#[test]
fn builder_bytes_are_deterministic_and_audit_passes() {
    let a = build_bytes().unwrap();
    assert_eq!(a, build_bytes().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("direct.xlsx");
    assert!(write_workbook(&out).unwrap() > 0);
    assert_eq!(std::fs::read(&out).unwrap(), a);
    let audit = Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .args(["audit", out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(audit.status.success(), "{audit:?}");
}

#[test]
fn cli_handles_flags_default_path_and_errors() {
    let a = build_bytes().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("nested/practice.xlsx");
    let exe = env!("CARGO_BIN_EXE_build_practice_calculators");
    let run_exe = |args: &[&str], cwd: Option<&std::path::Path>| {
        let mut cmd = Command::new(exe);
        cmd.args(args);
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        cmd.output().unwrap()
    };
    let help = run_exe(&["--help"], None);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--force"));
    assert!(run_exe(&["-h"], None).status.success());
    let empty = run_exe(&[], Some(dir.path()));
    assert_eq!(empty.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&empty.stderr).contains("--force"));
    let def_path = dir.path().join("templates/practice-calculators.xlsx");
    assert!(!def_path.exists());
    assert_eq!(run_exe(&["--unknown"], None).status.code(), Some(2));
    assert!(
        run_exe(&["--out", out.to_str().unwrap()], None)
            .status
            .success()
    );
    assert_eq!(std::fs::read(&out).unwrap(), a);
    let short_out = dir.path().join("short.xlsx");
    assert!(
        run_exe(&["-o", short_out.to_str().unwrap()], None)
            .status
            .success()
    );
    assert_eq!(std::fs::read(&short_out).unwrap(), a);
    let pos_out = dir.path().join("positional.xlsx");
    assert!(run_exe(&[pos_out.to_str().unwrap()], None).status.success());
    assert_eq!(std::fs::read(&pos_out).unwrap(), a);
    assert!(run_exe(&["--force"], Some(dir.path())).status.success());
    assert_eq!(std::fs::read(&def_path).unwrap(), a);
    assert_eq!(
        run_exe(&[dir.path().to_str().unwrap()], None).status.code(),
        Some(1)
    );
}
