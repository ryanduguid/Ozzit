// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: FSL-1.1-ALv2

//! The Excel value dump opens its output only inside its Excel cleanup.
use super::{index, read};

#[test]
fn value_dump_opens_its_output_inside_the_excel_cleanup() {
    let text = read("tools/dump_values.ps1");
    // a bad -Out path thrown before the try would skip Quit() and leave Excel running
    let start = index(&text, "New-Object -ComObject Excel.Application", 0);
    let cleanup = index(&text, "try {", start);
    assert!(cleanup < index(&text, "[System.IO.StreamWriter]::new(", 0));
}
