//! Independent signature, parameter-table and worked-example checks.
use crate::{common::*, formula, py};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

const CALL: &str = r"(?<![A-Za-z0-9_.])(?:oz\.)?([A-Za-z_][A-Za-z0-9_]*λ[A-Za-z0-9_]*)\s*\(";
const LABEL: &str = r"^(?:oz\.)?([A-Za-z_][A-Za-z0-9_]*λ[A-Za-z0-9_]*)$";
const SIGNATURE: &str = r"(?s)^([A-Za-z0-9_.]*λ[A-Za-z0-9_]*)\s*\((.*)\)\s*$";

pub fn literals(text: &str) -> String {
    formula::split_literals(text)
        .into_iter()
        .filter(|(literal, _)| *literal)
        .map(|(_, chunk)| {
            let inner = &chunk[1..];
            inner
                .strip_suffix('"')
                .unwrap_or(inner)
                .replace("\"\"", "\"")
        })
        .collect()
}

pub fn rows(body: &str) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = Vec::new();
    for raw in literals(body).split('¶') {
        let Some((label, text)) = raw.split_once('→') else {
            continue;
        };
        if py::strip(label).is_empty() && !rows.is_empty() {
            rows.last_mut()
                .expect("preceding help row")
                .1
                .push_str(text);
        } else {
            rows.push((py::strip(label).into(), text.into()));
        }
    }
    rows
}

fn names(text: &str) -> Vec<String> {
    text.split(',')
        .filter(|p| !py::strip(p).is_empty())
        .map(|p| py::strip(py::strip(p).trim_matches(['[', ']'])).to_owned())
        .collect()
}

pub fn declared(body: &str) -> Vec<String> {
    let Some(head) = body.find("LAMBDA(") else {
        return vec![];
    };
    let head = sub(r"//[^\n]*", &body[head + 7..], "");
    let mut args = formula::arguments(&head, 0);
    args.pop();
    names(&args.join(","))
}

pub fn worked_examples(body: &str) -> Vec<String> {
    let mut calls = Vec::new();
    let mut started = false;
    for (label, text) in rows(body) {
        if label.starts_with("EXAMPLE") {
            started = true;
        } else if started && label.ends_with(':') {
            break;
        } else if !started {
            continue;
        }
        calls.extend(captures(CALL, &text).into_iter().map(|m| m[1].clone()));
    }
    calls
}

pub fn parameter_table(body: &str) -> Option<Vec<String>> {
    let mut labels = Vec::new();
    let mut started = false;
    for (label, _) in rows(body) {
        if !started {
            started = label.starts_with("PARAMETERS");
        } else if label.ends_with(':') {
            break;
        } else if !label.ends_with('!') {
            labels.push(label);
        }
    }
    started.then_some(labels)
}

fn list_or_none(mut names: Vec<String>) -> String {
    names.sort();
    names.dedup();
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}

pub fn verify(args: &[String]) -> Output {
    if args.len() > 1 {
        return Output::stderr("FAIL: usage: ozzit verify_signatures [src dir]");
    }
    let src = args.first().map_or("src", String::as_str);
    let paths = match txt_files(Path::new(src)) {
        Ok(paths) => paths,
        Err(e) => return Output::failure(format!("FAIL: cannot read {src}: {e}")),
    };
    if paths.is_empty() {
        return Output::stderr(format!("no source modules found in {src}/"));
    }
    let mut modules = Vec::new();
    for path in &paths {
        let text = match read_text(path) {
            Ok(text) => text,
            Err(e) => return Output::failure(format!("FAIL: cannot read {}: {e}", display(path))),
        };
        modules.push(formula::statements(&text));
    }
    let declared_names = modules
        .iter()
        .flatten()
        .flat_map(|st| captures(formula::NAME, st))
        .map(|m| m[1].clone())
        .collect::<BTreeSet<_>>();
    let mut failures = Vec::new();
    let (mut no_signature, mut no_table, mut not_lambda) = (Vec::new(), Vec::new(), Vec::new());
    let (mut signatures, mut tables, mut examples, mut read) = (0, 0, 0, 0);
    for statement in modules.into_iter().flatten() {
        let matched = captures(formula::NAME, &statement);
        let Some(m) = matched.first() else {
            continue;
        };
        read += 1;
        let name = &m[1];
        let body = py::strip(&m[2]);
        let rows = rows(body);
        for (label, text) in &rows {
            if label.starts_with("WEBPAGE") && text.to_lowercase().contains("coming soon") {
                failures.push(format!("{name}: its WEBPAGE still says Coming Soon"));
            }
        }
        if name.starts_with("About") {
            let mut seen = BTreeMap::new();
            for (label, text) in &rows {
                let matched = captures(LABEL, label);
                if let Some(named) = matched.first()
                    && !declared_names.contains(&named[1])
                {
                    failures.push(format!(
                        "{name}'s table lists {label}, which the library does not declare"
                    ));
                }
                let bare = matched.first().map_or(label.as_str(), |m| m[1].as_str());
                if !declared_names.contains(bare) || bare == name {
                    continue;
                }
                let key = py::strip(&sub(r"\s+", text, " ")).to_lowercase();
                if key.is_empty() {
                    continue;
                }
                if let Some(previous) = seen.get(&key) {
                    failures.push(format!(
                        "{name}: {previous} and {label} share the same About-table description"
                    ));
                } else {
                    seen.insert(key, label);
                }
            }
        }
        let calls = captures(CALL, &literals(body))
            .into_iter()
            .map(|m| m[1].clone())
            .collect::<BTreeSet<_>>();
        for called in calls.difference(&declared_names) {
            failures.push(format!(
                "{name}'s help calls {called}, which the library does not declare"
            ));
        }
        if !body.starts_with("LAMBDA(") {
            not_lambda.push(name.clone());
            continue;
        }
        let real = declared(body);
        let without_internal = real
            .iter()
            .filter(|p| p.as_str() != "DoNotUse")
            .cloned()
            .collect::<Vec<_>>();
        if let Some((_, row)) = rows.iter().find(|(label, _)| label == "FUNCTION:") {
            signatures += 1;
            let signature = captures(SIGNATURE, py::strip(row));
            if let Some(sig) = signature.first() {
                if &sig[1] != name {
                    failures.push(format!("{name}: its help names {}", sig[1]));
                }
                let claimed = names(&sig[2]);
                if claimed != without_internal {
                    failures.push(format!(
                        "{name}: its signature says ({}) but it takes ({})",
                        claimed.join(", "),
                        without_internal.join(", ")
                    ));
                }
            } else {
                failures.push(format!(
                    "{name}: the help's FUNCTION line is not a signature: {}",
                    py::repr_str(prefix(py::strip(row), 70))
                ));
            }
        } else {
            no_signature.push(name.clone());
        }
        let calls = worked_examples(body);
        if !calls.is_empty() {
            examples += 1;
            if !calls.contains(name) {
                failures.push(format!(
                    "{name}'s worked examples never call it, they call {}",
                    calls
                        .into_iter()
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        if let Some(listed) = parameter_table(body) {
            tables += 1;
            if listed != real && listed != without_internal {
                failures.push(format!(
                    "{name}: its parameter table describes ({}) but it takes ({})",
                    if listed.is_empty() {
                        "nothing".into()
                    } else {
                        listed.join(", ")
                    },
                    real.join(", ")
                ));
            }
        } else {
            no_table.push(name.clone());
        }
    }
    let out = vec![
        format!(
            "{read} declarations read: {signatures} signatures, {tables} parameter tables and {examples} example blocks checked"
        ),
        format!(
            "   no FUNCTION line ({}): {}",
            no_signature.len(),
            list_or_none(no_signature)
        ),
        format!(
            "   no parameter table ({}): {}",
            no_table.len(),
            list_or_none(no_table)
        ),
        format!(
            "   not a LAMBDA ({}): {}",
            not_lambda.len(),
            list_or_none(not_lambda)
        ),
    ];
    if signatures < 100 || tables < 100 {
        return Output {
            code: 1,
            out,
            err: vec![format!(
                "only {signatures} signatures and {tables} tables parsed from {} module(s): the parser is not reading them",
                paths.len()
            )],
        };
    }
    let bad = format!("FAIL: {} help problem(s)", failures.len());
    let mut result = Output::report(
        failures,
        bad,
        format!(
            "OK: all {signatures} signatures, {tables} parameter tables and {examples} example blocks describe their own function"
        ),
    );
    result.out.splice(0..0, out);
    result
}
