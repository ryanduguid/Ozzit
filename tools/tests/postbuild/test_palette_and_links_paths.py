"""Refusal and apply paths of the remaining postbuild pass, on small synthetic workbooks.

The contract tests run each pass against the committed workbook, where every pass
is already applied. These build just the parts a pass reads, so the usage and
missing-input refusals, the anchor-count failures and the apply branch each run
once and their messages are held exactly.
"""

import ast
import io
import shutil
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

import working_capital_days as days  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402


def call_main(module, *args):
    """Run module.main() with these arguments; return (exit code, stdout)."""
    output = io.StringIO()
    code = None
    with mock.patch.object(sys, "argv", ["pass.py", *args]), redirect_stdout(output):
        try:
            module.main()
        except SystemExit as exc:
            code = exc.code
    return code, output.getvalue()


class ScratchTestCase(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-pass-paths-"))
        self.workbook = self.directory / "ozzit.xlsx"
        self.missing = self.directory / "absent.xlsx"

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def write_parts(self, parts):
        write_deterministic(self.workbook, {n: t.encode("utf-8") for n, t in parts.items()})


def run_source_guard(functions):
    """Run working_capital_days' import-time guard over these (name, about, block) rows.

    The guard is a module-level loop over the module's own FUNCTIONS, so it can only
    fail at import. This compiles that loop from the module's own file, under its own
    file name and line numbers, and runs it over the rows given instead.
    """
    path = Path(days.__file__)
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    guard = [
        node
        for node in tree.body
        if isinstance(node, ast.For) and ast.unparse(node.iter) == "FUNCTIONS"
    ]
    exec(compile(ast.Module(body=guard, type_ignores=[]), str(path), "exec"), {"FUNCTIONS": functions})


class WorkingCapitalDaysTests(ScratchTestCase):
    def test_the_source_guard_accepts_the_shipped_blocks(self):
        run_source_guard(days.FUNCTIONS)

    def test_the_source_guard_refuses_each_malformed_block(self):
        cases = [
            ("\nXλ = LAMBDA(a, a &amp; b)", "Xλ: &amp; is already escaped in the source"),
            ("\nXλ = LAMBDA(a,\tb)", "Xλ: tabs do not belong in the source"),
            ("\nYλ = LAMBDA(a, a)", "Xλ: the source block does not declare it"),
        ]
        for block, message in cases:
            with self.subTest(message=message):
                with self.assertRaises(ValueError) as caught:
                    run_source_guard([("Xλ", "about", block)])
                self.assertEqual(str(caught.exception), message)

    def test_add_to_src_needs_exactly_one_about_anchor(self):
        for text, hits in (("", 0), (days.ABOUT_ANCHOR * 2, 2)):
            with self.subTest(hits=hits):
                with self.assertRaises(ValueError) as caught:
                    days.add_to_src(text)
                self.assertEqual(
                    str(caught.exception),
                    f"src/Ratios.txt: About anchor found {hits} times, expected 1",
                )

    def test_run_refuses_a_missing_ratios_module(self):
        with self.assertRaises(ValueError) as caught:
            days.run(self.workbook, self.directory, None)
        self.assertEqual(str(caught.exception), f"src: missing {self.directory / 'Ratios.txt'}")

    def test_run_refuses_stores_in_different_states(self):
        shutil.copy2(ROOT / "src" / "Ratios.txt", self.directory / "Ratios.txt")
        self.write_parts({"xl/workbook.xml": "<workbook/>"})
        with self.assertRaises(ValueError) as caught:
            days.run(self.workbook, self.directory, None)
        self.assertEqual(
            str(caught.exception),
            "the stores disagree (Ratios applied, workbook absent); "
            "the views must not be applied separately",
        )

    def test_main_rejects_extra_arguments(self):
        code, _ = call_main(days, "a.xlsx", "src", "functions.csv", "extra")
        self.assertEqual(
            code,
            "FAIL: usage: python tools/postbuild/working_capital_days.py "
            "[workbook] [src dir] [functions.csv]",
        )

    def test_main_rejects_a_missing_workbook(self):
        code, _ = call_main(days, str(self.missing), str(self.directory))
        self.assertEqual(code, f"FAIL: no such workbook: {self.missing}")


if __name__ == "__main__":
    unittest.main()
