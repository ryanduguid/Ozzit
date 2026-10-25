//! Assertion sets transferred from tools/tests/test_check_gates.py.
//! Exception identities adapt to controlled errors; process launch is also tested physically.
use super::*;
use std::{cell::RefCell, collections::VecDeque};

const WORKFLOW: &str = ".github/workflows/current.yml";
const GATE: &str = "aggregate";
fn workflow(jobs: &[&str], comments: &str) -> String {
    format!(
        "name: fixture\njobs:\n{}{comments}",
        jobs.iter().map(|j| format!("  {j}:\n")).collect::<String>()
    )
}
fn set(items: &[&str]) -> Jobs {
    items.iter().map(|s| (*s).into()).collect()
}

#[test]
fn plain_and_quoted_ascii_job_ids() {
    for (text, expected) in [
        ("lint:", "lint"),
        ("_gate_2-test: value", "_gate_2-test"),
        ("Z9:\t{}", "Z9"),
        ("'lint':", "lint"),
        ("\"lint\": # comment", "lint"),
        ("'a-b_2' \t: value", "a-b_2"),
        ("lint \t:\tvalue", "lint"),
        ("lint: arbitrary YAML is not parsed", "lint"),
    ] {
        assert_eq!(job_key(text), Some(expected), "{text:?}");
    }
}
#[test]
fn unreadable_job_keys() {
    for text in [
        "",
        ":",
        "'':",
        "\"\":",
        "1lint:",
        "-lint:",
        "é:",
        "lintλ:",
        "'lint:",
        "\"lint:",
        "'lint\":",
        "\"lint':",
        "lint",
        "lint x:",
        "lint:[]",
        "lint:#comment",
        "lint::",
        " lint:",
        "lint\n:",
    ] {
        assert_eq!(job_key(text), None, "{text:?}");
    }
}
#[test]
fn jobs_block_requires_the_exact_delimiter() {
    for text in [
        "",
        "jobs:\n  lint:\n",
        "name: x\njobs: \n  lint:\n",
        "name: x\n jobs:\n  lint:\n",
        "name: x\r\njobs:\r\n  lint:\r\n",
    ] {
        assert_eq!(parse_jobs(text), None, "{text:?}");
    }
}
#[test]
fn job_indentation_comments_duplicates_and_top_level_boundary() {
    let text = "name: fixture\njobs:\n# comment\n\n  # job comment\n  \n  'lint':\n    steps:\n      - run: ignored\n  \"build-2\": {}\n  lint:\n   deeper: ignored\npermissions:\n  not-a-job:\n";
    assert_eq!(parse_jobs(text), Some((set(&["lint", "build-2"]), vec![])));
}
#[test]
fn unreadable_job_lines_preserve_order_and_spelling() {
    let lines = ["  2bad:", "  bad:[]", "  \ttab:"];
    let text = format!("name: fixture\njobs:\n  good:\n{}\n", lines.join("\n"));
    assert_eq!(
        parse_jobs(&text),
        Some((set(&["good"]), lines.map(String::from).to_vec()))
    );
}
#[test]
fn only_the_first_jobs_block_is_read() {
    assert_eq!(
        parse_jobs("name: x\njobs:\n  first:\nnext:\njobs:\n  second:\n"),
        Some((set(&["first"]), vec![]))
    );
    assert_eq!(parse_jobs("name: x\njobs:\n"), Some((Jobs::new(), vec![])));
}
#[test]
fn job_ids_returns_the_complete_set() {
    assert_eq!(
        job_ids(&workflow(&["lint", "build-2"], "")).unwrap(),
        set(&["lint", "build-2"])
    );
}
#[test]
fn missing_jobs_block_has_the_original_diagnostic() {
    assert_eq!(
        job_ids("name: no jobs\n").unwrap_err(),
        "Cannot find the jobs block; the gate check needs updating"
    );
}
#[test]
fn first_unreadable_job_line_is_reported_with_repr() {
    assert_eq!(
        job_ids("name: x\njobs:\n  \tbad:\n  1later:\n").unwrap_err(),
        "Cannot read the job key '  \\tbad:'; the gate check needs updating"
    );
}

fn base(answers: Vec<Result<String>>) -> (Result<Option<String>>, Vec<Vec<String>>) {
    let mut answers = VecDeque::from(answers);
    let mut calls = vec![];
    let result = base_copy_with(WORKFLOW, GATE, &mut |args| {
        calls.push(args.iter().map(|s| (*s).to_string()).collect());
        answers.pop_front().expect("unexpected Git call")
    });
    assert!(answers.is_empty(), "unconsumed Git replies");
    (result, calls)
}
fn replies(items: &[&str]) -> Vec<Result<String>> {
    items.iter().map(|s| Ok((*s).into())).collect()
}
fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).into()).collect()
}
fn same_path_calls() -> Vec<Vec<String>> {
    vec![
        argv(&["ls-tree", "--name-only", "HEAD^1", "--", WORKFLOW]),
        argv(&["show", &format!("HEAD^1:{WORKFLOW}")]),
    ]
}
fn discovery_calls() -> Vec<Vec<String>> {
    vec![
        same_path_calls()[0].clone(),
        argv(&[
            "ls-tree",
            "-r",
            "--name-only",
            "HEAD^1",
            "--",
            ".github/workflows/",
        ]),
    ]
}
#[test]
fn git_uses_separate_arguments_and_returns_untrimmed_stdout() {
    let args = ["show", "HEAD^1:path;echo fixture"];
    let value = git_with(&args, |command| {
        assert_eq!(command.get_program(), "git");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            args.map(std::ffi::OsStr::new)
        );
        assert_eq!(command.get_current_dir(), None);
        assert_eq!(command.get_envs().count(), 0);
        // output() supplies both capture pipes in production. Test decoded output separately.
        Ok((0, b" \r\nfixture\n ".to_vec(), vec![]))
    })
    .unwrap();
    assert_eq!(value, " \nfixture\n ");
}
#[test]
fn git_propagates_the_original_failure() {
    let result = git_with(&["show", "HEAD^1"], |_| {
        Err("fixture process failure".into())
    });
    assert_eq!(result.unwrap_err(), "fixture process failure");
}
#[test]
fn same_path_precedes_predecessor_discovery() {
    let (result, calls) = base(replies(&[
        &format!("{WORKFLOW}\n"),
        "not even a jobs block\n",
    ]));
    assert_eq!(result.unwrap().as_deref(), Some("not even a jobs block\n"));
    assert_eq!(calls, same_path_calls());
}
#[test]
fn new_workflow_without_a_predecessor_returns_none() {
    let (result, calls) = base(replies(&[" \n", ""]));
    assert_eq!(result.unwrap(), None);
    assert_eq!(calls, discovery_calls());
}
#[test]
fn predecessor_search_reads_only_top_level_yaml_workflows() {
    let paths = [
        ".github/workflows/nested/ignored.yml",
        ".github/workflows/ignored.txt",
        ".github/workflows/ignored.YML",
        ".github/workflows/no-jobs.yml",
        ".github/workflows/other.yaml",
        ".github/workflows/old.yaml",
    ];
    let predecessor = workflow(&[GATE, "lint"], "");
    let (result, calls) = base(replies(&[
        "",
        &paths.join("\r\n"),
        "name: no jobs\n",
        &workflow(&["other"], ""),
        &predecessor,
    ]));
    assert_eq!(result.unwrap(), Some(predecessor));
    let mut expected = discovery_calls();
    expected.extend(
        paths[3..]
            .iter()
            .map(|p| argv(&["show", &format!("HEAD^1:{p}")])),
    );
    assert_eq!(calls, expected);
}
#[test]
fn readable_workflows_without_the_gate_are_not_predecessors() {
    assert_eq!(
        base(replies(&[
            "",
            ".github/workflows/other.yml\n",
            &workflow(&["other"], "")
        ]))
        .0
        .unwrap(),
        None
    );
}
#[test]
fn predecessor_parse_retains_unreadable_lines_for_later_validation() {
    let original = workflow(&[GATE], "  1unreadable:\n");
    assert_eq!(
        base(replies(&["", ".github/workflows/old.yml\n", &original]))
            .0
            .unwrap(),
        Some(original)
    );
}
#[test]
fn multiple_predecessors_refuse_with_the_original_diagnostic() {
    let (result, calls) = base(replies(&[
        "",
        ".github/workflows/a.yml\n.github/workflows/b.yaml\n",
        &workflow(&[GATE], ""),
        &workflow(&[GATE], ""),
    ]));
    assert_eq!(
        result.unwrap_err(),
        "More than one base workflow has a job named aggregate; the gate check needs updating"
    );
    assert_eq!(calls.len(), 4);
}
#[test]
fn git_failures_never_waive_base_checking() {
    for prefix in [
        vec![],
        replies(&[WORKFLOW]),
        replies(&[""]),
        replies(&["", ".github/workflows/old.yml"]),
    ] {
        let count = prefix.len() + 1;
        let mut answers = prefix;
        answers.push(Err("fixture Git failure".into()));
        let (result, calls) = base(answers);
        assert_eq!(result.unwrap_err(), "fixture Git failure");
        assert_eq!(calls.len(), count);
    }
}

fn directory_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("current.yml"), b"fixture\n").unwrap();
    std::fs::write(dir.path().join("notes.txt"), b"unfiltered extension\n").unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::write(dir.path().join("nested/child.yml"), b"nested\n").unwrap();
    dir
}
fn directory_snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(
        root: &Path,
        path: &Path,
        result: &mut std::collections::BTreeMap<PathBuf, Option<Vec<u8>>>,
    ) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let directory = path.is_dir();
            result.insert(
                path.strip_prefix(root).unwrap().into(),
                if directory {
                    None
                } else {
                    Some(std::fs::read(&path).unwrap())
                },
            );
            if directory {
                visit(root, &path, result);
            }
        }
    }
    let mut result = std::collections::BTreeMap::new();
    visit(root, root, &mut result);
    result
}
#[test]
fn returns_the_directory_entry_path_without_an_extension_policy() {
    let dir = directory_fixture();
    let before = directory_snapshot(dir.path());
    for name in ["current.yml", "notes.txt"] {
        assert_eq!(
            workflow_file_in(&format!(".github/workflows/{name}"), dir.path()).unwrap(),
            dir.path().join(name)
        );
        assert_eq!(directory_snapshot(dir.path()), before);
    }
    assert_eq!(
        std::fs::read(dir.path().join("notes.txt")).unwrap(),
        b"unfiltered extension\n"
    );
}
#[test]
fn rejects_absent_directory_nested_and_alternate_prefix_operands() {
    let dir = directory_fixture();
    let before = directory_snapshot(dir.path());
    for operand in [
        "current.yml",
        ".github/workflows/absent.yml",
        ".github/workflows/nested",
        ".github/workflows/nested/child.yml",
        ".github/workflows/../current.yml",
        "./.github/workflows/current.yml",
        &dir.path().join("current.yml").display().to_string(),
        ".github/workflows/",
        ".github/workflow/current.yml",
    ] {
        assert_eq!(
            workflow_file_in(operand, dir.path()).unwrap_err(),
            format!("{operand} is not a file in .github/workflows")
        );
        assert_eq!(directory_snapshot(dir.path()), before);
    }
}
#[test]
fn directory_read_failure_propagates() {
    let dir = directory_fixture();
    let error = workflow_file_in(WORKFLOW, &dir.path().join("absent")).unwrap_err();
    assert!(error.starts_with("OSError:"), "{error}");
}

struct Fixture {
    text: String,
    args: Vec<String>,
    results: Option<String>,
    event: Option<String>,
    base: Result<Option<String>>,
    path_error: Option<String>,
}
impl Default for Fixture {
    fn default() -> Self {
        Self {
            text: workflow(&[GATE, "lint"], ""),
            args: argv(&[WORKFLOW, GATE]),
            results: Some(r#"{"lint":{"result":"success"}}"#.into()),
            event: Some("push".into()),
            base: Ok(None),
            path_error: None,
        }
    }
}
impl Fixture {
    fn pr() -> Self {
        Self {
            event: Some("pull_request".into()),
            ..Self::default()
        }
    }
    fn invoke(&self) -> (Output, Vec<String>) {
        let calls = RefCell::new(vec![]);
        let output = check_with(
            &self.args,
            |name| {
                Ok(match name {
                    "RESULTS" => self.results.clone(),
                    "GITHUB_EVENT_NAME" => self.event.clone(),
                    _ => panic!("unexpected env"),
                })
            },
            |path| {
                assert_eq!(path, WORKFLOW);
                calls.borrow_mut().push("resolve".into());
                self.path_error
                    .clone()
                    .map_or_else(|| Ok(PathBuf::from("owned/current.yml")), Err)
            },
            |path| {
                assert_eq!(path, Path::new("owned/current.yml"));
                calls.borrow_mut().push("read-utf8".into());
                Ok(self.text.clone())
            },
            |path, gate| {
                assert_eq!((path, gate), (WORKFLOW, GATE));
                calls.borrow_mut().push("base".into());
                self.base.clone()
            },
        );
        (output, calls.into_inner())
    }
    fn refuse(&self, message: &str, expected_calls: &[&str]) {
        let (output, calls) = self.invoke();
        assert_eq!(output.code, 1);
        assert_eq!(output.err, [message]);
        assert!(output.out.is_empty());
        assert_eq!(calls, expected_calls);
    }
    fn success(&self, expected_calls: &[&str]) {
        let (output, calls) = self.invoke();
        assert_eq!(output.code, 0, "{:?}", output.err);
        assert!(output.out.is_empty());
        assert!(output.err.is_empty());
        assert_eq!(calls, expected_calls);
    }
}
#[test]
fn success_reads_the_resolved_path_as_utf8_without_printing() {
    Fixture::default().success(&["resolve", "read-utf8"]);
}
#[test]
fn all_non_success_results_fail_in_sorted_name_order_before_io() {
    let results = r#"{"job6":{"result":"failure"},"job5":{"result":"cancelled"},"job4":{"result":"skipped"},"job3":{"result":"timed_out"},"job2":{"result":"neutral"},"job1":{"result":""},"job0":{"result":null},"passed":{"result":"success"}}"#;
    Fixture {
        results: Some(results.into()),
        ..Fixture::default()
    }
    .refuse(
        "Jobs did not succeed: job0, job1, job2, job3, job4, job5, job6",
        &[],
    );
}
#[test]
fn no_results_is_removed_everywhere_and_skips_only_statuses() {
    Fixture {
        text: workflow(&[GATE, "lint", "optional"], ""),
        results: Some(r#"{"lint":{}}"#.into()),
        args: argv(&[
            "--no-results",
            WORKFLOW,
            GATE,
            "--no-results",
            "--exempt",
            "optional",
            "--no-results",
        ]),
        ..Fixture::default()
    }
    .success(&["resolve", "read-utf8"]);
    Fixture {
        results: Some("{}".into()),
        args: argv(&[WORKFLOW, GATE, "--no-results"]),
        ..Fixture::default()
    }
    .refuse("Add these jobs to needs: lint", &["resolve", "read-utf8"]);
}
#[test]
fn results_environment_and_json_are_required_even_with_no_results() {
    for (results, message) in [
        (None, "KeyError: 'RESULTS'"),
        (
            Some("not JSON".into()),
            "JSONDecodeError: Expecting value: line 1 column 1 (char 0)",
        ),
    ] {
        Fixture {
            results,
            args: argv(&[WORKFLOW, GATE, "--no-results"]),
            ..Fixture::default()
        }
        .refuse(message, &[]);
    }
}
#[test]
fn malformed_arguments_and_result_shapes_keep_their_failure_types() {
    for args in [
        argv(&[]),
        argv(&[WORKFLOW]),
        argv(&[WORKFLOW, GATE, "extra"]),
        argv(&["--exempt", "lint"]),
    ] {
        let (out, calls) = Fixture {
            args,
            ..Fixture::default()
        }
        .invoke();
        assert_eq!(out.code, 1);
        assert!(out.err[0].starts_with("ValueError:"));
        assert!(calls.is_empty());
    }
    for (raw, category) in [
        ("[]", "AttributeError:"),
        ("null", "AttributeError:"),
        (r#"{"lint":{}}"#, "KeyError:"),
        (r#"{"lint":null}"#, "TypeError:"),
    ] {
        let (out, calls) = Fixture {
            results: Some(raw.into()),
            ..Fixture::default()
        }
        .invoke();
        assert_eq!(out.code, 1);
        assert!(out.err[0].starts_with(category), "{:?}", out.err);
        assert!(calls.is_empty());
    }
}
#[test]
fn exemptions_waive_only_missing_needs() {
    let args = argv(&[WORKFLOW, GATE, "--exempt", "optional", "optional"]);
    Fixture {
        args: args.clone(),
        text: workflow(&[GATE, "lint", "optional"], ""),
        ..Fixture::default()
    }
    .success(&["resolve", "read-utf8"]);
    Fixture {
        args: args.clone(),
        results: Some(r#"{"optional":{"result":"failure"}}"#.into()),
        ..Fixture::default()
    }
    .refuse("Jobs did not succeed: optional", &[]);
    Fixture {
        args,
        text: workflow(&[GATE, "lint"], "# removed-jobs: optional\n  optional:\n"),
        ..Fixture::default()
    }
    .refuse(
        "These removed-jobs declarations name jobs that still exist: optional",
        &["resolve", "read-utf8"],
    );
}
#[test]
fn workflow_resolution_failure_precedes_open_and_base() {
    Fixture {
        path_error: Some("fixture path refusal".into()),
        ..Fixture::default()
    }
    .refuse("fixture path refusal", &["resolve"]);
}
#[test]
fn workflow_parser_and_gate_refusals_precede_missing_needs() {
    for (text, message) in [
        (
            "name: no jobs\n".into(),
            "Cannot find the jobs block; the gate check needs updating".into(),
        ),
        (
            workflow(&[GATE], "  1bad:\n"),
            "Cannot read the job key '  1bad:'; the gate check needs updating".into(),
        ),
        (
            workflow(&["missing"], ""),
            format!("Cannot find {GATE} in {WORKFLOW}; the gate check needs updating"),
        ),
    ] {
        Fixture {
            text,
            ..Fixture::default()
        }
        .refuse(&message, &["resolve", "read-utf8"]);
    }
}
#[test]
fn missing_needs_are_sorted_and_precede_live_declarations() {
    Fixture {
        text: workflow(&[GATE, "zeta", "alpha"], "# removed-jobs: alpha\n"),
        ..Fixture::default()
    }
    .refuse(
        "Add these jobs to needs: alpha, zeta",
        &["resolve", "read-utf8"],
    );
}
#[test]
fn live_declarations_are_sorted_and_precede_base_lookup() {
    Fixture {
        text: workflow(
            &[GATE, "zeta", "alpha"],
            "  # removed-jobs: zeta, alpha, zeta\n",
        ),
        results: Some(r#"{"zeta":{"result":"success"},"alpha":{"result":"success"}}"#.into()),
        ..Fixture::default()
    }
    .refuse(
        "These removed-jobs declarations name jobs that still exist: alpha, zeta",
        &["resolve", "read-utf8"],
    );
}
#[test]
fn only_pull_request_looks_up_the_base_workflow() {
    for event in [
        None,
        Some("push"),
        Some("pull_request_target"),
        Some("workflow_dispatch"),
        Some("pull_request"),
    ] {
        let f = Fixture {
            event: event.map(String::from),
            base: Ok(Some(workflow(&[GATE, "lint"], ""))),
            ..Fixture::default()
        };
        f.success(if event == Some("pull_request") {
            &["resolve", "read-utf8", "base"]
        } else {
            &["resolve", "read-utf8"]
        });
    }
}
#[test]
fn new_workflow_without_a_base_succeeds() {
    Fixture::pr().success(&["resolve", "read-utf8", "base"]);
}
#[test]
fn declared_removals_print_once_in_sorted_order() {
    let (out, calls) = Fixture {
        text: workflow(
            &[GATE, "lint"],
            "  # removed-jobs: zeta, alpha,zeta, unused,\n",
        ),
        base: Ok(Some(workflow(&[GATE, "lint", "zeta", "alpha"], ""))),
        ..Fixture::pr()
    }
    .invoke();
    assert_eq!(out.code, 0);
    assert!(out.err.is_empty());
    assert_eq!(
        out.out,
        [format!("Declared removals from {WORKFLOW}: alpha, zeta")]
    );
    assert_eq!(calls, ["resolve", "read-utf8", "base"]);
}
#[test]
fn undeclared_removals_have_the_exact_sorted_instruction() {
    Fixture { base: Ok(Some(workflow(&[GATE, "lint", "zeta", "alpha"], ""))), ..Fixture::pr() }.refuse(&format!("These jobs were removed from {WORKFLOW}: alpha, zeta. If that is intended, add the comment '# removed-jobs: alpha, zeta' to that file."), &["resolve", "read-utf8", "base"]);
}
#[test]
fn mixed_removals_print_declared_names_before_refusing_the_rest() {
    let (out, calls) = Fixture {
        text: workflow(&[GATE, "lint"], "# removed-jobs: beta\n"),
        base: Ok(Some(workflow(&[GATE, "lint", "beta", "alpha"], ""))),
        ..Fixture::pr()
    }
    .invoke();
    assert_eq!(out.code, 1);
    assert_eq!(
        out.out,
        [format!("Declared removals from {WORKFLOW}: beta")]
    );
    assert_eq!(
        out.err,
        [format!(
            "These jobs were removed from {WORKFLOW}: alpha. If that is intended, add the comment '# removed-jobs: alpha' to that file."
        )]
    );
    assert_eq!(calls, ["resolve", "read-utf8", "base"]);
}
#[test]
fn exemption_cannot_waive_removal_and_other_comments_are_not_declarations() {
    Fixture { text: workflow(&[GATE, "lint"], "# removed-jobs-extra: old\n"), args: argv(&[WORKFLOW, GATE, "--exempt", "old"]), base: Ok(Some(workflow(&[GATE, "lint", "old"], ""))), ..Fixture::pr() }.refuse(&format!("These jobs were removed from {WORKFLOW}: old. If that is intended, add the comment '# removed-jobs: old' to that file."), &["resolve", "read-utf8", "base"]);
}
#[test]
fn base_git_failure_and_unreadable_base_are_not_waived() {
    Fixture {
        base: Err("fixture Git failure".into()),
        ..Fixture::pr()
    }
    .refuse("fixture Git failure", &["resolve", "read-utf8", "base"]);
    Fixture {
        base: Ok(Some(workflow(&[GATE], "  1bad:\n"))),
        ..Fixture::pr()
    }
    .refuse(
        "Cannot read the job key '  1bad:'; the gate check needs updating",
        &["resolve", "read-utf8", "base"],
    );
}

#[test]
fn git_decodes_both_streams_before_refusing_a_failed_status() {
    for (code, stdout, stderr) in [
        (0, vec![0xff], vec![]),
        (128, vec![0xff], vec![]),
        (0, vec![], vec![0xff]),
        (128, vec![], vec![0xff]),
    ] {
        let error = git_with(&["fixture"], |_| Ok((code, stdout, stderr))).unwrap_err();
        assert!(error.starts_with("UnicodeDecodeError:"), "{error}");
    }
    let error = git_with(&["show", "HEAD^1"], |_| {
        Ok((128, vec![], b"fatal: fixture\r\n".to_vec()))
    })
    .unwrap_err();
    assert!(error.starts_with("CalledProcessError:"));
    assert!(error.contains("128"));
    assert_eq!(
        git_with(&["fixture"], |_| Ok((
            0,
            b"\r\n\xc3\xa9\rtext\n ".to_vec(),
            vec![]
        )))
        .unwrap(),
        "\né\ntext\n "
    );
}

#[test]
fn no_results_retains_python_iteration_and_hashability_failures() {
    for raw in [r#"["lint",null,true,7,1.5]"#, r#"{"lint":{}}"#] {
        Fixture {
            results: Some(raw.into()),
            args: argv(&[WORKFLOW, GATE, "--no-results"]),
            ..Fixture::default()
        }
        .success(&["resolve", "read-utf8"]);
    }
    for (raw, category) in [
        ("null", "TypeError:"),
        ("7", "TypeError:"),
        ("[[]]", "TypeError:"),
        ("[{}]", "TypeError:"),
    ] {
        let (out, calls) = Fixture {
            results: Some(raw.into()),
            args: argv(&[WORKFLOW, GATE, "--no-results"]),
            ..Fixture::default()
        }
        .invoke();
        assert_eq!(out.code, 1);
        assert!(out.err[0].starts_with(category));
        assert_eq!(calls, ["resolve", "read-utf8"]);
    }
    // Strings iterate into characters, so only one-character job ids can match.
    Fixture {
        text: workflow(&[GATE, "l", "i"], ""),
        results: Some(r#""lint""#.into()),
        args: argv(&[WORKFLOW, GATE, "--no-results"]),
        ..Fixture::default()
    }
    .success(&["resolve", "read-utf8"]);
}

#[test]
fn duplicate_results_and_python_whitespace_keep_reference_semantics() {
    Fixture {
        results: Some(r#"{"lint":{"result":"failure"},"lint":{"result":"success"}}"#.into()),
        ..Fixture::default()
    }
    .success(&["resolve", "read-utf8"]);
    let (out, _) = Fixture {
        text: workflow(&[GATE, "lint"], "\u{1c}# removed-jobs: \u{a0}old\u{85}\n"),
        base: Ok(Some(workflow(&[GATE, "lint", "old"], ""))),
        ..Fixture::pr()
    }
    .invoke();
    assert_eq!(out.code, 0);
    assert_eq!(out.out, [format!("Declared removals from {WORKFLOW}: old")]);
}

#[test]
fn workflow_read_failure_stops_before_base_lookup() {
    let result = check_with(
        &argv(&[WORKFLOW, GATE]),
        |name| {
            Ok(if name == "RESULTS" {
                Some("{}".into())
            } else {
                panic!("event read too early")
            })
        },
        |_| Ok(PathBuf::from("owned")),
        |_| Err("fixture read failure".into()),
        |_, _| panic!("base called after read failure"),
    );
    assert_eq!(result.code, 1);
    assert!(result.out.is_empty());
    assert_eq!(result.err, ["fixture read failure"]);
}
