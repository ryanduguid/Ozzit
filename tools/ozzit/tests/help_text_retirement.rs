//! Contracts retained from the FY27 and GST Python help-text writer tests.
use ozzit::{
    archive::{BOOK, Parts, read_parts, write_deterministic},
    common::display,
    formula::MODULES,
    postbuild::{fy27_help_text, gst_help_text},
    py::PyPath,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

const EDATE_OLD: &str = "EDATE(\"\"1/1/2025\"\", SEQUENCE(,24, 0))";
const EDATE_NEW: &str = "EDATE(\"\"1/7/2026\"\", SEQUENCE(,24, 0))";
const COUNT_OLD: &str = "\"\"22/3/2012\"\", \"\"10/4/2012\"\"";
const COUNT_NEW: &str = "\"\"23/7/2026\"\", \"\"11/8/2026\"\"";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn gst() -> Value {
    serde_json::from_str(include_str!("../src/data/postbuild/gst_help_text.json")).unwrap()
}
fn defined_name<'a>(book: &'a str, name: &str) -> Result<&'a str, String> {
    let document = roxmltree::Document::parse(book).unwrap();
    document
        .descendants()
        .find(|n| n.has_tag_name("definedName") && n.attribute("name") == Some(name))
        .map(|n| &book[n.range()])
        .ok_or_else(|| format!("{name} missing from workbook.xml"))
}

struct Fixture {
    directory: tempfile::TempDir,
    workbook: PathBuf,
    src: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let workbook = directory.path().join("ozzit.xlsx");
        let src = directory.path().join("src");
        std::fs::copy(root().join("ozzit.xlsx"), &workbook).unwrap();
        std::fs::create_dir(&src).unwrap();
        for module in MODULES {
            std::fs::copy(
                root().join(format!("src/{module}.txt")),
                src.join(format!("{module}.txt")),
            )
            .unwrap();
        }
        Self {
            directory,
            workbook,
            src,
        }
    }
    fn book(&self) -> String {
        String::from_utf8(read_parts(&self.workbook).unwrap()[BOOK].clone()).unwrap()
    }
    fn source(&self, module: &str) -> String {
        std::fs::read_to_string(self.src.join(format!("{module}.txt"))).unwrap()
    }
    fn swap_book(&self, from: &str, to: &str) {
        let mut parts = read_parts(&self.workbook).unwrap();
        let text = String::from_utf8(parts[BOOK].clone()).unwrap();
        assert!(text.contains(from), "workbook precondition: {from}");
        parts.insert(BOOK.into(), text.replace(from, to).into_bytes());
        write_deterministic(&self.workbook, &parts).unwrap();
    }
    fn swap_source(&self, module: &str, from: &str, to: &str) {
        let text = self.source(module);
        std::fs::write(
            self.src.join(format!("{module}.txt")),
            text.replace(from, to),
        )
        .unwrap();
    }
    fn revert_gst_add(&self) {
        let data = gst();
        let (old, new) = (
            data["swaps"][0][0].as_str().unwrap(),
            data["swaps"][0][1].as_str().unwrap(),
        );
        assert!(self.source("Financial").contains(new));
        self.swap_book(new, old);
        self.swap_source("Financial", new, old);
    }
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut files =
            BTreeMap::from([("ozzit.xlsx".into(), std::fs::read(&self.workbook).unwrap())]);
        for module in MODULES {
            let name = format!("{module}.txt");
            if let Ok(bytes) = std::fs::read(self.src.join(&name)) {
                files.insert(name, bytes);
            }
        }
        files
    }
    fn run_args(&self, pass: &str, args: &[String]) -> (i32, String, String) {
        let result = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg(pass)
            .args(args)
            .current_dir(self.directory.path())
            .output()
            .unwrap();
        (
            result.status.code().unwrap(),
            String::from_utf8(result.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            String::from_utf8(result.stderr)
                .unwrap()
                .replace("\r\n", "\n"),
        )
    }
    fn run(&self, pass: &str) -> (i32, String, String) {
        self.run_args(pass, &[display(&self.workbook), display(&self.src)])
    }
    fn success(&self, pass: &str, label: &str) {
        assert_eq!(
            self.run(pass),
            (
                0,
                format!(
                    "applied {label} to: workbook, src/Financial.txt\nOK: {}\n",
                    display(&self.workbook)
                ),
                String::new()
            )
        );
    }
    fn refused(&self, pass: &str, diagnostic: &str) {
        let before = self.snapshot();
        let (code, out, err) = self.run(pass);
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert!(
            err.starts_with("FAIL: ") && err.contains(diagnostic),
            "{err}"
        );
        assert!(!err.contains("panicked"));
        assert_eq!(
            self.snapshot(),
            before,
            "validation must precede every write"
        );
    }
}

fn protected_gst_help(book: &str, financial: &str) {
    let about = defined_name(book, "oz.AboutFinancialλ").unwrap();
    let add = defined_name(book, "oz.GSTAddλ").unwrap();
    let extract = defined_name(book, "oz.GSTExtractλ").unwrap();
    assert!(add.contains("comment=\"Adds GST to one or more GST-exclusive amounts.\""));
    assert!(
        extract.contains(
            "comment=\"Returns the GST contained in one or more GST-inclusive amounts.\""
        )
    );
    assert!(!about.contains("ss 9-70") && !about.contains("NOTES!"));
    assert!(add.contains("ss 9-70 and 9-75") && extract.contains("ss 9-70 and 9-75"));
    assert_eq!(book.matches("ss 9-70").count(), 2);
    assert!(
        financial.contains("GSTAddλ            →Adds GST to one or more GST-exclusive amounts.¶")
    );
    assert!(financial.contains(
        "GSTExtractλ        →Returns the GST contained in one or more GST-inclusive amounts.¶"
    ));
    assert_eq!(financial.matches("ss 9-70").count(), 2);
}

#[test]
fn every_fy27_swap_preserves_character_length() {
    let data: Value =
        serde_json::from_str(include_str!("../src/data/postbuild/fy27_help_text.json")).unwrap();
    let swaps = data["swaps"].as_array().unwrap();
    assert_eq!(swaps.len(), 27);
    for swap in swaps {
        assert_eq!(
            swap[0].as_str().unwrap().chars().count(),
            swap[1].as_str().unwrap().chars().count()
        );
    }
}

#[test]
fn fy27_migrates_edate_in_both_stores_without_changing_character_length() {
    let f = Fixture::new();
    assert!(f.source("Financial").contains(EDATE_NEW));
    f.swap_book(EDATE_NEW, EDATE_OLD);
    f.swap_source("Financial", EDATE_NEW, EDATE_OLD);
    let before = f.book().chars().count();
    f.success("fy27_help_text", "FY27 help text");
    let book = f.book();
    assert!(book.contains(EDATE_NEW) && !book.contains(EDATE_OLD));
    assert!(f.source("Financial").contains(EDATE_NEW));
    assert_eq!(book.chars().count(), before);
    assert_eq!(
        read_parts(&f.workbook).unwrap(),
        read_parts(&root().join("ozzit.xlsx")).unwrap()
    );
}

#[test]
fn fy27_refuses_a_duplicate_countdow_anchor() {
    let f = Fixture::new();
    f.swap_book(COUNT_NEW, COUNT_OLD);
    assert_eq!(f.book().matches(COUNT_OLD).count(), 1);
    f.swap_book(COUNT_OLD, &COUNT_OLD.repeat(2));
    f.refused("fy27_help_text", "expected 1 hits, got 2");
}

#[test]
fn fy27_refuses_an_anchor_absent_from_the_entire_library() {
    let f = Fixture::new();
    let dummy = "EDATE(\"\"1/1/1900\"\", SEQUENCE(,24, 0))";
    f.swap_book(EDATE_NEW, dummy);
    for module in MODULES {
        f.swap_source(module, EDATE_NEW, dummy);
    }
    f.refused("fy27_help_text", "absent and its replacement is absent too");
}

#[test]
fn gst_notes_keep_padded_labels_legal_xml_text_and_the_scope_citation() {
    let data = gst();
    for swap in data["swaps"].as_array().unwrap() {
        let new = swap[1].as_str().unwrap();
        let notes = new[new.find("NOTES!").unwrap()..]
            .strip_suffix('"')
            .unwrap();
        assert!(notes.starts_with("NOTES!         →"));
        assert!(!notes.contains('&'));
        assert!(notes.contains("ss 9-70 and 9-75") && notes.contains("outside the GST system"));
        for row in notes.trim_end_matches('¶').split('¶') {
            assert_eq!(row.split_once('→').unwrap().0.chars().count(), 15);
        }
    }
}

#[test]
fn documented_order_syncs_afe_after_gst_and_sanitises_last() {
    let text = std::fs::read_to_string(root().join("tools/postbuild/README.md")).unwrap();
    let gst = text
        .find("tools/ozzit/target/release/ozzit gst_help_text ozzit.xlsx src")
        .unwrap();
    let afe = text
        .find("tools/ozzit/target/release/ozzit sync_afe_store ozzit.xlsx src")
        .unwrap();
    let sanitise = text
        .find("tools/ozzit/target/release/ozzit sanitise_workbook ozzit.xlsx")
        .unwrap();
    assert!(gst < afe && afe < sanitise);
}

#[test]
fn current_gst_help_preserves_about_rows_and_name_manager_comments() {
    let f = Fixture::new();
    protected_gst_help(&f.book(), &f.source("Financial"));
}

#[test]
fn gst_migrates_pre_note_text_without_touching_about_or_comments() {
    let f = Fixture::new();
    f.revert_gst_add();
    f.success("gst_help_text", "GST help text");
    let data = gst();
    let book = f.book();
    assert!(book.contains(data["swaps"][0][1].as_str().unwrap()));
    assert!(!book.contains(data["swaps"][0][0].as_str().unwrap()));
    assert!(
        f.source("Financial")
            .contains(data["swaps"][0][1].as_str().unwrap())
    );
    protected_gst_help(&book, &f.source("Financial"));
}

#[test]
fn gst_text_changes_leave_afe_stale_until_the_documented_sync() {
    let f = Fixture::new();
    f.revert_gst_add();
    assert_eq!(f.run("sync_afe_store").0, 0);
    assert_eq!(f.run("gst_help_text").0, 0);
    let stale = f.run("verify_afe");
    assert_eq!(stale.0, 1, "{stale:?}");
    assert_eq!(f.run("sync_afe_store").0, 0);
    let verified = f.run("verify_afe");
    assert_eq!(verified.0, 0, "{verified:?}");
}

#[test]
fn gst_refuses_a_duplicate_pre_note_anchor() {
    let f = Fixture::new();
    let data = gst();
    let old = data["swaps"][0][0].as_str().unwrap();
    f.swap_book(data["swaps"][0][1].as_str().unwrap(), old);
    assert_eq!(f.book().matches(old).count(), 1);
    f.swap_book(old, &old.repeat(2));
    f.refused(
        "gst_help_text",
        "got 2 (pre-note=2, legacy-note=0, current-note=0)",
    );
}

#[test]
fn gst_refuses_a_missing_anchor_in_both_stores() {
    let f = Fixture::new();
    let data = gst();
    let new = data["swaps"][0][1].as_str().unwrap();
    let dummy = "DESCRIPTION:   →Adds GST to one or more GST-exclusive amounts (removed).¶\"";
    f.swap_book(new, dummy);
    for module in MODULES {
        f.swap_source(module, new, dummy);
    }
    f.refused(
        "gst_help_text",
        "got 0 (pre-note=0, legacy-note=0, current-note=0)",
    );
}

#[test]
fn gst_names_a_missing_source_anchor_even_when_the_workbook_is_current() {
    let f = Fixture::new();
    let data = gst();
    let new = data["swaps"][0][1].as_str().unwrap();
    assert!(f.source("Financial").contains(new));
    f.swap_source(
        "Financial",
        new,
        "DESCRIPTION:   →Adds GST to one or more GST-exclusive amounts (removed).¶\"",
    );
    f.refused("gst_help_text", "src:");
}

#[test]
fn both_help_passes_report_every_missing_module_in_order() {
    let f = Fixture::new();
    for module in MODULES {
        std::fs::remove_file(f.src.join(format!("{module}.txt"))).unwrap();
    }
    write_deterministic(
        &f.workbook,
        &Parts::from([(BOOK.into(), b"<workbook/>".to_vec())]),
    )
    .unwrap();
    let src = PyPath::new(&display(&f.src));
    for (pass, failure) in [
        (
            "fy27_help_text",
            fy27_help_text::run(&f.workbook, &src).unwrap_err(),
        ),
        (
            "gst_help_text",
            gst_help_text::run(&f.workbook, &src).unwrap_err(),
        ),
    ] {
        let expected =
            MODULES.map(|m| format!("src: missing {}", display(&f.src.join(format!("{m}.txt")))));
        assert_eq!(
            failure
                .split("; ")
                .filter(|f| f.starts_with("src: missing"))
                .collect::<Vec<_>>(),
            expected
        );
        let result = f.run(pass);
        assert_eq!(result.0, 1);
        assert!(result.1.is_empty());
        assert!(
            result
                .2
                .starts_with(&format!("FAIL: src: missing {}", display(&f.src)))
        );
    }
}

#[test]
fn both_help_passes_reject_extra_arguments_with_native_usage() {
    let f = Fixture::new();
    for pass in ["fy27_help_text", "gst_help_text"] {
        assert_eq!(
            f.run_args(pass, &["a".into(), "b".into(), "c".into()]),
            (
                1,
                String::new(),
                format!("FAIL: usage: ozzit {pass} [workbook] [src dir]\n")
            )
        );
    }
}

#[test]
fn both_help_passes_refuse_a_missing_workbook() {
    let f = Fixture::new();
    let missing = display(&f.directory.path().join("missing.xlsx"));
    for pass in ["fy27_help_text", "gst_help_text"] {
        assert_eq!(
            f.run_args(pass, std::slice::from_ref(&missing)),
            (
                1,
                String::new(),
                format!("FAIL: no such workbook: {missing}\n")
            )
        );
    }
}

#[test]
fn the_defined_name_helper_returns_the_complete_matching_element() {
    let book = "<definedName name=\"oz.Aλ\" comment=\"c\">=LAMBDA(x, x)</definedName>";
    assert_eq!(defined_name(book, "oz.Aλ"), Ok(book));
}

#[test]
fn the_defined_name_helper_names_a_missing_definition() {
    let book = "<definedName name=\"oz.Aλ\" comment=\"c\">=LAMBDA(x, x)</definedName>";
    assert_eq!(
        defined_name(book, "oz.Bλ"),
        Err("oz.Bλ missing from workbook.xml".into())
    );
}

#[test]
fn gst_migrates_both_legacy_notes_to_current_text() {
    let f = Fixture::new();
    let data = gst();
    for (swap, legacy) in data["swaps"]
        .as_array()
        .unwrap()
        .iter()
        .zip(data["legacy"].as_array().unwrap())
    {
        let (new, legacy) = (swap[1].as_str().unwrap(), legacy.as_str().unwrap());
        f.swap_book(new, legacy);
        f.swap_source("Financial", new, legacy);
    }
    f.success("gst_help_text", "GST help text");
    assert_eq!(
        read_parts(&f.workbook).unwrap(),
        read_parts(&root().join("ozzit.xlsx")).unwrap()
    );
    assert_eq!(
        std::fs::read(f.src.join("Financial.txt")).unwrap(),
        std::fs::read(root().join("src/Financial.txt")).unwrap()
    );
    protected_gst_help(&f.book(), &f.source("Financial"));
}
