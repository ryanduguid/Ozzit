//! Physical comparator calls retain independent references and preserve inputs.
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn repository() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let directory = path.is_dir();
            result.insert(
                path.strip_prefix(root).unwrap().into(),
                if directory {
                    None
                } else {
                    Some(fs::read(&path).unwrap())
                },
            );
            if directory {
                visit(root, &path, result);
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}
struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new(prefix: &str) -> Self {
        let f = Self(
            tempfile::Builder::new()
                .prefix("comparator with spaces ")
                .tempdir()
                .unwrap(),
        );
        for directory in [
            ".github/workflows",
            "tools/ozzit/target/release",
            "src/nested",
        ] {
            fs::create_dir_all(f.root().join(directory)).unwrap();
        }
        let binary = if cfg!(windows) { "ozzit.exe" } else { "ozzit" };
        fs::copy(
            env!("CARGO_BIN_EXE_ozzit"),
            f.root().join("tools/ozzit/target/release").join(binary),
        )
        .unwrap();
        for path in ["functions.csv", "tools/released-names-v1.2.6.txt"] {
            fs::copy(repository().join(path), f.root().join(path)).unwrap();
        }
        let source =
            fs::read_to_string(repository().join("tools/verify_previous_names.py")).unwrap();
        fs::write(
            f.root().join("tools/verify_previous_names.py"),
            format!("{prefix}{source}"),
        )
        .unwrap();
        fs::write(
            f.root().join("src/Dates.txt"),
            "CountDOWλ = LAMBDA([Starts],",
        )
        .unwrap();
        fs::write(f.root().join("src/nested/keep.txt"), "keep").unwrap();
        fs::write(f.root().join("ozzit.xlsx"), b"abcde").unwrap();
        fs::write(f.root().join("CHANGELOG.md"), "unchanged").unwrap();
        f.workflow(
            "  run: tools/ozzit/target/release/ozzit verify_previous_names 'functions.csv'\n",
        );
        f
    }
    fn root(&self) -> &Path {
        self.0.path()
    }
    fn workflow(&self, text: &str) {
        fs::write(self.root().join(".github/workflows/verify.yml"), text).unwrap();
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ozzit"));
        c.arg("compare_gates")
            .arg(
                std::env::var("OZZIT_TEST_PYTHON")
                    .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into()),
            )
            .current_dir(self.root())
            .env("PYTHONDONTWRITEBYTECODE", "1");
        c
    }
    fn run(&self) -> (i32, String, String) {
        let before = snapshot(self.root());
        let output = self.command().output().unwrap();
        assert_eq!(snapshot(self.root()), before);
        channels(output)
    }
}
fn channels(output: std::process::Output) -> (i32, String, String) {
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
fn successful_lines() -> &'static str {
    "same: verify_previous_names (exit 0)\nsame: verify_previous_names on functions.csv without a renamed function's row (exit 1)\n"
}
#[test]
fn actual_reference_and_native_gate_match_on_original_and_broken_owned_inputs() {
    let f = Fixture::new("");
    assert_eq!(f.run(), (0, successful_lines().into(), String::new()));
}
#[test]
fn only_the_two_retired_references_are_excluded_even_when_files_exist() {
    let f = Fixture::new("");
    let retired = ["verify_help_spills", "verify_native_evidence"];
    let workflow = "run: tools/ozzit/target/release/ozzit verify_help_spills ozzit.xlsx\nrun: tools/ozzit/target/release/ozzit verify_native_evidence\n";
    f.workflow(&format!(
        "{workflow}run: tools/ozzit/target/release/ozzit verify_previous_names functions.csv\n"
    ));
    for gate in retired {
        fs::write(
            f.root().join(format!("tools/{gate}.py")),
            "raise AssertionError('retired reference must not execute')\n",
        )
        .unwrap();
    }
    assert_eq!(f.run(), (0, successful_lines().into(), String::new()));
    for gate in retired {
        fs::remove_file(f.root().join(format!("tools/{gate}.py"))).unwrap();
    }
    assert_eq!(f.run(), (0, successful_lines().into(), String::new()));
    f.workflow(workflow);
    assert_eq!(
        f.run(),
        (
            1,
            String::new(),
            "no Python reference gates found in .github/workflows/verify.yml\n".into()
        )
    );
}
#[test]
fn missing_required_and_unclassified_references_fail_and_continue_every_pair() {
    let f = Fixture::new("");
    fs::remove_file(f.root().join("tools/verify_previous_names.py")).unwrap();
    f.workflow("run: tools/ozzit/target/release/ozzit verify_previous_names functions.csv\nrun: tools/ozzit/target/release/ozzit verify_previous_names functions.csv\nrun: tools/ozzit/target/release/ozzit verify_unclassified\n");
    let (code, out, err) = f.run();
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert_eq!(err.matches("DIFFERENT:").count(), 5);
    assert_eq!(err.matches("--- ozzit:").count(), 5);
    assert_eq!(err.matches("--- python:").count(), 5);
    assert!(err.contains("verify_previous_names.py"));
    assert!(err.contains("DIFFERENT: verify_unclassified\n"));
}
#[test]
fn stderr_mismatch_reports_both_captures_and_continues_every_pair() {
    let f = Fixture::new("import os\nos.write(2, b'marker\\r\\n')\n");
    f.workflow("run: tools/ozzit/target/release/ozzit verify_previous_names functions.csv\nrun: tools/ozzit/target/release/ozzit verify_previous_names functions.csv\n");
    let (code, out, err) = f.run();
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert_eq!(err.matches("DIFFERENT:").count(), 4);
    assert_eq!(err.matches("--- ozzit:").count(), 4);
    assert_eq!(err.matches("--- python:").count(), 4);
    assert_eq!(err.matches("marker\n").count(), 4);
    assert!(!err.contains('\r'));
}
#[test]
fn malformed_workflow_and_invalid_child_utf8_fail_closed_without_writes() {
    let f = Fixture::new("");
    f.workflow("run: echo none\n");
    assert_eq!(
        f.run(),
        (
            1,
            String::new(),
            "no native verify_* gates found in .github/workflows/verify.yml\n".into()
        )
    );
    f.workflow("run: tools/ozzit/target/release/ozzit verify_previous_names 'bad\n");
    assert_eq!(
        f.run(),
        (
            1,
            String::new(),
            "ValueError: No closing quotation\n".into()
        )
    );
    f.workflow("run: tools/ozzit/target/release/ozzit verify_previous_names functions.csv\n");
    for stream in [1, 2] {
        fs::write(
            f.root().join("tools/verify_previous_names.py"),
            format!("import os\nos.write({stream},b'\\xff')\n"),
        )
        .unwrap();
        let (code, out, err) = f.run();
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert!(err.contains("utf-8"));
    }
    let before = snapshot(f.root());
    let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .args(["compare_gates", "missing-selected-python-ozzit-fixture"])
        .current_dir(f.root())
        .output()
        .unwrap();
    assert_eq!(snapshot(f.root()), before);
    let (code, out, err) = channels(output);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.starts_with("OSError:"));
}
#[test]
fn references_inherit_the_callers_open_stdin_pipe() {
    let f = Fixture::new("import os\nassert os.read(0, 1) == b'Q'\n");
    let before = snapshot(f.root());
    let mut child = f
        .command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"QQ").unwrap();
    assert_eq!(
        channels(child.wait_with_output().unwrap()),
        (0, successful_lines().into(), String::new())
    );
    assert_eq!(snapshot(f.root()), before);
}
#[test]
fn ordinary_workflow_selects_its_python_interpreter_for_the_native_comparator() {
    let text = fs::read_to_string(repository().join(".github/workflows/verify.yml")).unwrap();
    assert!(text.contains("run: tools/ozzit/target/release/ozzit compare_gates python\n"));
    let gates: Vec<_> = text
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("run: tools/ozzit/target/release/ozzit verify_")
        })
        .map(|line| line.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(
        gates,
        [
            "workbook",
            "sources",
            "signatures",
            "previous_names",
            "index",
            "cover",
            "afe",
            "help_spills",
            "native_evidence"
        ]
    );
    for retired in ["verify_help_spills", "verify_native_evidence"] {
        assert!(!repository().join(format!("tools/{retired}.py")).exists());
    }
    assert!(!repository().join(".github/ci/compare_gates.py").exists());
    assert!(
        !repository()
            .join("tools/tests/test_compare_gates.py")
            .exists()
    );
}
