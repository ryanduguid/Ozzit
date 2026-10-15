"""Regression coverage for aggregate workflow predecessor selection."""

import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("check_gates", ROOT / ".github/ci/check_gates.py")
CHECK_GATES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK_GATES)
BASE = (
    "name: Verify workbook\n\non: pull_request\n\njobs:\n"
    "  verify-gates:\n    needs: [lint, verify, coverage]\n"
    "    runs-on: ubuntu-latest\n    steps: [{run: 'true'}]\n"
    + "".join(f"  {job}:\n    runs-on: ubuntu-latest\n    steps: [{{run: 'true'}}]\n"
              for job in ("coverage", "lint", "verify", "codecov"))
)


class GatePredecessorTests(unittest.TestCase):
    def head_workflow(self, gate, name, declaration=""):
        head = (f"name: Verify workbook\n\non: pull_request\n\njobs:\n  {gate}:\n"
                f"    name: {name}\n    runs-on: ubuntu-latest\n"
                "    steps: [{run: 'true'}]\n")
        if declaration:
            head = f"# removed-jobs: {declaration}\n" + head
        return head

    def run_gate(self, gate, name, declaration="", *, sources=None, head=None, filename="renamed.yml"):
        if sources is None:
            sources = {".github/workflows/verify.yml": BASE}
        if head is None:
            head = self.head_workflow(gate, name, declaration)

        git_outputs = {
            ("ls-tree", "--name-only", "HEAD^1", "--", f".github/workflows/{filename}"):
                filename if f".github/workflows/{filename}" in sources else "",
            ("ls-tree", "-r", "--name-only", "HEAD^1", "--", ".github/workflows/"):
                "\n".join(sources) + "\n",
            ("ls-tree", "-r", "--name-only", "-z", "HEAD^1", "--", ".github/workflows/"):
                "\0".join(sources) + "\0",
        }
        git_outputs.update({("show", f"HEAD^1:{path}"): text for path, text in sources.items()})

        with tempfile.TemporaryDirectory() as directory:
            workflow = Path(directory) / filename
            workflow.write_text(head, encoding="utf-8")
            with (patch.object(CHECK_GATES, "git", side_effect=lambda *args: git_outputs[args]),
                  patch.object(CHECK_GATES, "workflow_file", return_value=str(workflow)),
                  patch.dict(os.environ, {"RESULTS": json.dumps({}), "GITHUB_EVENT_NAME": "pull_request"})):
                CHECK_GATES.main([
                    f".github/workflows/{filename}", gate,
                    "--exempt", "codecov",
                ])

    def test_renamed_file_and_id_preserve_removal_checks_for_the_same_reported_name(self):
        with self.assertRaisesRegex(SystemExit, "coverage.*lint.*verify"):
            self.run_gate("replacement", "verify-gates")

    def test_renamed_file_with_original_id_still_rejects_undeclared_removals(self):
        with self.assertRaisesRegex(
            SystemExit, "codecov, coverage, lint, verify"
        ):
            self.run_gate("verify-gates", "verify-gates")

    def test_a_new_gate_with_a_distinct_reported_name_has_no_predecessor(self):
        self.run_gate("replacement", "new-gates")

    def test_digit_prefixed_static_names_can_identify_unrelated_new_gates(self):
        for name in ("2026 checks", "1st gate", "-1 release checks"):
            with self.subTest(name=name):
                self.run_gate("replacement", name)

    def test_unreadable_base_indentation_cannot_establish_absence(self):
        base = ("name: Base\n\njobs:\n    former:\n      name: verify-gates\n"
                "      runs-on: ubuntu-latest\n    critical:\n      runs-on: ubuntu-latest\n")
        with self.assertRaisesRegex(SystemExit, "Cannot read base workflow"):
            self.run_gate("replacement", "verify-gates", sources={".github/workflows/base.yml": base})

    def test_declared_removals_allow_the_same_reported_name(self):
        self.run_gate(
            "replacement", "verify-gates",
            "codecov, coverage, lint, verify, verify-gates",
        )

    def test_explicit_base_context_survives_a_different_internal_id(self):
        base = BASE.replace("  verify-gates:\n", "  former:\n    name: verify-gates\n")
        with self.assertRaisesRegex(SystemExit, "These jobs were removed"):
            self.run_gate("replacement", "verify-gates", sources={".github/workflows/base.yml": base})

    def test_quoted_context_names_preserve_removal_checks(self):
        for name in ('"Validate gates"', "'Validate gates'", '"verify-\\u0067ates"'):
            with self.subTest(name=name):
                base = BASE.replace("  verify-gates:\n", f"  former:\n    name: {name}\n")
                with self.assertRaisesRegex(SystemExit, "These jobs were removed"):
                    self.run_gate("replacement", name, sources={".github/workflows/base.yml": base})

    def test_a_matching_context_and_an_unrelated_matching_id_are_ambiguous(self):
        other = "name: Other\n\njobs:\n  replacement:\n    runs-on: ubuntu-latest\n"
        with self.assertRaisesRegex(SystemExit, "More than one base workflow"):
            self.run_gate("replacement", "verify-gates", sources={
                ".github/workflows/verify.yml": BASE,
                ".github/workflows/other.yml": other,
            })

    def test_possible_expression_contexts_cannot_waive_removals(self):
        base = BASE.replace("  verify-gates:\n", "  former:\n    name: verify${{ inputs.suffix }}\n")
        with self.assertRaisesRegex(SystemExit, "These jobs were removed"):
            self.run_gate("replacement", "verify-gates", sources={".github/workflows/base.yml": base})

    def test_disjoint_expression_contexts_do_not_create_a_predecessor(self):
        base = "name: Other\n\njobs:\n  former:\n    name: Other (${{ matrix.language }})\n"
        self.run_gate("replacement", "new-gates", sources={".github/workflows/base.yml": base})

    def test_expression_literal_ends_cannot_overlap(self):
        self.assertFalse(CHECK_GATES.possible_context("new-${{ matrix.flavour }}-gates", "new-gates"))
        self.assertTrue(CHECK_GATES.possible_context("new-${{ matrix.flavour }}gates", "new-gates"))

    def test_unclosed_expression_remains_unknown(self):
        self.assertTrue(CHECK_GATES.possible_context("}}${{ inputs.context", "verify-gates"))

    def test_impossible_expression_predecessor_does_not_block_a_new_gate(self):
        other = "name: Other\n\njobs:\n  former:\n    name: new-${{ matrix.flavour }}-gates\n"
        self.run_gate("replacement", "new-gates", sources={
            ".github/workflows/verify.yml": BASE,
            ".github/workflows/other.yml": other,
        })

    def test_tab_comments_and_blank_lines_do_not_hide_the_head_name(self):
        for separator in ("\t# separator", "\t", " \t# separator"):
            with self.subTest(separator=separator):
                head = self.head_workflow("replacement", "verify-gates").replace(
                    "  replacement:\n", "  replacement:\n" + separator + "\n", 1)
                with self.assertRaisesRegex(SystemExit, "These jobs were removed"):
                    self.run_gate("replacement", "verify-gates", head=head)

    def test_tab_comment_does_not_hide_a_base_name(self):
        base = BASE.replace("  verify-gates:\n",
                            "  former:\n\t# separator\n    name: verify-gates\n", 1)
        with self.assertRaisesRegex(SystemExit, "These jobs were removed"):
            self.run_gate("replacement", "verify-gates", sources={".github/workflows/old.yml": base})

    def test_occupied_destination_cannot_suppress_a_context_predecessor(self):
        other = "name: Fuzzing\n\njobs:\n  fuzzing:\n    runs-on: ubuntu-latest\n"
        with self.assertRaisesRegex(SystemExit, "More than one base workflow"):
            self.run_gate("fuzzing", "verify-gates", filename="fuzzing.yml", sources={
                ".github/workflows/fuzzing.yml": other,
                ".github/workflows/verify.yml": BASE,
            })

    def test_same_path_candidate_is_deduplicated_and_accepts_declarations(self):
        self.run_gate("replacement", "verify-gates",
                      "codecov, coverage, lint, verify, verify-gates", filename="verify.yml")

    def test_equivalent_quoted_and_implicit_names_match(self):
        for base_name, head_name in ((None, '"verify-\\u0067ates"'),
                                     ('"verify-\\u0067ates"', "verify-gates"),
                                     ("'verify-gates'", '"verify-gates"')):
            with self.subTest(base_name=base_name, head_name=head_name):
                base = BASE if base_name is None else BASE.replace(
                    "  verify-gates:\n", f"  former:\n    name: {base_name}\n", 1)
                with self.assertRaisesRegex(SystemExit, "These jobs were removed"):
                    self.run_gate("replacement", head_name, sources={".github/workflows/base.yml": base})
        self.run_gate("replacement", '"different-gates"')

    def test_unknown_base_context_syntax_cannot_waive_removals(self):
        for name in ("*context", ">-\n      verify-gates", "verify-\n      gates"):
            with self.subTest(name=name):
                base = BASE.replace("  verify-gates:\n", f"  former:\n    name: {name}\n")
                with self.assertRaisesRegex(SystemExit, "These jobs were removed"):
                    self.run_gate("replacement", "verify-gates", sources={".github/workflows/base.yml": base})

    def test_unknown_head_contexts_fail_before_declaring_a_new_gate(self):
        for name in ("${{ inputs.context }}", "*context", ">-\n      verify-gates",
                     "verify-\n      gates", "verify-gates\n    name: different"):
            with self.subTest(name=name):
                with self.assertRaisesRegex(SystemExit, "Cannot determine the gate context"):
                    self.run_gate("replacement", name)

    def test_flow_mapping_head_cannot_hide_a_context(self):
        head = ("name: Validate\n\njobs:\n  replacement: {name: verify-gates, "
                "runs-on: ubuntu-latest, steps: [{run: 'true'}]}\n")
        with self.assertRaisesRegex(SystemExit, "Cannot determine the gate context"):
            self.run_gate("replacement", "verify-gates", head=head)

    def test_unsupported_property_indentation_cannot_hide_a_base_context(self):
        for indent in ("   ", "      "):
            with self.subTest(indent=len(indent)):
                base = ("name: Base\n\njobs:\n  former:\n" + indent + "name: verify-gates\n"
                        + indent + "runs-on: ubuntu-latest\n  critical:\n"
                        + indent + "runs-on: ubuntu-latest\n")
                with self.assertRaisesRegex(SystemExit, "These jobs were removed"):
                    self.run_gate("replacement", "verify-gates",
                                  sources={".github/workflows/base.yml": base})

    def test_unsupported_property_indentation_cannot_hide_a_head_context(self):
        for indent in ("   ", "      "):
            with self.subTest(indent=len(indent)):
                head = ("name: Head\n\njobs:\n  replacement:\n" + indent + "name: verify-gates\n"
                        + indent + "runs-on: ubuntu-latest\n")
                with self.assertRaisesRegex(SystemExit, "Cannot determine the gate context"):
                    self.run_gate("replacement", "verify-gates", head=head)

    def run_cli_fixture(self, *, quoted_path=False, occupied=False):
        git_executable = shutil.which("git")
        self.assertIsNotNone(git_executable, "Git is required for the gate CLI regression")
        with tempfile.TemporaryDirectory(prefix="gate-predecessor-") as directory:
            outer = Path(directory).resolve()
            self.assertEqual(outer.parent, Path(tempfile.gettempdir()).resolve())
            repo = outer / "repo"
            repo.mkdir()
            home = outer / "home"
            home.mkdir()
            env = {key: os.environ[key] for key in ("PATH", "SystemRoot", "TEMP", "TMP")
                   if key in os.environ}
            env.update(HOME=str(home), USERPROFILE=str(home), GIT_CONFIG_NOSYSTEM="1",
                       GIT_CONFIG_GLOBAL=os.devnull, GITHUB_EVENT_NAME="pull_request", RESULTS="{}")

            git_prefix = [rtk, "proxy", "git"] if (rtk := shutil.which("rtk")) else [git_executable]

            def git_command(*args):
                return subprocess.run([*git_prefix, *args], cwd=repo, env=env,
                                      capture_output=True, text=True, encoding="utf-8",
                                      check=True, timeout=10)

            workflows = repo / ".github/workflows"
            workflows.mkdir(parents=True)
            source_workflows = ROOT / ".github/workflows"
            for source in sorted([*source_workflows.glob("*.yml"),
                                  *source_workflows.glob("*.yaml")]):
                (workflows / source.name).write_bytes(source.read_bytes())
            base_name = "v\u00e9rify.yml" if quoted_path else "verify.yml"
            base = (source_workflows / "verify.yml").read_text(encoding="utf-8")
            if quoted_path:
                base = base.replace(".github/workflows/verify.yml", f".github/workflows/{base_name}")
                (workflows / "verify.yml").unlink()
                (workflows / base_name).write_text(base, encoding="utf-8")
            old = workflows / base_name
            git_command("init", "--quiet", "--initial-branch=main")
            git_command("config", "core.quotePath", "true")
            git_command("add", "--", ".github/workflows")
            identity = ("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                        "-c", "commit.gpgsign=false")
            git_command(*identity, "commit", "--quiet", "-m", "Base fixture")
            old.unlink()
            destination = "fuzzing.yml" if occupied else "renamed.yml"
            gate = "fuzzing" if occupied else "replacement"
            head = self.head_workflow(gate, "verify-gates")
            (workflows / destination).write_text(head, encoding="utf-8")
            git_command("add", "--all", "--", ".github/workflows")
            git_command(*identity, "commit", "--quiet", "-m", "Rename fixture")
            self.assertEqual(git_command("show", f"HEAD^1:.github/workflows/{base_name}").stdout,
                             base)
            changes = git_command("diff", "--no-renames", "--name-status", "-z", "HEAD^1", "HEAD").stdout.split("\0")[:-1]
            self.assertEqual(
                set(zip(changes[::2], changes[1::2])),
                {("D", f".github/workflows/{base_name}"),
                 ("M" if occupied else "A", f".github/workflows/{destination}")},
            )
            self.assertEqual((workflows / destination).read_text(encoding="utf-8"), head)
            if quoted_path:
                self.assertIn('".github/workflows/v\\303\\251rify.yml"', git_command(
                    "ls-tree", "-r", "--name-only", "HEAD^1", "--", ".github/workflows/").stdout)
            result = subprocess.run([
                sys.executable, "-I", str(ROOT / ".github/ci/check_gates.py"),
                f".github/workflows/{destination}", gate,
                "--exempt", "codecov",
            ], cwd=repo, env=env, capture_output=True, text=True, timeout=10)
            return result

    def test_cli_rejects_the_rename_in_a_real_git_parent(self):
        result = self.run_cli_fixture()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("These jobs were removed", result.stderr)
        self.assertIn("verify-gates", result.stderr)

    def test_cli_rejects_an_occupied_destination_bypass(self):
        result = self.run_cli_fixture(occupied=True)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("More than one base workflow", result.stderr)

    def test_cli_preserves_removals_from_a_git_quoted_path(self):
        result = self.run_cli_fixture(quoted_path=True)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("These jobs were removed", result.stderr)
