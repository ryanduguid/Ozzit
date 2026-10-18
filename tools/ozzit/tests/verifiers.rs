use ozzit::{archive::*, cover, spills, workbook};
use std::path::Path;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn minimal_cover() -> Parts {
    [
        (BOOK, r#"<workbook><sheets><sheet name="Cover" r:id="rId7"/></sheets></workbook>"#),
        ("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId7" Target="/xl/worksheets/sheet9.xml"/></Relationships>"#),
        ("xl/worksheets/sheet9.xml", r#"<worksheet><c r="A3" t="s"><v>1</v></c></worksheet>"#),
        ("xl/sharedStrings.xml", r#"<sst><si><t>Version wrong - 1 functions - ignored</t></si><si><r><t> Version 16 September 2026 - </t></r><r><t>137 functions - Current&#160; </t></r></si></sst>"#),
    ].into_iter().map(|(n,b)| (n.into(), b.as_bytes().to_vec())).collect()
}

#[test]
fn cover_resolves_relationship_cell_and_rich_shared_string() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cover.xlsx");
    write_deterministic(&path, &minimal_cover()).unwrap();
    assert_eq!(
        cover::cover_label(&path).unwrap(),
        ("16 September 2026".into(), 137.into())
    );
}
#[test]
fn cover_reports_each_missing_or_invalid_reference_without_scanning_other_strings() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cover.xlsx");
    for (part, old, new, reason) in [
        (BOOK, "Cover", "Hidden", "declares no sheet named Cover"),
        (
            "xl/_rels/workbook.xml.rels",
            "rId7",
            "rId8",
            "has no relationship target",
        ),
        (
            "xl/worksheets/sheet9.xml",
            "t=\"s\"",
            "t=\"str\"",
            "is not a shared-string cell",
        ),
        (
            "xl/worksheets/sheet9.xml",
            "<v>1</v>",
            "<v/>",
            "carries no shared-string index",
        ),
        (
            "xl/worksheets/sheet9.xml",
            "<v>1</v>",
            "<v>99999999999999999999999999999999999999999</v>",
            "beyond the 2 stored",
        ),
        (
            "xl/sharedStrings.xml",
            "Version 16",
            "v16",
            "is not a version label",
        ),
    ] {
        let mut parts = minimal_cover();
        let value = text(&parts, part).unwrap().replace(old, new);
        parts.insert(part.into(), value.into_bytes());
        write_deterministic(&path, &parts).unwrap();
        let error = cover::cover_label(&path).unwrap_err();
        assert!(error.contains(reason), "{error}");
    }
}
#[test]
fn changelog_sig_decoding_and_newlines_keep_python_heading_rules() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("CHANGELOG.md");
    std::fs::write(
        &path,
        "\u{feff}# Changes\r## v3.4.2, 16 September 2026, title\r",
    )
    .unwrap();
    assert_eq!(cover::current_cut_date(&path).unwrap(), "16 September 2026");
    std::fs::write(&path, "\u{feff}\u{feff}## v3.4.2, 16 September 2026, title").unwrap();
    assert!(cover::current_cut_date(&path).is_err());
}
#[test]
fn cover_inventory_excludes_only_about_names_and_rejects_empty_names() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("functions.csv");
    std::fs::write(&path, "function\noz.AboutDatesλ\noz.Aλ\n").unwrap();
    assert_eq!(cover::published_functions(&path).unwrap(), 1);
    for (text, expected) in [
        ("function\n", "lists no functions"),
        ("function,function\n", "duplicate CSV column(s): function"),
        ("signature\na\n", "'function'"),
        ("module,function\nMod\n", "empty function name"),
    ] {
        std::fs::write(&path, text).unwrap();
        assert!(
            cover::published_functions(&path)
                .unwrap_err()
                .contains(expected)
        );
    }
}
#[test]
fn all_three_verifiers_pass_the_frozen_library_without_a_writer() {
    let path = root().join("ozzit.xlsx");
    let before = std::fs::read(&path).unwrap();
    let (parts, names) = read_inventory(&path).unwrap();
    assert_eq!(names.len(), 169);
    let report = workbook::check(&parts, &names);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(report.functions, 142);
    let (stale, anchors) = spills::check(&parts).unwrap();
    assert!(stale.is_empty());
    assert_eq!(anchors, 43);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
#[test]
fn xml_part_refuses_dtds_and_invalid_utf8_before_parsing() {
    for input in [
        b"<!DoCtYpE root><root/>".as_slice(),
        b"\xff<root/>",
        b"<root>",
    ] {
        assert!(workbook::xml_part("fixture.xml", input).is_err());
    }
}
#[test]
fn central_directory_duplicate_names_keep_their_inventory_count() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("duplicate.zip");
    let parts = [("part.txt".into(), b"payload".to_vec())]
        .into_iter()
        .collect();
    let bytes = deterministic_bytes(&parts).unwrap();
    let end = bytes.len() - 22;
    let start = u32::from_le_bytes(bytes[end + 16..end + 20].try_into().unwrap()) as usize;
    let mut duplicated = bytes[..end].to_vec();
    duplicated.extend_from_slice(&bytes[start..end]);
    let mut footer = bytes[end..].to_vec();
    footer[8..10].copy_from_slice(&2u16.to_le_bytes());
    footer[10..12].copy_from_slice(&2u16.to_le_bytes());
    footer[12..16].copy_from_slice(&u32::try_from((end - start) * 2).unwrap().to_le_bytes());
    duplicated.extend(footer);
    std::fs::write(&path, duplicated).unwrap();
    let (parts, names) = read_inventory(&path).unwrap();
    assert_eq!(names, vec!["part.txt", "part.txt"]);
    assert_eq!(parts["part.txt"], b"payload");
}

#[test]
fn a_directory_workbook_reports_the_filesystem_error_before_zip_parsing() {
    let directory = tempfile::tempdir().unwrap();
    let expected = ozzit::common::read_bytes(directory.path()).unwrap_err();
    assert_eq!(open_zip(directory.path()).err().unwrap(), expected);
}
