//! The original Python release staging contracts, using real owned files.
use super::*;
use std::{cell::Cell, collections::BTreeMap, fs};

const WORKBOOK: &[u8] = b"synthetic workbook fixture\n";
const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
// Captured from the original Python fixture, including its final LF.
const PROVENANCE: &str = concat!(
    r#"{"assets":["ozzit.xlsx","provenance.json","SHA256SUMS"],"project":"Ozzit","schema":1,"source":{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","workbook_path":"ozzit.xlsx"},"verification":{"limitations":["The bundle copies the reviewed tracked workbook; it does not regenerate it from the earlier workbook.","The documented Mypy, unit, native Excel and cache gates remain required before publication."],"native_gates":{"included":false,"required_before_publication":true},"static_gates":[{"command":"python tools/verify_workbook.py ozzit.xlsx","output":"OK: fixture gate 1"},{"command":"python tools/verify_sources.py ozzit.xlsx src","output":"OK: fixture gate 2"},{"command":"python tools/verify_signatures.py src","output":"OK: fixture gate 3"},{"command":"python tools/verify_previous_names.py functions.csv","output":"OK: fixture gate 4"},{"command":"python tools/verify_index.py ozzit.xlsx src functions.csv","output":"OK: fixture gate 5"},{"command":"python tools/verify_cover.py ozzit.xlsx functions.csv CHANGELOG.md","output":"OK: fixture gate 6"},{"command":"python tools/verify_afe.py ozzit.xlsx src","output":"OK: fixture gate 7"}]},"version":"3.2.0","workbook":{"base_manifest":"release/workbook-base.json","copy_only":true,"git_blob_sha1":"cdcd75d14f66018911bfafa9d5c23b7f25f532fd","last_workbook_commit":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","sha256":"c14e507ac5aa35f6ddd08da5ff1a041ee9bba1f2db7f676e70ca88a94a665df6","size":27}}"#,
    "\n"
);
const CHECKSUMS: &[u8] = b"c14e507ac5aa35f6ddd08da5ff1a041ee9bba1f2db7f676e70ca88a94a665df6  ozzit.xlsx\n96d07dcb9e4e47e84b05eb64355912e21a1b3e78277bb575a752970b2c0c11ff  provenance.json\n";

fn gates() -> Vec<(String, String)> {
    GATE_COMMANDS
        .into_iter()
        .enumerate()
        .map(|(i, command)| (command.into(), format!("OK: fixture gate {}", i + 1)))
        .collect()
}

fn fixture_gates(root: &Path, workbook: &Path) -> Result<Vec<(String, String)>> {
    assert_eq!(fs::read(workbook).unwrap(), WORKBOOK);
    assert_eq!(fs::read(root.join("ozzit.xlsx")).unwrap(), WORKBOOK);
    assert!(!workbook.starts_with(root));
    Ok(gates())
}

fn names(directory: &Path) -> Vec<String> {
    let mut names = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

type Snapshot = BTreeMap<PathBuf, Option<Vec<u8>>>;

fn snapshot(root: &Path) -> Snapshot {
    fn visit(root: &Path, path: &Path, entries: &mut Snapshot) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.is_symlink());
            let contents = if metadata.is_dir() {
                visit(root, &path, entries);
                None
            } else {
                assert!(metadata.is_file());
                Some(fs::read(&path).unwrap())
            };
            entries.insert(path.strip_prefix(root).unwrap().to_owned(), contents);
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    workbook: PathBuf,
    manifest: PathBuf,
    source: Snapshot,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("repo");
        fs::create_dir(&root).unwrap();
        let workbook = root.join("ozzit.xlsx");
        fs::write(&workbook, WORKBOOK).unwrap();
        let manifest = root.join(BASE_MANIFEST);
        fs::create_dir(manifest.parent().unwrap()).unwrap();
        let base = object(vec![
            (
                "git_blob_sha1",
                Json::str_obj(&git_blob_sha1(&workbook).unwrap()),
            ),
            ("last_workbook_commit", Json::str_obj(&"b".repeat(40))),
            ("path", Json::str_obj("ozzit.xlsx")),
            ("schema", Json::Int(BigInt::from(1))),
            ("sha256", Json::str_obj(&sha256(&workbook).unwrap())),
            ("size", Json::Int(BigInt::from(WORKBOOK.len()))),
        ]);
        fs::write(&manifest, canonical_json(&base)).unwrap();
        let source = snapshot(&root);
        Self {
            directory,
            root,
            workbook,
            manifest,
            source,
        }
    }

    fn output(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }

    fn create_with<G, V>(&self, output: &Path, checks: (G, V)) -> Result<Json>
    where
        G: FnOnce(&Path, &Path) -> Result<Vec<(String, String)>>,
        V: FnOnce(&Path, &Path, bool) -> Result<Json>,
    {
        create_bundle_with(
            &self.root,
            &self.workbook,
            &self.manifest,
            &py::PyPath::new(&output.display().to_string()),
            "3.2.0",
            COMMIT,
            checks,
        )
    }

    fn create(&self, output: &Path) -> Result<Json> {
        self.create_with(output, (fixture_gates, verify_bundle))
    }

    fn assert_source_unchanged(&self) {
        assert_eq!(snapshot(&self.root), self.source);
    }

    fn assert_no_bundle(&self, output: &Path) {
        assert!(output.symlink_metadata().is_err());
        assert_eq!(names(self.directory.path()), ["repo"]);
    }
}

#[test]
fn create_writes_only_the_three_reviewed_assets() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    let document = fixture.create(&output).unwrap();
    assert_eq!(
        names(&output),
        ["SHA256SUMS", "ozzit.xlsx", "provenance.json"]
    );
    let expected = pyjson::loads(PROVENANCE).unwrap();
    assert!(py_eq(&document, &expected));
    assert_eq!(canonical_json(&document), PROVENANCE.as_bytes());
    assert_eq!(
        fs::read(output.join("provenance.json")).unwrap(),
        PROVENANCE.as_bytes()
    );
    assert_eq!(fs::read(output.join("ozzit.xlsx")).unwrap(), WORKBOOK);
    assert!(py_eq(
        &verify_bundle(&output, &fixture.root, false).unwrap(),
        &document
    ));
    assert_eq!(names(fixture.directory.path()), ["bundle", "repo"]);
    fixture.assert_source_unchanged();
}

#[test]
fn checksum_file_covers_workbook_and_provenance_without_a_cycle() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    fixture.create(&output).unwrap();
    let checksums = parse_checksums(&output.join("SHA256SUMS")).unwrap();
    assert_eq!(
        checksums
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        CHECKSUMMED
    );
    for (name, digest) in checksums {
        assert_eq!(sha256(&output.join(name)).unwrap(), digest);
    }
    assert_eq!(fs::read(output.join("SHA256SUMS")).unwrap(), CHECKSUMS);
    fixture.assert_source_unchanged();
}

#[test]
fn two_staged_builds_are_byte_identical() {
    let fixture = Fixture::new();
    let first = fixture.output("first");
    let second = fixture.output("second");
    fixture.create(&first).unwrap();
    fixture.create(&second).unwrap();
    for name in ASSETS {
        assert_eq!(
            fs::read(first.join(name)).unwrap(),
            fs::read(second.join(name)).unwrap(),
            "{name}"
        );
    }
    assert_eq!(names(fixture.directory.path()), ["first", "repo", "second"]);
    fixture.assert_source_unchanged();
}

#[test]
fn existing_output_is_never_replaced() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep.txt"), b"prior verified bundle").unwrap();
    let before = snapshot(fixture.directory.path());
    assert_eq!(
        fixture.create(&output).unwrap_err(),
        format!(
            "release bundle destination already exists: {}",
            output.display()
        )
    );
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn gate_failure_preserves_the_source_and_publishes_nothing() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    let called = Cell::new(false);
    let fail = |root: &Path, workbook: &Path| {
        fixture_gates(root, workbook).unwrap();
        called.set(true);
        Err("gate failed".into())
    };
    assert_eq!(
        fixture
            .create_with(&output, (fail, verify_bundle))
            .unwrap_err(),
        "gate failed"
    );
    assert!(called.get());
    fixture.assert_source_unchanged();
    fixture.assert_no_bundle(&output);
}

#[test]
fn structural_failure_cleans_staging_and_publishes_nothing() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    let called = Cell::new(false);
    let fail = |bundle: &Path, root: &Path, check_semantics: bool| {
        assert!(!check_semantics);
        verify_bundle(bundle, root, check_semantics).unwrap();
        called.set(true);
        Err("structural verification failed".into())
    };
    assert_eq!(
        fixture
            .create_with(&output, (fixture_gates, fail))
            .unwrap_err(),
        "structural verification failed"
    );
    assert!(called.get());
    fixture.assert_no_bundle(&output);
    fixture.assert_source_unchanged();
}

#[test]
fn output_inside_the_source_repository_is_rejected() {
    let fixture = Fixture::new();
    let output = fixture.root.join("bundle");
    assert_eq!(
        fixture.create(&output).unwrap_err(),
        "release bundle output must be outside the source repository"
    );
    fixture.assert_source_unchanged();
    fixture.assert_no_bundle(&output);
}

#[test]
fn base_mismatch_fails_before_staging() {
    let fixture = Fixture::new();
    fs::write(&fixture.workbook, b"changed workbook\n").unwrap();
    let before = snapshot(&fixture.root);
    let output = fixture.output("bundle");
    assert!(
        fixture
            .create(&output)
            .unwrap_err()
            .contains("base workbook")
    );
    assert_eq!(snapshot(&fixture.root), before);
    fixture.assert_no_bundle(&output);
}

#[test]
fn output_parent_must_already_exist() {
    let fixture = Fixture::new();
    let output = fixture.output("missing").join("bundle");
    assert_eq!(
        fixture.create(&output).unwrap_err(),
        "release bundle parent must be an existing non-symlink directory"
    );
    assert!(!output.parent().unwrap().exists());
    fixture.assert_no_bundle(&output);
    fixture.assert_source_unchanged();
}

#[test]
fn destination_created_during_staging_is_not_replaced() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    let called = Cell::new(false);
    let race = |bundle: &Path, root: &Path, check_semantics: bool| {
        assert!(!check_semantics);
        let provenance = verify_bundle(bundle, root, check_semantics)?;
        fs::create_dir(&output).unwrap();
        fs::write(output.join("keep.txt"), b"another writer").unwrap();
        called.set(true);
        Ok(provenance)
    };
    assert_eq!(
        fixture
            .create_with(&output, (fixture_gates, race))
            .unwrap_err(),
        format!(
            "release bundle destination already exists: {}",
            output.display()
        )
    );
    assert!(called.get());
    assert_eq!(names(&output), ["keep.txt"]);
    assert_eq!(
        fs::read(output.join("keep.txt")).unwrap(),
        b"another writer"
    );
    assert_eq!(names(fixture.directory.path()), ["bundle", "repo"]);
    fixture.assert_source_unchanged();
}

#[test]
fn independent_semantic_results_must_match_provenance() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    let document = fixture.create(&output).unwrap();
    let before = snapshot(fixture.directory.path());
    let calls = Cell::new(0);
    let matching = |root: &Path, workbook: &Path| {
        assert_eq!(root, fixture.root);
        assert_eq!(workbook, output.join("ozzit.xlsx"));
        calls.set(calls.get() + 1);
        Ok(gates())
    };
    assert!(py_eq(
        &verify_bundle_with(&output, &fixture.root, true, matching).unwrap(),
        &document
    ));
    assert_eq!(calls.get(), 1);
    let different = |root: &Path, workbook: &Path| {
        assert_eq!(root, fixture.root);
        assert_eq!(workbook, output.join("ozzit.xlsx"));
        calls.set(calls.get() + 1);
        Ok(vec![("same".into(), "different".into())])
    };
    assert_eq!(
        verify_bundle_with(&output, &fixture.root, true, different).unwrap_err(),
        "semantic gate evidence does not match an independent run"
    );
    assert_eq!(calls.get(), 2);
    assert!(py_eq(
        &verify_bundle_with(&output, &fixture.root, false, |_: &Path, _: &Path| {
            panic!("structural-only verification must not run semantic gates")
        })
        .unwrap(),
        &document
    ));
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn gate_evidence_is_independent_of_how_the_workbook_path_is_written() {
    let fixture = Fixture::new();
    let before = snapshot(fixture.directory.path());
    let detoured = fixture.root.join("..").join("repo").join("ozzit.xlsx");
    assert!(
        detoured
            .components()
            .any(|part| part == Component::ParentDir)
    );
    let mut recorded = Vec::new();
    let mut echo = |exe: &str, arguments: Vec<String>, root: &Path| {
        let output = format!("OK: {}\n", arguments[1..].join(" "));
        recorded.push((exe.to_string(), arguments, root.to_path_buf()));
        Ok((0, output))
    };
    let direct = run_semantic_gates_with(&fixture.root, &fixture.workbook, &mut echo).unwrap();
    let indirect = run_semantic_gates_with(&fixture.root, &detoured, &mut echo).unwrap();
    assert_eq!(direct, indirect);
    assert_eq!(direct.len(), 7);
    assert!(
        direct
            .iter()
            .all(|(command, output)| !command.contains("..") && !output.contains(".."))
    );
    let expected_outputs = [
        "OK: ozzit.xlsx",
        "OK: ozzit.xlsx src",
        "OK: src",
        "OK: functions.csv",
        "OK: ozzit.xlsx src functions.csv",
        "OK: ozzit.xlsx functions.csv ./CHANGELOG.md",
        "OK: ozzit.xlsx src",
    ];
    for ((command, output), (label, expected)) in direct
        .iter()
        .zip(GATE_COMMANDS.into_iter().zip(expected_outputs))
    {
        assert_eq!(command, label);
        assert_eq!(output, expected);
    }
    assert_gate_calls(&fixture, &recorded);
    assert_eq!(snapshot(fixture.directory.path()), before);
}

fn assert_gate_calls(fixture: &Fixture, recorded: &[(String, Vec<String>, PathBuf)]) {
    let w = py::resolve(&fixture.workbook).display().to_string();
    let src = fixture.root.join("src").display().to_string();
    let csv = fixture.root.join("functions.csv").display().to_string();
    let changelog = fixture.root.join("CHANGELOG.md").display().to_string();
    let expected_arguments = [
        vec!["verify_workbook".to_string(), w.clone()],
        vec!["verify_sources".into(), w.clone(), src.clone()],
        vec!["verify_signatures".into(), src.clone()],
        vec!["verify_previous_names".into(), csv.clone()],
        vec!["verify_index".into(), w.clone(), src.clone(), csv.clone()],
        vec!["verify_cover".into(), w.clone(), csv, changelog],
        vec!["verify_afe".into(), w, src],
    ];
    assert_eq!(recorded.len(), 14);
    let exe = std::env::current_exe().unwrap();
    for (i, (program, arguments, cwd)) in recorded.iter().enumerate() {
        assert_eq!(Path::new(program), exe);
        assert_eq!(arguments, &expected_arguments[i % 7]);
        assert_eq!(cwd, &fixture.root);
        assert!(
            arguments[1..]
                .iter()
                .all(|argument| Path::new(argument).is_absolute())
        );
    }
}

#[test]
fn failing_gate_reports_its_command_code_and_normalised_output() {
    let fixture = Fixture::new();
    let before = snapshot(fixture.directory.path());
    let calls = Cell::new(0);
    let fail = |exe: &str, arguments: Vec<String>, root: &Path| {
        calls.set(calls.get() + 1);
        assert_eq!(Path::new(exe), std::env::current_exe().unwrap());
        assert_eq!(
            arguments,
            [
                "verify_workbook".to_string(),
                py::resolve(&fixture.workbook).display().to_string()
            ]
        );
        assert_eq!(root, fixture.root);
        Ok((
            1,
            format!(
                "FAIL: {}\r\nsee {}\r\n",
                fixture.workbook.display(),
                fixture.root.join("src").display()
            ),
        ))
    };
    assert_eq!(
        run_semantic_gates_with(&fixture.root, &fixture.workbook, fail).unwrap_err(),
        "python tools/verify_workbook.py ozzit.xlsx failed with exit code 1\nFAIL: ozzit.xlsx\nsee src"
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn silent_failing_gate_reports_only_its_exit_code() {
    let fixture = Fixture::new();
    let before = snapshot(fixture.directory.path());
    let calls = Cell::new(0);
    let fail = |exe: &str, arguments: Vec<String>, root: &Path| {
        calls.set(calls.get() + 1);
        assert_eq!(Path::new(exe), std::env::current_exe().unwrap());
        assert_eq!(
            arguments,
            [
                "verify_workbook".to_string(),
                py::resolve(&fixture.workbook).display().to_string()
            ]
        );
        assert_eq!(root, fixture.root);
        Ok((3, "\r\n".into()))
    };
    assert_eq!(
        run_semantic_gates_with(&fixture.root, &fixture.workbook, fail).unwrap_err(),
        "python tools/verify_workbook.py ozzit.xlsx failed with exit code 3"
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(snapshot(fixture.directory.path()), before);
}

struct FakeGit {
    root: PathBuf,
    answers: BTreeMap<&'static str, String>,
    calls: Vec<Vec<String>>,
}

impl FakeGit {
    fn new(fixture: &Fixture) -> Self {
        Self {
            root: fixture.root.clone(),
            answers: BTreeMap::from([
                ("ls-files", "ozzit.xlsx".into()),
                ("hash-object", git_blob_sha1(&fixture.workbook).unwrap()),
                ("log", "b".repeat(40)),
                ("rev-parse", COMMIT.into()),
                ("status", String::new()),
            ]),
            calls: Vec::new(),
        }
    }

    fn query(&mut self, root: &Path, arguments: &[&str]) -> Result<String> {
        git_output_with(root, arguments, |program, args, cwd, timeout| {
            assert_eq!(program, "git");
            assert_eq!(cwd, Some(self.root.as_path()));
            assert_eq!(timeout, Duration::from_secs(30));
            self.calls.push(args.to_vec());
            Ok(Dump::Exited {
                code: 0,
                stdout: format!("{}\n", self.answers[args[0].as_str()]),
                stderr: String::new(),
            })
        })
    }
}

fn expected_git_queries() -> Vec<Vec<String>> {
    [
        vec!["ls-files", "--error-unmatch", "--", "ozzit.xlsx"],
        vec!["hash-object", "--", "ozzit.xlsx"],
        vec!["log", "-1", "--format=%H", "--", "ozzit.xlsx"],
        vec!["rev-parse", "--verify", "HEAD"],
        vec!["status", "--porcelain", "--untracked-files=all"],
    ]
    .into_iter()
    .map(|args| args.into_iter().map(String::from).collect())
    .collect()
}

#[test]
fn tracked_base_manifest_matches_the_current_workbook() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let shallow = git_output(&root, &["rev-parse", "--is-shallow-repository"]).unwrap();
    if shallow == "true" {
        eprintln!(
            "shallow Git clone: the manifest's last_workbook_commit cannot be checked against truncated history (run git fetch --unshallow)"
        );
        return;
    }
    assert_eq!(shallow, "false");
    let workbook = root.join("ozzit.xlsx");
    let base = verify_base_workbook(&workbook, &root.join(BASE_MANIFEST)).unwrap();
    assert_eq!(
        base.get("sha256").and_then(Json::as_str),
        Some(sha256(&workbook).unwrap().as_str())
    );
    assert!(py_eq(
        base.get("size").unwrap(),
        &Json::Int(BigInt::from(fs::metadata(&workbook).unwrap().len()))
    ));
    verify_repository_base(&root, &base).unwrap();
    println!("REAL_GIT_BINDING: shallow=false workbook=true manifest=true history=true");
}

#[test]
fn git_queries_are_read_only_and_bind_the_manifest() {
    let fixture = Fixture::new();
    let before = snapshot(fixture.directory.path());
    let base = load_json_object(&fixture.manifest).unwrap();
    let mut git = FakeGit::new(&fixture);
    verify_repository_base_with(&fixture.root, &base, |root, args| git.query(root, args)).unwrap();
    assert_eq!(
        current_commit_with(&fixture.root, |root, args| git.query(root, args)).unwrap(),
        COMMIT
    );
    ensure_clean_checkout_with(&fixture.root, |root, args| git.query(root, args)).unwrap();
    assert_eq!(git.calls, expected_git_queries());
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn mismatched_git_claims_are_rejected() {
    let fixture = Fixture::new();
    let before = snapshot(fixture.directory.path());
    let base = load_json_object(&fixture.manifest).unwrap();
    let cases = [
        (
            "ls-files",
            String::new(),
            "base workbook must be tracked as ozzit.xlsx",
        ),
        (
            "hash-object",
            "d".repeat(40),
            "base workbook Git blob does not match the manifest",
        ),
        (
            "log",
            "e".repeat(40),
            "base workbook last-change commit does not match Git history",
        ),
    ];
    for (index, (query, answer, error)) in cases.into_iter().enumerate() {
        let mut git = FakeGit::new(&fixture);
        git.answers.insert(query, answer);
        assert_eq!(
            verify_repository_base_with(&fixture.root, &base, |root, args| git.query(root, args))
                .unwrap_err(),
            error
        );
        assert_eq!(git.calls, expected_git_queries()[..=index]);
        assert_eq!(snapshot(fixture.directory.path()), before);
    }
}

#[test]
fn dirty_checkout_and_bad_head_are_refused() {
    let fixture = Fixture::new();
    let before = snapshot(fixture.directory.path());
    let mut git = FakeGit::new(&fixture);
    git.answers.insert("status", "?? stray.txt".into());
    git.answers.insert("rev-parse", "HEAD".into());
    assert_eq!(
        ensure_clean_checkout_with(&fixture.root, |root, args| git.query(root, args)).unwrap_err(),
        "candidate checkout must be clean before staging a release bundle"
    );
    assert_eq!(
        current_commit_with(&fixture.root, |root, args| git.query(root, args)).unwrap_err(),
        "source commit must be a full 40-character lowercase SHA"
    );
    let expected = expected_git_queries();
    assert_eq!(git.calls, [expected[4].clone(), expected[3].clone()]);
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn git_output_runs_a_checked_utf8_query_and_trims_it() {
    let root = Path::new("checkout");
    let before = std::env::current_dir().unwrap();
    let calls = Cell::new(0);
    let output = git_output_with(
        root,
        &["rev-parse", "HEAD"],
        |program, args, cwd, timeout| {
            calls.set(calls.get() + 1);
            assert_eq!(program, "git");
            assert_eq!(args, ["rev-parse", "HEAD"]);
            assert_eq!(cwd, Some(root));
            assert!(!root.is_absolute());
            assert_eq!(timeout, Duration::from_secs(30));
            Ok(Dump::Exited {
                code: 0,
                stdout: "  answer\n".into(),
                stderr: String::new(),
            })
        },
    );
    assert_eq!(output.unwrap(), "answer");
    assert_eq!(calls.get(), 1);
    assert_eq!(std::env::current_dir().unwrap(), before);
}

#[test]
fn failed_git_query_propagates() {
    let root = Path::new("checkout");
    let calls = Cell::new(0);
    for failure in [
        Err("runner failed".into()),
        Ok(Dump::Exited {
            code: 128,
            stdout: String::new(),
            stderr: "Git failed".into(),
        }),
    ] {
        let expected = if failure.is_err() {
            "runner failed"
        } else {
            "Command '['git', 'status']' returned non-zero exit status 128."
        };
        assert_eq!(
            git_output_with(root, &["status"], |program, args, cwd, timeout| {
                calls.set(calls.get() + 1);
                assert_eq!(program, "git");
                assert_eq!(args, ["status"]);
                assert_eq!(cwd, Some(root));
                assert_eq!(timeout, Duration::from_secs(30));
                failure
            })
            .unwrap_err(),
            expected
        );
    }
    assert_eq!(calls.get(), 2);
}

fn command_output(fixture: &Fixture) -> PathBuf {
    let parent = fixture.output("out");
    fs::create_dir(&parent).unwrap();
    parent.join("bundle")
}

fn command_arguments(command: &str, output: &Path, extra: &[&str]) -> Vec<String> {
    let mut args = if command == "create" {
        vec!["create", "--version", "3.2.0", "--output"]
    } else {
        vec!["verify", "--bundle"]
    }
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    args.push(output.display().to_string());
    args.extend(extra.iter().map(|s| s.to_string()));
    args
}

fn build_fixture_bundle(
    root: &Path,
    workbook: &Path,
    manifest: &Path,
    output: &py::PyPath,
    version: &str,
    commit: &str,
) -> Result<Json> {
    create_bundle_with(
        root,
        workbook,
        manifest,
        output,
        version,
        commit,
        (fixture_gates, verify_bundle),
    )
}

fn verify_fixture_bundle(bundle: &Path, root: &Path, semantics: bool) -> Result<Json> {
    verify_bundle_with(bundle, root, semantics, fixture_gates)
}

fn fixture_run<C, V>(
    fixture: &Fixture,
    arguments: &Arguments,
    git: &mut FakeGit,
    build: C,
    verify: V,
) -> Result<String>
where
    C: FnOnce(&Path, &Path, &Path, &py::PyPath, &str, &str) -> Result<Json>,
    V: FnOnce(&Path, &Path, bool) -> Result<Json>,
{
    let git = std::cell::RefCell::new(git);
    run_with(
        arguments,
        fixture.root.clone(),
        (
            |root: &Path| ensure_clean_checkout_with(root, |r, a| git.borrow_mut().query(r, a)),
            |root: &Path| current_commit_with(root, |r, a| git.borrow_mut().query(r, a)),
            |root: &Path, base: &Json| {
                verify_repository_base_with(root, base, |r, a| git.borrow_mut().query(r, a))
            },
        ),
        (
            |args: &Arguments, root: &Path, head: String| run_create_with(args, root, head, build),
            |args: &Arguments, root: &Path, head: &str| run_verify_with(args, root, head, verify),
        ),
    )
}

fn fixture_command<C, V>(
    fixture: &Fixture,
    arguments: &[String],
    git: &mut FakeGit,
    build: C,
    verify: V,
) -> Output
where
    C: FnOnce(&Path, &Path, &Path, &py::PyPath, &str, &str) -> Result<Json>,
    V: FnOnce(&Path, &Path, bool) -> Result<Json>,
{
    command_with(arguments, |args| {
        fixture_run(fixture, args, git, build, verify)
    })
}

fn expected_command_queries() -> Vec<Vec<String>> {
    let expected = expected_git_queries();
    [4, 3, 0, 1, 2].map(|index| expected[index].clone()).into()
}

fn assert_command_success(output: Output, line: &str) {
    assert_eq!(output.code, 0);
    assert_eq!(output.out, [line]);
    assert_eq!(output.out.join("\n") + "\n", format!("{line}\n"));
    assert!(output.err.is_empty());
}

fn assert_fixture_refusal(fixture: &Fixture, arguments: &[String], git: &mut FakeGit, error: &str) {
    let before = snapshot(fixture.directory.path());
    let parsed = parse(arguments).unwrap();
    let verifications = Cell::new(0);
    let gates_run = Cell::new(0);
    let verify = |bundle: &Path, root: &Path, semantics: bool| {
        assert!(semantics);
        verifications.set(verifications.get() + 1);
        verify_bundle_with(bundle, root, semantics, |r, w| {
            gates_run.set(gates_run.get() + 1);
            fixture_gates(r, w)
        })
    };
    assert_eq!(
        fixture_run(
            fixture,
            &parsed,
            git,
            |_, _, _, _, _, _| panic!("refusal must not build"),
            verify
        )
        .unwrap_err(),
        error
    );
    let calls = std::mem::take(&mut git.calls);
    let result = fixture_command(
        fixture,
        arguments,
        git,
        |_, _, _, _, _, _| panic!("refusal must not build"),
        verify,
    );
    assert_eq!(result.code, 1);
    assert!(result.out.is_empty());
    assert_eq!(result.err, [format!("ERROR: {error}")]);
    assert_eq!(git.calls, calls);
    let expected = if parsed.command == "verify" { 2 } else { 0 };
    assert_eq!(verifications.get(), expected);
    assert_eq!(gates_run.get(), expected);
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn create_stages_from_the_clean_head() {
    for source in [vec![], vec!["--source-commit", COMMIT]] {
        let fixture = Fixture::new();
        let output = command_output(&fixture);
        let arguments = command_arguments("create", &output, &source);
        let mut git = FakeGit::new(&fixture);
        let builds = Cell::new(0);
        let build = |root: &Path,
                     workbook: &Path,
                     manifest: &Path,
                     out: &py::PyPath,
                     version: &str,
                     commit: &str| {
            builds.set(builds.get() + 1);
            assert_eq!(root, fixture.root);
            assert_eq!(workbook, fixture.workbook);
            assert_eq!(manifest, fixture.manifest);
            assert_eq!(out.fs(), output);
            assert_eq!(version, "3.2.0");
            assert_eq!(commit, COMMIT);
            build_fixture_bundle(root, workbook, manifest, out, version, commit)
        };
        let result = fixture_command(&fixture, &arguments, &mut git, build, |_, _, _| {
            panic!("create branch must not verify through command")
        });
        assert_command_success(
            result,
            &format!("OK: created Ozzit 3.2.0 bundle at {}", output.display()),
        );
        assert_eq!(builds.get(), 1);
        assert_eq!(git.calls, expected_command_queries());
        let provenance = load_json_object(&output.join("provenance.json")).unwrap();
        assert_eq!(
            provenance
                .get("source")
                .and_then(|s| s.get("commit"))
                .and_then(Json::as_str),
            Some(COMMIT)
        );
        assert_eq!(
            fs::read(output.join("provenance.json")).unwrap(),
            PROVENANCE.as_bytes()
        );
        assert_eq!(fs::read(output.join("SHA256SUMS")).unwrap(), CHECKSUMS);
        assert_eq!(fs::read(output.join("ozzit.xlsx")).unwrap(), WORKBOOK);
        fixture.assert_source_unchanged();
    }
}

#[test]
fn create_refuses_a_source_commit_other_than_head() {
    let fixture = Fixture::new();
    let output = command_output(&fixture);
    let other = "c".repeat(40);
    let arguments = command_arguments("create", &output, &["--source-commit", &other]);
    let mut git = FakeGit::new(&fixture);
    assert_fixture_refusal(
        &fixture,
        &arguments,
        &mut git,
        "source commit must equal the candidate checkout HEAD",
    );
    assert!(!output.exists());
    assert_eq!(git.calls, expected_command_queries());
}

#[test]
fn create_refuses_a_dirty_checkout() {
    let fixture = Fixture::new();
    let output = command_output(&fixture);
    let arguments = command_arguments("create", &output, &[]);
    let mut git = FakeGit::new(&fixture);
    git.answers.insert("status", " M ozzit.xlsx".into());
    assert_fixture_refusal(
        &fixture,
        &arguments,
        &mut git,
        "candidate checkout must be clean before staging a release bundle",
    );
    assert_eq!(git.calls, [expected_git_queries()[4].clone()]);
    assert!(!output.exists());
}

#[test]
fn verify_accepts_a_bundle_matching_head_and_the_requested_claims() {
    let fixture = Fixture::new();
    let output = command_output(&fixture);
    fixture.create(&output).unwrap();
    let before = snapshot(fixture.directory.path());
    for extra in [
        vec![],
        vec!["--version", "3.2.0"],
        vec!["--source-commit", COMMIT],
    ] {
        let arguments = command_arguments("verify", &output, &extra);
        let mut git = FakeGit::new(&fixture);
        let result = fixture_command(
            &fixture,
            &arguments,
            &mut git,
            |_, _, _, _, _, _| panic!("verify branch must not build"),
            verify_fixture_bundle,
        );
        assert_command_success(
            result,
            &format!("OK: verified Ozzit 3.2.0 bundle at {}", output.display()),
        );
        assert_eq!(git.calls, expected_command_queries());
        assert_eq!(snapshot(fixture.directory.path()), before);
    }
}

#[test]
fn verify_rejects_mismatched_claims() {
    let fixture = Fixture::new();
    let output = command_output(&fixture);
    fixture.create(&output).unwrap();
    let other = "c".repeat(40);
    let cases = [
        (
            Some(other.as_str()),
            vec![],
            "bundle source commit does not match the candidate checkout HEAD",
        ),
        (
            None,
            vec!["--version", "3.2.1"],
            "bundle version does not match --version",
        ),
        (
            None,
            vec!["--version", "v3.2.0"],
            "version must be canonical X.Y.Z without a v prefix",
        ),
        (
            None,
            vec!["--source-commit", other.as_str()],
            "bundle source commit does not match --source-commit",
        ),
    ];
    for (head, extra, error) in cases {
        let mut git = FakeGit::new(&fixture);
        if let Some(head) = head {
            git.answers.insert("rev-parse", head.into());
        }
        let arguments = command_arguments("verify", &output, &extra);
        assert_fixture_refusal(&fixture, &arguments, &mut git, error);
        assert_eq!(git.calls, expected_command_queries());
    }
}

#[test]
fn verify_reruns_the_semantic_gates() {
    let fixture = Fixture::new();
    let output = command_output(&fixture);
    fixture.create(&output).unwrap();
    let before = snapshot(fixture.directory.path());
    let arguments = command_arguments("verify", &output, &[]);
    let mut git = FakeGit::new(&fixture);
    let verifications = Cell::new(0);
    let gates_run = Cell::new(0);
    let verify = |bundle: &Path, root: &Path, semantics: bool| {
        assert_eq!(bundle, output);
        assert_eq!(root, fixture.root);
        assert!(semantics);
        verifications.set(verifications.get() + 1);
        verify_bundle_with(bundle, root, semantics, |r, w| {
            assert_eq!(r, fixture.root);
            assert_eq!(w, output.join("ozzit.xlsx"));
            gates_run.set(gates_run.get() + 1);
            fixture_gates(r, w)
        })
    };
    let result = fixture_command(
        &fixture,
        &arguments,
        &mut git,
        |_, _, _, _, _, _| panic!("verify branch must not build"),
        verify,
    );
    assert_command_success(
        result,
        &format!("OK: verified Ozzit 3.2.0 bundle at {}", output.display()),
    );
    assert_eq!(verifications.get(), 1);
    assert_eq!(gates_run.get(), 1);
    assert_eq!(git.calls, expected_command_queries());
    assert_eq!(snapshot(fixture.directory.path()), before);
}

fn rewrite_static_provenance(bundle: &Path, change: impl FnOnce(&mut serde_json::Value)) {
    let path = bundle.join("provenance.json");
    let mut document = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    change(&mut document);
    let document = pyjson::loads(&document.to_string()).unwrap();
    fs::write(path, canonical_json(&document)).unwrap();
    let checksums = CHECKSUMMED
        .iter()
        .map(|name| ((*name).into(), sha256(&bundle.join(name)).unwrap()))
        .collect::<Vec<_>>();
    fs::write(bundle.join("SHA256SUMS"), checksum_bytes(&checksums)).unwrap();
}

fn assert_static_refusal(fixture: &Fixture, bundle: &Path, expected: &str) {
    let before = snapshot(fixture.directory.path());
    assert_eq!(
        verify_bundle(bundle, &fixture.root, false).unwrap_err(),
        expected
    );
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn falsified_native_gate_claim_is_rejected() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    fixture.create(&output).unwrap();
    rewrite_static_provenance(&output, |document| {
        document["verification"]["native_gates"]["included"] = serde_json::json!(true);
    });
    assert_static_refusal(
        &fixture,
        &output,
        "provenance verification claims are invalid",
    );
    fixture.assert_source_unchanged();
}

#[test]
fn unexpected_top_level_provenance_claim_is_rejected() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    fixture.create(&output).unwrap();
    rewrite_static_provenance(&output, |document| {
        document["independently_verified"] = serde_json::json!(true);
    });
    assert_static_refusal(
        &fixture,
        &output,
        "provenance.json has an unexpected top-level schema",
    );
    fixture.assert_source_unchanged();
}

#[test]
fn version_and_commit_reject_all_original_noncanonical_values() {
    for version in ["v3.2.0", "03.2.0", "3.2", "3.2.0; touch pwned"] {
        assert_eq!(
            validate_version(version).unwrap_err(),
            "version must be canonical X.Y.Z without a v prefix"
        );
    }
    for commit in [
        "main".into(),
        "a".repeat(39),
        "A".repeat(40),
        "a".repeat(40) + ";",
    ] {
        assert_eq!(
            validate_source_commit(&commit).unwrap_err(),
            "source commit must be a full 40-character lowercase SHA"
        );
    }
}

#[test]
fn malformed_manifest_json_reports_its_file_name() {
    let fixture = Fixture::new();
    for data in [b"{not json".as_slice(), b"{\"path\": \"\xff\"}"] {
        fs::write(&fixture.manifest, data).unwrap();
        let before = snapshot(fixture.directory.path());
        let error = verify_base_workbook(&fixture.workbook, &fixture.manifest).unwrap_err();
        assert!(
            error.starts_with("invalid JSON in workbook-base.json: "),
            "{error}"
        );
        assert_eq!(snapshot(fixture.directory.path()), before);
    }
}

#[test]
fn missing_json_file_is_reported_as_invalid() {
    let fixture = Fixture::new();
    let before = snapshot(fixture.directory.path());
    let error = load_json_object(&fixture.output("absent.json")).unwrap_err();
    assert!(
        error.starts_with("invalid JSON in absent.json: "),
        "{error}"
    );
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn workbook_and_manifest_must_be_regular_files() {
    let fixture = Fixture::new();
    let folder = fixture.output("folder");
    fs::create_dir(&folder).unwrap();
    let before = snapshot(fixture.directory.path());
    for (workbook, manifest, error) in [
        (
            &folder,
            &fixture.manifest,
            "base workbook must be a regular non-symlink file",
        ),
        (
            &fixture.workbook,
            &folder,
            "base workbook manifest must be a regular non-symlink file",
        ),
    ] {
        assert_eq!(verify_base_workbook(workbook, manifest).unwrap_err(), error);
        assert_eq!(snapshot(fixture.directory.path()), before);
    }
}

#[test]
fn manifest_fields_reject_the_remaining_original_values() {
    let fixture = Fixture::new();
    let pristine: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.manifest).unwrap()).unwrap();
    // Extra/schema/zero-size cases are retained in corpus cases 75/63/62/65.
    for (field, value, message) in [
        (
            "path",
            serde_json::json!("other.xlsx"),
            "path must be ozzit.xlsx",
        ),
        (
            "size",
            serde_json::json!("27"),
            "size must be a positive integer",
        ),
        (
            "sha256",
            serde_json::json!("A".repeat(64)),
            "SHA-256 is invalid",
        ),
        ("sha256", serde_json::json!(7), "SHA-256 is invalid"),
        (
            "git_blob_sha1",
            serde_json::json!("c".repeat(39)),
            "Git blob SHA-1 is invalid",
        ),
        (
            "git_blob_sha1",
            serde_json::Value::Null,
            "Git blob SHA-1 is invalid",
        ),
        (
            "last_workbook_commit",
            serde_json::json!(5),
            "last commit is invalid",
        ),
    ] {
        let mut document = pristine.clone();
        document[field] = value;
        fs::write(&fixture.manifest, document.to_string()).unwrap();
        let before = snapshot(fixture.directory.path());
        assert_eq!(
            verify_base_workbook(&fixture.workbook, &fixture.manifest).unwrap_err(),
            format!("base workbook manifest {message}")
        );
        assert_eq!(snapshot(fixture.directory.path()), before);
    }
}

#[test]
fn unreadable_checksum_files_are_rejected() {
    let fixture = Fixture::new();
    let path = fixture.output("SHA256SUMS");
    let before = snapshot(fixture.directory.path());
    let error = parse_checksums(&path).unwrap_err();
    assert!(error.starts_with("invalid SHA256SUMS: "), "{error}");
    assert_eq!(snapshot(fixture.directory.path()), before);
    fs::write(&path, "é\n").unwrap();
    let before = snapshot(fixture.directory.path());
    let error = parse_checksums(&path).unwrap_err();
    assert!(error.starts_with("invalid SHA256SUMS: "), "{error}");
    assert!(error.contains("ascii"), "{error}");
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn malformed_checksum_files_reject_all_original_rows() {
    let fixture = Fixture::new();
    let path = fixture.output("SHA256SUMS");
    let digest = "f".repeat(64);
    for (data, message) in [
        (
            format!("{digest}  ozzit.xlsx"),
            "must use canonical LF-terminated lines",
        ),
        (
            format!("{digest}  ozzit.xlsx\r\n"),
            "must use canonical LF-terminated lines",
        ),
        (
            format!("{digest} ozzit.xlsx\n"),
            "contains a malformed line",
        ),
        (
            format!("{digest}  ozzit.xlsx\n{digest}  ozzit.xlsx\n"),
            "repeats ozzit.xlsx",
        ),
        (
            format!("{digest}  ozzit.xlsx\n"),
            "has an unexpected inventory",
        ),
        (
            format!("{digest}  provenance.json\n{digest}  ozzit.xlsx\n"),
            "is not canonically ordered",
        ),
    ] {
        fs::write(&path, data).unwrap();
        let before = snapshot(fixture.directory.path());
        assert_eq!(
            parse_checksums(&path).unwrap_err(),
            format!("SHA256SUMS {message}")
        );
        assert_eq!(snapshot(fixture.directory.path()), before);
    }
}

#[test]
fn canonical_checksum_file_returns_both_original_digests() {
    let fixture = Fixture::new();
    let path = fixture.output("SHA256SUMS");
    let digest = "f".repeat(64);
    fs::write(
        &path,
        format!("{digest}  ozzit.xlsx\n{digest}  provenance.json\n"),
    )
    .unwrap();
    let before = snapshot(fixture.directory.path());
    assert_eq!(
        parse_checksums(&path).unwrap(),
        [
            ("ozzit.xlsx".into(), digest.clone()),
            ("provenance.json".into(), digest)
        ]
    );
    assert_eq!(snapshot(fixture.directory.path()), before);
}

#[test]
fn bundle_must_be_a_directory_containing_only_regular_files() {
    let fixture = Fixture::new();
    let output = fixture.output("bundle");
    fixture.create(&output).unwrap();
    assert_static_refusal(
        &fixture,
        &output.join("SHA256SUMS"),
        "bundle must be a regular non-symlink directory",
    );
    fs::create_dir(output.join("nested")).unwrap();
    assert_static_refusal(
        &fixture,
        &output,
        "bundle inventory may contain only regular non-symlink files",
    );
    fixture.assert_source_unchanged();
}

#[test]
fn parser_rejects_the_remaining_original_incomplete_commands() {
    // Empty argv and ["verify"] remain in argparse corpus cases 0 and 8.
    for args in [vec!["publish"], vec!["create", "--version", "3.2.0"]] {
        let args = args.into_iter().map(String::from).collect::<Vec<_>>();
        let output = command_with(&args, |_| panic!("parse failure must not execute"));
        assert_eq!(output.code, 2);
        assert!(output.out.is_empty());
        assert!(!output.err.is_empty());
    }
}
