//! Owned workbook fixtures: reintroduce each residue without requiring Python.
use ozzit::{
    archive::{BOOK, Parts, read_parts, write_deterministic},
    common::{captures, sub, sub_fn},
    postbuild::{lookup, sheet_targets},
};
use std::path::{Path, PathBuf};

pub const FREEZE: [(&str, &str, usize); 6] = [
    ("oz.IsOccurrenceDateλ", "H", 7),
    ("oz.Amortiseλ", "I", 8),
    ("oz.SumAmortiseλ", "I", 8),
    ("oz.Depreciateλ", "L", 11),
    ("oz.SumPeriodsλ", "I", 8),
    ("oz.TimelineOffsetλ", "C", 2),
];
const DXF: &str = r#"\b([A-Za-z]*[dD]xfId=")(\d+)(")"#;

pub fn original() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ozzit.xlsx")
}
pub fn text(parts: &Parts, name: &str) -> String {
    String::from_utf8(parts[name].clone()).unwrap()
}
pub fn set(parts: &mut Parts, name: &str, text: String) {
    parts.insert(name.into(), text.into_bytes());
}
pub fn number(text: &str, pattern: &str) -> usize {
    let found = captures(pattern, text);
    assert_eq!(found.len(), 1);
    found[0][1].parse().unwrap()
}
pub fn bump(text: &str, pattern: &str) -> String {
    sub_fn(pattern, text, |m| {
        format!("{}{}{}", &m[1], m[2].parse::<usize>().unwrap() + 1, &m[3])
    })
}
fn replace_once(text: &str, anchor: &str, replacement: &str) -> String {
    assert_eq!(text.matches(anchor).count(), 1, "{anchor}");
    text.replacen(anchor, replacement, 1)
}

pub struct Fixture {
    pub directory: tempfile::TempDir,
    pub workbook: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let workbook = directory.path().join("ozzit.xlsx");
        std::fs::copy(original(), &workbook).unwrap();
        Self {
            directory,
            workbook,
        }
    }
    pub fn write(&self, parts: &Parts) {
        write_deterministic(&self.workbook, parts).unwrap();
    }
    pub fn put_residue_back(&self) {
        let mut parts = read_parts(&self.workbook).unwrap();
        add_hidden_sheet(&mut parts);
        add_custom_properties(&mut parts);
        add_styles(&mut parts);
        for (name, _, _) in FREEZE {
            unfreeze(&mut parts, name);
        }
        self.write(&parts);
    }
}

fn add_hidden_sheet(parts: &mut Parts) {
    parts.insert(
        "xl/worksheets/sheet99.xml".into(),
        parts["xl/worksheets/sheet3.xml"].clone(),
    );
    parts.insert("xl/worksheets/_rels/sheet99.xml.rels".into(), br#"<Relationships><Relationship Id="rId1" Target="../tables/table99.xml"/></Relationships>"#.to_vec());
    parts.insert(
        "xl/tables/table99.xml".into(),
        parts["xl/tables/table2.xml"].clone(),
    );
    let book = replace_once(
        &text(parts, BOOK),
        r#"<sheet name="Data Validation""#,
        r#"<sheet name="FMTs" sheetId="999" state="hidden" r:id="rId999"/><sheet name="Data Validation""#,
    );
    set(parts, BOOK, bump(&book, r#"(localSheetId=")(\d+)(")"#));
    let rels = replace_once(
        &text(parts, "xl/_rels/workbook.xml.rels"),
        "</Relationships>",
        r#"<Relationship Id="rId999" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet99.xml"/></Relationships>"#,
    );
    set(parts, "xl/_rels/workbook.xml.rels", rels);
    let app = replace_once(
        &text(parts, "docProps/app.xml"),
        "<vt:lpstr>Data Validation</vt:lpstr>",
        "<vt:lpstr>FMTs</vt:lpstr><vt:lpstr>Data Validation</vt:lpstr>",
    );
    let app = bump(
        &app,
        r"(<vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>)(\d+)(</vt:i4>)",
    );
    set(
        parts,
        "docProps/app.xml",
        bump(&app, r#"(<TitlesOfParts><vt:vector size=")(\d+)(")"#),
    );
    let types = replace_once(
        &text(parts, "[Content_Types].xml"),
        "</Types>",
        concat!(
            "<Override PartName=\"/xl/worksheets/sheet99.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>",
            "<Override PartName=\"/xl/tables/table99.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml\"/>",
            "<Default Extension=\"bin\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.customProperty\"/></Types>"
        ),
    );
    set(parts, "[Content_Types].xml", types);
}

fn add_custom_properties(parts: &mut Parts) {
    parts.insert(
        "xl/customProperty1.bin".into(),
        "Cover".encode_utf16().flat_map(u16::to_le_bytes).collect(),
    );
    parts.insert("xl/worksheets/_rels/sheet1.xml.rels".into(), br#"<Relationships><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customProperty" Target="../customProperty1.bin"/></Relationships>"#.to_vec());
    let cover = replace_once(
        &text(parts, "xl/worksheets/sheet1.xml"),
        "</worksheet>",
        r#"<customProperties><customPr name="Description" r:id="rId2"/></customProperties></worksheet>"#,
    );
    set(parts, "xl/worksheets/sheet1.xml", cover);
    let web = replace_once(
        &text(parts, "xl/webextensions/webextension1.xml"),
        "</we:webextension>",
        r#"<we:extLst><a:ext xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" uri="{D87F86FE-615C-45B5-9D79-34F1136793EB}"><we:containsCustomFunctions/></a:ext></we:extLst></we:webextension>"#,
    );
    set(parts, "xl/webextensions/webextension1.xml", web);
}

fn add_styles(parts: &mut Parts) {
    let mut styles = text(parts, "xl/styles.xml");
    let n = number(&styles, r#"<dxfs count="(\d+)""#);
    styles = replace_once(
        &styles,
        &format!("<dxfs count=\"{n}\">"),
        &format!("<dxfs count=\"{}\"><dxf><font><b/></font></dxf>", n + 1),
    );
    for name in parts.keys().cloned().collect::<Vec<_>>() {
        if name.starts_with("xl/worksheets/sheet") || name.starts_with("xl/tables/table") {
            set(parts, &name, bump(&text(parts, &name), DXF));
        }
    }
    let table_styles = captures(r"(?s)<tableStyles.*?</tableStyles>", &styles);
    assert_eq!(table_styles.len(), 1);
    styles = replace_once(
        &styles,
        &table_styles[0][0],
        &bump(&table_styles[0][0], DXF),
    );
    let n = number(&styles, r#"<cellStyleXfs count="(\d+)""#);
    styles = replace_once(
        &styles,
        "</cellStyleXfs>",
        r#"<xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
    );
    styles = replace_once(
        &styles,
        &format!("<cellStyleXfs count=\"{n}\""),
        &format!("<cellStyleXfs count=\"{}\"", n + 1),
    );
    let names = number(&styles, r#"<cellStyles count="(\d+)""#);
    styles = replace_once(
        &styles,
        "</cellStyles>",
        &format!("<cellStyle name=\"Unused\" xfId=\"{n}\"/></cellStyles>"),
    );
    styles = replace_once(
        &styles,
        &format!("<cellStyles count=\"{names}\""),
        &format!("<cellStyles count=\"{}\"", names + 1),
    );
    set(parts, "xl/styles.xml", styles);
}

pub fn unfreeze(parts: &mut Parts, name: &str) {
    let targets = sheet_targets(parts).unwrap();
    let target = lookup(&targets, name).unwrap();
    let sheet = text(parts, target);
    assert_eq!(captures(r"<pane [^>]*/>", &sheet).len(), 1);
    assert_eq!(
        captures(r#"<selection pane="topRight"[^>]*/>"#, &sheet).len(),
        1
    );
    let sheet = sub(r"<pane [^>]*/>", &sheet, "");
    set(
        parts,
        target,
        sub(r#"<selection pane="topRight"[^>]*/>"#, &sheet, ""),
    );
}
