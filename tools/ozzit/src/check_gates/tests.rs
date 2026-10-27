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
fn ids(text: &str) -> Option<(Jobs, Vec<String>)> {
    parse_jobs(text).map(|(names, unreadable)| (names.into_keys().collect(), unreadable))
}
#[test]
fn job_indentation_comments_duplicates_and_top_level_boundary() {
    let text = "name: fixture\njobs:\n# comment\n\n  # job comment\n  \n  'lint':\n    steps:\n      - run: ignored\n  \"build-2\": {}\n  lint:\n   deeper: ignored\npermissions:\n  not-a-job:\n";
    // A repeated key is unreadable, so job_ids refuses it.
    assert_eq!(
        ids(text),
        Some((set(&["lint", "build-2"]), vec!["  lint:".into()]))
    );
}
#[test]
fn unreadable_job_lines_preserve_order_and_spelling() {
    let lines = ["  2bad:", "  bad:[]", "  \ttab:"];
    let text = format!("name: fixture\njobs:\n  good:\n{}\n", lines.join("\n"));
    assert_eq!(
        ids(&text),
        Some((set(&["good"]), lines.map(String::from).to_vec()))
    );
}
#[test]
fn only_the_first_jobs_block_is_read() {
    assert_eq!(
        ids("name: x\njobs:\n  first:\nnext:\njobs:\n  second:\n"),
        Some((set(&["first"]), vec![]))
    );
    assert_eq!(ids("name: x\njobs:\n"), Some((Jobs::new(), vec![])));
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

fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).into()).collect()
}

// Reader and resolver cases shared with the Python gate checkers.
const NEW: &str = ".github/workflows/new.yml";
const OLD: &str = ".github/workflows/old.yml";
const REPLACEMENT: &str = "replacement";
const PROTECTED: &str = "  replacement:\n    name: protected\n";
fn yaml(jobs: &str) -> String {
    format!("name: fixture\n\non: pull_request\n\njobs:\n{jobs}")
}
fn names(jobs: &str) -> BTreeMap<String, Names> {
    let (names, unreadable) = parse_jobs(&yaml(jobs)).unwrap();
    assert!(unreadable.is_empty(), "{unreadable:?}");
    names
}
fn possible(jobs: &str, job: &str, context: &str) -> bool {
    possible_context(&names(jobs)[job], context)
}
/// Resolve with base SOURCES, where neither the destination nor any source exists at HEAD.
fn base_of(sources: &[(&str, String)], head: &str) -> Result<Jobs> {
    let mut git = |args: &[&str]| -> Result<String> {
        if args[0] == "ls-tree" {
            return Ok(String::new());
        }
        let path = args[1].split_once(':').unwrap().1;
        let (_, text) = sources
            .iter()
            .find(|(p, _)| *p == path)
            .expect("unexpected show");
        Ok(text.clone())
    };
    let paths = || Ok(sources.iter().map(|(path, _)| (*path).into()).collect());
    base_jobs_with(NEW, REPLACEMENT, &yaml(head), &mut git, paths)
}
/// Resolve with scripted Git replies, recording each call in order ("paths" for the listing).
fn scripted(
    answers: Vec<Result<String>>,
    paths: Result<Vec<String>>,
    head: &str,
) -> (Result<Jobs>, Vec<Vec<String>>) {
    let mut answers = VecDeque::from(answers);
    let calls = RefCell::new(vec![]);
    let result = base_jobs_with(
        NEW,
        REPLACEMENT,
        &yaml(head),
        &mut |args| {
            calls.borrow_mut().push(argv(args));
            answers.pop_front().expect("unexpected Git call")
        },
        || {
            calls.borrow_mut().push(argv(&["paths"]));
            paths
        },
    );
    assert!(answers.is_empty(), "unconsumed Git replies");
    (result, calls.into_inner())
}
fn replies(items: &[&str]) -> Vec<Result<String>> {
    items.iter().map(|s| Ok((*s).into())).collect()
}
fn show(revision: &str, path: &str) -> Vec<String> {
    argv(&["show", &format!("{revision}:{path}")])
}
fn ls_tree(revision: &str, path: &str) -> Vec<String> {
    argv(&["ls-tree", "--name-only", revision, "--", path])
}

#[test]
fn literal_names_and_quoted_job_keys() {
    let names =
        names("  'plain':\n    runs-on: linux\n  other:\n    name: 'It''s quoted' # comment\n");
    assert_eq!(names["plain"], Some(set(&["plain"])));
    assert_eq!(names["other"], Some(set(&["It's quoted"])));
    for (text, expected) in [
        ("\"double\" # comment", Some("double")),
        ("\"a\\u0041\"", Some("aA")),
        ("\"\"", None),
        ("\"line\\nbreak\"", None),
        ("\"unterminated", None),
        ("\"value\" trailing", None),
        ("''", None),
        ("plain#hash # comment", Some("plain#hash")),
        ("key: value", None),
        ("Null", None),
        ("-1.5e3", None),
        ("   ", None),
    ] {
        assert_eq!(literal_name(text).as_deref(), expected, "{text:?}");
    }
}
#[test]
fn comments_strip_after_whitespace_only() {
    for (text, expected) in [
        ("  a#b # c", "a#b"),
        ("a\t#c", "a"),
        ("#lead", "#lead"),
        ("a\u{a0}#c", "a"),
        ("é # c", "é"),
    ] {
        assert_eq!(without_comment(text), expected, "{text:?}");
    }
}
#[test]
fn static_include_resolves_only_its_complete_rows() {
    let body = "  build:\n    name: ${{ matrix.package_dir }}\n    strategy:\n      matrix:\n        include:\n          - package_dir: first\n            import_name: first_import\n          - package_dir: 'second'\n            import_name: second_import\n";
    assert_eq!(names(body)["build"], Some(set(&["first", "second"])));
    for replacement in [
        "            import_name: first_import",
        "          - package_dir: ${{ inputs.package }}",
        "          - package_dir: *alias",
        "          - package_dir: first\n            package_dir: second",
    ] {
        let jobs = body.replace("          - package_dir: first", replacement);
        assert!(possible(&jobs, "build", "protected"), "{replacement:?}");
    }
}
#[test]
fn extra_matrix_dimension_is_unknown() {
    let jobs = "  build:\n    name: ${{ matrix.package_dir }}\n    strategy:\n      matrix:\n        os: [linux]\n        include:\n          - package_dir: first\n";
    assert!(possible(jobs, "build", "protected"));
}
#[test]
fn typed_yaml_matrix_scalars_are_not_misread_as_strings() {
    for value in ["0x10", "1_000", "2026-10-15", ".nan", "YES", "on"] {
        let jobs = format!(
            "  build:\n    name: ${{{{ matrix.label }}}}\n    strategy:\n      matrix:\n        include:\n          - label: {value}\n"
        );
        assert!(possible(&jobs, "build", "protected"), "{value}");
    }
}
#[test]
fn conditional_has_both_literal_outcomes() {
    for condition in ["linux", "other"] {
        let jobs = format!(
            "  check:\n    name: ${{{{ matrix.os == '{condition}' && 'first' || 'second' }}}}\n"
        );
        let names = &names(&jobs)["check"];
        assert_eq!(*names, Some(set(&["first", "second"])));
        assert!(!possible_context(names, "protected"));
    }
}
#[test]
fn unknown_conditionals_do_not_disappear() {
    for expression in [
        "matrix.os && '' || 'second'",
        "needs.build.outputs.name",
        "matrix.os == 'linux' && inputs.name || 'second'",
    ] {
        let jobs = format!("  check:\n    name: ${{{{ {expression} }}}}\n");
        assert!(possible(&jobs, "check", "protected"), "{expression}");
    }
}
#[test]
fn unknown_templates_keep_literal_ends() {
    let jobs = "  check:\n    name: prefix (${{ inputs.name }})\n";
    assert!(!possible(jobs, "check", "protected"));
    assert!(possible(jobs, "check", "prefix (protected)"));
}
#[test]
fn expression_literal_ends_cannot_overlap() {
    for (pattern, context, expected) in [
        ("new-${{ matrix.flavour }}-gates", "new-gates", false),
        ("new-${{ matrix.flavour }}gates", "new-gates", true),
        ("}}${{ inputs.context", "protected", true),
    ] {
        assert_eq!(
            possible_context(&Some(set(&[pattern])), context),
            expected,
            "{pattern}"
        );
    }
}
#[test]
fn tab_comments_and_blank_lines_do_not_end_the_jobs_block() {
    for separator in ["\t# separator", "\t", " \t# separator"] {
        let names = names(&format!(
            "  replacement:\n{separator}\n    name: protected\n  critical:\n    runs-on: linux\n"
        ));
        assert_eq!(
            names["replacement"],
            Some(set(&["protected"])),
            "{separator:?}"
        );
        assert_eq!(names["critical"], Some(set(&["critical"])), "{separator:?}");
        assert_eq!(names.len(), 2);
    }
}
#[test]
fn implicit_matrix_names_keep_possible_suffixes() {
    let jobs = "  build:\n    strategy:\n      matrix:\n        os: [linux]\n";
    assert!(possible(jobs, "build", "build (linux)"));
    assert!(possible(jobs, "build", "build"));
    assert!(!possible(jobs, "build", "build-gates"));
    assert!(!possible(jobs, "build", "protected"));
}
#[test]
fn implicit_matrix_matches_fail_closed_in_the_resolver() {
    let source = yaml(
        "  build:
    strategy:
      matrix:
        os: [linux]
",
    );
    for context in ["build", "build (linux)"] {
        assert_eq!(
            base_of(
                &[(OLD, source.clone())],
                &format!(
                    "  replacement:
    name: {context}
"
                )
            )
            .unwrap_err(),
            format!("Cannot resolve a possible gate context in {OLD}; update the reader"),
            "{context}"
        );
    }
    let head = "  replacement:
    name: build-gates
";
    assert_eq!(base_of(&[(OLD, source)], head).unwrap(), Jobs::new());
}
#[test]
fn reusable_jobs_keep_possible_called_job_names() {
    let jobs = "  call:\n    name: component\n    uses: ./.github/workflows/package.yml\n";
    assert!(possible(jobs, "call", "component / protected"));
    assert!(!possible(jobs, "call", "component"));
    assert_eq!(
        base_of(
            &[(OLD, yaml(jobs))],
            "  replacement:\n    name: component / protected\n"
        )
        .unwrap_err(),
        format!("Cannot resolve a possible gate context in {OLD}; update the reader")
    );
}
#[test]
fn unknown_matching_base_job_fails_even_when_it_is_the_only_candidate() {
    for name in [
        "${{ inputs.name }}",
        "*alias",
        "|\n      protected",
        "!tag protected",
    ] {
        let source = yaml(&format!("  old:\n    name: {name}\n"));
        assert_eq!(
            base_of(&[(OLD, source)], PROTECTED).unwrap_err(),
            format!("Cannot resolve a possible gate context in {OLD}; update the reader"),
            "{name:?}"
        );
    }
}
#[test]
fn two_matching_jobs_in_one_workflow_are_ambiguous() {
    let source = yaml("  first:\n    name: protected\n  second:\n    name: protected\n");
    assert_eq!(
        base_of(&[(OLD, source)], PROTECTED).unwrap_err(),
        "More than one base job can precede replacement; the gate check needs updating"
    );
}
#[test]
fn duplicate_names_and_keys_are_not_treated_as_unique() {
    for (jobs, message) in [
        (
            "  old:\n    name: protected\n    name: other\n",
            format!("Cannot resolve a possible gate context in {OLD}; update the reader"),
        ),
        (
            "  old:\n    name: protected\n  old:\n    name: other\n",
            format!("Cannot read base workflow {OLD}; the gate check needs updating"),
        ),
        (
            "  old: {name: protected}\n",
            format!("Cannot resolve a possible gate context in {OLD}; update the reader"),
        ),
    ] {
        assert_eq!(
            base_of(&[(OLD, yaml(jobs))], PROTECTED).unwrap_err(),
            message,
            "{jobs:?}"
        );
    }
}
#[test]
fn unrelated_id_collision_does_not_override_reported_name() {
    let sources = [
        (
            ".github/workflows/other.yml",
            yaml("  replacement:\n    name: other\n"),
        ),
        (OLD, yaml("  original:\n    name: protected\n")),
    ];
    assert_eq!(base_of(&sources, PROTECTED).unwrap(), set(&["original"]));
}
#[test]
fn non_yaml_whitespace_names_stay_unknown() {
    for text in [
        "protected\u{a0}#gate",
        "\"protected\u{a0}#gate\"",
        "protected\u{2003}",
    ] {
        assert_eq!(literal_name(text), None, "{text:?}");
    }
    assert_eq!(
        base_of(&[], "  replacement:\n    name: protected\u{a0}#gate\n").unwrap_err(),
        "Cannot determine the gate context; use a single-line literal name"
    );
}
#[test]
fn same_id_predecessor_binds_when_file_and_context_change() {
    let old = yaml("  replacement:\n    name: old-context\n  critical:\n    runs-on: linux\n");
    assert_eq!(
        base_of(&[(OLD, old.clone())], PROTECTED).unwrap(),
        set(&["replacement", "critical"])
    );
    assert_eq!(
        base_of(
            &[(OLD, old.clone()), (".github/workflows/two.yml", old)],
            PROTECTED
        )
        .unwrap_err(),
        "More than one base workflow can precede replacement; the gate check needs updating"
    );
}
#[test]
fn same_id_gate_left_in_place_is_not_a_predecessor() {
    let old = yaml("  replacement:\n    name: old-context\n  critical:\n    runs-on: linux\n");
    let (result, calls) = scripted(
        replies(&["", &old, OLD, &old]),
        Ok(vec![OLD.into()]),
        "  replacement:\n    name: new-context\n",
    );
    assert_eq!(result.unwrap(), Jobs::new());
    assert_eq!(
        calls,
        [
            ls_tree("HEAD^1", NEW),
            argv(&["paths"]),
            show("HEAD^1", OLD),
            ls_tree("HEAD", OLD),
            show("HEAD", OLD),
        ]
    );
}
#[test]
fn unknown_head_name_and_unreadable_base_fail() {
    assert_eq!(
        base_of(&[], "  replacement:\n    name: ${{ inputs.name }}\n").unwrap_err(),
        "Cannot determine the gate context; use a single-line literal name"
    );
    assert_eq!(
        base_of(&[(OLD, "jobs: *alias\n".into())], PROTECTED).unwrap_err(),
        format!("Cannot read base workflow {OLD}; the gate check needs updating")
    );
}
#[test]
fn same_path_and_context_sources_both_contribute_jobs() {
    let destination = yaml("  existing:\n    runs-on: linux\n");
    let source = yaml("  original:\n    name: protected\n");
    let (result, calls) = scripted(
        replies(&["new.yml\n", &destination, &source]),
        Ok(vec![OLD.into(), String::new()]),
        PROTECTED,
    );
    assert_eq!(result.unwrap(), set(&["existing", "original"]));
    assert_eq!(
        calls,
        [
            ls_tree("HEAD^1", NEW),
            show("HEAD^1", NEW),
            argv(&["paths"]),
            show("HEAD^1", OLD),
        ]
    );
}
#[test]
fn unreadable_destination_base_copy_refuses() {
    let (result, _) = scripted(
        replies(&["new.yml\n", &yaml("  1bad:\n")]),
        Ok(vec![]),
        PROTECTED,
    );
    assert_eq!(
        result.unwrap_err(),
        "Cannot read the job key '  1bad:'; the gate check needs updating"
    );
}
#[test]
fn predecessor_search_reads_only_top_level_yaml_workflows() {
    let paths = [
        ".github/workflows/nested/ignored.yml",
        ".github/workflows/ignored.txt",
        ".github/workflows/ignored.YML",
        ".github/workflows/other.yaml",
        OLD,
        "",
    ];
    let (result, calls) = scripted(
        replies(&[
            "",
            &yaml("  other:\n    runs-on: linux\n"),
            &yaml("  original:\n    name: protected\n"),
        ]),
        Ok(argv(&paths)),
        PROTECTED,
    );
    assert_eq!(result.unwrap(), set(&["original"]));
    assert_eq!(
        calls,
        [
            ls_tree("HEAD^1", NEW),
            argv(&["paths"]),
            show("HEAD^1", paths[3]),
            show("HEAD^1", OLD),
        ]
    );
}
#[test]
fn git_failures_never_waive_base_checking() {
    let old = yaml("  replacement:\n    name: old-context\n");
    for (prefix, paths) in [
        (vec![], Ok(vec![])),
        (replies(&["new.yml\n"]), Ok(vec![])),
        (replies(&[""]), Ok(vec![OLD.into()])),
        (replies(&["", &old]), Ok(vec![OLD.into()])),
        (replies(&["", &old, OLD]), Ok(vec![OLD.into()])),
    ] {
        let count = prefix.len() + 2;
        let mut answers = prefix;
        answers.push(Err("fixture Git failure".into()));
        let (result, calls) = scripted(answers, paths, PROTECTED);
        assert_eq!(result.unwrap_err(), "fixture Git failure");
        assert!(calls.len() <= count);
    }
    let (result, _) = scripted(
        replies(&[""]),
        Err("fixture listing failure".into()),
        PROTECTED,
    );
    assert_eq!(result.unwrap_err(), "fixture listing failure");
}
#[test]
fn event_forms_and_unknown_namespaces() {
    for (events, expected) in [
        ("pull_request", Some(vec!["standalone"])),
        (
            "[push, workflow_call]",
            Some(vec!["callable", "standalone"]),
        ),
        ("\n  workflow_call:\n    inputs: {}", Some(vec!["callable"])),
        ("*alias", None),
        ("{workflow_call: {}}", None),
        ("[]", None),
        ("[", None),
    ] {
        let text = format!("name: fixture\non: {events}\njobs:\n  gate:\n");
        assert_eq!(
            workflow_modes(&text),
            expected.map(|modes| modes.into_iter().collect()),
            "{events:?}"
        );
    }
    let source = "name: fixture\non: *alias\njobs:\n  old:\n    name: protected\n";
    assert_eq!(
        base_of(&[(OLD, source.into())], PROTECTED).unwrap_err(),
        format!("Cannot determine the invocation namespace in {OLD}; update the reader")
    );
    let callable = "name: fixture\non: workflow_call\njobs:\n  old:\n    name: protected\n";
    assert_eq!(
        base_of(&[(OLD, callable.into())], PROTECTED).unwrap(),
        Jobs::new()
    );
}
#[test]
fn head_namespace_must_be_single() {
    let head = format!("name: fixture\non: [push, workflow_call]\njobs:\n{PROTECTED}");
    let result = base_jobs_with(NEW, REPLACEMENT, &head, &mut |_| Ok(String::new()), || {
        Ok(vec![])
    });
    assert_eq!(
        result.unwrap_err(),
        "Cannot determine the workflow invocation namespace; update the reader"
    );
}
#[test]
fn path_listing_checks_status_then_keeps_linebreaks_and_rejects_invalid_utf8() {
    let paths = [".github/workflows/ci\r.yml", ".github/workflows/ci\n.yml"];
    let listing = format!("{}\0", paths.join("\0"));
    let result = base_workflow_paths_with(|command| {
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["ls-tree", "-r", "--name-only", "-z", "HEAD^1", "--", PREFIX]
                .map(std::ffi::OsStr::new)
        );
        Ok((0, listing.into_bytes(), vec![0xff]))
    });
    assert_eq!(result.unwrap(), [paths[0], paths[1], ""]);
    let error = base_workflow_paths_with(|_| Ok((0, b"\xff\0".to_vec(), vec![]))).unwrap_err();
    assert!(error.starts_with("UnicodeDecodeError:"), "{error}");
    let error = base_workflow_paths_with(|_| Ok((128, b"\xff".to_vec(), vec![]))).unwrap_err();
    assert!(error.starts_with("CalledProcessError:"), "{error}");
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
    base: Result<Jobs>,
    path_error: Option<String>,
}
impl Default for Fixture {
    fn default() -> Self {
        Self {
            text: workflow(&[GATE, "lint"], ""),
            args: argv(&[WORKFLOW, GATE]),
            results: Some(r#"{"lint":{"result":"success"}}"#.into()),
            event: Some("push".into()),
            base: Ok(Jobs::new()),
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
            |path, gate, head| {
                assert_eq!((path, gate, head), (WORKFLOW, GATE, self.text.as_str()));
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
            base: Ok(set(&[GATE, "lint"])),
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
        base: Ok(set(&[GATE, "lint", "zeta", "alpha"])),
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
    Fixture { base: Ok(set(&[GATE, "lint", "zeta", "alpha"])), ..Fixture::pr() }.refuse(&format!("These jobs were removed from {WORKFLOW}: alpha, zeta. If that is intended, add the comment '# removed-jobs: alpha, zeta' to that file."), &["resolve", "read-utf8", "base"]);
}
#[test]
fn mixed_removals_print_declared_names_before_refusing_the_rest() {
    let (out, calls) = Fixture {
        text: workflow(&[GATE, "lint"], "# removed-jobs: beta\n"),
        base: Ok(set(&[GATE, "lint", "beta", "alpha"])),
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
    Fixture { text: workflow(&[GATE, "lint"], "# removed-jobs-extra: old\n"), args: argv(&[WORKFLOW, GATE, "--exempt", "old"]), base: Ok(set(&[GATE, "lint", "old"])), ..Fixture::pr() }.refuse(&format!("These jobs were removed from {WORKFLOW}: old. If that is intended, add the comment '# removed-jobs: old' to that file."), &["resolve", "read-utf8", "base"]);
}
#[test]
fn base_failure_is_not_waived() {
    Fixture {
        base: Err("fixture Git failure".into()),
        ..Fixture::pr()
    }
    .refuse("fixture Git failure", &["resolve", "read-utf8", "base"]);
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
        base: Ok(set(&[GATE, "lint", "old"])),
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
        |_, _, _| panic!("base called after read failure"),
    );
    assert_eq!(result.code, 1);
    assert!(result.out.is_empty());
    assert_eq!(result.err, ["fixture read failure"]);
}
