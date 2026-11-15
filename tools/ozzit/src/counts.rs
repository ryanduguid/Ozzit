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
//!
//! A new place that states the size fails too: tracked prose, text and images are
//! swept for a function or name count the library has had, or any help-table
//! count, outside the register and its exemptions. `sync_counts` rewrites the
//! statements about `main` and the newest release after a release is tagged.
use crate::{
    cache::{Dump, run_process},
    common::*,
    cover, py,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Duration,
};

pub const REPOSITORY: &str = "ryanduguid/Ozzit";
const TIMEOUT: Duration = Duration::from_secs(30);
const REGISTER: &str = "tools/ozzit/src/counts.rs";
const BANNER: &str = r#"<title id="t">Ozzit: (?P<functions>\d+) native Excel LAMBDAs\."#;
const ABOUT: &str = r"^(?P<functions>\d+) Excel LAMBDA functions and (?P<help>\w+) help tables";
/// Help-table counts written as words, as 'five help tables'.
const WORDS: [&str; 10] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

/// A statement of the library's size, as '150 functions' or '155 names', for the
/// sweep for statements outside the register.
const SIZE_PHRASE: &str = r"\b(?P<n>\d+)(?:(?:\s|-)(?:native|Excel|LAMBDA|named|function))*(?:\s|-)(?:functions?|LAMBDAs?|names)\b";
/// A statement of the help-table count, in figures or words.
const HELP_PHRASE: &str = r"(?i)\b(?P<n>\d+|zero|one|two|three|four|five|six|seven|eight|nine)(?:\s+named)?\s+help\s+tables?\b";
/// Statements of a size the sweep finds that are deliberately outside the register:
/// file and a phrase containing the count, matched literally, so reflowing one
/// fails. An entry whose phrase is no longer in its file fails.
pub const EXEMPT: [(&str, &str); 2] = [
    // History: the size at the cut that fixed the cover label.
    ("tools/postbuild/README.md", "130 functions after 3 cuts"),
    // History: the names v1.2.6 shipped.
    ("tools/released-names-v1.2.6.txt", "The 130 function names"),
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
pub const STATEMENTS: [(&str, &str, Basis, &str); 18] = [
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
        "docs/modelling-conventions.md",
        "functions called from cells",
        Basis::Main,
        r"Ozzit's (?P<functions>\d+) functions exist to be called",
    ),
    (
        "docs/function-guide.md",
        "index size",
        Basis::Main,
        r"indexes all (?P<names>\d+) names",
    ),
    (
        "docs/workbook-use.md",
        "help tables oz.txt holds",
        Basis::Main,
        r"ships \(the (?P<help>\w+) help tables included\)",
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
    (
        ".github/social-preview.svg",
        "title",
        Basis::Release,
        BANNER,
    ),
];

/// The release tags, their indexes and the tracked files.
pub trait Releases {
    /// The `vX.Y.Z` tags, or why they could not be listed.
    fn tags(&self) -> Result<Vec<String>>;
    /// `functions.csv` at tag `v{version}`.
    fn index(&self, version: &str) -> Result<String>;
    /// The tracked files, as repository-relative paths with `/`.
    fn tracked(&self) -> Result<Vec<String>>;
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
    fn tracked(&self) -> Result<Vec<String>> {
        Ok(self
            .run(&["ls-files", "-z"])?
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect())
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

/// The regular expression a register pattern stands for: each space matches
/// any run of whitespace.
pub fn regex(pattern: &str) -> String {
    pattern.replace(' ', r"\s+")
}

/// The version of a `vX.Y.Z` release tag, with its numeric order.
fn release_version(tag: &str) -> Option<(Vec<u64>, String)> {
    let parts = captures(r"^v([0-9]+)\.([0-9]+)\.([0-9]+)$", tag).pop()?;
    let order = parts[1..]
        .iter()
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    Some((order, tag[1..].to_owned()))
}

/// The newest version among release tags, such as `3.6.0` for `v3.6.0`.
pub fn newest_release(tags: &[String]) -> Option<String> {
    tags.iter()
        .filter_map(|tag| release_version(tag))
        .max()
        .map(|(_, version)| version)
}

/// Whether a run is in GitHub Actions, where missing evidence fails the gate.
fn strict(var: &impl Fn(&str) -> Option<String>) -> bool {
    var("GITHUB_ACTIONS").as_deref() == Some("true")
}

/// Read the About description by REST through `gh`, only where
/// `GITHUB_REPOSITORY` names the upstream repository, so a local run or a fork
/// never fails on it. There a missing token or a failed read fails in GitHub
/// Actions, so dropping the workflow's token cannot turn the check off unseen.
pub fn read_about(
    var: impl Fn(&str) -> Option<String>,
    fetch: impl FnOnce() -> Result<Dump>,
) -> About {
    let set = |name: &str| var(name).filter(|value| !value.is_empty());
    let repository = set("GITHUB_REPOSITORY");
    if repository.as_deref() != Some(REPOSITORY) {
        return About::Unchecked(format!(
            "GITHUB_REPOSITORY is {}, not {REPOSITORY}",
            repository.map_or_else(|| "unset".to_owned(), |r| format!("{r:?}"))
        ));
    }
    let failed = if set("GH_TOKEN").or_else(|| set("GITHUB_TOKEN")).is_none() {
        "no GH_TOKEN or GITHUB_TOKEN".to_owned()
    } else {
        match fetch() {
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
        }
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

/// A count or version `sync_counts` rewrites: where it is, its byte range in the
/// file's newline-normalised text, what it said and what it should say.
struct Edit {
    at: String,
    file: String,
    start: usize,
    end: usize,
    was: String,
    text: String,
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
    edits: Vec<Edit>,
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

    /// Compare every match of `pattern` in a statement's text.
    fn statement(&mut self, statement: &Statement, pattern: &str) {
        let mut found = 0;
        for matched in re(&regex(pattern)).captures_iter(statement.text) {
            match matched {
                Ok(matched) => {
                    found += 1;
                    self.compare(statement, &matched);
                }
                Err(error) => {
                    self.failures.push(format!(
                        "FAIL: {}: the {} could not be matched: {error}",
                        statement.place, statement.what
                    ));
                    return;
                }
            }
        }
        if found == 0 {
            self.failures.push(format!(
                "FAIL: {}: the {} is missing or reworded; restore it or update the register in {REGISTER}",
                statement.place, statement.what
            ));
        }
    }

    /// Record a correction for `sync_counts`, which rewrites only files and only
    /// statements about `main` or the newest release. The banners and the social
    /// card draw their text as paths, so they are regenerated, not rewritten.
    fn edit(&mut self, statement: &Statement, found: &fancy_regex::Match<'_>, text: String) {
        let drawn = statement.place.ends_with(".svg");
        if statement.numbered && statement.basis != Basis::Named && !drawn {
            self.edits.push(Edit {
                at: statement.at(found.start()),
                file: statement.place.to_owned(),
                start: found.start(),
                end: found.end(),
                was: found.as_str().to_owned(),
                text,
            });
        }
    }

    /// The index a match describes and its name in messages, or None when
    /// release statements cannot be checked.
    fn source(
        &mut self,
        statement: &Statement,
        matched: &fancy_regex::Captures<'_, str>,
    ) -> Option<(String, Result<Inventory>)> {
        let latest = match (statement.basis, self.latest.clone()) {
            (Basis::Main, _) => return Some(("functions.csv".into(), Ok(self.main))),
            (_, None) => return None,
            (Basis::Release, Some(latest)) => {
                return Some((format!("v{latest}"), self.release(&latest)));
            }
            (_, Some(latest)) => latest,
        };
        let named = matched
            .name("version")
            .expect("a Named or Latest pattern captures its version");
        let version = named.as_str();
        if statement.basis == Basis::Latest && version != latest {
            self.failures.push(format!(
                "FAIL: {}: the {} names v{version}; the newest release tag is v{latest}",
                statement.at(named.start()),
                statement.what
            ));
            self.edit(statement, &named, latest);
        }
        Some((format!("v{version}"), self.release(version)))
    }

    /// Compare each count one match states with the index it describes.
    fn compare(&mut self, statement: &Statement, matched: &fancy_regex::Captures<'_, str>) {
        let Some((source, index)) = self.source(statement, matched) else {
            return;
        };
        let what = statement.what;
        let index = match index {
            Ok(index) => index,
            Err(error) => {
                let start = matched.get(0).map_or(0, |m| m.start());
                self.failures.push(format!(
                    "FAIL: {}: the {what}: {error}",
                    statement.at(start)
                ));
                return;
            }
        };
        self.checked += 1;
        for (group, noun, count) in [
            ("functions", "functions", index.functions),
            ("help", "help tables", index.help),
            ("names", "names", index.functions + index.help),
        ] {
            let Some(found) = matched.name(group) else {
                continue;
            };
            let at = statement.at(found.start());
            let stated = found.as_str();
            let value = stated
                .parse::<usize>()
                .ok()
                .or_else(|| WORDS.iter().position(|w| w.eq_ignore_ascii_case(stated)));
            match value {
                Some(value) if value == count => {}
                Some(value) => {
                    self.failures.push(format!(
                        "FAIL: {at}: the {what} says {value} {noun}; {source} has {count} {noun}"
                    ));
                    self.edit(statement, &found, render(stated, count));
                }
                None => self.failures.push(format!(
                    "FAIL: {at}: the {what} gives {} {noun}, not a count",
                    py::repr_str(stated)
                )),
            }
        }
    }

    /// Check the About description if it was read, or report why not.
    fn about(&mut self, about: &About, out: &mut Vec<String>) {
        match about {
            About::Unchecked(reason) => {
                out.push(format!("SKIP: About description not read: {reason}"));
            }
            About::Unreadable(reason) => self.failures.push(format!(
                "FAIL: About description could not be read: {reason}"
            )),
            About::Text(text) => {
                let statement = Statement {
                    place: "About description",
                    numbered: false,
                    what: "repository description",
                    basis: Basis::Release,
                    text,
                };
                self.statement(&statement, ABOUT);
            }
        }
    }

    /// Every function and name count the library has had: on `main` and at each
    /// release tag. A tag whose index cannot be read is reported, and fails when
    /// `strict`.
    fn sizes(&mut self, strict: bool, out: &mut Vec<String>) -> BTreeSet<usize> {
        let mut sizes = BTreeSet::from([self.main.functions, self.main.functions + self.main.help]);
        for version in self.tags.clone() {
            match self.release(&version) {
                Ok(index) => sizes.extend([index.functions, index.functions + index.help]),
                Err(error) if strict => self.failures.push(format!(
                    "FAIL: v{version}: its sizes could not be read: {error}"
                )),
                Err(error) => out.push(format!("SKIP: v{version}'s sizes not looked for: {error}")),
            }
        }
        sizes
    }

    /// Flag statements of the library's size in tracked text that the register
    /// does not hold: a function or name count the library has had, or any
    /// help-table count, outside every registered statement and exemption.
    fn sweep(&mut self, root: &Path, strict: bool, out: &mut Vec<String>) {
        let files = match self.releases.tracked() {
            Ok(files) => files,
            Err(error) => {
                let line = format!("the tracked files could not be listed: {error}");
                if strict {
                    self.failures.push(format!("FAIL: {line}"));
                } else {
                    out.push(format!(
                        "SKIP: unregistered statements not looked for: {line}"
                    ));
                }
                return;
            }
        };
        let sizes = self.sizes(strict, out);
        let mut used = [false; EXEMPT.len()];
        for file in files.iter().filter(|file| swept(file)) {
            let path = root.join(file);
            // A tracked file deleted in the working tree has nothing to state.
            if !path.exists() {
                continue;
            }
            match read_text(&path) {
                Ok(text) => self.sweep_file(file, &text, &sizes, &mut used),
                Err(error) => self.failures.push(format!("FAIL: {error}")),
            }
        }
        for ((file, phrase), used) in EXEMPT.iter().zip(used) {
            if !used {
                self.failures.push(format!(
                    "FAIL: {file}: the exemption '{phrase}' is no longer there; remove it from EXEMPT in {REGISTER}"
                ));
            }
        }
    }

    fn sweep_file(&mut self, file: &str, text: &str, sizes: &BTreeSet<usize>, used: &mut [bool]) {
        let mut covered = registered(file, text);
        for (i, (_, phrase)) in EXEMPT.iter().enumerate().filter(|(_, (f, _))| *f == file) {
            for (at, _) in text.match_indices(phrase) {
                used[i] = true;
                covered.push(at..at + phrase.len());
            }
        }
        for (pattern, sized) in [(SIZE_PHRASE, true), (HELP_PHRASE, false)] {
            for found in re(pattern).captures_iter(text) {
                let found = match found {
                    Ok(found) => found,
                    Err(error) => {
                        self.failures
                            .push(format!("FAIL: {file}: could not look for counts: {error}"));
                        return;
                    }
                };
                let count = found.name("n").expect("the phrase captures its count");
                let start = count.start();
                let size = count.as_str().parse::<usize>().ok();
                if (sized && !size.is_some_and(|n| sizes.contains(&n)))
                    || covered.iter().any(|range| range.contains(&start))
                {
                    continue;
                }
                let phrase = found.get(0).map_or("", |m| m.as_str());
                self.failures.push(format!(
                    "FAIL: {file}:{}: '{}' states the library's size outside the register; add it to STATEMENTS or EXEMPT in {REGISTER}",
                    text[..start].matches('\n').count() + 1,
                    phrase.split_whitespace().collect::<Vec<_>>().join(" ")
                ));
            }
        }
    }

    /// The closing line once every statement agrees.
    fn agreed(&mut self) -> String {
        let release = match self.latest.clone() {
            Some(latest) => self
                .release(&latest)
                .map(|index| {
                    format!(
                        "; v{latest} has {} functions and {} help tables",
                        index.functions, index.help
                    )
                })
                .unwrap_or_default(),
            None => String::new(),
        };
        format!(
            "OK: {} statements agree: main has {} functions and {} help tables{release}",
            self.checked, self.main.functions, self.main.help
        )
    }
}

/// `value` in the form `stated` used: a word for a count written as one.
fn render(stated: &str, value: usize) -> String {
    match WORDS.get(value) {
        Some(word) if stated.chars().all(char::is_alphabetic) => {
            if stated.starts_with(char::is_uppercase) {
                word[..1].to_uppercase() + &word[1..]
            } else {
                (*word).to_owned()
            }
        }
        _ => value.to_string(),
    }
}

/// Whether the sweep reads a tracked file: prose and images, not the changelogs
/// (records of earlier releases) or the generated module sources.
fn swept(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    [".md", ".txt", ".svg"]
        .iter()
        .any(|ext| path.ends_with(ext))
        && !name.starts_with("CHANGELOG")
        && !path.starts_with("src/")
        && path != "oz.txt"
}

/// The byte ranges registered statements cover in `file`. A pattern that fails to
/// run is reported by the statement check, so it covers nothing here.
fn registered(file: &str, text: &str) -> Vec<std::ops::Range<usize>> {
    STATEMENTS
        .iter()
        .filter(|(place, _, _, _)| *place == file)
        .flat_map(|(_, _, _, pattern)| {
            re(&regex(pattern))
                .captures_iter(text)
                .filter_map(|found| found.ok()?.get(0).map(|m| m.range()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// One statement's text and how to name a place in it.
struct Statement<'a> {
    place: &'a str,
    /// Whether a place is named by its line in `place`.
    numbered: bool,
    what: &'a str,
    basis: Basis,
    text: &'a str,
}
impl Statement<'_> {
    fn at(&self, offset: usize) -> String {
        if self.numbered {
            let line = self.text[..offset].matches('\n').count() + 1;
            format!("{}:{line}", self.place)
        } else {
            self.place.to_owned()
        }
    }
}

/// The release versions tagged, and the newest of them. Tags that cannot be read
/// or a checkout without any are reported, and fail when `strict`.
fn release_tags(
    releases: &dyn Releases,
    strict: bool,
    out: &mut Vec<String>,
    failures: &mut Vec<String>,
) -> (Vec<String>, Option<String>) {
    let missing = match releases.tags() {
        Ok(tags) => match newest_release(&tags) {
            Some(latest) => {
                let versions = tags
                    .iter()
                    .filter_map(|tag| release_version(tag))
                    .map(|(_, version)| version)
                    .collect();
                return (versions, Some(latest));
            }
            None => "this checkout has no vX.Y.Z release tags; fetch them, as actions/checkout does with fetch-depth: 0".to_owned(),
        },
        Err(error) => format!("the release tags could not be read: {error}"),
    };
    if strict {
        failures.push(format!("FAIL: {missing}"));
    } else {
        out.push(format!("SKIP: release statements not checked: {missing}"));
    }
    (Vec::new(), None)
}

/// Run every check, returning the checker with its failures and corrections, and
/// the lines reported before them.
fn run_checks<'a>(
    root: &Path,
    about: &About,
    releases: &'a dyn Releases,
    strict: bool,
) -> Result<(Checker<'a>, Vec<String>)> {
    let (functions, help) = cover::inventory(&root.join("functions.csv"))?;
    let (cut, _) = cover::current_cut(&root.join("CHANGELOG.md"))?;
    let mut out = Vec::new();
    let mut failures = Vec::new();
    let (tags, latest) = release_tags(releases, strict, &mut out, &mut failures);
    let mut checker = Checker {
        releases,
        main: Inventory { functions, help },
        cut,
        tags,
        latest,
        indexes: BTreeMap::new(),
        checked: 0,
        failures,
        edits: Vec::new(),
    };
    for (place, what, basis, pattern) in STATEMENTS {
        let text = read_text(&root.join(place))?;
        let statement = Statement {
            place,
            numbered: true,
            what,
            basis,
            text: &text,
        };
        checker.statement(&statement, pattern);
    }
    checker.about(about, &mut out);
    checker.sweep(root, strict, &mut out);
    Ok((checker, out))
}

/// Check every registered statement under `root`, look for unregistered ones and
/// check the About description if read. `strict` (GitHub Actions) fails where
/// release tags or the tracked files cannot be read.
pub fn check(root: &Path, about: &About, releases: &dyn Releases, strict: bool) -> Output {
    let run = || -> Result<Output> {
        let (mut checker, mut out) = run_checks(root, about, releases, strict)?;
        let code = u8::from(!checker.failures.is_empty());
        if checker.failures.is_empty() {
            out.push(checker.agreed());
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

/// Rewrite each file's corrections from the end, so earlier offsets hold.
fn apply(root: &Path, edits: Vec<Edit>) -> Result<Vec<String>> {
    let mut files: BTreeMap<String, Vec<Edit>> = BTreeMap::new();
    for edit in edits {
        files.entry(edit.file.clone()).or_default().push(edit);
    }
    let mut changed = Vec::new();
    for (file, mut edits) in files {
        let path = root.join(&file);
        let mut text = read_text(&path)?;
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
        if let Some(pair) = edits.windows(2).find(|pair| pair[1].end > pair[0].start) {
            return Err(format!(
                "{} and {} overlap; fix the register in {REGISTER}",
                pair[1].at, pair[0].at
            ));
        }
        for edit in &edits {
            text.replace_range(edit.start..edit.end, &edit.text);
        }
        replace_atomically(&path, text.as_bytes())?;
        changed.extend(
            edits
                .iter()
                .rev()
                .map(|edit| format!("CHANGED: {}: {} to {}", edit.at, edit.was, edit.text)),
        );
    }
    Ok(changed)
}

/// Move every statement about `main` or the newest release under `root` to the
/// counts and version it should state, then check the result, failing where the
/// release tags or the tracked files cannot be read. Statements naming
/// a release, such as release bullets, the About description and the drawn
/// banners and social card stay as they are.
pub fn sync_with(root: &Path, releases: &dyn Releases) -> Output {
    let about = About::Unchecked(
        "sync_counts does not change it; set it by hand, then run verify_counts in CI".into(),
    );
    let run = || -> Result<Output> {
        let mut out = Vec::new();
        // A statement moved to a newer release is compared with that release's
        // counts on the next pass.
        for _ in 0..3 {
            let (checker, _) = run_checks(root, &about, releases, true)?;
            if checker.edits.is_empty() {
                break;
            }
            out.extend(apply(root, checker.edits)?);
        }
        let after = check(root, &about, releases, true);
        out.extend(after.out);
        Ok(Output {
            code: after.code,
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

pub fn sync(args: &[String]) -> Output {
    if args.len() > 1 {
        return Output::stderr("FAIL: usage: ozzit sync_counts [repository-root]");
    }
    let root = py::PyPath::new(args.first().map_or(".", String::as_str)).fs();
    sync_with(&root, &Git(&root))
}
