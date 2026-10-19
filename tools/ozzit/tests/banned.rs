//! The workbook gate's banned-token scan matches Python's `BANNED.finditer`.
use ozzit::workbook::banned_tokens;
use serde_json::Value;
use std::path::Path;

#[test]
fn banned_tokens_match_python_finditer() {
    let bytes =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/banned-cases.json"))
            .unwrap();
    let cases: Value = serde_json::from_slice(&bytes).unwrap();
    for case in cases.as_array().unwrap() {
        let text = case[0].as_str().unwrap();
        let expected = case[1]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(banned_tokens(text), expected, "{text:?}");
    }
}

#[test]
fn a_large_store_is_scanned_without_exhausting_the_engine() {
    // The lookaround form hit the backtrack limit on the 1 MB AFE store part.
    let text = format!("{} BXL {}", "x".repeat(5_000_000), "y".repeat(5_000_000));
    assert_eq!(banned_tokens(&text), ["BXL"]);
}
