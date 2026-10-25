//! Published function metadata and the pre-rename migration map.
use crate::{archive::*, common::*, compiler, py};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

fn counts(csv: &Csv) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for row in &csv.rows {
        *counts
            .entry(csv.get(row, "function").unwrap_or(""))
            .or_insert(0) += 1;
    }
    counts
}

fn index_mismatches(
    csv: &Csv,
    expected: &BTreeMap<String, [String; 3]>,
    index: &Path,
) -> Vec<String> {
    let mut failures = repeated(csv, index);
    let actual = csv
        .rows
        .iter()
        .map(|row| (csv.get(row, "function").unwrap_or(""), row))
        .collect::<BTreeMap<_, _>>();
    for name in expected.keys().filter(|n| !actual.contains_key(n.as_str())) {
        failures.push(format!(
            "{name} is in the workbook but missing from functions.csv"
        ));
    }
    for name in actual.keys().filter(|n| !expected.contains_key(**n)) {
        failures.push(format!(
            "{name} is in functions.csv but missing from the workbook"
        ));
    }
    for (name, wanted) in expected {
        let Some(row) = actual.get(name.as_str()) else {
            continue;
        };
        for (field, value) in ["module", "signature", "description"].iter().zip(wanted) {
            let cell = if csv.headers.iter().any(|h| h == field) {
                csv.get(row, field)
            } else {
                Some("")
            };
            if cell != Some(value.as_str()) {
                let authority = if *field == "module" {
                    "src/"
                } else {
                    "workbook"
                };
                failures.push(format!("{name}: {field} differs from {authority}"));
            }
        }
    }
    failures
}

/// "{name} appears {count} times" for each function listed more than once.
fn repeated(csv: &Csv, index: &Path) -> Vec<String> {
    counts(csv)
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(name, count)| format!("{name} appears {count} times in {}", basename(index)))
        .collect()
}

/// The released names in `baseline`, or in the bundled v1.2.6 list when none is given.
fn released_names(baseline: Option<&str>) -> Result<BTreeSet<String>> {
    let text = match baseline {
        Some(path) => read_text(Path::new(path))?,
        None => include_str!("../../released-names-v1.2.6.txt").to_owned(),
    };
    Ok(text
        .lines()
        .filter(|line| !py::strip(line).is_empty() && !line.starts_with('#'))
        .map(|line| py::strip(line).to_owned())
        .collect())
}

/// The module, signature and description each shipped function should list.
fn expected_rows(
    book: &str,
    modules: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, [String; 3]>> {
    let mut expected = BTreeMap::new();
    for (name, (attrs, body)) in compiler::shipped_names(book)? {
        let (signature, description) =
            compiler::index_fields(&attrs, &body, name.rsplit('.').next().unwrap_or(""))?;
        let module = modules.get(&name).cloned().unwrap_or_default();
        expected.insert(name, [module, signature, description]);
    }
    Ok(expected)
}

pub fn verify(args: &[String]) -> Output {
    if args.len() > 3 {
        return Output::stderr(
            "FAIL: usage: ozzit verify_index [workbook] [src dir] [functions.csv]",
        );
    }
    let workbook = Path::new(args.first().map_or("ozzit.xlsx", String::as_str));
    let src = Path::new(args.get(1).map_or("src", String::as_str));
    let index = Path::new(args.get(2).map_or("functions.csv", String::as_str));
    let inputs = || -> Result<_> { Ok((read_book(&read_parts(workbook)?)?, Csv::read(index)?)) };
    let (book, csv) = match inputs() {
        Ok(inputs) => inputs,
        Err(e) => return Output::failure(format!("FAIL: cannot read index inputs: {e}")),
    };
    let duplicates = csv.duplicates(&["function", "module", "signature", "description"]);
    if !duplicates.is_empty() {
        return Output::failure(format!(
            "FAIL: {}: duplicate CSV column(s): {}",
            basename(index),
            duplicates.join(", ")
        ));
    }
    let modules = match compiler::source_modules(src) {
        Ok(modules) => modules,
        Err(e) => return Output::failure(format!("FAIL: cannot read index inputs: {e}")),
    };
    let expected = match expected_rows(&book, &modules) {
        Ok(expected) => expected,
        Err(e) => return Output::failure(format!("FAIL: cannot read index inputs: {e}")),
    };
    let failures = index_mismatches(&csv, &expected, index);
    let bad = format!("FAIL: {} functions.csv problem(s)", failures.len());
    Output::report(
        failures,
        bad,
        format!(
            "OK: all {} functions.csv rows match the workbook and source modules",
            csv.rows.len()
        ),
    )
}

/// Why the previous-name check cannot run on these inputs, if it cannot.
fn refusal(csv: &Csv, released: &BTreeSet<String>, index: &Path, floor: usize) -> Option<Output> {
    let duplicates = csv.duplicates(&["function", "previous_name", "signature", "description"]);
    if !duplicates.is_empty() {
        return Some(Output::stderr(format!(
            "{} has duplicate CSV column(s): {}",
            display(index),
            duplicates.join(", ")
        )));
    }
    if csv.rows.is_empty() {
        return Some(Output::stderr(format!(
            "no rows read from {}",
            display(index)
        )));
    }
    for column in ["function", "previous_name"] {
        if !csv.headers.iter().any(|h| h == column) {
            return Some(Output::stderr(format!(
                "{} has no {column} column",
                display(index)
            )));
        }
    }
    if csv.rows.len() < floor || released.len() < floor {
        return Some(Output::stderr(format!(
            "only {} functions and {} baseline names read: the check is not reading them",
            csv.rows.len(),
            released.len()
        )));
    }
    None
}

/// Each released name a row claims to replace, mapped to that row's function.
fn claims(
    csv: &Csv,
    released: &BTreeSet<String>,
    baseline_name: &str,
    failures: &mut Vec<String>,
) -> std::result::Result<BTreeMap<String, String>, Output> {
    let mut claimed = BTreeMap::new();
    for row in &csv.rows {
        let was = py::strip(csv.get(row, "previous_name").unwrap_or(""));
        if was.is_empty() {
            continue;
        }
        let function = csv.get(row, "function").unwrap_or("None");
        if !released.contains(was) {
            failures.push(format!(
                "{function} claims to replace {was}, which {} never shipped",
                basename(Path::new(baseline_name))
            ));
        } else if let Some(previous) = claimed.get(was) {
            failures.push(format!(
                "{was} is claimed by both {previous} and {function}"
            ));
        } else {
            claimed.insert(was.to_owned(), function.to_owned());
            let Some((_, bare)) = was.rsplit_once('.') else {
                return Err(Output::stderr(
                    "previous baseline name has no module prefix",
                ));
            };
            let old_bare = bare.trim_end_matches('λ');
            let name = csv.get(row, "function").unwrap_or("");
            let Some((_, new_bare)) = name.split_once('.') else {
                failures.push(format!("{name} is not a qualified function name"));
                continue;
            };
            if !new_bare.starts_with(old_bare) {
                failures.push(format!("{function} is not a renaming of {was}: {new_bare} does not begin with {old_bare}"));
            }
        }
    }
    Ok(claimed)
}

fn published_fields(csv: &Csv, failures: &mut Vec<String>) {
    for row in &csv.rows {
        let function = csv.get(row, "function").unwrap_or("None");
        let sig = csv.get(row, "signature").unwrap_or("");
        let blurb = csv.get(row, "description").unwrap_or("");
        if count(sig, "(") != count(sig, ")") {
            failures.push(format!(
                "{function} publishes a signature with unbalanced brackets: {sig}"
            ));
        }
        for (field, text) in [("signature", sig), ("description", blurb)] {
            if text.contains("_x00") {
                failures.push(format!(
                    "{function}'s {field} carries a raw OOXML escape: {}",
                    prefix(text, 60)
                ));
            }
        }
        if blurb.starts_with('→') {
            failures.push(format!(
                "{function}'s description starts with the help table's column delimiter"
            ));
        }
        if sig.is_empty() || blurb.is_empty() {
            failures.push(format!(
                "{function} publishes an empty {}",
                if sig.is_empty() {
                    "signature"
                } else {
                    "description"
                }
            ));
        }
    }
}

pub fn previous_names(args: &[String]) -> Output {
    previous_names_with_floor(args, 100)
}

fn previous_names_with_floor(args: &[String], floor: usize) -> Output {
    if args.len() > 2 {
        return Output::stderr(
            "FAIL: usage: ozzit verify_previous_names [functions.csv] [baseline]",
        );
    }
    let index = Path::new(args.first().map_or("functions.csv", String::as_str));
    let baseline_name = args
        .get(1)
        .map_or("released-names-v1.2.6.txt", String::as_str);
    let released = match released_names(args.get(1).map(|_| baseline_name)) {
        Ok(released) => released,
        Err(e) => return Output::failure(format!("FAIL: cannot read {baseline_name}: {e}")),
    };
    let csv = match Csv::read(index) {
        Ok(csv) => csv,
        Err(e) => return Output::failure(format!("FAIL: cannot read {}: {e}", display(index))),
    };
    if let Some(refusal) = refusal(&csv, &released, index, floor) {
        return refusal;
    }
    let mut failures = repeated(&csv, index);
    let claimed = match claims(&csv, &released, baseline_name, &mut failures) {
        Ok(claimed) => claimed,
        Err(refusal) => return refusal,
    };
    published_fields(&csv, &mut failures);
    for was in released
        .iter()
        .filter(|was| !claimed.contains_key(was.as_str()))
    {
        failures.push(format!(
            "{was} shipped in the baseline and no function claims it"
        ));
    }
    let fresh = csv
        .rows
        .iter()
        .filter(|row| py::strip(csv.get(row, "previous_name").unwrap_or("")).is_empty())
        .count();
    let bad = format!("FAIL: {} problem(s) in {}", failures.len(), display(index));
    Output::report(
        failures,
        bad,
        format!(
            "OK: {} functions, {} of them replacing a name from {}, {fresh} new since it",
            csv.rows.len(),
            claimed.len(),
            basename(Path::new(baseline_name))
        ),
    )
}

#[cfg(test)]
#[path = "index_previous_names.rs"]
mod previous_names_tests;
