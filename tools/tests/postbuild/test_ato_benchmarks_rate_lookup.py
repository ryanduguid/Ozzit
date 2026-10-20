"""Recovery and applied-state contract for the ATO benchmark and rate pass.

The pass writes 4 kinds of store: src/Ratios.txt, src/Dates.txt, xl/workbook.xml and
functions.csv. As the native working-capital tests check for the earlier pass, an interrupted
run leaves none of them half-written and a store missing part of the change is not
reported as already applied. The "absent" input is the committed one with the 6
functions taken back out of every store, which is what the pass was written to run
against.

test_benchmark_arithmetic.py keeps the independent arithmetic mirrors and formula
pins separately, so the retained Excel cross-checks do not import this Python pass.
"""

import ast
import csv
import io
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import ato_benchmarks_rate_lookup as helpers  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402
from workbook import read_book, read_parts  # noqa: E402

SCRIPT = TOOLS / "postbuild" / "ato_benchmarks_rate_lookup.py"


def call_main(*args):
    output = io.StringIO()
    with mock.patch.object(sys, "argv", ["pass.py", *args]), redirect_stdout(output):
        try:
            helpers.main()
        except SystemExit as exc:
            return exc.code, output.getvalue()
    return None, output.getvalue()


def run_source_guard(functions):
    """Run the module's import-time guard over these (module, name, about, block) rows."""
    path = Path(helpers.__file__)
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    guard = [
        node
        for node in tree.body
        if isinstance(node, ast.For) and ast.unparse(node.iter) == "FUNCTIONS"
    ]
    exec(compile(ast.Module(body=guard, type_ignores=[]), str(path), "exec"), {"FUNCTIONS": functions})


class AtoBenchmarksRateLookupContractTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-ato-benchmarks-"))

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
        """The module with what add_to_src appends and inserts taken back out."""
        functions = helpers.module_functions(module)
        anchor, heading, _width, follower = helpers.ABOUT[module]
        blocks = "\n\n\n".join(source for _n, _a, source in functions)
        text = text.replace(blocks + helpers.gap_before(text, follower), "", 1)
        line_start = text.rfind("\n", 0, text.index(anchor)) + 1
        indent = text[line_start : text.index(anchor)]
        rows = [f'{indent}"→¶" &', f'{indent}"{heading}¶" &'] if heading else []
        rows.extend(f"{indent}{helpers.about_row(module, name, about)} &" for name, about, _s in functions)
        return text.replace(anchor + " &\n" + "\n".join(rows), anchor + " &", 1)

    def _absent_copy(self):
        """The same stores with the 6 functions removed from each of them."""
        workbook, src, index = self._applied_copy()
        for module in helpers.MODULES:
            path = src / f"{module}.txt"
            applied = path.read_text(encoding="utf-8")
            absent = self._unapply_src(module, applied)
            self.assertEqual(helpers.add_to_src(module, absent), applied, f"{module} does not round-trip")
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
        modules = tuple((src / f"{m}.txt").read_bytes() for m in helpers.MODULES)
        return workbook.read_bytes(), modules, index.read_bytes()

    def _run_cli(self, *args):
        return subprocess.run(
            [sys.executable, str(SCRIPT), *map(str, args)], capture_output=True, text=True, check=False
        )

    def test_the_absent_input_is_one_the_pass_applies(self):
        workbook, src, index = self._absent_copy()
        result = self._run_cli(workbook, src, index)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("added the benchmark and rate functions", result.stdout)
        self.assertEqual(index.read_bytes(), (ROOT / "functions.csv").read_bytes())
        for module in helpers.MODULES:
            with self.subTest(module=module):
                self.assertEqual(
                    (src / f"{module}.txt").read_bytes(), (ROOT / "src" / f"{module}.txt").read_bytes()
                )

    def test_a_failed_source_write_restores_every_store(self):
        workbook, src, index = self._absent_copy()
        before = self._snapshot(workbook, src, index)
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

        # The first module written, the failed second, then both restored.
        self.assertEqual(calls, ["Ratios.txt", "Dates.txt", "Ratios.txt", "Dates.txt"])
        self.assertEqual(self._snapshot(workbook, src, index), before)

    def test_a_failed_workbook_write_restores_every_store(self):
        # The last write: both modules and the index are already on disk when it fails.
        workbook, src, index = self._absent_copy()
        before = self._snapshot(workbook, src, index)
        with mock.patch.object(helpers, "write_deterministic", side_effect=OSError("synthetic")):
            with self.assertRaises(OSError):
                helpers.run(workbook, src, index)
        self.assertEqual(self._snapshot(workbook, src, index), before)

    def test_an_index_missing_one_function_is_not_reported_as_applied(self):
        workbook, src, index = self._applied_copy()
        self._write_index(index, [r for r in self._rows(index) if r["function"] != "oz.RateAtDateλ"])
        result = self._run_cli(workbook, src, index)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("holds 5 of 6 benchmark and rate rows", result.stdout + result.stderr)

    def test_a_named_index_that_does_not_exist_is_refused(self):
        workbook, src, index = self._absent_copy()
        index.unlink()
        before = workbook.read_bytes()
        result = self._run_cli(workbook, src, index)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("no such index", result.stdout + result.stderr)
        self.assertEqual(workbook.read_bytes(), before)

    def test_no_index_cannot_apply_the_change(self):
        workbook, src, index = self._absent_copy()
        index.unlink()
        before = self._snapshot(workbook, src, workbook)
        result = self._run_cli(workbook, src)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("functions.csv is required", result.stdout + result.stderr)
        self.assertEqual(self._snapshot(workbook, src, workbook), before)

    def test_a_module_holding_part_of_the_change_is_refused(self):
        workbook, src, index = self._applied_copy()
        path = src / "Ratios.txt"
        path.write_text(
            path.read_text(encoding="utf-8").replace(helpers.FUNCTIONS[4][3], ""),
            encoding="utf-8",
            newline="\n",
        )
        result = self._run_cli(workbook, src, index)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("holds 4 of 5 functions", result.stdout + result.stderr)

    def test_stores_in_different_states_are_refused(self):
        workbook, src, index = self._absent_copy()
        shutil.copy2(ROOT / "src" / "Dates.txt", src / "Dates.txt")
        result = self._run_cli(workbook, src, index)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("the stores disagree (Ratios absent, Dates applied", result.stdout + result.stderr)

    def test_the_committed_stores_all_read_as_applied(self):
        workbook, src, index = self._applied_copy()
        code, output = call_main(str(workbook), str(src), str(index))
        self.assertIsNone(code, output)
        self.assertIn("already applied", output)
        # Without an index argument the default beside the workbook is read.
        code, output = call_main(str(workbook), str(src))
        self.assertIsNone(code, output)

    def test_a_bare_workbook_and_src_confirm_the_applied_state(self):
        # The idempotency test's copy has no functions.csv beside the workbook.
        workbook, src, index = self._applied_copy()
        index.unlink()
        self.assertEqual(helpers.run(workbook, src, None), [])

    def test_run_refuses_a_missing_module(self):
        workbook, src, _index = self._applied_copy()
        (src / "Dates.txt").unlink()
        with self.assertRaises(ValueError) as caught:
            helpers.run(workbook, src, None)
        self.assertEqual(str(caught.exception), f"src: missing {src / 'Dates.txt'}")

    def test_add_to_src_needs_exactly_one_about_anchor(self):
        anchor = helpers.ABOUT["Dates"][0]
        for text, hits in (("", 0), (anchor * 2, 2)):
            with self.subTest(hits=hits):
                with self.assertRaises(ValueError) as caught:
                    helpers.add_to_src("Dates", text)
                self.assertEqual(
                    str(caught.exception), f"src/Dates.txt: About anchor found {hits} times, expected 1"
                )

    def test_add_to_src_needs_exactly_one_following_block(self):
        with self.assertRaises(ValueError) as caught:
            helpers.add_to_src("Dates", helpers.ABOUT["Dates"][0] + " &")
        self.assertEqual(str(caught.exception), "src/Dates.txt: DateDifλ block found 0 times, expected 1")

    def test_the_source_guard_accepts_the_shipped_blocks(self):
        run_source_guard(helpers.FUNCTIONS)

    def test_the_source_guard_refuses_each_malformed_block(self):
        cases = [
            ("\nXλ = LAMBDA(a, a &amp; b)", "Xλ: &amp; does not belong in the source"),
            ("\nXλ = LAMBDA(a, __ATO_PAGE__)", "Xλ: __ATO_PAGE__ does not belong in the source"),
            ("\nXλ = LAMBDA(a,\tb)", "Xλ: tabs do not belong in the source"),
            ("\nYλ = LAMBDA(a, a)", "Xλ: the source block does not declare it"),
        ]
        for block, message in cases:
            with self.subTest(message=message):
                with self.assertRaises(ValueError) as caught:
                    run_source_guard([("Ratios", "Xλ", "about", block)])
                self.assertEqual(str(caught.exception), message)

    def test_main_rejects_extra_arguments(self):
        code, _ = call_main("a.xlsx", "src", "functions.csv", "extra")
        self.assertEqual(
            code,
            "FAIL: usage: python tools/postbuild/ato_benchmarks_rate_lookup.py "
            "[workbook] [src dir] [functions.csv]",
        )

    def test_main_rejects_a_missing_workbook(self):
        missing = self.directory / "absent.xlsx"
        code, _ = call_main(str(missing))
        self.assertEqual(code, f"FAIL: no such workbook: {missing}")


if __name__ == "__main__":
    unittest.main()
