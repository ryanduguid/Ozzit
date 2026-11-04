//! The formula helper's COM message filter covers every Excel call.
//!
//! A rejected method call throws `RPC_E_CALL_REJECTED`, but a rejected property
//! read comes back `$null` in PowerShell. On 1 November 2026 that made
//! `HasFormula` read null for a formula cell, so 3 of 4 runs on an unchanged
//! workbook failed. The helper registers a filter that retries a rejected call
//! and reads `HasFormula` and `HasSpill` through `Read-Flag`. The filter's own
//! behaviour needs Windows PowerShell and is tested in
//! `tools/tests/test_excel_eval_formulas.py`.
use super::{index, read};
use ozzit::common::count;

#[test]
fn the_filter_covers_every_excel_call_and_is_revoked_last() {
    let text = read("tools/excel_eval_formulas.ps1");
    // The main try opens with the filter, so the finally that quits Excel also revokes it.
    let main = index(&text, "try {\n    [OzzitMessageFilter]::Register()\n", 0);
    assert!(main < index(&text, "New-Object -ComObject Excel.Application", 0));
    let last_finally = text.rfind("finally {").expect("a finally block");
    assert!(last_finally < index(&text, "[OzzitMessageFilter]::Revoke()", 0));
    for flag in ["HasFormula", "HasSpill"] {
        // Every read of the flag goes through Read-Flag.
        let through = count(&text, &format!("Read-Flag {{ $cell.{flag} }}"));
        assert!(through > 0, "{flag}: no read goes through Read-Flag");
        assert_eq!(count(&text, &format!("$cell.{flag}")), through, "{flag}");
    }
}
