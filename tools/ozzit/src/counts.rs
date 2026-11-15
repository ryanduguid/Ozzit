// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! Every statement of the library's size must agree with the index it describes.
//!
//! The register lists each place that states a count, by a pattern anchored on its
//! wording, and what the statement describes ([`Basis`]). Between releases `main`
//! can hold more functions than the newest release, and a release candidate still
//! presents the release before it, so each statement is compared with its own
//! index: `main`'s `functions.csv`, or `functions.csv` at a release tag. True
//! statements about older releases therefore pass, and a wrong one fails.
//!
//! A registered statement that matches nowhere fails, so rewording one cannot drop
//! it from the gate unnoticed. A space in a pattern matches any run of whitespace,
//! so reflowing a paragraph keeps its statement; a pattern that needs one literal
//! space, as inside a class, writes `\x20`.
use crate::{
    cache::{Dump, run_process},
    common::*,
    cover, py,
};
use std::{collections::BTreeMap, path::Path, time::Duration};

pub const REPOSITORY: &str = "ryanduguid/Ozzit";
const TIMEOUT: Duration = Duration::from_secs(30);
const REGISTER: &str = "tools/ozzit/src/counts.rs";
const VERSION: &str = r"v(?P<version>\d+\.\d+\.\d+)";
const BANNER: &str = r#"<title id="t">Ozzit: (?P<functions>\d+) native Excel LAMBDAs\."#;
const ABOUT: &str = r"^(?P<functions>\d+) Excel LAMBDA functions and (?P<help>\w+) help tables";
/// Help-table counts written as words, as 'five help tables'.
const WORDS: [&str; 10] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

/// What a statement describes, and so which index it is compared with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    /// The library on this branch: `functions.csv` in the working tree.
    Main,
    /// The newest published release: the newest `vX.Y.Z` tag.
    Release,
    /// The release its `version` group names: that tag, or the working tree
    /// for the changelog's newest cut before it is tagged.
    Named,
    /// As [`Basis::Named`], and the version named must be the newest tag.
    Latest,
}

/// File, what the statement is, its basis and its pattern. The named groups
/// `functions`, `help` and `names` capture the counts; `version` the release.
pub const STATEMENTS: [(&str, &str, Basis, &str); 15] = [
    (
        "README.md",
        "banner alt text",
        Basis::Release,
        r#"alt="Ozzit\. (?P<functions>\d+) native Excel LAMBDAs\."#,
    ),
    (
        "README.md",
        "introduction",
        Basis::Latest,
        r"(?P<functions>\d+) functions and (?P<help>\w+) named help tables in release v(?P<version>\d+\.\d+\.\d+)\.",
    ),
    (
        "README.md",
        "release bullet",
        Basis::Named,
        r"\*\*\[?v(?P<version>\d+\.\d+\.\d+)\]?[^*]*\*\*, (?:published|cut dated) [^.]*?, with (?P<functions>\d+) functions",
    ),
    (
        "README.md",
        "'does not need all N functions' line",
        Basis::Release,
        r"does not need all (?P<functions>\d+) functions",
    ),
    (
        "README.md",
        "oz.txt name count",
        Basis::Latest,
        r"that file is the v(?P<version>\d+\.\d+\.\d+) library as one module, and its (?P<names>\d+) names",
    ),
    (
        "AGENTS.md",
        "library size",
        Basis::Main,
        r"The library has (?P<functions>\d+) native Excel LAMBDA functions and (?P<help>\w+) named help tables",
    ),
    (
        "llms.txt",
        "release count",
        Basis::Named,
        r"v(?P<version>\d+\.\d+\.\d+)(?: \(published [^)]*\))?,? (?:cut on [^,]+, )?(?:ships|has) (?P<functions>\d+) functions(?: and (?P<help>\w+) help tables)?",
    ),
    (
        "docs/between-spreadsheets.md",
        "library size",
        Basis::Release,
        r"authority for its (?P<functions>\d+) LAMBDAs and (?P<help>\w+) help tables",
    ),
    (
        "docs/practice-improvements.md",
        "existing library",
        Basis::Release,
        r"reuse the (?P<functions>\d+)-function library",
    ),
    (
        "docs/modelling-conventions.md",
        "library size",
        Basis::Main,
        r"Ozzit is a library of (?P<functions>\d+) LAMBDA functions and (?P<help>\w+) help tables",
    ),
    (
        "docs/function-guide.md",
        "index size",
        Basis::Main,
        r"indexes all (?P<names>\d+) names",
    ),
    (".github/assets/banner.svg", "title", Basis::Release, BANNER),
    (
        ".github/assets/banner-light.svg",
        "title",
        Basis::Release,
        BANNER,
    ),
    (
        ".github/assets/banner-phone.svg",
        "title",
        Basis::Release,
        BANNER,
    ),
    (
        ".github/assets/banner-phone-light.svg",
        "title",
        Basis::Release,
        BANNER,
    ),
];

/// The release tags and their indexes.
pub trait Releases {
    /// The `vX.Y.Z` tags, or why they could not be listed.
    fn tags(&self) -> Result<Vec<String>>;
    /// `functions.csv` at tag `v{version}`.
    fn index(&self, version: &str) -> Result<String>;
}

/// The release tags of the Git checkout at a path.
pub struct Git<'a>(pub &'a Path);
impl Git<'_> {
    fn run(&self, args: &[&str]) -> Result<String> {
        let owned = args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
        let command = format!("git {}", args.join(" "));
        match run_process("git", &owned, Some(self.0), TIMEOUT)? {
            Dump::Exited {
                code: 0, stdout, ..
            } => Ok(stdout),
            Dump::Exited { code, stderr, .. } => Err(format!(
                "{command} exited {code}: {}",
                stderr.lines().next().unwrap_or_default()
            )),
            Dump::NotFound => Err("git is not installed".into()),
            Dump::TimedOut => Err(format!("{command} took longer than 30 seconds")),
        }
    }
}
impl Releases for Git<'_> {
    fn tags(&self) -> Result<Vec<String>> {
        Ok(self
            .run(&["tag", "--list", "v*"])?
            .lines()
            .map(str::to_owned)
            .collect())
    }
    fn index(&self, version: &str) -> Result<String> {
        self.run(&["show", &format!("v{version}:functions.csv")])
    }
}

/// The repository's About description, or why it was not read.
#[derive(Debug)]
pub enum About {
    /// Not read, as in an offline run or a fork: reported, never a failure.
    Unchecked(String),
    /// Not read where it must be, in the upstream repository's CI: a failure.
    Unreadable(String),
    Text(String),
}

/// Whether a run is in GitHub Actions, where missing evidence fails the gate.
fn strict(var: &impl Fn(&str) -> Option<String>) -> bool {
    var("GITHUB_ACTIONS").as_deref() == Some("true")
}

/// Read the About description by REST through `gh`, only with a token and only
/// in the upstream repository, so an offline run or a fork never fails on it.
pub fn read_about(
    var: impl Fn(&str) -> Option<String>,
    fetch: impl FnOnce() -> Result<Dump>,
) -> About {
    let set = |name: &str| var(name).filter(|value| !value.is_empty());
    if set("GH_TOKEN").or_else(|| set("GITHUB_TOKEN")).is_none() {
        return About::Unchecked("no GH_TOKEN or GITHUB_TOKEN, as in an offline run".into());
    }
    let repository = set("GITHUB_REPOSITORY").unwrap_or_else(|| REPOSITORY.into());
    if repository != REPOSITORY {
        return About::Unchecked(format!("this run is in {repository}, not {REPOSITORY}"));
    }
    let failed = match fetch() {
        Ok(Dump::Exited {
            code: 0, stdout, ..
        }) => return About::Text(py::strip(&stdout).into()),
        Ok(Dump::Exited { code, stderr, .. }) => format!(
            "gh api exited {code}: {}",
            stderr.lines().next().unwrap_or_default()
        ),
        Ok(Dump::NotFound) => "gh is not installed".into(),
        Ok(Dump::TimedOut) => "gh api took longer than 30 seconds".into(),
        Err(error) => format!("gh could not run: {error}"),
    };
    if strict(&var) {
        About::Unreadable(failed)
    } else {
        About::Unchecked(failed)
    }
}

#[derive(Clone, Copy)]
struct Inventory {
    functions: usize,
    help: usize,
}

struct Checker<'a> {
    releases: &'a dyn Releases,
    main: Inventory,
    /// The changelog's newest cut, which `main` describes until it is tagged.
    cut: String,
    tags: Vec<String>,
    /// The newest tag, or None when release statements cannot be checked.
    latest: Option<String>,
    indexes: BTreeMap<String, Result<Inventory>>,
    checked: usize,
    failures: Vec<String>,
}

impl Checker<'_> {
    /// The index of release `version`, read once.
    fn release(&mut self, version: &str) -> Result<Inventory> {
        if let Some(found) = self.indexes.get(version) {
            return found.clone();
        }
        let found = if self.tags.iter().any(|tag| tag == version) {
            self.releases.index(version).and_then(|text| {
                let csv = Csv::parse(text.strip_prefix('\u{feff}').unwrap_or(&text))?;
                let (functions, help) =
                    cover::inventory_of(&csv, &format!("v{version}:functions.csv"))?;
                Ok(Inventory { functions, help })
            })
        } else if version == self.cut {
            Ok(self.main)
        } else {
            Err(format!(
                "v{version} has no release tag and is not the changelog's newest cut, v{}",
                self.cut
            ))
        };
        self.indexes.insert(version.to_owned(), found.clone());
        found
    }

    /// Compare every match of `pattern` in `text`. `numbered` places a match by
    /// its line in `place`.
    fn statement(
        &mut self,
        place: &str,
        numbered: bool,
        what: &str,
        basis: Basis,
        text: &str,
        pattern: &str,
    ) {
        let pattern = pattern.replace(' ', r"\s+");
        let mut found = 0;
        for matched in re(&pattern).captures_iter(text) {
            let matched = match matched {
                Ok(matched) => matched,
                Err(error) => {
                    self.failures.push(format!(
                        "FAIL: {place}: the {what} could not be matched: {error}"
                    ));
                    return;
                }
            };
            found += 1;
            let start = matched.get(0).map_or(0, |m| m.start());
            let at = if numbered {
                format!("{place}:{}", text[..start].matches('\n').count() + 1)
            } else {
                place.to_owned()
            };
            let named = matched.name("version").map(|m| m.as_str().to_owned());
            let (source, index) = match (basis, named, self.latest.clone()) {
                (Basis::Main, _, _) => ("functions.csv".to_owned(), Ok(self.main)),
                (_, _, None) => continue,
                (Basis::Release, _, Some(latest)) => (format!("v{latest}"), self.release(&latest)),
                (_, None, _) => unreachable!("a {basis:?} pattern captures its version"),
                (_, Some(version), Some(latest)) => {
                    if basis == Basis::Latest && version != latest {
                        self.failures.push(format!(
                            "FAIL: {at}: the {what} names v{version}; the newest release tag is v{latest}"
                        ));
                    }
                    (format!("v{version}"), self.release(&version))
                }
            };
            let index = match index {
                Ok(index) => index,
                Err(error) => {
                    self.failures
                        .push(format!("FAIL: {at}: the {what}: {error}"));
                    continue;
                }
            };
            self.checked += 1;
            for (group, noun, count) in [
                ("functions", "functions", index.functions),
                ("help", "help tables", index.help),
                ("names", "names", index.functions + index.help),
            ] {
                let Some(stated) = matched.name(group).map(|m| m.as_str()) else {
                    continue;
                };
                let value = stated
                    .parse::<usize>()
                    .ok()
                    .or_else(|| WORDS.iter().position(|w| w.eq_ignore_ascii_case(stated)));
                match value {
                    Some(value) if value == count => {}
                    Some(value) => self.failures.push(format!(
                        "FAIL: {at}: the {what} says {value} {noun}; {source} has {count} {noun}"
                    )),
                    None => self.failures.push(format!(
                        "FAIL: {at}: the {what} gives {} {noun}, not a count",
                        py::repr_str(stated)
                    )),
                }
            }
        }
        if found == 0 {
            self.failures.push(format!(
                "FAIL: {place}: the {what} is missing or reworded; restore it or update the register in {REGISTER}"
            ));
        }
    }
}

/// Check every registered statement under `root`, and the About description if
/// read. `strict` (GitHub Actions) fails where release tags are missing.
pub fn check(root: &Path, about: &About, releases: &dyn Releases, strict: bool) -> Output {
    let run = || -> Result<Output> {
        let (functions, help) = cover::inventory(&root.join("functions.csv"))?;
        let (cut, _) = cover::current_cut(&root.join("CHANGELOG.md"))?;
        let mut out = Vec::new();
        let mut failures = Vec::new();
        let tags = match releases.tags() {
            Ok(tags) => tags
                .iter()
                .filter_map(|tag| {
                    let parts = captures(r"^v([0-9]+)\.([0-9]+)\.([0-9]+)$", tag).pop()?;
                    let order = parts[1..]
                        .iter()
                        .map(|part| part.parse::<u64>().ok())
                        .collect::<Option<Vec<_>>>()?;
                    Some((order, tag[1..].to_owned()))
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                let line = format!("the release tags could not be read: {error}");
                if strict {
                    failures.push(format!("FAIL: {line}"));
                } else {
                    out.push(format!("SKIP: release statements not checked: {line}"));
                }
                Vec::new()
            }
        };
        let latest = tags.iter().max().map(|(_, version)| version.clone());
        if latest.is_none() && out.is_empty() && failures.is_empty() {
            let line = "this checkout has no vX.Y.Z release tags";
            if strict {
                failures.push(format!(
                    "FAIL: {line}; fetch them, as actions/checkout does with fetch-depth: 0"
                ));
            } else {
                out.push(format!("SKIP: release statements not checked: {line}"));
            }
        }
        let mut checker = Checker {
            releases,
            main: Inventory { functions, help },
            cut,
            tags: tags.into_iter().map(|(_, version)| version).collect(),
            latest,
            indexes: BTreeMap::new(),
            checked: 0,
            failures,
        };
        for (file, what, basis, pattern) in STATEMENTS {
            let text = read_text(&root.join(file))?;
            checker.statement(file, true, what, basis, &text, pattern);
        }
        match about {
            About::Unchecked(reason) => {
                out.push(format!("SKIP: About description not read: {reason}"));
            }
            About::Unreadable(reason) => checker.failures.push(format!(
                "FAIL: About description could not be read: {reason}"
            )),
            About::Text(text) => checker.statement(
                "About description",
                false,
                "repository description",
                Basis::Release,
                text,
                ABOUT,
            ),
        }
        let code = u8::from(!checker.failures.is_empty());
        if checker.failures.is_empty() {
            let release = match checker.latest.clone() {
                Some(latest) => checker.release(&latest).map_or_else(
                    |_| String::new(),
                    |index| {
                        format!(
                            "; v{latest} has {} functions and {} help tables",
                            index.functions, index.help
                        )
                    },
                ),
                None => String::new(),
            };
            out.push(format!(
                "OK: {} statements agree: main has {functions} functions and {help} help tables{release}",
                checker.checked
            ));
        }
        out.extend(checker.failures);
        Ok(Output {
            code,
            out,
            err: vec![],
        })
    };
    run().unwrap_or_else(|e| Output::failure(format!("FAIL: {e}")))
}

pub fn verify(args: &[String]) -> Output {
    if args.len() > 1 {
        return Output::stderr("FAIL: usage: ozzit verify_counts [repository-root]");
    }
    let root = py::PyPath::new(args.first().map_or(".", String::as_str)).fs();
    let var = |name: &str| std::env::var(name).ok();
    let about = read_about(var, || {
        let args = [
            "api",
            &format!("repos/{REPOSITORY}"),
            "--jq",
            ".description",
        ]
        .map(|a| a.to_owned());
        run_process("gh", &args, None, TIMEOUT)
    });
    check(&root, &about, &Git(&root), strict(&var))
}
