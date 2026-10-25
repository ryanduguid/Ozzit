//! Apply, refusal and command contracts retained from the Python help-link tests.
use ozzit::{
    archive::{BOOK, Parts, read_parts, write_deterministic},
    common::display,
    formula::MODULES,
    postbuild::help_links,
    py::PyPath,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

const PAIRS: [(&str, &str, &str); 2] = [
    (
        "(Current Assets/Liabilities)¶\" {AMP} \"WEBPAGE:       →",
        "https://www.investopedia.com/terms/c/cash-ratio.asp",
        "https://www.investopedia.com/terms/c/currentratio.asp",
    ),
    (
        "(Net Return on Investment/Cost of Investment)¶\" {AMP} \"WEBPAGE:               →",
        "https://www.investopedia.com/terms/r/returnonequity.asp",
        "https://www.investopedia.com/terms/r/returnoninvestment.asp",
    ),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn anchors(source: bool, fixed: bool) -> Vec<String> {
    PAIRS
        .iter()
        .map(|(prefix, old, new)| {
            let prefix = if source {
                prefix
                    .replace("{AMP}", "&")
                    .replace("¶\" & \"", "¶\" & \n                        \"")
            } else {
                prefix.replace("{AMP}", "&amp;")
            };
            format!("{prefix}{}", if fixed { new } else { old })
        })
        .collect()
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
        std::fs::create_dir(&src).unwrap();
        let f = Self {
            directory,
            workbook,
            src,
        };
        f.write_book(&format!(
            "<workbook>{}</workbook>",
            anchors(false, false).join("")
        ));
        std::fs::write(f.ratios(), anchors(true, false).join("\n")).unwrap();
        f
    }
    fn reverted_committed() -> Self {
        let f = Self::new();
        std::fs::copy(root().join("ozzit.xlsx"), &f.workbook).unwrap();
        for module in MODULES {
            std::fs::copy(
                root().join(format!("src/{module}.txt")),
                f.src.join(format!("{module}.txt")),
            )
            .unwrap();
        }
        let mut parts = read_parts(&f.workbook).unwrap();
        let mut book = String::from_utf8(parts[BOOK].clone()).unwrap();
        let mut ratios = std::fs::read_to_string(f.ratios()).unwrap();
        for (source, text) in [(false, &mut book), (true, &mut ratios)] {
            for (old, new) in anchors(source, false)
                .into_iter()
                .zip(anchors(source, true))
            {
                assert_eq!(text.matches(&new).count(), 1);
                *text = text.replace(&new, &old);
            }
        }
        parts.insert(BOOK.into(), book.into_bytes());
        write_deterministic(&f.workbook, &parts).unwrap();
        std::fs::write(f.ratios(), ratios).unwrap();
        let mut original = std::fs::read(&f.workbook).unwrap();
        let end = original.len();
        assert_eq!(&original[end - 2..], &[0, 0]);
        let comment = b"original archive";
        original[end - 2..].copy_from_slice(&(comment.len() as u16).to_le_bytes());
        original.extend(comment);
        std::fs::write(&f.workbook, original).unwrap();
        f
    }
    fn ratios(&self) -> PathBuf {
        self.src.join("Ratios.txt")
    }
    fn py_src(&self) -> PyPath {
        PyPath::new(&display(&self.src))
    }
    fn write_book(&self, book: &str) {
        write_deterministic(
            &self.workbook,
            &Parts::from([(BOOK.into(), book.as_bytes().to_vec())]),
        )
        .unwrap();
    }
    fn book(&self) -> String {
        String::from_utf8(read_parts(&self.workbook).unwrap()[BOOK].clone()).unwrap()
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
    fn command(&self, args: &[String]) -> (i32, String, String) {
        let result = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg("help_links")
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
    fn run_cli(&self) -> (i32, String, String) {
        self.command(&[display(&self.workbook), display(&self.src)])
    }
}

#[test]
fn the_command_applies_both_stores_and_reports_the_exact_changes() {
    let f = Fixture::new();
    assert_eq!(
        f.run_cli(),
        (
            0,
            format!(
                "applied help links to: workbook, src/Ratios.txt\nOK: {}\n",
                display(&f.workbook)
            ),
            String::new()
        )
    );
    assert_eq!(
        f.book(),
        format!("<workbook>{}</workbook>", anchors(false, true).join(""))
    );
    assert_eq!(
        std::fs::read_to_string(f.ratios()).unwrap(),
        anchors(true, true).join("\n")
    );
}

#[test]
fn a_missing_ratios_module_is_refused_before_writing() {
    let f = Fixture::new();
    std::fs::remove_file(f.ratios()).unwrap();
    let before = f.snapshot();
    assert_eq!(
        help_links::run(&f.workbook, &f.py_src()),
        Err(format!("src: missing {}", display(&f.ratios())))
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
fn all_four_missing_anchors_are_reported_in_store_order_before_writing() {
    let f = Fixture::new();
    f.write_book("<workbook/>");
    std::fs::write(f.ratios(), "").unwrap();
    let before = f.snapshot();
    let failures = help_links::run(&f.workbook, &f.py_src()).unwrap_err();
    let rows: Vec<_> = failures.split("; ").collect();
    assert_eq!(rows.len(), 4);
    for (i, source) in [false, false, true, true].into_iter().enumerate() {
        let old = &anchors(source, false)[i % 2];
        let tail: String = old
            .chars()
            .skip(old.chars().count().saturating_sub(60))
            .collect();
        let store = if source { "src" } else { "workbook" };
        assert_eq!(
            rows[i],
            format!(
                "{store}: '{tail}' expected exactly 1 recognised anchor, got wrong-link=0 fixed-link=0"
            )
        );
    }
    assert_eq!(f.snapshot(), before);
}

#[test]
fn extra_arguments_report_the_native_usage() {
    let f = Fixture::new();
    assert_eq!(
        f.command(&["a.xlsx".into(), "src".into(), "extra".into()]),
        (
            1,
            String::new(),
            "FAIL: usage: ozzit help_links [workbook] [src dir]\n".into()
        )
    );
}

#[test]
fn a_missing_workbook_is_refused() {
    let f = Fixture::new();
    let missing = f.directory.path().join("absent.xlsx");
    assert_eq!(
        f.command(&[display(&missing), display(&f.src)]),
        (
            1,
            String::new(),
            format!("FAIL: no such workbook: {}\n", display(&missing))
        )
    );
}

#[test]
fn a_missing_source_is_reported_by_the_command() {
    let f = Fixture::new();
    std::fs::remove_file(f.ratios()).unwrap();
    let before = f.snapshot();
    assert_eq!(
        f.run_cli(),
        (
            1,
            String::new(),
            format!("FAIL: src: missing {}\n", display(&f.ratios()))
        )
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
fn both_real_stores_are_repaired_then_the_second_run_is_a_byte_noop() {
    let f = Fixture::reverted_committed();
    assert_eq!(
        help_links::run(&f.workbook, &f.py_src()).unwrap(),
        ["workbook", "src/Ratios.txt"]
    );
    assert_eq!(
        read_parts(&f.workbook).unwrap(),
        read_parts(&root().join("ozzit.xlsx")).unwrap()
    );
    for module in MODULES {
        assert_eq!(
            std::fs::read(f.src.join(format!("{module}.txt"))).unwrap(),
            std::fs::read(root().join(format!("src/{module}.txt"))).unwrap()
        );
    }
    let before = f.snapshot();
    assert!(
        help_links::run(&f.workbook, &f.py_src())
            .unwrap()
            .is_empty()
    );
    assert_eq!(f.snapshot(), before);
}
