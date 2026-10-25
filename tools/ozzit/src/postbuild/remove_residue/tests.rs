//! Component contracts retained from the Python residue and OPC tests.
use super::*;

const STYLES: &str = concat!(
    "<styleSheet><cellStyleXfs count=\"1\"><xf numFmtId=\"0\"/></cellStyleXfs>",
    "<cellXfs count=\"1\"><xf xfId=\"0\"/></cellXfs>",
    "<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\"/></cellStyles>",
    "%s</styleSheet>"
);

fn style_parts(dxfs: &str, sheet: &str, template: &str) -> Parts {
    Parts::from_iter([
        (
            "xl/styles.xml".into(),
            template.replace("%s", dxfs).into_bytes(),
        ),
        ("xl/worksheets/sheet1.xml".into(), sheet.as_bytes().to_vec()),
    ])
}

#[test]
fn a_residue_sheet_without_its_relationship_is_refused() {
    let mut parts = Parts::from_iter([
        (BOOK.into(), br#"<workbook><sheets><sheet name="FMTs" sheetId="9" r:id="rId9"/></sheets></workbook>"#.to_vec()),
        ("xl/_rels/workbook.xml.rels".into(), b"<Relationships></Relationships>".to_vec()),
    ]);
    assert_eq!(
        drop_sheet(&mut parts, "FMTs", &mut vec![]),
        Err("FMTs: relationship rId9 missing".into())
    );
}

#[test]
fn an_absent_web_extension_preserves_parts_and_log() {
    let mut parts = Parts::from_iter([(BOOK.into(), b"<workbook/>".to_vec())]);
    let before = parts.clone();
    let mut log = vec![];
    drop_custom_function_declaration(&mut parts, &mut log).unwrap();
    assert_eq!(parts, before);
    assert!(log.is_empty());
}

#[test]
fn malformed_style_collections_are_refused() {
    let cases = [
        (
            style_parts("", "<worksheet/>", STYLES),
            "styles.xml has no dxfs collection",
        ),
        (
            style_parts(r#"<dxfs count="2"><dxf/></dxfs>"#, "<worksheet/>", STYLES),
            "styles.xml dxfs count disagrees with its entries",
        ),
        (
            style_parts(
                r#"<dxfs count="1"><dxf/></dxfs>"#,
                r#"<cfRule dxfId="1"/>"#,
                STYLES,
            ),
            "a dxfId points past the dxfs collection",
        ),
        (
            style_parts(
                r#"<dxfs count="0"></dxfs>"#,
                "<worksheet/>",
                "<styleSheet>%s</styleSheet>",
            ),
            "styles.xml lacks a cell style collection",
        ),
        (
            style_parts(
                r#"<dxfs count="0"></dxfs>"#,
                "<worksheet/>",
                &STYLES.replace(r#"<xf xfId="0"/>"#, r#"<xf xfId="1"/>"#),
            ),
            "an xfId points past the cellStyleXfs collection",
        ),
    ];
    for (mut parts, message) in cases {
        assert_eq!(prune_styles(&mut parts, &mut vec![]), Err(message.into()));
    }
}

#[test]
fn unused_formats_are_pruned_without_table_styles() {
    let mut parts = style_parts(
        r#"<dxfs count="2"><dxf><b/></dxf><dxf><i/></dxf></dxfs>"#,
        r#"<cfRule dxfId="1"/>"#,
        STYLES,
    );
    let mut log = vec![];
    prune_styles(&mut parts, &mut log).unwrap();
    assert_eq!(log, ["dropped 1 unused differential formats"]);
    assert_eq!(parts["xl/worksheets/sheet1.xml"], br#"<cfRule dxfId="0"/>"#);
    assert!(
        part_text(&parts, "xl/styles.xml")
            .unwrap()
            .contains(r#"<dxfs count="1"><dxf><i/></dxf></dxfs>"#)
    );
}

fn freeze_parts(drop_first: bool) -> Parts {
    let mut parts = Parts::new();
    let mut book = String::new();
    let mut rels = String::new();
    for (i, (name, _)) in FREEZE.iter().skip(usize::from(drop_first)).enumerate() {
        book.push_str(&format!(r#"<sheet name="{name}" r:id="rId{i}"/>"#));
        rels.push_str(&format!(
            r#"<Relationship Id="rId{i}" Target="worksheets/sheet{i}.xml"/>"#
        ));
        parts.insert(
            format!("xl/worksheets/sheet{i}.xml"),
            b"<worksheet/>".to_vec(),
        );
    }
    set_text(&mut parts, BOOK, book);
    set_text(&mut parts, "xl/_rels/workbook.xml.rels", rels);
    parts
}

#[test]
fn freeze_panes_refuse_a_missing_sheet_or_sheet_view() {
    for (drop_first, message) in [
        (true, "oz.IsOccurrenceDateλ is not in the workbook"),
        (false, "oz.IsOccurrenceDateλ has no sheetView"),
    ] {
        assert_eq!(
            freeze_panes(&mut freeze_parts(drop_first), &mut vec![]),
            Err(message.into())
        );
    }
}

fn book_parts(first: &str) -> Parts {
    Parts::from_iter([
        (BOOK.into(), format!(r#"<workbook><sheets><sheet name="{first}" sheetId="1" r:id="rId1"/><sheet name="Notes" sheetId="2" r:id="rId2"/></sheets></workbook>"#).into_bytes()),
        ("xl/_rels/workbook.xml.rels".into(), br#"<Relationships><Relationship Id="rId1" Type="ws" Target="/xl/worksheets/sheet1.xml"/><Relationship Id="rId2" Type="ws" Target="worksheets/sheet2.xml"/></Relationships>"#.to_vec()),
    ])
}

#[test]
fn absolute_and_relative_sheet_targets_resolve_together() {
    let targets = sheet_targets(&book_parts("Data")).unwrap();
    assert_eq!(targets.len(), 2);
    assert_eq!(
        lookup(&targets, "Data").unwrap(),
        "xl/worksheets/sheet1.xml"
    );
    assert_eq!(
        lookup(&targets, "Notes").unwrap(),
        "xl/worksheets/sheet2.xml"
    );
}

#[test]
fn dropping_an_absolute_sheet_target_removes_its_content_type_override() {
    let mut parts = book_parts("FMTs");
    parts.insert("xl/worksheets/sheet1.xml".into(), b"<worksheet/>".to_vec());
    parts.insert(
        "[Content_Types].xml".into(),
        br#"<Types><Override PartName="/xl/worksheets/sheet1.xml" ContentType="ws"/></Types>"#
            .to_vec(),
    );
    parts.insert(
        "docProps/app.xml".into(),
        b"<vt:lpstr>FMTs</vt:lpstr>".to_vec(),
    );
    drop_sheet(&mut parts, "FMTs", &mut vec![]).unwrap();
    assert!(!parts.contains_key("xl/worksheets/sheet1.xml"));
    assert_eq!(parts["[Content_Types].xml"], b"<Types></Types>");
}

#[test]
fn other_binary_parts_keep_the_default_content_type() {
    let content_type =
        br#"<Types><Default Extension="bin" ContentType="application/octet-stream"/></Types>"#;
    let printer = "xl/printerSettings/printerSettings1.bin";
    let mut parts = Parts::from_iter([
        ("xl/customProperty1.bin".into(), b"fabricated".to_vec()),
        (printer.into(), b"fabricated".to_vec()),
        ("[Content_Types].xml".into(), content_type.to_vec()),
    ]);
    drop_custom_properties(&mut parts, &mut vec![]).unwrap();
    assert_eq!(parts["[Content_Types].xml"], content_type);
    assert!(!parts.contains_key("xl/customProperty1.bin"));
    assert_eq!(parts[printer], b"fabricated");
}
