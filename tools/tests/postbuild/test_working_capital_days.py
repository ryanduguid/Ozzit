"""Recovery and applied-state contract for the working-capital days pass.

The pass writes 3 kinds of store: src/Ratios.txt, xl/workbook.xml and functions.csv.
Two things it promises about that are checked here, as test_rate_date_helpers checks
them for the earlier pass: an interrupted run leaves none of them half-written, and a
store missing part of the change is not reported as already applied.

The "absent" input is the committed one with the 4 functions taken back out of every
store, which is what the pass was written to run against.
"""

import csv
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import working_capital_days as helpers  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402
from workbook import read_book, read_parts  # noqa: E402

SCRIPT = TOOLS / "postbuild" / "working_capital_days.py"


class WorkingCapitalDaysContractTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-working-capital-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _applied_copy(self):
        """The committed stores, copied where they can be edited."""
        workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(ROOT / "ozzit.xlsx", workbook)
        src = self.directory / "src"
        shutil.copytree(ROOT / "src", src)
        index = self.directory / "functions.csv"
        shutil.copy2(ROOT / "functions.csv", index)
        return workbook, src, index

    @staticmethod
    def _unapply_src(text):
        """Ratios.txt with what add_to_src appends and inserts taken back out."""
        blocks = "\n\n\n" + "\n\n\n".join(source for _n, _a, source in helpers.FUNCTIONS)
        if text.endswith(blocks):
            text = text[: -len(blocks)]
        anchor = helpers.ABOUT_ANCHOR
        line_start = text.rfind("\n", 0, text.index(anchor)) + 1
        indent = text[line_start : text.index(anchor)]
        rows = [f"{indent}{helpers.about_row(name, about)} &" for name, about, _s in helpers.FUNCTIONS]
        return text.replace(anchor + " &\n" + "\n".join(rows), anchor + " &", 1)

    def _absent_copy(self):
        """The same stores with the 4 functions removed from each of them."""
        workbook, src, index = self._applied_copy()

        path = src / f"{helpers.MODULE}.txt"
        applied = path.read_text(encoding="utf-8")
        absent = self._unapply_src(applied)
        self.assertEqual(helpers.add_to_src(absent), applied, "Ratios.txt does not round-trip through the pass")
        path.write_text(absent, encoding="utf-8", newline="\n")

        parts = read_parts(workbook)
        book = read_book(parts)
        for name in helpers.QUALIFIED:
            book, count = re.subn(
                r'<definedName name="%s"[^>]*>.*?</definedName>' % re.escape(name),
                "",
                book,
                count=1,
                flags=re.DOTALL,
            )
            self.assertEqual(count, 1, f"{name} is not a defined name in the workbook")
        parts["xl/workbook.xml"] = book.encode("utf-8")
        write_deterministic(workbook, parts)

        self._write_index(index, [r for r in self._rows(index) if r["function"] not in helpers.QUALIFIED])
        return workbook, src, index

    @staticmethod
    def _rows(index):
        with index.open(encoding="utf-8-sig", newline="") as handle:
            return list(csv.DictReader(handle))

    @staticmethod
    def _write_index(index, rows):
        with index.open("w", encoding="utf-8", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=list(rows[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)

    @staticmethod
    def _snapshot(workbook, src, index):
        return workbook.read_bytes(), (src / f"{helpers.MODULE}.txt").read_bytes(), index.read_bytes()

    def _run_cli(self, workbook, src, index):
        return subprocess.run(
            [sys.executable, str(SCRIPT), str(workbook), str(src), str(index)],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_the_absent_input_is_one_the_pass_applies(self):
        workbook, src, index = self._absent_copy()
        result = self._run_cli(workbook, src, index)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("added the working-capital days functions", result.stdout)
        self.assertEqual(len(self._rows(index)), len(self._rows(ROOT / "functions.csv")))
        self.assertEqual(
            (src / f"{helpers.MODULE}.txt").read_bytes(), (ROOT / "src" / f"{helpers.MODULE}.txt").read_bytes()
        )

    def test_a_failed_source_write_restores_every_store(self):
        workbook, src, index = self._absent_copy()
        before = self._snapshot(workbook, src, index)

        real_write = helpers.write_text
        calls = []

        def failing_write(path, text):
            calls.append(Path(path).name)
            if len(calls) == 1:
                raise PermissionError("synthetic failure on the source write")
            return real_write(path, text)

        with mock.patch.object(helpers, "write_text", side_effect=failing_write):
            with self.assertRaises(PermissionError):
                helpers.run(workbook, src, index)

        # The failed write, then the restore of the same module.
        self.assertEqual(calls, ["Ratios.txt", "Ratios.txt"])
        self.assertEqual(self._snapshot(workbook, src, index), before)

    def test_a_failed_compile_restores_every_store(self):
        workbook, src, index = self._absent_copy()
        before = self._snapshot(workbook, src, index)

        with mock.patch.object(helpers, "compile_sources", side_effect=ValueError("synthetic compilation failure")):
            with self.assertRaises(ValueError):
                helpers.run(workbook, src, index)

        self.assertEqual(self._snapshot(workbook, src, index), before)

    def test_a_failed_workbook_write_restores_every_store(self):
        # The last write: src and the index are already on disk when it fails.
        workbook, src, index = self._absent_copy()
        before = self._snapshot(workbook, src, index)

        with mock.patch.object(helpers, "write_deterministic", side_effect=OSError("synthetic workbook failure")):
            with self.assertRaises(OSError):
                helpers.run(workbook, src, index)

        self.assertEqual(self._snapshot(workbook, src, index), before)

    def test_an_index_missing_one_function_is_not_reported_as_applied(self):
        workbook, src, index = self._applied_copy()
        self._write_index(index, [row for row in self._rows(index) if row["function"] != "oz.WIPDaysλ"])
        result = self._run_cli(workbook, src, index)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("holds 3 of 4 working-capital rows", result.stdout + result.stderr)

    def test_a_named_index_that_does_not_exist_is_refused(self):
        # It used to count as no index at all: src/ and the workbook were written, the
        # run exited 0, and the index was left behind.
        workbook, src, index = self._absent_copy()
        index.unlink()
        workbook_before = workbook.read_bytes()
        source_before = (src / f"{helpers.MODULE}.txt").read_bytes()
        result = self._run_cli(workbook, src, index)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("no such index", result.stdout + result.stderr)
        self.assertEqual(workbook.read_bytes(), workbook_before)
        self.assertEqual((src / f"{helpers.MODULE}.txt").read_bytes(), source_before)

    def test_no_index_cannot_apply_the_change(self):
        # Without an index argument and with no functions.csv beside the workbook, the
        # pass may confirm the applied state but must not write src/ and the workbook alone.
        workbook, src, index = self._absent_copy()
        index.unlink()
        workbook_before = workbook.read_bytes()
        source_before = (src / f"{helpers.MODULE}.txt").read_bytes()
        result = subprocess.run(
            [sys.executable, str(SCRIPT), str(workbook), str(src)], capture_output=True, text=True, check=False
        )
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("functions.csv is required", result.stdout + result.stderr)
        self.assertEqual(workbook.read_bytes(), workbook_before)
        self.assertEqual((src / f"{helpers.MODULE}.txt").read_bytes(), source_before)

    def test_a_module_holding_part_of_the_change_is_refused(self):
        workbook, src, index = self._applied_copy()
        path = src / f"{helpers.MODULE}.txt"
        path.write_text(
            path.read_text(encoding="utf-8").replace(helpers.WIP_DAYS_SRC, ""), encoding="utf-8", newline="\n"
        )
        result = self._run_cli(workbook, src, index)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("holds 3 of 4 functions", result.stdout + result.stderr)

    def test_the_committed_stores_all_read_as_applied(self):
        workbook, src, index = self._applied_copy()
        result = self._run_cli(workbook, src, index)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("already applied", result.stdout)


if __name__ == "__main__":
    unittest.main()
