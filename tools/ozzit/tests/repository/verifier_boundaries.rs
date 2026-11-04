// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! The Excel value dump opens its output only inside its Excel cleanup, and every
//! cell it writes comes through the COM message filter and one checked range.
//!
//! `ozzit verify_cache` failed 1 of 3 runs on an unchanged workbook on 1 November
//! 2026 with an invalid cell position. A rejected property read comes back `$null`
//! in PowerShell, so a null row or column would write a position of 0, and a null
//! `Value2` would drop a sheet. The behaviour of the filter and `Read-Block` needs
//! Windows PowerShell and is tested in
//! `tools/tests/test_excel_eval_formulas.py`.
use super::{index, read};
use ozzit::common::count;

#[test]
fn value_dump_opens_its_output_inside_the_excel_cleanup() {
    let text = read("tools/dump_values.ps1");
    // a bad -Out path thrown before the try would skip Quit() and leave Excel running
    let start = index(&text, "New-Object -ComObject Excel.Application", 0);
    let cleanup = index(&text, "try {", start);
    assert!(cleanup < index(&text, "[System.IO.StreamWriter]::new(", 0));
}

/// The message filter's C# source, between `Add-Type` and the closing here-string.
fn filter(text: &str) -> &str {
    let start = index(text, "Add-Type -TypeDefinition @'\n", 0);
    &text[start..index(text, "\n'@", start)]
}

#[test]
fn value_dump_filters_excel_calls_and_reads_each_sheet_through_one_range() {
    let text = read("tools/dump_values.ps1");
    assert_eq!(
        filter(&text),
        filter(&read("tools/excel_eval_formulas.ps1"))
    );
    let excel = index(&text, "New-Object -ComObject Excel.Application", 0);
    assert!(index(&text, "[OzzitMessageFilter]::Register()", 0) < excel);
    let last_finally = text.rfind("finally {").expect("a finally block");
    assert!(last_finally < index(&text, "[OzzitMessageFilter]::Revoke()", 0));
    for read in ["$block = Read-Block $ws", "$ws.UsedRange", ".Value2"] {
        assert_eq!(count(&text, read), 1, "{read}");
    }
}
