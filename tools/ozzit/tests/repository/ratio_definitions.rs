// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! `docs/ratio-definitions.md`, held to the ratio formulas in `src/Ratios.txt`.
//!
//! The page states each ratio function's arithmetic. This reads the `Result`
//! line of every function in the source module and fails when the page names a
//! function the module does not define, leaves one out, or shows arithmetic the
//! source no longer performs. Whitespace is ignored on both sides; nothing else
//! is normalised, so a changed operand or operator fails.
use super::{flat, read, search};
use indexmap::IndexMap;
use ozzit::common;
use std::collections::BTreeMap;

const DOCUMENT: &str = "docs/ratio-definitions.md";
const ROW: &str = r"(?m)^\| `oz\.(\w+λ)` \| `([^`]+)` \|";
const BLOCK: &str = r"(?m)^/\*\s+FUNCTION NAME:\s*(\S+)";
const ASSIGNMENT: &str = r"(?m)^(\S+λ)\s*=";
const RESULT: &str = r"(?m)^\s*Result,\s*(.+?),\s*$";

/// `"".join(text.split())`.
fn compact(text: &str) -> String {
    ozzit::py::split_whitespace(text).concat()
}

/// Each function's `Result` formula, or the label that heads another definition.
fn source_formulas(text: &str) -> Result<IndexMap<String, String>, String> {
    let starts: Vec<(usize, String)> = common::re(BLOCK)
        .captures_iter(text)
        .map(|m| {
            let m = m.expect("regular expression execution");
            (m.get(0).unwrap().start(), m[1].to_owned())
        })
        .collect();
    let mut formulas = IndexMap::new();
    for (i, (start, name)) in starts.iter().enumerate() {
        let end = starts.get(i + 1).map_or(text.len(), |(next, _)| *next);
        let block = &text[*start..end];
        // The comment is only a label: the name the module defines is the one
        // its formula is assigned to, so a renamed function cannot hide behind it.
        if search(ASSIGNMENT, block).is_none_or(|assigned| assigned[1] != *name) {
            return Err(format!(
                "the FUNCTION NAME comment {name} does not head a definition of that name"
            ));
        }
        let Some((_, procedure)) = block.split_once("//  Procedure") else {
            continue;
        };
        let procedure = procedure
            .split_once("//  Return Result")
            .map_or(procedure, |p| p.0);
        if let Some(found) = search(RESULT, procedure) {
            formulas.insert(name.clone(), compact(&found[1]));
        }
    }
    Ok(formulas)
}

#[test]
fn the_page_lists_every_ratio_function_with_its_source_arithmetic() {
    let rows = common::captures(ROW, &read(DOCUMENT));
    let source = source_formulas(&read("src/Ratios.txt")).unwrap_or_else(|e| panic!("{e}"));
    // A floor, so a parser that finds nothing cannot pass by comparing two empty sets.
    assert_eq!(source.len(), 47);
    // Counted before the rows become a mapping, so a duplicated row cannot hide.
    assert_eq!(rows.len(), source.len());
    let documented: BTreeMap<&str, String> = rows
        .iter()
        .map(|row| (row[1].as_str(), compact(&row[2])))
        .collect();
    let mut defined: Vec<&str> = source.keys().map(String::as_str).collect();
    defined.sort_unstable();
    assert_eq!(documented.keys().copied().collect::<Vec<_>>(), defined);
    for (name, formula) in &source {
        assert_eq!(&documented[name.as_str()], formula, "function={name}");
    }
}

#[test]
fn the_page_states_the_count_it_documents() {
    let text = read(DOCUMENT);
    assert!(flat(&text).contains("Each of the 47 ratio functions"));
    assert!(text.contains("### The 47 functions"));
}

// The helper on input that takes its other paths, from test_test_helper_paths.py.

#[test]
fn a_procedure_without_a_result_line_is_left_out() {
    let text = concat!(
        "/* FUNCTION NAME: Aλ */\nAλ = LAMBDA(x,\n//  Procedure\n",
        "    LET(\n    Result,  x + 1,\n//  Return Result\n",
        "/* FUNCTION NAME: Bλ */\nBλ = LAMBDA(x,\n//  Procedure\n    x\n//  Return Result\n",
    );
    let formulas = source_formulas(text).unwrap();
    assert_eq!(
        formulas.into_iter().collect::<Vec<_>>(),
        [("Aλ".to_owned(), "x+1".to_owned())]
    );
}

#[test]
fn a_comment_heading_another_definition_fails() {
    assert_eq!(
        source_formulas("/* FUNCTION NAME: Cλ */\nDλ = LAMBDA(x, x)\n").unwrap_err(),
        "the FUNCTION NAME comment Cλ does not head a definition of that name"
    );
}
