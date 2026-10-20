//! Apply, refusal and physical CLI contracts migrated from the Python writer.
use ozzit as tooling;
#[path = "working_capital_retirement/mod.rs"]
mod fixtures;
use fixtures::*;
use tooling::{
    archive::{BOOK, read_parts, write_deterministic},
    common::{Csv, display},
};

fn cli(f: &Fixture, args: &[String]) -> (i32, String, String) {
    f.cli(env!("CARGO_BIN_EXE_ozzit"), args)
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
    let f = Fixture::absent();
    assert_eq!(
        cli(&f, &f.args()),
        (
            0,
            format!(
                "added the working-capital days functions to: src/Ratios.txt, workbook, functions.csv\nOK: {}\n",
                display(&f.book)
            ),
            String::new()
        )
    );
    assert_eq!(
        Csv::read(&f.index).unwrap().rows.len(),
        Csv::read(&root().join("functions.csv")).unwrap().rows.len()
    );
    assert_eq!(
        std::fs::read(f.src.join("Ratios.txt")).unwrap(),
        std::fs::read(root().join("src/Ratios.txt")).unwrap()
    );
    let parts = read_parts(&f.book).unwrap();
    let book = String::from_utf8(parts[BOOK].clone()).unwrap();
    for row in functions() {
        assert_eq!(
            book.matches(&format!("<definedName name=\"oz.{}\"", row[1]))
                .count(),
            1
        );
    }
    let before = f.snapshot();
    assert_eq!(
        cli(&f, &f.args()),
        (
            0,
            format!(
                "working-capital days functions already applied; no changes\nOK: {}\n",
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
    f.remove_index_names(&["oz.WIPDaysλ".into()]);
    refused(
        &f,
        &f.args(),
        "functions.csv holds 3 of 4 working-capital rows; it is neither state this pass recognises",
    );
}

#[test]
fn a_named_index_that_does_not_exist_is_refused() {
    let f = Fixture::absent();
    std::fs::remove_file(&f.index).unwrap();
    refused(
        &f,
        &f.args(),
        &format!(
            "{}: no such index; supply an existing functions.csv or none",
            display(&f.index)
        ),
    );
}

#[test]
fn no_index_cannot_apply_the_change() {
    let f = Fixture::absent();
    std::fs::remove_file(&f.index).unwrap();
    refused(
        &f,
        &f.args()[..2],
        "functions.csv is required to apply the working-capital days functions",
    );
}

#[test]
fn a_module_holding_part_of_the_change_is_refused() {
    let f = Fixture::new();
    let wip = &functions()[2][3];
    assert_eq!(f.source().matches(wip).count(), 1);
    std::fs::write(f.src.join("Ratios.txt"), f.source().replace(wip, "")).unwrap();
    refused(
        &f,
        &f.args(),
        "src/Ratios.txt holds 3 of 4 functions and 4 About rows; it is neither state this pass recognises",
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
                        "working-capital days functions already applied; no changes\nOK: {}\n",
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
fn run_refuses_a_missing_ratios_module() {
    let f = Fixture::new();
    let missing = f.src.join("Ratios.txt");
    std::fs::remove_file(&missing).unwrap();
    refused(
        &f,
        &f.args(),
        &format!("src: missing {}", display(&missing)),
    );
}

#[test]
fn run_refuses_stores_in_different_states() {
    let f = Fixture::new();
    std::fs::remove_file(&f.index).unwrap();
    write_deterministic(
        &f.book,
        &[(BOOK.into(), b"<workbook/>".to_vec())]
            .into_iter()
            .collect(),
    )
    .unwrap();
    refused(
        &f,
        &f.args()[..2],
        "the stores disagree (Ratios applied, workbook absent); the views must not be applied separately",
    );
}

#[test]
fn main_rejects_extra_arguments() {
    let f = Fixture::new();
    refused(
        &f,
        &["a.xlsx", "src", "functions.csv", "extra"].map(String::from),
        "usage: ozzit working_capital_days [workbook] [src dir] [functions.csv]",
    );
}

#[test]
fn main_rejects_a_missing_workbook() {
    let f = Fixture::new();
    let missing = f.directory.path().join("absent.xlsx");
    refused(
        &f,
        &[display(&missing), display(&f.src)],
        &format!("no such workbook: {}", display(&missing)),
    );
}
