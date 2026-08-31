import ast
import re
import subprocess
import unittest
import zipfile
from datetime import datetime
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SECURITY = ROOT / "SECURITY.md"
RELEASING = ROOT / "RELEASING.md"
VERIFY_WORKFLOW = ROOT / ".github" / "workflows" / "verify.yml"
DEPENDABOT = ROOT / ".github" / "dependabot.yml"
MYPY_CONFIG = ROOT / "mypy.ini"
EDITORCONFIG = ROOT / ".editorconfig"
CITATION = ROOT / "CITATION.cff"
CHANGELOG = ROOT / "CHANGELOG.md"
README = ROOT / "README.md"
AGENTS = ROOT / "AGENTS.md"
CLAUDE = ROOT / "CLAUDE.md"
CONTRIBUTING = ROOT / "CONTRIBUTING.md"
SRC = ROOT / "src"
TOOLS = ROOT / "tools"


EXPECTED_VERIFY_COMMANDS = (
    'python -m pip install "mypy==2.3.1"',
    "python -m mypy --config-file mypy.ini",
    "python tools/verify_workbook.py ozzit.xlsx",
    "python tools/verify_sources.py ozzit.xlsx src",
    "python tools/verify_signatures.py src",
    "python tools/verify_previous_names.py functions.csv",
    "python tools/verify_index.py ozzit.xlsx src functions.csv",
    "python tools/verify_afe.py ozzit.xlsx src",
    "python -m unittest discover -s tools/tests -v",
)


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


def workflow_commands(workflow: str) -> tuple[str, ...]:
    """Read every scalar and block ``run`` gate without a YAML dependency."""
    lines = workflow.splitlines()
    commands: list[str] = []
    index = 0
    while index < len(lines):
        match = re.match(r"^\s*(?:-\s+)?run:\s*(.*?)\s*$", lines[index])
        if match is None:
            index += 1
            continue

        value = match.group(1)
        block = re.fullmatch(r"(?P<style>[|>])[+-]?", value)
        if block is None:
            commands.append(value)
            index += 1
            continue

        key_indent = lines[index].index("run:")
        cursor = index + 1
        while cursor < len(lines) and not lines[cursor].strip():
            cursor += 1
        if cursor == len(lines):
            commands.append("")
            break
        content_indent = len(lines[cursor]) - len(lines[cursor].lstrip())
        if content_indent <= key_indent:
            commands.append("")
            index = cursor
            continue

        body: list[str] = []
        while cursor < len(lines):
            line = lines[cursor]
            indent = len(line) - len(line.lstrip())
            if line.strip() and indent < content_indent:
                break
            if line.strip():
                body.append(line[content_indent:].rstrip())
            cursor += 1
        separator = "\n" if block.group("style") == "|" else " "
        commands.append(separator.join(body))
        index = cursor
    return tuple(commands)


def physical_code_lines(paths):
    return sum(
        1
        for path in paths
        for line in read_utf8(path).splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    )


class RepositoryPolicyTests(unittest.TestCase):
    def test_cross_runtime_contributor_guidance_preserves_workbook_authority(self):
        self.assertTrue(AGENTS.is_file(), "AGENTS.md is required")
        self.assertTrue(CLAUDE.is_file(), "CLAUDE.md is required")
        self.assertTrue(CONTRIBUTING.is_file(), "CONTRIBUTING.md is required")

        agents = read_utf8(AGENTS)
        contributing = read_utf8(CONTRIBUTING)
        self.assertIn("`ozzit.xlsx` is the shipped authority", agents)
        self.assertIn("Never fabricate Excel recalculation or cached-value evidence.", agents)
        self.assertIn("native Excel", agents)
        self.assertIn("no macros", agents)
        self.assertIn("RELEASING.md", agents)
        for document in (agents, contributing):
            normalised = re.sub(r"\s+", " ", document)
            self.assertIn("`tools/postbuild/README.md`", normalised)
            self.assertIn("sync the AFE store after any `src/` change", normalised)
            self.assertIn("sanitise the workbook last", normalised)
        commands = workflow_commands(read_utf8(VERIFY_WORKFLOW))
        self.assertEqual(commands, EXPECTED_VERIFY_COMMANDS)
        mutated = read_utf8(VERIFY_WORKFLOW).replace(
            "python tools/verify_afe.py ozzit.xlsx src",
            "python tools/verify_afe.py ozzit.xlsx changed-src",
            1,
        )
        self.assertNotEqual(commands, workflow_commands(mutated))
        for command in commands:
            self.assertIn(command, agents)

        self.assertEqual(read_utf8(CLAUDE), "@AGENTS.md\n")
        self.assertIn("`ozzit.xlsx` is the shipped authority", contributing)
        self.assertIn(
            "`src/*.txt`, the AFE store and `functions.csv`",
            contributing.replace("\n", " "),
        )
        self.assertIn(
            "Cached formula results may change only with Excel-backed recalculation evidence.",
            contributing,
        )
        for command in commands:
            self.assertIn(command, contributing)

    def test_workflow_command_parser_detects_non_python_and_multiline_gates(self):
        workflow = read_utf8(VERIFY_WORKFLOW)
        marker = "        run: python -m unittest discover -s tools/tests -v"
        mutated = workflow.replace(
            marker,
            marker
            + "\n      - name: Check PowerShell policy\n"
            + "        run: pwsh -File tools/check_policy.ps1\n"
            + "      - name: Check frontend assets\n"
            + "        run: |\n"
            + "          npm ci\n"
            + "          npm test",
            1,
        )

        self.assertEqual(
            workflow_commands(mutated)[-2:],
            ("pwsh -File tools/check_policy.ps1", "npm ci\nnpm test"),
        )

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

    def test_editorconfig_exempts_the_bytes_the_afe_gate_compares(self):
        # verify_afe.py compares src/*.txt with the workbook's store byte for byte.
        # Under the [*] rules, saving a module in any editorconfig-honouring editor
        # rewrites exactly those bytes and fails the gate with no hint why.
        config = read_utf8(EDITORCONFIG)
        self.assertIn("[src/*.txt]", config, ".editorconfig does not scope src/*.txt")
        section = config[config.index("[src/*.txt]"):]
        self.assertGreater(config.index("[src/*.txt]"), config.index("[*]"))
        self.assertIn("trim_trailing_whitespace = false", section)
        self.assertIn("insert_final_newline = false", section)

        modules = sorted(SRC.glob("*.txt"))
        self.assertGreaterEqual(len(modules), 6)
        padded = sum(
            1
            for path in modules
            for line in read_utf8(path).split("\n")
            if line != line.rstrip()
        )
        self.assertGreater(padded, 1000, "the exemption is only needed while padding is")
        self.assertTrue(
            any(not read_utf8(path).endswith("\n") for path in modules),
            "at least one module ends without a final newline",
        )

    def test_release_metadata_describes_the_newest_changelog_release(self):
        changelog = read_utf8(CHANGELOG)
        heading = re.search(r"^## v(\d+\.\d+\.\d+), (\d{1,2} \w+ \d{4}),", changelog, re.M)
        self.assertIsNotNone(heading, "CHANGELOG.md has no dated release heading")
        version, written = heading.group(1), heading.group(2)
        released = datetime.strptime(written, "%d %B %Y").date()

        citation = read_utf8(CITATION)
        self.assertIn(f"version: {version}\n", citation)
        self.assertIn(f"date-released: {released.isoformat()}\n", citation)

        readme = read_utf8(README)
        self.assertIn(f"releases/tag/v{version})", readme)
        self.assertIn(f"dated {written};", readme)

        # At a release commit the tag is the last word on which release this is.
        tags = subprocess.run(
            ["git", "tag", "--points-at", "HEAD"],
            cwd=ROOT, capture_output=True, check=True, text=True,
        ).stdout.split()
        releases = [tag for tag in tags if re.fullmatch(r"v\d+\.\d+\.\d+", tag)]
        for tag in releases:
            self.assertEqual(tag, f"v{version}", "the tagged commit publishes another version")

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


class UpstreamAttributionTests(unittest.TestCase):
    """The upstream author's name was removed under a written waiver."""

    NAME = re.compile(("hat" + "maker" + "|hat" + "maekr").encode(), re.IGNORECASE)

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
        self.assertEqual(offenders, [], f"upstream author name present in: {offenders}")

    def test_workbook_creator_credits_the_project(self):
        with zipfile.ZipFile(ROOT / "ozzit.xlsx") as archive:
            core = archive.read("docProps/core.xml")
        self.assertIn(b"<dc:creator>Ozzit project</dc:creator>", core)


if __name__ == "__main__":
    unittest.main()
