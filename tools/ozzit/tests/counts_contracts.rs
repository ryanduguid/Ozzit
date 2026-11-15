// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

//! The function count gate: every registered statement of the library's size
//! agrees with the index it describes, and true history about older releases passes.
use ozzit::{
    cache::Dump,
    common::{Csv, Output, Result, re},
    counts::{self, About, Git, Releases},
    cover,
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The functions and help tables `main` publishes.
fn main_index() -> (usize, usize) {
    cover::inventory(&root().join("functions.csv")).unwrap()
}

/// The newest release tag's version and its functions and help tables.
fn latest() -> (String, usize, usize) {
    let git = Git(&root());
    let version = counts::newest_release(&git.tags().unwrap())
        .expect("the checkout has release tags; run git fetch --tags");
    let csv = Csv::parse(&git.index(&version).unwrap()).unwrap();
    let (functions, help) = cover::inventory_of(&csv, "functions.csv").unwrap();
    (version, functions, help)
}

/// An index of `functions` functions and `help` help tables.
fn index(functions: usize, help: usize) -> String {
    let mut text = "function\n".to_owned();
    for i in 0..functions {
        text += &format!("oz.F{i}λ\n");
    }
    for i in 0..help {
        text += &format!("oz.About{i}λ\n");
    }
    text
}

/// The repository's release tags, with extra releases or a replaced tag list.
#[derive(Default)]
struct Overlay {
    tags: Option<Result<Vec<String>>>,
    extra: Vec<(&'static str, String)>,
    tracked: Option<Result<Vec<String>>>,
}
impl Releases for Overlay {
    fn tags(&self) -> Result<Vec<String>> {
        if let Some(tags) = &self.tags {
            return tags.clone();
        }
        let mut tags = Git(&root()).tags()?;
        tags.extend(self.extra.iter().map(|(version, _)| format!("v{version}")));
        Ok(tags)
    }
    fn index(&self, version: &str) -> Result<String> {
        match self.extra.iter().find(|(v, _)| *v == version) {
            Some((_, text)) => Ok(text.clone()),
            None => Git(&root()).index(version),
        }
    }
    fn tracked(&self) -> Result<Vec<String>> {
        self.tracked.clone().unwrap_or_else(|| Git(&root()).tracked())
    }
}

/// A copy of every file the gate reads, taken from the repository.
struct Fixture {
    dir: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let files = counts::STATEMENTS
            .iter()
            .map(|(file, _, _, _)| *file)
            .chain(counts::EXEMPT.iter().map(|(file, _)| *file))
            .chain(["functions.csv", "CHANGELOG.md"]);
        for file in files {
            let target = dir.path().join(file);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(root().join(file), target).unwrap();
        }
        Self { dir }
    }
    fn path(&self) -> &Path {
        self.dir.path()
    }
    /// Replace the one occurrence of `old`, returning the line it was on.
    fn rewrite(&self, file: &str, old: &str, new: &str) -> usize {
        let path = self.path().join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches(old).count(), 1, "{file}: {old}");
        let line = text[..text.find(old).unwrap()].matches('\n').count() + 1;
        std::fs::write(&path, text.replacen(old, new, 1)).unwrap();
        line
    }
    /// Append `text` to `file`, returning the line it starts on.
    fn append(&self, file: &str, text: &str) -> usize {
        let path = self.path().join(file);
        let old = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{old}\n{text}\n")).unwrap();
        old.matches('\n').count() + 2
    }
    fn check_with(&self, about: &About, releases: &dyn Releases, strict: bool) -> Output {
        counts::check(self.path(), about, releases, strict)
    }
    fn check(&self) -> Output {
        self.check_with(&unchecked(), &Overlay::default(), false)
    }
}

fn unchecked() -> About {
    About::Unchecked("offline".into())
}

fn passes(output: &Output) {
    assert_eq!(output.code, 0, "{:?} {:?}", output.out, output.err);
    assert!(output.err.is_empty(), "{:?}", output.err);
    assert!(
        output.out.last().unwrap().starts_with("OK: "),
        "{:?}",
        output.out
    );
}

fn fails_with(output: &Output, expected: &[&str]) {
    assert_eq!(output.code, 1, "{:?}", output.out);
    let failures = output
        .out
        .iter()
        .filter(|line| line.starts_with("FAIL: "))
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), expected.len(), "{failures:?}");
    for (failure, expected) in failures.iter().zip(expected) {
        assert!(failure.contains(expected), "{failure:?} lacks {expected:?}");
    }
}

#[test]
fn the_repository_states_one_count() {
    passes(&counts::check(&root(), &unchecked(), &Git(&root()), false));
}

#[test]
fn a_stale_count_fails_naming_file_and_line() {
    let f = Fixture::new();
    let (version, functions, _) = latest();
    let stale = functions + 1;
    let line = f.rewrite(
        "docs/practice-improvements.md",
        &format!("the {functions}-function library"),
        &format!("the {stale}-function library"),
    );
    fails_with(
        &f.check(),
        &[&format!(
            "docs/practice-improvements.md:{line}: the existing library says {stale} functions; v{version} has {functions} functions"
        )],
    );
}

/// Counts written as words in the registered statements, as 'five help tables'.
const WORDS: [&str; 10] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

#[test]
fn every_registered_count_is_checked() {
    let mut checked = 0;
    for (file, what, _, pattern) in counts::STATEMENTS {
        for (group, noun) in [
            ("functions", "functions"),
            ("help", "help tables"),
            ("names", "names"),
        ] {
            let f = Fixture::new();
            let path = f.path().join(file);
            let text = std::fs::read_to_string(&path).unwrap();
            let captures = re(&counts::regex(pattern))
                .captures(&text)
                .unwrap()
                .unwrap_or_else(|| panic!("{file}: the {what} is missing"));
            let Some(stated) = captures.name(group) else {
                continue;
            };
            let value = stated.as_str().parse::<usize>().unwrap_or_else(|_| {
                WORDS
                    .iter()
                    .position(|w| *w == stated.as_str())
                    .expect("a count")
            });
            let line = text[..stated.start()].matches('\n').count() + 1;
            let changed = format!(
                "{}{}{}",
                &text[..stated.start()],
                value + 1,
                &text[stated.end()..]
            );
            std::fs::write(&path, changed).unwrap();
            let output = f.check();
            fails_with(
                &output,
                &[&format!(
                    "{file}:{line}: the {what} says {} {noun}; ",
                    value + 1
                )],
            );
            let failure = output.out.iter().find(|l| l.starts_with("FAIL")).unwrap();
            assert!(
                failure.ends_with(&format!(" has {value} {noun}")),
                "{failure}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 23);
}

#[test]
fn every_release_pattern_captures_its_version() {
    for (file, what, basis, pattern) in counts::STATEMENTS {
        let named = matches!(basis, counts::Basis::Named | counts::Basis::Latest);
        assert_eq!(
            pattern.contains("(?P<version>"),
            named,
            "{file}: the {what}"
        );
    }
}

#[test]
fn a_count_that_is_not_a_number_fails() {
    let (_, h) = main_index();
    let f = Fixture::new();
    let line = f.rewrite(
        "AGENTS.md",
        &format!("functions and {h} named help"),
        "functions and several named help",
    );
    fails_with(
        &f.check(),
        &[&format!(
            "AGENTS.md:{line}: the library size gives 'several' help tables, not a count"
        )],
    );
}

#[test]
fn statements_about_older_releases_are_checked_against_their_tags() {
    let bullet = |version: &str, functions: usize| {
        format!(
            "- **[v{version}](https://github.com/ryanduguid/Ozzit/releases/tag/v{version})**, published\n  1 May 2020 from the cut dated 1 May 2020, with {functions} functions."
        )
    };
    let releases = Overlay {
        extra: vec![("0.9.0", index(7, 2))],
        ..Overlay::default()
    };
    let f = Fixture::new();
    f.append("README.md", &bullet("0.9.0", 7));
    passes(&f.check_with(&unchecked(), &releases, false));
    let f = Fixture::new();
    // The count is on the bullet's second line, which a failure names.
    let line = f.append("README.md", &bullet("0.9.0", 8)) + 1;
    fails_with(
        &f.check_with(&unchecked(), &releases, false),
        &[&format!(
            "README.md:{line}: the release bullet says 8 functions; v0.9.0 has 7 functions"
        )],
    );
    let f = Fixture::new();
    let line = f.append("README.md", &bullet("0.8.0", 7));
    fails_with(
        &f.check_with(&unchecked(), &releases, false),
        &[&format!(
            "README.md:{line}: the release bullet: v0.8.0 has no release tag and is not the changelog's newest cut"
        )],
    );
}

#[test]
fn an_untagged_cut_is_checked_against_main() {
    let (n, _) = main_index();
    for (stated, failure) in [(n, None), (n + 1, Some(n))] {
        let f = Fixture::new();
        let changelog = f.path().join("CHANGELOG.md");
        let text = std::fs::read_to_string(&changelog).unwrap();
        let heading = text.find("\n## v").unwrap() + 5;
        let end = heading + text[heading..].find(',').unwrap();
        std::fs::write(
            &changelog,
            format!("{}9.9.9{}", &text[..heading], &text[end..]),
        )
        .unwrap();
        let line = f.append(
            "README.md",
            &format!("- **v9.9.9**, cut dated 1 May 2027 from `main`, with {stated} functions:"),
        );
        let output = f.check();
        match failure {
            None => passes(&output),
            Some(count) => fails_with(
                &output,
                &[&format!(
                    "README.md:{line}: the release bullet says {stated} functions; v9.9.9 has {count} functions"
                )],
            ),
        }
    }
}

#[test]
fn a_newer_tag_makes_the_latest_statements_stale() {
    let (version, _, _) = latest();
    let text = Git(&root()).index(&version).unwrap();
    let releases = Overlay {
        extra: vec![("99.0.0", text)],
        ..Overlay::default()
    };
    let f = Fixture::new();
    fails_with(
        &f.check_with(&unchecked(), &releases, false),
        &[
            &format!("the introduction names v{version}; the newest release tag is v99.0.0"),
            &format!("the oz.txt name count names v{version}; the newest release tag is v99.0.0"),
        ],
    );
}

#[test]
fn missing_release_tags_skip_offline_and_fail_in_ci() {
    let none = Overlay {
        tags: Some(Ok(vec!["v1".into(), "release".into()])),
        ..Overlay::default()
    };
    let unreadable = Overlay {
        tags: Some(Err("not a git repository".into())),
        ..Overlay::default()
    };
    let f = Fixture::new();
    let output = f.check_with(&unchecked(), &none, false);
    passes(&output);
    assert_eq!(
        output.out[0],
        "SKIP: release statements not checked: this checkout has no vX.Y.Z release tags; fetch them, as actions/checkout does with fetch-depth: 0"
    );
    let output = f.check_with(&unchecked(), &unreadable, false);
    passes(&output);
    assert_eq!(
        output.out[0],
        "SKIP: release statements not checked: the release tags could not be read: not a git repository"
    );
    fails_with(
        &f.check_with(&unchecked(), &none, true),
        &["FAIL: this checkout has no vX.Y.Z release tags; fetch them"],
    );
    fails_with(
        &f.check_with(&unchecked(), &unreadable, true),
        &["FAIL: the release tags could not be read: not a git repository"],
    );
}

#[test]
fn a_reworded_statement_fails_as_missing() {
    let (_, r, _) = latest();
    let f = Fixture::new();
    f.rewrite(
        "README.md",
        &format!("does not need all {r} functions"),
        "does not need every function",
    );
    fails_with(
        &f.check(),
        &["README.md: the 'does not need all N functions' line is missing or reworded"],
    );
}

#[test]
fn the_about_description_is_checked_when_read() {
    let f = Fixture::new();
    let (version, r, rh) = latest();
    let check = |about: About| f.check_with(&about, &Overlay::default(), false);
    let output = check(About::Text(format!(
        "{r} Excel LAMBDA functions and {rh} help tables for Australian GST."
    )));
    passes(&output);
    assert!(output.out.iter().all(|line| !line.contains("About")));
    fails_with(
        &check(About::Text(format!(
            "{} Excel LAMBDA functions and {rh} help tables for Australian GST.",
            r + 1
        ))),
        &[&format!(
            "FAIL: About description: the repository description says {} functions; v{version} has {r} functions",
            r + 1
        )],
    );
    fails_with(
        &check(About::Text("Excel LAMBDA functions.".into())),
        &["FAIL: About description: the repository description is missing or reworded"],
    );
    fails_with(
        &check(About::Unreadable("gh is not installed".into())),
        &["FAIL: About description could not be read: gh is not installed"],
    );
    let output = check(About::Unchecked("no token".into()));
    passes(&output);
    assert_eq!(output.out[0], "SKIP: About description not read: no token");
}

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

fn never() -> Result<Dump> {
    panic!("the description must not be fetched")
}

fn exited(code: i64, stdout: &str, stderr: &str) -> Result<Dump> {
    Ok(Dump::Exited {
        code,
        stdout: stdout.into(),
        stderr: stderr.into(),
    })
}

fn reason(about: About) -> String {
    match about {
        About::Unchecked(reason) => reason,
        other => panic!("{other:?}"),
    }
}

const UPSTREAM: (&str, &str) = ("GITHUB_REPOSITORY", "ryanduguid/Ozzit");
const ACTIONS: (&str, &str) = ("GITHUB_ACTIONS", "true");

#[test]
fn the_description_is_read_only_in_the_upstream_repository() {
    assert_eq!(
        reason(counts::read_about(
            env(&[("GH_TOKEN", "t"), ACTIONS]),
            never
        )),
        "GITHUB_REPOSITORY is unset, not ryanduguid/Ozzit"
    );
    assert_eq!(
        reason(counts::read_about(
            env(&[
                ("GITHUB_TOKEN", "t"),
                ("GITHUB_REPOSITORY", "someone/Ozzit"),
                ACTIONS
            ]),
            never
        )),
        "GITHUB_REPOSITORY is \"someone/Ozzit\", not ryanduguid/Ozzit"
    );
    for pairs in [
        &[("GH_TOKEN", "t"), UPSTREAM][..],
        &[("GITHUB_TOKEN", "t"), UPSTREAM, ACTIONS],
    ] {
        match counts::read_about(env(pairs), || {
            exited(0, " 150 Excel LAMBDA functions\n", "")
        }) {
            About::Text(text) => assert_eq!(text, "150 Excel LAMBDA functions"),
            other => panic!("{other:?}"),
        }
    }
}

/// The environment, the read and the reason it was not used.
type FailedRead = (
    &'static [(&'static str, &'static str)],
    fn() -> Result<Dump>,
    &'static str,
);

#[test]
fn a_failed_read_upstream_is_reported_locally_and_fails_in_ci() {
    let cases: [FailedRead; 5] = [
        (
            &[UPSTREAM, ("GH_TOKEN", "")],
            never,
            "no GH_TOKEN or GITHUB_TOKEN",
        ),
        (
            &[UPSTREAM, ("GH_TOKEN", "t")],
            || Ok(Dump::NotFound),
            "gh is not installed",
        ),
        (
            &[UPSTREAM, ("GH_TOKEN", "t")],
            || Ok(Dump::TimedOut),
            "gh api took longer than 30 seconds",
        ),
        (
            &[UPSTREAM, ("GITHUB_TOKEN", "t")],
            || {
                exited(
                    1,
                    "",
                    "error connecting to api.github.com\ncheck your internet connection\n",
                )
            },
            "gh api exited 1: error connecting to api.github.com",
        ),
        (
            &[UPSTREAM, ("GH_TOKEN", "t")],
            || Err("denied".into()),
            "gh could not run: denied",
        ),
    ];
    for (pairs, fetch, expected) in cases {
        assert_eq!(reason(counts::read_about(env(pairs), fetch)), expected);
        let strict = [pairs, &[ACTIONS]].concat();
        match counts::read_about(env(&strict), fetch) {
            About::Unreadable(reason) => assert_eq!(reason, expected),
            other => panic!("{other:?}"),
        }
    }
}

fn cli(dir: &Path, command: &str, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .current_dir(dir)
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_ACTIONS")
        .env_remove("GITHUB_REPOSITORY")
        // The fixture is no checkout; stop Git finding one above the temporary folder.
        .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
        .arg(command)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn the_command_reads_the_working_directory_or_a_named_root() {
    let f = Fixture::new();
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    for output in [
        cli(f.path(), "verify_counts", &[]),
        cli(&root(), "verify_counts", &[f.path().to_str().unwrap()]),
    ] {
        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8(output.stdout).unwrap();
        let lines = stdout.split(newline).collect::<Vec<_>>();
        assert_eq!(lines.len(), 5, "{stdout}");
        assert!(
            lines[0].starts_with(
                "SKIP: release statements not checked: the release tags could not be read: git tag --list v* exited "
            ),
            "{stdout}"
        );
        assert_eq!(
            lines[1],
            "SKIP: About description not read: GITHUB_REPOSITORY is unset, not ryanduguid/Ozzit"
        );
        assert!(
            lines[2].starts_with(
                "SKIP: unregistered statements not looked for: the tracked files could not be listed: git ls-files -z exited "
            ),
            "{stdout}"
        );
        assert!(lines[3].starts_with("OK: "), "{stdout}");
    }
    for command in ["verify_counts", "sync_counts"] {
        let output = cli(f.path(), command, &["a", "b"]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!("FAIL: usage: ozzit {command} [repository-root]{newline}")
        );
    }
    let output = cli(&f.path().join("docs"), "verify_counts", &[]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.starts_with("FAIL: ") && stdout.contains("functions.csv"),
        "{stdout}"
    );
}

#[test]
fn a_statement_of_the_size_outside_the_register_fails() {
    let (n, h) = main_index();
    let f = Fixture::new();
    let help = f.append("AGENTS.md", "The FIVE help\ntables come last.");
    let size = f.append(
        "docs/function-guide.md",
        &format!("Ozzit has {n}\nfunctions, and {}\nnames in all.", n + h),
    );
    // A count the library has never had is not its size.
    f.append("docs/function-guide.md", "A model may call 7 functions.");
    fails_with(
        &f.check(),
        &[
            &format!("FAIL: AGENTS.md:{help}: 'FIVE help tables' states the library's size outside the register; add it to STATEMENTS or EXEMPT in tools/ozzit/src/counts.rs"),
            &format!("FAIL: docs/function-guide.md:{size}: '{n} functions' states"),
            &format!("FAIL: docs/function-guide.md:{}: '{} names' states", size + 1, n + h),
        ],
    );
}

#[test]
fn an_exemption_whose_phrase_is_gone_fails() {
    let (file, phrase) = counts::EXEMPT[0];
    let f = Fixture::new();
    f.rewrite(file, phrase, "an earlier count");
    fails_with(
        &f.check(),
        &[&format!(
            "FAIL: {file}: the exemption '{phrase}' is no longer there; remove it from EXEMPT"
        )],
    );
}

#[test]
fn unlisted_tracked_files_skip_the_sweep_offline_and_fail_in_ci() {
    let (n, _) = main_index();
    let releases = Overlay {
        tracked: Some(Err("not a git repository".into())),
        ..Overlay::default()
    };
    let f = Fixture::new();
    f.append("docs/function-guide.md", &format!("Ozzit has {n} functions."));
    let output = f.check_with(&unchecked(), &releases, false);
    passes(&output);
    assert!(
        output.out.contains(
            &"SKIP: unregistered statements not looked for: the tracked files could not be listed: not a git repository"
                .to_owned()
        ),
        "{:?}",
        output.out
    );
    fails_with(
        &f.check_with(&unchecked(), &releases, true),
        &["FAIL: the tracked files could not be listed: not a git repository"],
    );
}

#[test]
fn sync_counts_moves_the_statements_to_a_new_release() {
    let (n, h) = main_index();
    let (version, _, _) = latest();
    let releases = Overlay {
        extra: vec![("99.0.0", index(n + 3, h + 1))],
        ..Overlay::default()
    };
    let f = Fixture::new();
    let agents = f.rewrite(
        "AGENTS.md",
        &format!("has {n} native"),
        &format!("has {} native", n - 1),
    );
    let read = |file: &str| std::fs::read_to_string(f.path().join(file)).unwrap();
    let llms = read("llms.txt");
    let output = counts::sync_with(f.path(), &releases);
    // The banners and the social card are drawn, so they are regenerated instead.
    let drawn = counts::STATEMENTS
        .iter()
        .filter(|(file, _, _, _)| file.ends_with(".svg"))
        .map(|(file, _, _, _)| format!("FAIL: {file}:"))
        .collect::<Vec<_>>();
    assert_eq!(drawn.len(), 5);
    fails_with(&output, &drawn.iter().map(String::as_str).collect::<Vec<_>>());
    let stale = format!("the title says {n} functions; v99.0.0 has {} functions", n + 3);
    assert!(output.out.iter().filter(|l| l.contains(&stale)).count() == 5, "{:?}", output.out);
    let changed = |prefix: &str, change: &str| {
        assert!(
            output
                .out
                .iter()
                .any(|line| line.starts_with(&format!("CHANGED: {prefix}"))
                    && line.ends_with(&format!(": {change}"))),
            "{prefix} {change}: {:?}",
            output.out
        );
    };
    changed(&format!("AGENTS.md:{agents}"), &format!("{} to {n}", n - 1));
    changed("README.md:", &format!("{version} to 99.0.0"));
    changed("README.md:", &format!("{n} to {}", n + 3));
    changed("README.md:", &format!("{} to {}", n + h, n + 3 + h + 1));
    changed("README.md:", &format!("{h} to {}", h + 1));
    changed(
        "docs/between-spreadsheets.md:",
        &format!("{} to {}", WORDS[h], WORDS[h + 1]),
    );
    assert_eq!(read("llms.txt"), llms, "a statement naming a release stays");
    assert!(!output.out.iter().any(|l| l.contains(".svg") && l.starts_with("CHANGED")));
}

#[test]
fn sync_counts_leaves_a_wrong_statement_naming_a_release_failing() {
    let (file, what, _, pattern) = counts::STATEMENTS
        .into_iter()
        .find(|(file, _, _, _)| *file == "llms.txt")
        .unwrap();
    let f = Fixture::new();
    let path = f.path().join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    let stated = re(&counts::regex(pattern))
        .captures(&text)
        .unwrap()
        .unwrap()
        .name("functions")
        .unwrap();
    let wrong = stated.as_str().parse::<usize>().unwrap() + 1;
    let changed = format!("{}{wrong}{}", &text[..stated.start()], &text[stated.end()..]);
    std::fs::write(&path, &changed).unwrap();
    let output = counts::sync_with(f.path(), &Overlay::default());
    fails_with(&output, &[&format!("the {what} says {wrong} functions")]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), changed);
}
