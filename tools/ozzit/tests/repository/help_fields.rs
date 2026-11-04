//! Every function help table carries the same WEBPAGE and VERSION rows.
//!
//! A function copied into a model takes only its own help with it, so that help
//! is the one place a reader can see where the function came from and how old it
//! is. Up to v3.5.0, 17 help tables had no VERSION row, 4 had no link, and 10
//! labelled the link WEBSITE where the rest say WEBPAGE. The module About tables
//! are a different layout, with their own REPOSITORY and WEBSITE rows, and are
//! not read here: they are assigned with `TRIM(TEXTSPLIT(...))`, not LAMBDA.
//!
//! VERSION is the date the function's formula last changed. Up to v3.5.0 nothing
//! checked that, and 29 functions changed after the date their help gave:
//! `Amortiseλ` gained `FinalPeriodEnd` and still read 20 Aug 2026. The formula
//! here is the LAMBDA without its comments, whitespace or help text, so a
//! help-only edit leaves VERSION alone. The gate compares it with `VERSION_BASE`,
//! which CI sets on a push to main to the commit main pointed at before it, or
//! else with HEAD's first parent, which in a pull request is the base it merges into.
use super::{modules, root};
use chrono::NaiveDate;
use indexmap::IndexMap;
use ozzit::common::{self, bounded};
use std::collections::{BTreeSet, HashMap};
use std::process::Command;

const REPOSITORY: &str = "https://github.com/ryanduguid/Ozzit";
const FUNCTION: &str = r"(?m)^([A-Za-z0-9_]*λ[A-Za-z0-9_]*)\s*=\s*LAMBDA\(";
const ROW: &str = r#"(?m)^\s*"([A-Z][A-Z ]*):\s*→\s*([^"]*?)\s*¶?""#;
const DATE: &str = r"[1-9]\d? (Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) 20\d\d";
/// A literal (group 1), a comment or a run of whitespace; only literals are kept.
const TOKEN: &str = r#"(?s)("(?:[^"]|"")*")|//[^\n]*|/\*.*?\*/|\s+"#;
/// The help: the Help binding's TEXTSPLIT up to its "→" and "¶" delimiters.
const HELP: &str = r#"(?s)\bHelp,\s*TRIM\(\s*TEXTSPLIT\(.*?"→",\s*"¶""#;
// The diagnostic companions take the same arguments as the function they check and
// return a table of findings; they have no help of their own.
const WITHOUT_HELP: [&str; 3] = ["AmortiseλDV", "CorkscrewλDV", "DepreciateλDV"];

type Functions = IndexMap<String, String>;

/// Each LAMBDA's text in one module, from its assignment to the next one.
fn split(text: &str, into: &mut Functions) {
    let starts: Vec<(usize, String)> = common::re(FUNCTION)
        .captures_iter(text)
        .map(|m| {
            let m = m.expect("regular expression execution");
            (m.get(0).unwrap().start(), m[1].to_owned())
        })
        .collect();
    for (i, (start, name)) in starts.iter().enumerate() {
        let end = starts.get(i + 1).map_or(text.len(), |(next, _)| *next);
        into.insert(name.clone(), text[*start..end].to_owned());
    }
}

/// Each LAMBDA's text in `src/`.
fn functions() -> Functions {
    let mut found = Functions::new();
    for (_, text) in modules() {
        split(&text, &mut found);
    }
    found
}

fn git(args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

/// Each LAMBDA's text in `src/` at a commit, or None when Git cannot show it.
fn committed_functions(revision: &str) -> Option<Functions> {
    let listing = git(&["ls-tree", "--name-only", revision, "src/"])?;
    let mut found = Functions::new();
    for name in ozzit::py::split_whitespace(&String::from_utf8(listing).unwrap()) {
        if name.ends_with(".txt") {
            let shown = git(&["show", &format!("{revision}:{name}")])?;
            split(&String::from_utf8(shown).unwrap(), &mut found);
        }
    }
    Some(found)
}

/// The labelled rows of a help table; a repeated label keeps its last value.
fn rows(body: &str) -> HashMap<String, String> {
    common::captures(ROW, body)
        .into_iter()
        .map(|row| (row[1].clone(), row[2].clone()))
        .collect()
}

/// The LAMBDA without comments, whitespace outside literals or help text.
///
/// The help becomes one marker whatever its rows say; every other literal,
/// message text and its spacing included, is part of the formula.
fn formula(body: &str) -> String {
    let body = common::sub(&bounded(HELP), body, "Help,¶");
    common::sub_fn(TOKEN, &body, |m| {
        m.get(1)
            .map_or(String::new(), |literal| literal.as_str().to_owned())
    })
}

/// `rows.get(label, "")`.
fn field<'a>(rows: &'a HashMap<String, String>, label: &str) -> &'a str {
    rows.get(label).map_or("", String::as_str)
}

fn version(body: &str) -> NaiveDate {
    let text = rows(body).remove("VERSION").expect("a VERSION row");
    NaiveDate::parse_from_str(&text, "%d %b %Y").unwrap_or_else(|e| panic!("VERSION {text:?}: {e}"))
}

/// Functions whose formula changed since base while VERSION stayed put.
///
/// A function changed again on the day its VERSION was set keeps that date, so
/// a VERSION within a day of today passes; the day allows for time zones.
fn unmoved(current: &Functions, base: &Functions, today: NaiveDate) -> Vec<String> {
    current
        .iter()
        .filter(|(name, body)| {
            base.get(*name).is_some_and(|before| {
                !WITHOUT_HELP.contains(&name.as_str())
                    && formula(body) != formula(before)
                    && version(body) <= version(before)
                    && (version(body) - today).num_days().abs() > 1
            })
        })
        .map(|(name, _)| name.clone())
        .collect()
}

/// The functions at the named base, at HEAD's first parent when none is named,
/// or None when that parent cannot be read.
fn base(named: &str) -> Result<Option<Functions>, String> {
    let revision = if named.is_empty() { "HEAD^1" } else { named };
    match committed_functions(revision) {
        None if named.is_empty() => Ok(None),
        Some(base) if !base.is_empty() => Ok(Some(base)),
        _ => Err(format!("no functions under src/ at {revision}")),
    }
}

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%d %b %Y").unwrap()
}

#[test]
fn the_library_was_read() {
    assert!(functions().len() >= 140);
}

#[test]
fn only_the_diagnostic_companions_have_no_help() {
    let without: BTreeSet<String> = functions()
        .into_iter()
        .filter(|(_, body)| !rows(body).contains_key("FUNCTION"))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        without,
        WITHOUT_HELP.map(String::from).into_iter().collect()
    );
}

#[test]
fn every_help_names_its_webpage_and_version() {
    for (name, body) in functions() {
        if WITHOUT_HELP.contains(&name.as_str()) {
            continue;
        }
        let rows = rows(&body);
        assert!(
            !rows.contains_key("WEBSITE"),
            "function={name}: label the link WEBPAGE, as every other help does"
        );
        assert!(
            field(&rows, "WEBPAGE").starts_with("https://"),
            "function={name}: {:?}",
            rows.get("WEBPAGE")
        );
        let version = field(&rows, "VERSION");
        assert!(
            common::matches(&format!(r"^{DATE}\n?\z"), version),
            "function={name}: VERSION {version:?}"
        );
    }
}

#[test]
fn every_webpage_in_this_repository_is_a_file_on_main() {
    // A WEBPAGE inside the repository names the page that explains the function; the
    // repository root is kept only where no page does. A renamed page breaks the link.
    let on_main = format!("{REPOSITORY}/blob/main/");
    for (name, body) in functions() {
        let rows = rows(&body);
        let link = field(&rows, "WEBPAGE");
        if link == REPOSITORY || !link.starts_with(&format!("{REPOSITORY}/")) {
            continue;
        }
        let page = link
            .strip_prefix(&on_main)
            .unwrap_or_else(|| panic!("function={name}: {link}"));
        assert!(root().join(page).is_file(), "function={name}: {link}");
    }
}

#[test]
fn a_changed_formula_moves_its_version() {
    let named = std::env::var("VERSION_BASE").unwrap_or_default();
    let Some(base) = base(&named).unwrap_or_else(|e| panic!("{e}")) else {
        // CI checks out the full history.
        eprintln!("skipped: needs HEAD's first parent; a shallow clone or archive has none");
        return;
    };
    assert_eq!(
        unmoved(&functions(), &base, today()),
        Vec::<String>::new(),
        "the formula changed: set VERSION to the date of the change"
    );
}

/// The version gate run in a child test process with `VERSION_BASE` set, so the gate's
/// own reading of the variable is under test; the output joins stdout and stderr.
fn gate_with_base(named: &str) -> (bool, String) {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "help_fields::a_changed_formula_moves_its_version",
            "--test-threads=1",
        ])
        .env("VERSION_BASE", named)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

#[test]
fn a_named_base_is_read_and_one_that_cannot_be_read_fails() {
    let (passed, output) = gate_with_base("HEAD");
    assert!(passed && output.contains("1 passed"), "{output}");
    let (passed, output) = gate_with_base("no-such-revision");
    assert!(!passed && output.contains("1 failed"), "{output}");
    assert!(
        output.contains("no functions under src/ at no-such-revision"),
        "{output}"
    );
}

/// A one-function module in the shape the gate reads.
fn body(version: &str, rows: &str, code: &str) -> String {
    format!(
        "Fλ = LAMBDA([X], LET(\n    Help, TRIM(TEXTSPLIT(\"FUNCTION:→Fλ(X)¶\" &\n        \
         \"VERSION:   →{version}¶\" &\n{rows}        \"→\", \"¶\")),\n    \
         // the arithmetic\n    {code}));\n"
    )
}

fn one(name: &str, body: String) -> Functions {
    Functions::from([(name.to_owned(), body)])
}

#[test]
fn the_version_gate_reads_formulas_not_help() {
    let base = one("Fλ", body("1 Sep 2026", "", "X + 1"));
    let today = date("3 Nov 2026");
    let cases = [
        (
            "formula changed, VERSION kept",
            body("1 Sep 2026", "", "X + 2"),
            vec!["Fλ"],
        ),
        (
            "formula changed, VERSION moved",
            body("2 Nov 2026", "", "X + 2"),
            vec![],
        ),
        (
            "formula changed on the VERSION's day",
            body("4 Nov 2026", "", "X+2"),
            vec![],
        ),
        (
            "help row added",
            body("1 Sep 2026", "        \"NOTE!→More.¶\" &\n", "X + 1"),
            vec![],
        ),
        (
            "comment and spacing",
            body("1 Sep 2026", "", "X  +\n        1").replace("// the arithmetic", "/* add one */"),
            vec![],
        ),
        (
            "message text changed",
            body("1 Sep 2026", "", r#"IF(X, "Bad.", 1)"#),
            vec!["Fλ"],
        ),
    ];
    for (case, current, expected) in cases {
        assert_eq!(
            unmoved(&one("Fλ", current), &base, today),
            expected,
            "case={case}"
        );
    }
    let mut both = base.clone();
    both.insert("Gλ".into(), base["Fλ"].clone());
    assert!(unmoved(&both, &base, today).is_empty());
    assert!(committed_functions("no-such-revision").is_none());
    // Help separators in a message outside the help, and the spacing inside any
    // literal, are formula text.
    for (before, after) in [(r#""A→B¶""#, r#""A→C¶""#), (r#""A B""#, r#""AB""#)] {
        let message = one("Fλ", body("1 Sep 2026", "", &format!("IF(X, {before}, 1)")));
        let changed = one("Fλ", body("1 Sep 2026", "", &format!("IF(X, {after}, 1)")));
        assert_eq!(unmoved(&changed, &message, today), ["Fλ"], "{after}");
    }
}
