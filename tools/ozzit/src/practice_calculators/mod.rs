// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! Native Rust builder for Ozzit's standalone practice-calculators workbook.
mod model;
mod ooxml;
mod sheets_early;
mod sheets_late;
mod sheets_mid;

use crate::archive::{Parts, deterministic_bytes};
use crate::common::{Result, io_error, replace_atomically};
use model::{CellStyle, Sheet, WHITE};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub const SHEET_NAMES: [&str; 22] = [
    "Read me",
    "Allocation",
    "Contractor",
    "Vendor finance",
    "Mixed loan",
    "Rental",
    "Packaging",
    "Amortisation",
    "Timeline",
    "Deed register",
    "Carry forward",
    "Loan offers",
    "Break even",
    "Working capital",
    "Construction WIP",
    "KPI awards",
    "Asset disposal",
    "Exit pay",
    "Tax cash",
    "Investment",
    "Action register",
    "Benefit totals",
];

fn build_sheets() -> Vec<Sheet> {
    let mut sheets = vec![
        sheets_early::read_me(&SHEET_NAMES),
        sheets_early::allocation(),
        sheets_early::contractor(),
        sheets_early::vendor_finance(),
        sheets_early::mixed_loan(),
        sheets_early::rental(),
        sheets_mid::packaging(),
        sheets_mid::amortisation(),
        sheets_mid::timeline(),
        sheets_mid::deed_register(),
        sheets_mid::carry_forward(),
        sheets_mid::loan_offers(),
        sheets_mid::break_even(),
        sheets_mid::working_capital(),
        sheets_late::construction_wip(),
        sheets_late::kpi_awards(),
        sheets_late::asset_disposal(),
        sheets_late::exit_pay(),
        sheets_late::tax_cash(),
        sheets_late::investment(),
        sheets_late::action_register(),
        sheets_late::benefit_totals(),
    ];
    add_navigation(&mut sheets);
    sheets
}

fn add_navigation(sheets: &mut [Sheet]) {
    for (i, name) in SHEET_NAMES.iter().enumerate().skip(1) {
        let row = 13 + i as u16;
        sheets[0].add_hyperlink(&format!("B{row}"), &format!("'{name}'!B2"), name);
        sheets[0].cell_mut((row, 2)).style = CellStyle::hyperlink(Some(WHITE), Some("center"));
        sheets[i].cell_text("B1", "Contents");
        sheets[i].cell_mut((1, 2)).style = CellStyle::hyperlink(None, None);
        sheets[i].add_hyperlink("B1", "'Read me'!B2", "Contents");
    }
}

pub fn build_parts() -> Result<Parts> {
    let sheets = build_sheets();
    ooxml::serialize_workbook(&sheets)
}

pub fn build_bytes() -> Result<Vec<u8>> {
    deterministic_bytes(&build_parts()?)
}

pub fn write_workbook(path: &Path) -> Result<usize> {
    let bytes = build_bytes()?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| io_error(&e, parent))?;
    }
    let len = bytes.len();
    replace_atomically(path, &bytes)?;
    Ok(len)
}

pub fn run(args: &[OsString]) -> std::result::Result<String, (u8, String)> {
    const USAGE: &str = "usage: build_practice_calculators [--out <path> | <path>]";
    let target: PathBuf = match args {
        [] => "templates/practice-calculators.xlsx".into(),
        [flag] if flag == "--help" || flag == "-h" => return Ok(USAGE.to_owned()),
        [flag, path] if flag == "--out" || flag == "-o" => path.into(),
        [path] if !path.as_encoded_bytes().starts_with(b"-") => path.into(),
        _ => return Err((2, USAGE.to_owned())),
    };
    let len = write_workbook(&target).map_err(|e| (1, format!("FAIL: {e}")))?;
    Ok(format!("Saved {} ({len} bytes)", target.display()))
}

pub fn cli(args: &[OsString]) -> ExitCode {
    match run(args) {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err((code, msg)) => {
            eprintln!("{msg}");
            ExitCode::from(code)
        }
    }
}
