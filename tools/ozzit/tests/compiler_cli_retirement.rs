//! Former Python compiler command contracts, on owned synthetic or shipped copies.
use ozzit::{archive, common::display, formula};
use serde_json::Value;
use std::{path::PathBuf, process::Command};

struct Fixture {
    directory: tempfile::TempDir,
    workbook: PathBuf,
    src: PathBuf,
    index: PathBuf,
}

fn original() -> Value {
    serde_json::from_str(include_str!("data/compiler-cli.json")).unwrap()
}

fn original_parts(name: &str) -> archive::Parts {
    original()[name]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, text)| (name.clone(), text.as_str().unwrap().as_bytes().to_vec()))
        .collect()
}

impl Fixture {
    fn empty() -> Self {
        let directory = tempfile::tempdir().unwrap();
        Self {
            workbook: directory.path().join("ozzit.xlsx"),
            src: directory.path().join("src"),
            index: directory.path().join("functions.csv"),
            directory,
        }
    }

    fn synthetic() -> Self {
        let fixture = Self::empty();
        std::fs::create_dir(&fixture.src).unwrap();
        let data = original();
        let sources = data["sources"].as_object().unwrap();
        assert_eq!(sources.len(), formula::MODULES.len());
        for module in formula::MODULES {
            let name = format!("{module}.txt");
            std::fs::write(fixture.src.join(&name), sources[&name].as_str().unwrap()).unwrap();
        }
        archive::write_deterministic(&fixture.workbook, &original_parts("stale_parts")).unwrap();
        std::fs::write(&fixture.index, data["index_header"].as_str().unwrap()).unwrap();
        assert_eq!(archive::read_parts(&fixture.workbook).unwrap().len(), 1);
        fixture
    }

    fn shipped() -> Self {
        let fixture = Self::empty();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::copy(root.join("ozzit.xlsx"), &fixture.workbook).unwrap();
        std::fs::copy(root.join("functions.csv"), &fixture.index).unwrap();
        std::fs::create_dir(&fixture.src).unwrap();
        for entry in std::fs::read_dir(root.join("src")).unwrap() {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            std::fs::copy(entry.path(), fixture.src.join(entry.file_name())).unwrap();
        }
        fixture
    }

    fn args(&self) -> Vec<String> {
        vec![display(&self.workbook), display(&self.src)]
    }

    fn cli(&self, args: &[String]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg("compile_sources")
            .args(args)
            .current_dir(self.directory.path())
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            String::from_utf8(output.stderr)
                .unwrap()
                .replace("\r\n", "\n"),
        )
    }

    fn snapshot(&self) -> archive::Parts {
        let mut files: archive::Parts = [
            ("ozzit.xlsx".into(), std::fs::read(&self.workbook).unwrap()),
            ("functions.csv".into(), std::fs::read(&self.index).unwrap()),
        ]
        .into_iter()
        .collect();
        for entry in std::fs::read_dir(&self.src).unwrap() {
            let entry = entry.unwrap();
            files.insert(
                format!("src/{}", entry.file_name().to_str().unwrap()),
                std::fs::read(entry.path()).unwrap(),
            );
        }
        files
    }

    fn checked(&self) -> String {
        format!(
            "OK: {} checked against {}/\n",
            display(&self.workbook),
            display(&self.src)
        )
    }
}

#[test]
fn unknown_flags_and_extra_paths_keep_distinct_usage_messages() {
    let fixture = Fixture::synthetic();
    let before = fixture.snapshot();
    for (args, flags) in [
        (vec!["--force".into()], " [--index=functions.csv]"),
        (vec!["a".into(), "b".into(), "c".into()], ""),
    ] {
        assert_eq!(
            fixture.cli(&args),
            (
                1,
                String::new(),
                format!(
                    "FAIL: usage: ozzit compile_sources [workbook] [src dir] [--check]{flags}\n"
                )
            )
        );
        assert_eq!(fixture.snapshot(), before);
    }
}

#[test]
fn missing_and_unreadable_workbooks_fail_without_writing() {
    let fixture = Fixture::empty();
    // Both paths are missing: workbook refusal must precede reading sources.
    assert_eq!(
        fixture.cli(&fixture.args()),
        (
            1,
            String::new(),
            format!("FAIL: no such workbook: {}\n", display(&fixture.workbook))
        )
    );
    assert!(!fixture.workbook.exists());
    assert!(!fixture.src.exists());
    std::fs::write(&fixture.workbook, b"not a zip archive").unwrap();
    // Supply the original complete source fixture so ZIP parsing is reached.
    std::fs::create_dir(&fixture.src).unwrap();
    for (name, source) in original()["sources"].as_object().unwrap() {
        std::fs::write(fixture.src.join(name), source.as_str().unwrap()).unwrap();
    }
    assert_eq!(
        fixture.cli(&fixture.args()),
        (1, String::new(), "FAIL: File is not a zip file\n".into())
    );
    assert_eq!(
        std::fs::read(&fixture.workbook).unwrap(),
        b"not a zip archive"
    );
    assert!(!fixture.index.exists());
}

#[test]
fn check_reports_only_the_stale_debt_definition_without_writing() {
    let fixture = Fixture::synthetic();
    let before = fixture.snapshot();
    let mut args = fixture.args();
    args.push("--check".into());
    assert_eq!(
        fixture.cli(&args),
        (
            0,
            format!("would recompile: oz.Debtλ\n{}", fixture.checked()),
            String::new()
        )
    );
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn recompile_updates_the_workbook_and_index_then_reports_no_changes() {
    let fixture = Fixture::synthetic();
    let mut args = fixture.args();
    args.push(format!("--index={}", display(&fixture.index)));
    assert_eq!(
        fixture.cli(&args),
        (
            0,
            format!(
                "recompiled: oz.Debtλ, functions.csv\nOK: {}\n",
                display(&fixture.workbook)
            ),
            String::new()
        )
    );
    let parts = archive::read_parts(&fixture.workbook).unwrap();
    assert_eq!(parts, original_parts("expected_parts"));
    let book = String::from_utf8(parts[archive::BOOK].clone()).unwrap();
    assert!(book.contains(concat!(
        "<definedName name=\"oz.Debtλ\" comment=\"The Debt sample\">",
        "_xlfn.LAMBDA(_xlop.x, _xlpm.x + 1)</definedName>"
    )));
    assert_eq!(
        std::fs::read_to_string(&fixture.index)
            .unwrap()
            .trim_start_matches('\u{feff}'),
        original()["expected_index"].as_str().unwrap()
    );
    let before = fixture.snapshot();
    assert_eq!(
        fixture.cli(&args),
        (
            0,
            format!(
                "every defined name already matches src/; no changes\nOK: {}\n",
                display(&fixture.workbook)
            ),
            String::new()
        )
    );
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn check_on_shipped_copies_reports_matching_names_without_writing() {
    let fixture = Fixture::shipped();
    let before = fixture.snapshot();
    let mut args = fixture.args();
    args.push("--check".into());
    assert_eq!(
        fixture.cli(&args),
        (
            0,
            format!(
                "every defined name already matches src/\n{}",
                fixture.checked()
            ),
            String::new()
        )
    );
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn omitted_paths_use_the_workbook_and_src_in_the_current_directory() {
    let fixture = Fixture::shipped();
    let before = fixture.snapshot();
    assert_eq!(
        fixture.cli(&[]),
        (
            0,
            "every defined name already matches src/; no changes\nOK: ozzit.xlsx\n".into(),
            String::new()
        )
    );
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn check_accepts_interleaved_flags_and_ignores_an_unreadable_index() {
    let fixture = Fixture::synthetic();
    let before = fixture.snapshot();
    let missing = fixture.directory.path().join("missing.csv");
    let args = vec![
        "--check".into(),
        display(&fixture.workbook),
        format!("--index={}", display(&missing)),
        display(&fixture.src),
    ];
    assert_eq!(
        fixture.cli(&args),
        (
            0,
            format!("would recompile: oz.Debtλ\n{}", fixture.checked()),
            String::new()
        )
    );
    assert_eq!(fixture.snapshot(), before);
    assert!(!missing.exists());
}

#[test]
fn the_last_index_flag_selects_the_only_index_to_write() {
    let fixture = Fixture::synthetic();
    let missing = fixture.directory.path().join("missing.csv");
    let args = vec![
        format!("--index={}", display(&missing)),
        display(&fixture.workbook),
        format!("--index={}", display(&fixture.index)),
        display(&fixture.src),
    ];
    assert_eq!(
        fixture.cli(&args),
        (
            0,
            format!(
                "recompiled: oz.Debtλ, functions.csv\nOK: {}\n",
                display(&fixture.workbook)
            ),
            String::new()
        )
    );
    assert_eq!(
        archive::read_parts(&fixture.workbook).unwrap(),
        original_parts("expected_parts")
    );
    assert_eq!(
        std::fs::read_to_string(&fixture.index).unwrap(),
        original()["expected_index"].as_str().unwrap()
    );
    assert!(!missing.exists());
}
