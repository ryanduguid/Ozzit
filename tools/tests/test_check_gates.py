"""Baseline contracts for the CI aggregate-gate helper before its Rust port."""

import importlib.util
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / ".github" / "ci" / "check_gates.py"
SPEC = importlib.util.spec_from_file_location("check_gates", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
check_gates = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check_gates)

WORKFLOW = ".github/workflows/current.yml"
GATE = "aggregate"


def workflow(*jobs, comments=""):
    return "name: fixture\njobs:\n" + "".join(f"  {job}:\n" for job in jobs) + comments


def snapshot(directory):
    return {
        str(path.relative_to(directory)): path.read_bytes() if path.is_file() else None
        for path in directory.rglob("*")
    }


class JobGrammarTests(unittest.TestCase):
    def test_plain_and_quoted_ascii_job_ids(self):
        cases = (
            ("lint:", "lint"),
            ("_gate_2-test: value", "_gate_2-test"),
            ("Z9:\t{}", "Z9"),
            ("'lint':", "lint"),
            ('"lint": # comment', "lint"),
            ("'a-b_2' \t: value", "a-b_2"),
            ("lint \t:\tvalue", "lint"),
            ("lint: arbitrary YAML is not parsed", "lint"),
        )
        for text, expected in cases:
            with self.subTest(text=text):
                self.assertEqual(check_gates.job_key(text), expected)

    def test_unreadable_job_keys(self):
        for text in (
            "", ":", "'':", '"":', "1lint:", "-lint:", "é:", "lintλ:",
            "'lint:", '"lint:', "'lint\":", '"lint\':', "lint", "lint x:",
            "lint:[]", "lint:#comment", "lint::", " lint:", "lint\n:",
        ):
            with self.subTest(text=text):
                self.assertIsNone(check_gates.job_key(text))

    def test_jobs_block_requires_the_exact_delimiter(self):
        for text in (
            "", "jobs:\n  lint:\n", "name: x\njobs: \n  lint:\n",
            "name: x\n jobs:\n  lint:\n", "name: x\r\njobs:\r\n  lint:\r\n",
        ):
            with self.subTest(text=text):
                self.assertIsNone(check_gates.parse_jobs(text))

    def test_job_indentation_comments_duplicates_and_top_level_boundary(self):
        text = (
            "name: fixture\njobs:\n"
            "# comment\n\n  # job comment\n  \n"
            "  'lint':\n    steps:\n      - run: ignored\n"
            '  "build-2": {}\n  lint:\n   deeper: ignored\n'
            "permissions:\n  not-a-job:\n"
        )
        self.assertEqual(check_gates.parse_jobs(text), ({"lint", "build-2"}, []))

    def test_unreadable_job_lines_preserve_order_and_spelling(self):
        lines = ["  2bad:", "  bad:[]", "  \ttab:"]
        text = "name: fixture\njobs:\n  good:\n" + "\n".join(lines) + "\n"
        self.assertEqual(check_gates.parse_jobs(text), ({"good"}, lines))

    def test_only_the_first_jobs_block_is_read(self):
        self.assertEqual(
            check_gates.parse_jobs("name: x\njobs:\n  first:\nnext:\njobs:\n  second:\n"),
            ({"first"}, []),
        )
        self.assertEqual(check_gates.parse_jobs("name: x\njobs:\n"), (set(), []))

    def test_job_ids_returns_the_complete_set(self):
        self.assertEqual(check_gates.job_ids(workflow("lint", "build-2")), {"lint", "build-2"})

    def test_missing_jobs_block_has_the_original_diagnostic(self):
        with self.assertRaises(SystemExit) as caught:
            check_gates.job_ids("name: no jobs\n")
        self.assertEqual(caught.exception.code, "Cannot find the jobs block; the gate check needs updating")

    def test_first_unreadable_job_line_is_reported_with_repr(self):
        first = "  \tbad:"
        with self.assertRaises(SystemExit) as caught:
            check_gates.job_ids("name: x\njobs:\n" + first + "\n  1later:\n")
        self.assertEqual(
            caught.exception.code,
            f"Cannot read the job key {first!r}; the gate check needs updating",
        )


class GitAndBaseTests(unittest.TestCase):
    def test_git_uses_separate_arguments_and_returns_untrimmed_stdout(self):
        completed = SimpleNamespace(stdout=" \r\nfixture\n ")
        with mock.patch.object(check_gates.subprocess, "run", return_value=completed) as run:
            self.assertEqual(check_gates.git("show", "HEAD^1:path;echo fixture"), completed.stdout)
        run.assert_called_once_with(
            ["git", "show", "HEAD^1:path;echo fixture"],
            capture_output=True, text=True, check=True,
        )

    def test_git_propagates_the_original_failure(self):
        error = subprocess.CalledProcessError(128, ["git", "show", "HEAD^1"])
        with mock.patch.object(check_gates.subprocess, "run", side_effect=error):
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                check_gates.git("show", "HEAD^1")
        self.assertIs(caught.exception, error)

    def test_same_path_precedes_predecessor_discovery(self):
        original = "not even a jobs block\n"
        with mock.patch.object(check_gates, "git", side_effect=[WORKFLOW + "\n", original]) as git:
            self.assertEqual(check_gates.base_copy(WORKFLOW, GATE), original)
        self.assertEqual(git.call_args_list, [
            mock.call("ls-tree", "--name-only", "HEAD^1", "--", WORKFLOW),
            mock.call("show", f"HEAD^1:{WORKFLOW}"),
        ])

    def test_new_workflow_without_a_predecessor_returns_none(self):
        with mock.patch.object(check_gates, "git", side_effect=[" \n", ""]) as git:
            self.assertIsNone(check_gates.base_copy(WORKFLOW, GATE))
        self.assertEqual(git.call_args_list, [
            mock.call("ls-tree", "--name-only", "HEAD^1", "--", WORKFLOW),
            mock.call("ls-tree", "-r", "--name-only", "HEAD^1", "--", ".github/workflows/"),
        ])

    def test_predecessor_search_reads_only_top_level_yaml_workflows(self):
        paths = [
            ".github/workflows/nested/ignored.yml", ".github/workflows/ignored.txt",
            ".github/workflows/ignored.YML", ".github/workflows/no-jobs.yml",
            ".github/workflows/other.yaml", ".github/workflows/old.yaml",
        ]
        predecessor = workflow(GATE, "lint")
        with mock.patch.object(check_gates, "git", side_effect=[
            "", "\r\n".join(paths), "name: no jobs\n", workflow("other"), predecessor,
        ]) as git:
            self.assertEqual(check_gates.base_copy(WORKFLOW, GATE), predecessor)
        self.assertEqual(git.call_args_list, [
            mock.call("ls-tree", "--name-only", "HEAD^1", "--", WORKFLOW),
            mock.call("ls-tree", "-r", "--name-only", "HEAD^1", "--", ".github/workflows/"),
            *[mock.call("show", "HEAD^1:" + path) for path in paths[3:]],
        ])

    def test_readable_workflows_without_the_gate_are_not_predecessors(self):
        with mock.patch.object(check_gates, "git", side_effect=[
            "", ".github/workflows/other.yml\n", workflow("other"),
        ]):
            self.assertIsNone(check_gates.base_copy(WORKFLOW, GATE))

    def test_predecessor_parse_retains_unreadable_lines_for_later_validation(self):
        original = workflow(GATE) + "  1unreadable:\n"
        with mock.patch.object(check_gates, "git", side_effect=[
            "", ".github/workflows/old.yml\n", original,
        ]):
            self.assertEqual(check_gates.base_copy(WORKFLOW, GATE), original)

    def test_multiple_predecessors_refuse_with_the_original_diagnostic(self):
        with mock.patch.object(check_gates, "git", side_effect=[
            "", ".github/workflows/a.yml\n.github/workflows/b.yaml\n",
            workflow(GATE), workflow(GATE),
        ]) as git:
            with self.assertRaises(SystemExit) as caught:
                check_gates.base_copy(WORKFLOW, GATE)
        self.assertEqual(
            caught.exception.code,
            f"More than one base workflow has a job named {GATE}; the gate check needs updating",
        )
        self.assertEqual(len(git.call_args_list), 4)

    def test_git_failures_never_waive_base_checking(self):
        for answers in ([None], [WORKFLOW, None], ["", None], ["", ".github/workflows/old.yml", None]):
            with self.subTest(answers=answers):
                error = subprocess.CalledProcessError(128, ["git", "fixture"])
                with mock.patch.object(check_gates, "git", side_effect=[
                    error if answer is None else answer for answer in answers
                ]) as git:
                    with self.assertRaises(subprocess.CalledProcessError) as caught:
                        check_gates.base_copy(WORKFLOW, GATE)
                self.assertIs(caught.exception, error)
                self.assertEqual(git.call_count, len(answers))


class WorkflowFileTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="ozzit-gate-paths-")
        self.addCleanup(directory.cleanup)
        self.directory = Path(directory.name)
        (self.directory / "current.yml").write_bytes(b"fixture\n")
        (self.directory / "notes.txt").write_bytes(b"unfiltered extension\n")
        (self.directory / "nested").mkdir()
        (self.directory / "nested" / "child.yml").write_bytes(b"nested\n")
        with os.scandir(self.directory) as entries:
            self.entries = list(entries)

    def test_returns_the_directory_entry_path_without_an_extension_policy(self):
        before = snapshot(self.directory)
        with mock.patch.object(check_gates.os, "scandir", return_value=self.entries) as scan:
            self.assertEqual(check_gates.workflow_file(WORKFLOW), str(self.directory / "current.yml"))
            self.assertEqual(
                check_gates.workflow_file(".github/workflows/notes.txt"),
                str(self.directory / "notes.txt"),
            )
        self.assertEqual(scan.call_args_list, [mock.call(".github/workflows/")] * 2)
        self.assertEqual(snapshot(self.directory), before)

    def test_rejects_absent_directory_nested_and_alternate_prefix_operands(self):
        before = snapshot(self.directory)
        for operand in (
            "current.yml", ".github/workflows/absent.yml", ".github/workflows/nested",
            ".github/workflows/nested/child.yml", ".github/workflows/../current.yml",
            "./.github/workflows/current.yml", str(self.directory / "current.yml"),
            ".github/workflows/", ".github/workflow/current.yml",
        ):
            with self.subTest(operand=operand):
                with mock.patch.object(check_gates.os, "scandir", return_value=self.entries):
                    with self.assertRaises(SystemExit) as caught:
                        check_gates.workflow_file(operand)
                self.assertEqual(caught.exception.code, f"{operand} is not a file in .github/workflows")
                self.assertEqual(snapshot(self.directory), before)

    def test_directory_read_failure_propagates(self):
        error = PermissionError("fixture directory is unreadable")
        with mock.patch.object(check_gates.os, "scandir", side_effect=error):
            with self.assertRaises(PermissionError) as caught:
                check_gates.workflow_file(WORKFLOW)
        self.assertIs(caught.exception, error)


class MainTests(unittest.TestCase):
    def invoke(self, *, text=None, argv=None, results=None, environment=None, base=None,
               base_error=None, path_error=None):
        if environment is None:
            environment = {
                "RESULTS": json.dumps({"lint": {"result": "success"}} if results is None else results),
                "GITHUB_EVENT_NAME": "push",
            }
        printed = io.StringIO()
        resolver = mock.Mock(return_value="owned/current.yml", side_effect=path_error)
        opener = mock.mock_open(read_data=workflow(GATE, "lint") if text is None else text)
        predecessor = mock.Mock(return_value=base, side_effect=base_error)
        error = None
        with (
            mock.patch.object(check_gates, "os", SimpleNamespace(environ=environment)),
            mock.patch.object(check_gates, "workflow_file", resolver),
            mock.patch.object(check_gates, "open", opener, create=True),
            mock.patch.object(check_gates, "base_copy", predecessor),
            redirect_stdout(printed),
        ):
            try:
                result = check_gates.main([WORKFLOW, GATE] if argv is None else argv)
                self.assertIsNone(result)
            except (Exception, SystemExit) as caught:
                error = caught
        return SimpleNamespace(error=error, stdout=printed.getvalue(), path=resolver,
                               open=opener, base=predecessor)

    def assert_refusal(self, result, message):
        self.assertIsInstance(result.error, SystemExit)
        self.assertEqual(result.error.code, message)
        result.base.assert_not_called()

    def test_success_reads_the_resolved_path_as_utf8_without_printing(self):
        result = self.invoke()
        self.assertIsNone(result.error)
        self.assertEqual(result.stdout, "")
        result.path.assert_called_once_with(WORKFLOW)
        result.open.assert_called_once_with("owned/current.yml", encoding="utf-8")
        result.base.assert_not_called()

    def test_all_non_success_results_fail_in_sorted_name_order_before_io(self):
        statuses = ["failure", "cancelled", "skipped", "timed_out", "neutral", "", None]
        results = {f"job{6-i}": {"result": status} for i, status in enumerate(statuses)}
        results["passed"] = {"result": "success"}
        result = self.invoke(results=results)
        self.assert_refusal(result, "Jobs did not succeed: " + ", ".join(f"job{i}" for i in range(7)))
        self.assertEqual(result.stdout, "")
        result.path.assert_not_called()
        result.open.assert_not_called()

    def test_no_results_is_removed_everywhere_and_skips_only_statuses(self):
        result = self.invoke(
            text=workflow(GATE, "lint", "optional"), results={"lint": {}},
            argv=["--no-results", WORKFLOW, GATE, "--no-results", "--exempt", "optional", "--no-results"],
        )
        self.assertIsNone(result.error)
        result.path.assert_called_once_with(WORKFLOW)
        missing = self.invoke(results={}, argv=[WORKFLOW, GATE, "--no-results"])
        self.assert_refusal(missing, "Add these jobs to needs: lint")

    def test_results_environment_and_json_are_required_even_with_no_results(self):
        for environment, expected in [({}, KeyError), ({"RESULTS": "not JSON"}, json.JSONDecodeError)]:
            with self.subTest(environment=environment):
                result = self.invoke(environment=environment, argv=[WORKFLOW, GATE, "--no-results"])
                self.assertIsInstance(result.error, expected)
                result.path.assert_not_called()
                result.open.assert_not_called()
                result.base.assert_not_called()
                self.assertEqual(result.stdout, "")

    def test_malformed_arguments_and_result_shapes_keep_their_failure_types(self):
        for argv in ([], [WORKFLOW], [WORKFLOW, GATE, "extra"], ["--exempt", "lint"]):
            with self.subTest(argv=argv):
                result = self.invoke(argv=argv)
                self.assertIsInstance(result.error, ValueError)
                result.path.assert_not_called()
        for raw, expected in [
            ("[]", AttributeError), ("null", AttributeError), ('{"lint": {}}', KeyError),
            ('{"lint": null}', TypeError),
        ]:
            with self.subTest(raw=raw):
                result = self.invoke(environment={"RESULTS": raw})
                self.assertIsInstance(result.error, expected)
                result.path.assert_not_called()

    def test_exemptions_waive_only_missing_needs(self):
        argv = [WORKFLOW, GATE, "--exempt", "optional", "optional"]
        result = self.invoke(text=workflow(GATE, "lint", "optional"), argv=argv)
        self.assertIsNone(result.error)
        failure = self.invoke(argv=argv, results={"optional": {"result": "failure"}})
        self.assert_refusal(failure, "Jobs did not succeed: optional")
        live = self.invoke(text=workflow(GATE, "lint", comments="# removed-jobs: optional\n")
                           + "  optional:\n", argv=argv)
        self.assert_refusal(live, "These removed-jobs declarations name jobs that still exist: optional")

    def test_workflow_resolution_failure_precedes_open_and_base(self):
        error = SystemExit("fixture path refusal")
        result = self.invoke(path_error=error)
        self.assertIs(result.error, error)
        result.open.assert_not_called()
        result.base.assert_not_called()

    def test_workflow_parser_and_gate_refusals_precede_missing_needs(self):
        for text, message in [
            ("name: no jobs\n", "Cannot find the jobs block; the gate check needs updating"),
            (workflow(GATE) + "  1bad:\n", "Cannot read the job key '  1bad:'; the gate check needs updating"),
            (workflow("missing"), f"Cannot find {GATE} in {WORKFLOW}; the gate check needs updating"),
        ]:
            with self.subTest(text=text):
                self.assert_refusal(self.invoke(text=text), message)

    def test_missing_needs_are_sorted_and_precede_live_declarations(self):
        result = self.invoke(text=workflow(GATE, "zeta", "alpha", comments="# removed-jobs: alpha\n"))
        self.assert_refusal(result, "Add these jobs to needs: alpha, zeta")

    def test_live_declarations_are_sorted_and_precede_base_lookup(self):
        result = self.invoke(
            text=workflow(GATE, "zeta", "alpha", comments="  # removed-jobs: zeta, alpha, zeta\n"),
            results={name: {"result": "success"} for name in ["zeta", "alpha"]},
        )
        self.assert_refusal(result, "These removed-jobs declarations name jobs that still exist: alpha, zeta")

    def test_only_pull_request_looks_up_the_base_workflow(self):
        for event in (None, "push", "pull_request_target", "workflow_dispatch", "pull_request"):
            with self.subTest(event=event):
                environment = {"RESULTS": '{"lint":{"result":"success"}}'}
                if event is not None:
                    environment["GITHUB_EVENT_NAME"] = event
                result = self.invoke(environment=environment, base=workflow(GATE, "lint"))
                self.assertIsNone(result.error)
                self.assertEqual(result.stdout, "")
                if event == "pull_request":
                    result.base.assert_called_once_with(WORKFLOW, GATE)
                else:
                    result.base.assert_not_called()

    def pull_request(self, **kwargs):
        return self.invoke(environment={
            "RESULTS": '{"lint":{"result":"success"}}', "GITHUB_EVENT_NAME": "pull_request",
        }, **kwargs)

    def test_new_workflow_without_a_base_succeeds(self):
        result = self.pull_request(base=None)
        self.assertIsNone(result.error)
        self.assertEqual(result.stdout, "")
        result.base.assert_called_once_with(WORKFLOW, GATE)

    def test_declared_removals_print_once_in_sorted_order(self):
        result = self.pull_request(
            text=workflow(GATE, "lint", comments="  # removed-jobs: zeta, alpha,zeta, unused,\n"),
            base=workflow(GATE, "lint", "zeta", "alpha"),
        )
        self.assertIsNone(result.error)
        self.assertEqual(result.stdout, f"Declared removals from {WORKFLOW}: alpha, zeta\n")
        result.base.assert_called_once_with(WORKFLOW, GATE)

    def test_undeclared_removals_have_the_exact_sorted_instruction(self):
        result = self.pull_request(base=workflow(GATE, "lint", "zeta", "alpha"))
        self.assertIsInstance(result.error, SystemExit)
        self.assertEqual(result.stdout, "")
        self.assertEqual(result.error.code,
            f"These jobs were removed from {WORKFLOW}: alpha, zeta. If that is intended, "
            "add the comment '# removed-jobs: alpha, zeta' to that file.")

    def test_mixed_removals_print_declared_names_before_refusing_the_rest(self):
        result = self.pull_request(
            text=workflow(GATE, "lint", comments="# removed-jobs: beta\n"),
            base=workflow(GATE, "lint", "beta", "alpha"),
        )
        self.assertIsInstance(result.error, SystemExit)
        self.assertEqual(result.stdout, f"Declared removals from {WORKFLOW}: beta\n")
        self.assertEqual(result.error.code,
            f"These jobs were removed from {WORKFLOW}: alpha. If that is intended, "
            "add the comment '# removed-jobs: alpha' to that file.")

    def test_exemption_cannot_waive_removal_and_other_comments_are_not_declarations(self):
        result = self.pull_request(
            text=workflow(GATE, "lint", comments="# removed-jobs-extra: old\n"),
            argv=[WORKFLOW, GATE, "--exempt", "old"], base=workflow(GATE, "lint", "old"),
        )
        self.assertIsInstance(result.error, SystemExit)
        self.assertEqual(result.stdout, "")
        self.assertEqual(result.error.code,
            f"These jobs were removed from {WORKFLOW}: old. If that is intended, "
            "add the comment '# removed-jobs: old' to that file.")

    def test_base_git_failure_and_unreadable_base_are_not_waived(self):
        error = subprocess.CalledProcessError(128, ["git", "ls-tree", "HEAD^1"])
        result = self.pull_request(base_error=error)
        self.assertIs(result.error, error)
        self.assertEqual(result.stdout, "")
        unreadable = self.pull_request(base=workflow(GATE) + "  1bad:\n")
        self.assertIsInstance(unreadable.error, SystemExit)
        self.assertEqual(unreadable.error.code,
            "Cannot read the job key '  1bad:'; the gate check needs updating")


class PhysicalCommandTests(unittest.TestCase):
    def test_script_status_channels_and_owned_input_preservation(self):
        with tempfile.TemporaryDirectory(prefix="ozzit-gate-command-") as directory:
            root = Path(directory)
            path = root / WORKFLOW
            path.parent.mkdir(parents=True)
            path.write_bytes(workflow(GATE, "lint").encode())
            before = snapshot(root)
            for status, extra, code, stderr in [
                ("success", [], 0, ""),
                ("failure", [], 1, "Jobs did not succeed: lint\n"),
                ("failure", ["--no-results"], 0, ""),
            ]:
                with self.subTest(status=status, extra=extra):
                    with mock.patch.dict(os.environ, {
                        "RESULTS": json.dumps({"lint": {"result": status}}), "GITHUB_EVENT_NAME": "push",
                    }):
                        result = subprocess.run(
                            [sys.executable, "-I", str(SCRIPT), WORKFLOW, GATE, *extra],
                            cwd=root, capture_output=True, text=True, encoding="utf-8", timeout=30, check=False,
                        )
                    self.assertEqual(result.returncode, code)
                    self.assertEqual(result.stdout, "")
                    self.assertEqual(result.stderr, stderr)
                    self.assertEqual(snapshot(root), before)


if __name__ == "__main__":
    unittest.main()
