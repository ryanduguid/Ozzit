//! Replay the active fuzz target without requiring nightly Rust or libFuzzer.
#[path = "../fuzz/harness.rs"]
mod harness;

use ozzit::selftest;
use std::path::Path;

#[test]
fn replay_parser_seeds() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus");
    let mut count = 0;
    for entry in std::fs::read_dir(corpus).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        harness::test_one_input(&std::fs::read(entry.path()).unwrap());
        count += 1;
    }
    assert!(count > 0, "the parser seed corpus is empty");
}

#[test]
fn replay_byte_and_line_boundaries() {
    harness::test_one_input(b"\xff\xfe\xc3\x28\r\n-1\x1c0\x851\n");
    harness::test_one_input("1\r2\r\n3\n4\u{85}5\u{2028}6\u{2029}7".as_bytes());
    harness::test_one_input(format!("0.{}1%", "0".repeat(400)).as_bytes());
}

#[test]
fn decimal_sign_does_not_depend_on_float_range() {
    for (text, positive) in [("-1", false), ("-0", false), ("0", false), ("1", true)] {
        let (value, tolerance) = selftest::number(text).unwrap();
        assert_eq!(value.is_positive(), positive, "{text}");
        assert!(tolerance.is_positive());
    }
    let text = format!("0.{}1%", "0".repeat(400));
    let (value, tolerance) = selftest::number(&text).unwrap();
    assert!(value.is_positive());
    assert!(tolerance.is_positive());
    // These exact positive decimals underflow when displayed as f64.
    assert_eq!(value.shown(), "0.0");
    assert_eq!(tolerance.shown(), "0.0");
}
