// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! `docs/modelling-conventions.md`, held to the facts it states about `src/` and `ozzit.xlsx`.
//!
//! The page records where Ozzit follows or departs from the FAST Standard and
//! the ICAEW Financial Modelling Code. Each check recomputes a figure or fact
//! the page states and fails when it changes, so the page is revisited rather
//! than left out of date. Help-text lines (those holding the help table's
//! arrows or opening with a quotation mark) are excluded, so an example inside
//! a help table does not count as a function calling the function it shows.
use super::{findall, flat, modules, names, part, parts, read, search};
use indexmap::IndexMap;
use ozzit::common::{self, bounded};
use std::collections::BTreeSet;

const WORKBOOK: &str = "ozzit.xlsx";
const DEFINITION: &str = r"(?m)^([A-Za-z](?:\w|\.)*λ\w*)\s*=\s*LAMBDA\(";
const SHEET: &str = r"xl/worksheets/sheet\d+\.xml";

fn page() -> String {
    flat(&read("docs/modelling-conventions.md"))
}

fn page_says(page: &str, claim: &str) {
    assert!(page.contains(claim), "the page no longer says {claim:?}");
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|&n| n.to_owned()).collect()
}

/// Each definition's start and name, in the module's order.
fn definitions(text: &str) -> Vec<(usize, String)> {
    common::re(DEFINITION)
        .captures_iter(text)
        .map(|m| {
            let m = m.expect("regular expression execution");
            (m.get(0).unwrap().start(), m[1].to_owned())
        })
        .collect()
}

/// Each definition's text, from its assignment to the next one.
fn blocks(text: &str) -> Vec<(String, &str)> {
    let starts = definitions(text);
    starts
        .iter()
        .enumerate()
        .map(|(i, (start, name))| {
            let end = starts.get(i + 1).map_or(text.len(), |(next, _)| *next);
            (name.clone(), &text[*start..end])
        })
        .collect()
}

/// Each function's source lines, without its help text.
fn function_bodies() -> IndexMap<String, Vec<String>> {
    let mut bodies = IndexMap::new();
    for (_, text) in modules() {
        for (name, block) in blocks(&text) {
            let lines = ozzit::py::splitlines(block)
                .into_iter()
                .filter(|line| {
                    !line.contains('→')
                        && !line
                            .trim_start_matches(ozzit::py::is_space)
                            .starts_with('"')
                })
                .map(str::to_owned)
                .collect();
            bodies.insert(name, lines);
        }
    }
    bodies
}

/// The formula, if any, in the first cell of every merged range.
fn merged_formulas() -> Vec<String> {
    let parts = parts(WORKBOOK);
    let mut formulas = vec![];
    for name in names(WORKBOOK) {
        if !common::fullmatch(SHEET, &name) {
            continue;
        }
        let sheet = part(&parts, &name);
        for first in findall(r#"<mergeCell ref="([A-Z]+\d+):"#, &sheet) {
            let cell = search(&format!(r#"(?s)<c r="{first}"[^>]*>(.*?)</c>"#), &sheet);
            let formula = cell.and_then(|c| search(r"(?s)<f[^>]*>(.*?)</f>", &c[1]));
            if let Some(formula) = formula {
                formulas.push(format!("={}", common::unescape(&formula[1]).unwrap()));
            }
        }
    }
    formulas
}

fn calling(pattern: &str) -> BTreeSet<String> {
    let pattern = bounded(pattern);
    function_bodies()
        .into_iter()
        .filter(|(_, lines)| lines.iter().any(|line| common::matches(&pattern, line)))
        .map(|(name, _)| name)
        .collect()
}

#[test]
fn the_function_and_help_table_counts() {
    let page = page();
    assert_eq!(function_bodies().len(), 150);
    page_says(&page, "a library of 150 LAMBDA functions and 5 help tables");
    page_says(&page, "Ozzit's 150 functions exist to be called from cells");
    let tables: BTreeSet<String> = modules()
        .iter()
        .flat_map(|(_, text)| findall(r"(?m)^(About\w+λ)\s*=", text))
        .collect();
    assert_eq!(tables.len(), 5);
    page_says(&page, "5 About tables list each module's functions");
}

#[test]
fn every_function_is_listed_in_an_about_table() {
    // An About table row opens with the function's name; the diagnostic companions
    // are listed with the function they check.
    let mut listed = BTreeSet::new();
    for (_, text) in modules() {
        for table in findall(r"(?ms)^About\w+λ\s*=\s*TRIM\(.*?\);$", &text) {
            listed.extend(findall(r#""(\w+λ)\s*→"#, &table));
        }
    }
    let unlisted: BTreeSet<String> = function_bodies()
        .into_keys()
        .filter(|name| !name.ends_with("λDV") && !listed.contains(name))
        .collect();
    assert_eq!(unlisted, BTreeSet::new());
}

#[test]
fn utilities_repeats_the_essentials_functions() {
    let defined = |module: &str| -> BTreeSet<String> {
        findall(DEFINITION, &read(&format!("src/{module}.txt")))
            .into_iter()
            .collect()
    };
    let (essentials, utilities) = (defined("Essentials"), defined("Utilities"));
    assert_eq!(essentials.len(), 16);
    // Each Essentials name reappears in Utilities with a U before the λ; the
    // E-tagged range function is Essentials' own copy of RangeToDAλ.
    let renamed: BTreeSet<String> = essentials
        .iter()
        .map(|name| name.replace("Eλ", "λ").replace('λ', "Uλ"))
        .collect();
    assert_eq!(renamed, utilities);
    page_says(
        &page(),
        "Utilities repeats the 16 Essentials functions with a `U` suffix",
    );
}

#[test]
fn if_and_nested_if() {
    let page = page();
    assert_eq!(calling(r"\bIF\(").len(), 81);
    let nested_if = bounded(r"\bIF\(");
    let nested = function_bodies()
        .into_values()
        .filter(|lines| {
            lines
                .iter()
                .any(|line| common::captures(&nested_if, line).len() > 1)
        })
        .count();
    assert!(nested >= 22, "{nested} functions nest IF");
    page_says(&page, "81 functions call `IF`");
    page_says(&page, "At least 22 functions nest one `IF` inside another");
}

#[test]
fn help_tables_and_the_closing_choose() {
    let validators = set(&["AmortiseλDV", "CorkscrewλDV", "DepreciateλDV"]);
    let helped: BTreeSet<String> = function_bodies()
        .into_keys()
        .filter(|name| !validators.contains(name))
        .collect();
    assert_eq!(
        calling(r"CHOOSE\(\s*(Help\?\s*\+\s*1|Return)\s*,\s*Result\s*,\s*Help\s*[,)]"),
        helped
    );
    assert_eq!(
        calling(r"CHOOSE\(\s*Return\s*,\s*TRUE\s*,\s*Messages\s*,\s*#VALUE!\s*\)"),
        validators
    );
    // Every parameter of a function with a help table has a Required or Optional row.
    for (_, text) in modules() {
        for (name, block) in blocks(&text) {
            if validators.contains(&name) {
                continue;
            }
            let declaration = block.split_once("LET(").map_or(block, |d| d.0);
            for parameter in findall(r"(?m)^\s*\[(\w+)\]", declaration) {
                let row = format!(r#""\s*{parameter}\s*→\s*\((Required|Optional)"#);
                assert!(
                    common::matches(&row, block),
                    "function={name} parameter={parameter}"
                );
            }
        }
    }
    assert_eq!(calling(r"\bINDEX\(").len(), 17);
    page_says(&page(), "use `INDEX` (17 functions)");
}

#[test]
fn offset_indirect_and_npv() {
    assert_eq!(
        calling(r"\bOFFSET\("),
        set(&["RangeToDAλ", "RangeToDAEλ", "RangeToDAUλ"])
    );
    assert_eq!(calling(r"\b(INDIRECT|X?NPV)\("), BTreeSet::new());
}

#[test]
fn rounding_and_the_days_a_month_constant() {
    assert_eq!(
        calling(r"\bROUND(UP|DOWN)?\("),
        set(&[
            "Allocateλ",
            "Amortiseλ",
            "DayCountRateλ",
            "Depreciateλ",
            "DiminishingValueλ",
            "PrimeCostλ",
            "TimelineOffsetλ",
        ])
    );
    assert_eq!(
        calling(r"30\.5"),
        set(&["DayCountRateλ", "Depreciateλ", "TimelineOffsetλ"])
    );
}

#[test]
fn the_embedded_gst_rate_and_the_days_a_year_convention() {
    let page = page();
    assert_eq!(
        calling(r"IF\(ISOMITTED\(Rate\), 0\.1,"),
        set(&["GSTAddλ", "GSTExtractλ"])
    );
    page_says(&page, "fall back to a rate of 0.1 written into the formula");
    assert_eq!(calling(r"DpY,\s*365"), BTreeSet::new());
    page_says(
        &page,
        "`oz.DSIλ` defaults its days to 365 and takes `Days` to override it",
    );
}

#[test]
fn names_merged_cells_and_macros() {
    let parts = parts(WORKBOOK);
    let members = names(WORKBOOK);
    let workbook = part(&parts, "xl/workbook.xml");
    let sheets: Vec<&String> = members
        .iter()
        .filter(|n| common::fullmatch(SHEET, n))
        .collect();
    let merged = sheets
        .iter()
        .filter(|n| part(&parts, n).contains("<mergeCell "))
        .count();
    let local = findall(r#"<definedName name="([^"]+)"[^>]*localSheetId"#, &workbook);
    assert!(!local.is_empty());
    assert_eq!(
        local.into_iter().collect::<BTreeSet<_>>(),
        set(&["_xlnm.Print_Area"])
    );
    assert_eq!((merged, sheets.len()), (48, 49));
    let formulas = merged_formulas();
    let titles = formulas
        .iter()
        .filter(|f| *f == r#"=_xlfn.TEXTAFTER(CELL("filename",A1),"]")"#)
        .count();
    assert_eq!((titles, formulas.len()), (46, 51));
    let page = page();
    page_says(&page, "Of the 51 formulas in merged ranges, 46 are a title");
    page_says(&page, "48 of the workbook's 49 sheets");
    assert!(
        !members
            .iter()
            .any(|n| n.to_lowercase().contains("vbaproject"))
    );
}
