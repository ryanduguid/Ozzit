// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! Independent arithmetic and formula pins for the ATO benchmark and rate helpers.
//!
//! The arithmetic lives in Excel LAMBDAs, so it cannot run here. The pins hold
//! each published procedure line, and the mirrors below restate the ATO rules
//! from the ATO's "How we calculate benchmark ratios" page (last updated 16 March
//! 2026, read 19 October 2026). `cross_checks` compares those mirrors with values
//! a native Excel run returned.
use super::{almost_equal, findall, json, read, spaced};
use std::collections::BTreeSet;

/// Other sales, unless nil, blank or under half of total business income.
pub fn ato_turnover(other_sales: Option<i64>, total_business_income: i64) -> i64 {
    match other_sales {
        Some(sales) if sales != 0 && 2 * sales >= total_business_income => sales,
        _ => total_business_income,
    }
}

/// Wages (the larger of the return's and W1) plus contractors less associates.
pub fn ato_labour(
    salary_wages: i64,
    contractors: i64,
    turnover: i64,
    associates: i64,
    w1: i64,
) -> f64 {
    (salary_wages.max(w1) + contractors - associates) as f64 / turnover as f64
}

/// The last rate whose effective date is on or before the date; None before the first.
pub fn rate_at_date(effective_dates: &[i64], rates: &[f64], date: i64) -> Option<f64> {
    effective_dates
        .iter()
        .zip(rates)
        .filter(|(start, _)| **start <= date)
        .map(|(_, rate)| *rate)
        .next_back()
}

// The mirrors against amounts worked by hand in the comments beside them.

#[test]
fn turnover_keeps_other_sales_at_half_or_more() {
    assert_eq!(ato_turnover(Some(480_000), 500_000), 480_000);
    assert_eq!(ato_turnover(Some(250_000), 500_000), 250_000); // exactly half
}

#[test]
fn turnover_falls_back_below_half_or_when_nil() {
    assert_eq!(ato_turnover(Some(249_999), 500_000), 500_000);
    assert_eq!(ato_turnover(Some(0), 500_000), 500_000);
    assert_eq!(ato_turnover(None, 500_000), 500_000);
}

#[test]
fn labour_takes_the_larger_wage_figure() {
    // (95,000 + 20,000 - 10,000) / 480,000 = 105,000 / 480,000
    assert!(almost_equal(
        ato_labour(90_000, 20_000, 480_000, 10_000, 95_000),
        0.21875,
        7
    ));
    // W1 below the return's wages: (90,000 + 20,000 - 10,000) / 480,000
    assert!(almost_equal(
        ato_labour(90_000, 20_000, 480_000, 10_000, 85_000),
        100.0 / 480.0,
        7
    ));
}

#[test]
fn rate_at_date_applies_each_rate_until_the_next() {
    let (starts, rates) = ([1, 10, 20], [0.04, 0.045, 0.05]);
    assert_eq!(rate_at_date(&starts, &rates, 0), None);
    assert_eq!(rate_at_date(&starts, &rates, 10), Some(0.045));
    assert_eq!(rate_at_date(&starts, &rates, 19), Some(0.045));
    assert_eq!(rate_at_date(&starts, &rates, 99), Some(0.05));
}

// Each LAMBDA still spells the rule its mirror restates; the pins fail on a change.

const PINS: [(&str, &[&str]); 6] = [
    (
        "ATOTurnoverλ",
        &[
            r#"Sales, IF(OtherSales = "", 0, IF(ISNUMBER(OtherSales), OtherSales, #VALUE!)),"#,
            "Result, IF((Sales = 0) + (Sales < Income / 2), Income, Sales),",
        ],
    ),
    ("ATOBenchmarkRatioλ", &["Result, Amount / Turnover,"]),
    (
        "ATOTotalExpensesRatioλ",
        &["Result, (TotalExpenses - Associates) / Turnover,"],
    ),
    (
        "ATOCostOfSalesRatioλ",
        &["Result, (CostOfSales - Wages) / Turnover,"],
    ),
    (
        "ATOLabourRatioλ",
        &[
            "Wages, IF(Activity > SalaryWages, Activity, SalaryWages),",
            "Result, (Wages + Contractors - Associates) / Turnover,",
        ],
    ),
    (
        "RateAtDateλ",
        &[
            "Complete?, AND(Count = ROWS(Values), ISNUMBER(EffectiveDates), ISNUMBER(Rates)),",
            "Steps, IF(Count = 1, 1, DROP(Starts, 1) - DROP(Starts, -1)),",
            "Result, IF(ISNUMBER(Dates), XLOOKUP(Dates, Starts, Values, NA(), -1, 2), #VALUE!),",
            "Return, IF(Help?, 2, IF(NOT(Complete?), 3, IF(Unsorted?, 4, IF(Repeated?, 5, 1)))),",
        ],
    ),
];

/// The published (module, name, about, block) rows of the pass's source table.
fn functions() -> Vec<[String; 4]> {
    let table = json(&read(
        "tools/ozzit/src/data/postbuild/ato_benchmarks_rate_lookup.json",
    ));
    table["functions"]
        .as_array()
        .expect("a list of functions")
        .iter()
        .map(|row| {
            let row = row.as_array().expect("a function row");
            std::array::from_fn(|i| row[i].as_str().expect("a text field").to_owned())
        })
        .collect()
}

#[test]
fn every_function_is_pinned() {
    let pinned: BTreeSet<&str> = PINS.iter().map(|(name, _)| *name).collect();
    let published: Vec<String> = functions()
        .into_iter()
        .map(|[_, name, _, _]| name)
        .collect();
    assert_eq!(published.len(), pinned.len(), "{published:?}");
    assert_eq!(
        published
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        pinned
    );
}

#[test]
fn each_published_procedure_is_the_one_the_mirror_restates() {
    let functions = functions();
    for (name, lines) in PINS {
        let [.., block] = functions
            .iter()
            .rfind(|row| row[1] == name)
            .unwrap_or_else(|| panic!("{name} is not published"));
        for line in lines {
            assert!(
                spaced(block).contains(&spaced(line)),
                "name={name} line={line}"
            );
        }
    }
}

#[test]
fn the_committed_modules_carry_the_published_blocks() {
    for [module, name, _, block] in functions() {
        assert!(
            read(&format!("src/{module}.txt")).contains(&block),
            "name={name}"
        );
    }
}

#[test]
fn no_rate_table_ships_in_the_help() {
    // llms.txt says the library carries no rate card: the only rates in the help
    // are the synthetic example's.
    let sources: Vec<String> = functions()
        .into_iter()
        .filter(|row| row[1] == "RateAtDateλ")
        .map(|[.., source]| source)
        .collect();
    let [source] = sources.as_slice() else {
        panic!("{} RateAtDateλ rows", sources.len())
    };
    let help_rates: BTreeSet<String> = findall(r"0\.0\d+", source).into_iter().collect();
    assert_eq!(
        help_rates.into_iter().collect::<Vec<_>>(),
        ["0.04", "0.045", "0.05"]
    );
}
