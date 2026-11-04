// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: FSL-1.1-ALv2

//! Guards on help text that was copied from a neighbour.
//!
//! `ozzit verify_signatures` already reads FUNCTION lines, parameter tables and
//! worked examples. These checks cover the leftovers outside those 3: Name
//! Manager comments in `functions.csv`, the About-table prose, and source
//! comments that never make it into a LAMBDA body.
use super::{modules, names, part, parts, read, root, search};
use ozzit::common::{self, Csv};

const COLUMN_FUNCTIONS: [&str; 4] = [
    "oz.CountColsλ",
    "oz.CountColsUλ",
    "oz.CountAColsλ",
    "oz.CountAColsUλ",
];

/// Each indexed function's description; a repeated function keeps its last row.
fn descriptions() -> Vec<(String, String)> {
    let index = Csv::read(&root().join("functions.csv")).unwrap_or_else(|e| panic!("{e}"));
    index
        .rows
        .iter()
        .map(|row| {
            let field = |key| {
                index
                    .get(row, key)
                    .unwrap_or_else(|| panic!("no {key} column"))
                    .to_owned()
            };
            (field("function"), field("description"))
        })
        .collect()
}

fn description(rows: &[(String, String)], name: &str, first: bool) -> String {
    let mut matching = rows.iter().filter(|(function, _)| function == name);
    let found = if first {
        matching.next()
    } else {
        matching.next_back()
    };
    found
        .unwrap_or_else(|| panic!("{name} is not indexed"))
        .1
        .clone()
}

#[test]
fn corkscrew_dv_describes_corkscrew_not_depreciate() {
    let text = read("src/Financial.txt");
    let block = search(
        r"(?s)FUNCTION NAME:\s+CorkscrewλDV\s+DESCRIPTION:\*//\*\*(.*?)\*/",
        &text,
    )
    .expect("CorkscrewλDV description comment missing");
    assert!(block[1].contains("Corkscrewλ"));
    assert!(!block[1].contains("Depreciateλ"));

    let row = description(&descriptions(), "oz.CorkscrewλDV", true);
    assert!(row.contains("Corkscrewλ"));
    assert!(!row.contains("Depreciateλ"));
}

#[test]
fn about_financial_does_not_describe_rolling_avg_as_a_maximum() {
    let text = read("src/Financial.txt");
    let row = search(r#""RollingAvgλ\s+→([^"]+)¶""#, &text)
        .expect("AboutFinancialλ RollingAvg row missing");
    let row = row[1].to_lowercase();
    assert!(!row.contains("maximum"));
    assert!(row.contains("average"));
}

#[test]
fn column_functions_are_not_indexed_as_row_functions() {
    let rows = descriptions();
    for name in COLUMN_FUNCTIONS {
        let description = description(&rows, name, false).to_lowercase();
        assert!(description.contains("column"), "{name}");
        assert!(!description.contains("each row"), "{name}");
    }
}

#[test]
fn source_has_no_coming_soon_placeholder_or_year_2924() {
    for (stem, text) in modules() {
        assert!(!common::matches("(?i)coming soon", &text), "{stem}.txt");
        assert!(!text.contains("2924"), "{stem}.txt");
    }
}

#[test]
fn workbook_no_longer_ships_the_copied_help() {
    let parts = parts("ozzit.xlsx");
    let book = part(&parts, "xl/workbook.xml");
    let sheets: String = names("ozzit.xlsx")
        .iter()
        .filter(|name| name.starts_with("xl/worksheets/"))
        .map(|name| part(&parts, name))
        .collect();
    assert!(!book.contains("Coming Soon"));
    assert!(!sheets.contains("Coming Soon"));
    assert!(!book.contains(
        r#"name="oz.CorkscrewλDV" comment="Provides extended data validation of arguments for Depreciateλ.""#
    ));
    assert!(book.contains(
        r#"name="oz.CorkscrewλDV" comment="Provides extended data validation of arguments for Corkscrewλ.""#
    ));
    assert!(!book.contains("RollingAvgλ        →Finds the maximum value the n preceding values"));
    for name in COLUMN_FUNCTIONS {
        let pattern = format!(
            r#"name="{}" comment="[^"]*each row"#,
            fancy_regex::escape(name)
        );
        assert!(!common::matches(&pattern, &book), "{name}");
    }
}
