import csv
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

    def _column_index(self, columns, overrides=(), *, header_only=False, bom=False):
        with (ROOT / "functions.csv").open(encoding="utf-8-sig", newline="") as source:
            fields, *rows = list(csv.reader(source))
        target = self.directory / "functions.csv"
        with target.open("w", encoding="utf-8-sig" if bom else "utf-8", newline="") as output:
            writer = csv.writer(output, lineterminator="\n")
            writer.writerow(columns)
            for row in (() if header_only else rows):
                values = [row[fields.index(field)] if field in fields else "extra,\nvalue"
                          for field in columns]
                for position, value in overrides:
                    values[position] = value
                writer.writerow(values)
        return target

    def _run_index(self, index):
        return subprocess.run(
            [sys.executable, str(GATE), str(self.workbook), str(index), str(self.changelog)],
            capture_output=True, text=True, check=False,
        )

    def test_repeated_function_columns_fail_before_counting_rows(self):
        for columns, overrides, empty in (
            (["function", "function"], ((0, ""),), False),
            (["function", "function"], (), False),
            (["function", "signature", "function"], ((2, ""),), False),
            (["function", "function", "function"], (), False),
            (["function", "function"], (), True),
        ):
            with self.subTest(columns=columns, overrides=overrides, empty=empty):
                index = self._column_index(columns, overrides, header_only=empty)
                before = index.read_bytes()
                result = self._run_index(index)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn("duplicate CSV column", result.stdout)
                self.assertIn("function", result.stdout)
                self.assertIn("functions.csv", result.stdout)
                self.assertEqual(result.stderr, "")
                self.assertEqual(index.read_bytes(), before)

    def test_ignored_columns_and_minimal_reordered_indexes_remain_usable(self):
        for columns, overrides, bom in (
            (["signature", "function"], (), False),
            (["signature", "function"], (), True),
            (["description", "function", "description"], ((0, "ignored contradiction"),), False),
            (["extra", "function", "extra"], ((0, "ignored contradiction"),), False),
            (["Function", "extra", "function"], (), True),
        ):
            with self.subTest(columns=columns, bom=bom):
                result = self._run_index(self._column_index(columns, overrides, bom=bom))
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn("143 functions", result.stdout)

    def test_the_committed_cover_matches_the_index_and_the_changelog(self):
        result = self.run_gate()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("143 functions", result.stdout)

    def test_a_stale_function_count_fails(self):
        rewrite_label(self.workbook, "143 functions", "133 functions")
        result = self.run_gate()
        self.assertEqual(result.returncode, 1)
        self.assertIn("cover says 133 functions", result.stdout)

    def test_the_cell_is_resolved_not_the_first_matching_string(self):
        # A current-looking label stored elsewhere must not rescue a stale cell.
        current = "Version 19 October 2026    -    143 functions    -    Microsoft 365 or Excel 2024 and later"
        stale = current.replace("19 October 2026", "20 August 2026").replace("143", "133")
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
        self.assertIn("cover says 133 functions", result.stdout)

    def test_a_byte_order_marked_index_is_read(self):
        index = self.directory / "functions.csv"
        index.write_bytes("﻿".encode("utf-8") + (ROOT / "functions.csv").read_bytes())
        result = subprocess.run(
            [sys.executable, str(GATE), str(self.workbook), str(index), str(self.changelog)],
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_missing_function_names_fail_without_a_traceback_or_a_false_count(self):
        with (ROOT / "functions.csv").open(encoding="utf-8-sig", newline="") as handle:
            names = [row["function"] for row in csv.DictReader(handle)]
        names.remove(next(name for name in names if not name.startswith("oz.About")))
        index = self.directory / "functions.csv"
        for missing in (None, "", "   "):
            with self.subTest(missing=missing):
                with index.open("w", encoding="utf-8", newline="") as handle:
                    writer = csv.writer(handle)
                    writer.writerow(["signature", "function"])
                    writer.writerows(["LAMBDA()", name] for name in names)
                    writer.writerow(["LAMBDA()"] if missing is None else ["LAMBDA()", missing])
                result = subprocess.run(
                    [sys.executable, str(GATE), str(self.workbook), str(index), str(self.changelog)],
                    capture_output=True, text=True, check=False,
                )
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stderr, "")
                self.assertIn("FAIL: functions.csv contains an empty function name", result.stdout)

    def test_a_stale_date_fails(self):
        text = self.changelog.read_text(encoding="utf-8")
        self.changelog.write_text(
            text.replace("19 October 2026", "20 October 2026", 1), encoding="utf-8"
        )
        result = self.run_gate()
        self.assertEqual(result.returncode, 1)
        self.assertIn("current cut is 20 October 2026", result.stdout)


if __name__ == "__main__":
    unittest.main()
