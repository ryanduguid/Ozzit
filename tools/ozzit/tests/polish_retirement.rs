//! Original polish helper assertions and physical commands, using owned inputs.
#[path = "polish_retirement/mod.rs"]
mod fixtures;
use fixtures::*;
use ozzit::{archive, common::display, polish};

#[test]
fn every_rule_applies_once_and_a_second_run_is_a_no_op() {
    assert_eq!(polish::EXPECTED_STRAPLINES, STRAPLINES);
    assert_eq!(polish::COVER_OLD, COVER_OLD);
    assert_eq!(polish::COVER_NEW, COVER_NEW);
    let fixture = Fixture::new();
    let (code, stdout, stderr) = fixture.cli(&fixture.args());
    let mut changes = vec!["updated Cover formula-highlight wording".to_string()];
    changes.extend(STRAPLINES.map(|(name, _)| format!("set {name}!A2")));
    changes.extend([
        "replaced 1 mint fill definition".into(),
        "raised 2 strapline font colour".into(),
        "made 1 date styles locale-safe".into(),
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
    assert_eq!(
        stdout,
        format!(
            "{}\n{}",
            changes.join("\n"),
            fixture.status(&display(&fixture.book))
        )
    );
    assert_polished_parts(&fixture.parts());
    let before = std::fs::read(&fixture.book).unwrap();
    assert_eq!(polish::polish(&fixture.book).unwrap(), ["already polished"]);
    assert_eq!(std::fs::read(&fixture.book).unwrap(), before);
}

#[test]
fn too_many_arguments_print_the_usage() {
    let fixture = Fixture::new();
    refused(
        &fixture,
        &["a".into(), "b".into()],
        "FAIL: usage: ozzit polish_workbook [workbook]",
    );
}

#[test]
fn a_missing_workbook_is_refused() {
    let fixture = Fixture::new();
    let missing = fixture.directory.path().join("absent.xlsx");
    refused(
        &fixture,
        &[display(&missing)],
        &format!("FAIL: no such workbook: {}", display(&missing)),
    );
    assert!(!missing.exists());
}

#[test]
fn a_workbook_without_sheets_fails_cleanly() {
    let fixture = Fixture::new();
    let mut parts = fixture.parts();
    parts.insert(
        archive::BOOK.into(),
        format!("<workbook xmlns=\"{}\"/>", polish::MAIN_NS).into_bytes(),
    );
    archive::write_deterministic(&fixture.book, &parts).unwrap();
    refused(
        &fixture,
        &fixture.args(),
        &format!(
            "FAIL: cannot polish {}: workbook has no sheets collection",
            display(&fixture.book)
        ),
    );
}

#[test]
fn a_repointed_ratios_cell_adds_no_reference() {
    let (text, changed, inserted) =
        polish::add_financial_ratios_strapline(&sheet(true), 4).unwrap();
    assert_eq!((changed, inserted), (true, false));
    assert!(text.contains("<c r=\"A2\" s=\"1\" t=\"s\"><v>4</v></c>"));
}

#[test]
fn an_existing_ratios_merge_is_not_added_twice() {
    let sheet = sheet(false).replace(
        "<mergeCells count=\"1\"><mergeCell ref=\"A1:K1\"/>",
        "<mergeCells count=\"1\"><mergeCell ref=\"A2:K2\"/>",
    );
    let (text, changed, inserted) = polish::add_financial_ratios_strapline(&sheet, 3).unwrap();
    assert_eq!((changed, inserted), (true, true));
    assert_eq!(text.matches("<mergeCell ").count(), 1);
    assert!(text.contains("<c r=\"A2\" s=\"288\" t=\"s\"><v>3</v></c>"));
}

#[test]
fn a_ratios_sheet_without_rows_is_refused() {
    assert_eq!(
        polish::add_financial_ratios_strapline("<sheetData/>", 0).unwrap_err(),
        "could not insert oz.FinancialRatios row 2"
    );
}

#[test]
fn a_ratios_sheet_without_merges_is_refused() {
    assert_eq!(
        polish::add_financial_ratios_strapline("<sheetData><row r=\"1\"></row></sheetData>", 0)
            .unwrap_err(),
        "oz.FinancialRatios mergeCells not found"
    );
}

#[test]
fn a_sheet_without_a_shared_string_a2_is_refused() {
    assert_eq!(
        polish::set_existing_shared_cell("<c r=\"A2\"><v>1</v></c>", "A2", 0).unwrap_err(),
        "shared-string cell A2 not found"
    );
}

#[test]
fn styles_without_fonts_or_cell_formats_are_refused() {
    let styles = text(&Fixture::new().parts(), "xl/styles.xml");
    for (missing, message) in [
        ("fonts", "styles.xml fonts section missing"),
        ("cellXfs", "styles.xml cellXfs section missing"),
    ] {
        let changed = styles
            .replace(&format!("<{missing} "), &format!("<x{missing} "))
            .replace(&format!("</{missing}>"), &format!("</x{missing}>"));
        let mut parts = archive::Parts::from([("xl/styles.xml".into(), changed.into_bytes())]);
        assert_eq!(polish::update_styles(&mut parts).unwrap_err(), message);
    }
}

#[test]
fn polished_styles_are_left_alone() {
    let mut parts = archive::Parts::from([(
        "xl/styles.xml".into(),
        b"<fonts></fonts><cellXfs></cellXfs>".to_vec(),
    )]);
    let before = parts.clone();
    assert_eq!(
        polish::update_styles(&mut parts).unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(parts, before);
}

#[test]
fn polish_workbook_sheet_paths_resolve_absolute_and_relative_targets() {
    let parts = archive::Parts::from([
        (archive::BOOK.into(), format!("<workbook xmlns=\"{}\" xmlns:r=\"{}\"><sheets><sheet name=\"Data\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"Notes\" sheetId=\"2\" r:id=\"rId2\"/></sheets></workbook>", polish::MAIN_NS, polish::DOC_REL_NS).into_bytes()),
        ("xl/_rels/workbook.xml.rels".into(), b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"ws\" Target=\"/xl/worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"ws\" Target=\"worksheets/sheet2.xml\"/></Relationships>".to_vec()),
    ]);
    assert_eq!(
        polish::sheet_paths(&parts)
            .unwrap()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        [
            ("Data".into(), "xl/worksheets/sheet1.xml".into()),
            ("Notes".into(), "xl/worksheets/sheet2.xml".into()),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn polish_tool_is_idempotent_on_a_shipped_owned_copy() {
    let fixture = Fixture::shipped();
    let first = fixture.cli(&fixture.args());
    assert_eq!(first.0, 0, "{first:?}");
    assert_eq!(first.2, "");
    let before = std::fs::read(&fixture.book).unwrap();
    let second = fixture.cli(&fixture.args());
    assert_eq!(
        second,
        (
            0,
            format!(
                "already polished\n{}",
                fixture.status(&display(&fixture.book))
            ),
            String::new()
        )
    );
    assert_eq!(std::fs::read(&fixture.book).unwrap(), before);
}

#[test]
fn default_argument_selects_the_owned_workbook() {
    let fixture = Fixture::shipped();
    let before = std::fs::read(&fixture.book).unwrap();
    assert_eq!(
        fixture.cli(&[]),
        (
            0,
            format!("already polished\n{}", fixture.status("ozzit.xlsx")),
            String::new()
        )
    );
    assert_eq!(std::fs::read(&fixture.book).unwrap(), before);
}
