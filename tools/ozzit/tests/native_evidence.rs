use base64::Engine;
use ozzit::{
    native_evidence as evidence,
    pyjson::{self, Json},
};
use serde_json::{Value, json};
use std::path::{Component, Path};

#[test]
fn unique_keys_apply_at_each_object_close_without_changing_default_json() {
    for input in [r#"{"a":0,"a":1}"#, r#"{"a":[{"x":1,"\u0078":2}]}"#] {
        assert_eq!(
            pyjson::loads_unique(input).unwrap_err().to_string(),
            "duplicate evidence key"
        );
        assert!(pyjson::loads(input).is_ok());
    }
    assert!(pyjson::loads_unique(r#"{"a":{"x":1},"b":{"x":2}}"#).is_ok());
    assert!(
        pyjson::loads_unique(r#"{"a":0,"a":1,}"#)
            .unwrap_err()
            .to_string()
            .starts_with("Illegal trailing comma")
    );
    let data = pyjson::loads(r#"{"a":0,"a":1}"#).unwrap();
    assert_eq!(data.get("a"), Some(&Json::Int(1.into())));
}

#[test]
fn retained_evidence_passes_without_claiming_native_execution() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let before = std::fs::read(root.join("ozzit.xlsx")).unwrap();
    let messages = evidence::verify(&root, None, None).unwrap();
    assert!(messages[1].starts_with("Historical release evidence:"));
    assert_eq!(
        messages.last().unwrap(),
        "No native execution performed. Unprovided release or installed files are not verified."
    );
    assert_eq!(std::fs::read(root.join("ozzit.xlsx")).unwrap(), before);
}

#[test]
fn sha256_streaming_matches_standard_vectors_and_reports_directories() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("input");
    for (bytes, expected) in [
        (
            b"".as_slice(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc".as_slice(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
    ] {
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(evidence::sha256(&path).unwrap(), expected);
    }
    assert_eq!(
        evidence::sha256(temporary.path()).unwrap_err(),
        ozzit::common::read_bytes(temporary.path()).unwrap_err()
    );
}

/// Recreates one recorded tree under `root` and runs the observation on it.
fn observe(
    record: &Value,
    root: &Path,
    assets: &std::collections::HashMap<&str, Vec<u8>>,
) -> Result<Value, String> {
    match record["kind"].as_str().unwrap() {
        "verify" => {
            for (name, hash) in record["files"].as_object().unwrap() {
                assert!(
                    Path::new(name)
                        .components()
                        .all(|c| matches!(c, Component::Normal(_)))
                );
                let path = root.join(name);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, &assets[hash.as_str().unwrap()]).unwrap();
                assert_eq!(evidence::sha256(&path).unwrap(), hash.as_str().unwrap());
            }
            for name in record["directories"].as_array().unwrap() {
                std::fs::create_dir_all(root.join(name.as_str().unwrap())).unwrap();
            }
            let workbook = record["options"]["workbook"].as_str().map(|p| root.join(p));
            let destination = record["options"]["destination"]
                .as_str()
                .map(|p| root.join(p));
            evidence::verify(root, workbook.as_deref(), destination.as_deref()).map(|v| json!(v))
        }
        "grid" => {
            evidence::check_grid(&pyjson::loads(record["argument"].as_str().unwrap()).unwrap())
                .map(|()| Value::Null)
        }
        other => panic!("unknown observation {other}"),
    }
}

/// The observation's result with the temporary root replaced by {ROOT}.
fn actual_of(result: Result<Value, String>, root: &Path) -> Value {
    match result {
        Ok(value) => json!({"expected":value}),
        Err(error) => {
            let name = ozzit::common::display(root);
            let quoted = ozzit::py::repr_str(&name);
            json!({"error":error.replace(&quoted[1..quoted.len()-1],"{ROOT}").replace(&name,"{ROOT}")})
        }
    }
}

/// The recorded result, preferring a Linux-specific error on Linux.
fn expected_of(record: &Value) -> Value {
    let error = if cfg!(target_os = "linux") {
        record.get("linux_error").or_else(|| record.get("error"))
    } else {
        record.get("error")
    };
    if let Some(error) = error {
        json!({"error":error})
    } else {
        json!({"expected":record["expected"]})
    }
}

#[test]
fn python_evidence_and_grid_observations_replay_without_differences() {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/native-evidence-contracts.json.gz"),
    )
    .unwrap();
    let corpus: Value = serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap();
    let assets = corpus["assets"].as_object().unwrap();
    let records = corpus["records"].as_array().unwrap();
    assert!(records.len() > 1000);
    let assets = assets
        .iter()
        .map(|(hash, b64)| {
            (
                hash.as_str(),
                base64::engine::general_purpose::STANDARD
                    .decode(b64.as_str().unwrap())
                    .unwrap(),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let mut mismatches = Vec::new();
    for (i, record) in records.iter().enumerate() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let result = observe(record, root, &assets);
        let actual = actual_of(result, root);
        let expected = expected_of(record);
        if actual != expected {
            mismatches.push(format!(
                "observation {i}: expected {expected}, actual {actual}"
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} difference(s):\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
