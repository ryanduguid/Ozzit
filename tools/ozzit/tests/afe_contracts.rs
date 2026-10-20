//! AFE contracts from test_afe_paths.py, plus native writer safety checks.
use base64::Engine;
use ozzit::{
    afe, archive,
    common::display,
    compiler, formula,
    pyjson::{self, Json},
};
use std::path::{Path, PathBuf};
use std::process::Command;

struct Fixture {
    _directory: tempfile::TempDir,
    workbook: PathBuf,
    src: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let workbook = directory.path().join("ozzit.xlsx");
        let src = directory.path().join("src");
        std::fs::create_dir(&src).unwrap();
        Self {
            _directory: directory,
            workbook,
            src,
        }
    }
    fn store(&self, value: serde_json::Value, names: &[&str]) {
        let encoded = base64::engine::general_purpose::STANDARD.encode(
            value
                .to_string()
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        let mut parts = archive::Parts::new();
        parts.insert(
            "customXml/item1.xml".into(),
            format!("<store>{encoded}</store>").into_bytes(),
        );
        parts.insert(
            archive::BOOK.into(),
            format!(
                "<workbook><definedNames>{}</definedNames></workbook>",
                names
                    .iter()
                    .map(|n| format!("<definedName name=\"{n}\">1</definedName>"))
                    .collect::<String>()
            )
            .into_bytes(),
        );
        parts.insert("opaque.bin".into(), vec![0, 255, 128, 33]);
        archive::write_deterministic(&self.workbook, &parts).unwrap();
    }
    fn sync_fixture() -> Self {
        let fixture = Self::new();
        for (module, text) in [
            ("Dates", "Aλ = LAMBDA(x, x);\nCλ = LAMBDA(x, x + 1);\n"),
            ("Essentials", "Bλ = LAMBDA(x, x);\n"),
            ("Financial", ""),
            ("Ratios", ""),
            ("Utilities", "not a declaration;\n"),
            ("Debt", ""),
        ] {
            std::fs::write(fixture.src.join(format!("{module}.txt")), text).unwrap();
        }
        fixture.store(
            serde_json::json!({
                "schema": afe::SCHEMA,
                "files": [{"path":"/projects/Dates", "text":"stale"},
                    {"path":"/projects/Essentials", "text":"Bλ = LAMBDA(x, x);\n"}],
                "projectNames": ["oz.Aλ", "oz.Bλ", "oz.Goneλ"],
                "extension": {"keep": [1, "λ", null]},
            }),
            &["oz.Aλ", "oz.Bλ", "oz.Cλ", "oz.Zλ"],
        );
        fixture
    }
}

#[test]
fn blob_selection_uses_longest_run_and_first_tie_without_decoding_xml() {
    assert_eq!(
        afe::find_blob(b"<store>short</store>").unwrap_err(),
        "AFE store has no base64 payload"
    );
    let a = "A".repeat(100);
    let b = "B".repeat(104);
    let raw = format!("{a}==!{b}?!{b}");
    assert_eq!(afe::find_blob(raw.as_bytes()).unwrap(), b.as_bytes());
}

#[test]
fn malformed_store_shapes_are_refused_before_synchronisation_checks() {
    let fixture = Fixture::new();
    for (mut value, message) in [
        (
            serde_json::json!({"schema":1,"files":[],"projectNames":[]}),
            "schema must be a string",
        ),
        (
            serde_json::json!({"schema":afe::SCHEMA,"files":{},"projectNames":[]}),
            "files must be a list",
        ),
        (
            serde_json::json!({"schema":afe::SCHEMA,"files":["Dates"],"projectNames":[]}),
            "files[0] must be an object",
        ),
        (
            serde_json::json!({"schema":afe::SCHEMA,"files":[{"path":3,"text":"x"}],"projectNames":[]}),
            "files[0] path and text must be strings",
        ),
        (
            serde_json::json!({"schema":afe::SCHEMA,"files":[],"projectNames":[3]}),
            "projectNames must be a list of strings",
        ),
    ] {
        value["padding"] = "x".repeat(80).into();
        fixture.store(value, &[]);
        assert_eq!(
            afe::decode_store(&fixture.workbook).unwrap_err(),
            format!("cannot decode AFE store: {message}")
        );
    }
}

#[test]
fn garbled_payloads_and_unreadable_stores_report_one_controlled_failure() {
    let fixture = Fixture::new();
    let failure = afe::check(&fixture.workbook, &fixture.src);
    assert_eq!(failure.len(), 1);
    assert!(failure[0].starts_with("cannot read AFE store: "));
    for (size, detail) in [(100, "truncated data"), (104, "Expecting value")] {
        let mut parts = archive::Parts::new();
        parts.insert(
            "customXml/item1.xml".into(),
            format!("<store>{}</store>", "A".repeat(size)).into_bytes(),
        );
        archive::write_deterministic(&fixture.workbook, &parts).unwrap();
        let error = afe::decode_store(&fixture.workbook).unwrap_err();
        assert!(error.starts_with("cannot decode AFE store: "), "{error}");
        assert!(error.contains(detail), "{error}");
    }
}

#[test]
fn utf16_decoder_rejects_each_incomplete_or_unpaired_sequence() {
    for (bytes, detail) in [
        (vec![0], "truncated data"),
        (vec![0, 0xd8], "unexpected end of data"),
        (vec![0, 0xd8, 0, 0], "illegal UTF-16 surrogate"),
        (vec![0, 0xdc], "illegal encoding"),
    ] {
        assert!(afe::utf16le(&bytes).unwrap_err().contains(detail));
    }
    assert_eq!(afe::utf16le(&[0x3d, 0xd8, 0, 0xde]).unwrap(), "😀");
}

#[test]
fn schema_missing_source_and_unknown_name_are_independent_failures() {
    let fixture = Fixture::new();
    let files = formula::MODULES
        .iter()
        .map(|module| {
            let text = format!("{module} source\n");
            if *module != "Dates" {
                std::fs::write(fixture.src.join(format!("{module}.txt")), &text).unwrap();
            }
            serde_json::json!({"path":format!("/projects/{module}"), "text":text})
        })
        .collect::<Vec<_>>();
    fixture.store(
        serde_json::json!({"schema":"http://example.invalid/afeprojects/0.1", "files":files,
        "projectNames":["oz.Keptλ", "oz.Ghostλ"]}),
        &["oz.Keptλ"],
    );
    assert_eq!(
        afe::check(&fixture.workbook, &fixture.src),
        vec![
            "AFE schema is 'http://example.invalid/afeprojects/0.1'".into(),
            format!(
                "source module {} missing",
                display(&fixture.src.join("Dates.txt"))
            ),
            "AFE projectNames has unknown name oz.Ghostλ".into(),
        ]
    );
}

#[test]
fn sync_adds_modules_groups_names_preserves_extensions_and_is_idempotent() {
    let fixture = Fixture::sync_fixture();
    assert_eq!(
        compiler::source_modules(&fixture.src).unwrap(),
        [
            ("oz.Aλ".into(), "Dates".into()),
            ("oz.Cλ".into(), "Dates".into()),
            ("oz.Bλ".into(), "Essentials".into()),
        ]
        .into_iter()
        .collect()
    );
    let original = archive::read_parts(&fixture.workbook).unwrap();
    assert_eq!(
        afe::sync(&fixture.workbook, &fixture.src).unwrap(),
        vec![
            "synced AFE module /projects/Dates",
            "added AFE module /projects/Financial",
            "added AFE module /projects/Ratios",
            "added AFE module /projects/Utilities",
            "added AFE module /projects/Debt",
            "synced AFE projectNames: 2 added, 1 dropped, 4 listed",
        ]
    );
    let store = afe::decode_store(&fixture.workbook).unwrap();
    assert_eq!(
        store.get("projectNames"),
        Some(&Json::Array(
            ["oz.Aλ", "oz.Cλ", "oz.Bλ", "oz.Zλ"]
                .map(Json::str_obj)
                .to_vec()
        ))
    );
    assert_eq!(
        pyjson::dumps_compact(store.get("extension").unwrap()),
        r#"{"keep":[1,"λ",null]}"#
    );
    assert!(afe::check(&fixture.workbook, &fixture.src).is_empty());
    for (name, bytes) in archive::read_parts(&fixture.workbook).unwrap() {
        if name != "customXml/item1.xml" {
            assert_eq!(bytes, original[&name], "{name}");
        }
    }
    let once = std::fs::read(&fixture.workbook).unwrap();
    assert_eq!(
        afe::sync(&fixture.workbook, &fixture.src).unwrap(),
        vec!["AFE store already matches src/"]
    );
    assert_eq!(std::fs::read(&fixture.workbook).unwrap(), once);
}

#[test]
fn failed_sync_keeps_every_original_workbook_byte() {
    let fixture = Fixture::sync_fixture();
    let before = std::fs::read(&fixture.workbook).unwrap();
    std::fs::remove_file(fixture.src.join("Debt.txt")).unwrap();
    assert!(afe::sync(&fixture.workbook, &fixture.src).is_err());
    assert_eq!(std::fs::read(&fixture.workbook).unwrap(), before);
}

fn sync_cli(fixture: &Fixture, args: &[String], encoding: &str) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .arg("sync_afe_store")
        .args(args)
        .env("PYTHONIOENCODING", encoding)
        .current_dir(fixture._directory.path())
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        // Match the former subprocess tests' universal-newline text mode.
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        String::from_utf8(output.stderr)
            .unwrap()
            .replace("\r\n", "\n"),
    )
}

fn sync_args(fixture: &Fixture) -> Vec<String> {
    vec![display(&fixture.workbook), display(&fixture.src)]
}

fn sync_status(workbook: &Path, encoding: &str) -> String {
    let mut size = std::fs::metadata(workbook).unwrap().len();
    let mut groups = Vec::new();
    while size >= 1000 {
        groups.push(format!("{:03}", size % 1000));
        size /= 1000;
    }
    groups.push(size.to_string());
    groups.reverse();
    let path = display(workbook);
    let path = if encoding == "cp1252" {
        path.replace('λ', r"\u03bb").replace('→', r"\u2192")
    } else {
        path
    };
    format!("OK: {path} AFE store synced, {} bytes\n", groups.join(","))
}

#[test]
fn sync_main_prints_each_change_and_the_synced_size() {
    let fixture = Fixture::sync_fixture();
    let (code, stdout, stderr) = sync_cli(&fixture, &sync_args(&fixture), "utf-8");
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
    assert_eq!(
        stdout,
        format!(
            "synced AFE module /projects/Dates\nadded AFE module /projects/Financial\nadded AFE module /projects/Ratios\nadded AFE module /projects/Utilities\nadded AFE module /projects/Debt\nsynced AFE projectNames: 2 added, 1 dropped, 4 listed\n{}",
            sync_status(&fixture.workbook, "utf-8")
        )
    );
    assert_eq!(stdout.lines().count(), 7);
    assert!(afe::check(&fixture.workbook, &fixture.src).is_empty());
}

#[test]
fn sync_main_refuses_bad_arguments_and_unreadable_workbooks() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.workbook, b"not a zip archive").unwrap();
    let missing = fixture._directory.path().join("missing.xlsx");
    for (args, message) in [
        (
            vec!["a".into(), "b".into(), "c".into()],
            "FAIL: usage: ozzit sync_afe_store [workbook] [src dir]".into(),
        ),
        (
            vec![display(&missing)],
            format!("FAIL: no such workbook: {}", display(&missing)),
        ),
        (
            sync_args(&fixture),
            "FAIL: cannot sync AFE store: File is not a zip file".into(),
        ),
    ] {
        assert_eq!(
            sync_cli(&fixture, &args, "utf-8"),
            (1, String::new(), format!("{message}\n"))
        );
        assert_eq!(
            std::fs::read(&fixture.workbook).unwrap(),
            b"not a zip archive"
        );
        assert!(!missing.exists());
        assert_eq!(std::fs::read_dir(&fixture.src).unwrap().count(), 0);
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn sync_reports_success_in_supported_encodings() {
    let mut fixture = Fixture::new();
    fixture.workbook = fixture._directory.path().join("synced-λ→.xlsx");
    fixture.src = repository_root().join("src");
    let original = std::fs::read(repository_root().join("ozzit.xlsx")).unwrap();
    std::fs::write(&fixture.workbook, &original).unwrap();
    for encoding in ["cp1252", "utf-8"] {
        assert_eq!(
            sync_cli(&fixture, &sync_args(&fixture), encoding),
            (
                0,
                format!(
                    "AFE store already matches src/\n{}",
                    sync_status(&fixture.workbook, encoding)
                ),
                String::new()
            ),
            "{encoding}"
        );
        assert_eq!(std::fs::read(&fixture.workbook).unwrap(), original);
    }
}

fn stale_dates_parts(original: &archive::Parts, current: &Json) -> archive::Parts {
    let raw = &original["customXml/item1.xml"];
    let encoded = afe::find_blob(raw).unwrap();
    let mut stale: serde_json::Value =
        serde_json::from_str(&pyjson::dumps_compact(current)).unwrap();
    let dates = stale["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["path"] == "/projects/Dates")
        .unwrap();
    let text = dates["text"].as_str().unwrap();
    let changed = text.replacen("20 Aug 2026", "19 Aug 2026", 1);
    assert_ne!(text, changed);
    dates["text"] = changed.into();
    let replacement = base64::engine::general_purpose::STANDARD.encode(
        stale
            .to_string()
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let mut parts = original.clone();
    let raw = String::from_utf8(raw.clone()).unwrap();
    let encoded = std::str::from_utf8(encoded).unwrap();
    parts.insert(
        "customXml/item1.xml".into(),
        raw.replace(encoded, &replacement).into_bytes(),
    );
    parts
}

#[test]
fn sync_reports_success_after_update_in_supported_encodings() {
    let mut fixture = Fixture::new();
    fixture.workbook = fixture._directory.path().join("updated-λ→.xlsx");
    fixture.src = repository_root().join("src");
    let shipped = repository_root().join("ozzit.xlsx");
    let current = afe::decode_store(&shipped).unwrap();
    let original = archive::read_parts(&shipped).unwrap();
    let parts = stale_dates_parts(&original, &current);
    for encoding in ["cp1252", "utf-8"] {
        archive::write_deterministic(&fixture.workbook, &parts).unwrap();
        assert_ne!(afe::decode_store(&fixture.workbook).unwrap(), current);
        let failures = afe::check(&fixture.workbook, &fixture.src);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].starts_with("AFE module /projects/Dates differs"));
        let result = sync_cli(&fixture, &sync_args(&fixture), encoding);
        assert_eq!(
            result,
            (
                0,
                format!(
                    "synced AFE module /projects/Dates\n{}",
                    sync_status(&fixture.workbook, encoding)
                ),
                String::new()
            ),
            "{encoding}"
        );
        assert_eq!(afe::decode_store(&fixture.workbook).unwrap(), current);
        let restored = archive::read_parts(&fixture.workbook).unwrap();
        assert_eq!(restored.len(), original.len());
        for (name, data) in &original {
            if name != "customXml/item1.xml" {
                assert_eq!(restored[name], *data, "{encoding}: {name}");
            }
        }
    }
}

#[test]
fn sync_default_arguments_select_only_the_owned_workbook_and_source() {
    let fixture = Fixture::sync_fixture();
    let (code, stdout, stderr) = sync_cli(&fixture, &[], "utf-8");
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
    assert_eq!(stdout.lines().count(), 7);
    assert!(stdout.starts_with("synced AFE module /projects/Dates\n"));
    let status =
        sync_status(&fixture.workbook, "utf-8").replace(&display(&fixture.workbook), "ozzit.xlsx");
    assert!(stdout.ends_with(&status));
    assert!(afe::check(&fixture.workbook, &fixture.src).is_empty());
    let original = std::fs::read(&fixture.workbook).unwrap();
    assert_eq!(
        sync_cli(&fixture, &[display(&fixture.workbook)], "utf-8"),
        (
            0,
            format!(
                "AFE store already matches src/\n{}",
                sync_status(&fixture.workbook, "utf-8")
            ),
            String::new()
        )
    );
    assert_eq!(std::fs::read(&fixture.workbook).unwrap(), original);
}

#[test]
fn duplicates_are_reported_even_when_the_final_copy_matches() {
    let fixture = Fixture::sync_fixture();
    afe::sync(&fixture.workbook, &fixture.src).unwrap();
    let store = afe::decode_store(&fixture.workbook).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_str(&pyjson::dumps_compact(&store)).unwrap();
    let duplicate = value["files"][0].clone();
    value["files"].as_array_mut().unwrap().insert(0, duplicate);
    value["projectNames"]
        .as_array_mut()
        .unwrap()
        .push("oz.Aλ".into());
    fixture.store(value, &["oz.Aλ", "oz.Bλ", "oz.Cλ", "oz.Zλ"]);
    assert_eq!(
        afe::check(&fixture.workbook, &fixture.src),
        vec![
            "AFE file path /projects/Dates appears 2 times",
            "AFE projectNames entry oz.Aλ appears 2 times",
        ]
    );
}

#[test]
fn module_help_and_invalid_options_do_not_access_inputs_or_write_destinations() {
    let fixture = Fixture::new();
    let output = fixture.src.join("result.txt");
    std::fs::write(&output, b"sentinel").unwrap();
    let mut args = vec![
        display(Path::new("missing-src")),
        display(&output),
        "missing.xlsx".into(),
        "missing.csv".into(),
    ];
    for flag in ["--help", "--chek", "extra.txt"] {
        args.push(flag.into());
        let result = ozzit::module::command(&args);
        assert_eq!(result.code, if flag == "--help" { 0 } else { 2 });
        assert_eq!(std::fs::read(&output).unwrap(), b"sentinel");
        args.pop();
    }
}
