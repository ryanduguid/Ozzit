// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! `ozzit audit` reports hard-coded numbers inside formula runs, external links
//! whose target is missing, hidden names and sheets, unused names, buried checks,
//! rows a SUM total leaves out and circular references, without writing.
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
    calc: String,
    links: Vec<(&'static str, String)>,
    /// (sheet index, table name, range).
    tables: Vec<(usize, &'static str, &'static str)>,
}

impl Book {
    fn new() -> Self {
        Self {
            sheets: vec![],
            names: String::new(),
            calc: String::new(),
            links: vec![],
            tables: vec![],
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
    fn calc(mut self, calc: &str) -> Self {
        self.calc = calc.to_owned();
        self
    }
    /// An external link part with one relationship of `kind` to `target`.
    fn link(mut self, kind: &'static str, target: &str) -> Self {
        self.links.push((kind, target.to_owned()));
        self
    }
    /// A table on the sheet at `sheet` (from 0) over `range`.
    fn table(mut self, sheet: usize, name: &'static str, range: &'static str) -> Self {
        self.tables.push((sheet, name, range));
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
        self.table_parts(&mut parts);
        let external = self.link_parts(&mut parts, &mut rels);
        let names = if self.names.is_empty() {
            String::new()
        } else {
            format!("<definedNames>{}</definedNames>", self.names)
        };
        parts.insert(
            "xl/workbook.xml".into(),
            format!("<workbook xmlns=\"{MAIN}\" xmlns:r=\"{REL}\"><sheets>{sheets}</sheets>{external}{names}{}</workbook>", self.calc)
                .into_bytes(),
        );
        parts.insert(
            "xl/_rels/workbook.xml.rels".into(),
            format!("<Relationships xmlns=\"{PKG}\">{rels}</Relationships>").into_bytes(),
        );
        parts
    }
    /// Each table's part and its sheet's relationship to it.
    fn table_parts(&self, parts: &mut Parts) {
        for (i, (sheet, name, range)) in self.tables.iter().enumerate() {
            let n = i + 1;
            parts.insert(
                format!("xl/tables/table{n}.xml"),
                format!("<table xmlns=\"{MAIN}\" id=\"{n}\" name=\"{name}\" displayName=\"{name}\" ref=\"{range}\"/>")
                    .into_bytes(),
            );
            let rels = format!("xl/worksheets/_rels/sheet{}.xml.rels", sheet + 1);
            let mut body = parts
                .get(&rels)
                .map(|b| String::from_utf8(b.clone()).unwrap())
                .unwrap_or_else(|| format!("<Relationships xmlns=\"{PKG}\"></Relationships>"));
            body = body.replace(
                "</Relationships>",
                &format!("<Relationship Id=\"rT{n}\" Type=\"{REL}/table\" Target=\"../tables/table{n}.xml\"/></Relationships>"),
            );
            parts.insert(rels, body.into_bytes());
        }
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
fn the_shipped_workbooks_pass_the_audit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for book in [
        "ozzit.xlsx",
        "templates/13-week-cash-flow-forecast.xlsx",
        "templates/practice-calculators.xlsx",
        "templates/retention-reconciliation.xlsx",
    ] {
        let (code, text) = audit(&[book], &root);
        assert_eq!(code, 0, "{text}");
    }
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
fn mac_volume_paths_are_checked_as_mac_paths() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("On Mac.xlsx"), b"").unwrap();
    // The file's own path in volume-colon form, read from the startup disk.
    let present = dir.path().join("On Mac.xlsx").display().to_string();
    let present = present
        .split_once(':')
        .map_or(present.as_str(), |(_, rest)| rest)
        .replace(['/', '\\'], ":");
    let present = format!("Macintosh%20HD{present}");
    Book::new()
        .sheet("Model", "", CLEAN_ROW)
        .link(LINK_PATH, &present)
        .link(LINK_PATH, "Macintosh%20HD:Users:nobody-ozzit:Gone.xlsx")
        .link(LINK_PATH, "file:///Data:Gone.xlsx")
        .write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "external link: xl/externalLinks/externalLink2.xml targets Macintosh%20HD:Users:nobody-ozzit:Gone.xlsx, a Mac volume path not found on this computer",
            "external link: xl/externalLinks/externalLink3.xml targets file:///Data:Gone.xlsx, a Mac volume path not found on this computer",
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
            "unused name: Shown, which no formula uses",
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

/// A cell holding inline text.
fn label(at: &str, text: &str) -> String {
    format!(r#"<c r="{at}" t="inlineStr"><is><t>{text}</t></is></c>"#)
}

/// A formula cell.
fn formula(at: &str, text: &str) -> String {
    format!(r#"<c r="{at}"><f>{text}</f><v>0</v></c>"#)
}

fn number(at: &str) -> String {
    format!(r#"<c r="{at}"><v>1</v></c>"#)
}

fn row(r: usize, cells: &[String]) -> String {
    format!(r#"<row r="{r}">{}</row>"#, cells.concat())
}

#[test]
fn names_that_nothing_uses_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let names = concat!(
        r#"<definedName name="Rate">Model!$A$1</definedName>"#,
        r#"<definedName name="Unused">Model!$A$2</definedName>"#,
        r#"<definedName name="Local" localSheetId="0">Model!$A$3</definedName>"#,
        r#"<definedName name="Twice">Rate*2</definedName>"#,
        r#"<definedName name="Fn">_xlfn.LAMBDA(_xlpm.x, _xlpm.x)</definedName>"#,
        r#"<definedName name="_xlnm.Print_Area" localSheetId="0">Model!$A$1:$B$2</definedName>"#,
    );
    let rows = row(2, &[number("A2"), formula("B2", "twice+1")]);
    Book::new()
        .sheet("Model", "", &rows)
        .names(names)
        .write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "unused name: Unused, which no formula uses",
            "unused name: Local (sheet Model), which no formula uses",
        ]
    );
}

#[test]
fn a_check_row_that_nothing_reads_is_buried() {
    let dir = tempfile::tempdir().unwrap();
    let bs = [
        row(5, &[label("A5", "Assets"), number("C5"), number("D5")]),
        row(8, &[label("A8", "Liabilities"), number("C8"), number("D8")]),
        // Read only by its own row: still buried.
        row(
            10,
            &[
                label("A10", "Balance check"),
                formula("C10", "C5-C8"),
                formula("D10", "C10+D5-D8"),
            ],
        ),
        row(11, &[label("A11", "Cash check"), formula("C11", "C5-1")]),
        row(12, &[label("A12", "Tax Check:"), formula("C12", "C8-1")]),
    ]
    .concat();
    let summary = [
        // A roll-up reads a check, so it is not buried itself.
        row(
            2,
            &[label("A2", "All checks"), formula("B2", "AND(BS!C11=0)")],
        ),
        row(3, &[label("A3", "Checklist"), formula("B3", "1")]),
    ]
    .concat();
    // A formatting rule over its own cell does not show it in a summary.
    let rule = r#"<conditionalFormatting sqref="C12"><cfRule type="expression" priority="1"><formula>C12&lt;&gt;0</formula></cfRule></conditionalFormatting>"#;
    let bs = format!("{bs}</sheetData>{rule}<sheetData>");
    Book::new()
        .sheet("BS", "", &bs)
        .sheet("Summary", "", &summary)
        .write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "buried check: BS!C10:D10 ('Balance check') is read by no other formula, so no summary shows it",
            "buried check: BS!C12 ('Tax Check:') is read by no other formula, so no summary shows it",
        ]
    );
}

#[test]
fn a_row_between_a_sum_and_its_total_that_nothing_reads_is_unlinked() {
    let dir = tempfile::tempdir().unwrap();
    let rows = [
        row(2, &[label("A2", "Wages"), number("C2"), number("D2")]),
        row(3, &[label("A3", "Rent"), number("C3"), number("D3")]),
        row(4, &[label("A4", "Contractors"), number("C4"), number("D4")]),
        row(
            5,
            &[
                label("A5", "Total"),
                formula("C5", "SUM(C2:C3)"),
                formula("D5", "SUM(D2:D3)"),
            ],
        ),
        // A gap cell another formula reads is linked; a blank gap is empty.
        row(7, &[number("C7"), number("E7"), number("G7")]),
        row(8, &[number("C8"), formula("E8", "E7*2")]),
        row(
            9,
            &[
                formula("C9", "SUM(C7:C7)"),
                formula("E9", "SUM(E7:E7)"),
                formula("G9", "SUM(G7:G7)"),
            ],
        ),
        row(10, &[formula("C10", "C8")]),
    ]
    .concat();
    Book::new().sheet("Model", "", &rows).write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "unlinked row: Model!C4:D4 ('Contractors') lies between SUM(C2:C3) and its total C5, which leaves it out, and no formula reads it",
            "unlinked row: Model!E8 lies between SUM(E7:E7) and its total E9, which leaves it out, and no formula reads it",
        ]
    );
}

#[test]
fn a_column_between_a_sum_and_its_total_that_nothing_reads_is_unlinked() {
    let dir = tempfile::tempdir().unwrap();
    let cells = |r: usize| {
        let mut cells = vec![label(&format!("A{r}"), "Contractors")];
        cells.extend(["G", "H", "I"].map(|c| number(&format!("{c}{r}"))));
        cells.push(formula(&format!("J{r}"), &format!("SUM(G{r}:H{r})")));
        row(r, &cells)
    };
    // Row 3's I3 is read from another row, even in its own column, so only I2
    // is unlinked.
    let rows = [cells(2), cells(3), row(4, &[formula("I4", "I3")])].concat();
    Book::new().sheet("Model", "", &rows).write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "unlinked row: Model!I2 ('Contractors') lies between SUM(G2:H2) and its total J2, which leaves it out, and no formula reads it"
        ]
    );
}

#[test]
fn circular_references_and_iterative_calculation_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let rows = row(
        2,
        &[
            formula("B2", "C2+1"),
            formula("C2", "Other!A1*B2"),
            formula("D2", "SUM(D:D)"),
            formula("E2", "ROW(E2)+COLUMNS(E2:F2)"),
            formula("F2", "IFERROR(\"F2\",0)"),
            formula("G2", "CELL(\"filename\",G2)"),
        ],
    );
    let other = row(1, &[formula("A1", "1")]);
    Book::new()
        .sheet("Model", "", &rows)
        .sheet("Other", "", &other)
        .calc(r#"<calcPr calcId="191029" iterate="1"/>"#)
        .write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "circular reference: Model!B2, Model!C2 read one another",
            "circular reference: Model!D2 read one another",
            "iterative calculation is on, so Excel does not warn of a circular reference",
        ]
    );
}

#[test]
fn indirect_or_offset_leaves_checks_and_rows_unchecked() {
    let dir = tempfile::tempdir().unwrap();
    let rows = row(
        2,
        &[
            label("A2", "Balance check"),
            formula("C2", "INDIRECT(\"B1\")"),
        ],
    );
    Book::new().sheet("Model", "", &rows).write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        findings(&text),
        [
            "not checked for buried checks or unlinked rows: Model!C2 uses INDIRECT or OFFSET, so what reads a cell is not known"
        ]
    );
}

#[test]
fn references_excel_reads_otherwise_make_no_false_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let names = concat!(
        r#"<definedName name="D0">1</definedName>"#,
        r#"<definedName name="Choice">Model!$F$1</definedName>"#,
        // A sheet's constant shadows the workbook's reference.
        r#"<definedName name="Choice" localSheetId="3">1</definedName>"#,
        r#"<definedName name="Inputs">Model!$H$1</definedName>"#,
        r#"<definedName name="Self">Model!$I$1</definedName>"#,
        r#"<definedName name="SelfJ">Model!$J$1</definedName>"#,
        // A 3-D qualifier is no use of the name Jan.
        r#"<definedName name="Jan">Total!$A$1</definedName>"#,
        r#"<definedName name="SelfL">Model!$L$1</definedName>"#,
        r#"<definedName name="ſ1">1</definedName>"#,
        r#"<definedName name="K1">2</definedName>"#,
    );
    let model = [
        // A 3-D range, a structured and an external reference, a shared name.
        r#"<row r="1"><c r="A1"><f>SUM(Sales[Cost A1])+SUM([1]Data!A1:A1)</f><v>0</v></c><c r="D1"><f>E2</f><v>0</v></c><c r="E1"><f t="shared" ref="E1:E2" si="0">D0</f><v>1</v></c>"#,
        &formula("F1", "Choice"),
        &number("G1"),
        &formula("H1", "Inputs!B1"),
        &formula("I1", "ROW(Self)"),
        &formula("J1", "SUM(SelfJ)"),
        // Another workbook's name, and this workbook's ([0]) name of L1.
        &formula("K1", "[1]!FooK1"),
        &formula("L1", "[0]!SelfL"),
        // Names that only look like cells: long s and the Kelvin sign.
        &formula("M1", "ſ1+K1"),
        // Escapes inside a structured reference hide no cell reference.
        &formula("N1", "SUM(Sales['#Items])+N(\"N1\")+SUM(Sales[x']N1])"),
        r#"</row><row r="2"><c r="E2"><f t="shared" si="0"/><v>1</v></c></row>"#,
    ]
    .concat();
    Book::new()
        .sheet("Jan", "", &row(1, &[number("A1")]))
        .sheet("Feb", "", &row(1, &[number("A1")]))
        .sheet("Total", "", &row(1, &[formula("A1", "SUM(Jan:Feb!A1)")]))
        .sheet("Model", "", &model)
        .sheet("Inputs", "", &row(1, &[number("B1")]))
        .names(names)
        .write(dir.path());
    let (_, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(
        findings(&text),
        [
            "unused name: Inputs, which no formula uses",
            "unused name: Jan, which no formula uses",
            "circular reference: Model!J1 read one another",
            "circular reference: Model!L1 read one another",
        ]
    );
}

#[test]
fn checks_read_by_any_route_are_not_buried() {
    let dir = tempfile::tempdir().unwrap();
    let names = concat!(
        r#"<definedName name="CheckRange">Checks!$C$11</definedName>"#,
        r#"<definedName name="CheckAlias">CheckRange</definedName>"#,
        // Read from Summary as Checks!Flag: the name in Checks's scope.
        r#"<definedName name="Flag" localSheetId="0">Checks!$C$14</definedName>"#,
    );
    let checks = [
        r#"<row r="10">"#,
        &label("A10", "Balance check"),
        r#"<c r="C10"><f t="array" ref="C10:D10">{1,1}</f><v>1</v></c><c r="D10"><v>1</v></c></row>"#,
        &row(11, &[label("A11", "Alias check"), formula("C11", "1")]),
        &row(12, &[label("A12", "Lower check"), formula("C12", "1")]),
        &row(13, &[label("A13", "Lost check"), formula("C13", "1")]),
        &row(14, &[label("A14", "Check"), formula("C14", "1")]),
    ]
    .concat();
    let summary = row(
        2,
        &[
            formula("B2", "Checks!D10+CheckAlias+checks!c12"),
            formula("B3", "'Cash\"flow'!B2+SUM('Q1 a:Q1 b'!B2)"),
            formula("B4", "Checks!Flag"),
        ],
    );
    let check = |l: &str| row(2, &[label("A2", l), formula("B2", "1")]);
    Book::new()
        .sheet("Checks", "", &checks)
        .sheet("Summary", "", &summary)
        .sheet("Cash&quot;flow", "", &check("Cash check"))
        .sheet("Q1 a", "", "")
        .sheet("Q1 b", "", &check("Q check"))
        .names(names)
        .write(dir.path());
    let (_, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(
        findings(&text),
        [
            "buried check: Checks!C13 ('Lost check') is read by no other formula, so no summary shows it"
        ]
    );
}

#[test]
fn only_a_bare_structured_reference_reads_its_own_table() {
    let dir = tempfile::tempdir().unwrap();
    // Inside Local, a reference to Other reads Other alone; inside Bare, a
    // reference without a table reads Bare.
    let rows = [
        row(
            2,
            &[
                formula("A2", "SUM([1]!Other[Amount])"),
                formula("B2", "SUM(Other[Amount])"),
            ],
        ),
        row(3, &[label("A3", "Unrelated check"), formula("B3", "1")]),
        row(6, &[formula("B6", "SUM([Amount])")]),
        row(7, &[label("A7", "Read check"), formula("B7", "1")]),
        // [0] here is a column of the containing table, not this workbook.
        row(10, &[formula("B10", "SUM([0])")]),
        row(11, &[label("A11", "Zero check"), formula("B11", "1")]),
    ]
    .concat();
    Book::new()
        .sheet("T", "", &rows)
        .table(0, "Local", "A1:B3")
        .table(0, "Other", "D1:E3")
        .table(0, "Bare", "A5:B7")
        .table(0, "Zero", "A9:B11")
        .write(dir.path());
    let (_, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!(
        findings(&text),
        [
            "buried check: T!B3 ('Unrelated check') is read by no other formula, so no summary shows it"
        ]
    );
}

#[test]
fn a_this_workbook_qualifier_names_the_workbook_scope() {
    // [0]! is the workbook's name, not the sheet's, wherever the sheet is (as
    // Excel 16 resolves it): A1 reads itself, B1 does not, and C1 reads only
    // its own position.
    for model_first in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let local = if model_first { "0" } else { "1" };
        let names = [
            r#"<definedName name="PickA">Model!$A$1</definedName>"#.to_owned(),
            format!(r#"<definedName name="PickA" localSheetId="{local}">1</definedName>"#),
            r#"<definedName name="PickB">1</definedName>"#.to_owned(),
            format!(r#"<definedName name="PickB" localSheetId="{local}">Model!$B$1</definedName>"#),
            r#"<definedName name="PickC">Model!$C$1</definedName>"#.to_owned(),
        ]
        .concat();
        let model = row(
            1,
            &[
                formula("A1", "[0]!PickA"),
                formula("B1", "[0]!PickB"),
                formula("C1", "ROW([0]!PickC)"),
            ],
        );
        let book = Book::new();
        let book = if model_first {
            book.sheet("Model", "", &model).sheet("Other", "", "")
        } else {
            book.sheet("Other", "", "").sheet("Model", "", &model)
        };
        book.names(&names).write(dir.path());
        let (_, text) = audit(&["book.xlsx"], dir.path());
        assert_eq!(
            findings(&text),
            ["circular reference: Model!A1 read one another"],
            "Model first: {model_first}"
        );
    }
}

#[test]
fn this_workbook_sheet_qualifiers_read_the_local_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let check = |l: &str| row(3, &[label("A3", l), formula("B3", "1")]);
    let summary = row(
        1,
        &[
            formula("A1", "'[0]Cash flow'!B3"),
            formula("B1", "SUM([0]Jan:Feb!B3)"),
        ],
    );
    Book::new()
        .sheet("Jan", "", &check("Month check"))
        .sheet("Feb", "", &check("Month check"))
        .sheet("Cash flow", "", &check("Cash check"))
        .sheet("Summary", "", &summary)
        .write(dir.path());
    let (code, text) = audit(&["book.xlsx"], dir.path());
    assert_eq!((code, findings(&text)), (0, vec![]), "{text}");
}
