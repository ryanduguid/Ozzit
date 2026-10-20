pub mod afe;
pub mod archive;
pub mod cache;
pub mod common;
pub mod compiler;
pub mod cover;
pub mod formula;
pub mod help;
pub mod index;
pub mod module;
pub mod native_evidence;
mod output;
pub mod polish;
pub mod postbuild;
pub mod py;
pub mod pyjson;
mod pyunicode;
pub mod reference;
pub mod refresh;
pub mod release;
pub mod sanitise;
pub mod selftest;
pub mod sources;
pub mod spills;
pub mod workbook;

use common::Output;
type Handler = fn(&[String]) -> Output;
const COMMANDS: &[(&str, Handler)] = &[
    ("compile_sources", compiler::command),
    ("verify_sources", sources::verify),
    ("verify_signatures", help::verify),
    ("function_reference", reference::command),
    ("verify_index", index::verify),
    ("verify_previous_names", index::previous_names),
    ("verify_afe", afe::verify),
    ("sync_afe_store", afe::sync_command),
    ("build_module", module::command),
    ("verify_cover", cover::verify),
    ("verify_help_spills", spills::verify),
    ("verify_workbook", workbook::verify),
    ("verify_native_evidence", native_evidence::command),
    ("verify_cache", cache::verify),
    ("sanitise_workbook", sanitise::command),
    ("refresh_cache", refresh::command),
    ("polish_workbook", polish::command),
    ("generate_selftest_examples", selftest::command),
    ("prepare_release_bundle", release::command),
    ("cover_label", postbuild::cover_label::command),
    ("sheet_names", postbuild::sheet_names::command),
    ("help_links", postbuild::help_links::command),
    ("fy27_help_text", postbuild::fy27_help_text::command),
    ("gst_help_text", postbuild::gst_help_text::command),
    (
        "strip_revision_history",
        postbuild::strip_revision_history::command,
    ),
    ("remove_residue", postbuild::remove_residue::command),
    ("workbook_palette", postbuild::workbook_palette::command),
    ("help_corrections", postbuild::help_corrections::command),
    ("aasb16_leases", postbuild::aasb16_leases::command),
    (
        "working_capital_days",
        postbuild::function_pass::working_capital_days,
    ),
    (
        "rate_date_helpers",
        postbuild::function_pass::rate_date_helpers,
    ),
    (
        "ato_benchmarks_rate_lookup",
        postbuild::function_pass::ato_benchmarks_rate_lookup,
    ),
    (
        "variance_fy_periods",
        postbuild::function_pass::variance_fy_periods,
    ),
];
pub fn dispatch(args: &[String]) -> Output {
    let command = args.first().map(String::as_str);
    if command == Some("--version") {
        return Output::ok(vec![format!("ozzit {}", env!("CARGO_PKG_VERSION"))]);
    }
    if let Some((_, run)) = COMMANDS.iter().find(|(name, _)| Some(*name) == command) {
        return run(&args[1..]);
    }
    let usage = format!(
        "usage: ozzit <command> [arguments]\ncommands: {}",
        COMMANDS
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ")
    );
    if command == Some("--help") {
        Output::ok(vec![usage])
    } else {
        Output {
            code: 2,
            out: vec![],
            err: vec![usage],
        }
    }
}
pub fn run() -> std::process::ExitCode {
    use std::io::Write;
    // The arguments only select a command and its operands; the program
    // name (argument 0) is skipped and never trusted.
    // nosemgrep: rust.lang.security.args.args
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let encoding = if matches!(
        args.first().map(String::as_str),
        Some("verify_afe" | "sync_afe_store")
    ) {
        match output::Encoding::from_override(
            &std::env::var("PYTHONIOENCODING").unwrap_or_default(),
        ) {
            Ok(encoding) => encoding,
            Err(error) => {
                eprintln!("{error}");
                return 2.into();
            }
        }
    } else {
        output::Encoding::Utf8
    };
    let result = dispatch(&args);
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    let emit = |lines: Vec<String>, sink: &mut dyn Write| {
        for line in lines {
            let line = line.replace('\n', newline);
            encoding.write(&line, sink)?;
            encoding.write(newline, sink)?;
        }
        std::io::Result::Ok(())
    };
    if emit(result.out, &mut std::io::stdout().lock())
        .and_then(|()| emit(result.err, &mut std::io::stderr().lock()))
        .is_err()
    {
        return std::process::ExitCode::FAILURE;
    }
    result.code.into()
}
