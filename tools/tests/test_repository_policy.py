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

    def test_workflow_and_dependabot_match_reviewed_controls(self):
        workflow = read_utf8(VERIFY_WORKFLOW)
        self.assertIn("persist-credentials: false", workflow)
        self.assertIn("python tools/verify_workbook.py ozzit.xlsx", workflow)
        self.assertIn("python tools/verify_sources.py ozzit.xlsx src", workflow)
        self.assertIn("python -m unittest discover -s tools/tests -v", workflow)
        self.assertEqual(read_utf8(DEPENDABOT).replace("\r\n", "\n"), DEPENDABOT_CANONICAL)


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
