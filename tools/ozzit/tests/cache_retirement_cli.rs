//! Physical cache command refusals that never reach a process launcher.
use std::process::Command;

fn refusal(dir: &std::path::Path, args: &[&str], expected: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        expected
    );
    assert_eq!(output.stderr, b"");
    assert_eq!(std::fs::read_dir(dir).unwrap().count(), 0);
}

#[test]
fn an_explicit_missing_workbook_is_a_physical_command_refusal() {
    let dir = tempfile::tempdir().unwrap();
    refusal(
        dir.path(),
        &["verify_cache", "missing.xlsx"],
        "FAIL: no such workbook: missing.xlsx\n",
    );
}

#[test]
fn the_default_missing_workbook_is_a_physical_command_refusal() {
    let dir = tempfile::tempdir().unwrap();
    refusal(
        dir.path(),
        &["verify_cache"],
        "FAIL: no such workbook: ozzit.xlsx\n",
    );
}
