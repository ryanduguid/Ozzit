//! The function-adding passes that compile through `compile_sources`:
//! working-capital days, rate and date helpers, and ATO benchmark and rate lookup.
//!
//! Each adds functions and About rows to `src/`, the defined names and the About
//! tables to the workbook, and rows to functions.csv, in one recoverable step.
//! A second run reports "already applied"; a store holding part of a change fails.
use super::*;
use crate::compiler;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
    /// Blocks appended, keeping whether the module ended in a newline.
    AppendKeepEnding,
    /// Blocks appended after a guaranteed newline, ending in one.
    Append,
    /// Blocks inserted before the first block an earlier pass appended.
    BeforeFollower,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IndexRule {
    /// A named index must exist, and applying needs one.
    Required,
    /// A missing index is "no index"; applying notes it and carries on.
    Optional,
}

struct Spec {
    stem: &'static str,
    data: &'static str,
    what: &'static str,
    rows: &'static str,
    layout: Layout,
    index: IndexRule,
    /// Always list the index among the changes once applied.
    always_index: bool,
}

const SPECS: [Spec; 3] = [
    Spec {
        stem: "working_capital_days",
        data: include_str!("../data/postbuild/working_capital_days.json"),
        what: "working-capital days functions",
        rows: "working-capital",
        layout: Layout::AppendKeepEnding,
        index: IndexRule::Required,
        always_index: false,
    },
    Spec {
        stem: "rate_date_helpers",
        data: include_str!("../data/postbuild/rate_date_helpers.json"),
        what: "rate and date helpers",
        rows: "helper",
        layout: Layout::Append,
        index: IndexRule::Optional,
        always_index: false,
    },
    Spec {
        stem: "ato_benchmarks_rate_lookup",
        data: include_str!("../data/postbuild/ato_benchmarks_rate_lookup.json"),
        what: "benchmark and rate functions",
        rows: "benchmark and rate",
        layout: Layout::BeforeFollower,
        index: IndexRule::Required,
        always_index: true,
    },
];

struct Function {
    module: String,
    name: String,
    about: String,
    block: String,
}

struct About {
    anchor: String,
    heading: Option<String>,
    width: usize,
    follower: Option<String>,
}

struct Tables {
    functions: Vec<Function>,
    about: Vec<(String, About)>,
}

fn tables(spec: &Spec) -> Tables {
    let data = data(spec.data);
    let functions = data["functions"]
        .as_array()
        .expect("functions")
        .iter()
        .map(|f| {
            let f = strings(f);
            Function {
                module: f[0].clone(),
                name: f[1].clone(),
                about: f[2].clone(),
                block: f[3].clone(),
            }
        })
        .collect();
    let about = data["about"]
        .as_object()
        .expect("about")
        .iter()
        .map(|(module, v)| {
            (
                module.clone(),
                About {
                    anchor: v[0].as_str().expect("anchor").into(),
                    heading: v[1].as_str().map(String::from),
                    width: v[2].as_u64().expect("width") as usize,
                    follower: v[3].as_str().map(String::from),
                },
            )
        })
        .collect();
    Tables { functions, about }
}

impl Tables {
    fn qualified(&self) -> Vec<String> {
        self.functions
            .iter()
            .map(|f| format!("oz.{}", f.name))
            .collect()
    }
    fn of(&self, module: &str) -> Vec<&Function> {
        self.functions
            .iter()
            .filter(|f| f.module == module)
            .collect()
    }
    fn about_row(&self, module: &str, f: &Function) -> String {
        let width = lookup_about(&self.about, module).width;
        format!("\"{}→{}¶\"", pad(&f.name, width), f.about)
    }
}

fn lookup_about<'a>(about: &'a [(String, About)], module: &str) -> &'a About {
    &about.iter().find(|(m, _)| m == module).expect("module").1
}

fn add_to_src(spec: &Spec, t: &Tables, module: &str, text: &str) -> Result<String> {
    let about = lookup_about(&t.about, module);
    let functions = t.of(module);
    let rows = functions
        .iter()
        .map(|f| t.about_row(module, f))
        .collect::<Vec<_>>();
    let place = format!("src/{module}.txt");
    let text = insert_about_rows(text, &place, &about.anchor, &rows, about.heading.as_deref())?;
    let joined = functions
        .iter()
        .map(|f| f.block.as_str())
        .collect::<Vec<_>>()
        .join("\n\n\n");
    Ok(match spec.layout {
        Layout::AppendKeepEnding => format!(
            "{}\n\n\n{joined}{}",
            text.trim_end_matches('\n'),
            if text.ends_with('\n') { "\n" } else { "" }
        ),
        Layout::Append => {
            let text = if text.ends_with('\n') {
                text
            } else {
                format!("{text}\n")
            };
            format!("{text}\n\n\n{joined}\n")
        }
        Layout::BeforeFollower => {
            let follower = about.follower.as_deref().expect("follower");
            let hits = count(&text, follower);
            if hits != 1 {
                let name = follower.split_whitespace().last().unwrap_or_default();
                return Err(format!(
                    "{place}: {name} block found {hits} times, expected 1"
                ));
            }
            let start = text.find(follower).expect("one follower");
            let before = text[..start].trim_end_matches('\n');
            let gap = &text[before.len()..start];
            text.replacen(follower, &format!("{joined}{gap}{follower}"), 1)
        }
    })
}

type Sources = Vec<(String, py::PyPath, String)>;
/// Each module the pass extends: its name, its path and its current text.
fn read_sources(t: &Tables, src: &py::PyPath) -> Result<Sources> {
    let mut texts = Vec::new();
    for (module, _) in &t.about {
        let path = src.join(&format!("{module}.txt"));
        if !path.fs().is_file() {
            return Err(format!("src: missing {path}"));
        }
        texts.push((module.clone(), path.clone(), read_text(&path.fs())?));
    }
    Ok(texts)
}

/// Whether the stores already hold the pass; an error when they disagree.
fn applied(spec: &Spec, texts: &Sources, book: &str, index: Option<&py::PyPath>) -> Result<bool> {
    let t = tables(spec);
    let qualified = t.qualified();
    let mut states: Vec<(String, &str)> = Vec::new();
    for (module, _, text) in texts {
        let rows = t
            .of(module)
            .iter()
            .map(|f| (f.name.clone(), t.about_row(module, f)))
            .collect::<Vec<_>>();
        states.push((
            module.clone(),
            source_state(&format!("src/{module}.txt"), text, &rows)?,
        ));
    }
    states.push((
        "workbook".into(),
        workbook_state(book, &qualified, None, "defined names")?,
    ));
    if let (Some(path), Some(state)) = (index, index_state(index, &qualified, spec.rows)?) {
        states.push((path.name().to_string(), state));
    }
    if states.iter().any(|(_, s)| *s != states[0].1) {
        let detail = states
            .iter()
            .map(|(store, state)| format!("{store} {state}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "the stores disagree ({detail}); the views must not be applied separately"
        ));
    }
    Ok(states
        .iter()
        .any(|(store, s)| store == "workbook" && *s == "applied"))
}

/// Puts back every store the failed pass may have written.
fn restore(texts: &Sources, index: Option<&py::PyPath>, index_before: Option<&[u8]>) -> Result<()> {
    for (_, path, text) in texts {
        write_text(&path.fs(), text)?;
    }
    if let (Some(path), Some(before)) = (index, index_before) {
        write_bytes(&path.fs(), before)?;
    }
    Ok(())
}

/// Regenerates the index when there is one; true when it was rewritten.
fn refresh_index(
    index: Option<&py::PyPath>,
    given: Option<&py::PyPath>,
    compiled: &[compiler::Compiled],
    book: &str,
    notes: &mut Vec<String>,
) -> Result<bool> {
    if let Some(path) = index {
        let modules = compiled
            .iter()
            .map(|c| (c.name.clone(), c.module.clone()))
            .collect();
        if let Some(text) = compiler::render_index(&path.fs(), book, &modules)? {
            write_bytes(&path.fs(), text.as_bytes())?;
            return Ok(true);
        }
    } else if let Some(path) = given {
        notes.push(format!("note: {path} not found, index not updated"));
    }
    Ok(false)
}

fn run(
    spec: &Spec,
    workbook: &Path,
    src: &py::PyPath,
    index: Option<&py::PyPath>,
    notes: &mut Vec<String>,
) -> Result<Vec<String>> {
    let t = tables(spec);
    let qualified = t.qualified();
    let texts = read_sources(&t, src)?;
    let mut parts = read_parts(workbook)?;
    let book = read_book(&parts)?;
    let given = index;
    let index = match spec.index {
        IndexRule::Optional => index.filter(|i| i.fs().is_file()),
        IndexRule::Required => index,
    };
    if applied(spec, &texts, &book, index)? {
        return Ok(vec![]);
    }
    if spec.index == IndexRule::Required && index.is_none() {
        return Err(format!(
            "functions.csv is required to apply the {}",
            spec.what
        ));
    }

    // src/ first (the compiler reads it from disk), then the index, then the
    // workbook, which marks the pass applied. A failure restores every store.
    let index_before = index.map(|i| read_bytes(&i.fs())).transpose()?;
    let mut changed = Vec::new();
    let mut indexed = false;
    let mut apply = || -> Result<()> {
        for (module, path, text) in &texts {
            write_text(&path.fs(), &add_to_src(spec, &t, module, text)?)?;
            changed.push(format!("src/{module}.txt"));
        }
        let compiled = compiler::compile_sources(&src.fs())?;
        let added = compiled
            .iter()
            .filter(|c| qualified.contains(&c.name))
            .cloned()
            .collect::<Vec<_>>();
        let book = compiler::insert_names(&book, &added)?;
        let (book, _) = compiler::apply(&book, &compiled)?;
        indexed = refresh_index(index, given, &compiled, &book, notes)?;
        parts.insert(BOOK.into(), book.into_bytes());
        write_deterministic(workbook, &parts)
    };
    if let Err(e) = apply() {
        restore(&texts, index, index_before.as_deref())?;
        return Err(e);
    }
    changed.push("workbook".into());
    if let Some(path) = index
        && (indexed || spec.always_index)
    {
        changed.push(path.name().to_string());
    }
    Ok(changed)
}

fn command(spec: &Spec, args: &[String]) -> Output {
    if args.len() > 3 {
        return Output::stderr(format!(
            "FAIL: usage: ozzit {} [workbook] [src dir] [functions.csv]",
            spec.stem
        ));
    }
    let workbook = py::PyPath::new(args.first().map_or("ozzit.xlsx", String::as_str));
    let src = py::PyPath::new(args.get(1).map_or("src", String::as_str));
    let default = workbook.parent().join("functions.csv");
    let index = match (args.get(2), spec.index) {
        (Some(path), _) => Some(py::PyPath::new(path)),
        (None, IndexRule::Optional) => Some(default),
        (None, IndexRule::Required) => default.fs().is_file().then_some(default),
    };
    if !workbook.fs().is_file() {
        return Output::stderr(format!("FAIL: no such workbook: {workbook}"));
    }
    let mut notes = Vec::new();
    match run(spec, &workbook.fs(), &src, index.as_ref(), &mut notes) {
        Err(e) => {
            let mut output = Output::stderr(format!("FAIL: {e}"));
            output.out = notes;
            output
        }
        Ok(changed) => {
            let mut out = notes;
            out.push(if changed.is_empty() {
                format!("{} already applied; no changes", spec.what)
            } else {
                format!("added the {} to: {}", spec.what, changed.join(", "))
            });
            out.push(format!("OK: {workbook}"));
            Output::ok(out)
        }
    }
}

pub fn working_capital_days(args: &[String]) -> Output {
    command(&SPECS[0], args)
}
pub fn rate_date_helpers(args: &[String]) -> Output {
    command(&SPECS[1], args)
}
pub fn ato_benchmarks_rate_lookup(args: &[String]) -> Output {
    command(&SPECS[2], args)
}
