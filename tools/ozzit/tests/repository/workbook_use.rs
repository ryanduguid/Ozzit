// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! `docs/workbook-use.md`, held to the calls between definitions in `src/`.
//!
//! The page tells a reader which definitions need others defined first when
//! pasted into Name Manager. A callee left off that list gives the reader a
//! definition that fails with #NAME? in their workbook.
use super::{findall, flat, modules, read};
use ozzit::{common, formula};
use std::collections::{BTreeMap, BTreeSet};

type Calls = BTreeMap<String, BTreeSet<String>>;

const NAME: &str = r"[A-Za-z_][A-Za-z0-9_.?λ]*";

/// Each definition that names another, from its code outside literals.
fn calls_in_source() -> Calls {
    let definitions = modules()
        .iter()
        .flat_map(|(_, text)| formula::statements(text))
        .flat_map(|statement| common::captures(formula::NAME, &statement))
        .map(|m| (m[1].clone(), m[2].clone()))
        .collect::<BTreeMap<_, _>>();
    let mut calls = Calls::new();
    for (name, body) in &definitions {
        let code = formula::split_literals(body)
            .into_iter()
            .filter(|(literal, _)| !literal)
            .map(|(_, code)| code)
            .collect::<String>();
        let callees = findall(NAME, &code)
            .into_iter()
            .filter(|word| word != name && definitions.contains_key(word))
            .collect::<BTreeSet<_>>();
        if !callees.is_empty() {
            calls.insert(name.clone(), callees);
        }
    }
    calls
}

/// The clauses after the colon: "`A` calls `B`, `C`; `D` and `E` call `F`; ...".
fn calls_on_the_page(page: &str) -> Calls {
    let list = page
        .split_once("under the names the definition uses: ")
        .expect("the list of callees")
        .1;
    let list = list.split_once('.').expect("the end of the list").0;
    let mut calls = Calls::new();
    for clause in list.split(';') {
        let (callers, callees) = clause
            .split_once(" calls ")
            .or_else(|| clause.split_once(" call "))
            .unwrap_or_else(|| panic!("no call in {clause:?}"));
        let callees = findall("`[^`]+`", callees);
        for caller in findall("`[^`]+`", callers) {
            calls.insert(
                caller.trim_matches('`').to_owned(),
                callees
                    .iter()
                    .map(|c| c.trim_matches('`').to_owned())
                    .collect(),
            );
        }
    }
    calls
}

#[test]
fn the_page_lists_every_definition_that_calls_another() {
    let page = flat(&read("docs/workbook-use.md"));
    let calls = calls_in_source();
    assert_eq!(calls_on_the_page(&page), calls);
    let claim = format!("stands alone except for {} that call", calls.len());
    assert!(page.contains(&claim), "the page no longer says {claim:?}");
}

#[test]
fn a_clause_with_two_callers_and_a_shorter_list_are_both_read() {
    let page = "under the names the definition uses: `A` calls `B`, `C` and `D`; \
                `E` and `F` call `G`; and `H` calls `I`. To change";
    let calls = calls_on_the_page(page);
    assert_eq!(calls.len(), 4);
    assert_eq!(calls["A"].len(), 3);
    assert_eq!(calls["F"], BTreeSet::from(["G".to_owned()]));
    assert_eq!(calls["H"], BTreeSet::from(["I".to_owned()]));
}
