"""Recovery and applied-state contract for the rate and date helpers pass.

The pass writes 3 kinds of store: the src modules, xl/workbook.xml and
functions.csv. Two things it promises about that are checked here: an interrupted
run leaves none of them half-written, and a store missing part of the change is
not reported as already applied.

The "absent" input is the committed one with the 4 helpers taken back out of every
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

import rate_date_helpers as helpers  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402
from workbook import read_book, read_parts  # noqa: E402

SCRIPT = TOOLS / "postbuild" / "rate_date_helpers.py"


class RateDateHelperContractTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-rate-date-"))

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
    def _unapply_src(module, text):
        """One module with what add_to_src appends and inserts taken back out."""
        functions = helpers.module_functions(module)
        blocks = "\n\n\n" + "\n\n\n".join(source for _n, _a, source in functions) + "\n"
        if text.endswith(blocks):
            text = text[: -len(blocks)]
        anchor, heading, _width = helpers.ABOUT[module]
        line_start = text.rfind("\n", 0, text.index(anchor)) + 1
        indent = text[line_start : text.index(anchor)]
        rows = [f'{indent}"→¶" &', f'{indent}"{heading}¶" &'] if heading else []
        rows.extend(
            f"{indent}{helpers.about_row(module, name, about)} &"
            for name, about, _source in functions
        )
        return text.replace(anchor + " &\n" + "\n".join(rows), anchor + " &", 1)

    def _absent_copy(self):
        """The same stores with the 4 helpers removed from each of them."""
        workbook, src, index = self._applied_copy()

        for module in helpers.MODULES:
            path = src / f"{module}.txt"
            applied = path.read_text(encoding="utf-8")
            absent = self._unapply_src(module, applied)
            self.assertEqual(
                helpers.add_to_src(module, absent),
                applied,
                f"src/{module}.txt does not round-trip through the pass",
            )
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
        self.assertIn("added the rate and date helpers", result.stdout)
        self.assertEqual(len(self._rows(index)), len(self._rows(ROOT / "functions.csv")))

    def test_a_failed_source_write_leaves_no_module_applied(self):
        # The source writes used to sit outside the recovery block, so a failure on the
        # second one left the first module written: the pass then read Financial as
        # applied and Dates as absent and refused to run at all.
        workbook, src, index = self._absent_copy()
        before = {path.name: path.read_bytes() for path in sorted(src.glob("*.txt"))}
        workbook_before = workbook.read_bytes()
        index_before = index.read_bytes()

        real_write = helpers.write_text
        calls = []

        def failing_write(path, text):
            calls.append(Path(path).name)
            if len(calls) == 2:
                raise PermissionError("synthetic failure on the second source write")
            return real_write(path, text)

        with mock.patch.object(helpers, "write_text", side_effect=failing_write):
            with self.assertRaises(PermissionError):
                helpers.run(workbook, src, index)

        # 2 writes attempted, then the same 2 modules written back: the restore loop
        # covers the module that was already on disk when the second write failed.
        written = [f"{module}.txt" for module in helpers.MODULES]
        self.assertEqual(calls, written * 2)
        self.assertEqual({path.name: path.read_bytes() for path in sorted(src.glob("*.txt"))}, before)
        self.assertEqual(workbook.read_bytes(), workbook_before)
        self.assertEqual(index.read_bytes(), index_before)

    def test_a_later_failure_still_restores_every_store(self):
        # The control: recovery already worked once execution reached the protected block.
        workbook, src, index = self._absent_copy()
        before = {path.name: path.read_bytes() for path in sorted(src.glob("*.txt"))}
        index_before = index.read_bytes()

        with mock.patch.object(
            helpers, "compile_sources", side_effect=ValueError("synthetic compilation failure")
        ):
            with self.assertRaises(ValueError):
                helpers.run(workbook, src, index)

        self.assertEqual({path.name: path.read_bytes() for path in sorted(src.glob("*.txt"))}, before)
        self.assertEqual(index.read_bytes(), index_before)

    def test_an_index_missing_one_helper_is_not_reported_as_applied(self):
        # Every other store holds all 4, so the pass used to read the change as applied
        # and exit 0, leaving the missing row for the separate index gate to find.
        workbook, src, index = self._applied_copy()
        self._write_index(
            index, [row for row in self._rows(index) if row["function"] != "oz.PeriodRateλ"]
        )
        result = self._run_cli(workbook, src, index)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("holds 3 of 4 helper rows", result.stdout + result.stderr)

    def test_the_committed_stores_all_read_as_applied(self):
        workbook, src, index = self._applied_copy()
        result = self._run_cli(workbook, src, index)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("already applied", result.stdout)


if __name__ == "__main__":
    unittest.main()
