//! Physical lease apply, refusal and final idempotency contracts from Python.
use ozzit as tooling;
#[path = "working_capital_retirement/mod.rs"]
#[allow(dead_code)]
mod copies;
#[path = "leases_retirement/mod.rs"]
mod fixtures;
use fixtures::*;
use tooling::{
    archive::{BOOK, read_parts},
    common::{captures, display, unescape},
    formula::canonical,
};

fn cli(f: &Fixture, args: &[String]) -> (i32, String, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .current_dir(f.directory.path())
        .arg("aasb16_leases")
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
fn assert_definitions(book: &str) {
    let shipped =
        String::from_utf8(read_parts(&root().join("ozzit.xlsx")).unwrap()[BOOK].clone()).unwrap();
    for row in functions() {
        let pattern = format!(
            r#"(?s)<definedName name="oz.{}" comment="([^"]*)">(.*?)<"#,
            row[0]
        );
        let ours = captures(&pattern, book);
        let theirs = captures(&pattern, &shipped);
        assert_eq!(ours.len(), 1);
        assert_eq!(theirs.len(), 1);
        assert_eq!(ours[0][1], theirs[0][1]);
        assert_eq!(
            canonical(&unescape(&ours[0][2]).unwrap()),
            canonical(&unescape(&theirs[0][2]).unwrap())
        );
    }
    assert_eq!(
        captures(r#"<definedName name="([^"]+)""#, book),
        captures(r#"<definedName name="([^"]+)""#, &shipped)
    );
}
#[test]
fn a_reverted_copy_gets_all_four_stores_back() {
    let f = absent();
    let heading = data()["heading"].as_str().unwrap().to_string();
    assert!(!source(&f).contains(&heading));
    assert!(
        !String::from_utf8(read_parts(&f.book).unwrap()[BOOK].clone())
            .unwrap()
            .contains(&heading)
    );
    std::fs::write(f.src.join("Notes.txt"), "not a definition;\n").unwrap();
    assert_eq!(
        cli(&f, &f.args()),
        (
            0,
            format!(
                "added the AASB 16 lease functions to: src/Financial.txt, workbook, functions.csv\nOK: {}\n",
                display(&f.book)
            ),
            String::new()
        )
    );
    assert!(source(&f).contains(&heading));
    assert_eq!(
        std::fs::read(&f.index).unwrap(),
        std::fs::read(root().join("functions.csv")).unwrap()
    );
    let book = String::from_utf8(read_parts(&f.book).unwrap()[BOOK].clone()).unwrap();
    assert!(book.contains(&heading));
    assert_definitions(&book);
    let before = f.snapshot();
    assert_eq!(
        cli(&f, &f.args()),
        (
            0,
            format!(
                "AASB 16 lease functions already applied; no changes\nOK: {}\n",
                display(&f.book)
            ),
            String::new()
        )
    );
    assert_eq!(f.snapshot(), before);
}
#[test]
fn src_and_workbook_in_different_states_are_refused() {
    let f = absent();
    std::fs::copy(root().join("ozzit.xlsx"), &f.book).unwrap();
    refused(
        &f,
        &f.args(),
        "src/ is absent but the workbook is applied; the two views must not be applied separately",
    );
}
#[test]
fn a_missing_financial_module_fails_cleanly() {
    let f = Fixture::new();
    let path = f.src.join("Financial.txt");
    std::fs::remove_file(&path).unwrap();
    refused(
        &f,
        &f.args()[..2],
        &format!("src: missing {}", display(&path)),
    );
}
#[test]
fn too_many_arguments_print_the_usage() {
    let f = Fixture::new();
    refused(
        &f,
        &["a", "b", "c", "d"].map(String::from),
        "usage: ozzit aasb16_leases [workbook] [src dir] [functions.csv]",
    );
}
#[test]
fn a_missing_workbook_is_refused() {
    let f = Fixture::new();
    let missing = f.directory.path().join("absent.xlsx");
    refused(
        &f,
        &[display(&missing)],
        &format!("no such workbook: {}", display(&missing)),
    );
}
#[test]
fn each_xml_pass_is_a_byte_noop() {
    let f = Fixture::new();
    std::fs::remove_file(&f.index).unwrap();
    let before = f.snapshot();
    for _ in 0..2 {
        assert_eq!(
            cli(&f, &f.args()[..2]),
            (
                0,
                format!(
                    "note: {} not found, index not updated\nAASB 16 lease functions already applied; no changes\nOK: {}\n",
                    display(&f.index),
                    display(&f.book)
                ),
                String::new()
            )
        );
        assert_eq!(f.snapshot(), before);
    }
}
