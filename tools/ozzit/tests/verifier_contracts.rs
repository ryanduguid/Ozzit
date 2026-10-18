//! Replay the original direct workbook/report and help-cache fixture paths.
use base64::Engine;
use ozzit::{afe, archive::Parts, spills, workbook};
use serde_json::{Value, json};

fn parts(record: &Value) -> Parts {
    record["parts"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, bytes)| {
            (
                name.clone(),
                base64::engine::general_purpose::STANDARD
                    .decode(bytes.as_str().unwrap())
                    .unwrap(),
            )
        })
        .collect()
}

#[test]
fn strict_base64_matches_the_cpython_reference_padding_states() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/base64-contracts.json.gz"),
    )
    .unwrap();
    let data: Value = serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap();
    let records = data["records"].as_array().unwrap();
    assert_eq!(records.len(), 5461);
    let mut differences = Vec::new();
    for record in records {
        let input = record["input"].as_str().unwrap();
        let (actual, expected) = match afe::base64_decode_strict(input.as_bytes()) {
            Ok(bytes) => (json!(bytes), &record["bytes"]),
            Err(error) => (json!(error), &record["error"]),
        };
        if actual != *expected {
            differences.push(format!("{input:?}: {actual} != {expected}"));
        }
    }
    assert!(
        differences.is_empty(),
        "{} strict decoder differences: {:?}",
        differences.len(),
        &differences[..differences.len().min(25)]
    );
}
fn parser_reason(line: &str) -> &str {
    if line.starts_with("malformed XML in ") {
        line.split_once(": ").unwrap().0
    } else {
        line
    }
}

#[test]
fn workbook_and_help_cache_contracts_match_the_python_fixtures() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/verifier-contracts.json.gz"),
    )
    .unwrap();
    let data: Value = serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap();
    let records = data["records"].as_array().unwrap();
    assert!(records.len() >= 60, "the direct paths must not be omitted");
    for (i, record) in records.iter().enumerate() {
        let result = match record["kind"].as_str().unwrap() {
            "workbook" => {
                let names = record["names"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>();
                let actual = workbook::check(&parts(record), &names);
                let mut expected = record["failures"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| parser_reason(v.as_str().unwrap()))
                    .collect::<Vec<_>>();
                let mut found = actual
                    .failures
                    .iter()
                    .map(|s| parser_reason(s))
                    .collect::<Vec<_>>();
                // Findings are identical; undefined-reference tokens are sorted
                // deterministically rather than using Python's hash-set order.
                expected.sort_unstable();
                found.sort_unstable();
                assert_eq!(found, expected, "workbook observation {i}");
                if found.is_empty() {
                    assert_eq!(json!(actual.functions), record["functions"]);
                }
                continue;
            }
            "spills" => spills::check(&parts(record)).map(|(stale, count)| json!([stale, count])),
            "table" => spills::help_table(
                record["stored"].as_str().unwrap(),
                record["name"].as_str().unwrap(),
            )
            .map(|rows| json!(rows)),
            other => panic!("unmapped contract {other}"),
        };
        match result {
            Ok(actual) => assert_eq!(actual, record["expected"], "observation {i}"),
            Err(error) => assert_eq!(json!(error), record["error"], "observation {i}"),
        }
    }
}
