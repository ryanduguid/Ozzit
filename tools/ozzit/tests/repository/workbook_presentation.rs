// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: FSL-1.1-ALv2

//! The shipped workbook's fills, dates, straplines, fonts and Cover wording.
use super::xml::{self, MAIN, REL};
use super::{part, parts};
use ozzit::archive::Parts;
use ozzit::common::{self, bounded};
use roxmltree::{Document, Node};

const EXPECTED_STRAPLINES: [(&str, &str); 8] = [
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
    ("oz.FinancialRatios", "Three dozen financial ratios"),
];

struct Workbook {
    parts: Parts,
    /// Each sheet's name and part, in workbook order.
    sheets: Vec<(String, String)>,
    strings: Vec<String>,
    styles: String,
}

impl Workbook {
    fn new() -> Self {
        let parts = parts("ozzit.xlsx");
        let workbook = part(&parts, "xl/workbook.xml");
        let rels = part(&parts, "xl/_rels/workbook.xml.rels");
        let rels = xml::parse(&rels);
        let target = |id: &str| {
            xml::elements(rels.root_element())
                .filter(|rel| rel.attribute("Id") == Some(id))
                .last()
                .and_then(|rel| rel.attribute("Target"))
                .unwrap_or_else(|| panic!("no relationship {id}"))
                .to_owned()
        };
        let book = xml::parse(&workbook);
        let sheets =
            xml::elements(xml::child(book.root_element(), MAIN, "sheets").expect("sheets"))
                .map(|sheet| {
                    let id = sheet.attribute((REL, "id")).expect("r:id");
                    let path = format!("xl/{}", target(id).trim_start_matches('/'));
                    (sheet.attribute("name").expect("name").to_owned(), path)
                })
                .collect();
        let strings = part(&parts, "xl/sharedStrings.xml");
        let strings = xml::children(xml::parse(&strings).root_element(), MAIN, "si")
            .map(xml::text)
            .collect();
        let styles = part(&parts, "xl/styles.xml");
        Self {
            parts,
            sheets,
            strings,
            styles,
        }
    }

    fn sheet(&self, name: &str) -> String {
        let (_, path) = self
            .sheets
            .iter()
            .find(|(sheet, _)| sheet == name)
            .unwrap_or_else(|| panic!("no sheet {name}"));
        part(&self.parts, path)
    }

    /// The cell's displayed text: its shared string, its value or "".
    fn cell_text(&self, root: Node<'_, '_>, address: &str) -> String {
        let Some(value) = xml::cell(root, address).and_then(|cell| {
            xml::child(cell, MAIN, "v").map(|v| (cell.attribute("t"), v.text().unwrap_or_default()))
        }) else {
            return String::new();
        };
        match value {
            (Some("s"), index) => {
                self.strings[index.parse::<usize>().expect("a shared string index")].clone()
            }
            (_, text) => text.to_owned(),
        }
    }
}

/// The styles part's `name` list, such as `cellXfs` or `fonts`.
fn style_list<'a, 'i>(styles: &'a Document<'i>, name: &'static str) -> Node<'a, 'i> {
    xml::child(styles.root_element(), MAIN, name).unwrap_or_else(|| panic!("no {name}"))
}

#[test]
fn no_legacy_mint_fill_is_used() {
    let book = Workbook::new();
    let styles = xml::parse(&book.styles);
    let cell_xfs = style_list(&styles, "cellXfs");
    let mint_fill_ids: Vec<usize> = xml::elements(style_list(&styles, "fills"))
        .enumerate()
        .filter(|(_, fill)| {
            fill.descendants()
                .any(|c| c.attribute("indexed") == Some("42"))
        })
        .map(|(index, _)| index)
        .collect();
    let mut used = vec![];
    for (name, path) in &book.sheets {
        let text = part(&book.parts, path);
        for cell in xml::descendants(xml::parse(&text).root_element(), MAIN, "c") {
            let Some(s) = cell.attribute("s") else {
                continue;
            };
            let style = xml::nth(cell_xfs, s.parse().expect("a style index"));
            if mint_fill_ids.contains(&xml::index(style, "fillId")) {
                used.push(format!(
                    "{name}!{}",
                    cell.attribute("r").unwrap_or_default()
                ));
            }
        }
    }
    assert_eq!(used, Vec::<String>::new());
}

#[test]
fn every_help_anchor_has_room_to_spill() {
    // A demonstration sheet spills its function's help from a bare =oz.Nameλ() call.
    // A help that grows past the cells below it caches #SPILL!, which the file stores
    // as an error value; ozzit verify_help_spills skips such an anchor.
    let book = Workbook::new();
    let mut anchors = 0;
    for (name, path) in &book.sheets {
        let text = part(&book.parts, path);
        let sheet = xml::parse(&text);
        for cell in sheet.descendants().filter(|n| n.has_tag_name((MAIN, "c"))) {
            let formula = xml::child(cell, MAIN, "f")
                .and_then(|f| f.text())
                .unwrap_or_default();
            if !common::fullmatch(r"oz\.\w+\(\)", formula) {
                continue;
            }
            anchors += 1;
            assert_ne!(
                cell.attribute("t"),
                Some("e"),
                "sheet={name} cell={}: the help has no room to spill here",
                cell.attribute("r").unwrap_or_default()
            );
        }
    }
    assert!(anchors >= 40, "{anchors} help anchors");
}

#[test]
fn populated_dates_do_not_use_locale_dependent_builtin_14() {
    let book = Workbook::new();
    let styles = xml::parse(&book.styles);
    let cell_xfs = style_list(&styles, "cellXfs");
    let mut bad = vec![];
    for (name, path) in &book.sheets {
        let text = part(&book.parts, path);
        for cell in xml::descendants(xml::parse(&text).root_element(), MAIN, "c") {
            let Some(s) = cell.attribute("s") else {
                continue;
            };
            let Some(value) = xml::child(cell, MAIN, "v")
                .and_then(|v| v.text())
                .filter(|v| !v.is_empty())
            else {
                continue;
            };
            if matches!(cell.attribute("t"), Some("s" | "str" | "inlineStr")) {
                continue;
            }
            // No styled cell holds a non-number value.
            let Ok(serial) = ozzit::py::parse_float(value) else {
                continue;
            };
            let style = xml::nth(cell_xfs, s.parse().expect("a style index"));
            if serial > 20_000.0 && style.attribute("numFmtId").unwrap_or("0") == "14" {
                bad.push(format!(
                    "{name}!{}",
                    cell.attribute("r").unwrap_or_default()
                ));
            }
        }
    }
    assert_eq!(bad, Vec::<String>::new());
}

#[test]
fn essentials_and_ratio_straplines_match_toc_descriptions() {
    let book = Workbook::new();
    for (sheet, expected) in EXPECTED_STRAPLINES {
        let text = book.sheet(sheet);
        assert_eq!(
            book.cell_text(xml::parse(&text).root_element(), "A2"),
            expected,
            "{sheet}"
        );
    }
}

#[test]
fn function_strapline_font_meets_house_neutral() {
    let book = Workbook::new();
    let styles = xml::parse(&book.styles);
    let (cell_xfs, fonts) = (style_list(&styles, "cellXfs"), style_list(&styles, "fonts"));
    let mut bad = vec![];
    for (name, path) in &book.sheets {
        if !name.starts_with("oz.") {
            continue;
        }
        let text = part(&book.parts, path);
        let root = xml::parse(&text);
        let root = root.root_element();
        let Some(cell) = xml::descendants(root, MAIN, "c")
            .find(|c| c.attribute("r") == Some("A2") && c.has_attribute("s"))
        else {
            continue; // every oz. sheet has a styled A2 strapline
        };
        if book.cell_text(root, "A2").is_empty() {
            continue;
        }
        let style = xml::nth(cell_xfs, xml::index(cell, "s"));
        let font = xml::nth(fonts, xml::index(style, "fontId"));
        let rgb = font
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name().ends_with("color"))
            .and_then(|node| node.attribute("rgb"));
        if !matches!(rgb, Some("FF6E6862" | "FF000000" | "FF66023C")) {
            bad.push((name.clone(), rgb.map(str::to_owned)));
        }
    }
    assert_eq!(bad, vec![]);
}

#[test]
fn cover_describes_brand_neutral_formula_highlights() {
    let book = Workbook::new();
    let text = book.sheet("Cover");
    let cover = xml::parse(&text);
    let cover = cover.root_element();
    // The 3 rows under the "Accessing the Functions" heading, wherever the
    // heading sits: the Cover's rows have moved before.
    let heading: usize = xml::descendants(cover, MAIN, "row")
        .map(|row| row.attribute("r").expect("a row number"))
        .find(|r| book.cell_text(cover, &format!("A{r}")) == "Accessing the Functions")
        .expect("the Cover has no 'Accessing the Functions' heading")
        .parse()
        .expect("a row number");
    let text = (1..=3)
        .flat_map(|offset| {
            ["A", "B"].map(|column| book.cell_text(cover, &format!("{column}{}", heading + offset)))
        })
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    assert!(
        !common::matches(&bounded(r"\bgreen\s+shaded\b"), &text),
        "{text}"
    );
    assert!(text.contains("purple"), "{text}");
}
