// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! Cross-check fixtures: synthetic cases with independently computed expected values.
//!
//! Each case is an Excel formula and the grid it should return, worked out here
//! from the rule the function implements rather than from the LAMBDA's own steps:
//! present values from the closed-form annuity factor, loan repayments from the
//! level-payment formula, the ATO ratios and rate lookup from the mirrors in
//! `benchmark_arithmetic`, and the variance and financial-year period functions
//! from their definitions with calendar dates.
//!
//! The lease scenarios follow the patterns of the IFRS 16 illustrative examples
//! the AASB reproduces (Example 13, initial measurement and a change in lease
//! term at a revised rate; Example 14, a payment linked to an index remeasured at
//! an unchanged rate). Every input below is invented for these fixtures; none is
//! taken from those examples.
//!
//! The loan cases mirror what the Moneysmart mortgage calculator and the
//! AussieCalcs Division 7A calculator compute, with inputs of our own. Neither
//! site's terms were found, so nothing is taken from them; their results are not
//! evidence here until a person enters the same inputs by hand and records the
//! outcome.
//!
//! `tools/tests/fixtures/cross-checks-cases.json` is this module's `cases()`, the
//! input `tools/excel_eval_formulas.ps1` reads. `cross-checks-results.json` is
//! what that native run returned against the committed workbook, retained by
//! `tools/excel_eval_formulas_verified.ps1`. Regenerate the cases with
//! `cargo test --manifest-path tools/ozzit/Cargo.toml --test repository
//! write_the_cases -- --ignored`, then rerun the native evaluation.
//!
//! Floats keep Python's types and text: an amount Python held as an integer is
//! written as one, and powers go through `powf`, the platform `pow` Python's
//! `**` calls.
use super::benchmark_arithmetic::{ato_labour, ato_turnover, rate_at_date};
use super::{almost_equal, read, read_sig, root, sha256};
use chrono::{Datelike, NaiveDate};
use ozzit::common;
use ozzit::py::float_repr as r;
use ozzit::pyjson::{self, Json};
use std::collections::BTreeSet;
use std::fmt::Display;

const CASES: &str = "tools/tests/fixtures/cross-checks-cases.json";
const RESULTS: &str = "tools/tests/fixtures/cross-checks-results.json";

// Excel error values as excel_eval_formulas.ps1 writes them.
const NA: &str = "#ERR:-2146826246";
const VALUE: &str = "#ERR:-2146826273";
const DIV0: &str = "#ERR:-2146826281";

type Case = (String, String, Json);

fn int(value: i64) -> Json {
    Json::Int(value.into())
}

fn float(value: f64) -> Json {
    Json::Float(value)
}

fn text(value: &str) -> Json {
    Json::Str(value.to_owned())
}

/// `[[a, b, ...]]`.
fn row(values: impl IntoIterator<Item = Json>) -> Json {
    Json::Array(vec![Json::Array(values.into_iter().collect())])
}

/// `[[a], [b], ...]`.
fn column(values: impl IntoIterator<Item = Json>) -> Json {
    Json::Array(
        values
            .into_iter()
            .map(|value| Json::Array(vec![value]))
            .collect(),
    )
}

fn one(value: Json) -> Json {
    row([value])
}

fn case(id: impl Into<String>, formula: impl Into<String>, expected: Json) -> Case {
    (id.into(), formula.into(), expected)
}

/// A JSON number as a float, the way Python's arithmetic reads an int or a float.
fn number(value: &Json) -> f64 {
    match value {
        Json::Float(value) => *value,
        Json::Int(value) => i64::try_from(value).expect("a small integer") as f64,
        other => panic!("{} is not a number", other.repr()),
    }
}

/// Present value of 1 a period for this many periods, paid in arrears.
fn annuity(rate: f64, periods: i64) -> f64 {
    if rate == 0.0 {
        periods as f64
    } else {
        (1.0 - (1.0 + rate).powf(-periods as f64)) / rate
    }
}

fn level_payment(principal: f64, rate: f64, periods: i64) -> f64 {
    principal / annuity(rate, periods)
}

/// Opening, payment, interest and closing rows for payments that follow interest.
fn unwind(mut opening: f64, payments: &[i64], rate: f64) -> Json {
    let mut rows: [Vec<Json>; 4] = Default::default();
    for &payment in payments {
        let interest = opening * rate;
        let closing = opening + interest - payment as f64;
        let values = [
            float(opening),
            int(payment),
            float(interest),
            float(closing),
        ];
        for (row, value) in rows.iter_mut().zip(values) {
            row.push(value);
        }
        opening = closing;
    }
    Json::Array(rows.into_iter().map(Json::Array).collect())
}

fn straight_line(cost: f64, periods: i64) -> Json {
    let charge = cost / periods as f64;
    Json::Array(vec![
        Json::Array(
            (0..periods)
                .map(|k| float(cost - charge * k as f64))
                .collect(),
        ),
        Json::Array(vec![float(charge); periods as usize]),
        Json::Array(
            (0..periods)
                .map(|k| float(cost - charge * (k + 1) as f64))
                .collect(),
        ),
    ])
}

/// AASB 16 para 39: the adjustment goes to the asset until it is nil, then to profit.
fn remeasure(
    revised_liability: f64,
    paid_now: f64,
    carrying_liability: f64,
    carrying_rou: f64,
) -> [f64; 4] {
    let adjustment = revised_liability + paid_now - carrying_liability;
    let asset = carrying_rou + adjustment;
    // Python's max(asset, 0.0) and min(asset, 0.0): the first argument wins a tie.
    let kept = if 0.0 > asset { 0.0 } else { asset };
    let excess = if 0.0 < asset { 0.0 } else { asset };
    [revised_liability, adjustment, kept, excess]
}

/// The Excel date serial: days since 30 December 1899.
fn serial(year: i32, month: u32, day: u32) -> i64 {
    (date(year, month, day) - date(1899, 12, 30)).num_days()
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("a valid date")
}

fn level_row(amount: impl Display, count: i64) -> String {
    format!("SEQUENCE(1,{count},{amount},0)")
}

fn lease_cases() -> Vec<Case> {
    [
        lease_l1_l2_cases(),
        lease_l3_cases(),
        lease_remeasure_cases(),
    ]
    .concat()
}

fn lease_l1_l2_cases() -> Vec<Case> {
    // L1: 36 monthly payments of 2,400 in arrears at 0.5% a month.
    let (rate, pay, n) = (0.005, 2400, 36);
    let opening = pay as f64 * annuity(rate, n);
    let mut cases = vec![
        case(
            "lease_l1_liability",
            format!("=oz.LeaseLiabilityλ({},{})", level_row(pay, n), r(rate)),
            one(float(opening)),
        ),
        case(
            "lease_l1_schedule",
            format!("=oz.LeaseScheduleλ({},{})", level_row(pay, n), r(rate)),
            unwind(opening, &[pay; 36], rate),
        ),
    ];

    // L2: 12 quarterly payments of 7,500 in advance at 7% a year effective. The asset
    // adds the commencement payment and 3,000 of initial direct costs and deducts a
    // 1,200 incentive (the Example 13 part 1 pattern), then runs straight-line.
    let (rate_text, rate) = ("1.07^(1/4)-1", 1.07_f64.powf(0.25) - 1.0);
    let (pay, n) = (7500, 12);
    let liability = pay as f64 * annuity(rate, n - 1);
    let cost = liability + pay as f64 + 3000.0 - 1200.0;
    let l2 = format!("oz.LeaseLiabilityλ({},{rate_text},TRUE)", level_row(pay, n));
    cases.extend([
        case(
            "lease_l2_liability_in_advance",
            format!("={l2}"),
            one(float(liability)),
        ),
        case(
            "lease_l2_schedule_in_advance",
            format!("=oz.LeaseScheduleλ({},{rate_text},TRUE)", level_row(pay, n)),
            unwind(liability, &[pay; 11], rate),
        ),
        case(
            "lease_l2_rou",
            format!("=oz.ROUScheduleλ({l2}+{pay}+3000-1200,{n})"),
            straight_line(cost, n),
        ),
    ]);
    cases
}

fn lease_l3_cases() -> Vec<Case> {
    // L3: 7 annual payments of 18,000 in advance at 4.5%. At the start of year 4, before
    // that year's payment, an index moves from 112 to 118.72, so each remaining payment
    // becomes 19,080; the rate is unchanged (the Example 14 pattern).
    let (rate, pay, n) = (0.045, 18000, 7);
    let initial = pay as f64 * annuity(rate, n - 1);
    let cost = initial + pay as f64;
    let carrying = pay as f64 + pay as f64 * annuity(rate, 3); // 4 payments left, one due today
    let rou = cost * 4.0 / 7.0;
    let revised_pay = pay as f64 * 118.72 / 112.0;
    vec![
        case(
            "lease_l3_index_remeasure",
            format!(
                "=oz.LeaseRemeasureλ({},{},{},{},TRUE)",
                level_row(r(revised_pay), 4),
                r(rate),
                r(carrying),
                r(rou)
            ),
            column(
                remeasure(revised_pay * annuity(rate, 3), revised_pay, carrying, rou).map(float),
            ),
        ),
        case(
            "lease_l3_carrying_matches_schedule",
            format!(
                "=INDEX(oz.LeaseScheduleλ({},{},TRUE),4,3)",
                level_row(pay, n),
                r(rate)
            ),
            one(float(carrying - pay as f64)),
        ),
    ]
}

fn lease_remeasure_cases() -> Vec<Case> {
    // L4: 48 monthly payments of 3,100 in arrears at 0.45% a month. After 24 payments the
    // term extends by 12 months at 3,250 and the rate is revised to 0.52% (the Example 13
    // part 2 pattern); the asset then runs straight-line over the 36 months left.
    let (rate, pay, n) = (0.0045, 3100, 48);
    let cost = pay as f64 * annuity(rate, n);
    let carrying = pay as f64 * annuity(rate, 24);
    let rou = cost * 24.0 / 48.0;
    let new_rate = 0.0052;
    let revised = pay as f64 * annuity(new_rate, 24)
        + 3250.0 * annuity(new_rate, 12) / (1.0 + new_rate).powf(24.0);
    let l4 = remeasure(revised, 0.0, carrying, rou);
    let revised_payments = format!("HSTACK({},{})", level_row(pay, 24), level_row(3250, 12));
    // L5: payments fall far enough to exhaust the asset: 3 annual payments of 10,000 at
    // 6% against a carried liability of 50,000 and asset of 4,000.
    let l5 = 10000.0 * annuity(0.06, 3);
    vec![
        case(
            "lease_l4_term_remeasure",
            format!(
                "=oz.LeaseRemeasureλ({revised_payments},{},{},{})",
                r(new_rate),
                r(carrying),
                r(rou)
            ),
            column(l4.map(float)),
        ),
        case(
            "lease_l4_carrying_matches_schedule",
            format!(
                "=INDEX(oz.LeaseScheduleλ({},{}),4,24)",
                level_row(pay, n),
                r(rate)
            ),
            one(float(carrying)),
        ),
        case(
            "lease_l4_rou_after",
            format!("=oz.ROUScheduleλ({},36)", r(l4[2])),
            straight_line(l4[2], 36),
        ),
        case(
            "lease_l5_asset_exhausted",
            format!(
                "=oz.LeaseRemeasureλ({},0.06,50000,4000)",
                level_row(10000, 3)
            ),
            column(remeasure(l5, 0.0, 50000.0, 4000.0).map(float)),
        ),
    ]
}

fn loan_cases() -> Vec<Case> {
    [mortgage_cases(), division_7a_cases()].concat()
}

fn mortgage_cases() -> Vec<Case> {
    let mut cases = vec![];
    // Monthly principal and interest loans. The same inputs can be entered in a mortgage calculator by hand.
    for (label, principal, apr, months) in
        [("m1", 600_000, 0.0624, 360), ("m2", 450_000, 0.0589, 300)]
    {
        let rate = apr / 12.0;
        let payment = level_payment(f64::from(principal), rate, months);
        let loan = format!(
            "oz.Amortiseλ({principal},{},{months},DATE(2026,11,1))",
            r(apr)
        );
        cases.extend([
            case(
                format!("loan_{label}_repayment"),
                format!("=INDEX({loan},4,1)"),
                one(float(-payment)),
            ),
            case(
                format!("loan_{label}_total_interest"),
                format!("=SUM(CHOOSEROWS({loan},3))"),
                one(float(payment * months as f64 - f64::from(principal))),
            ),
            case(
                format!("loan_{label}_first_interest"),
                format!("=INDEX({loan},3,1)"),
                one(float(f64::from(principal) * rate)),
            ),
            case(
                format!("loan_{label}_final_closing"),
                format!("=ROUND(INDEX({loan},5,{months}),6)"),
                one(float(0.0)),
            ),
        ]);
    }
    cases
}

fn division_7a_cases() -> Vec<Case> {
    let mut cases = vec![];
    // Annual level repayments, the shape of a Division 7A minimum yearly repayment, at
    // the ATO's 8.77% benchmark for 2026-27. No calculator output is recorded here.
    for (label, principal, years) in [("d1", 85_000, 7), ("d2", 240_000, 22)] {
        let payment = level_payment(f64::from(principal), 0.0877, years);
        let interest = f64::from(principal) * 0.0877;
        let loan = format!("oz.AmortiseBλ({principal},0.0877,{years},{years},1,12)");
        cases.extend([
            case(
                format!("loan_{label}_yearly_repayment"),
                format!("=INDEX({loan},3,1)"),
                one(float(payment)),
            ),
            case(
                format!("loan_{label}_first_interest"),
                format!("=INDEX({loan},2,1)"),
                one(float(interest)),
            ),
            case(
                format!("loan_{label}_first_principal"),
                format!("=INDEX({loan},5,1)"),
                one(float(payment - interest)),
            ),
            case(
                format!("loan_{label}_final_closing"),
                format!("=ROUND(INDEX({loan},4,{years}),6)"),
                one(float(0.0)),
            ),
        ]);
    }
    cases
}

fn ato_cases() -> Vec<Case> {
    [ato_turnover_cases(), ato_ratio_cases(), ato_labour_cases()].concat()
}

fn ato_turnover_cases() -> Vec<Case> {
    let turnover = |other: Option<i64>, total| one(int(ato_turnover(other, total)));
    vec![
        case(
            "ato_turnover_keeps_other_sales",
            "=oz.ATOTurnoverλ(480000,500000)",
            turnover(Some(480_000), 500_000),
        ),
        case(
            "ato_turnover_below_half",
            "=oz.ATOTurnoverλ(200000,500000)",
            turnover(Some(200_000), 500_000),
        ),
        case(
            "ato_turnover_exactly_half",
            "=oz.ATOTurnoverλ(250000,500000)",
            turnover(Some(250_000), 500_000),
        ),
        case(
            "ato_turnover_nil",
            r#"=oz.ATOTurnoverλ("",500000)"#,
            turnover(None, 500_000),
        ),
        case(
            "ato_turnover_text",
            r#"=oz.ATOTurnoverλ("x",500000)"#,
            one(text(VALUE)),
        ),
        case(
            "ato_turnover_rows",
            "=oz.ATOTurnoverλ({480000,200000},{500000,500000})",
            row([
                int(ato_turnover(Some(480_000), 500_000)),
                int(ato_turnover(Some(200_000), 500_000)),
            ]),
        ),
    ]
}

fn ato_ratio_cases() -> Vec<Case> {
    let ratio = |amount: i64, turnover: i64| one(float(amount as f64 / turnover as f64));
    vec![
        case(
            "ato_rent_ratio",
            "=oz.ATOBenchmarkRatioλ(24000,480000)",
            ratio(24000, 480_000),
        ),
        case(
            "ato_ratio_zero_turnover",
            "=oz.ATOBenchmarkRatioλ(24000,0)",
            one(text(DIV0)),
        ),
        case(
            "ato_total_expenses",
            "=oz.ATOTotalExpensesRatioλ(410000,480000,30000)",
            ratio(380_000, 480_000),
        ),
        case(
            "ato_total_expenses_no_associates",
            "=oz.ATOTotalExpensesRatioλ(410000,480000)",
            ratio(410_000, 480_000),
        ),
        case(
            "ato_cost_of_sales_wages_in",
            "=oz.ATOCostOfSalesRatioλ(260000,480000,60000)",
            ratio(200_000, 480_000),
        ),
        case(
            "ato_cost_of_sales_wages_out",
            "=oz.ATOCostOfSalesRatioλ(260000,480000)",
            ratio(260_000, 480_000),
        ),
    ]
}

fn ato_labour_cases() -> Vec<Case> {
    vec![
        case(
            "ato_labour_w1_higher",
            "=oz.ATOLabourRatioλ(90000,20000,480000,10000,95000)",
            one(float(ato_labour(90_000, 20_000, 480_000, 10_000, 95_000))),
        ),
        case(
            "ato_labour_w1_lower",
            "=oz.ATOLabourRatioλ(90000,20000,480000,10000,85000)",
            one(float(ato_labour(90_000, 20_000, 480_000, 10_000, 85_000))),
        ),
        case(
            "ato_labour_minimal",
            "=oz.ATOLabourRatioλ(90000,20000,480000)",
            one(float(ato_labour(90_000, 20_000, 480_000, 0, 0))),
        ),
    ]
}

fn rate_cases() -> Vec<Case> {
    [rate_lookup_cases(), rate_refusal_cases()].concat()
}

fn rate_lookup_cases() -> Vec<Case> {
    let table = "DATE(2026,{1;7;10},1),{0.04;0.045;0.05}";
    let starts = [serial(2026, 1, 1), serial(2026, 7, 1), serial(2026, 10, 1)];
    let rates = [0.04, 0.045, 0.05];
    let rate = |date: i64| rate_at_date(&starts, &rates, date).map_or(Json::Null, float);
    vec![
        case(
            "rate_at_date_example",
            format!("=oz.RateAtDateλ({table},DATE(2026,8,15))"),
            one(rate(serial(2026, 8, 15))),
        ),
        case(
            "rate_at_date_on_start",
            format!("=oz.RateAtDateλ({table},DATE(2026,10,1))"),
            one(rate(serial(2026, 10, 1))),
        ),
        case(
            "rate_at_date_before_first",
            format!("=oz.RateAtDateλ({table},DATE(2025,12,31))"),
            one(text(NA)),
        ),
        case(
            "rate_at_date_column",
            format!("=oz.RateAtDateλ({table},DATE(2026,{{3;7;12}},1))"),
            column([3, 7, 12].map(|month| rate(serial(2026, month, 1)))),
        ),
        case(
            "rate_at_date_text_date",
            format!(r#"=oz.RateAtDateλ({table},"2026-08-01")"#),
            one(text(VALUE)),
        ),
    ]
}

fn rate_refusal_cases() -> Vec<Case> {
    vec![
        case(
            "rate_at_date_unsorted",
            "=oz.RateAtDateλ(DATE(2026,{7;1},1),{0.04;0.045},DATE(2026,8,1))",
            one(text(
                "RateAtDateλ needs EffectiveDates in ascending order. Sort the table by effective date.",
            )),
        ),
        case(
            "rate_at_date_repeated",
            "=oz.RateAtDateλ(DATE(2026,{7;7},1),{0.04;0.045},DATE(2026,8,1))",
            one(text(
                "RateAtDateλ found an effective date that repeats. Keep one rate for each date.",
            )),
        ),
        case(
            "rate_at_date_gap",
            r#"=oz.RateAtDateλ(DATE(2026,{1;7},1),{0.04;""},DATE(2026,8,1))"#,
            one(text(
                "RateAtDateλ needs EffectiveDates and Rates of the same length with a number in every \
                 entry. A blank or text entry leaves a gap in the table.",
            )),
        ),
    ]
}

/// Month of the financial year that starts in month `start`, from 1 to 12.
fn fy_month(day: NaiveDate, start: i64) -> i64 {
    (i64::from(day.month()) - start).rem_euclid(12) + 1
}

/// Each row's total over the dates from the first day of as_at's financial year.
fn year_to_date(
    rows: &[[i64; 4]; 2],
    dates: &[NaiveDate; 4],
    as_at: NaiveDate,
    start: u32,
) -> Json {
    let first = date(as_at.year() - i32::from(as_at.month() < start), start, 1);
    column(rows.iter().map(|row| {
        let in_year = row
            .iter()
            .zip(dates)
            .filter(|(_, day)| first <= **day && **day <= as_at);
        int(in_year.map(|(value, _)| value).sum())
    }))
}

/// Actual less budget, signed by the line's type: 1 for income, -1 for an expense.
fn variance(actual: i64, budget: i64, kind: i64) -> i64 {
    (actual - budget) * kind
}

/// The signed variance as a fraction of the budget's size.
fn variance_pct(actual: i64, budget: i64, kind: i64) -> Json {
    float(variance(actual, budget, kind) as f64 / budget.abs() as f64)
}

fn variance_cases() -> Vec<Case> {
    vec![
        case(
            "variance_income_above",
            "=oz.Varianceλ(120,100)",
            one(int(variance(120, 100, 1))),
        ),
        case(
            "variance_expense_below",
            "=oz.Varianceλ(90,100,-1)",
            one(int(variance(90, 100, -1))),
        ),
        case(
            "variance_types_by_line",
            "=oz.Varianceλ({120;90;80},{100;100;100},{1;-1;1})",
            column(
                [(120, 100, 1), (90, 100, -1), (80, 100, 1)]
                    .map(|(a, b, t)| int(variance(a, b, t))),
            ),
        ),
        case(
            "variance_type_refused",
            "=oz.Varianceλ(120,100,2)",
            one(text(
                "Varianceλ needs Type to be 1 for income or -1 for an expense in every entry.",
            )),
        ),
        case(
            "variance_pct_income",
            "=oz.VariancePctλ(110,100)",
            one(variance_pct(110, 100, 1)),
        ),
        case(
            "variance_pct_negative_budget",
            "=oz.VariancePctλ(-30,-20)",
            one(variance_pct(-30, -20, 1)),
        ),
        case(
            "variance_pct_negative_budget_expense",
            "=oz.VariancePctλ(-30,-20,-1)",
            one(variance_pct(-30, -20, -1)),
        ),
        case(
            "variance_pct_zero_budget",
            "=oz.VariancePctλ(5,0)",
            one(text(DIV0)),
        ),
    ]
}

fn year_to_date_cases() -> Vec<Case> {
    let rows = [[10, 20, 30, 40], [1, 2, 3, 4]];
    let months = [5, 6, 7, 8].map(|month| date(2026, month, 1));
    let dates = "DATE(2026,{5,6,7,8},1)";
    let grid = "{10,20,30,40;1,2,3,4}";
    vec![
        case(
            "year_to_date_example",
            format!("=oz.YearToDateλ({grid},{dates},DATE(2026,8,31))"),
            year_to_date(&rows, &months, date(2026, 8, 31), 7),
        ),
        case(
            "year_to_date_june",
            format!("=oz.YearToDateλ({grid},{dates},DATE(2026,6,30))"),
            year_to_date(&rows, &months, date(2026, 6, 30), 7),
        ),
        case(
            "year_to_date_calendar",
            format!("=oz.YearToDateλ({grid},{dates},DATE(2026,7,15),1)"),
            year_to_date(&rows, &months, date(2026, 7, 15), 1),
        ),
        case(
            "year_to_date_shape",
            format!("=oz.YearToDateλ({grid},DATE(2026,{{5,6,7}},1),DATE(2026,8,31))"),
            one(text(
                "YearToDateλ needs Dates to be one row with a date for each column of Values.",
            )),
        ),
    ]
}

fn financial_period_cases() -> Vec<Case> {
    let quarter = |month| (fy_month(date(2026, month, 1), 7) - 1) / 3 + 1;
    vec![
        case(
            "financial_month_row",
            "=oz.FinancialMonthλ(DATE(2026,{1,6,7,12},15))",
            row([1, 6, 7, 12].map(|month| int(fy_month(date(2026, month, 15), 7)))),
        ),
        case(
            "financial_month_april_start",
            "=oz.FinancialMonthλ(DATE(2026,{3;4},1),4)",
            column([3, 4].map(|month| int(fy_month(date(2026, month, 1), 4)))),
        ),
        case(
            "financial_quarter_row",
            "=oz.FinancialQuarterλ(DATE(2026,{7,9,10,1,6},1))",
            row([7, 9, 10, 1, 6].map(|month| int(quarter(month)))),
        ),
        case(
            "financial_quarter_start_refused",
            "=oz.FinancialQuarterλ(DATE(2026,7,1),13)",
            one(text(
                "FinancialQuarterλ needs StartMonth to be one whole number from 1 to 12.",
            )),
        ),
    ]
}

fn refusal_cases() -> Vec<Case> {
    let per_row = [([120, 130], 1), ([90, 80], -1)]
        .map(|(row, t)| Json::Array(row.map(|a| int(variance(a, 100, t))).into()));
    vec![
        case(
            "variance_type_row_refused",
            "=oz.Varianceλ({120;90},{100;100},{1,-1})",
            one(text(
                "Varianceλ needs Actual and Budget of the same shape, and Type as one value, one for each row or one for each cell.",
            )),
        ),
        case(
            "variance_type_per_row_across_months",
            "=oz.Varianceλ({120,130;90,80},{100,100;100,100},{1;-1})",
            Json::Array(per_row.into()),
        ),
        case(
            "variance_text_amount_refused",
            r#"=oz.Varianceλ("120",100)"#,
            one(text(
                "Varianceλ needs Actual and Budget to hold numbers or blank cells; text and TRUE or FALSE are refused.",
            )),
        ),
        case(
            "year_to_date_start_range_refused",
            "=oz.YearToDateλ({10,20},DATE(2026,{2,5},1),DATE(2026,8,31),{1,7})",
            one(text(
                "YearToDateλ needs StartMonth to be one whole number from 1 to 12.",
            )),
        ),
        case(
            "year_to_date_boolean_refused",
            "=oz.YearToDateλ({TRUE,20},DATE(2026,{7,8},1),DATE(2026,8,31))",
            one(text(
                "YearToDateλ needs Values to hold numbers or blank cells; text and TRUE or FALSE are refused.",
            )),
        ),
    ]
}

/// Every case, in the order the committed file lists them.
fn cases() -> Vec<Case> {
    let periods = [
        variance_cases(),
        year_to_date_cases(),
        financial_period_cases(),
        refusal_cases(),
    ];
    [
        lease_cases(),
        loan_cases(),
        ato_cases(),
        rate_cases(),
        periods.concat(),
    ]
    .concat()
}

/// `json.dumps(CASES, ensure_ascii=False, indent=2) + "\n"`.
fn cases_json() -> String {
    let cases = cases()
        .into_iter()
        .map(|(id, formula, expected)| {
            Json::Object(vec![
                ("id".into(), Json::Str(id)),
                ("formula".into(), Json::Str(formula)),
                ("expected".into(), expected),
            ])
        })
        .collect();
    pyjson::dumps(&Json::Array(cases), false) + "\n"
}

/// `math.isclose(got, want, rel_tol=1e-9, abs_tol=1e-6)`, or equality when either is text.
fn close(got: &Json, want: &Json) -> bool {
    if matches!(got, Json::Str(_)) || matches!(want, Json::Str(_)) {
        return got == want;
    }
    let (a, b) = (number(got), number(want));
    if a == b {
        return true;
    }
    if a.is_infinite() || b.is_infinite() {
        return false;
    }
    let diff = (b - a).abs();
    diff <= 1e-9 * b.abs() || diff <= 1e-9 * a.abs() || diff <= 1e-6
}

fn load(text: &str) -> Json {
    pyjson::loads(text).unwrap_or_else(|e| panic!("{}", e.0))
}

fn field<'a>(value: &'a Json, key: &str) -> &'a Json {
    value.get(key).unwrap_or_else(|| panic!("no {key}"))
}

fn list(value: &Json) -> &[Json] {
    match value {
        Json::Array(items) => items,
        other => panic!("{} is not a list", other.repr()),
    }
}

#[test]
fn the_committed_cases_are_this_module_s_cases() {
    assert_eq!(read(CASES), cases_json());
}

#[test]
#[ignore = "writes tools/tests/fixtures/cross-checks-cases.json"]
fn write_the_cases() {
    common::write_bytes(&root().join(CASES), cases_json().as_bytes())
        .unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn the_hand_checks_behind_the_references() {
    // 1 a year for 3 years at 6%: 1/1.06 + 1/1.06^2 + 1/1.06^3 = 2.673012
    assert!(almost_equal(annuity(0.06, 3), 2.673_011_9, 6));
    assert_eq!(annuity(0.0, 4), 4.0);
    // 100 at 10% over 2 years: 100 / 1.735537 = 57.619048
    assert!(almost_equal(level_payment(100.0, 0.1, 2), 57.619_047_6, 6));
    // The unwound liability ends at nil and its interest is payments less the opening.
    let rows = unwind(100.0 * annuity(0.05, 3), &[100; 3], 0.05);
    let rows: Vec<Vec<f64>> = list(&rows)
        .iter()
        .map(|row| list(row).iter().map(number).collect())
        .collect();
    let closing = *rows[3].last().expect("a closing balance");
    assert!(almost_equal(closing, 0.0, 9), "{closing}");
    let interest: f64 = rows[2].iter().sum();
    assert!(almost_equal(interest, 300.0 - rows[0][0], 9), "{interest}");
    assert_eq!(remeasure(30.0, 0.0, 50.0, 4.0), [30.0, -20.0, 0.0, -16.0]);
    assert_eq!(serial(2026, 1, 1), 46023);
}

#[test]
fn every_case_has_a_distinct_id_and_names_an_indexed_function() {
    let index = read("functions.csv");
    let cases = cases();
    let ids: BTreeSet<&str> = cases.iter().map(|(id, _, _)| id.as_str()).collect();
    assert_eq!(cases.len(), ids.len());
    for (id, formula, _) in &cases {
        let (_, after) = formula
            .split_once("oz.")
            .unwrap_or_else(|| panic!("case={id}: no oz. call"));
        let name = after.split('(').next().unwrap_or_default();
        assert!(
            index.contains(&format!("oz.{name},")),
            "case={id}: oz.{name} is not in functions.csv"
        );
    }
}

#[test]
fn the_excel_results_record_the_committed_workbook_and_inputs() {
    let results = load(&read_sig(RESULTS));
    let base = load(&read("release/workbook-base.json"));
    let workbook = field(&results, "workbook_sha256");
    assert_eq!(workbook, field(&base, "sha256"));
    assert_eq!(workbook.as_str(), Some(sha256("ozzit.xlsx").as_str()));
    // Retained by tools/excel_eval_formulas_verified.ps1, which records what it ran.
    for (key, path) in [
        ("cases_sha256", CASES),
        ("helper_sha256", "tools/excel_eval_formulas.ps1"),
        (
            "evidence_helper_sha256",
            "tools/excel_eval_formulas_verified.ps1",
        ),
    ] {
        assert_eq!(
            field(&results, key).as_str(),
            Some(sha256(path).as_str()),
            "{key}"
        );
    }
    let excel = field(&results, "excel").as_str().expect("an Excel version");
    assert!(
        common::matches(r"^\d+\.\d+ build \d+\n?\z", excel),
        "{excel:?}"
    );
}

#[test]
fn excel_returned_every_expected_value() {
    let results = load(&read_sig(RESULTS));
    let Json::Object(returned) = field(&results, "results") else {
        panic!("results is not a mapping")
    };
    let cases = cases();
    let mut names: Vec<&str> = returned.iter().map(|(id, _)| id.as_str()).collect();
    let mut ids: Vec<&str> = cases.iter().map(|(id, _, _)| id.as_str()).collect();
    names.sort_unstable();
    ids.sort_unstable();
    assert_eq!(names, ids);
    for (id, _, expected) in &cases {
        let got = list(field(field(&results, "results"), id));
        let shape = |grid: &[Json]| grid.iter().map(|row| list(row).len()).collect::<Vec<_>>();
        assert_eq!(
            shape(got),
            shape(list(expected)),
            "case={id}: shape differs"
        );
        for (got_row, want_row) in got.iter().zip(list(expected)) {
            for (value, want) in list(got_row).iter().zip(list(want_row)) {
                assert!(
                    close(value, want),
                    "case={id}: got {}, want {}",
                    value.repr(),
                    want.repr()
                );
            }
        }
    }
}
