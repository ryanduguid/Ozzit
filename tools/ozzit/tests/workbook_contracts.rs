//! Workbook helper and physical command contracts retained from Python.
use base64::{Engine, engine::general_purpose::STANDARD};
use ozzit::{archive::*, common::*, workbook};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn newline() -> &'static str {
    if cfg!(windows) { "\r\n" } else { "\n" }
}

struct Fixture {
    dir: tempfile::TempDir,
    book: PathBuf,
}
impl Fixture {
    fn published() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("ozzit.xlsx");
        std::fs::copy(root().join("ozzit.xlsx"), &book).unwrap();
        Self { dir, book }
    }
    fn parts(&self) -> Parts {
        read_parts(&self.book).unwrap()
    }
    fn write(&self, parts: &Parts) {
        write_deterministic(&self.book, parts).unwrap();
    }
    fn snapshot(&self) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                assert!(!entry.file_type().unwrap().is_symlink());
                let path = entry.path();
                let bytes = if path.is_dir() {
                    visit(root, &path, result);
                    None
                } else {
                    Some(std::fs::read(&path).unwrap())
                };
                result.insert(path.strip_prefix(root).unwrap().into(), bytes);
            }
        }
        let mut result = BTreeMap::new();
        visit(self.dir.path(), self.dir.path(), &mut result);
        result
    }
    fn cli(&self, args: &[String], encoding: &str) -> std::process::Output {
        let before = self.snapshot();
        let result = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg("verify_workbook")
            .args(args)
            .env("PYTHONIOENCODING", encoding)
            .current_dir(self.dir.path())
            .output()
            .unwrap();
        assert_eq!(self.snapshot(), before);
        result
    }
    fn run(&self) -> std::process::Output {
        self.cli(&[display(&self.book)], "utf-8")
    }
    fn refuses(&self, message: &str) -> String {
        let result = self.run();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stderr.is_empty(), "{:?}", result.stderr);
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.contains(message), "{text}");
        text
    }
}

fn frozen(index: usize) -> (Parts, Vec<String>) {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/verifier-contracts.json.gz"),
    )
    .unwrap();
    let data: Value = serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap();
    let record = &data["records"][index];
    assert_eq!(record["kind"], "workbook");
    let parts: Parts = record["parts"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, bytes)| {
            (
                name.clone(),
                STANDARD.decode(bytes.as_str().unwrap()).unwrap(),
            )
        })
        .collect();
    assert_eq!(
        parts.keys().cloned().collect::<Vec<_>>(),
        record["names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    let failures = record["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().into())
        .collect();
    (parts, failures)
}

fn replace(parts: &mut Parts, part: &str, before: &str, after: &str) {
    let text = text(parts, part).unwrap();
    assert!(text.contains(before), "{part}: {before}");
    parts.insert(part.into(), text.replace(before, after).into_bytes());
}

#[test]
fn the_exact_fourteen_defect_fixture_retains_its_physical_header_and_multiset() {
    let f = Fixture::published();
    let (parts, mut expected) = frozen(0);
    assert_eq!(parts.len(), 13);
    assert_eq!(expected.len(), 14);
    f.write(&parts);
    let output = f.refuses(&format!("FAIL: 14 problem(s) in {}", display(&f.book)));
    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(
        lines[0],
        format!("FAIL: 14 problem(s) in {}", display(&f.book))
    );
    let mut actual = lines[1..]
        .iter()
        .map(|line| line.strip_prefix("  - ").unwrap().to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected);
}

#[test]
fn both_original_store_failures_stop_before_defined_name_checks() {
    for (index, message) in [
        (1, "AFE project store missing"),
        (2, "invalid AFE project store: Expecting value"),
    ] {
        let f = Fixture::published();
        let (parts, expected) = frozen(index);
        assert_eq!(parts.len(), if index == 1 { 4 } else { 5 });
        assert_eq!(expected.len(), 1);
        assert!(expected[0].starts_with(message));
        f.write(&parts);
        let output = f.refuses(message);
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0],
            format!("FAIL: 1 problem(s) in {}", display(&f.book))
        );
        assert!(lines[1].starts_with(&format!("  - {message}")));
        assert!(!output.contains("no defined names"));
    }
}

#[test]
fn the_original_broken_xml_is_refused_as_one_helper_error() {
    let error = workbook::xml_part("fixture.xml", b"<broken>")
        .err()
        .unwrap();
    assert!(
        error.starts_with("malformed XML in fixture.xml:"),
        "{error}"
    );
}

#[test]
fn the_original_doctype_is_refused_before_entity_expansion() {
    assert_eq!(
        workbook::xml_part(
            "xl/worksheets/sheet1.xml",
            b"<!DOCTYPE x [<!ENTITY a \"boom\">]><x>&a;</x>"
        )
        .err()
        .unwrap(),
        "DOCTYPE declaration in xl/worksheets/sheet1.xml"
    );
}

#[test]
fn the_original_ordinary_windows_user_path_is_banned() {
    assert!(!workbook::banned_tokens(r"C:\Users\Example\Documents\Ozzit").is_empty());
}

#[test]
fn all_three_original_malformed_afe_payloads_fail_without_tracebacks() {
    for payload in [vec![b'x'; 151], vec![b'x'; 150], vec![b'{'; 150]] {
        let f = Fixture::published();
        let mut parts = f.parts();
        parts.insert(
            "customXml/item1.xml".into(),
            format!("<root>{}</root>", STANDARD.encode(payload)).into_bytes(),
        );
        f.write(&parts);
        f.refuses("invalid AFE project store");
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Keeps all seven original add-in mutations and the passing baseline together."
)]
fn the_hidden_afe_add_in_passes_and_all_seven_original_mutations_fail() {
    let f = Fixture::published();
    let original = f.parts();
    let baseline = f.run();
    assert_eq!(baseline.status.code(), Some(0));
    assert!(baseline.stderr.is_empty());
    for (index, message) in [
        "expected one web extension",
        "an add-in other than",
        "one hidden task pane",
        "containsCustomFunctions",
        "containsCustomFunctions",
        "expected one web extension",
        "does not resolve",
    ]
    .into_iter()
    .enumerate()
    {
        let mut parts = original.clone();
        let extension = "xl/webextensions/webextension1.xml";
        let relationships = "xl/webextensions/_rels/taskpanes.xml.rels";
        match index {
            0 => {
                parts.insert(
                    "xl/webextensions/webextension2.xml".into(),
                    parts[extension].clone(),
                );
            }
            1 => replace(&mut parts, extension, "wa200003696", "wa104379999"),
            2 => replace(
                &mut parts,
                "xl/webextensions/taskpanes.xml",
                "visibility=\"0\"",
                "visibility=\"1\"",
            ),
            3 => replace(
                &mut parts,
                extension,
                "<we:bindings/>",
                "<we:bindings/><we:containsCustomFunctions/>",
            ),
            4 => replace(
                &mut parts,
                extension,
                "<we:properties>",
                "<we:properties><we:property name=\"containsCustomFunctions\" value=\"true\"/>",
            ),
            5 => {
                parts.insert(
                    "xl/webextensions/other.xml".into(),
                    text(&original, extension)
                        .unwrap()
                        .replace("wa200003696", "wa104379999")
                        .into_bytes(),
                );
                replace(&mut parts, relationships, "webextension1.xml", "other.xml");
                assert_eq!(parts[extension], original[extension]);
            }
            6 => replace(
                &mut parts,
                relationships,
                "webextension1.xml",
                "missing.xml",
            ),
            _ => unreachable!(),
        }
        f.write(&parts);
        f.refuses(message);
    }
}

#[test]
fn custom_function_words_in_cell_text_are_not_declarations() {
    let f = Fixture::published();
    let mut parts = f.parts();
    replace(
        &mut parts,
        "xl/sharedStrings.xml",
        "</sst>",
        "<si><t>containsCustomFunctions</t></si></sst>",
    );
    f.write(&parts);
    let result = f.run();
    assert!(result.stderr.is_empty());
    // The unchanged uniqueCount may fail independently; the literal is no declaration.
    assert!(
        !String::from_utf8(result.stdout)
            .unwrap()
            .contains("containsCustomFunctions")
    );
}

#[test]
fn both_sheet_scoped_names_are_checked_instead_of_collapsing_their_key() {
    let f = Fixture::published();
    let mut parts = f.parts();
    replace(
        &mut parts,
        BOOK,
        "</definedNames>",
        "<definedName name=\"fixture\" localSheetId=\"0\">(</definedName><definedName name=\"fixture\" localSheetId=\"1\">1</definedName></definedNames>",
    );
    f.write(&parts);
    f.refuses("unbalanced parentheses in fixture");
}

#[test]
fn the_current_workbook_refuses_the_original_8195_character_name() {
    let f = Fixture::published();
    let mut parts = f.parts();
    let book = text(&parts, BOOK).unwrap();
    let start = book.find("<definedName name=\"oz.GSTExtractλ\"").unwrap();
    let open_end = start + book[start..].find('>').unwrap() + 1;
    let close = open_end + book[open_end..].find("</definedName>").unwrap();
    let padded = format!("\"{}\"", "1".repeat(8193));
    assert_eq!(padded.chars().count(), 8195);
    parts.insert(
        BOOK.into(),
        format!("{}{padded}{}", &book[..open_end], &book[close..]).into_bytes(),
    );
    f.write(&parts);
    f.refuses("oz.GSTExtractλ is 8195 characters");
}

#[test]
fn all_seven_original_forbidden_parts_are_refused() {
    for part in [
        "xl/vbaProject.bin",
        "xl/activeX/activeX1.xml",
        "xl/ctrlProps/ctrlProp1.xml",
        "xl/externalLinks/externalLink1.xml",
        "xl/embeddings/oleObject1.bin",
        "xl/macrosheets/sheet1.xml",
        "xl/dialogsheets/sheet1.xml",
    ] {
        let f = Fixture::published();
        let mut parts = f.parts();
        assert!(!parts.contains_key(part));
        parts.insert(
            part.into(),
            b"forbidden executable or linked content".to_vec(),
        );
        f.write(&parts);
        // XML-named injected payloads are malformed too; other findings may coexist.
        f.refuses(&format!("forbidden workbook part {part}"));
    }
}

#[test]
fn both_original_malformed_parts_are_skipped_after_reporting() {
    let json = format!("{{\"padding\": \"{}\"}}", "x".repeat(80));
    let utf16 = json
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let store = format!("<store>{}</store>", STANDARD.encode(utf16)).into_bytes();
    for part in ["docProps/app.xml", "xl/slicerCaches/slicerCache1.xml"] {
        let f = Fixture::published();
        let parts = [
            ("customXml/item1.xml".into(), store.clone()),
            (part.into(), b"<p>\xff</p>".to_vec()),
        ]
        .into_iter()
        .collect();
        f.write(&parts);
        f.refuses(&format!("malformed XML in {part}"));
    }
}

#[test]
fn physical_defaults_and_explicit_arguments_force_utf8_and_preserve_inputs() {
    let mut f = Fixture::published();
    let unicode = f.dir.path().join("workbook-λ→.xlsx");
    std::fs::copy(&f.book, &unicode).unwrap();
    f.book = unicode;
    for encoding in ["cp1252", "utf-8"] {
        for args in [vec![], vec![display(&f.book)]] {
            let result = f.cli(&args, encoding);
            assert_eq!(result.status.code(), Some(0));
            assert!(result.stderr.is_empty());
            let name = args.first().map_or("ozzit.xlsx", String::as_str);
            assert_eq!(
                result.stdout,
                format!("OK: {name}, 153 functions, 169 parts{}", newline()).into_bytes()
            );
        }
    }
}

#[test]
fn physical_surplus_arguments_fail_before_reading_or_writing() {
    let f = Fixture::published();
    let args = [
        display(&f.book),
        display(&f.dir.path().join("unrequested.xlsx")),
    ];
    let result = f.cli(&args, "utf-8");
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(
        result.stderr,
        format!("FAIL: usage: ozzit verify_workbook [workbook]{}", newline()).into_bytes()
    );
}

#[test]
fn both_original_unreadable_workbooks_fail_without_tracebacks_or_writes() {
    let f = Fixture::published();
    let invalid = f.dir.path().join("not-a-zip.xlsx");
    let missing = f.dir.path().join("missing.xlsx");
    std::fs::write(&invalid, b"not a zip archive").unwrap();
    for path in [invalid, missing] {
        let result = f.cli(&[display(&path)], "utf-8");
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stderr.is_empty());
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(
            text.starts_with(&format!("FAIL: cannot read {}", display(&path))),
            "{text}"
        );
        assert!(!text.contains("Traceback") && !text.contains("OK:"));
    }
}
