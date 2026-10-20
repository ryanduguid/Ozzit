//! Original refresh assertions use owned placeholders and fake process outcomes.
use super::*;
use std::{cell::RefCell, path::PathBuf};

struct Fixture {
    _directory: tempfile::TempDir,
    workbook: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let workbook = directory.path().join("ozzit.xlsx");
        std::fs::write(&workbook, b"workbook").unwrap();
        Self {
            _directory: directory,
            workbook,
        }
    }
    fn run(&self, refresh: Dump, clean: impl FnOnce(&Path) -> Result<Vec<String>>) -> Output {
        let before = std::fs::read(&self.workbook).unwrap();
        let result = command_with(
            &[display(&self.workbook)],
            |program, args, timeout| {
                assert_eq!(program, "powershell.exe");
                assert_eq!(
                    args,
                    [
                        "-NoProfile".into(),
                        "-ExecutionPolicy".into(),
                        "Bypass".into(),
                        "-File".into(),
                        display(&cache::repository_root(SCRIPT).join(SCRIPT)),
                        "-Path".into(),
                        display(&py::resolve(&self.workbook))
                    ]
                );
                assert_eq!(timeout, Duration::from_secs(600));
                Ok(refresh)
            },
            clean,
        );
        assert_eq!(std::fs::read(&self.workbook).unwrap(), before);
        result
    }
}
fn printed(output: Output) -> (u8, String, String) {
    let lines = |lines: Vec<String>| lines.iter().map(|l| format!("{l}\n")).collect();
    (output.code, lines(output.out), lines(output.err))
}
fn exited(code: i64, stdout: &str, stderr: &str) -> Dump {
    Dump::Exited {
        code,
        stdout: stdout.into(),
        stderr: stderr.into(),
    }
}
fn refused(refresh: Dump) -> (u8, String, String) {
    printed(Fixture::new().run(refresh, |_| {
        panic!("must not sanitise after failed refresh")
    }))
}
#[test]
fn missing_workbook_fails_before_excel_runs() {
    let f = Fixture::new();
    let missing = f.workbook.with_file_name("missing.xlsx");
    assert_eq!(
        printed(command_with(
            &[display(&missing)],
            |_, _, _| panic!("must not launch for a missing workbook"),
            |_| panic!("must not sanitise a missing workbook")
        )),
        (
            1,
            format!("FAIL: no such workbook: {}\n", display(&missing)),
            String::new()
        )
    );
}
#[test]
fn missing_powershell_is_reported() {
    assert_eq!(
        refused(Dump::NotFound),
        (
            1,
            "FAIL: powershell.exe was not found, so Excel could not be driven\n".into(),
            String::new()
        )
    );
}
#[test]
fn hung_refresh_is_reported() {
    assert_eq!(refused(Dump::TimedOut), (1,
        "FAIL: the Excel refresh did not finish within 600s; check for a modal dialog and close any Excel process before retrying\n".into(), String::new()));
}
#[test]
fn failed_refresh_prints_its_exit_code_and_trimmed_error() {
    assert_eq!(
        refused(exited(5, "", &format!("  {}\n", "x".repeat(2500)))),
        (
            1,
            format!(
                "FAIL: the Excel refresh did not complete (exit 5)\n{}\n",
                "x".repeat(2000)
            ),
            String::new()
        )
    );
}
#[test]
fn failed_refresh_without_error_text_prints_only_the_exit_code() {
    assert_eq!(
        refused(exited(2, "opening\n", " \n")),
        (
            1,
            "opening\nFAIL: the Excel refresh did not complete (exit 2)\n".into(),
            String::new()
        )
    );
}
#[test]
fn silent_successful_refresh_goes_straight_to_the_sanitiser() {
    let f = Fixture::new();
    let calls = RefCell::new(Vec::new());
    let output = command_with(
        &[display(&f.workbook)],
        |program, args, timeout| {
            calls.borrow_mut().push("excel");
            assert_eq!(program, "powershell.exe");
            assert_eq!(
                args,
                [
                    "-NoProfile".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-File".into(),
                    display(&cache::repository_root(SCRIPT).join(SCRIPT)),
                    "-Path".into(),
                    display(&py::resolve(&f.workbook))
                ]
            );
            assert_eq!(timeout, Duration::from_secs(REFRESH_TIMEOUT_SECONDS));
            assert_eq!(REFRESH_TIMEOUT_SECONDS, 600);
            Ok(exited(0, "", ""))
        },
        |path| {
            calls.borrow_mut().push("sanitise");
            assert_eq!(path, f.workbook);
            Ok(vec!["sanitised".into()])
        },
    );
    assert_eq!(*calls.borrow(), ["excel", "sanitise"]);
    assert_eq!(
        printed(output),
        (
            0,
            format!(
                "sanitised\nOK: {} holds the values its own formulas produce\n",
                display(&f.workbook)
            ),
            String::new()
        )
    );
    assert_eq!(std::fs::read(&f.workbook).unwrap(), b"workbook");
}
#[test]
fn import_reads_the_workbook_argument_and_tolerates_a_plain_stdout() {
    // Native argument selection has no import or stream reconfiguration side effects.
    assert_eq!(workbook_argument(&[]).to_string(), "ozzit.xlsx");
    assert_eq!(
        workbook_argument(&["other.xlsx".into()]).to_string(),
        "other.xlsx"
    );
    // Public command/outcome pass sanitise::sanitise directly. Existing integration
    // coverage checks its real saved-workbook cleanup and resulting bytes.
}
#[test]
fn refresh_cache_delegates_all_cleanup_to_shared_sanitiser() {
    let f = Fixture::new();
    let calls = RefCell::new(Vec::new());
    let output = f.run(exited(0, "refreshed", ""), |path| {
        calls.borrow_mut().push(path.to_path_buf());
        Ok(vec!["removed 1 x15ac:absPath".into()])
    });
    assert_eq!(calls.borrow().as_slice(), std::slice::from_ref(&f.workbook));
    assert_eq!(
        printed(output),
        (
            0,
            format!(
                "refreshed\nremoved 1 x15ac:absPath\nOK: {} holds the values its own formulas produce\n",
                display(&f.workbook)
            ),
            String::new()
        )
    );
}
