// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: FSL-1.1-ALv2

//! The tracked Debt module's debt functions no longer call themselves by name.
use super::{piece, read};
use ozzit::common;

#[test]
fn tracked_debt_module_no_longer_recurses_by_name() {
    let text = read("src/Debt.txt");
    for name in [
        "AmortiseBλ",
        "DebtSculptFixedλ",
        "DebtSculptVariableλ",
        "DebtSculptVariableLRVλ",
        "InterestLRVλ",
    ] {
        let body = piece(
            text.split_once(&format!("\n{name} = LAMBDA("))
                .unwrap_or_else(|| panic!("{name} is not defined"))
                .1,
            "\n);",
            0,
        );
        // help text names the function; only code outside string literals can call it
        let code = common::sub(r#""(?:[^"]|"")*""#, body, r#""""#);
        assert!(
            !code.contains(&format!("{name}(")),
            "{name} still calls itself"
        );
    }
}
