//! Replay the retired prepare_release_bundle.py's recorded argument parsing and
//! verification results, with recorded bundles moved to provenance schema 2.
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

/// The schema 1 gate labels the Python tool recorded, and their schema 2 names.
const LABELS: [(&str, &str); 7] = [
    (
        "python tools/verify_workbook.py ozzit.xlsx",
        "ozzit verify_workbook ozzit.xlsx",
    ),
    (
        "python tools/verify_sources.py ozzit.xlsx src",
        "ozzit verify_sources ozzit.xlsx src",
    ),
    (
        "python tools/verify_signatures.py src",
        "ozzit verify_signatures src",
    ),
    (
        "python tools/verify_previous_names.py functions.csv",
        "ozzit verify_previous_names functions.csv",
    ),
    (
        "python tools/verify_index.py ozzit.xlsx src functions.csv",
        "ozzit verify_index ozzit.xlsx src functions.csv",
    ),
    (
        "python tools/verify_cover.py ozzit.xlsx functions.csv CHANGELOG.md",
        "ozzit verify_cover ozzit.xlsx functions.csv CHANGELOG.md",
    ),
    (
        "python tools/verify_afe.py ozzit.xlsx src",
        "ozzit verify_afe ozzit.xlsx src",
    ),
];

fn replace_bytes(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}

/// Each integer `"schema"` member moves up by one, so recorded 1 becomes the
/// supported 2 and every recorded refusal (2, 0, true, 1.0, ...) stays refused.
fn shift_schema(data: &[u8]) -> Vec<u8> {
    let key = b"\"schema\"";
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if !data[i..].starts_with(key) {
            out.push(data[i]);
            i += 1;
            continue;
        }
        let mut j = i + key.len();
        while data.get(j).is_some_and(u8::is_ascii_whitespace) {
            j += 1;
        }
        if data.get(j) != Some(&b':') {
            out.extend_from_slice(&data[i..j]);
            i = j;
            continue;
        }
        j += 1;
        while data.get(j).is_some_and(u8::is_ascii_whitespace) {
            j += 1;
        }
        let digits = j;
        while data.get(j).is_some_and(u8::is_ascii_digit) {
            j += 1;
        }
        let whole = j > digits
            && data
                .get(j)
                .is_none_or(|b| b.is_ascii_whitespace() || matches!(b, b',' | b'}' | b']'));
        out.extend_from_slice(&data[i..digits]);
        if whole {
            let n: u64 = std::str::from_utf8(&data[digits..j])
                .unwrap()
                .parse()
                .unwrap();
            out.extend_from_slice((n + 1).to_string().as_bytes());
        } else {
            out.extend_from_slice(&data[digits..j]);
        }
        i = j;
    }
    out
}

fn hex_sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The Python tool recorded schema 1 bundles; the native tool now accepts only
/// schema 2. Move each recorded provenance to schema 2 bytes, preserving its
/// formatting and defects, and repoint a SHA256SUMS digest that matched it.
fn schema_two(files: &Value) -> Value {
    let mut files = files.clone();
    let map = files.as_object_mut().unwrap();
    let decode = |v: &Value| {
        v.as_str()
            .map(|s| base64::engine::general_purpose::STANDARD.decode(s).unwrap())
    };
    let provenance_paths: Vec<String> = map
        .keys()
        .filter(|k| k.ends_with("provenance.json"))
        .cloned()
        .collect();
    for path in provenance_paths {
        let Some(old) = decode(&map[&path]) else {
            continue;
        };
        let mut new = shift_schema(&old);
        for (from, to) in LABELS {
            let quoted = |s: &str| format!("\"{s}\"").into_bytes();
            new = replace_bytes(&new, &quoted(from), &quoted(to));
        }
        let sums = format!(
            "{}SHA256SUMS",
            &path[..path.len() - "provenance.json".len()]
        );
        if let Some(listing) = map.get(&sums).and_then(decode) {
            let listing = replace_bytes(
                &listing,
                format!("{}  provenance.json", hex_sha256(&old)).as_bytes(),
                format!("{}  provenance.json", hex_sha256(&new)).as_bytes(),
            );
            map.insert(
                sums,
                json!(base64::engine::general_purpose::STANDARD.encode(listing)),
            );
        }
        map.insert(
            path,
            json!(base64::engine::general_purpose::STANDARD.encode(new)),
        );
    }
    files
}

#[test]
fn schema_two_adapter_preserves_each_recorded_refusal() {
    assert_eq!(
        shift_schema(br#"{"schema":1,"x":{"schema" : 2 },"schema":true,"schema":1.0}"#),
        br#"{"schema":2,"x":{"schema" : 3 },"schema":true,"schema":1.0}"#
    );
    let provenance =
        br#"{"schema":1,"static_gates":[{"command":"python tools/verify_afe.py ozzit.xlsx src"}]}"#;
    let sums = format!(
        "{}  ozzit.xlsx\n{}  provenance.json\n",
        "0".repeat(64),
        hex_sha256(provenance)
    );
    let encode = |b: &[u8]| json!(base64::engine::general_purpose::STANDARD.encode(b));
    let moved = schema_two(&json!({
        "bundle/provenance.json": encode(provenance),
        "bundle/SHA256SUMS": encode(sums.as_bytes()),
        "repo/release/workbook-base.json": encode(br#"{"schema":1}"#),
    }));
    let new = br#"{"schema":2,"static_gates":[{"command":"ozzit verify_afe ozzit.xlsx src"}]}"#;
    assert_eq!(moved["bundle/provenance.json"], encode(new));
    assert_eq!(
        moved["bundle/SHA256SUMS"],
        encode(
            format!(
                "{}  ozzit.xlsx\n{}  provenance.json\n",
                "0".repeat(64),
                hex_sha256(new)
            )
            .as_bytes()
        )
    );
    assert_eq!(
        moved["repo/release/workbook-base.json"],
        encode(br#"{"schema":1}"#)
    );
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
        write_files(&tmp, &schema_two(&case["files"]));
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
