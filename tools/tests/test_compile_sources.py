"""Contract for tools/compile_sources.py, the src/ to workbook compiler.

The compiler is trusted with every defined name in the workbook, so the first
contract is that it reproduces all of them from the tracked src/ without a
change. The rest pin the marker rules Excel needs, the refusal of anything it
cannot classify, and the two views it keeps in step: the Name Manager comment
and functions.csv.
"""

import csv
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import compile_sources  # noqa: E402

ROOT = TOOLS.parent
WORKBOOK = ROOT / "ozzit.xlsx"
SRC = ROOT / "src"
INDEX = ROOT / "functions.csv"

LIBRARY = {"Otherλ", "Thisλ"}


def render(body: str) -> str:
    return compile_sources.render(body, LIBRARY)


class RenderTests(unittest.TestCase):
    def test_parameters_bindings_and_functions_carry_their_markers(self):
        stored = render(
            "LAMBDA([Values], [Size],\n"
            "    LET(Help, TRIM(TEXTSPLIT(\"FUNCTION: →Thisλ(Values)¶\", \"→\", \"¶\")),\n"
            "        Rows, ROWS(Values),\n"
            "        Kept, FILTER(Values, Values > Size),\n"
            "        CHOOSE(Rows, Kept, Help)))"
        )
        self.assertTrue(stored.startswith("_xlfn.LAMBDA(_xlop.Values, _xlop.Size, _xlfn.LET("))
        self.assertIn("_xlpm.Help, TRIM(_xlfn.TEXTSPLIT(", stored)
        self.assertIn("_xlpm.Rows, ROWS(_xlpm.Values)", stored)
        self.assertIn("_xlfn._xlws.FILTER(_xlpm.Values, _xlpm.Values > _xlpm.Size)", stored)
        self.assertIn("CHOOSE(_xlpm.Rows, _xlpm.Kept, _xlpm.Help)", stored)
        # help text is copied verbatim, markers and all
        self.assertIn('"FUNCTION: →Thisλ(Values)¶"', stored)

    def test_library_calls_are_qualified_and_case_is_normalised(self):
        stored = render("LAMBDA([x], LET(Mpp, 12, Switch(x, 1, Otherλ(X / mpp), 0)))")
        self.assertIn("_xlfn.SWITCH(_xlpm.x, 1, oz.Otherλ(_xlpm.x / _xlpm.Mpp), 0)", stored)

    def test_implicit_intersection_is_stored_as_single(self):
        stored = render("LAMBDA([Items], LET(First, @INDEX(Items, 1), @First))")
        self.assertIn("_xlpm.First, _xlfn.SINGLE(INDEX(_xlpm.Items, 1))", stored)
        self.assertIn("_xlfn.SINGLE(_xlpm.First)", stored)

    def test_error_literals_and_booleans_are_not_names(self):
        stored = render("LAMBDA([x], IF(x = true, #Value!, #N/A))")
        self.assertIn("IF(_xlpm.x = TRUE, #VALUE!, #N/A)", stored)

    def test_an_unknown_identifier_is_refused(self):
        with self.assertRaisesRegex(ValueError, "Undeclared reached the stored form"):
            render("LAMBDA([x], x + Undeclared)")

    def test_inner_lambda_parameters_and_nested_let_names_are_bindings(self):
        stored = render(
            "LAMBDA([Values], MAP(Values, LAMBDA(v, LET(Twice, v * 2, Twice))))"
        )
        self.assertIn("_xlfn.MAP(_xlpm.Values, _xlfn.LAMBDA(_xlpm.v, _xlfn.LET(_xlpm.Twice, _xlpm.v * 2, _xlpm.Twice)))", stored)

    def test_a_binding_named_like_a_function_is_a_function_when_called(self):
        stored = render("LAMBDA([Values], LET(Rows, ROWS(Values), Rows + 1))")
        self.assertIn("_xlpm.Rows, ROWS(_xlpm.Values), _xlpm.Rows + 1", stored)

    def test_header_comment_encodes_line_breaks_and_keeps_excels_limit(self):
        text = (
            "/*  FUNCTION NAME:  Thisλ\n"
            "    DESCRIPTION:*//**First line \n    second line*/\n"
            "Thisλ = LAMBDA([x], x);\n"
            "/*  FUNCTION NAME:  Longλ\n"
            "    DESCRIPTION:*//**" + "x" * 300 + "*/\n"
            "Longλ = LAMBDA([x], x);\n"
        )
        comments = compile_sources.header_comments(text)
        self.assertEqual(comments["Thisλ"], "First line _x000a_    second line")
        self.assertEqual(len(comments["Longλ"]), compile_sources.COMMENT_LIMIT)


class WorkbookTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-compile-"))
        self.workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, self.workbook)
        self.src = self.directory / "src"
        shutil.copytree(SRC, self.src)
        self.index = self.directory / "functions.csv"
        shutil.copy2(INDEX, self.index)

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _book(self):
        with zipfile.ZipFile(self.workbook) as archive:
            return archive.read("xl/workbook.xml").decode("utf-8")

    def test_tracked_sources_reproduce_every_shipped_definition(self):
        self.assertEqual(compile_sources.run(self.workbook, self.src, True, None), [])

    def test_only_a_changed_definition_is_rewritten(self):
        before = self._book()
        module = self.src / "Essentials.txt"
        text = module.read_text(encoding="utf-8")
        old = "        Result,         ISNUMBER( XMATCH( Value, TOCOL( List), 0)),"
        self.assertEqual(text.count(old), 1)
        module.write_text(
            text.replace(old, "        Result,         ISNUMBER( XMATCH( Value, TOCOL( List), 0)) + 0,"),
            encoding="utf-8",
        )
        changed = compile_sources.run(self.workbook, self.src, False, None)
        self.assertEqual(changed, ["oz.IsInListλ"])
        after = self._book()
        self.assertIn("ISNUMBER(_xlfn.XMATCH(_xlpm.Value,_xlfn.TOCOL(_xlpm.List),0))+0", compile_sources.tight(after))
        # every other definition is untouched, byte for byte
        strip = lambda book: [
            line for line in book.split("</definedName>") if 'name="oz.IsInListλ"' not in line
        ]
        self.assertEqual(strip(before), strip(after))
        # and a second run has nothing to do
        self.assertEqual(compile_sources.run(self.workbook, self.src, False, None), [])

    def test_a_changed_header_rewrites_the_name_manager_comment(self):
        module = self.src / "Ratios.txt"
        text = module.read_text(encoding="utf-8")
        old = "/**Measures the extent of a company’s leverage */"
        self.assertEqual(text.count(old), 1)
        module.write_text(text.replace(old, "/**Leverage, the short way*/"), encoding="utf-8")
        changed = compile_sources.run(self.workbook, self.src, False, self.index)
        self.assertEqual(changed, ["oz.DebtRatioλ", "functions.csv"])
        self.assertIn('<definedName name="oz.DebtRatioλ" comment="Leverage, the short way">', self._book())
        with self.index.open(encoding="utf-8-sig", newline="") as handle:
            rows = {row["function"]: row for row in csv.DictReader(handle)}
        self.assertEqual(rows["oz.DebtRatioλ"]["description"], "Leverage, the short way")
        self.assertEqual(rows["oz.DebtRatioλ"]["previous_name"], "nabla.r.DebtRatioλ")

    def test_a_source_name_that_does_not_ship_is_refused(self):
        module = self.src / "Debt.txt"
        module.write_text(
            module.read_text(encoding="utf-8")
            + "\n/*  FUNCTION NAME:  Extraλ\n    DESCRIPTION:*//**Extra*/\nExtraλ = LAMBDA([x], x);\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "oz.Extraλ is declared in src/ but does not ship"):
            compile_sources.run(self.workbook, self.src, True, None)

    def test_regenerated_index_matches_the_tracked_one(self):
        book = self._book()
        modules = {c.name: c.module for c in compile_sources.compile_sources(self.src)}
        self.assertFalse(compile_sources.update_index(self.index, book, modules))
        self.assertEqual(self.index.read_bytes(), INDEX.read_bytes())

    def test_cli_check_mode_reports_and_writes_nothing(self):
        before = self.workbook.read_bytes()
        result = subprocess.run(
            [sys.executable, str(TOOLS / "compile_sources.py"), str(self.workbook), str(self.src), "--check"],
            capture_output=True, text=True, check=False, encoding="utf-8",
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("every defined name already matches src/", result.stdout)
        self.assertEqual(self.workbook.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
