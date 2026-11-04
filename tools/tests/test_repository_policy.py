# Copyright 2026 Ryan Duguid
# SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

"""Repository policy checks that need Python's YAML and syntax-tree parsers.

The rest of the repository policy runs as Rust, in
tools/ozzit/tests/repository/repository_policy.rs. These stay in Python: the
release-attestation and citation checks parse YAML with ruamel.yaml, which
requirements-citation.txt pins, and the signature check walks the production
tools with the ast module.
"""

import ast
import re
import subprocess
import unittest
from datetime import datetime
from pathlib import Path

from ruamel.yaml import YAML
from ruamel.yaml.error import YAMLError

ROOT = Path(__file__).resolve().parents[2]
RELEASING = ROOT / "RELEASING.md"
ATTEST_WORKFLOW = ROOT / ".github" / "workflows" / "attest-release.yml"
CITATION = ROOT / "CITATION.cff"
LICENSE_URL = "https://github.com/ryanduguid/Ozzit/blob/main/LICENSE"
CHANGELOG = ROOT / "CHANGELOG.md"
README = ROOT / "README.md"
TOOLS = ROOT / "tools"


def read_utf8(path: Path) -> str:
    return path.read_bytes().decode("utf-8")


class RepositoryPolicyTests(unittest.TestCase):
    def test_release_attestation_signs_the_tagged_workbook_on_a_draft_only(self):
        # Scorecard's Signed-Releases check needs a provenance asset, and an
        # immutable release accepts none once published.
        text = read_utf8(ATTEST_WORKFLOW)
        workflow = YAML(typ="safe", pure=True).load(text)
        self.assertEqual(list(workflow["on"]), ["workflow_dispatch"])
        self.assertEqual(workflow["permissions"], {"contents": "read"})
        attest = workflow["jobs"]["attest"]
        self.assertEqual(attest["needs"], ["checks", "build"])
        # Compilation runs without write or OIDC permissions; the privileged job
        # only checks the built binary's digest and runs it.
        build = workflow["jobs"]["build"]
        self.assertEqual(build["permissions"], {"contents": "read"})
        self.assertIn("cargo build --locked --release", str(build["steps"]))
        self.assertNotIn("cargo", str(attest["steps"]))
        self.assertNotIn("setup-python", str(attest["steps"]))
        self.assertIn("needs.build.outputs.sha256", str(attest["steps"]))
        self.assertIn("sha256sum --check --strict", str(attest["steps"]))
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
        self.assertIn('"$RUNNER_TEMP/native/ozzit" prepare_release_bundle verify', text)
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

        self.assertIn(TOOLS / "pyxirr_comparison.py", production)
        self.assertIn(
            (TOOLS / "pyxirr_comparison.py", "main"),
            [(path, function.name) for path, function in functions],
        )
        self.assertEqual(incomplete, [])

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
        citation = f"version: 1.3.0\nlicense-url: {LICENSE_URL}\nabstract: unreleased\n"
        readme += "v1.3.0 candidate is unreleased; historical version under MIT\n"
        self.assert_citation_matches_history(changelog, citation, readme, [])
        with self.assertRaises(AssertionError):
            self.assert_citation_matches_history(changelog, citation, readme, ["v1.3.0"])

    def test_candidate_citation_rejects_quoted_release_dates(self):
        changelog = "## Unreleased: v1.3.0 candidate\n## v1.2.3, 9 May 2024, synthetic release\n"
        citation = f"version: 1.3.0\nlicense-url: {LICENSE_URL}\nabstract: unreleased\n"
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
            self.assertNotIn("license", metadata)
            self.assertEqual(metadata.get("license-url"), LICENSE_URL)
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


if __name__ == "__main__":
    unittest.main()
