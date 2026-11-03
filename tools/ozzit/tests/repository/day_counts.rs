//! Day-count arithmetic for `oz.DayCountRateλ`, checked without Excel.
//!
//! The native `rate_date_helpers` pass carries the published source of the four
//! rate and date helpers. Re-running the pass proves that it writes nothing, but
//! does not assert what the day counts come to. That gap let the Actual/Actual
//! defect recorded in the CHANGELOG reach a release.
//!
//! The arithmetic itself lives in an Excel LAMBDA, so it cannot be executed
//! here. This module closes the gap in 2 halves that have to agree:
//!
//! 1. `fraction()` restates each convention, and the cases below assert it
//!    against year fractions worked out by hand in the comments beside them.
//! 2. The formula pins assert that the shipped LAMBDA still spells those same 4
//!    rules, comparing whitespace-normalised text against the published source.
//!    The native pass's source table is byte-identical to `src/Financial.txt`,
//!    which `ozzit verify_sources` proves against the workbook, so pinning the
//!    source table pins the shipped function.
//!
//! Change the LAMBDA and the pins fail, which is the prompt to re-derive the
//! mirror and its hand calculations rather than to edit the pin.
//!
//! Dates are period boundaries the way the LAMBDA reads them: `start` is the
//! first day of the period and `following` is the first day of the next one, the
//! LAMBDA's `Starts` and `Next`.
//!
//! The QuantLib tests also check frozen compiled QuantLib values and a retained
//! native Excel run, without starting Excel or importing QuantLib;
//! `docs/quantlib-day-count-comparison.md` defines their scope and regeneration
//! procedure.
use super::{almost_equal, json, read, sha256, spaced, within};
use chrono::{Datelike, NaiveDate};
use ozzit::common;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const FIXTURES: &str = "tools/tests/postbuild/fixtures";

// The Convention argument's numbers, as the shipped PARAMETERS row states them.
const THIRTY_360: i64 = 1;
const ACTUAL_360: i64 = 2;
const ACTUAL_365: i64 = 3;
const ACTUAL_ACTUAL: i64 = 4;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("a valid date")
}

fn iso(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap_or_else(|e| panic!("{text}: {e}"))
}

fn days(from: NaiveDate, to: NaiveDate) -> i64 {
    (to - from).num_days()
}

/// 366 in a leap year, 365 otherwise, counted the way the LAMBDA counts it.
fn days_in_year(year: i32) -> i64 {
    days(date(year, 1, 1), date(year + 1, 1, 1))
}

/// The year fraction one period contributes, restating the shipped LAMBDA.
fn fraction(start: NaiveDate, following: NaiveDate, convention: i64) -> Result<f64, String> {
    match convention {
        THIRTY_360 => {
            // European rule: a 31st counts as the 30th at either end.
            let days = 360 * i64::from(following.year() - start.year())
                + 30 * (i64::from(following.month()) - i64::from(start.month()))
                + i64::from(following.day().min(30))
                - i64::from(start.day().min(30));
            Ok(days as f64 / 360.0)
        }
        ACTUAL_360 => Ok(days(start, following) as f64 / 360.0),
        ACTUAL_365 => Ok(days(start, following) as f64 / 365.0),
        ACTUAL_ACTUAL => {
            // ISDA: each calendar year is divided by its own length, and every whole
            // year in between contributes exactly one.
            let split = following.min(date(start.year() + 1, 1, 1));
            let rest = if following.year() > start.year() {
                f64::from(following.year() - start.year() - 1)
                    + days(date(following.year(), 1, 1), following) as f64
                        / days_in_year(following.year()) as f64
            } else {
                0.0
            };
            Ok(days(start, split) as f64 / days_in_year(start.year()) as f64 + rest)
        }
        _ => Err(format!(
            "convention {convention} is not one the helper accepts"
        )),
    }
}

fn year_fraction(start: NaiveDate, following: NaiveDate, convention: i64) -> f64 {
    fraction(start, following, convention).unwrap_or_else(|e| panic!("{e}"))
}

fn assert_places(first: f64, second: f64, places: i32) {
    assert!(
        almost_equal(first, second, places),
        "{first} != {second} within {places} places"
    );
}

fn assert_within(first: f64, second: f64, delta: f64, context: &str) {
    assert!(
        within(first, second, delta),
        "{context}: {first} != {second} within {delta}"
    );
}

#[test]
fn a_whole_thirty_360_month_is_thirty_days() {
    // 1 Jul 2026 to 1 Aug 2026: 360*0 + 30*(8-7) + 1 - 1 = 30 days; 30/360 = 1/12.
    assert_places(
        year_fraction(date(2026, 7, 1), date(2026, 8, 1), THIRTY_360),
        1.0 / 12.0,
        12,
    );
}

#[test]
fn thirty_360_month_end_days_clamp_to_the_thirtieth_at_both_ends() {
    // 31 Jan 2026 to 31 Mar 2026: 30*(3-1) + min(31,30) - min(31,30) = 60 days.
    // 60/360 = 1/6. The European rule is why February's short month does not show.
    assert_places(
        year_fraction(date(2026, 1, 31), date(2026, 3, 31), THIRTY_360),
        1.0 / 6.0,
        12,
    );
}

#[test]
fn thirty_360_february_in_a_leap_year_is_still_thirty_days() {
    // 1 Feb 2024 to 1 Mar 2024: 30*(3-2) + 1 - 1 = 30 days, although February
    // actually ran 29 days. 30/360 = 1/12.
    assert_places(
        year_fraction(date(2024, 2, 1), date(2024, 3, 1), THIRTY_360),
        1.0 / 12.0,
        12,
    );
}

#[test]
fn three_whole_thirty_360_years_are_three_hundred_and_sixty_days_each() {
    // 1 Jul 2023 to 1 Jul 2026: 360*3 + 30*0 + 1 - 1 = 1,080 days; 1080/360 = 3.
    assert_places(
        year_fraction(date(2023, 7, 1), date(2026, 7, 1), THIRTY_360),
        3.0,
        12,
    );
}

#[test]
fn july_is_thirty_one_actual_days_over_a_three_hundred_and_sixty_day_year() {
    // 1 Jul 2026 to 1 Aug 2026 is 31 days. 31/360 = 0.086111111...
    let july = year_fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_360);
    assert_places(july, 31.0 / 360.0, 12);
    assert_places(july, 0.086_111_111_1, 9);
}

#[test]
fn a_leap_year_runs_past_one_on_a_three_hundred_and_sixty_day_year() {
    // 1 Jan 2024 to 1 Jan 2025 is 366 actual days. 366/360 = 1.016666...
    assert_places(
        year_fraction(date(2024, 1, 1), date(2025, 1, 1), ACTUAL_360),
        366.0 / 360.0,
        12,
    );
}

#[test]
fn july_is_thirty_one_actual_days_over_a_three_hundred_and_sixty_five_day_year() {
    // 1 Jul 2026 to 1 Aug 2026 is 31 days. 31/365 = 0.084931506...
    let july = year_fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_365);
    assert_places(july, 31.0 / 365.0, 12);
    assert_places(july, 0.084_931_506_8, 9);
}

#[test]
fn a_leap_year_exceeds_one_because_the_denominator_is_fixed() {
    // 1 Jan 2024 to 1 Jan 2025 is 366 days; the denominator stays 365, so the
    // year comes to 366/365 = 1.002739726..., not 1. That is the convention,
    // and it is the difference Actual/Actual removes.
    let year = year_fraction(date(2024, 1, 1), date(2025, 1, 1), ACTUAL_365);
    assert_places(year, 366.0 / 365.0, 12);
    assert_places(year, 1.002_739_726_0, 9);
}

#[test]
fn three_actual_365_years_including_a_leap_year() {
    // 1 Jul 2023 to 1 Jul 2026 is 365 + 366 + 365 = 1,096 days (2024 is the leap
    // year). 1096/365 = 3.002739726...
    assert_eq!(days(date(2023, 7, 1), date(2026, 7, 1)), 1096);
    assert_places(
        year_fraction(date(2023, 7, 1), date(2026, 7, 1), ACTUAL_365),
        1096.0 / 365.0,
        12,
    );
}

#[test]
fn an_actual_actual_period_inside_one_calendar_year_uses_that_year_only() {
    // 1 Jul 2026 to 1 Aug 2026: Next is before 1 Jan 2027, so Split is Next and
    // nothing is added. 31/365 = 0.084931506...
    assert_places(
        year_fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_ACTUAL),
        31.0 / 365.0,
        12,
    );
}

#[test]
fn an_actual_actual_leap_year_is_exactly_one() {
    // 1 Jan 2024 to 1 Jan 2025: the first year contributes 366/366 = 1, and
    // YearLast (2025) exceeds YearOne (2024) by one, so the whole-year term is
    // 2025 - 2024 - 1 = 0 and the last part is 0/365. Total exactly 1.
    assert_eq!(
        year_fraction(date(2024, 1, 1), date(2025, 1, 1), ACTUAL_ACTUAL),
        1.0
    );
}

#[test]
fn an_actual_actual_non_leap_year_is_exactly_one() {
    // 1 Jan 2026 to 1 Jan 2027: 365/365 = 1, with nothing added.
    assert_eq!(
        year_fraction(date(2026, 1, 1), date(2027, 1, 1), ACTUAL_ACTUAL),
        1.0
    );
}

#[test]
fn an_actual_actual_period_spanning_several_years_counts_each_year_by_its_own_length() {
    // 1 Jul 2023 to 1 Jul 2026, the defect the CHANGELOG records.
    // First part:  1 Jul 2023 to 1 Jan 2024 is 31+31+30+31+30+31 = 184 days,
    //              over 2023's 365, so 184/365.
    // Whole years: 2026 - 2023 - 1 = 2 (2024 and 2025), one each regardless of
    //              2024 being a leap year.
    // Last part:   1 Jan 2026 to 1 Jul 2026 is 31+28+31+30+31+30 = 181 days,
    //              over 2026's 365, so 181/365.
    // Total:       184/365 + 2 + 181/365 = 2 + 365/365 = 3 exactly.
    assert_eq!(days(date(2023, 7, 1), date(2024, 1, 1)), 184);
    assert_eq!(days(date(2026, 1, 1), date(2026, 7, 1)), 181);
    assert_places(
        year_fraction(date(2023, 7, 1), date(2026, 7, 1), ACTUAL_ACTUAL),
        3.0,
        12,
    );
}

#[test]
fn an_actual_actual_period_crossing_into_a_leap_year_splits_at_new_year() {
    // 1 Dec 2023 to 1 Feb 2024, 2 months straddling 1 January.
    // First part: 1 Dec 2023 to 1 Jan 2024 is 31 days over 2023's 365 = 31/365.
    // Last part:  1 Jan 2024 to 1 Feb 2024 is 31 days over 2024's 366 = 31/366,
    //             because 2024 is a leap year. No whole years in between.
    // Total:      31/365 + 31/366 = 0.0849315068 + 0.0846994536 = 0.1696309604.
    assert_eq!(days_in_year(2024), 366);
    let period = year_fraction(date(2023, 12, 1), date(2024, 2, 1), ACTUAL_ACTUAL);
    assert_places(period, 31.0 / 365.0 + 31.0 / 366.0, 12);
    assert_places(period, 0.169_630_960_4, 9);
}

#[test]
fn the_leap_day_itself_is_carried_by_the_leap_year_denominator() {
    // 1 Feb 2024 to 1 Mar 2024 is 29 days, over 2024's 366: 29/366 = 0.0792349727.
    assert_eq!(days(date(2024, 2, 1), date(2024, 3, 1)), 29);
    assert_places(
        year_fraction(date(2024, 2, 1), date(2024, 3, 1), ACTUAL_ACTUAL),
        29.0 / 366.0,
        12,
    );
}

#[test]
fn the_period_rate_is_the_apr_times_the_year_fraction() {
    // The LAMBDA's final step is Rates = Rate * Fraction. At the help's own
    // example, APR 0.073 over 1 Jul 2026 to 1 Aug 2026 on the Actual/365
    // default: 0.073 * 31/365 = 0.0062 exactly, which is what the help prints.
    let rate = 0.073 * year_fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_365);
    assert_places(rate, 0.0062, 12);
}

#[test]
fn an_unrecognised_convention_is_rejected() {
    assert!(fraction(date(2026, 7, 1), date(2026, 8, 1), 5).is_err());
}

// Independent compiled values and a retained, hash-bound Excel run. CI checks
// this record and the restatement; it does not start Excel. The native run
// covers the first period at APR=1, with two supplied dates.

fn record() -> Value {
    json(&read(&format!("{FIXTURES}/quantlib-day-counts.json")))
}

fn number(value: &Value, context: &str) -> f64 {
    // A JSON number, never a Boolean; serde_json reads no NaN or infinity.
    value
        .as_f64()
        .unwrap_or_else(|| panic!("{context}: {value} is not a number"))
}

fn text<'a>(value: &'a Value, context: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{context}: {value} is not text"))
}

fn array<'a>(value: &'a Value, context: &str) -> &'a [Value] {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{context}: {value} is not a list"))
}

fn object<'a>(value: &'a Value, context: &str) -> &'a serde_json::Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{context}: {value} is not a mapping"))
}

/// A QuantLib case's id, boundaries and fractions by method.
type QuantLibCase<'a> = (&'a str, NaiveDate, NaiveDate, Vec<(&'a str, f64)>);

/// Each QuantLib case in the record.
fn quantlib_cases(record: &Value) -> Vec<QuantLibCase<'_>> {
    array(&record["cases"], "cases")
        .iter()
        .map(|case| {
            let id = text(&case["id"], "id");
            let fractions = object(&case["fractions"], id)
                .iter()
                .map(|(method, expected)| (method.as_str(), number(expected, id)))
                .collect();
            (
                id,
                iso(text(&case["start"], id)),
                iso(text(&case["following"], id)),
                fractions,
            )
        })
        .collect()
}

/// The Excel version and evaluation time a retained native run records.
fn assert_native_run(native: &Value) {
    let excel = text(&native["excel"], "excel");
    assert!(
        common::matches(r"^\d+\.\d+ build \d+\n?\z", excel),
        "{excel:?}"
    );
    let at = text(&native["evaluated_at_utc"], "evaluated_at_utc");
    assert!(
        common::matches(r"^\d{4}-\d{2}-\d{2}T.*Z\n?\z", at),
        "{at:?}"
    );
}

fn method(text: &str) -> i64 {
    text.parse()
        .unwrap_or_else(|_| panic!("{text:?} is not a method"))
}

#[test]
fn the_restatement_agrees_with_quantlib() {
    let record = record();
    let tolerance = number(&record["absolute_tolerance"], "absolute_tolerance");
    for (id, start, following, fractions) in quantlib_cases(&record) {
        for (method_text, expected) in fractions {
            let context = format!("case={id} method={method_text}");
            assert!(expected.is_finite(), "{context}");
            assert_within(
                year_fraction(start, following, method(method_text)),
                expected,
                tolerance,
                &context,
            );
        }
    }
}

#[test]
fn retained_excel_results_agree_with_quantlib() {
    let record = record();
    let tolerance = number(&record["absolute_tolerance"], "absolute_tolerance");
    let native = &record["excel"];
    assert!(!text(&native["excel"], "excel").is_empty());
    assert_eq!(native["workbook_sha256"], sha256("ozzit.xlsx"));
    let results = object(&native["results"], "results");
    let cases = quantlib_cases(&record);
    let wanted: BTreeSet<String> = cases
        .iter()
        .flat_map(|(id, _, _, fractions)| fractions.iter().map(move |(m, _)| format!("{id}_{m}")))
        .collect();
    assert_eq!(results.keys().cloned().collect::<BTreeSet<_>>(), wanted);
    for (id, _, _, fractions) in &cases {
        for (method_text, expected) in fractions {
            let context = format!("case={id} method={method_text}");
            let rows = array(&results[&format!("{id}_{method_text}")], &context);
            let [row] = rows else {
                panic!("{context}: {} rows", rows.len())
            };
            let [actual] = array(row, &context) else {
                panic!("{context}: {row}")
            };
            let actual = number(actual, &context);
            assert!(actual.is_finite(), "{context}");
            assert_within(actual, *expected, tolerance, &context);
        }
    }
}

#[test]
fn the_reference_generator_is_bound_to_the_record() {
    let record = record();
    assert_eq!(
        record["generator_sha256"],
        sha256("tools/tests/postbuild/generate_quantlib_day_count_fixture.py")
    );
    assert_eq!(record["interval_semantics"], "[start, following)");
    assert_eq!(
        record["constructor_expressions"],
        serde_json::json!({
            "1": "Thirty360(European)",
            "2": "Actual360(False)",
            "3": "Actual365Fixed(Standard)",
            "4": "ActualActual(ISDA)",
        })
    );
}

#[test]
fn fixture_provenance() {
    let record = record();
    assert_eq!(record["schema_version"], 1);
    assert_eq!(record["quantlib_version"], "1.43");
    assert_eq!(record["wheel"]["version"], "1.43");
    assert_eq!(
        record["wheel"]["sha256"],
        "35cd2c178aa9a30c3b7a9ca050536037c3185afd6728e6a1cf2085adef54464e"
    );
    assert_eq!(
        record["binary_sha256"],
        "a2a50e566b2cd6706085c4bb7da15f73419da1f4dd2d9359c51bd91d95aca889"
    );
    assert_eq!(
        number(&record["absolute_tolerance"], "absolute_tolerance"),
        1e-12
    );
}

#[test]
fn fixture_coverage() {
    let record = record();
    let cases = quantlib_cases(&record);
    assert_eq!(cases.len(), 18);
    assert_eq!(
        cases
            .iter()
            .map(|case| case.0)
            .collect::<BTreeSet<_>>()
            .len(),
        18
    );
    for (id, _, _, fractions) in &cases {
        let methods: BTreeSet<&str> = fractions.iter().map(|(m, _)| *m).collect();
        assert_eq!(methods, BTreeSet::from(["1", "2", "3", "4"]), "{id}");
    }
    let native = &record["excel"];
    assert_native_run(native);
    let largest = cases
        .iter()
        .flat_map(|(id, _, _, fractions)| {
            fractions.iter().map(move |(m, expected)| {
                let key = format!("{id}_{m}");
                (number(&native["results"][&key][0][0], &key) - expected).abs()
            })
        })
        .fold(f64::NEG_INFINITY, f64::max);
    assert_eq!(
        largest,
        number(&record["max_abs_difference"], "max_abs_difference")
    );
}

#[test]
fn fixture_variant_controls() {
    let record = record();
    let controls: BTreeSet<(&str, &str, &str)> =
        array(&record["variant_controls"], "variant_controls")
            .iter()
            .map(|c| {
                (
                    text(&c["case_id"], "case_id"),
                    text(&c["method"], "method"),
                    text(&c["alternative"], "alternative"),
                )
            })
            .collect();
    assert_eq!(
        array(&record["variant_controls"], "variant_controls").len(),
        8
    );
    assert_eq!(
        controls,
        BTreeSet::from([
            ("common_february_end", "1", "Thirty360(USA)"),
            ("common_february_end", "1", "Thirty360(BondBasis)"),
            ("leap_february_end", "1", "Thirty360(ISDA)"),
            ("zero_leap_day", "2", "Actual360(True)"),
            ("full_leap_year", "3", "Actual365Fixed(NoLeap)"),
            ("full_leap_year", "3", "ActualActual(ISDA)"),
            ("split_common_leap", "4", "ActualActual(ISMA)"),
            ("split_common_leap", "4", "ActualActual(AFB)"),
        ])
    );
}

#[test]
fn native_inputs_are_bound_to_the_evaluated_formulas() {
    let record = record();
    let inputs = format!("{FIXTURES}/quantlib-day-count-formulas.json");
    assert_eq!(record["excel"]["cases_sha256"], sha256(&inputs));
    assert_eq!(
        record["excel"]["helper_sha256"],
        sha256("tools/excel_eval_formulas.ps1")
    );
    assert_eq!(
        record["excel"]["evidence_helper_sha256"],
        sha256("tools/excel_eval_formulas_verified.ps1")
    );
    let numbers = |iso: NaiveDate| format!("{},{},{}", iso.year(), iso.month(), iso.day());
    let wanted: Vec<Value> = quantlib_cases(&record)
        .iter()
        .flat_map(|(id, start, following, fractions)| {
            fractions.iter().map(move |(m, _)| {
                serde_json::json!({
                    "id": format!("{id}_{m}"),
                    "formula": format!(
                        "=INDEX(oz.DayCountRateλ(HSTACK(DATE({}),DATE({})),1,{m}),1,1)",
                        numbers(*start),
                        numbers(*following)
                    ),
                })
            })
        })
        .collect();
    assert_eq!(json(&read(&inputs)), Value::Array(wanted));
}

#[test]
fn the_comparison_rejects_recorded_wrong_variants() {
    let record = record();
    let tolerance = number(&record["absolute_tolerance"], "absolute_tolerance");
    let cases: BTreeMap<&str, (NaiveDate, NaiveDate)> = quantlib_cases(&record)
        .into_iter()
        .map(|(id, start, following, _)| (id, (start, following)))
        .collect();
    let controls = array(&record["variant_controls"], "variant_controls");
    let methods: BTreeSet<&str> = controls
        .iter()
        .map(|c| text(&c["method"], "method"))
        .collect();
    assert_eq!(methods, BTreeSet::from(["1", "2", "3", "4"]));
    for control in controls {
        let alternative = text(&control["alternative"], "alternative");
        let (start, following) = cases[text(&control["case_id"], alternative)];
        let restated = year_fraction(
            start,
            following,
            method(text(&control["method"], alternative)),
        );
        let wrong = number(&control["fraction"], alternative);
        assert!(
            !within(restated, wrong, tolerance),
            "alternative={alternative}: {restated} matches the wrong variant {wrong}"
        );
    }
}

// Retained Excel results for schedules, coercion and APR broadcasting.
//
// Rational terms are declared independently for each positive schedule.
// Descending dates and malformed inputs remain observations, not financial
// expectations. A changed workbook needs a fresh read-only native run.

const NATIVE_CASES: &str = "tools/tests/postbuild/fixtures/native-day-count-cases.json";

const COVERAGE: [&str; 21] = [
    "inferred_last",
    "inferred_first",
    "leap_year",
    "common_year",
    "month_end",
    "short_gap",
    "gap_threshold",
    "month_clamp",
    "end_dates",
    "year_crossing",
    "annual",
    "column_dates",
    "scalar_apr",
    "row_apr",
    "column_apr",
    "both_columns",
    "text_dates",
    "mixed_dates",
    "end_flag",
    "default_convention",
    "invalid_method",
];

const SCHEDULES: [&str; 10] = [
    "leap_months",
    "common_months",
    "month_end_starts",
    "weekly",
    "gap_27",
    "gap_28",
    "end_months",
    "end_weekly",
    "year_crossing",
    "annual",
];

fn native_cases() -> Vec<Value> {
    let Value::Array(cases) = json(&read(NATIVE_CASES)) else {
        panic!("{NATIVE_CASES} is not a list")
    };
    cases
}

fn native_results() -> Value {
    json(&read(&format!("{FIXTURES}/native-day-count-results.json")))
}

fn case_id(case: &Value) -> &str {
    text(&case["id"], "id")
}

/// Each case by id; a repeated id keeps its last case, as a dict comprehension does.
fn by_id(cases: &[Value]) -> BTreeMap<&str, &Value> {
    cases.iter().map(|case| (case_id(case), case)).collect()
}

fn result<'a>(native: &'a Value, id: &str) -> &'a Value {
    native["results"]
        .get(id)
        .unwrap_or_else(|| panic!("no native result for {id}"))
}

#[test]
fn the_native_record_is_bound_to_the_workbook_inputs_and_helpers() {
    let native = native_results();
    for (field, path) in [
        ("workbook_sha256", "ozzit.xlsx"),
        ("cases_sha256", NATIVE_CASES),
        ("helper_sha256", "tools/excel_eval_formulas.ps1"),
        (
            "evidence_helper_sha256",
            "tools/excel_eval_formulas_verified.ps1",
        ),
    ] {
        assert_eq!(native[field], sha256(path), "field={field}");
    }
    assert_native_run(&native);
    let cases = native_cases();
    let by_id = by_id(&cases);
    let results: BTreeSet<&str> = object(&native["results"], "results")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(results, by_id.keys().copied().collect());
    assert_eq!(cases.len(), by_id.len());
}

#[test]
fn positive_periods_match_declared_rational_terms_and_apr() {
    let (cases, native) = (native_cases(), native_results());
    let by_id = by_id(&cases);
    let mut values = 0;
    for case in &cases {
        let id = case_id(case);
        let reference = match (case.get("equivalent_to"), case.get("terms")) {
            (Some(other), _) => by_id[text(other, id)],
            (None, Some(_)) => case,
            (None, None) => continue,
        };
        let terms = array(&reference["terms"], id);
        let apr: Vec<f64> = case.get("apr").map_or_else(
            || vec![1.0; terms.len()],
            |apr| array(apr, id).iter().map(|rate| number(rate, id)).collect(),
        );
        assert_eq!(apr.len(), terms.len(), "case={id}");
        let expected = apr.iter().zip(terms).map(|(rate, period)| {
            let parts = array(period, id).iter().map(|term| {
                let [n, d] = array(term, id) else {
                    panic!("case={id}: {term} is not n/d")
                };
                number(n, id) / number(d, id)
            });
            rate * parts.sum::<f64>()
        });
        let rows = array(result(&native, id), id);
        let [actual] = rows else {
            panic!("case={id}: {} rows", rows.len())
        };
        let actual = array(actual, id);
        let expected: Vec<f64> = expected.collect();
        assert_eq!(actual.len(), expected.len(), "case={id}");
        for (value, wanted) in actual.iter().zip(expected) {
            let value = number(value, id);
            assert!(value.is_finite(), "case={id}");
            assert_within(value, wanted, 1e-12, &format!("case={id}"));
            values += 1;
        }
    }
    assert_eq!(values, 312);
}

#[test]
fn qualification_keeps_observations_separate_and_covers_the_edges() {
    let cases = native_cases();
    for case in &cases {
        let id = case_id(case);
        let categories = ["terms", "equivalent_to", "error_control", "observation"]
            .into_iter()
            .filter(|key| case.get(key).is_some())
            .count();
        assert_eq!(categories, 1, "case={id}");
        assert!(text(&case["formula"], id).starts_with('='), "case={id}");
    }
    assert_eq!(cases.len(), 113);
    let observations = cases
        .iter()
        .filter(|case| case.get("observation").is_some());
    assert_eq!(observations.count(), 23);
    let tags: BTreeSet<&str> = cases
        .iter()
        .filter_map(|case| case.get("coverage"))
        .flat_map(|tags| array(tags, "coverage"))
        .map(|tag| text(tag, "coverage"))
        .collect();
    assert_eq!(tags, BTreeSet::from(COVERAGE));
    let by_id = by_id(&cases);
    for schedule in SCHEDULES {
        for method in 1..=4 {
            let id = format!("{schedule}_{method}");
            let case = by_id
                .get(id.as_str())
                .unwrap_or_else(|| panic!("no case {id}"));
            assert!(case.get("terms").is_some(), "case={id}");
        }
    }
}

#[test]
fn invalid_numeric_conventions_return_the_native_value_error() {
    let (cases, native) = (native_cases(), native_results());
    assert_eq!(
        *result(&native, "value_error"),
        serde_json::json!([["#ERR:-2146826273"]])
    );
    assert_eq!(
        *result(&native, "na_error"),
        serde_json::json!([["#ERR:-2146826246"]])
    );
    for case in &cases {
        if let Some(control) = case.get("error_control") {
            let id = case_id(case);
            assert_eq!(
                result(&native, id),
                result(&native, text(control, id)),
                "case={id}"
            );
        }
    }
}

#[test]
fn descending_cross_year_observation_does_not_qualify_signed_isda() {
    let cases = native_cases();
    let id = "reverse_cross_year_4";
    assert!(by_id(&cases)[id].get("observation").is_some());
    let actual = number(&result(&native_results(), id)[0][0], id);
    // 15 December to 1 January is 17 common-year days; 1 January to
    // 15 January is 14 leap-year days. Reversing the interval negates both.
    let signed_isda = -(17.0 / 365.0 + 14.0 / 366.0);
    assert_within(actual, -31.0 / 366.0, 1e-12, id);
    assert!(
        (actual - signed_isda).abs() > 1e-4,
        "{actual} is the signed ISDA {signed_isda}"
    );
}

// The shipped LAMBDA must still spell the rules the mirror above restates.

/// The published `DayCountRateλ` definition, which must be the only one.
fn definition() -> String {
    let table = json(&read(
        "tools/ozzit/src/data/postbuild/rate_date_helpers.json",
    ));
    let definitions: Vec<&str> = array(&table["functions"], "functions")
        .iter()
        .filter_map(|row| {
            let [module, name, _, source] = array(row, "a function row") else {
                panic!("{row} is not a function row")
            };
            (module == "Financial" && name == "DayCountRateλ").then(|| text(source, "source"))
        })
        .collect();
    let [definition] = definitions.as_slice() else {
        panic!("{} DayCountRateλ definitions", definitions.len())
    };
    (*definition).to_owned()
}

fn assert_pinned(lines: &[&str]) {
    let source = spaced(&definition());
    for line in lines {
        assert!(
            source.contains(line),
            "{line:?} is not in the published DayCountRateλ"
        );
    }
}

#[test]
fn the_builder_carries_the_shipped_definition() {
    assert!(read("src/Financial.txt").contains(&definition()));
}

#[test]
fn the_convention_numbers_match_the_constants_used_here() {
    assert_pinned(&[
        "(Optional: Default = 3) 1 = 30/360, 2 = Actual/360, 3 = Actual/365, \
                     4 = Actual/Actual.",
    ]);
    assert_eq!(
        (THIRTY_360, ACTUAL_360, ACTUAL_365, ACTUAL_ACTUAL),
        (1, 2, 3, 4)
    );
}

#[test]
fn the_thirty_three_sixty_day_count_is_the_european_rule() {
    assert_pinned(&["Days, IF(Method = 1, 360 * (YEAR(Next) - YEAR(Starts)) \
                     + 30 * (MONTH(Next) - MONTH(Starts)) \
                     + IF(DAY(Next) > 30, 30, DAY(Next)) - IF(DAY(Starts) > 30, 30, DAY(Starts)), \
                     Next - Starts)"]);
}

#[test]
fn the_actual_actual_terms_split_at_new_year_on_each_year_length() {
    assert_pinned(&[
        "Boundary, DATE(YearOne + 1, 1, 1)",
        "Split, IF(Next < Boundary, Next, Boundary)",
        "DaysOne, IF(DAY(DATE(YearOne, 2, 29)) = 29, 366, 365)",
        "DaysLast, DATE(YearLast + 1, 1, 1) - LastYearStart",
    ]);
}

#[test]
fn each_convention_keeps_its_denominator() {
    assert_pinned(&[
        "Next, Ends + 1",
        "Fraction, SWITCH(Method, 1, Days / 360, 2, Days / 360, \
         4, (Split - Starts) / DaysOne + IF(YearLast > YearOne, \
         YearLast - YearOne - 1 + (Next - LastYearStart) / DaysLast, 0), \
         Days / 365)",
        "Rates, Rate * Fraction",
    ]);
}
