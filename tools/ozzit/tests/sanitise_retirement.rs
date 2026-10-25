//! Original sanitise pass contracts and physical CLI assertions on owned inputs.
#[path = "sanitise_retirement/mod.rs"]
mod fixtures;
use fixtures::*;
use ozzit::{archive, common, sanitise};

#[test]
fn canonical_clean_workbook_is_byte_identical() {
    let fixture = Fixture::shipped();
    fixture.write(&fixture.parts());
    fixture.repeat();
}

#[test]
fn clean_noncanonical_workbook_is_repacked() {
    let fixture = Fixture::shipped();
    fixture.noncanonical(&fixture.parts());
    let before = fixture.bytes();
    let log = sanitise::sanitise(&fixture.book).unwrap();
    assert!(log.contains(&"canonicalised archive metadata and compression".into()));
    assert_ne!(fixture.bytes(), before);
    canonical_metadata(&fixture.bytes());
}

#[test]
fn long_literal_split_by_an_excel_save_is_folded_back() {
    let fixture = Fixture::shipped();
    let before = fixture.bytes();
    let mut parts = fixture.parts();
    let expected = parts.clone();
    let book = text(&parts, archive::BOOK);
    let literals = common::captures(r#""((?:[^"]|"")*)""#, &book);
    let literal = &literals
        .iter()
        .max_by_key(|c| c[1].chars().count())
        .unwrap()[1];
    let chars: Vec<_> = literal.chars().collect();
    assert!(
        chars.len() > 255,
        "precondition: a literal Excel would split"
    );
    let cut = chars.len() / 2;
    let split = format!(
        "_xlfn._LONGTEXT(\"{}\",\"{}\")",
        chars[..cut].iter().collect::<String>(),
        chars[cut..].iter().collect::<String>()
    );
    let plain = format!("\"{literal}\"");
    assert_eq!(book.matches(&plain).count(), 1);
    parts.insert(
        archive::BOOK.into(),
        book.replacen(&plain, &split, 1).into_bytes(),
    );
    fixture.write(&parts);
    assert_ne!(fixture.bytes(), before);
    let log = sanitise::sanitise(&fixture.book).unwrap();
    assert!(log.contains(&"folded 1 _LONGTEXT literal(s) in defined names".into()));
    assert_eq!(fixture.parts(), expected);
    assert_eq!(
        sanitise::fold_longtext(r#"_xlfn._LONGTEXT("a""b","c")"#),
        (r#""a""bc""#.into(), 1)
    );
    assert_eq!(
        sanitise::fold_longtext(r#""plain""#),
        (r#""plain""#.into(), 0)
    );
}

#[test]
fn strips_printer_settings_and_writes_deterministically() {
    let fixture = Fixture::shipped();
    let mut parts = fixture.parts();
    let printer = "xl/printerSettings/printerSettings99.bin";
    let rels = "xl/worksheets/_rels/sheet2.xml.rels";
    let sheet = "xl/worksheets/sheet2.xml";
    parts.insert(printer.into(), b"printer-specific bytes".to_vec());
    parts.insert(rels.into(), concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>",
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
        "<Relationship Id=\"rId99\" ",
        "Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/printerSettings\" ",
        "Target=\"../printerSettings/printerSettings99.bin\"/></Relationships>"
    ).as_bytes().to_vec());
    parts.insert(
        sheet.into(),
        text(&parts, sheet)
            .replacen("<pageSetup ", "<pageSetup r:id=\"rId99\" ", 1)
            .into_bytes(),
    );
    fixture.noncanonical(&parts);
    let log = sanitise::sanitise(&fixture.book).unwrap();
    for change in [
        "removed 1 printerSettings parts",
        "cleared pageSetup r:id on 1 sheets",
        "dropped 1 empty worksheet rels",
    ] {
        assert!(log.iter().any(|line| line == change), "{change}");
    }
    canonical_metadata(&fixture.bytes());
    let parts = fixture.parts();
    assert!(!parts.contains_key(printer));
    assert!(!parts.contains_key(rels));
    assert!(!text(&parts, sheet).contains("r:id=\"rId99\""));
    fixture.repeat();
}

#[test]
fn session_state_is_pinned_so_two_saves_agree() {
    let fixture = Fixture::shipped();
    let mut parts = fixture.parts();
    let expected = parts.clone();
    stale_core(&mut parts, true);
    stale_book(&mut parts);
    fixture.noncanonical(&parts);
    assert!(
        sanitise::sanitise(&fixture.book)
            .unwrap()
            .contains(&"pinned session state in 2 part(s)".into())
    );
    let after = fixture.parts();
    let core = text(&after, CORE);
    assert!(core.contains("<cp:lastModifiedBy>Ozzit project</cp:lastModifiedBy>"));
    assert!(core.contains(&format!("{STAMP}</dcterms:modified>")));
    let book = text(&after, archive::BOOK);
    assert!(!book.contains("<xr:revisionPtr"));
    assert!(!book.contains("rupBuild"));
    assert!(book.contains(&format!("<workbookView {WINDOW}")));
    assert_eq!(after, expected);
    fixture.repeat();
}

#[test]
fn session_state_is_pinned_without_a_creator() {
    let fixture = Fixture::shipped();
    let mut parts = fixture.parts();
    stale_core(&mut parts, false);
    fixture.noncanonical(&parts);
    assert!(
        sanitise::sanitise(&fixture.book)
            .unwrap()
            .contains(&"pinned session state in 1 part(s)".into())
    );
    let core = text(&fixture.parts(), CORE);
    for absent in ["Somebody Else", "<cp:lastModifiedBy>", STALE_DATE] {
        assert!(!core.contains(absent));
    }
    assert!(core.contains(&format!("{STAMP}</dcterms:modified>")));
    fixture.repeat();
}

#[test]
fn cli_reports_non_zip_input_without_traceback() {
    broken(&Fixture::shipped());
}

#[test]
fn cli_reports_missing_input_without_traceback() {
    missing(&Fixture::shipped());
}

#[test]
fn excel_save_residue_is_removed() {
    let fixture = Fixture::residue();
    assert_eq!(sanitise::sanitise(&fixture.book).unwrap(), residue_log());
    assert_eq!(fixture.parts(), residue_parts(true));
    fixture.repeat();
}

#[test]
fn usage_is_required() {
    let fixture = Fixture::residue();
    let before = fixture.bytes();
    for args in [vec![], vec!["a.xlsx".into(), "b.xlsx".into()]] {
        assert_eq!(
            fixture.cli(&args),
            (
                1,
                String::new(),
                "FAIL: usage: ozzit sanitise_workbook ozzit.xlsx\n".into()
            )
        );
        assert_eq!(fixture.bytes(), before);
    }
}

#[test]
fn missing_and_unreadable_workbooks_fail_cleanly() {
    let fixture = Fixture::residue();
    let before = fixture.bytes();
    missing(&fixture);
    broken(&fixture);
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn success_prints_each_change_and_the_final_size() {
    let fixture = Fixture::residue();
    let result = fixture.cli(&fixture.args());
    assert_eq!(
        result,
        (
            0,
            format!("{}\n{}", residue_log().join("\n"), fixture.status()),
            String::new()
        )
    );
    assert_eq!(fixture.parts(), residue_parts(true));
}

#[test]
fn window_geometry_is_replaced_after_unrelated_attributes() {
    let fixture = Fixture::shipped();
    let mut parts = fixture.parts();
    let book = common::sub(
        &common::bounded(r"<workbookView\b[^>]*/>"),
        &text(&parts, archive::BOOK),
        "<workbookView visibility=\"visible\" xWindow=\"99\" yWindow=\"88\" windowWidth=\"1\" windowHeight=\"2\"/>",
    );
    parts.insert(archive::BOOK.into(), book.into_bytes());
    fixture.write(&parts);
    sanitise::sanitise(&fixture.book).unwrap();
    let book = text(&fixture.parts(), archive::BOOK);
    let view = common::re(&common::bounded(r"<workbookView\b[^>]*/>"));
    let view = view.find(&book).unwrap().unwrap().as_str();
    assert_eq!(view.matches("xWindow=").count(), 1);
    assert!(view.contains("visibility=\"visible\""));
    assert!(view.contains(WINDOW));
}
