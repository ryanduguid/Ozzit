//! The documentation, template and repository checks that ran as Python
//! unittest modules in `tools/tests`. Each module is named after the file it
//! replaced and keeps that file's assertions; a module's own doc comment says
//! what it holds.
//!
//! Patterns go through `ozzit::common::re`, which gives `\s`, `\w` and `\d`
//! Python's meaning by expanding each into a class. A class that would hold
//! one of them, such as Python's `[\w.]`, is written as an alternation.

mod afe_store;
mod benchmark_arithmetic;
mod cash_flow_template;
mod cross_checks;
mod day_counts;
mod depreciation_comparison;
mod excel_eval_formulas;
mod help_copy_paste;
mod help_fields;
mod help_matches_code;
mod modelling_conventions;
mod practice_template;
mod ratio_definitions;
mod repository_policy;
mod row_defaults_and_life_guards;
mod verifier_boundaries;
mod workbook_presentation;
mod xml;

use ozzit::{archive::Parts, common};
use std::path::{Path, PathBuf};

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `Path.read_text(encoding="utf-8")` of a repository file.
pub fn read(relative: &str) -> String {
    common::read_text(&root().join(relative)).unwrap_or_else(|e| panic!("{e}"))
}

/// The file's bytes decoded as UTF-8, with no newline translation.
pub fn read_raw(relative: &str) -> String {
    common::read_raw(&root().join(relative)).unwrap_or_else(|e| panic!("{e}"))
}

/// The file read as UTF-8 with any byte order mark removed.
pub fn read_sig(relative: &str) -> String {
    common::read_sig(&root().join(relative)).unwrap_or_else(|e| panic!("{e}"))
}

pub fn sha256(relative: &str) -> String {
    ozzit::native_evidence::sha256(&root().join(relative)).unwrap_or_else(|e| panic!("{e}"))
}

/// Every member of a repository workbook, in archive order.
pub fn parts(relative: &str) -> Parts {
    ozzit::archive::read_parts(&root().join(relative)).unwrap_or_else(|e| panic!("{e}"))
}

/// Every member name, repeats included, as `ZipFile.namelist()` gives them.
pub fn names(relative: &str) -> Vec<String> {
    ozzit::archive::read_inventory(&root().join(relative))
        .unwrap_or_else(|e| panic!("{e}"))
        .1
}

/// A member decoded as UTF-8.
pub fn part(parts: &Parts, name: &str) -> String {
    ozzit::archive::text(parts, name).unwrap_or_else(|e| panic!("{e}"))
}

/// The `src/*.txt` modules, sorted by path, as (stem, text).
pub fn modules() -> Vec<(String, String)> {
    common::txt_files(&root().join("src"))
        .unwrap_or_else(|e| panic!("{e}"))
        .into_iter()
        .map(|path| {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            (
                stem,
                common::read_text(&path).unwrap_or_else(|e| panic!("{e}")),
            )
        })
        .collect()
}

pub fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}"))
}

/// `" ".join(text.split())`.
pub fn flat(text: &str) -> String {
    ozzit::py::split_whitespace(text).join(" ")
}

/// `re.sub(r"\s+", " ", text)`.
pub fn spaced(text: &str) -> String {
    common::sub(r"\s+", text, " ")
}

/// `re.search(pattern, text)`: the first match's groups, an unmatched one as "".
pub fn search(pattern: &str, text: &str) -> Option<Vec<String>> {
    common::re(pattern)
        .captures(text)
        .expect("regular expression execution")
        .map(|m| {
            (0..m.len())
                .map(|i| m.get(i).map_or("", |g| g.as_str()).to_owned())
                .collect()
        })
}

/// `re.findall(pattern, text)` for a pattern with one group, or none.
pub fn findall(pattern: &str, text: &str) -> Vec<String> {
    common::captures(pattern, text)
        .into_iter()
        .map(|mut groups| groups.pop().expect("a match has its whole text"))
        .collect()
}

/// `text.split(marker)[n]`.
pub fn piece<'a>(text: &'a str, marker: &str, n: usize) -> &'a str {
    text.split(marker)
        .nth(n)
        .unwrap_or_else(|| panic!("{marker:?} does not split the text into {} parts", n + 1))
}

/// `text.index(needle, start)`.
pub fn index(text: &str, needle: &str, start: usize) -> usize {
    text[start..]
        .find(needle)
        .map(|i| i + start)
        .unwrap_or_else(|| panic!("{needle:?} not found"))
}

/// `assertAlmostEqual(first, second, places=places)`.
pub fn almost_equal(first: f64, second: f64, places: i32) -> bool {
    first == second || ((first - second).abs() * 10f64.powi(places)).round_ties_even() == 0.0
}

/// `assertAlmostEqual(first, second, delta=delta)`.
pub fn within(first: f64, second: f64, delta: f64) -> bool {
    first == second || (first - second).abs() <= delta
}

/// The text of a workbook's defined name, or the failure the Python helper raised.
pub fn defined_name(book: &str, name: &str) -> Result<String, String> {
    search(
        &format!(
            r#"(?s)<definedName name="{}"[^>]*>(.*?)</definedName>"#,
            fancy_regex::escape(name)
        ),
        book,
    )
    .map(|groups| groups[1].clone())
    .ok_or_else(|| format!("{name} missing from workbook.xml"))
}
