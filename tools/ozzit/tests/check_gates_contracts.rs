//! Physical CI-helper commands on owned trees and real first-parent Git history.
use std::{collections::BTreeMap, fs, path::Path, process::Command};

const WORKFLOW: &str = ".github/workflows/current.yml";
const GATE: &str = "aggregate";
fn workflow(jobs: &[&str], comments: &str) -> String {
    format!(
        "name: fixture\njobs:\n{}{comments}",
        jobs.iter().map(|j| format!("  {j}:\n")).collect::<String>()
    )
}
fn snapshot(root: &Path) -> BTreeMap<std::path::PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, path: &Path, result: &mut BTreeMap<std::path::PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let metadata = path.symlink_metadata().unwrap();
            let bytes = if metadata.is_dir() {
                None
            } else if metadata.is_symlink() {
                Some(
                    fs::read_link(&path)
                        .unwrap()
                        .as_os_str()
                        .as_encoded_bytes()
                        .to_vec(),
                )
            } else {
                Some(fs::read(&path).unwrap())
            };
            result.insert(path.strip_prefix(root).unwrap().into(), bytes);
            if metadata.is_dir() {
                visit(root, &path, result);
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}
struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new() -> Self {
        let result = Self(tempfile::tempdir().unwrap());
        fs::create_dir_all(result.root().join(".github/workflows")).unwrap();
        result
    }
    fn root(&self) -> &Path {
        self.0.path()
    }
    fn write(&self, path: &str, text: &str) {
        fs::write(self.root().join(path), text).unwrap();
    }
    fn git(&self, args: &[&str]) {
        let result = Command::new("git")
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.autocrlf=false",
                // A commit otherwise starts a detached `git maintenance run
                // --auto` (Unix), which can touch .git during a snapshot.
                "-c",
                "maintenance.auto=false",
                "-c",
            ])
            .arg(format!(
                "core.hooksPath={}",
                self.root().join("empty-hooks").display()
            ))
            .args(args)
            .current_dir(self.root())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fn commit(&self) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--allow-empty", "-m", "fixture"]);
    }
    fn base(files: &[(&str, String)]) -> Self {
        let f = Self::new();
        fs::create_dir(f.root().join("empty-hooks")).unwrap();
        f.git(&["init", "--quiet"]);
        for (path, text) in files {
            f.write(path, text);
        }
        f.commit();
        f
    }
    fn run(
        &self,
        path: &str,
        extra: &[&str],
        results: Option<&str>,
        event: &str,
    ) -> (i32, String, String) {
        let before = snapshot(self.root());
        let mut command = Command::new(env!("CARGO_BIN_EXE_ozzit"));
        command
            .args(["check_gates", path, GATE])
            .args(extra)
            .current_dir(self.root())
            .env("GITHUB_EVENT_NAME", event)
            .env_remove("RESULTS");
        if let Some(results) = results {
            command.env("RESULTS", results);
        }
        let output = command.output().unwrap();
        assert_eq!(
            snapshot(self.root()),
            before,
            "check_gates changed an input"
        );
        // Decoding is strict; platform line endings are checked before normalising.
        let out = String::from_utf8(output.stdout).unwrap();
        let err = String::from_utf8(output.stderr).unwrap();
        if cfg!(windows) {
            for text in [&out, &err] {
                assert!(!text.replace("\r\n", "").contains('\n'));
            }
        } else {
            assert!(!out.contains('\r'));
            assert!(!err.contains('\r'));
        }
        (
            output.status.code().unwrap(),
            out.replace("\r\n", "\n"),
            err.replace("\r\n", "\n"),
        )
    }
    fn pr(&self) -> (i32, String, String) {
        self.run(
            WORKFLOW,
            &[],
            Some(r#"{"lint":{"result":"success"}}"#),
            "pull_request",
        )
    }
}

#[test]
fn script_status_channels_and_owned_input_preservation() {
    let f = Fixture::new();
    f.write(WORKFLOW, &workflow(&[GATE, "lint"], ""));
    for (status, extra, code, err) in [
        ("success", vec![], 0, ""),
        ("failure", vec![], 1, "Jobs did not succeed: lint\n"),
        ("failure", vec!["--no-results"], 0, ""),
    ] {
        let results = format!(r#"{{"lint":{{"result":"{status}"}}}}"#);
        assert_eq!(
            f.run(WORKFLOW, &extra, Some(&results), "push"),
            (code, "".into(), err.into())
        );
    }
}
#[test]
fn same_path_real_git_and_sorted_mixed_removals() {
    let f = Fixture::base(&[(
        WORKFLOW,
        workflow(&[GATE, "lint", "zeta", "alpha", "beta"], ""),
    )]);
    f.write(
        WORKFLOW,
        &workflow(&[GATE, "lint"], "# removed-jobs: zeta,beta\n"),
    );
    f.commit();
    assert_eq!(
        f.pr(),
        (
            1,
            format!("Declared removals from {WORKFLOW}: beta, zeta\n"),
            format!(
                "These jobs were removed from {WORKFLOW}: alpha. If that is intended, add the comment '# removed-jobs: alpha' to that file.\n"
            )
        )
    );
}
#[test]
fn same_path_wins_over_another_gate_workflow() {
    let text = workflow(&[GATE, "lint"], "");
    let f = Fixture::base(&[
        (WORKFLOW, text.clone()),
        (".github/workflows/other.yml", text),
    ]);
    f.commit();
    assert_eq!(f.pr(), (0, "".into(), "".into()));
}
#[test]
fn renamed_workflow_checks_its_single_predecessor() {
    let old = ".github/workflows/old.yaml";
    let f = Fixture::base(&[(old, workflow(&[GATE, "lint", "old"], ""))]);
    fs::remove_file(f.root().join(old)).unwrap();
    f.write(
        WORKFLOW,
        &workflow(&[GATE, "lint"], "# removed-jobs: old\n"),
    );
    f.commit();
    assert_eq!(
        f.pr(),
        (
            0,
            format!("Declared removals from {WORKFLOW}: old\n"),
            "".into()
        )
    );
}
#[test]
fn entirely_new_workflow_has_no_predecessor() {
    let f = Fixture::base(&[(".github/workflows/other.yml", workflow(&["other"], ""))]);
    f.write(WORKFLOW, &workflow(&[GATE, "lint"], ""));
    f.commit();
    assert_eq!(f.pr(), (0, "".into(), "".into()));
}
#[test]
fn ambiguous_real_predecessors_refuse() {
    let text = workflow(&[GATE, "lint"], "");
    let f = Fixture::base(&[
        (".github/workflows/a.yml", text.clone()),
        (".github/workflows/b.yaml", text.clone()),
    ]);
    f.write(WORKFLOW, &text);
    f.commit();
    assert_eq!(f.pr(), (1, "".into(), "More than one base workflow has a job named aggregate; the gate check needs updating\n".into()));
}
#[test]
fn missing_parent_is_a_controlled_failure() {
    let f = Fixture::base(&[(WORKFLOW, workflow(&[GATE, "lint"], ""))]);
    let (code, out, err) = f.pr();
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.starts_with("CalledProcessError:"), "{err}");
}
#[test]
fn utf8_and_crlf_workflow_and_git_text_are_normalised() {
    let text = workflow(&[GATE, "lint"], "# français 日本語\n").replace('\n', "\r\n");
    let f = Fixture::base(&[(WORKFLOW, text)]);
    f.commit();
    assert_eq!(f.pr(), (0, "".into(), "".into()));
    f.write(
        WORKFLOW,
        "name: fixture\r\njobs:\r\n  aggregate:\r\n  é:\r\n",
    );
    assert_eq!(
        f.run(WORKFLOW, &[], Some("{}"), "push"),
        (
            1,
            "".into(),
            "Cannot read the job key '  é:'; the gate check needs updating\n".into()
        )
    );
}
#[test]
fn malformed_json_and_missing_results_refuse_even_with_no_results() {
    let f = Fixture::new();
    for results in [None, Some("not JSON")] {
        let (code, out, err) = f.run(WORKFLOW, &["--no-results"], results, "push");
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert!(err.starts_with(if results.is_none() {
            "KeyError:"
        } else {
            "JSONDecodeError:"
        }));
    }
}
#[test]
fn accepted_extension_and_invalid_utf8_follow_the_native_policy() {
    let f = Fixture::new();
    let path = ".github/workflows/notes.txt";
    f.write(path, &workflow(&[GATE], ""));
    assert_eq!(
        f.run(path, &[], Some("{}"), "push"),
        (0, "".into(), "".into())
    );
    fs::write(f.root().join(path), b"name: \xff\njobs:\n  aggregate:\n").unwrap();
    let (code, out, err) = f.run(path, &[], Some("{}"), "push");
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.starts_with("UnicodeDecodeError:"), "{err}");
}
#[cfg(unix)]
#[test]
fn workflow_entry_follows_a_file_symlink() {
    let f = Fixture::new();
    f.write("target.txt", &workflow(&[GATE], ""));
    std::os::unix::fs::symlink("../../target.txt", f.root().join(WORKFLOW)).unwrap();
    assert_eq!(
        f.run(WORKFLOW, &[], Some("{}"), "push"),
        (0, "".into(), "".into())
    );
}

#[cfg(unix)]
#[test]
fn git_process_inherits_stdin() {
    use std::{io::Write, os::unix::fs::PermissionsExt, process::Stdio};
    let f = Fixture::new();
    f.write(WORKFLOW, &workflow(&[GATE], ""));
    let bin = f.root().join("bin");
    fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("git");
    // Every query reads a line from the parent's stdin. Empty ls-tree replies
    // mean a new workflow; a closed stdin must make the helper fail.
    fs::write(
        &wrapper,
        "#!/bin/sh\nread -r token || exit 91\n[ \"$token\" = inherited ] || exit 92\n",
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let before = snapshot(f.root());
    let mut child = Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .args(["check_gates", WORKFLOW, GATE])
        .current_dir(f.root())
        .env("RESULTS", "{}")
        .env("GITHUB_EVENT_NAME", "pull_request")
        .env("PATH", bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"inherited\ninherited\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(snapshot(f.root()), before);
}

// Live aggregate callers retain their required context and process contracts.
const CALLERS: [(&str, &str, &str, &str); 2] = [
    (
        "verify.yml",
        "verify-gates",
        "needs: [lint, verify, rust, coverage]",
        r#"{"lint":{"result":"success"},"verify":{"result":"success"},"rust":{"result":"success"},"coverage":{"result":"success"}}"#,
    ),
    (
        "codeql.yml",
        "codeql-gates",
        "needs: [analyze]",
        r#"{"analyze":{"result":"success"}}"#,
    ),
];
fn repository() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}
fn aggregate_section<'a>(text: &'a str, gate: &str) -> &'a str {
    let (_, section) = text.split_once(&format!("\n  {gate}:\n")).unwrap();
    let mut end = 0;
    for line in section.split_inclusive('\n') {
        if line.starts_with("  ")
            && line
                .as_bytes()
                .get(2)
                .is_some_and(|b| !b.is_ascii_whitespace() && *b != b'#')
        {
            break;
        }
        end += line.len();
    }
    &section[..end]
}
fn native_invocation(section: &str) -> &str {
    section
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("run: tools/ozzit/target/debug/ozzit "))
        .unwrap()
}
fn assert_no_extra_workflow_capabilities(section: &str, gate: &str) {
    for forbidden in [
        "setup-python",
        "sparse-checkout",
        "continue-on-error",
        "id-token:",
        "security-events:",
        "upload-artifact",
        "download-artifact",
        "--release",
    ] {
        assert!(
            !section.contains(forbidden),
            "{gate}: unexpected {forbidden}"
        );
    }
}

#[test]
fn aggregate_workflows_build_the_pinned_native_gate_without_extra_privileges() {
    for (file, gate, needs, _) in CALLERS {
        let source = fs::read_to_string(repository().join(".github/workflows").join(file)).unwrap();
        let section = aggregate_section(&source, gate);
        for line in [
            needs,
            "if: always()",
            "runs-on: ubuntu-latest",
            "timeout-minutes: 20",
            "permissions:\n      contents: read",
            "persist-credentials: false",
            "fetch-depth: 2",
            "RESULTS: ${{ toJSON(needs) }}",
        ] {
            assert!(section.contains(line), "{gate}: missing {line}");
        }
        assert_no_extra_workflow_capabilities(section, gate);
        let checkout = section
            .find("uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1")
            .unwrap();
        let install = section.find("run: rustup toolchain install").unwrap();
        let build = section
            .find("run: cargo build --locked --manifest-path tools/ozzit/Cargo.toml")
            .unwrap();
        let invocation = native_invocation(section);
        let run = section.find(invocation).unwrap();
        assert!(checkout < install && install < build && build < run);
        let extra = if gate == "verify-gates" {
            " --exempt codecov"
        } else {
            ""
        };
        assert_eq!(
            invocation,
            format!("check_gates .github/workflows/{file} {gate}{extra}")
        );
    }
    assert!(!repository().join(".github/ci/check_gates.py").exists());
    assert!(
        !repository()
            .join("tools/tests/test_check_gates.py")
            .exists()
    );
}

fn caller_output(
    root: &Path,
    invocation: &str,
    results: &serde_json::Value,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .args(invocation.split_whitespace())
        .current_dir(root)
        .env("RESULTS", results.to_string())
        .env("GITHUB_EVENT_NAME", "pull_request")
        .output()
        .unwrap()
}

#[test]
fn actual_workflow_invocations_check_success_failure_and_missing_needs() {
    let files = CALLERS.map(|(file, _, _, _)| {
        (
            format!(".github/workflows/{file}"),
            fs::read_to_string(repository().join(".github/workflows").join(file)).unwrap(),
        )
    });
    let fixture = Fixture::base(
        &files
            .iter()
            .map(|(p, text)| (p.as_str(), text.clone()))
            .collect::<Vec<_>>(),
    );
    fixture.commit();
    let before = snapshot(fixture.root());
    for ((_, gate, _, results), (_, text)) in CALLERS.into_iter().zip(&files) {
        let invocation = native_invocation(aggregate_section(text, gate));
        let mut results: serde_json::Value = serde_json::from_str(results).unwrap();
        let name = if gate == "verify-gates" {
            "lint"
        } else {
            "analyze"
        };
        for state in ["success", "failure", "missing"] {
            if state == "missing" {
                results.as_object_mut().unwrap().remove(name);
            } else {
                results[name]["result"] = state.into();
            }
            let output = caller_output(fixture.root(), invocation, &results);
            assert_eq!(output.status.code(), Some(i32::from(state != "success")));
            assert!(output.stdout.is_empty());
            let expected = match state {
                "success" => String::new(),
                "failure" => format!("Jobs did not succeed: {name}\n"),
                _ => format!("Add these jobs to needs: {name}\n"),
            };
            assert_eq!(
                String::from_utf8(output.stderr)
                    .unwrap()
                    .replace("\r\n", "\n"),
                expected
            );
            assert_eq!(snapshot(fixture.root()), before);
        }
    }
}
