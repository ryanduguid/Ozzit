import ast
import json
import re
import subprocess
import unittest
import zipfile
from datetime import datetime
from pathlib import Path

from ruamel.yaml import YAML
from ruamel.yaml.error import YAMLError

ROOT = Path(__file__).resolve().parents[2]
SECURITY = ROOT / "SECURITY.md"
RELEASING = ROOT / "RELEASING.md"
VERIFY_WORKFLOW = ROOT / ".github" / "workflows" / "verify.yml"
ATTEST_WORKFLOW = ROOT / ".github" / "workflows" / "attest-release.yml"
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
WORKBOOK_BASE = ROOT / "release" / "workbook-base.json"


EXPECTED_VERIFY_COMMANDS = (
    "python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-ruff.txt",
    "python -m ruff check .",
    "python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-mypy.txt",
    "python -m mypy --config-file mypy.ini",
    "python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-citation.txt",
    "python -m unittest discover -s tools/tests -v",
    "rustup toolchain install",
    "cargo fmt --manifest-path tools/ozzit/Cargo.toml --check",
    "cargo clippy --locked --manifest-path tools/ozzit/Cargo.toml --all-targets -- -D warnings",
    "cargo test --locked --manifest-path tools/ozzit/Cargo.toml",
    "cargo build --locked --release --manifest-path tools/ozzit/Cargo.toml",
    "tools/ozzit/target/release/ozzit verify_workbook ozzit.xlsx",
    "tools/ozzit/target/release/ozzit verify_sources ozzit.xlsx src",
    "tools/ozzit/target/release/ozzit verify_signatures src",
    "tools/ozzit/target/release/ozzit verify_previous_names functions.csv",
    "tools/ozzit/target/release/ozzit verify_index ozzit.xlsx src functions.csv",
    "tools/ozzit/target/release/ozzit verify_cover ozzit.xlsx functions.csv CHANGELOG.md",
    "tools/ozzit/target/release/ozzit verify_afe ozzit.xlsx src",
    "tools/ozzit/target/release/ozzit verify_help_spills ozzit.xlsx",
    "tools/ozzit/target/release/ozzit verify_native_evidence",
    "python .github/ci/compare_gates.py",
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
    ignore:
      # The release-policy pin is an immutable full commit SHA, advanced only
      # by a reviewed re-pin of its workflow family. A Dependabot bump adds an
      # unapproved SHA to the portfolio audit without changing the workflow,
      # so re-pin by hand instead.
      - dependency-name: "ryanduguid/release-policy/.github/workflows/*"

  # pyproject.toml is the Python manifest, and pip is how the verify workflow
  # installs. Grouped into one pull request a week, as the sibling repositories
  # group their Python updates.
  #
  # This finds nothing today. pyproject.toml carries tool configuration and
  # declares no dependencies, and the only pinned packages are the ruff and mypy
  # versions the lint job installs by command, which the pip ecosystem does not
  # read. The entry is here so the first declared dependency is covered from the
  # commit that declares it rather than whenever someone remembers.
  - package-ecosystem: pip
    directory: /
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
    open-pull-requests-limit: 2
    groups:
      python-dependencies:
        patterns:
          - "*"

  - package-ecosystem: cargo
    directory: /tools/ozzit
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
    open-pull-requests-limit: 2
    groups:
      rust-dependencies:
        patterns:
          - "*"

  - package-ecosystem: cargo
    directory: /tools/ozzit/fuzz
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
    open-pull-requests-limit: 2
    groups:
      rust-dependencies:
        patterns:
          - "*"
"""


def read_utf8(path: Path) -> str:
    return path.read_bytes().decode("utf-8")


def workflow_commands(workflow: str) -> tuple[str, ...]:
    """Read every scalar and block ``run`` gate without a YAML dependency."""
    # The aggregate job checks job results, not the repository, so its scripts
    # are not contributor commands; nor are the CI-only coverage jobs after it.
    workflow = re.split(r"(?m)^  [\w-]+-gates:$", workflow, maxsplit=1)[0]
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
            "tools/ozzit/target/release/ozzit verify_afe ozzit.xlsx src",
            "tools/ozzit/target/release/ozzit verify_afe ozzit.xlsx changed-src",
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
        marker = "        run: python .github/ci/compare_gates.py"
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
        self.assertIn("exactly 3 files", releasing)
        self.assertIn("tools/ozzit/target/release/ozzit prepare_release_bundle create", releasing)
        self.assertIn("tools/ozzit/target/release/ozzit prepare_release_bundle verify", releasing)
        self.assertNotIn("Ozzit-<version>-source.zip", releasing)
        self.assertNotIn("Ozzit-<version>-verification.txt", releasing)

    def test_workflow_and_dependabot_match_reviewed_controls(self):
        workflow = read_utf8(VERIFY_WORKFLOW)
        self.assertIn("persist-credentials: false", workflow)
        self.assertIn("fetch-depth: 0", workflow)
        self.assertIn("tools/ozzit/target/release/ozzit verify_workbook ozzit.xlsx", workflow)
        self.assertIn("tools/ozzit/target/release/ozzit verify_sources ozzit.xlsx src", workflow)
        self.assertIn("python -m unittest discover -s tools/tests -v", workflow)
        self.assertEqual(read_utf8(DEPENDABOT).replace("\r\n", "\n"), DEPENDABOT_CANONICAL)

    def test_release_attestation_signs_the_tagged_workbook_on_a_draft_only(self):
        # Scorecard's Signed-Releases check needs a provenance asset, and an
        # immutable release accepts none once published.
        text = read_utf8(ATTEST_WORKFLOW)
        workflow = YAML(typ="safe", pure=True).load(text)
        self.assertEqual(list(workflow["on"]), ["workflow_dispatch"])
        self.assertEqual(workflow["permissions"], {"contents": "read"})
        attest = workflow["jobs"]["attest"]
        self.assertEqual(attest["needs"], "checks")
        self.assertEqual(
            attest["permissions"],
            {"attestations": "write", "contents": "write", "id-token": "write"},
        )
        steps = {step.get("name", step.get("uses")): step for step in attest["steps"]}
        attestation = steps["Attest the tagged workbook"]
        self.assertEqual(
            attestation["uses"], "actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6"
        )
        self.assertEqual(
            attestation["with"], {"subject-path": "${{ runner.temp }}/subject/ozzit.xlsx"}
        )
        self.assertIn('git show "$GITHUB_SHA:ozzit.xlsx"', text)
        self.assertIn("python tools/prepare_release_bundle.py verify", text)
        self.assertIn(".verification.verified == true", text)
        self.assertIn(".github/workflows/verify.yml: verify-gates", text)
        self.assertIn("ozzit-${GITHUB_REF_NAME#v}.intoto.jsonl", text)
        self.assertEqual(text.count(".draft == true"), 3)
        self.assertNotIn("gh release edit", text)
        self.assertNotIn("--clobber", text)
        # The signed name must match the ref, and the upload must reach the verified
        # release by ID, after the draft and its assets are compared with the snapshot.
        self.assertIn(".tag == $tag", text)
        self.assertNotIn("gh release upload", text)
        self.assertIn("releases/$RELEASE_ID/assets", text)
        self.assertEqual(text.count("cmp -s - snapshot.json"), 2)
        releasing = read_utf8(RELEASING)
        self.assertIn("gh workflow run attest-release.yml --ref vX.Y.Z", releasing)
        self.assertIn("sits outside the checksum set", releasing)
        self.assertIn("gh api repos/ryanduguid/Ozzit/immutable-releases", releasing)

    def test_production_type_check_is_pinned_and_rejects_untyped_definitions(self):
        workflow = read_utf8(VERIFY_WORKFLOW)
        config = read_utf8(MYPY_CONFIG)

        self.assertIn("python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-mypy.txt", workflow)
        self.assertIn("mypy==2.4.0 \\", read_utf8(ROOT / "requirements-mypy.txt"))
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
        incomplete = [
            f"{path.relative_to(ROOT).as_posix()}:{function.lineno}:{function.name}"
            for path, function in functions
            if function.returns is None
            or any(
                parameter.annotation is None
                for parameter in (
                    *function.args.posonlyargs,
                    *function.args.args,
                    *function.args.kwonlyargs,
                    *(p for p in (function.args.vararg, function.args.kwarg) if p is not None),
                )
            )
        ]

        self.assertGreaterEqual(len(production), 20)
        self.assertGreaterEqual(len(functions), 146)
        self.assertEqual(incomplete, [])

    def test_editorconfig_exempts_the_bytes_the_afe_gate_compares(self):
        # verify_afe.py compares src/*.txt with the workbook's store byte for byte.
        # Under the [*] rules, saving a module in any editorconfig-honouring editor
        # rewrites exactly those bytes and fails the gate with no hint why.
        config = read_utf8(EDITORCONFIG)
        self.assertIn("[src/*.txt]", config, ".editorconfig does not scope src/*.txt")
        section = re.split(r"(?m)^\[", config.split("[src/*.txt]", 1)[1], maxsplit=1)[0]
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

    def test_citation_distinguishes_an_unreleased_candidate_from_release_history(self):
        tags = subprocess.run(
            ["git", "tag", "--points-at", "HEAD"],
            cwd=ROOT, capture_output=True, check=True, text=True,
        ).stdout.split()
        self.assert_citation_matches_history(
            read_utf8(CHANGELOG), read_utf8(CITATION), read_utf8(README), tags,
        )

    def test_citation_policy_checks_published_metadata_and_candidate_tags(self):
        changelog = "## v1.2.3, 9 May 2024, synthetic release\n"
        citation = "version: 1.2.3\ndate-released: 2024-05-09\n"
        readme = "releases/tag/v1.2.3 dated 9 May 2024\n"
        self.assert_citation_matches_history(
            changelog, citation, readme, ["unrelated-tag", "v1.2.3"],
        )
        with self.assertRaises(AssertionError):
            self.assert_citation_matches_history(changelog, citation, readme, ["v1.2.4"])
        with self.assertRaises(AssertionError):
            self.assert_citation_matches_history(
                changelog, citation.replace("2024-05-09", "2024-05-10"), readme, [],
            )

        changelog = "## Unreleased: v1.3.0 candidate\n" + changelog
        citation = "version: 1.3.0\nlicense: Apache-2.0\nabstract: unreleased\n"
        readme += "v1.3.0 candidate is unreleased; historical version under MIT\n"
        self.assert_citation_matches_history(changelog, citation, readme, [])
        with self.assertRaises(AssertionError):
            self.assert_citation_matches_history(changelog, citation, readme, ["v1.3.0"])

    def test_candidate_citation_rejects_quoted_release_dates(self):
        changelog = "## Unreleased: v1.3.0 candidate\n## v1.2.3, 9 May 2024, synthetic release\n"
        citation = "version: 1.3.0\nlicense: Apache-2.0\nabstract: unreleased\n"
        readme = (
            "releases/tag/v1.2.3 dated 9 May 2024\n"
            "v1.3.0 candidate is unreleased; historical version under MIT\n"
        )
        for key in ('"date-released"', "'date-released'", '"date-\\x72eleased"'):
            with self.subTest(key=key), self.assertRaises(AssertionError):
                self.assert_citation_matches_history(
                    changelog, citation + f"{key}: 2024-05-09\n", readme, [],
                )

    def assert_citation_matches_history(self, changelog, citation, readme, tags):
        try:
            metadata = YAML(typ="safe", pure=True).load(citation)
        except YAMLError as error:
            self.fail(f"CITATION.cff is invalid YAML: {error}")
        self.assertIsInstance(metadata, dict, "CITATION.cff must be a mapping")

        heading = re.search(r"^## v(\d+\.\d+\.\d+), (\d{1,2} \w+ \d{4}),", changelog, re.M)
        self.assertIsNotNone(heading, "CHANGELOG.md has no dated release heading")
        version, written = heading.group(1), heading.group(2)
        released = datetime.strptime(written, "%d %B %Y").date()

        candidate = re.search(r"^## Unreleased: v(\d+\.\d+\.\d+) candidate$", changelog, re.M)
        if candidate:
            self.assertEqual(metadata.get("version"), candidate.group(1))
            self.assertNotIn("date-released", metadata)
            self.assertIn("unreleased", citation)
            self.assertEqual(metadata.get("license"), "Apache-2.0")
        else:
            self.assertEqual(metadata.get("version"), version)
            self.assertEqual(str(metadata.get("date-released")), released.isoformat())

        self.assertIn(f"releases/tag/v{version}", readme)
        self.assertIn(f"dated {written}", readme)
        if candidate:
            self.assertIn(f"v{candidate.group(1)} candidate is unreleased", readme)
            self.assertIn("under MIT", readme)

        # At a release commit the tag is the last word on which release this is.
        releases = [tag for tag in tags if re.fullmatch(r"v\d+\.\d+\.\d+", tag)]
        if candidate:
            self.assertEqual(releases, [], "an unreleased candidate must not have a release tag")
        self.assertEqual(
            releases,
            [f"v{version}"] * len(releases),
            "the tagged commit publishes another version",
        )

    def test_citation_policy_rejects_invalid_or_duplicate_yaml(self):
        changelog = "## v1.2.3, 9 May 2024, synthetic release\n"
        citation = "version: 1.2.3\ndate-released: 2024-05-09\n"
        readme = "releases/tag/v1.2.3 dated 9 May 2024\n"
        for invalid in ("", "[]", citation + "broken: [", citation + '"version": 1.2.3\n'):
            with self.subTest(citation=invalid), self.assertRaises(AssertionError):
                self.assert_citation_matches_history(changelog, invalid, readme, [])
        self.assert_citation_matches_history(
            changelog, '"version": 1.2.3\n"date-released": "2024-05-09"\n', readme, [],
        )

    def test_readme_links_the_disclaimer(self):
        # The disclaimer states what the library is not advice about. A reader who
        # never reaches it is the reader it was written for, so the README links it.
        self.assertTrue((ROOT / "DISCLAIMER.md").is_file(), "DISCLAIMER.md is required")
        self.assertIn("[DISCLAIMER.md](DISCLAIMER.md)", read_utf8(README))

    def test_readme_publishes_the_tracked_workbook_digest_as_well_as_the_release_asset(self):
        # The release asset and the tracked workbook are different files whenever
        # main is ahead of the tag. Publishing only the asset digest left readers
        # unable to check the file they clone, so the README states both and this
        # test keeps the tracked one equal to the manifest the gates already check.
        base = json.loads(read_utf8(WORKBOOK_BASE))
        readme = read_utf8(README)
        self.assertIn(f"`{base['sha256']}` ({base['size']:,} bytes)", readme)
        self.assertIn("release/workbook-base.json", readme)


class RepositoryAttributionTests(unittest.TestCase):

    def test_workbook_creator_credits_the_project(self):
        with zipfile.ZipFile(ROOT / "ozzit.xlsx") as archive:
            core = archive.read("docProps/core.xml")
        self.assertIn(b"<dc:creator>Ozzit project</dc:creator>", core)


if __name__ == "__main__":
    unittest.main()
