import ast
import re
import subprocess
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SECURITY = ROOT / "SECURITY.md"
RELEASING = ROOT / "RELEASING.md"
VERIFY_WORKFLOW = ROOT / ".github" / "workflows" / "verify.yml"
DEPENDABOT = ROOT / ".github" / "dependabot.yml"
MYPY_CONFIG = ROOT / "mypy.ini"
TOOLS = ROOT / "tools"


DEPENDABOT_CANONICAL = """version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
    open-pull-requests-limit: 2
    groups:
      # init and analyze are separate dependencies to Dependabot but must
      # move together: a job running one release against the other's config
      # fails outright. Grouped so the pair can never arrive split.
      codeql-action:
        patterns:
          - "github/codeql-action*"
"""


def read_utf8(path: Path) -> str:
    return path.read_bytes().decode("utf-8")


def physical_code_lines(paths):
    return sum(
        1
        for path in paths
        for line in read_utf8(path).splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    )


class RepositoryPolicyTests(unittest.TestCase):
    def test_security_and_releasing_docs_exist_and_name_the_reporting_path(self):
        security = read_utf8(SECURITY)
        releasing = read_utf8(RELEASING)
        self.assertTrue(SECURITY.is_file())
        self.assertTrue(RELEASING.is_file())
        self.assertIn("private vulnerability reporting", security.lower())
        self.assertIn("synthetic", security.lower())
        self.assertIn("SHA256SUMS", releasing)
        self.assertIn("git archive", releasing.lower())
        self.assertIn("exactly three files", releasing)
        self.assertIn("python tools/prepare_release_bundle.py create", releasing)
        self.assertIn("python tools/prepare_release_bundle.py verify", releasing)
        self.assertNotIn("Ozzit-<version>-source.zip", releasing)
        self.assertNotIn("Ozzit-<version>-verification.txt", releasing)

    def test_workflow_and_dependabot_match_reviewed_controls(self):
        workflow = read_utf8(VERIFY_WORKFLOW)
        self.assertIn("persist-credentials: false", workflow)
        self.assertIn("fetch-depth: 0", workflow)
        self.assertIn("python tools/verify_workbook.py ozzit.xlsx", workflow)
        self.assertIn("python tools/verify_sources.py ozzit.xlsx src", workflow)
        self.assertIn("python -m unittest discover -s tools/tests -v", workflow)
        self.assertEqual(read_utf8(DEPENDABOT).replace("\r\n", "\n"), DEPENDABOT_CANONICAL)

    def test_production_type_check_is_pinned_and_rejects_untyped_definitions(self):
        workflow = read_utf8(VERIFY_WORKFLOW)
        config = read_utf8(MYPY_CONFIG)

        self.assertIn('python -m pip install "mypy==2.3.1"', workflow)
        self.assertIn("python -m mypy --config-file mypy.ini", workflow)
        self.assertIn("files = tools", config)
        self.assertIn("exclude = ^tools[/\\\\]tests[/\\\\]", config)
        self.assertIn("disallow_untyped_defs = True", config)
        self.assertIn("check_untyped_defs = True", config)
        self.assertNotIn("disable_error_code", config)

    def test_every_production_function_has_a_complete_signature(self):
        production = sorted(
            path
            for path in TOOLS.rglob("*.py")
            if "tests" not in path.relative_to(TOOLS).parts
        )
        functions = [
            (path, function)
            for path in production
            for function in ast.walk(ast.parse(read_utf8(path), filename=str(path)))
            if isinstance(function, (ast.FunctionDef, ast.AsyncFunctionDef))
        ]
        incomplete = []
        for path, function in functions:
            parameters = [
                *function.args.posonlyargs,
                *function.args.args,
                *function.args.kwonlyargs,
            ]
            parameters.extend(
                parameter
                for parameter in (function.args.vararg, function.args.kwarg)
                if parameter is not None
            )
            if function.returns is None or any(
                parameter.annotation is None for parameter in parameters
            ):
                incomplete.append(
                    f"{path.relative_to(ROOT).as_posix()}:{function.lineno}:{function.name}"
                )

        self.assertGreaterEqual(len(production), 20)
        self.assertGreaterEqual(len(functions), 146)
        self.assertEqual(incomplete, [])

    def test_regression_tests_remain_proportionate_to_production_tools(self):
        production = [
            path
            for path in TOOLS.rglob("*.py")
            if "tests" not in path.relative_to(TOOLS).parts
        ]
        tests = list((TOOLS / "tests").rglob("*.py"))
        production_lines = physical_code_lines(production)
        test_lines = physical_code_lines(tests)

        self.assertGreaterEqual(
            test_lines / production_lines,
            0.50,
            f"test/tool physical-line ratio is {test_lines}/{production_lines}",
        )


class RepositoryAttributionTests(unittest.TestCase):

    NAME = re.compile(("H[a]t(?:maker|maekr)" + "|H[a]t(?:maker|maekr)").encode(), re.IGNORECASE)

    def tracked_files(self):
        listing = subprocess.run(
            ["git", "ls-files"],
            cwd=ROOT, capture_output=True, check=True, text=True,
        )
        for entry in listing.stdout.splitlines():
            if entry:
                yield ROOT / entry

    def test_no_tracked_file_carries_the_upstream_author_name(self):
        offenders = [
            path.relative_to(ROOT).as_posix()
            for path in self.tracked_files()
            if path.is_file() and self.NAME.search(path.read_bytes())
        ]
        self.assertEqual(offenders, [], f"legacy creator marker present in: {offenders}")

    def test_workbook_creator_credits_the_project(self):
        with zipfile.ZipFile(ROOT / "ozzit.xlsx") as archive:
            core = archive.read("docProps/core.xml")
        self.assertIn(b"<dc:creator>Ozzit project</dc:creator>", core)


if __name__ == "__main__":
    unittest.main()
