// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! `ozzit audit` reports hard-coded numbers inside formula runs, external links
//! whose target is missing, and hidden names and sheets, without writing.
use ozzit::archive::{Parts, deterministic_bytes};
use std::path::Path;
use std::process::Command;

const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const LINK_PATH: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLinkPath";
const PATH_MISSING: &str =
    "http://schemas.microsoft.com/office/2006/relationships/xlExternalLinkPath/xlPathMissing";

struct Book {
    sheets: Vec<(&'static str, &'static str, String)>,
    names: String,
    links: Vec<(&'static str, String)>,
}

impl Book {
    fn new() -> Self {
        Self {
            sheets: vec![],
            names: String::new(),
            links: vec![],
        }
    }
    /// A sheet: its name, its `state` attribute (empty for visible) and its rows.
    fn sheet(mut self, name: &'static str, state: &'static str, rows: &str) -> Self {
        self.sheets.push((name, state, rows.to_owned()));
        self
    }
    fn names(mut self, names: &str) -> Self {
        self.names = names.to_owned();
        self
    }
    /// An external link part with one relationship of `kind` to `target`.
    fn link(mut self, kind: &'static str, target: &str) -> Self {
        self.links.push((kind, target.to_owned()));
        self
    }
    fn parts(&self) -> Parts {
        let mut parts = Parts::new();
        let mut sheets = String::new();
        let mut rels = String::new();
        for (i, (name, state, rows)) in self.sheets.iter().enumerate() {
            let n = i + 1;
            let state = if state.is_empty() {
                String::new()
            } else {
                format!(" state=\"{state}\"")
            };
            sheets.push_str(&format!(
                "<sheet name=\"{name}\" sheetId=\"{n}\"{state} r:id=\"rId{n}\"/>"
            ));
            rels.push_str(&format!(
                "<Relationship Id=\"rId{n}\" Type=\"{REL}/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"
            ));
            parts.insert(
                format!("xl/worksheets/sheet{n}.xml"),
                format!("<worksheet xmlns=\"{MAIN}\"><sheetData>{rows}</sheetData></worksheet>")
                    .into_bytes(),
            );
        }
        let external = self.link_parts(&mut parts, &mut rels);
        let names = if self.names.is_empty() {
            String::new()
        } else {
            format!("<definedNames>{}</definedNames>", self.names)
        };
        parts.insert(
            "xl/workbook.xml".into(),
            format!("<workbook xmlns=\"{MAIN}\" xmlns:r=\"{REL}\"><sheets>{sheets}</sheets>{external}{names}</workbook>")
                .into_bytes(),
        );
        parts.insert(
            "xl/_rels/workbook.xml.rels".into(),
            format!("<Relationships xmlns=\"{PKG}\">{rels}</Relationships>").into_bytes(),
        );
        parts
    }
    /// The external link parts, their workbook relationships and the
    /// `externalReferences` element.
    fn link_parts(&self, parts: &mut Parts, rels: &mut String) -> String {
        let mut external = String::new();
        for (i, (kind, target)) in self.links.iter().enumerate() {
            let n = i + 1;
            external.push_str(&format!("<externalReference r:id=\"rIdX{n}\"/>"));
            rels.push_str(&format!(
                "<Relationship Id=\"rIdX{n}\" Type=\"{REL}/externalLink\" Target=\"externalLinks/externalLink{n}.xml\"/>"
            ));
            parts.insert(
                format!("xl/externalLinks/externalLink{n}.xml"),
                format!("<externalLink xmlns=\"{MAIN}\" xmlns:r=\"{REL}\"><externalBook r:id=\"rId1\"/></externalLink>")
                    .into_bytes(),
            );
            parts.insert(
                format!("xl/externalLinks/_rels/externalLink{n}.xml.rels"),
                format!("<Relationships xmlns=\"{PKG}\"><Relationship Id=\"rId1\" Type=\"{kind}\" Target=\"{target}\" TargetMode=\"External\"/></Relationships>")
                    .into_bytes(),
            );
        }
        if external.is_empty() {
            external
        } else {
            format!("<externalReferences>{external}</externalReferences>")
        }
    }
    fn write(&self, dir: &Path) -> std::path::PathBuf {
        let path = dir.join("book.xlsx");
        std::fs::write(&path, deterministic_bytes(&self.parts()).unwrap()).unwrap();
        path
    }
}

fn audit(args: &[&str], dir: &Path) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .arg("audit")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    let text =
        String::from_utf8(output.stdout).unwrap() + &String::from_utf8(output.stderr).unwrap();
    (output.status.code().unwrap(), text.replace("\r\n", "\n"))
}

fn findings(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| line.strip_prefix("  - "))
        .collect()
}

const CLEAN_ROW: &str = r#"<row r="2"><c r="A2"><v>1</v></c><c r="B2"><f>A2*2</f><v>2</v></c><c r="C2"><f>B2*2</f><v>4</v></c><c r="D2"><f>C2*2</f><v>8</v></c></row>"#;

#[test]
fn a_workbook_without_findings_passes() {
    let dir = tempfile::tempdir().unwrap();
    Book::new().sheet("Model", "", CLEAN_ROW).write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!((code, text.as_str()), (0, "OK: book.xlsx, no findings\n"));
}

#[test]
fn a_number_between_formulas_of_one_pattern_in_a_row_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let rows = r#"<row r="2"><c r="A2"><v>1</v></c><c r="B2"><f>A2*2</f><v>2</v></c><c r="C2"><v>5</v></c><c r="D2"><f>C2*2</f><v>10</v></c></row>"#;
    Book::new().sheet("Model", "", rows).write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert!(
        text.starts_with("FOUND: 1 finding(s) in book.xlsx\n"),
        "{text}"
    );
    assert_eq!(
        findings(&text),
        ["hard-coded number: Model!C2 holds 5 between B2 and D2, formulas of one pattern"]
    );
}

#[test]
fn a_number_inside_a_shared_formula_column_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let rows = r#"<row r="2"><c r="B2"><f t="shared" ref="B2:B5" si="0">A2+1</f><v>1</v></c></row><row r="3"><c r="B3"><f t="shared" si="0"/><v>1</v></c></row><row r="4"><c r="B4" t="n"><v>7</v></c></row><row r="5"><c r="B5"><f t="shared" si="0"/><v>1</v></c></row>"#;
    Book::new().sheet("Cash flow", "", rows).write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        ["hard-coded number: 'Cash flow'!B4 holds 7 between B3 and B5, formulas of one pattern"]
    );
}

#[test]
fn absolute_references_keep_their_row_and_column() {
    let dir = tempfile::tempdir().unwrap();
    // B2 and D2 read the same absolute input; C2 overrides it.
    let rows = r#"<row r="2"><c r="B2"><f>$A$1*B1</f><v>0</v></c><c r="C2"><v>3</v></c><c r="D2"><f>$A$1*D1</f><v>0</v></c></row>"#;
    Book::new().sheet("Model", "", rows).write(dir.path());
    let (_, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(findings(&text).len(), 1, "{text}");
}

#[test]
fn whole_column_and_row_references_compare_by_position() {
    let dir = tempfile::tempdir().unwrap();
    let rows = concat!(
        r#"<row r="2"><c r="B2"><f>SUM(A:A)</f><v>0</v></c><c r="C2"><v>4</v></c><c r="D2"><f>SUM(C:C)</f><v>0</v></c></row>"#,
        r#"<row r="3"><c r="F3"><f>SUM(2:2)</f><v>0</v></c></row>"#,
        r#"<row r="4"><c r="F4"><v>6</v></c></row>"#,
        r#"<row r="5"><c r="F5"><f>SUM(4:4)</f><v>0</v></c></row>"#,
    );
    Book::new().sheet("Model", "", rows).write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "hard-coded number: Model!C2 holds 4 between B2 and D2, formulas of one pattern",
            "hard-coded number: Model!F4 holds 6 between F3 and F5, formulas of one pattern",
        ]
    );
}

#[test]
fn numbers_beside_unlike_formulas_text_or_function_names_are_not_reported() {
    let dir = tempfile::tempdir().unwrap();
    let rows = concat!(
        // Unlike patterns either side.
        r#"<row r="2"><c r="B2"><f>A2*2</f><v>0</v></c><c r="C2"><v>5</v></c><c r="D2"><f>SUM(A1:A3)</f><v>0</v></c></row>"#,
        // Text, a Boolean and an error between like formulas.
        r#"<row r="3"><c r="B3"><f>A3*2</f><v>0</v></c><c r="C3" t="s"><v>0</v></c><c r="D3"><f>C3*2</f><v>0</v></c></row>"#,
        r#"<row r="4"><c r="B4"><f>A4*2</f><v>0</v></c><c r="C4" t="b"><v>1</v></c><c r="D4"><f>C4*2</f><v>0</v></c></row>"#,
        // LOG10 is a function, not cell LOG10, so these two patterns differ.
        r#"<row r="5"><c r="B5"><f>LOG10(A5)</f><v>0</v></c><c r="C5"><v>9</v></c><c r="D5"><f>LOG10(A5)</f><v>0</v></c></row>"#,
        // A reference inside a string literal or a quoted sheet name is text.
        r#"<row r="6"><c r="B6"><f>"A6"&amp;'Q1 A6'!A6</f><v>0</v></c><c r="C6"><v>9</v></c><c r="D6"><f>"A6"&amp;'Q1 A6'!C6</f><v>0</v></c></row>"#,
    );
    Book::new().sheet("Model", "", rows).write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(
        (code, findings(&text)),
        (
            1,
            vec!["hard-coded number: Model!C6 holds 9 between B6 and D6, formulas of one pattern"]
        ),
        "{text}"
    );
}

#[test]
fn external_links_report_a_missing_or_unchecked_target() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Present.xlsx"), b"").unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/In%20Sub.xlsx"), b"").unwrap();
    let present = dir.path().join("Present.xlsx");
    let present = present.display().to_string().replace('\\', "/");
    let absolute = format!(
        "file://{}{present}",
        if present.starts_with('/') { "" } else { "/" }
    );
    Book::new()
        .sheet("Model", "", CLEAN_ROW)
        .link(LINK_PATH, "Present.xlsx")
        .link(LINK_PATH, &absolute)
        .link(LINK_PATH, "Gone.xlsx")
        .link(LINK_PATH, "sub/In%2520Sub.xlsx")
        .link(PATH_MISSING, "Present.xlsx")
        .link(LINK_PATH, "https://example.invalid/Book.xlsx")
        .write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "external link: xl/externalLinks/externalLink3.xml targets Gone.xlsx, which does not exist",
            "external link: xl/externalLinks/externalLink5.xml targets Present.xlsx, which Excel marked missing",
            "external link: xl/externalLinks/externalLink6.xml targets https://example.invalid/Book.xlsx, a remote file not checked",
        ]
    );
}

#[test]
fn hidden_names_and_sheets_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let names = concat!(
        r#"<definedName name="Shown">Model!$A$1</definedName>"#,
        r#"<definedName name="Secret" hidden="1">Model!$A$2</definedName>"#,
        r#"<definedName name="_xlnm._FilterDatabase" localSheetId="1" hidden="1">Calc!$A$1:$B$2</definedName>"#,
    );
    Book::new()
        .sheet("Model", "", CLEAN_ROW)
        .sheet("Calc", "hidden", "")
        .sheet("Keys", "veryHidden", "")
        .names(names)
        .write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "hidden sheet: Calc",
            "very hidden sheet: Keys",
            "hidden name: Secret",
            "hidden name: _xlnm._FilterDatabase (sheet Calc)",
        ]
    );
}

#[test]
fn the_audit_does_not_change_the_workbook() {
    let dir = tempfile::tempdir().unwrap();
    let path = Book::new()
        .sheet("Calc", "hidden", CLEAN_ROW)
        .write(dir.path());
    let before = std::fs::read(&path).unwrap();
    audit(&["book.xlsx"], dir.path());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn usage_and_unreadable_input_fail() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        audit(&["a", "b"], dir.path()),
        (1, "FAIL: usage: ozzit audit [workbook]\n".into())
    );
    let (code, text) = audit(&["missing.xlsx"], dir.path());
    assert_eq!(code, 1);
    assert!(
        text.starts_with("FAIL: cannot read missing.xlsx: "),
        "{text}"
    );
}
