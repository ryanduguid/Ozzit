//! Physical and supplied-file contracts supplement the unchanged evidence replay.
use ozzit::{
    common::display,
    native_evidence as evidence,
    pyjson::{self, Json},
};
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
    root: PathBuf,
}
impl Fixture {
    fn original() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let fixture = Self {
            root: dir.path().join("root"),
            dir,
        };
        for name in [
            evidence::COMPARISON,
            evidence::INSTALLATION,
            evidence::RUNNERS[0],
            evidence::RUNNERS[1],
            evidence::INSTALL_RUNNER,
        ] {
            let path = fixture.root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::copy(root().join(name), path).unwrap();
        }
        std::fs::write(
            fixture.root.join("ozzit.xlsx"),
            b"Different synthetic workbook",
        )
        .unwrap();
        fixture
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
    fn verify(
        &self,
        workbook: Option<&Path>,
        destination: Option<&Path>,
    ) -> Result<Vec<String>, String> {
        let before = self.snapshot();
        let result = evidence::verify(&self.root, workbook, destination);
        assert_eq!(self.snapshot(), before);
        result
    }
    fn cli(&self, args: &[String]) -> std::process::Output {
        let executable = self
            .root
            .join(if cfg!(windows) { "ozzit.exe" } else { "ozzit" });
        std::fs::copy(env!("CARGO_BIN_EXE_ozzit"), &executable).unwrap();
        let before = self.snapshot();
        let output = Command::new(executable)
            .arg("verify_native_evidence")
            .args(args)
            .env("PYTHONIOENCODING", "utf-8")
            .current_dir(self.dir.path())
            .output()
            .unwrap();
        assert_eq!(self.snapshot(), before);
        output
    }
}

#[test]
fn the_original_supplied_installed_workbook_is_confirmed_without_release_confirmation() {
    let f = Fixture::original();
    std::fs::write(f.root.join("ozzit.xlsx"), b"synthetic checkout workbook").unwrap();
    let installed = f.dir.path().join("installed.xlsx");
    std::fs::write(&installed, b"synthetic installed workbook").unwrap();
    let path = f.root.join(evidence::INSTALLATION);
    let mut record = evidence::read_record(&path).unwrap();
    *record.get_mut("destination_sha256").unwrap() =
        Json::Str(evidence::sha256(&installed).unwrap());
    std::fs::write(path, pyjson::dumps(&record, false)).unwrap();

    let messages = f.verify(None, Some(&installed)).unwrap();
    assert!(
        messages.contains(&"Supplied installed workbook bytes match the retained evidence.".into())
    );
    assert!(
        !messages.contains(&"Supplied release workbook bytes match the retained evidence.".into())
    );
}

#[test]
fn both_original_supplied_file_refusals_preserve_every_owned_input() {
    let f = Fixture::original();
    let path = f.root.join("provided.xlsx");
    let original = b"synthetic mismatched workbook";
    std::fs::write(&path, original).unwrap();
    for (workbook, destination, label) in [
        (Some(path.as_path()), None, "release workbook"),
        (None, Some(path.as_path()), "installed workbook"),
    ] {
        assert_eq!(
            f.verify(workbook, destination).unwrap_err(),
            format!("{label} hash does not match the retained evidence")
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

#[test]
fn the_physical_default_prints_every_checkout_message_without_a_writer() {
    let f = Fixture::original();
    std::fs::copy(root().join("ozzit.xlsx"), f.root.join("ozzit.xlsx")).unwrap();
    let expected = evidence::verify(&root(), None, None).unwrap();
    assert_eq!(f.verify(None, None).unwrap(), expected);
    let output = f.cli(&[]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        format!("{}{}", expected.join(newline()), newline()).into_bytes()
    );
}

#[test]
fn the_physical_mismatched_release_has_the_original_exact_stdout_error() {
    let f = Fixture::original();
    let supplied = f.dir.path().join("release.xlsx");
    std::fs::write(&supplied, b"not the release workbook").unwrap();
    let output = f.cli(&["--workbook".into(), display(&supplied)]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        format!(
            "FAIL: native evidence: release workbook hash does not match the retained evidence{}",
            newline()
        )
        .into_bytes()
    );
}
