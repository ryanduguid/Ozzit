//! Replay prepare_release_bundle.py's recorded argument parsing and verification results.
use base64::Engine;
use ozzit::{py::PyPath, release};
use serde_json::{Value, json};
use std::path::Path;

fn corpus() -> Value {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/release-contracts.json.gz"),
    )
    .unwrap();
    serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap()
}

fn printed(lines: &[String]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// The parsed namespace as the recorder printed it.
fn namespace(a: &release::Arguments) -> Value {
    let mut namespace = serde_json::Map::new();
    namespace.insert("command".into(), json!(a.command));
    let fields: &[(&str, &Option<String>)] = if a.command == "create" {
        &[
            ("version", &a.version),
            ("source_commit", &a.source_commit),
            ("output", &a.output),
        ]
    } else {
        &[
            ("bundle", &a.bundle),
            ("version", &a.version),
            ("source_commit", &a.source_commit),
        ]
    };
    for (name, value) in fields {
        // argparse's type=Path prints a value through pathlib.
        let shown = value.as_ref().map(|v| {
            if matches!(*name, "output" | "bundle") {
                PyPath::new(v).display()
            } else {
                v.clone()
            }
        });
        namespace.insert((*name).into(), json!(shown));
    }
    json!({"code": 0, "stdout": "", "stderr": "", "namespace": namespace})
}

#[test]
fn arguments_parse_as_argparse_does() {
    let data = corpus();
    let vectors = data["argparse"].as_array().unwrap();
    assert_eq!(vectors.len(), 55);
    let mut differences = Vec::new();
    for record in vectors {
        let argv = record["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        let got = match release::parse(&argv) {
            Ok(a) => namespace(&a),
            Err(output) => json!({
                "code": output.code,
                "stdout": printed(&output.out),
                "stderr": printed(&output.err),
                "namespace": null,
            }),
        };
        let mut expected = record.clone();
        expected.as_object_mut().unwrap().remove("argv");
        if got != expected {
            differences.push(format!(
                "{argv:?}:\n  got      {got}\n  expected {expected}"
            ));
        }
    }
    assert!(
        differences.is_empty(),
        "{} differences:\n{}",
        differences.len(),
        differences.join("\n")
    );
}

/// Writes each recorded file, or makes the directory for a null entry.
fn write_files(tmp: &Path, files: &Value) {
    for (rel, data) in files.as_object().unwrap() {
        let path = tmp.join(rel);
        match data.as_str() {
            None => std::fs::create_dir_all(&path).unwrap(),
            Some(encoded) => {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(
                    &path,
                    base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .unwrap(),
                )
                .unwrap();
            }
        }
    }
}

/// The recorded function's result: the bundle version, if it reports one.
fn run_case(case: &Value, tmp: &Path, args: &[String]) -> Result<Option<String>, String> {
    let root = tmp.join("repo");
    match case["function"].as_str().unwrap() {
        "verify_bundle" => release::verify_bundle(&tmp.join(&args[0]), &root, false)
            .map(|d| d.get("version").and_then(|v| v.as_str()).map(String::from)),
        "verify_base_workbook" => {
            release::verify_base_workbook(&tmp.join(&args[0]), &tmp.join(&args[1])).map(|_| None)
        }
        "parse_checksums" => release::parse_checksums(&tmp.join(&args[0])).map(|_| None),
        _ => {
            let output = args[0].replace("<tmp>", &tmp.display().to_string());
            release::create_bundle(
                &root,
                &root.join("ozzit.xlsx"),
                &root.join(release::BASE_MANIFEST),
                &PyPath::new(&output),
                "3.2.0",
                &"a".repeat(40),
            )
            .map(|_| None)
        }
    }
}

#[test]
fn verification_results_match_the_python_tool() {
    let data = corpus();
    let cases = data["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 91);
    let mut differences = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let tmp = ozzit::py::resolve(dir.path());
        write_files(&tmp, &case["files"]);
        let args = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        let result = run_case(case, &tmp, &args);
        let got = match result {
            Ok(version) => json!({"ok": true, "version": version}),
            Err(error) => json!({
                "error": error
                    .replace(&tmp.display().to_string(), "<tmp>")
                    .replace('\\', "/"),
            }),
        };
        let mut expected = case["expected"].clone();
        if let Some(error) = expected.get("error").and_then(Value::as_str) {
            expected = json!({"error": error.replace('\\', "/")});
        }
        if got != expected {
            differences.push(format!("{name}: got {got}, expected {expected}"));
        }
    }
    assert!(
        differences.is_empty(),
        "{} differences:\n{}",
        differences.len(),
        differences.join("\n")
    );
}
