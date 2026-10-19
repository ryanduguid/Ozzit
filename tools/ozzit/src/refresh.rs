//! Recalculate the workbook in Excel, then sanitise what Excel adds on save.
//!
//! `tools/refresh_cache.ps1` opens the workbook in Excel, forces a full rebuild
//! and saves; the sanitiser then removes everything that save puts back. Run
//! `ozzit verify_cache` afterwards: that gate proves the saved answers.
use crate::{
    cache::{self, Dump},
    common::*,
    py, sanitise,
};
use std::time::Duration;

pub const SCRIPT: &str = "tools/refresh_cache.ps1";
/// A full rebuild of this workbook takes seconds; 10 minutes is a hang, not a slow run.
pub const REFRESH_TIMEOUT_SECONDS: u64 = 600;

/// The finding for `workbook` once the Excel refresh has finished.
pub fn outcome(workbook: &py::PyPath, refresh: Result<Dump>) -> Output {
    let fail = |out: Vec<String>| Output {
        code: 1,
        out,
        err: vec![],
    };
    let (code, stdout, stderr) = match refresh {
        Err(error) => return Output::failure(format!("FAIL: {error}")),
        Ok(Dump::NotFound) => {
            return Output::failure(
                "FAIL: powershell.exe was not found, so Excel could not be driven",
            );
        }
        Ok(Dump::TimedOut) => {
            return Output::failure(format!(
                "FAIL: the Excel refresh did not finish within {REFRESH_TIMEOUT_SECONDS}s; check for a modal dialog and close any Excel process before retrying"
            ));
        }
        Ok(Dump::Exited {
            code,
            stdout,
            stderr,
        }) => (code, stdout, stderr),
    };
    let mut out = Vec::new();
    let printed = py::strip(&stdout);
    if !printed.is_empty() {
        out.push(printed.to_string());
    }
    if code != 0 {
        out.push(format!(
            "FAIL: the Excel refresh did not complete (exit {code})"
        ));
        let detail = py::strip(&stderr);
        if !detail.is_empty() {
            out.push(py::prefix_chars(detail, 2000).into());
        }
        return fail(out);
    }
    match sanitise::sanitise(&workbook.fs()) {
        Ok(log) => out.extend(log),
        Err(e) => {
            out.push(format!("FAIL: cannot sanitise {workbook}: {e}"));
            return fail(out);
        }
    }
    out.push(format!(
        "OK: {workbook} holds the values its own formulas produce"
    ));
    Output::ok(out)
}

pub fn command(args: &[String]) -> Output {
    let workbook = py::PyPath::new(args.first().map_or("ozzit.xlsx", String::as_str));
    if !workbook.fs().exists() {
        return Output::failure(format!("FAIL: no such workbook: {workbook}"));
    }
    // Excel automation can stop on a modal dialog, and without a timeout the gate
    // would wait for it forever.
    let refresh = cache::run_dumper(
        "powershell.exe",
        &[
            "-NoProfile".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-File".into(),
            cache::repository_root(SCRIPT)
                .join(SCRIPT)
                .display()
                .to_string(),
            "-Path".into(),
            py::resolve(&workbook.fs()).display().to_string(),
        ],
        Duration::from_secs(REFRESH_TIMEOUT_SECONDS),
    );
    outcome(&workbook, refresh)
}
