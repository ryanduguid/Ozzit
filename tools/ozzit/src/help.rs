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

/// What `verify` has found so far across the declarations it has read.
#[derive(Default)]
struct Tally {
    failures: Vec<String>,
    no_signature: Vec<String>,
    no_table: Vec<String>,
    not_lambda: Vec<String>,
    signatures: usize,
    tables: usize,
    examples: usize,
    read: usize,
}

impl Tally {
    fn statement(&mut self, statement: &str, declared_names: &BTreeSet<String>) {
        let matched = captures(formula::NAME, statement);
        let Some(m) = matched.first() else {
            return;
        };
        self.read += 1;
        let name = &m[1];
        let body = py::strip(&m[2]);
        let rows = rows(body);
        for (label, text) in &rows {
            if label.starts_with("WEBPAGE") && text.to_lowercase().contains("coming soon") {
                self.failures
                    .push(format!("{name}: its WEBPAGE still says Coming Soon"));
            }
        }
        if name.starts_with("About") {
            self.about_table(name, &rows, declared_names);
        }
        let calls = captures(CALL, &literals(body))
            .into_iter()
            .map(|m| m[1].clone())
            .collect::<BTreeSet<_>>();
        for called in calls.difference(declared_names) {
            self.failures.push(format!(
                "{name}'s help calls {called}, which the library does not declare"
            ));
        }
        if !body.starts_with("LAMBDA(") {
            self.not_lambda.push(name.clone());
            return;
        }
        let real = declared(body);
        let without_internal = real
            .iter()
            .filter(|p| p.as_str() != "DoNotUse")
            .cloned()
            .collect::<Vec<_>>();
        self.signature(name, &rows, &without_internal);
        self.worked_examples(name, body);
        self.parameter_table(name, body, &real, &without_internal);
    }

    fn about_table(
        &mut self,
        name: &str,
        rows: &[(String, String)],
        declared_names: &BTreeSet<String>,
    ) {
        let mut seen = BTreeMap::new();
        for (label, text) in rows {
            let matched = captures(LABEL, label);
            if let Some(named) = matched.first()
                && !declared_names.contains(&named[1])
            {
                self.failures.push(format!(
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
                self.failures.push(format!(
                    "{name}: {previous} and {label} share the same About-table description"
                ));
            } else {
                seen.insert(key, label);
            }
        }
    }

    fn signature(&mut self, name: &str, rows: &[(String, String)], without_internal: &[String]) {
        let Some((_, row)) = rows.iter().find(|(label, _)| label == "FUNCTION:") else {
            self.no_signature.push(name.into());
            return;
        };
        self.signatures += 1;
        let signature = captures(SIGNATURE, py::strip(row));
        if let Some(sig) = signature.first() {
            if sig[1] != name {
                self.failures
                    .push(format!("{name}: its help names {}", sig[1]));
            }
            let claimed = names(&sig[2]);
            if claimed != without_internal {
                self.failures.push(format!(
                    "{name}: its signature says ({}) but it takes ({})",
                    claimed.join(", "),
                    without_internal.join(", ")
                ));
            }
        } else {
            self.failures.push(format!(
                "{name}: the help's FUNCTION line is not a signature: {}",
                py::repr_str(prefix(py::strip(row), 70))
            ));
        }
    }

    fn worked_examples(&mut self, name: &str, body: &str) {
        let calls = worked_examples(body);
        if calls.is_empty() {
            return;
        }
        self.examples += 1;
        if !calls.iter().any(|c| c == name) {
            self.failures.push(format!(
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

    fn parameter_table(
        &mut self,
        name: &str,
        body: &str,
        real: &[String],
        without_internal: &[String],
    ) {
        let Some(listed) = parameter_table(body) else {
            self.no_table.push(name.into());
            return;
        };
        self.tables += 1;
        if listed != real && listed != without_internal {
            self.failures.push(format!(
                "{name}: its parameter table describes ({}) but it takes ({})",
                if listed.is_empty() {
                    "nothing".into()
                } else {
                    listed.join(", ")
                },
                real.join(", ")
            ));
        }
    }

    fn finish(self, modules: usize, floor: usize) -> Output {
        let Self {
            failures,
            no_signature,
            no_table,
            not_lambda,
            signatures,
            tables,
            examples,
            read,
        } = self;
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
        if signatures < floor || tables < floor {
            return Output {
                code: 1,
                out,
                err: vec![format!(
                    "only {signatures} signatures and {tables} tables parsed from {modules} module(s): the parser is not reading them"
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
}

pub fn verify(args: &[String]) -> Output {
    verify_with_floor(args, 100)
}

fn verify_with_floor(args: &[String], floor: usize) -> Output {
    if args.len() > 1 {
        return Output::stderr("FAIL: usage: ozzit verify_signatures [src dir]");
    }
    let src = args.first().map_or("src", String::as_str);
    let mut paths = match txt_files(Path::new(src)) {
        Ok(paths) => paths,
        Err(e) => return Output::failure(format!("FAIL: cannot read {src}: {e}")),
    };
    // Python's glob.glob skips hidden files; the other twins' Path.glob does not.
    paths.retain(|p| !basename(p).starts_with('.'));
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
    let mut tally = Tally::default();
    for statement in modules.into_iter().flatten() {
        tally.statement(&statement, &declared_names);
    }
    tally.finish(paths.len(), floor)
}

#[cfg(test)]
mod signature_contract_tests {
    use super::*;

    fn help_function(
        name: &str,
        params: &[&str],
        signature: &str,
        table: &[&str],
        example: &str,
    ) -> String {
        let mut rows = vec![format!("FUNCTION:→{signature}"), "PARAMETERS:→".into()];
        rows.extend(table.iter().map(|p| format!("{p}→a value")));
        rows.extend([
            "EXAMPLES:→".into(),
            format!("EXAMPLE→{example}"),
            "END:→".into(),
        ]);
        let help = rows
            .iter()
            .map(|row| format!("\"{row}¶\""))
            .collect::<Vec<_>>()
            .join(" & ");
        let declared = params
            .iter()
            .map(|p| format!("[{p}]"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{name} = LAMBDA({declared}, LET(Help, TRIM(TEXTSPLIT({help}, \"→\", \"¶\")), 0));\n"
        )
    }

    fn fixture() -> (tempfile::TempDir, String, Vec<u8>) {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        let mut text = help_function("Goodλ", &["x"], "Goodλ(x)", &["x"], "=Goodλ(1)");
        text += &help_function("NotSigλ", &["x"], "not a signature", &["x"], "=NotSigλ(1)");
        text += &help_function("WrongNameλ", &["x"], "Goodλ(x)", &["x"], "=WrongNameλ(1)");
        text += &help_function(
            "WrongParamsλ",
            &["x", "y"],
            "WrongParamsλ(x)",
            &["x", "y"],
            "=WrongParamsλ(1, 2)",
        );
        text += &help_function(
            "WrongExampleλ",
            &["x"],
            "WrongExampleλ(x)",
            &["x"],
            "=Goodλ(1)",
        );
        text += &help_function(
            "WrongTableλ",
            &["x"],
            "WrongTableλ(x)",
            &["z"],
            "=WrongTableλ(1)",
        );
        text += "AboutFixtureλ = TRIM(TEXTSPLIT(\"Function→Description¶\" & \"Goodλ→¶\", \"→\", \"¶\"));\nnot a declaration;\n";
        std::fs::write(src.join("Fixture.txt"), &text).unwrap();
        (dir, display(&src), text.into_bytes())
    }

    fn assert_preserved(dir: &Path, before: &[u8]) {
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 1);
        let src = dir.join("src");
        assert_eq!(std::fs::read_dir(&src).unwrap().count(), 1);
        assert_eq!(std::fs::read(src.join("Fixture.txt")).unwrap(), before);
    }

    #[test]
    fn all_five_original_help_mismatches_follow_the_exact_fixture_tallies() {
        let (dir, src, before) = fixture();
        let result = verify_with_floor(&[src], 1);
        assert_preserved(dir.path(), &before);
        assert_eq!(result.code, 1);
        assert!(result.err.is_empty());
        assert_eq!(
            result.out,
            [
                "7 declarations read: 6 signatures, 6 parameter tables and 6 example blocks checked",
                "   no FUNCTION line (0): none",
                "   no parameter table (0): none",
                "   not a LAMBDA (1): AboutFixtureλ",
                "FAIL: 5 help problem(s)",
                "  - NotSigλ: the help's FUNCTION line is not a signature: 'not a signature'",
                "  - WrongNameλ: its help names Goodλ",
                "  - WrongParamsλ: its signature says (x) but it takes (x, y)",
                "  - WrongExampleλ's worked examples never call it, they call Goodλ",
                "  - WrongTableλ: its parameter table describes (z) but it takes (x)",
            ]
        );
    }

    #[test]
    fn the_original_six_signature_fixture_fails_the_normal_floor_first() {
        let (dir, src, before) = fixture();
        let result = verify(&[src]);
        assert_preserved(dir.path(), &before);
        assert_eq!(result.code, 1);
        assert_eq!(
            result.out,
            [
                "7 declarations read: 6 signatures, 6 parameter tables and 6 example blocks checked",
                "   no FUNCTION line (0): none",
                "   no parameter table (0): none",
                "   not a LAMBDA (1): AboutFixtureλ",
            ]
        );
        assert_eq!(
            result.err,
            [
                "only 6 signatures and 6 tables parsed from 1 module(s): the parser is not reading them"
            ]
        );
    }
}
