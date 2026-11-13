// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! The per-function record of native checks stays bound to the files it is read from.
use ozzit::native_checks::{OUTPUT, record};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .arg("native_checks")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    let text =
        String::from_utf8(output.stdout).unwrap() + &String::from_utf8(output.stderr).unwrap();
    (output.status.code().unwrap(), text)
}

#[test]
fn the_tracked_record_matches_its_sources() {
    let tracked = std::fs::read_to_string(root().join(OUTPUT)).unwrap();
    assert_eq!(
        tracked,
        record(&root()).unwrap(),
        "{OUTPUT} is stale: run `ozzit native_checks` from the repository root"
    );
}

#[test]
fn the_command_writes_then_checks_the_record() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "functions.csv",
        "ozzit.xlsx",
        "tools/excel_selftest.ps1",
        "tools/selftest_examples.ps1",
    ] {
        let to = dir.path().join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(file), to).unwrap();
    }
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    let written = dir.path().join(OUTPUT);

    let (code, text) = run(dir.path(), &["--check"]);
    assert_eq!(
        (code, text.trim()),
        (
            1,
            "FAIL: docs/native-checks.csv is stale; run ozzit native_checks"
        )
    );
    assert!(!written.exists());

    let (code, text) = run(dir.path(), &[]);
    assert_eq!(code, 0, "{text}");
    assert!(text.starts_with("wrote docs/native-checks.csv: "));
    assert_eq!(
        std::fs::read_to_string(&written).unwrap(),
        std::fs::read_to_string(root().join(OUTPUT)).unwrap()
    );

    let (code, text) = run(dir.path(), &["--check"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.starts_with("OK: docs/native-checks.csv is current"));

    let (code, text) = run(dir.path(), &["--write"]);
    assert_eq!(
        (code, text.trim()),
        (1, "FAIL: usage: ozzit native_checks [--check]")
    );

    std::fs::remove_file(dir.path().join("functions.csv")).unwrap();
    let (code, text) = run(dir.path(), &[]);
    assert_eq!(code, 1);
    assert!(text.starts_with("FAIL: "), "{text}");
}
