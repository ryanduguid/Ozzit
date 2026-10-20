"""Refusal and edge paths of the rate and date helper pass."""

import ast
import contextlib
import csv
import io
import re
import shutil
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

WORKBOOK = ROOT / "ozzit.xlsx"


def run_main(module, *argv):
    """main() under the given arguments: (exit code or None, stdout)."""
    output = io.StringIO()
    code = None
    with mock.patch.object(sys, "argv", [module.__name__, *map(str, argv)]), \
            contextlib.redirect_stdout(output):
        try:
            module.main()
        except SystemExit as exc:
            code = exc.code
    return code, output.getvalue()


class ScratchDirectory(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-pass-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)


def guard_loop():
    """The import-time loop that vets FUNCTIONS, compiled from the module's own lines."""
    path = Path(helpers.__file__)
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    loop = next(node for node in tree.body
                if isinstance(node, ast.For) and getattr(node.iter, "id", None) == "FUNCTIONS")
    return compile(ast.Module(body=[loop], type_ignores=[]), str(path), "exec")


class RateDateHelperPathTests(ScratchDirectory):
    def test_the_import_guard_refuses_malformed_source_blocks(self):
        code = guard_loop()
        good = "/* header */\nFλ = LAMBDA(x, x)"
        exec(code, {"FUNCTIONS": [("Financial", "Fλ", "about", good)]})
        for block, message in (
            (good + " &amp; more", "Fλ: &amp; is already escaped in the source"),
            (good + "\t", "Fλ: tabs do not belong in the source"),
            ("/* header */\nGλ = LAMBDA(x, x)", "Fλ: the source block does not declare it"),
        ):
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, f"^{re.escape(message)}$"):
                exec(code, {"FUNCTIONS": [("Financial", "Fλ", "about", block)]})

    def test_a_module_holding_part_of_the_change_is_refused(self):
        functions = helpers.module_functions("Financial")
        text = f"\n{functions[0][0]} = LAMBDA("
        with self.assertRaisesRegex(
            ValueError, f"src/Financial.txt holds 1 of {len(functions)} functions and 0 About rows"
        ):
            helpers.src_state("Financial", text)

    def test_add_to_src_needs_exactly_one_anchor_and_ends_the_module_with_a_newline(self):
        anchor = helpers.ABOUT["Dates"][0]
        with self.assertRaisesRegex(ValueError, "^src/Dates.txt: About anchor found 0 times, expected 1$"):
            helpers.add_to_src("Dates", "no anchor here\n")
        text = f'Help, TEXTSPLIT(\n    {anchor} &\n    "→"'
        self.assertEqual(helpers.add_to_src("Dates", text), helpers.add_to_src("Dates", text + "\n"))

    def test_run_refuses_a_missing_source_module(self):
        src = self.directory / "src"
        src.mkdir()
        with self.assertRaisesRegex(ValueError, "^src: missing .*Financial.txt$"):
            helpers.run(self.directory / "ozzit.xlsx", src, None)

    def copy_stores(self):
        workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, workbook)
        src = self.directory / "src"
        shutil.copytree(ROOT / "src", src)
        index = self.directory / "functions.csv"
        shutil.copy2(ROOT / "functions.csv", index)
        return workbook, src, index

    def unapply(self, workbook, src):
        """Take the 4 helpers back out of the workbook and src/."""
        for module in helpers.MODULES:
            path = src / f"{module}.txt"
            text = path.read_text(encoding="utf-8")
            functions = helpers.module_functions(module)
            blocks = "\n\n\n" + "\n\n\n".join(source for _n, _a, source in functions) + "\n"
            self.assertTrue(text.endswith(blocks))
            text = text[: -len(blocks)]
            anchor, heading, _width = helpers.ABOUT[module]
            indent = text[text.rfind("\n", 0, text.index(anchor)) + 1 : text.index(anchor)]
            rows = [f'{indent}"→¶" &', f'{indent}"{heading}¶" &'] if heading else []
            rows.extend(f"{indent}{helpers.about_row(module, n, a)} &" for n, a, _s in functions)
            text = text.replace(anchor + " &\n" + "\n".join(rows), anchor + " &", 1)
            path.write_text(text, encoding="utf-8", newline="\n")
        parts = read_parts(workbook)
        book = read_book(parts)
        for name in helpers.QUALIFIED:
            book = re.sub(r'<definedName name="%s"[^>]*>.*?</definedName>' % re.escape(name), "", book,
                          count=1, flags=re.DOTALL)
        parts["xl/workbook.xml"] = book.encode("utf-8")
        write_deterministic(workbook, parts)
        self.assertEqual(helpers.workbook_state(book), "absent")

    def test_an_index_without_any_helper_row_disagrees_with_the_applied_stores(self):
        workbook, src, index = self.copy_stores()
        with index.open(encoding="utf-8-sig", newline="") as handle:
            rows = [row for row in csv.DictReader(handle) if row["function"] not in helpers.QUALIFIED]
        with index.open("w", encoding="utf-8", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=list(rows[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)
        with self.assertRaisesRegex(ValueError, (
            r"^the stores disagree \(Financial applied, Dates applied, workbook applied, "
            r"functions.csv absent\); the views must not be applied separately$"
        )):
            helpers.run(workbook, src, index)

    def test_a_missing_or_omitted_index_is_noted_and_left_unwritten(self):
        for label, index_name in (("missing", "absent.csv"), ("omitted", None)):
            with self.subTest(label=label):
                workbook, src, _index = self.copy_stores()
                self.unapply(workbook, src)
                index = None if index_name is None else self.directory / index_name
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    changed = helpers.run(workbook, src, index)
                self.assertEqual(changed, ["src/Financial.txt", "src/Dates.txt", "workbook"])
                expected = "" if index is None else f"note: {index} not found, index not updated\n"
                self.assertEqual(output.getvalue(), expected)
                for module in helpers.MODULES:
                    self.assertEqual((src / f"{module}.txt").read_bytes(),
                                     (ROOT / "src" / f"{module}.txt").read_bytes())
                self.assertEqual(helpers.workbook_state(read_book(read_parts(workbook))), "applied")
                if index is not None:
                    self.assertFalse(index.exists())
                shutil.rmtree(self.directory)
                self.directory.mkdir()

    def test_a_failure_without_an_index_restores_the_sources(self):
        workbook, src, _index = self.copy_stores()
        self.unapply(workbook, src)
        before = {path.name: path.read_bytes() for path in src.glob("*.txt")}
        index = self.directory / "absent.csv"
        with mock.patch.object(helpers, "compile_sources", side_effect=ValueError("synthetic failure")), \
                self.assertRaisesRegex(ValueError, "^synthetic failure$"):
            helpers.run(workbook, src, index)
        self.assertEqual({path.name: path.read_bytes() for path in src.glob("*.txt")}, before)
        self.assertFalse(index.exists())

    def test_main_refuses_extra_arguments_and_a_missing_workbook(self):
        missing = self.directory / "missing.xlsx"
        self.assertEqual(run_main(helpers, "a", "b", "c", "d"), (
            "FAIL: usage: python tools/postbuild/rate_date_helpers.py [workbook] [src dir] [functions.csv]", ""
        ))
        self.assertEqual(run_main(helpers, missing), (f"FAIL: no such workbook: {missing}", ""))


if __name__ == "__main__":
    unittest.main()
