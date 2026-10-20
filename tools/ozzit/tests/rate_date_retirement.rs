//! Physical CLI and applied/index-state contracts of the retired Python twin.
use ozzit as tooling;
#[path = "working_capital_retirement/mod.rs"]
#[allow(dead_code)] // Shared owned copies also contain working-capital-only helpers.
mod copies;
#[path = "rate_date_retirement/mod.rs"]
mod fixtures;
use fixtures::*;
use tooling::common::{Csv, display};

fn cli(f: &Fixture, args: &[String]) -> (i32, String, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .current_dir(f.directory.path())
        .arg("rate_date_helpers")
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
                "added the rate and date helpers to: src/Financial.txt, src/Dates.txt, workbook, functions.csv\nOK: {}\n",
                display(&f.book)
            ),
            String::new()
        )
    );
    assert_eq!(
        Csv::read(&f.index).unwrap().rows.len(),
        Csv::read(&root().join("functions.csv")).unwrap().rows.len()
    );
    assert_applied(&f);
    let before = f.snapshot();
    assert_eq!(
        cli(&f, &f.args()),
        (
            0,
            format!(
                "rate and date helpers already applied; no changes\nOK: {}\n",
                display(&f.book)
            ),
            String::new()
        )
    );
    assert_eq!(f.snapshot(), before);
}
#[test]
fn an_index_missing_one_helper_is_not_reported_as_applied() {
    let f = Fixture::new();
    remove_index_names(&f, &["oz.PeriodRateλ".into()]);
    refused(
        &f,
        &f.args(),
        "functions.csv holds 3 of 4 helper rows; it is neither state this pass recognises",
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
                        "rate and date helpers already applied; no changes\nOK: {}\n",
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
fn an_index_without_any_helper_row_disagrees_with_the_applied_stores() {
    let f = Fixture::new();
    remove_index_names(
        &f,
        &functions()
            .iter()
            .map(|r| format!("oz.{}", r[1]))
            .collect::<Vec<_>>(),
    );
    refused(
        &f,
        &f.args(),
        "the stores disagree (Financial applied, Dates applied, workbook applied, functions.csv absent); the views must not be applied separately",
    );
}
#[test]
fn the_cli_notes_missing_explicit_and_default_indexes_without_creating_them() {
    for explicit in [true, false] {
        let f = absent();
        std::fs::remove_file(&f.index).unwrap();
        let index = if explicit {
            f.directory.path().join("absent.csv")
        } else {
            f.index.clone()
        };
        let mut args = f.args()[..2].to_vec();
        if explicit {
            args.push(display(&index));
        }
        assert_eq!(
            cli(&f, &args),
            (
                0,
                format!(
                    "note: {} not found, index not updated\nadded the rate and date helpers to: src/Financial.txt, src/Dates.txt, workbook\nOK: {}\n",
                    display(&index),
                    display(&f.book)
                ),
                String::new()
            )
        );
        assert_applied(&f);
        assert!(!index.exists());
        let before = f.snapshot();
        assert_eq!(
            cli(&f, &args),
            (
                0,
                format!(
                    "rate and date helpers already applied; no changes\nOK: {}\n",
                    display(&f.book)
                ),
                String::new()
            )
        );
        assert_eq!(f.snapshot(), before);
    }
}
#[test]
fn main_refuses_extra_arguments() {
    let f = Fixture::new();
    refused(
        &f,
        &["a", "b", "c", "d"].map(String::from),
        "usage: ozzit rate_date_helpers [workbook] [src dir] [functions.csv]",
    );
}
#[test]
fn main_refuses_a_missing_workbook() {
    let f = Fixture::new();
    let missing = f.directory.path().join("missing.xlsx");
    refused(
        &f,
        &[display(&missing)],
        &format!("no such workbook: {}", display(&missing)),
    );
}
