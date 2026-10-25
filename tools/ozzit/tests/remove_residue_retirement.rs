//! Whole-workbook and physical CLI contracts retained from Python residue tests.
#[path = "remove_residue_retirement/mod.rs"]
mod fixtures;
use fixtures::*;
use ozzit::{
    archive::{BOOK, read_parts},
    common::{captures, display},
    postbuild::{lookup, remove_residue, sheet_targets},
};
use std::{collections::BTreeSet, process::Command};

fn references(text: &str, pattern: &str) -> BTreeSet<usize> {
    captures(pattern, text)
        .into_iter()
        .map(|m| m[1].parse().unwrap())
        .collect()
}

#[test]
fn the_committed_workbook_has_no_residue_and_is_a_byte_noop() {
    let f = Fixture::new();
    let parts = read_parts(&f.workbook).unwrap();
    assert!(!parts.keys().any(|n| n.starts_with("xl/customProperty")));
    assert!(!text(&parts, BOOK).contains(r#"name="FMTs""#));
    assert!(!text(&parts, "xl/webextensions/webextension1.xml").contains("<we:extLst>"));
    let styles = text(&parts, "xl/styles.xml");
    let dxfs = number(&styles, r#"<dxfs count="(\d+)""#);
    let xfs = number(&styles, r#"<cellStyleXfs count="(\d+)""#);
    let mut used = BTreeSet::new();
    for (name, data) in &parts {
        if name.starts_with("xl/worksheets/sheet") || name.starts_with("xl/tables/table") {
            used.extend(references(
                std::str::from_utf8(data).unwrap(),
                r#"\b[A-Za-z]*[dD]xfId="(\d+)""#,
            ));
        }
    }
    let table_styles = captures(r"(?s)<tableStyles.*?</tableStyles>", &styles);
    assert_eq!(table_styles.len(), 1);
    used.extend(references(&table_styles[0][0], r#"dxfId="(\d+)""#));
    assert!(parts.keys().any(|n| n.starts_with("xl/tables/table")));
    assert_eq!(used, (0..dxfs).collect());
    let cell_xfs = captures(r"(?s)<cellXfs.*?</cellXfs>", &styles);
    assert_eq!(cell_xfs.len(), 1);
    let mut used_xfs = references(&cell_xfs[0][0], r#"xfId="(\d+)""#);
    used_xfs.insert(0);
    assert_eq!(used_xfs, (0..xfs).collect());
    assert_eq!(number(&styles, r#"<cellStyles count="(\d+)""#), xfs);
    let targets = sheet_targets(&parts).unwrap();
    for (name, column, split) in FREEZE {
        assert!(
            text(&parts, lookup(&targets, name).unwrap()).contains(&format!(
                r#"<pane xSplit="{split}" topLeftCell="{column}1""#
            ))
        );
    }
    assert!(remove_residue::run(&f.workbook).unwrap().is_empty());
    assert_eq!(
        std::fs::read(&f.workbook).unwrap(),
        std::fs::read(original()).unwrap()
    );
}

#[test]
fn all_residue_is_removed_and_every_part_is_restored_exactly() {
    let f = Fixture::new();
    let expected = read_parts(&f.workbook).unwrap();
    f.put_residue_back();
    assert_ne!(read_parts(&f.workbook).unwrap(), expected);
    assert_eq!(
        remove_residue::run(&f.workbook).unwrap(),
        [
            "removed the hidden FMTs sheet, its table and its 1 table part(s)",
            "removed 1 per-sheet custom properties",
            "removed the stale custom-function declaration from the Excel Labs reference",
            "dropped 1 unused differential formats",
            "dropped 1 unused named cell styles",
            "froze the label columns on 6 wide demonstration sheets",
        ]
    );
    let after = read_parts(&f.workbook).unwrap();
    assert_eq!(
        after, expected,
        "includes parts, styles, references and print-area indexes"
    );
    let before = std::fs::read(&f.workbook).unwrap();
    assert!(remove_residue::run(&f.workbook).unwrap().is_empty());
    assert_eq!(std::fs::read(&f.workbook).unwrap(), before);
}

#[test]
fn a_properties_list_that_disagrees_is_refused_before_writing() {
    let f = Fixture::new();
    let mut parts = read_parts(&f.workbook).unwrap();
    let book = text(&parts, BOOK).replacen(
        "<sheets>",
        r#"<sheets><sheet name="FMTs" sheetId="998" state="hidden" r:id="rId1"/>"#,
        1,
    );
    assert_ne!(book, text(&parts, BOOK));
    set(&mut parts, BOOK, book);
    f.write(&parts);
    let before = std::fs::read(&f.workbook).unwrap();
    assert_eq!(
        remove_residue::run(&f.workbook),
        Err("docProps/app.xml lists FMTs 0 times, expected 1".into())
    );
    assert_eq!(std::fs::read(&f.workbook).unwrap(), before);
}

fn run(f: &Fixture, args: &[&str], code: i32, out: &str, err: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .current_dir(f.directory.path())
        .arg("remove_residue")
        .args(args)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(code));
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        out.replace('\n', newline)
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        err.replace('\n', newline)
    );
}

#[test]
fn the_cli_refuses_extra_arguments_a_missing_workbook_and_a_non_archive() {
    let f = Fixture::new();
    let missing = f.directory.path().join("missing.xlsx");
    let invalid = f.directory.path().join("not-zip.xlsx");
    std::fs::write(&invalid, b"not a zip archive").unwrap();
    let before = std::fs::read(&f.workbook).unwrap();
    run(
        &f,
        &["a.xlsx", "b.xlsx"],
        1,
        "",
        "FAIL: usage: ozzit remove_residue [workbook]\n",
    );
    run(
        &f,
        &[&display(&missing)],
        1,
        "",
        &format!("FAIL: no such workbook: {}\n", display(&missing)),
    );
    run(
        &f,
        &[&display(&invalid)],
        1,
        "",
        "FAIL: File is not a zip file\n",
    );
    assert_eq!(std::fs::read(&f.workbook).unwrap(), before);
    assert_eq!(std::fs::read(&invalid).unwrap(), b"not a zip archive");
    assert!(!missing.exists());
}

#[test]
fn the_cli_prints_each_applied_change_and_restores_all_parts() {
    let f = Fixture::new();
    let expected = read_parts(&f.workbook).unwrap();
    let mut parts = expected.clone();
    unfreeze(&mut parts, "oz.IsOccurrenceDateλ");
    f.write(&parts);
    let path = display(&f.workbook);
    run(
        &f,
        &[&path],
        0,
        &format!("froze the label columns on 1 wide demonstration sheets\nOK: {path}\n"),
        "",
    );
    assert_eq!(read_parts(&f.workbook).unwrap(), expected);
    let before = std::fs::read(&f.workbook).unwrap();
    run(
        &f,
        &[],
        0,
        "residue already removed; no changes\nOK: ozzit.xlsx\n",
        "",
    );
    assert_eq!(std::fs::read(&f.workbook).unwrap(), before);
}
