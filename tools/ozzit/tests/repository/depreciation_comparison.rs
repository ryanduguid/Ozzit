// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! The arithmetic in `docs/depreciation-comparison.md`, checked independently.
//!
//! The figures in that document are recomputed here from the two conventions it
//! describes, in exact cents, and compared with the numbers written in its
//! tables. The schedules are not read out of the workbook and no Excel
//! recalculation is claimed or implied: this proves the document's arithmetic is
//! right, and the document says so.
use super::{flat, json, read, sha256};
use ozzit::common;
use std::fmt;

const DOCUMENT: &str = "docs/depreciation-comparison.md";
const COST: Cents = Cents(12_000_000);
const LIFE: i64 = 5;
const TABLE_ROW: &str = r"(?m)^\| \d+ \| ((?:\d|,)+\.\d{2}) \| ((?:\d|,)+\.\d{2}) \|$";

/// An exact amount of money, shown as dollars and cents.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Cents(i64);

impl fmt::Debug for Cents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sign}{}.{:02}", self.0.abs() / 100, self.0.abs() % 100)
    }
}

impl Cents {
    fn parse(text: &str) -> Self {
        let digits = text.replace([',', '.'], "");
        Self(
            digits
                .parse()
                .unwrap_or_else(|_| panic!("{text} is not an amount")),
        )
    }

    /// This amount times `numerator / denominator`, rounded half up to the cent.
    fn times(self, numerator: i64, denominator: i64) -> Self {
        let product = self.0 * numerator;
        let rounded = (2 * product.abs() + denominator) / (2 * denominator);
        Self(rounded * product.signum())
    }
}

/// Straight line over `life` periods, the shape oz.PrimeCostλ produces.
fn prime_cost(cost: Cents, life: i64) -> Vec<Cents> {
    vec![cost.times(1, life); life as usize]
}

/// 200% of the straight-line rate, residual written off in the last period.
///
/// The final period is the one that matters: the function reconciles its
/// schedule to cost, so the closing balance is nil rather than a stub.
fn diminishing_value(cost: Cents, life: i64) -> Vec<Cents> {
    let mut remaining = cost;
    (1..=life)
        .map(|period| {
            let charge = if period == life {
                remaining
            } else {
                remaining.times(2, life)
            };
            remaining = Cents(remaining.0 - charge.0);
            charge
        })
        .collect()
}

/// The final-period charge and leftover if the factor kept being applied.
fn unwritten_residual(cost: Cents, life: i64) -> (Cents, Cents) {
    let (mut charge, mut remaining) = (Cents(0), cost);
    for _ in 0..life {
        charge = remaining.times(2, life);
        remaining = Cents(remaining.0 - charge.0);
    }
    (charge, remaining)
}

/// The first `LIFE` rows' given money column under the first heading containing `heading`.
fn table_column(heading: &str, column: usize) -> Vec<Cents> {
    let text = read(DOCUMENT);
    let start = text
        .find(heading)
        .unwrap_or_else(|| panic!("{heading:?} not found"));
    common::captures(TABLE_ROW, &text[start..])
        .iter()
        .take(LIFE as usize)
        .map(|row| Cents::parse(&row[column]))
        .collect()
}

#[test]
fn the_document_claims_no_excel_evidence_and_adds_nothing() {
    let text = read(DOCUMENT);
    assert!(text.contains("NOT_RUN"));
    assert!(text.contains("no recalculation is claimed"));
    assert!(
        flat(&text)
            .contains("adds a function or introduces an add-in, a macro or a remote formula")
    );
}

#[test]
fn the_prime_cost_table_matches_independent_arithmetic() {
    let expected = prime_cost(COST, LIFE);
    assert_eq!(expected, vec![Cents(2_400_000); 5]);
    assert_eq!(table_column("### Prime cost", 1), expected);
}

#[test]
fn the_prime_cost_balances_run_down_to_nil() {
    let balances = table_column("### Prime cost", 2);
    let mut remaining = COST;
    for (charge, shown) in prime_cost(COST, LIFE).into_iter().zip(&balances) {
        remaining = Cents(remaining.0 - charge.0);
        assert_eq!(*shown, remaining);
    }
    assert_eq!(balances.last(), Some(&Cents(0)));
}

#[test]
fn the_diminishing_value_table_matches_independent_arithmetic() {
    let expected = diminishing_value(COST, LIFE);
    assert_eq!(
        expected,
        [4_800_000, 2_880_000, 1_728_000, 1_036_800, 1_555_200].map(Cents)
    );
    assert_eq!(table_column("### Diminishing value", 1), expected);
}

#[test]
fn the_diminishing_value_schedule_reconciles_to_cost() {
    let total: i64 = diminishing_value(COST, LIFE).iter().map(|c| c.0).sum();
    assert_eq!(Cents(total), COST);
    assert_eq!(
        table_column("### Diminishing value", 2).last(),
        Some(&Cents(0))
    );
}

/// The figures the document uses to name the convention difference.
#[test]
fn the_documented_alternative_final_period_is_correct() {
    let (charge, residual) = unwritten_residual(COST, LIFE);
    assert_eq!(charge, Cents(622_080));
    assert_eq!(residual, Cents(933_120));
    let text = read(DOCUMENT);
    assert!(text.contains("6,220.80"));
    assert!(text.contains("9,331.20"));
}

/// A surviving balance is unallocated depreciable amount, not a choice.
///
/// The document used to say neither treatment was wrong. On the stated facts,
/// a five-year life with no residual value, AASB 116 leaves the 9,331.20
/// nowhere to go, so saying so is the point of the comparison.
#[test]
fn the_unwritten_balance_is_not_called_acceptable() {
    let text = read(DOCUMENT);
    assert!(!text.contains("Neither is wrong"));
    assert!(text.contains("no residual value"));
    assert!(text.contains("depreciable amount"));
}

/// AASB 116 changes the method only where the consumption pattern changed.
///
/// The document once listed a switch to straight line beside a residual value
/// and a longer life as ways to account for the closing balance. A method
/// changed to force a balance to nil, with the pattern unchanged, is not a
/// permitted change.
#[test]
fn a_method_change_is_not_offered_as_a_way_to_clear_the_balance() {
    let text = read(DOCUMENT);
    assert!(!text.contains("or a switch to straight line"));
    assert!(text.contains("changed significantly"));
    assert!(text.contains("change in estimate"));
}

#[test]
fn the_documented_movement_closes() {
    let (opening, additions, depreciation) = (COST, Cents(0), Cents(2_400_000));
    assert_eq!(
        Cents(opening.0 + additions.0 - depreciation.0),
        Cents(9_600_000)
    );
    assert!(read(DOCUMENT).contains("120,000.00 + 0.00 - 24,000.00 = 96,000.00"));
}

#[test]
fn the_document_keeps_accounting_and_tax_apart() {
    let text = flat(&read(DOCUMENT));
    assert!(text.contains("It is not a deduction under ITAA 1997 Division 40"));
    assert!(text.contains("an AASB 116 carrying amount is not one either"));
}

/// This example must not have edited the shipped authority.
#[test]
fn the_committed_workbook_is_untouched_by_this_document() {
    let base = json(&read("release/workbook-base.json"));
    let path = base["path"].as_str().expect("a workbook path");
    assert_eq!(sha256(path), base["sha256"]);
}
