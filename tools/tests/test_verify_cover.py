import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
ROOT = TOOLS.parent
GATE = TOOLS / "verify_cover.py"


def rewrite_label(workbook: Path, old: str, new: str) -> None:
    with zipfile.ZipFile(workbook) as archive:
        parts = {name: archive.read(name) for name in archive.namelist()}
    strings = parts["xl/sharedStrings.xml"].decode("utf-8")
    assert strings.count(old) == 1
    parts["xl/sharedStrings.xml"] = strings.replace(old, new).encode("utf-8")
    with zipfile.ZipFile(workbook, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in parts.items():
            archive.writestr(name, data)


class VerifyCoverTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-cover-"))
        self.workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(ROOT / "ozzit.xlsx", self.workbook)
        self.changelog = self.directory / "CHANGELOG.md"
        shutil.copy2(ROOT / "CHANGELOG.md", self.changelog)

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def run_gate(self):
        return subprocess.run(
            [sys.executable, str(GATE), str(self.workbook), str(ROOT / "functions.csv"),
             str(self.changelog)],
            capture_output=True, text=True, check=False,
        )

    def test_the_committed_cover_matches_the_index_and_the_changelog(self):
        result = self.run_gate()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("133 functions", result.stdout)

    def test_a_stale_function_count_fails(self):
        rewrite_label(self.workbook, "133 functions", "130 functions")
        result = self.run_gate()
        self.assertEqual(result.returncode, 1)
        self.assertIn("cover says 130 functions", result.stdout)

    def test_the_cell_is_resolved_not_the_first_matching_string(self):
        # A current-looking label stored elsewhere must not rescue a stale cell.
        current = "Version 16 September 2026    -    133 functions    -    Microsoft 365 or Excel 2024 and later"
        stale = current.replace("16 September 2026", "20 August 2026").replace("133", "130")
        rewrite_label(self.workbook, f"<t>{current}</t>", f"<t>{stale}</t>")
        with zipfile.ZipFile(self.workbook) as archive:
            parts = {name: archive.read(name) for name in archive.namelist()}
        strings = parts["xl/sharedStrings.xml"].decode("utf-8")
        parts["xl/sharedStrings.xml"] = strings.replace(
            "</sst>", f"<si><t>{current}</t></si></sst>"
        ).encode("utf-8")
        with zipfile.ZipFile(self.workbook, "w", zipfile.ZIP_DEFLATED) as archive:
            for name, data in parts.items():
                archive.writestr(name, data)
        result = self.run_gate()
        self.assertEqual(result.returncode, 1)
        self.assertIn("cover says 130 functions", result.stdout)

    def test_a_byte_order_marked_index_is_read(self):
        index = self.directory / "functions.csv"
        index.write_bytes("﻿".encode("utf-8") + (ROOT / "functions.csv").read_bytes())
        result = subprocess.run(
            [sys.executable, str(GATE), str(self.workbook), str(index), str(self.changelog)],
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_a_stale_date_fails(self):
        text = self.changelog.read_text(encoding="utf-8")
        self.changelog.write_text(
            text.replace("16 September 2026", "17 September 2026", 1), encoding="utf-8"
        )
        result = self.run_gate()
        self.assertEqual(result.returncode, 1)
        self.assertIn("current cut is 17 September 2026", result.stdout)


if __name__ == "__main__":
    unittest.main()
