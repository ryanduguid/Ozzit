//! ATO apply, refusal and physical command contracts retained from Python.
use ozzit as tooling;
#[path = "working_capital_retirement/mod.rs"]
#[allow(dead_code)] // Shared owned copies also hold working-capital-only helpers.
mod copies;
#[path = "ato_retirement/mod.rs"]
mod fixtures;
use fixtures::*;
use tooling::common::display;

fn cli(f: &Fixture, args: &[String]) -> (i32, String, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .current_dir(f.directory.path())
        .arg("ato_benchmarks_rate_lookup")
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap().replace("\r\n", "\n"),
        String::from_utf8(out.stderr).unwrap().replace("\r\n", "\n"),
    )
}
fn refused(f: &Fixture, args: &[String], message: &str) {
    let before = f.snapshot();
    assert_eq!(
        cli(f, args),
        (1, String::new(), format!("FAIL: {message}\n"))
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
fn the_absent_input_is_one_the_pass_applies() {
    let f = absent();
    assert_eq!(
        cli(&f, &f.args()),
        (
            0,
            format!(
                "added the benchmark and rate functions to: src/Ratios.txt, src/Dates.txt, workbook, functions.csv\nOK: {}\n",
                display(&f.book)
            ),
            String::new()
        )
    );
    assert_applied(&f);
    let before = f.snapshot();
    assert_eq!(
        cli(&f, &f.args()),
        (
            0,
            format!(
                "benchmark and rate functions already applied; no changes\nOK: {}\n",
                display(&f.book)
            ),
            String::new()
        )
    );
    assert_eq!(f.snapshot(), before);
}
#[test]
fn an_index_missing_one_function_is_not_reported_as_applied() {
    let f = Fixture::new();
    f.remove_index_names(&["oz.RateAtDateλ".into()]);
    refused(
        &f,
        &f.args(),
        "functions.csv holds 5 of 6 benchmark and rate rows; it is neither state this pass recognises",
    );
}
#[test]
fn a_named_index_that_does_not_exist_is_refused() {
    let f = absent();
    std::fs::remove_file(&f.index).unwrap();
    refused(
        &f,
        &f.args(),
        &format!(
            "{}: no such index; supply an existing functions.csv or none",
            display(&f.index)
        ),
    );
    assert!(!f.index.exists());
}
#[test]
fn no_index_cannot_apply_the_change() {
    let f = absent();
    std::fs::remove_file(&f.index).unwrap();
    refused(
        &f,
        &f.args()[..2],
        "functions.csv is required to apply the benchmark and rate functions",
    );
    assert!(!f.index.exists());
}
#[test]
fn a_module_holding_part_of_the_change_is_refused() {
    let f = Fixture::new();
    let path = f.src.join("Ratios.txt");
    std::fs::write(
        &path,
        source(&f, "Ratios").replacen(&functions()[4][3], "", 1),
    )
    .unwrap();
    refused(
        &f,
        &f.args(),
        "src/Ratios.txt holds 4 of 5 functions and 5 About rows; it is neither state this pass recognises",
    );
}
#[test]
fn stores_in_different_states_are_refused() {
    let f = absent();
    std::fs::copy(root().join("src/Dates.txt"), f.src.join("Dates.txt")).unwrap();
    refused(
        &f,
        &f.args(),
        "the stores disagree (Ratios absent, Dates applied, workbook absent, functions.csv absent); the views must not be applied separately",
    );
}
#[test]
fn the_committed_stores_all_read_as_applied() {
    let f = Fixture::new();
    let before = f.snapshot();
    for args in [f.args(), f.args()[..2].to_vec()] {
        for _ in 0..2 {
            assert_eq!(
                cli(&f, &args),
                (
                    0,
                    format!(
                        "benchmark and rate functions already applied; no changes\nOK: {}\n",
                        display(&f.book)
                    ),
                    String::new()
                )
            );
            assert_eq!(f.snapshot(), before);
        }
    }
}
#[test]
fn main_rejects_extra_arguments() {
    let f = Fixture::new();
    refused(
        &f,
        &["a", "b", "c", "d"].map(String::from),
        "usage: ozzit ato_benchmarks_rate_lookup [workbook] [src dir] [functions.csv]",
    );
}
#[test]
fn main_rejects_a_missing_workbook() {
    let f = Fixture::new();
    let missing = f.directory.path().join("absent.xlsx");
    refused(
        &f,
        &[display(&missing)],
        &format!("no such workbook: {}", display(&missing)),
    );
}
