//! Workbook, template, CLI and no-write contracts retained from Python.
#[path = "workbook_palette_retirement/mod.rs"]
mod fixtures;
use fixtures::*;
use ozzit::{archive::read_parts, common::display, postbuild::workbook_palette};

#[test]
fn pass_is_byte_noop_on_current_workbook() {
    let f = Fixture::new();
    let before = f.bytes();
    for _ in 0..2 {
        assert_eq!(
            f.success(false),
            format!(
                "Palette already applied; no changes\nOK: {}\n",
                display(&f.book)
            )
        );
        assert_eq!(f.bytes(), before);
    }
}

#[test]
fn pass_keeps_the_toc_warning_in_crimson() {
    let f = Fixture::new();
    f.success(false);
    let styles = text(&read_parts(&f.book).unwrap(), "xl/styles.xml");
    let dxfs = section(&styles, "dxfs");
    assert!(dxfs.contains("FFF0D9DE"));
    assert!(dxfs.contains("FF990024"));
}

#[test]
fn pass_refolds_a_reverted_green_font_and_mint_fill() {
    let f = Fixture::new();
    let mut parts = read_parts(&f.book).unwrap();
    let mut styles = text(&parts, "xl/styles.xml");
    let fonts = section(&styles, "fonts");
    assert!(fonts.contains(r#"rgb="FF66023C""#));
    styles = styles.replacen(
        &fonts,
        &fonts.replacen(r#"rgb="FF66023C""#, r#"rgb="FF006600""#, 1),
        1,
    );
    let fills = section(&styles, "fills");
    assert!(fills.contains(r#"rgb="FFE8D9E2""#));
    styles = styles.replacen(
        &fills,
        &fills.replacen(r#"rgb="FFE8D9E2""#, r#"indexed="42""#, 1),
        1,
    );
    set(&mut parts, "xl/styles.xml", styles);
    f.write(&parts);
    f.success(false);
    let written = read_parts(&f.book).unwrap();
    let styles = text(&written, "xl/styles.xml");
    assert!(!section(&styles, "fonts").contains(r#"rgb="FF006600""#));
    assert!(!section(&styles, "fills").contains(r#"indexed="42""#));
    f.assert_noop(false);
}

#[test]
fn pass_folds_rich_text_font_and_colour_in_shared_strings() {
    let f = Fixture::new();
    f.success(false);
    let shared = text(&read_parts(&f.book).unwrap(), "xl/sharedStrings.xml");
    assert!(!shared.contains(r#"rFont val="Aptos Narrow""#));
    assert!(!shared.contains(r#"rgb="FF2E4950""#));
}

#[test]
fn pass_refolds_a_reverted_rich_text_run_in_shared_strings() {
    let f = Fixture::new();
    let mut parts = read_parts(&f.book).unwrap();
    let shared = text(&parts, "xl/sharedStrings.xml");
    assert!(shared.contains(r#"rgb="FF000000""#));
    assert!(shared.contains(r#"rFont val="Aptos""#));
    set(
        &mut parts,
        "xl/sharedStrings.xml",
        shared
            .replacen(r#"rgb="FF000000""#, r#"rgb="FF2E4950""#, 1)
            .replacen(r#"rFont val="Aptos""#, r#"rFont val="Aptos Narrow""#, 1),
    );
    f.write(&parts);
    f.success(false);
    let shared = text(&read_parts(&f.book).unwrap(), "xl/sharedStrings.xml");
    assert!(!shared.contains(r#"rFont val="Aptos Narrow""#));
    assert!(!shared.contains(r#"rgb="FF2E4950""#));
    assert!(shared.contains(r#"rgb="FF000000""#));
    f.assert_noop(false);
}

#[test]
fn run_sets_tabs_the_green_outline_and_the_input_fill() {
    let f = Fixture::new();
    let mut parts = palette_parts(DXFS);
    set(
        &mut parts,
        "xl/worksheets/sheet1.xml",
        r#"<worksheet><sheetPr><tabColor rgb="FF5C2D91"/></sheetPr></worksheet>"#.into(),
    );
    set(
        &mut parts,
        "xl/worksheets/sheet2.xml",
        "<worksheet/>".into(),
    );
    set(
        &mut parts,
        "xl/drawings/drawing1.xml",
        r#"<a:ln><a:solidFill><a:prstClr val="green"/></a:solidFill></a:ln>"#.into(),
    );
    set(&mut parts, "xl/drawings/drawing2.xml", "<xdr:wsDr/>".into());
    let styles = text(&parts, "xl/styles.xml").replace(
        "</fills>",
        &format!("<fill><patternFill>{INPUT_FILL}</patternFill></fill></fills>"),
    );
    set(&mut parts, "xl/styles.xml", styles);
    f.write(&parts);
    let changes = workbook_palette::run(&f.book, false).unwrap();
    assert_eq!(changes[0], "tab colours set (1 sheets)");
    assert!(changes.contains(&"xl/drawings/drawing1.xml: green outline made platinum".into()));
    assert!(changes.contains(&"input fills made platinum (1 entries)".into()));
    let written = read_parts(&f.book).unwrap();
    assert!(text(&written, "xl/worksheets/sheet1.xml").contains(r#"<tabColor rgb="FF66023C"/>"#));
    assert!(text(&written, "xl/drawings/drawing1.xml").contains(r#"<a:schemeClr val="accent3"/>"#));
    assert!(text(&written, "xl/styles.xml").contains(r#"<fgColor rgb="FFE5E4E2"/>"#));
    f.assert_noop(false);
}

#[test]
fn template_run_without_shared_strings_then_nothing_to_do() {
    let f = Fixture::new();
    let mut parts = palette_parts("<dxfs></dxfs>");
    parts.shift_remove("xl/sharedStrings.xml");
    let styles = text(&parts, "xl/styles.xml").replace(
        "</fonts>",
        r#"<font><color rgb="FFFFFFFF"/></font></fonts>"#,
    );
    set(&mut parts, "xl/styles.xml", styles);
    f.write(&parts);
    assert_eq!(
        workbook_palette::run(&f.book, true).unwrap(),
        [
            "xl/theme/theme1.xml: theme colours set",
            "white text made ivory (1 entries)",
        ]
    );
    f.assert_noop(true);
}

#[test]
fn run_folds_fonts_and_fills_then_finds_nothing_to_do() {
    let f = Fixture::new();
    f.write(&palette_parts(DXFS));
    assert_eq!(
        workbook_palette::run(&f.book, false).unwrap(),
        [
            "xl/theme/theme1.xml: theme colours set",
            "xl/styles.xml: 2 colours remapped",
            r#"font family <name val="Arial"/> -> <name val="Aptos"/>"#,
            r#"rich-text font <rFont val="Arial"/> -> <rFont val="Aptos"/>"#,
            "help-label fonts folded to palette (1 entries)",
            "help-block fills folded to ivory (1 entries)",
        ]
    );
    let parts = read_parts(&f.book).unwrap();
    assert!(text(&parts, "xl/theme/theme1.xml").contains(r#"val="66023C""#));
    let styles = text(&parts, "xl/styles.xml");
    for expected in [
        r#"<name val="Aptos"/><color rgb="FF66023C"/>"#,
        r#"<fgColor rgb="FFFFFFF0"/>"#,
        r#"<color rgb="FF990024"/>"#,
        r#"<bgColor rgb="FFF0D9DE"/>"#,
    ] {
        assert!(styles.contains(expected), "{expected}");
    }
    assert!(text(&parts, "xl/sharedStrings.xml").contains(r#"<rFont val="Aptos"/>"#));
    f.assert_noop(false);
}

#[test]
fn main_rejects_extra_arguments() {
    let f = Fixture::new();
    let before = f.bytes();
    f.refused(
        &["a.xlsx".into(), "b.xlsx".into()],
        "usage: ozzit workbook_palette [--template] [workbook]",
    );
    assert_eq!(f.bytes(), before);
}

#[test]
fn template_mode_skips_the_ozzit_folds_and_guard() {
    let f = Fixture::new();
    f.write(&palette_parts("<dxfs></dxfs>"));
    assert!(f.success(true).contains("theme colours set"));
    let styles = text(&read_parts(&f.book).unwrap(), "xl/styles.xml");
    assert!(styles.contains(r#"<name val="Arial"/><color rgb="FF006600"/>"#));
    assert!(styles.contains(r#"<fgColor rgb="FFCCFFCC"/>"#));
    f.assert_noop(true);
}

#[test]
fn main_rejects_a_missing_workbook() {
    let f = Fixture::new();
    let missing = f.directory.path().join("absent.xlsx");
    f.refused(
        &[display(&missing)],
        &format!("no such workbook: {}", display(&missing)),
    );
    assert!(!missing.exists());
}

#[test]
fn main_reports_a_remapped_toc_warning_without_writing() {
    let f = Fixture::new();
    f.write(&palette_parts("<dxfs></dxfs>"));
    let before = f.bytes();
    f.refused(&[display(&f.book)], "TOC warning colours are missing");
    assert_eq!(f.bytes(), before);
}

#[test]
fn main_reports_a_file_that_is_not_a_workbook() {
    let f = Fixture::new();
    std::fs::write(&f.book, b"not a zip archive").unwrap();
    f.refused(&[display(&f.book)], "File is not a zip file");
    assert_eq!(f.bytes(), b"not a zip archive");
}

#[test]
fn palette_does_not_claim_an_indexed_background_was_folded() {
    let f = Fixture::new();
    let mut parts = read_parts(&f.book).unwrap();
    let styles = text(&parts, "xl/styles.xml").replace(
        "</fills>",
        r#"<fill><patternFill><bgColor indexed="42"/></patternFill></fill></fills>"#,
    );
    set(&mut parts, "xl/styles.xml", styles);
    f.write(&parts);
    f.assert_noop(false);
}
