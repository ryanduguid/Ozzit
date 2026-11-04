// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! Guards on help that promised something the formula did not do.
//!
//! `ozzit verify_signatures` compares a help's FUNCTION line, parameter table and
//! worked examples with the LAMBDA's own declaration. None of that reads what an
//! example claims the answer is, or whether the body applies the default the
//! table names, so each of these shipped with the help and the code
//! contradicting each other and every gate passing.
//!
//! Each check reads every store that states the thing. `src/` is what a reader
//! diffs and imports; the defined name in `ozzit.xlsx` is what Excel actually
//! evaluates, and the 2 have drifted apart before. A claim spilled onto a
//! demonstration worksheet is checked in the cell caching that spill as well. A
//! claim typed into a label or description column is checked in the shared
//! string table, because such a cell has no formula and sits under no spill
//! anchor: Excel never revisits it, so correcting the defined name alone leaves
//! the sentence a reader sees unchanged.
use super::{defined_name, findall, modules, names, part, parts, piece, read};
use ozzit::common;
use std::collections::{BTreeSet, HashMap};

/// A function name written out on its own, the way a label column carries one.
const NAME: &str = r"(?<![A-Za-z0-9_.])(?:oz\.)?([A-Za-z_][A-Za-z0-9_]*λ[A-Za-z0-9_]*)";

/// Every entry of the shared string table, as the text a cell displays.
fn shared_strings(part: &str) -> Vec<String> {
    findall(r"(?s)<si>(.*?)</si>", part)
        .iter()
        .map(|item| {
            findall(r"(?s)<t[^>]*>(.*?)</t>", item)
                .iter()
                .map(|run| common::unescape(run).unwrap_or_else(|e| panic!("{e}")))
                .collect()
        })
        .collect()
}

struct Stores {
    book: String,
    sheets: String,
    strings: Vec<String>,
    modules: HashMap<String, String>,
}

impl Stores {
    fn new() -> Self {
        let parts = parts("ozzit.xlsx");
        Self {
            book: part(&parts, "xl/workbook.xml"),
            sheets: names("ozzit.xlsx")
                .iter()
                .filter(|name| name.starts_with("xl/worksheets/"))
                .map(|name| part(&parts, name))
                .collect(),
            strings: shared_strings(&part(&parts, "xl/sharedStrings.xml")),
            modules: modules().into_iter().collect(),
        }
    }

    fn module(&self, stem: &str) -> &str {
        &self.modules[stem]
    }

    fn shipped(&self, name: &str) -> String {
        defined_name(&self.book, name).unwrap_or_else(|e| panic!("{e}"))
    }

    /// The text of a declaration up to its closing `\n);`.
    fn declaration(&self, module: &str, name: &str) -> &str {
        let body = piece(self.module(module), &format!("{name} = LAMBDA("), 1);
        piece(body, "\n);", 0)
    }
}

#[test]
fn is_between_helpers_apply_the_inclusive_default_they_document() {
    // Both help tables call Inclusive optional with TRUE as the default. An
    // omitted LAMBDA argument evaluates as an empty value, so without this
    // binding IF( Inclusive, ...) takes the exclusive branch and the help's own
    // =IsBetweenEλ(2, 2, 4) example returns FALSE where it prints TRUE.
    let stores = Stores::new();
    let binding = "Inclusive,      IF( ISLOGICAL( Inclusive), Inclusive, TRUE),";
    let stored = "_xlpm.Inclusive, IF( ISLOGICAL( _xlpm.Inclusive), _xlpm.Inclusive, TRUE)";
    for (module, name) in [("Essentials", "IsBetweenEλ"), ("Utilities", "IsBetweenUλ")] {
        let declaration = stores.declaration(module, name);
        assert!(
            declaration.contains("(Optional) If set to TRUE (default)"),
            "function={name}"
        );
        let bound = declaration
            .find(binding)
            .unwrap_or_else(|| panic!("function={name}: no binding"));
        let read = declaration
            .find("Result,         IF( Inclusive,")
            .unwrap_or_else(|| panic!("function={name}: no result"));
        assert!(
            bound < read,
            "{name} must bind Inclusive before it reads it"
        );
        assert!(
            stores.shipped(&format!("oz.{name}")).contains(stored),
            "function={name}"
        );
    }
}

#[test]
fn sum_periods_help_says_which_dates_fall_outside_the_periods() {
    // The calculation drops a date before the first period start and gives one after
    // the last start to the last period. Up to v3.5.0 the help said neither.
    let stores = Stores::new();
    let note = "A date before the first period start is left out; one on or after the last start goes to the last period.";
    assert!(
        stores
            .declaration("Financial", "SumPeriodsλ")
            .contains(note)
    );
    assert!(stores.shipped("oz.SumPeriodsλ").contains(note));
}

#[test]
fn movement_example_passes_the_two_arguments_it_declares() {
    // Movementλ = LAMBDA([BeginningValues], [Values], ...). Excel rejects a
    // LAMBDA call carrying more arguments than the function has parameters,
    // so the printed example could not be evaluated at all.
    let stores = Stores::new();
    let stale = "=oz.Movementλ(,{100,110,130,100}, 4)";
    let current = "=oz.Movementλ(,{100,110,130,100})";
    assert!(stores.module("Financial").contains(current));
    assert!(!stores.module("Financial").contains(stale));
    assert!(stores.shipped("oz.Movementλ").contains(current));
    assert!(!stores.book.contains(stale));
    assert!(stores.sheets.contains(&format!("<v>{current}</v>")));
    assert!(!stores.sheets.contains(&format!("<v>{stale}</v>")));
}

#[test]
fn reversal_example_prints_the_row_the_function_returns() {
    // Column 1 is the opening value and every later column negates the previous
    // input, which is what reversing "in the next period" means. The old example
    // printed the negated input instead, one period early.
    let stores = Stores::new();
    let stale = r#""-100,-110,-130 →=oz.Reversalλ( , {100,110,130})""#;
    let current = r#""0,-100,-110    →=oz.Reversalλ( , {100,110,130})""#;
    assert!(stores.module("Financial").contains(current));
    assert!(!stores.module("Financial").contains(stale));
    assert!(stores.shipped("oz.Reversalλ").contains(current));
    assert!(stores.sheets.contains("<v>0,-100,-110</v>"));
    assert!(!stores.sheets.contains("<v>-100,-110,-130</v>"));
}

#[test]
fn period_diff_example_prints_the_number_its_formula_returns() {
    // 1 Jan to 15 Apr spans Q1 and Q2: Months = 4, Periods = 1, PartialPeriod
    // is TRUE, so Periods + OR(...) is 2. The inclusive count is deliberate;
    // Depreciateλ relies on it for the salvage period.
    let stores = Stores::new();
    let example = r#"→=oz.PeriodDiffλ(""2026-01-01"", ""2026-04-15"", ""Q"")"#;
    for (store, text) in [
        ("src", stores.module("Financial")),
        ("workbook", stores.book.as_str()),
    ] {
        assert!(
            text.contains(&format!("\"2              {example}")),
            "store={store}"
        );
        assert!(
            !text.contains(&format!("\"1              {example}")),
            "store={store}"
        );
    }
}

#[test]
fn about_dates_lists_no_diagnostic_function_the_library_never_shipped() {
    // The block told readers to insert 'DV' and type CountDOWλDV( Start, End, 1).
    // The library declares 3 λDV functions, all in Financial, and no release
    // ever defined a Dates one, so every call it named returned #NAME?.
    let stores = Stores::new();
    for text in [
        stores.module("Dates"),
        stores.shipped("oz.AboutDatesλ").as_str(),
    ] {
        assert!(!text.contains("DIAGNOSTICS"));
        assert!(!text.contains("λDV"));
        assert!(text.contains("FinancialYearλ"));
    }
}

#[test]
fn about_ratios_names_the_two_ratios_as_the_library_declares_them() {
    // The same 2 rows are typed a second time into the label column of the
    // oz.FinancialRatios sheet, in cells no formula feeds, so both stores and
    // the shared string table have to agree.
    let stores = Stores::new();
    let strings = stores.strings.join("\n");
    for (store, text) in [
        ("src", stores.module("Ratios")),
        ("definedName", stores.shipped("oz.AboutRatiosλ").as_str()),
        ("worksheets", stores.sheets.as_str()),
        ("sharedStrings", strings.as_str()),
    ] {
        assert!(!text.contains("DSIRatioλ"), "store={store}");
        assert!(!text.contains("DividendPayoutRatioλ"), "store={store}");
    }
    assert!(stores.strings.iter().any(|s| s == "DSIλ"));
    assert!(stores.strings.iter().any(|s| s == "DPRλ"));
}

#[test]
fn no_static_cell_names_a_function_the_library_never_declared() {
    // A label column is a list of functions to go and type, and every cell in
    // one is a static literal: no formula, no covering spill anchor, so Excel
    // never refreshes it. ozzit verify_signatures makes this check against src/;
    // nothing made it against the cells, which is how the pre-rename Aboutλ,
    // DSIRatioλ and DividendPayoutRatioλ outlived the names they came from.
    let stores = Stores::new();
    let declared: BTreeSet<String> = stores
        .modules
        .values()
        .flat_map(|text| findall(r"(?m)^([A-Za-z0-9_]*λ[A-Za-z0-9_]*)\s*=", text))
        .collect();
    assert!(declared.len() > 100, "no declarations were read");
    let unknown: BTreeSet<(String, &String)> = stores
        .strings
        .iter()
        .flat_map(|text| {
            findall(NAME, text)
                .into_iter()
                .map(move |name| (name, text))
        })
        .filter(|(name, _)| !declared.contains(name))
        .collect();
    assert_eq!(unknown, BTreeSet::new());
}

#[test]
fn static_signature_cells_match_the_declaration_they_describe() {
    // oz.IsOccurrenceDateλ's demonstration sheet writes the signature out in a
    // cell of its own. It listed a fifth argument, [Diagnostics], that the
    // LAMBDA has never declared and its own FUNCTION line has never shown.
    let stores = Stores::new();
    let signatures: Vec<&String> = stores
        .strings
        .iter()
        .filter(|text| text.starts_with("IsOccurrenceDateλ("))
        .collect();
    assert_eq!(
        signatures,
        ["IsOccurrenceDateλ(Dates, FirstOccurrence, [LastOccurrence], [Repeats])"]
    );
    let declaration = piece(stores.module("Dates"), "IsOccurrenceDateλ = LAMBDA(", 1);
    assert!(!piece(declaration, "//  Help", 0).contains("Diagnostics"));
    assert!(declaration.contains(&format!("→{}¶", signatures[0])));
}

#[test]
fn corkscrew_dv_reports_its_findings_without_a_sixth_argument() {
    // Errors2Show gated both message arrays on an undocumented Diagnostics
    // argument. Omitted, it evaluated as 0, so a real problem fell through to
    // CHOOSE's third branch and the caller saw #VALUE! rather than the advice.
    // AmortiseλDV and DepreciateλDV never carried the multiplier.
    let stores = Stores::new();
    let stored = stores.shipped("oz.CorkscrewλDV");
    assert!(!stored.contains("Diagnostics"));
    assert!(stored.contains("_xlpm.Errors2Show, _xlfn.VSTACK(_xlpm.ErrorsInArgs, _xlpm.DVErrors)"));
    let declaration = stores.declaration("Financial", "CorkscrewλDV");
    assert!(!declaration.contains("Diagnostics"));
    assert!(declaration.contains("Errors2Show,    VSTACK( ErrorsInArgs, DVErrors)"));
}

#[test]
fn sum_depreciate_is_not_described_as_totalling_book_value() {
    // The block's Book Value row falls to the SWITCH default and stays 0.
    // Totalling a balance row would be meaningless, so the code is right.
    // The same sentence is typed into 2 static cells, the TOC row for the
    // function and the heading of its own demonstration sheet, and both read
    // the one shared string.
    let stores = Stores::new();
    let claim = "CAPEX, Depreciation, Book Value, Salvage Value";
    let corrected = "CAPEX, Depreciation, Salvage Value, and Disposal costs";
    assert!(!stores.module("Financial").contains(claim));
    assert!(!stores.book.contains(claim));
    assert!(!stores.sheets.contains(claim));
    assert!(!read("README.md").contains(claim));
    assert!(stores.shipped("oz.AboutFinancialλ").contains(corrected));
    let described: Vec<&String> = stores
        .strings
        .iter()
        .filter(|text| text.contains("Disposal costs in Depreciateλ"))
        .collect();
    assert_eq!(
        described,
        [&format!(
            "Create row totals for {corrected} in Depreciateλ results"
        )]
    );
}

// The shared helper on input that takes its other paths, from test_test_helper_paths.py.

const BOOK: &str = r#"<definedName name="oz.Aλ" comment="c">=LAMBDA(x, x)</definedName>"#;

#[test]
fn defined_name_returns_its_part_of_a_present_name() {
    assert_eq!(defined_name(BOOK, "oz.Aλ").unwrap(), "=LAMBDA(x, x)");
}

#[test]
fn defined_name_fails_on_a_missing_name() {
    assert_eq!(
        defined_name(BOOK, "oz.Bλ").unwrap_err(),
        "oz.Bλ missing from workbook.xml"
    );
}
