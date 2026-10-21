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
