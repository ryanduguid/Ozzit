// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn text_inputs() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("src")).unwrap();
    for module in ozzit::formula::MODULES {
        let name = format!("src/{module}.txt");
        fs::copy(repo().join(&name), temp.path().join(name)).unwrap();
    }
    fs::copy(
        repo().join("release/workbook-source.json"),
        temp.path().join("layout.json"),
    )
    .unwrap();
    temp
}

fn invoke(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .current_dir(dir)
        .arg("workbook_source")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn clean_text_inputs_regenerate_the_shipped_bytes_and_round_trip() {
    let temp = text_inputs();
    let dir = temp.path();
    let build = ["build", "layout.json", "src", "result.xlsx"];
    let result = invoke(dir, &build);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(dir.join("result.xlsx")).unwrap();
    assert!(
        bytes == fs::read(repo().join("ozzit.xlsx")).unwrap(),
        "regenerated workbook differs"
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(repo().join("release/workbook-base.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["sha256"], ozzit::common::sha256_hex(&bytes));
    assert!(
        !invoke(dir, &build).status.success(),
        "must refuse an existing output"
    );
    assert!(bytes == fs::read(dir.join("result.xlsx")).unwrap());
    let snapshot = invoke(dir, &["capture", "result.xlsx", "src", "recaptured.json"]);
    assert!(
        snapshot.status.success(),
        "{}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    assert!(
        fs::read(dir.join("layout.json")).unwrap()
            == fs::read(dir.join("recaptured.json")).unwrap(),
        "baseline capture drifted"
    );
}

#[test]
fn stale_formulas_and_invalid_layouts_leave_no_workbook() {
    let temp = text_inputs();
    let dir = temp.path();
    let changed = dir.join("src/Financial.txt");
    let original = fs::read_to_string(&changed).unwrap();
    let modified = original.replacen("LAMBDA(", "LAMBDA(InspectedParameter,", 1);
    assert_ne!(original, modified);
    fs::write(changed, modified).unwrap();
    let build = ["build", "layout.json", "src", "changed.xlsx"];
    let stale = invoke(dir, &build);
    assert_eq!(stale.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("changed: capture the verified workbook")
    );
    assert!(!dir.join("changed.xlsx").exists());
    fs::write(dir.join("layout.json"), r#"{"afe":[],"parts":{}}"#).unwrap();
    let result = invoke(dir, &build);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("missing AFE files"));
    assert!(!dir.join("changed.xlsx").exists());
}

#[test]
fn recorded_windows_spills_match_the_shared_compatibility_cases() {
    let cases = fs::read(repo().join("tools/compatibility-cases.json")).unwrap();
    let evidence: Value = serde_json::from_slice(
        &fs::read(repo().join("docs/compatibility-windows-2026-11-23.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(evidence["cases_sha256"], ozzit::common::sha256_hex(&cases));
    for (field, path) in [
        ("workbook_sha256", "ozzit.xlsx"),
        ("helper_sha256", "tools/excel_eval_formulas.ps1"),
        (
            "evidence_helper_sha256",
            "tools/excel_eval_formulas_verified.ps1",
        ),
    ] {
        let committed = Command::new("git")
            .current_dir(repo())
            .args([
                "show",
                &format!("4db806839abc83466f575807c275b6182bb558c5:{path}"),
            ])
            .output()
            .unwrap();
        assert!(
            committed.status.success(),
            "missing historical input {path}"
        );
        assert_eq!(
            evidence[field],
            ozzit::common::sha256_hex(&committed.stdout)
        );
    }
    let cases: Vec<Value> = serde_json::from_slice(&cases).unwrap();
    let office = fs::read_to_string(repo().join("tools/excel_compatibility.ts")).unwrap();
    let embedded = office
        .split_once("const probes: ProbeCase[] = ")
        .unwrap()
        .1
        .split_once(';')
        .unwrap()
        .0;
    assert_eq!(serde_json::from_str::<Vec<Value>>(embedded).unwrap(), cases);
    assert_eq!(cases.len(), 4);
    assert_eq!(evidence["results"].as_object().unwrap().len(), cases.len());
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert_eq!(evidence["results"][id], case["expected"], "{id}");
    }
}
