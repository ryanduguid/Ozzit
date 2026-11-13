// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! Which native Excel checks reach each library name, read from tracked files.
//!
//! For every name in `functions.csv` the record counts the generated help and
//! worked-example assertions in `tools/selftest_examples.ps1` that name it, the
//! hand-written assertion calls in `tools/excel_selftest.ps1` that name it
//! directly or through a variable, and the worksheet formula cells in
//! `ozzit.xlsx` that call it, which the recalculation and cached-value gates
//! cover. The scripts are read by pattern, not run: only the arguments Excel
//! evaluates count, an assertion call inside a loop counts once, and neither a
//! name reached only from inside another function's definition nor one the
//! script builds at run time is counted, so the record can understate a
//! function's checks. Nothing here runs PowerShell or Excel.
use crate::{archive::*, common::*, formula};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const OUTPUT: &str = "docs/native-checks.csv";
const INDEX: &str = "functions.csv";
const SELFTEST: &str = "tools/excel_selftest.ps1";
const EXAMPLES: &str = "tools/selftest_examples.ps1";
const WORKBOOK: &str = "ozzit.xlsx";
const HEADER: [&str; 5] = [
    "function",
    "help_check",
    "worked_examples",
    "selftest_calls",
    "workbook_formula_cells",
];

/// A library name as the scripts spell it: `oz.Name$L`, `oz.Name${L}` or `oz.Name${L}DV`.
const SCRIPT_NAME: &str = r"oz\.([A-Za-z0-9_]+)(?:\$\{L\}([A-Za-z0-9_]*)|\$L(?![A-Za-z0-9_]))";
const VARIABLE: &str = r"\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?";
const ASSIGNMENT: &str = r"(?s)^\s*\$([A-Za-z_][A-Za-z0-9_]*)\s*=(.*)$";
const FOREACH: &str = r"(?s)^\s*foreach\s*\(\s*\$([A-Za-z_][A-Za-z0-9_]*)\s+in\s+(.*?)\)";
const CALL: &str = r"(?s)^\s*(Check|Near|Same|SameAsArray|Fuzz)\s+(?:'([a-z]+):)?";
/// `if ($a -eq $b) {` or `-ne`, alone on its line: the block narrows `$a`.
const NARROW: &str = r"^\s*if\s*\(\s*\$([A-Za-z_][A-Za-z0-9_]*)\s+-(eq|ne)\s+\$([A-Za-z_][A-Za-z0-9_]*)\s*\)\s*\{\s*$";
const FUNCTION: &str = r"^\s*function\s.*\{\s*$";
/// The condition of `$x = if (...) { ... } else { ... }`, which names no value of `$x`.
const IF_CONDITION: &str = r"^\s*if\s*\([^()]*\)";

/// A block opened at `indent`: a function body, whose calls are not assertions, or
/// an `if` that narrows a variable, whose names are restored when the block ends.
enum Scope {
    Function(usize),
    Narrow(usize, String, Option<BTreeSet<String>>),
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// A command line's arguments: quoted strings with their quotes, or bare words.
fn arguments(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        match bytes[i] {
            quote @ (b'\'' | b'"') => {
                i += 1;
                while i < bytes.len() {
                    let escaped = quote == b'"' && bytes[i] == b'`';
                    let doubled = bytes[i] == quote && bytes.get(i + 1) == Some(&quote);
                    if escaped || doubled {
                        i += 2;
                    } else if bytes[i] == quote {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
            }
            _ => {
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
            }
        }
        out.push(&line[start..i.min(line.len())]);
    }
    out
}

/// The arguments Excel evaluates: all but the id, and for `Same` not the expected
/// text, which is compared as a string rather than run.
fn evaluated(line: &str) -> String {
    let words = arguments(line);
    let take = if words.first() == Some(&"Same") {
        3
    } else {
        words.len()
    };
    words.get(2..take).unwrap_or_default().join(" ")
}
const WORKBOOK_NAME: &str = r"(?<![A-Za-z0-9_.λ])oz\.[A-Za-z0-9_.λ]+";
const FORMULA: &str = r"(?s)<f\b([^>]*?)(?:/>|>(.*?)</f>)";
const SHARED_INDEX: &str = r#"\bsi="(\d+)""#;

/// Assertion calls per name in one script, keyed by the id's prefix (`help`,
/// `example` or empty for an id without one).
type Calls = BTreeMap<String, BTreeMap<String, usize>>;

/// Physical lines joined across PowerShell's trailing-backtick continuations.
fn logical_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = String::new();
    for line in text.lines() {
        if let Some(head) = line.strip_suffix('`') {
            pending.push_str(head);
            pending.push(' ');
        } else {
            pending.push_str(line);
            out.push(std::mem::take(&mut pending));
        }
    }
    out
}

/// The library names `text` spells out or reaches through a variable.
fn names_in(text: &str, variables: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    let mut names = captures(SCRIPT_NAME, text)
        .into_iter()
        .map(|m| format!("oz.{}λ{}", m[1], m[2]))
        .collect::<BTreeSet<_>>();
    for m in captures(VARIABLE, text) {
        if let Some(held) = variables.get(&m[1].to_lowercase()) {
            names.extend(held.iter().cloned());
        }
    }
    names
}

/// The library names each script variable holds, keyed by its lower-cased name.
type Variables = BTreeMap<String, BTreeSet<String>>;

/// Ends the blocks `line` closes, restoring the names each narrowing `if` changed.
fn close_scopes(line: &str, scopes: &mut Vec<Scope>, variables: &mut Variables) {
    while let Some(scope) = scopes.last() {
        let opened = match scope {
            Scope::Function(at) | Scope::Narrow(at, ..) => *at,
        };
        if !(line.trim_start().starts_with('}') && indent(line) <= opened) {
            break;
        }
        if let Some(Scope::Narrow(_, name, saved)) = scopes.pop() {
            match saved {
                Some(names) => variables.insert(name, names),
                None => variables.remove(&name),
            };
        }
    }
}

/// Narrows the variable a `NARROW` header (`m`) tests and returns the scope that
/// restores it.
fn narrow(line: &str, m: &[String], variables: &mut Variables) -> Scope {
    let name = m[1].to_lowercase();
    let saved = variables.get(&name).cloned();
    let held = saved.clone().unwrap_or_default();
    let other = variables
        .get(&m[3].to_lowercase())
        .cloned()
        .unwrap_or_default();
    let narrowed = if &m[2] == "eq" {
        held.intersection(&other).cloned().collect()
    } else {
        held.difference(&other).cloned().collect()
    };
    variables.insert(name.clone(), narrowed);
    Scope::Narrow(indent(line), name, saved)
}

/// The assertion calls in a script and the library names each reaches.
///
/// Variables take the names their latest assignment or `foreach` header holds, in
/// line order, so a variable reused in a later section names only that section's
/// functions. PowerShell variable names are case-insensitive. A block ends at the
/// first later line that starts with `}` at or left of its opening indent.
fn script_calls(text: &str, known: &BTreeSet<String>, file: &str) -> Result<Calls> {
    let mut variables = Variables::new();
    let mut scopes: Vec<Scope> = Vec::new();
    let mut calls = Calls::new();
    for line in logical_lines(text) {
        if line.trim_start().starts_with('#') {
            continue;
        }
        close_scopes(&line, &mut scopes, &mut variables);
        if matches(FUNCTION, &line) {
            scopes.push(Scope::Function(indent(&line)));
            continue;
        }
        if let Some(m) = captures(NARROW, &line).first() {
            scopes.push(narrow(&line, m, &mut variables));
            continue;
        }
        if scopes.iter().any(|s| matches!(s, Scope::Function(_))) {
            continue;
        }
        for pattern in [FOREACH, ASSIGNMENT] {
            if let Some(m) = captures(pattern, &line).first() {
                let names = names_in(&sub(IF_CONDITION, &m[2], ""), &variables);
                variables.insert(m[1].to_lowercase(), names);
            }
        }
        if let Some(m) = captures(CALL, &line).first() {
            for name in names_in(&evaluated(line.trim_start()), &variables) {
                if !known.contains(&name) {
                    return Err(format!("{file} calls {name}, which {INDEX} does not list"));
                }
                *calls
                    .entry(name)
                    .or_default()
                    .entry(m[2].clone())
                    .or_default() += 1;
            }
        }
    }
    Ok(calls)
}

/// The generated fragment's calls, each id prefixed `help` or `example`.
fn example_calls(text: &str, known: &BTreeSet<String>) -> Result<Calls> {
    let calls = script_calls(text, known, EXAMPLES)?;
    match calls
        .values()
        .flat_map(BTreeMap::keys)
        .find(|p| !matches!(p.as_str(), "help" | "example"))
    {
        Some(prefix) => Err(format!(
            "{EXAMPLES} has an assertion id prefixed {prefix:?}; the record counts only help and example"
        )),
        None => Ok(calls),
    }
}

/// Worksheet formula cells per library name they call, a shared formula's
/// dependent cells counting under its anchor's text. String literals are skipped.
fn workbook_cells(parts: &Parts, known: &BTreeSet<String>) -> Result<BTreeMap<String, usize>> {
    let mut cells = BTreeMap::new();
    for (part, bytes) in parts {
        if !part.starts_with("xl/worksheets/") || !part.ends_with(".xml") {
            continue;
        }
        let sheet = decode(bytes)?;
        let formulas = captures(FORMULA, &sheet);
        let anchors = formulas
            .iter()
            .filter(|f| !f[2].is_empty())
            .filter_map(|f| {
                captures(SHARED_INDEX, &f[1])
                    .first()
                    .map(|si| (si[1].clone(), f[2].clone()))
            })
            .collect::<BTreeMap<_, _>>();
        for f in &formulas {
            let text = if f[2].is_empty() {
                let Some(si) = captures(SHARED_INDEX, &f[1]).into_iter().next() else {
                    continue;
                };
                anchors
                    .get(&si[1])
                    .ok_or_else(|| format!("{part}: shared formula {} has no anchor", si[1]))?
                    .clone()
            } else {
                f[2].clone()
            };
            let text = unescape(&text)?;
            let mut names = BTreeSet::new();
            for (literal, piece) in formula::split_literals(&text) {
                if !literal {
                    names.extend(
                        captures(WORKBOOK_NAME, piece)
                            .into_iter()
                            .map(|m| m[0].clone()),
                    );
                }
            }
            for name in names {
                if !known.contains(&name) {
                    return Err(format!("{part} calls {name}, which {INDEX} does not list"));
                }
                *cells.entry(name).or_default() += 1;
            }
        }
    }
    Ok(cells)
}

/// The record as CSV text, one row per `functions.csv` name in its order.
pub fn record(root: &Path) -> Result<String> {
    let csv = Csv::read(&root.join(INDEX))?;
    let names = csv
        .rows
        .iter()
        .map(|row| {
            csv.get(row, "function")
                .map(str::to_owned)
                .ok_or_else(|| format!("{INDEX} has a row without a function"))
        })
        .collect::<Result<Vec<_>>>()?;
    let known = names.iter().cloned().collect::<BTreeSet<_>>();
    let examples = example_calls(&read_text(&root.join(EXAMPLES))?, &known)?;
    let selftest = script_calls(&read_text(&root.join(SELFTEST))?, &known, SELFTEST)?;
    let cells = workbook_cells(&read_parts(&root.join(WORKBOOK))?, &known)?;
    let count = |calls: &Calls, name: &str, prefix: &str| {
        calls
            .get(name)
            .and_then(|by| by.get(prefix))
            .copied()
            .unwrap_or(0)
    };
    let mut out = csv_row(&HEADER.map(str::to_owned));
    for name in &names {
        out.push_str(&csv_row(&[
            name.clone(),
            count(&examples, name, "help").to_string(),
            count(&examples, name, "example").to_string(),
            selftest
                .get(name)
                .map_or(0, |by| by.values().sum::<usize>())
                .to_string(),
            cells.get(name).copied().unwrap_or(0).to_string(),
        ]));
    }
    Ok(out)
}

/// `ozzit native_checks [--check]`, run from the repository root.
pub fn command(args: &[String]) -> Output {
    let check_only = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Output::stderr("FAIL: usage: ozzit native_checks [--check]"),
    };
    let text = match record(Path::new(".")) {
        Ok(text) => text,
        Err(e) => return Output::failure(format!("FAIL: {e}")),
    };
    let rows = text.lines().count() - 1;
    let current = read_text(Path::new(OUTPUT)).is_ok_and(|before| before == text);
    if current {
        return Output::ok(vec![format!("OK: {OUTPUT} is current, {rows} names")]);
    }
    if check_only {
        return Output::failure(format!("FAIL: {OUTPUT} is stale; run ozzit native_checks"));
    }
    match write_bytes(Path::new(OUTPUT), text.as_bytes()) {
        Ok(()) => Output::ok(vec![format!("wrote {OUTPUT}: {rows} names")]),
        Err(e) => Output::failure(format!("FAIL: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| format!("oz.{n}")).collect()
    }

    fn totals(calls: &Calls) -> BTreeMap<String, usize> {
        calls
            .iter()
            .map(|(name, by)| (name.clone(), by.values().sum()))
            .collect()
    }

    #[test]
    fn an_if_on_the_loop_variable_narrows_it() {
        let script = r#"
$a = "oz.A$L"
$b = "oz.B$L"
foreach ($fn in $a, $b) {
    Same "both $fn" "$fn(1)" '1'
    $width = if ($fn -eq $a) { '1' } else { '{1,1}' }
    Same "width alone $fn" "SUM($width)" '1'
    if ($fn -eq $a) {
        Same 'only a' "$fn(2)" '2'
    }
    if ($fn -ne $a) {
        Same 'only b' "$fn(3)" '3'
        Same 'only b again' "$fn(4)" '4'
    }
    Same "both again $fn" "$fn(5)" '5'
}
"#;
        let calls = script_calls(script, &known(&["Aλ", "Bλ"]), "t.ps1").unwrap();
        assert_eq!(
            totals(&calls),
            BTreeMap::from([("oz.Aλ".into(), 3), ("oz.Bλ".into(), 4)])
        );
    }

    #[test]
    fn only_arguments_excel_evaluates_count() {
        let script = r#"
function Probe($id) {
    Same $id "oz.W$L(1)" '1'
}
Same 'display' "'Sheet'!B1" "=oz.X$L(1)"
Same "an id naming oz.Y$L" "1" '1'
Near 'want runs' "1" "oz.Z$L(1)"
"#;
        let calls = script_calls(script, &known(&["Wλ", "Xλ", "Yλ", "Zλ"]), "t.ps1").unwrap();
        assert_eq!(totals(&calls), BTreeMap::from([("oz.Zλ".into(), 1)]));
    }

    #[test]
    fn calls_follow_aliases_loops_and_continuations() {
        let script = r#"
$a = "oz.Amortise$L"
$adv = "oz.Amortise${L}DV"
# Same 'comment' "oz.Ignored$L" 'x'
Same 'direct' "oz.GSTAdd${L}(1)" '1'
Near 'alias' "$a(1)" `
     '1'
foreach ($fn in $a, $ADV) {
    Same "loop $fn" "$fn()" 'x'
}
$fn = "oz.GSTAdd$L"
Same 'reused' "$fn(1)" '1'
Check 'neither' "=1"
"#;
        let calls = script_calls(
            script,
            &known(&["Amortiseλ", "AmortiseλDV", "GSTAddλ"]),
            "t.ps1",
        )
        .unwrap();
        let total = |name: &str| calls.get(name).map_or(0, |by| by.values().sum::<usize>());
        assert_eq!(total("oz.Amortiseλ"), 2);
        assert_eq!(total("oz.AmortiseλDV"), 1);
        assert_eq!(total("oz.GSTAddλ"), 2);
        assert_eq!(calls.len(), 3);
    }

    #[test]
    fn an_assignment_inside_a_function_leaves_top_level_aliases() {
        let script = r#"
$fn = "oz.A$L"
function Probe {
    $fn = "oz.B$L"
}
Same 'top' "$fn(1)" '1'
"#;
        let calls = script_calls(script, &known(&["Aλ", "Bλ"]), "t.ps1").unwrap();
        assert_eq!(totals(&calls), BTreeMap::from([("oz.Aλ".into(), 1)]));
    }

    #[test]
    fn a_narrowing_if_inside_a_function_leaves_top_level_aliases() {
        let script = r#"
$a = "oz.A$L"
$b = "oz.B$L"
$fn = $a
function Probe {
    if ($fn -eq $b) {
        Same 'inner' "$fn(1)" '1'
    }
}
Same 'top' "$fn(2)" '2'
"#;
        let calls = script_calls(script, &known(&["Aλ", "Bλ"]), "t.ps1").unwrap();
        assert_eq!(totals(&calls), BTreeMap::from([("oz.Aλ".into(), 1)]));
    }

    #[test]
    fn calls_split_by_id_prefix() {
        let script = "Same 'help: X${L}' \"INDEX(oz.X${L}(),1,1)\" 'FUNCTION:'\n\
                      Near 'example: X${L} #1' \"oz.X${L}(1)\" '2'\n\
                      Near 'example: X${L} #2' \"oz.X${L}(2)\" '4'\n";
        let calls = example_calls(script, &known(&["Xλ"])).unwrap();
        assert_eq!(
            calls["oz.Xλ"],
            BTreeMap::from([("example".into(), 2), ("help".into(), 1)])
        );
        assert_eq!(
            example_calls("Near 'fuzz: X' \"oz.X$L(1)\" '2'", &known(&["Xλ"])).unwrap_err(),
            "tools/selftest_examples.ps1 has an assertion id prefixed \"fuzz\"; the record counts only help and example"
        );
    }

    #[test]
    fn a_call_to_an_unlisted_name_is_refused() {
        let error =
            script_calls("Same 'x' \"oz.Gone$L(1)\" '1'", &known(&["Xλ"]), "t.ps1").unwrap_err();
        assert_eq!(
            error,
            "t.ps1 calls oz.Goneλ, which functions.csv does not list"
        );
    }

    #[test]
    fn workbook_cells_expand_shared_formulas_and_skip_literals() {
        let sheet = r##"<worksheet><sheetData><row>
<c r="A1"><f t="shared" ref="A1:A3" si="0">oz.Xλ(B1)+oz.Yλ(1)</f><v>1</v></c>
<c r="A2"><f t="shared" si="0"/><v>1</v></c>
<c r="A3"><f t="shared" si="0"></f><v>1</v></c>
<c r="A4"><f>HYPERLINK("#A1","oz.Yλ")&amp;oz.Xλ(1)</f><v>1</v></c>
</row></sheetData></worksheet>"##;
        let parts = Parts::from([("xl/worksheets/sheet1.xml".into(), sheet.as_bytes().to_vec())]);
        let cells = workbook_cells(&parts, &known(&["Xλ", "Yλ"])).unwrap();
        assert_eq!(
            cells,
            BTreeMap::from([("oz.Xλ".into(), 4), ("oz.Yλ".into(), 3)])
        );
    }

    #[test]
    fn a_shared_formula_without_its_anchor_is_refused() {
        let sheet = r#"<worksheet><c r="A2"><f t="shared" si="7"/></c></worksheet>"#;
        let parts = Parts::from([("xl/worksheets/sheet1.xml".into(), sheet.as_bytes().to_vec())]);
        assert_eq!(
            workbook_cells(&parts, &known(&["Xλ"])).unwrap_err(),
            "xl/worksheets/sheet1.xml: shared formula 7 has no anchor"
        );
    }
}
