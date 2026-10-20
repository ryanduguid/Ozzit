//! Per-function reference data read from each definition's own help block.
//!
//! Every LAMBDA carries its documentation as the first argument of the
//! `TEXTSPLIT` bound to `Help`: rows split on `¶`, each with a label and text
//! split on `→`. This module reads those rows into signature, description,
//! parameters, notes and worked examples, joins each function to its row in
//! `functions.csv`, and writes one JSON file a website or an agent can render.
use crate::{common::*, formula, help, py};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const FORMAT: &str = "ozzit-function-reference/1";
const REQUIREMENT: &str = r"(?s)^\((Required|Optional)(?:\s*[:;,]\s*([^)]*))?\)\s*(.*)$";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub requirement: Option<String>,
    pub qualifier: Option<String>,
    pub text: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Example {
    pub formula: String,
    pub result: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Section {
    pub heading: String,
    pub rows: Vec<(String, String)>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct HelpBlock {
    pub signature: Option<String>,
    pub description: String,
    pub see_also: Option<String>,
    pub version: Option<String>,
    pub parameters: Vec<Parameter>,
    pub notes: Vec<String>,
    pub examples: Vec<Example>,
    pub extra: Vec<Section>,
}

/// The `¶`/`→` rows of the help text, keeping continuation rows separate.
pub fn help_rows(text: &str) -> Vec<(String, String)> {
    text.split('¶')
        .filter_map(|raw| raw.split_once('→'))
        .map(|(label, text)| (py::strip(label).to_owned(), py::strip(text).to_owned()))
        .collect()
}

/// The help text of a LAMBDA body: the literals of the `Help` binding's
/// `TEXTSPLIT` first argument. `None` when the body has no such binding.
pub fn help_text(body: &str) -> Option<String> {
    let help = body.find("Help,")?;
    let call = help + body[help..].find("TEXTSPLIT(")? + "TEXTSPLIT(".len();
    let args = formula::arguments(body, call);
    (args.len() == 3 && help::literals(&args[1]) == "→" && help::literals(&args[2]) == "¶")
        .then(|| help::literals(&args[0]))
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Append a continuation row. Wrapped prose rejoins with a space. A `•`
/// bullet row, and every row after a bullet or after a row ending in `:`,
/// starts a new line so a list survives; a row starting in lower case still
/// continues the sentence before it.
fn join(target: &mut String, more: &str) {
    if more.is_empty() {
        return;
    }
    if !target.is_empty() {
        let after_bullet = target.lines().last().is_some_and(|l| l.starts_with('•'));
        let listing = target.ends_with(':') || target.contains(":\n");
        let wrapped = more.starts_with(char::is_lowercase);
        target.push(
            if more.starts_with('•') || after_bullet || (listing && !wrapped) {
                '\n'
            } else {
                ' '
            },
        );
    }
    target.push_str(more);
}

fn parameter(name: &str, text: &str) -> Parameter {
    match captures(REQUIREMENT, text).first() {
        Some(m) => Parameter {
            name: name.into(),
            requirement: Some(m[1].clone()),
            qualifier: Some(collapse(&m[2])).filter(|q| !q.is_empty()),
            text: collapse(&m[3]),
        },
        None => Parameter {
            name: name.into(),
            text: collapse(text),
            ..Parameter::default()
        },
    }
}

/// Turn the rows under EXAMPLES into formulas and the results the help shows.
///
/// The help writes examples in 3 shapes: `result → =formula` rows under a
/// `Result → Formula` header; a vertical `Formula`, `=formula`, `Result`,
/// `value` list, or `Formula:` and `Result:` rows; and a formula on the EXAMPLES
/// row itself. A row that does not start with `=` continues the previous
/// formula, except the value after a bare `Result` marker and the rows that
/// continue it.
pub fn examples(rows: &[(String, String)]) -> Vec<Example> {
    let mut out: Vec<Example> = Vec::new();
    let mut result_next = false;
    let mut in_result = false;
    for (label, text) in rows {
        let header =
            (label == "Result" && text.starts_with("Formula")) || text.starts_with("Formula (");
        if header || (label.is_empty() && text.is_empty()) {
            continue;
        }
        if label.is_empty() && text == "Result" {
            result_next = true;
            in_result = false;
            continue;
        }
        if text.starts_with('=') {
            out.push(Example {
                formula: text.clone(),
                result: Some(label.clone()).filter(|l| !l.is_empty()),
            });
            result_next = false;
            in_result = false;
        } else if let Some(last) = out.last_mut() {
            if result_next {
                last.result = Some(text.clone());
                result_next = false;
                in_result = true;
            } else if in_result && label.is_empty() {
                let result = last.result.get_or_insert_with(String::new);
                result.push('\n');
                result.push_str(text);
            } else if label.is_empty() {
                last.formula.push('\n');
                last.formula.push_str(text);
            }
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum In {
    Start,
    Signature,
    Description,
    SeeAlso,
    Parameters,
    Examples,
    Note,
    Extra,
    Ignored,
}

/// Start the section a heading names, keeping any text on the heading's own row.
fn heading(
    block: &mut HelpBlock,
    examples: &mut Vec<(String, String)>,
    heading: String,
    text: &str,
) -> In {
    let some = |t: String| Some(t).filter(|t| !t.is_empty());
    match heading.as_str() {
        "FUNCTION" => {
            block.signature = some(text.into());
            In::Signature
        }
        "DESCRIPTION" => {
            join(&mut block.description, &collapse(text));
            In::Description
        }
        "SEE ALSO" => {
            block.see_also = some(collapse(text));
            In::SeeAlso
        }
        "VERSION" => {
            block.version = some(text.into());
            In::Ignored
        }
        "PARAMETERS" => In::Parameters,
        "EXAMPLE" | "EXAMPLES" | "FORMULA" => {
            examples.push((String::new(), text.into()));
            In::Examples
        }
        "RESULT" => {
            examples.push((String::new(), "Result".into()));
            examples.push((String::new(), text.into()));
            In::Examples
        }
        "NOTE" | "NOTES" => {
            block.notes.push(collapse(text));
            In::Note
        }
        "WEBPAGE" | "WEBSITE" | "REPOSITORY" => In::Ignored,
        _ => {
            let rows = if text.is_empty() {
                vec![]
            } else {
                vec![(String::new(), collapse(text))]
            };
            block.extra.push(Section { heading, rows });
            In::Extra
        }
    }
}

/// A row under a heading: a new parameter or example, or the continuation of
/// the previous entry when its label is empty.
fn row(
    block: &mut HelpBlock,
    examples: &mut Vec<(String, String)>,
    state: In,
    label: String,
    text: &str,
) {
    let more = label.is_empty();
    let extend = |target: Option<&mut String>, text: &str| {
        if let Some(target) = target {
            join(target, text);
        }
    };
    match state {
        In::Signature if more => extend(block.signature.as_mut(), text),
        In::Description if more => join(&mut block.description, &collapse(text)),
        In::SeeAlso if more => extend(block.see_also.as_mut(), &collapse(text)),
        In::Parameters if !more => block.parameters.push(parameter(&label, text)),
        In::Parameters => extend(
            block.parameters.last_mut().map(|p| &mut p.text),
            &collapse(text),
        ),
        In::Examples => examples.push((label, text.into())),
        In::Note if more => extend(block.notes.last_mut(), &collapse(text)),
        In::Extra if !(more && text.is_empty()) => {
            if let Some(section) = block.extra.last_mut() {
                section.rows.push((label, collapse(text)));
            }
        }
        _ => {}
    }
}

pub fn parse(text: &str) -> HelpBlock {
    let mut block = HelpBlock::default();
    let mut example_rows = Vec::new();
    let mut state = In::Start;
    for (label, text) in help_rows(text) {
        if let Some(name) = label.strip_suffix(':') {
            let name = py::strip(name).to_uppercase();
            state = heading(&mut block, &mut example_rows, name, &text);
        } else if label.ends_with('!') {
            block.notes.push(collapse(&text));
            state = In::Note;
        } else {
            row(&mut block, &mut example_rows, state, label, &text);
        }
    }
    block.notes.retain(|n| !n.is_empty());
    block.examples = examples(&example_rows);
    block
}

type IndexRow = BTreeMap<&'static str, String>;
const INDEX_FIELDS: [&str; 5] = [
    "function",
    "module",
    "previous_name",
    "signature",
    "description",
];

fn block_lists(block: &HelpBlock) -> [Value; 3] {
    let parameters = block.parameters.iter().map(|p| {
        json!({"name": p.name, "requirement": p.requirement, "qualifier": p.qualifier, "text": p.text})
    });
    let examples = block
        .examples
        .iter()
        .map(|e| json!({"formula": e.formula, "result": e.result}));
    let extra = block.extra.iter().map(|s| {
        let rows = s.rows.iter().map(|(l, t)| json!({"label": l, "text": t}));
        json!({"heading": s.heading, "rows": rows.collect::<Vec<_>>()})
    });
    [
        Value::Array(parameters.collect()),
        Value::Array(examples.collect()),
        Value::Array(extra.collect()),
    ]
}

fn function_json(name: &str, body: &str, row: Option<&IndexRow>) -> Value {
    let field = |key: &str| {
        row.and_then(|r| r.get(key))
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    };
    let block = help_text(body).map(|t| parse(&t)).unwrap_or_default();
    let [parameters, examples, extra] = block_lists(&block);
    json!({
        "name": format!("oz.{name}"),
        "previous_name": field("previous_name"),
        "summary": field("description"),
        "kind": if body.starts_with("LAMBDA(") { "lambda" } else { "table" },
        "signature": block.signature.as_deref().or_else(|| field("signature")),
        "description": Some(block.description.as_str()).filter(|d| !d.is_empty()),
        "see_also": block.see_also,
        "version": block.version,
        "parameters": parameters,
        "notes": block.notes,
        "examples": examples,
        "extra": extra,
    })
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The index rows by function name, noting any name listed twice.
fn index_rows(index: &Path, problems: &mut Vec<String>) -> Result<BTreeMap<String, IndexRow>> {
    let csv = Csv::read(index)?;
    let mut rows = BTreeMap::new();
    for row in &csv.rows {
        let fields: IndexRow = INDEX_FIELDS
            .iter()
            .map(|&key| (key, csv.get(row, key).unwrap_or("").to_owned()))
            .collect();
        let name = fields["function"].clone();
        if rows.insert(name.clone(), fields).is_some() {
            problems.push(format!("{name} is listed more than once in the index"));
        }
    }
    Ok(rows)
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect()
}

/// Problems with one source definition against its index row.
fn index_problems(
    key: &str,
    module: &str,
    help: Option<&str>,
    row: Option<&IndexRow>,
) -> Vec<String> {
    let Some(row) = row else {
        return vec![format!("{key} is in src/{module}.txt but not in the index")];
    };
    let mut problems = Vec::new();
    if row["module"] != module {
        problems.push(format!(
            "{key} is in src/{module}.txt but the index files it under {}",
            row["module"]
        ));
    }
    if let Some(help) = help
        && squash(help) != squash(&row["signature"])
    {
        problems.push(format!(
            "{key}: its help signature {help} disagrees with the index signature {}",
            row["signature"]
        ));
    }
    problems
}

/// One module's functions in source order, noting any the index disagrees with.
fn module_json(
    src: &Path,
    module: &str,
    rows: &BTreeMap<String, IndexRow>,
    seen: &mut BTreeSet<String>,
    problems: &mut Vec<String>,
) -> Result<Value> {
    let path = src.join(format!("{module}.txt"));
    let bytes = read_bytes(&path)?;
    let mut functions = Vec::new();
    for statement in formula::statements(&read_text(&path)?) {
        let Some(m) = captures(formula::NAME, &statement).into_iter().next() else {
            continue;
        };
        let key = format!("oz.{}", m[1]);
        if !seen.insert(key.clone()) {
            problems.push(format!("{key} is defined more than once in the sources"));
        }
        let body = py::strip(&m[2]);
        let help = help_text(body).and_then(|t| parse(&t).signature);
        problems.extend(index_problems(
            &key,
            module,
            help.as_deref(),
            rows.get(&key),
        ));
        functions.push(function_json(&m[1], body, rows.get(&key)));
    }
    let source = format!("src/{module}.txt");
    Ok(json!({"module": module, "source": source, "sha256": hex(&bytes), "functions": functions}))
}

/// Build the reference from the module sources and the published index.
/// Returns the JSON document and the number of functions, or the problems
/// that stop the two from describing exactly the same library.
pub fn build(src: &Path, index: &Path) -> std::result::Result<(Value, usize), Vec<String>> {
    let mut problems = Vec::new();
    let rows = index_rows(index, &mut problems).map_err(|e| vec![e])?;
    let mut seen = BTreeSet::new();
    let mut modules = Vec::new();
    for module in formula::MODULES {
        modules
            .push(module_json(src, module, &rows, &mut seen, &mut problems).map_err(|e| vec![e])?);
    }
    for name in rows.keys().filter(|name| !seen.contains(*name)) {
        problems.push(format!("{name} is in the index but not in the sources"));
    }
    if problems.is_empty() {
        Ok((json!({"format": FORMAT, "modules": modules}), seen.len()))
    } else {
        Err(problems)
    }
}

pub fn command(args: &[String]) -> Output {
    let [src, index, output] = args else {
        return Output::stderr(
            "usage: ozzit function_reference <src dir> <functions.csv> <output.json>",
        );
    };
    match build(Path::new(src), Path::new(index)) {
        Ok((document, count)) => {
            let mut text = serde_json::to_string_pretty(&document).expect("JSON values serialise");
            text.push('\n');
            match std::fs::write(output, text) {
                Ok(()) => Output::ok(vec![format!(
                    "wrote {count} functions in {} modules to {output}",
                    formula::MODULES.len()
                )]),
                Err(e) => Output::failure(format!("FAIL: cannot write {output}: {e}")),
            }
        }
        Err(problems) => Output::report(
            problems.clone(),
            format!("FAIL: {} reference problem(s)", problems.len()),
            String::new(),
        ),
    }
}
