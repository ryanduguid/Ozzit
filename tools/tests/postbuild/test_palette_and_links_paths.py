"""Refusal and apply paths of 5 postbuild passes, on small synthetic workbooks.

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

import help_corrections  # noqa: E402
import help_links  # noqa: E402
import sheet_names  # noqa: E402
import workbook_palette as palette  # noqa: E402
import working_capital_days as days  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402
from workbook import read_parts  # noqa: E402


def call_main(module, *args):
    """Run module.main() with these arguments; return (exit code, stdout)."""
    output = io.StringIO()
    with mock.patch.object(sys, "argv", ["pass.py", *args]), redirect_stdout(output):
        try:
            module.main()
        except SystemExit as exc:
            return exc.code, output.getvalue()
    return None, output.getvalue()


class ScratchTestCase(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-pass-paths-"))
        self.workbook = self.directory / "ozzit.xlsx"
        self.missing = self.directory / "absent.xlsx"

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def write_parts(self, parts):
        write_deterministic(self.workbook, {n: t.encode("utf-8") for n, t in parts.items()})

    def text_parts(self):
        return {n: data.decode("utf-8") for n, data in read_parts(self.workbook).items()}


DXFS = (
    '<dxfs><dxf><font><color rgb="FFC00000"/></font>'
    '<fill><patternFill><bgColor rgb="FFFFCCCC"/></patternFill></fill></dxf></dxfs>'
)


def palette_parts(dxfs=DXFS):
    return {
        "xl/theme/theme1.xml": '<a:theme><a:srgbClr val="3F656F"/></a:theme>',
        "xl/styles.xml": (
            '<styleSheet><fonts><font><name val="Arial"/><color rgb="FF006600"/></font></fonts>'
            '<fills><fill><patternFill><fgColor rgb="FFCCFFCC"/></patternFill></fill></fills>'
            f"{dxfs}</styleSheet>"
        ),
        "xl/sharedStrings.xml": '<sst><si><r><rPr><rFont val="Arial"/></rPr><t>x</t></r></si></sst>',
        "xl/worksheets/sheet1.xml": "<worksheet/>",
    }


class WorkbookPaletteTests(ScratchTestCase):
    def test_hue_map_moves_teal_and_salmon_into_the_palette_bands(self):
        self.assertEqual(palette.hue_map("40A0A0"), "6F5A86")
        self.assertEqual(palette.hue_map("C08060"), "9B8F85")

    def test_recolour_remaps_a_six_digit_theme_value(self):
        self.assertEqual(
            palette.recolour('<a:srgbClr val="3F656F"/>'), ('<a:srgbClr val="5C2D91"/>', 1)
        )

    def test_recolour_refuses_a_colour_still_remappable_after_one_pass(self):
        with self.assertRaisesRegex(
            ValueError, "remap is not a fixed point: A28F7E is still remappable after one pass"
        ):
            palette.recolour('<color rgb="FFE07040"/>')

    def test_run_folds_fonts_and_fills_then_finds_nothing_to_do(self):
        self.write_parts(palette_parts())
        self.assertEqual(
            palette.run(self.workbook),
            [
                "xl/theme/theme1.xml: 1 colours remapped",
                'font family <name val="Arial"/> -> <name val="Aptos"/>',
                'rich-text font <rFont val="Arial"/> -> <rFont val="Aptos"/>',
                "help-label fonts folded to palette (1 entries)",
                "help-block fills folded to neutrals (1 entries)",
            ],
        )
        parts = self.text_parts()
        self.assertIn('val="5C2D91"', parts["xl/theme/theme1.xml"])
        styles = parts["xl/styles.xml"]
        self.assertIn('<name val="Aptos"/><color rgb="FF5C2D91"/>', styles)
        self.assertIn('<fgColor rgb="FFF3F1F6"/>', styles)
        self.assertIn('<rFont val="Aptos"/>', parts["xl/sharedStrings.xml"])
        self.assertEqual(palette.run(self.workbook), [])

    def test_main_rejects_extra_arguments(self):
        code, _ = call_main(palette, "a.xlsx", "b.xlsx")
        self.assertEqual(code, "FAIL: usage: python tools/postbuild/workbook_palette.py [workbook]")

    def test_main_rejects_a_missing_workbook(self):
        code, _ = call_main(palette, str(self.missing))
        self.assertEqual(code, f"FAIL: no such workbook: {self.missing}")

    def test_main_reports_a_remapped_toc_warning_without_writing(self):
        self.write_parts(palette_parts(dxfs="<dxfs></dxfs>"))
        before = self.workbook.read_bytes()
        code, _ = call_main(palette, str(self.workbook))
        self.assertEqual(code, "FAIL: TOC warning red was remapped")
        self.assertEqual(self.workbook.read_bytes(), before)

    def test_main_reports_a_file_that_is_not_a_workbook(self):
        self.workbook.write_bytes(b"not a zip archive")
        code, _ = call_main(palette, str(self.workbook))
        self.assertEqual(code, "FAIL: File is not a zip file")


def sheet_name_parts(name="oz.SumContains", toc_links=4):
    return {
        "xl/workbook.xml": (
            '<workbook><sheets><sheet name="TOC" sheetId="1" r:id="rId1"/>'
            f'<sheet name="{name}" sheetId="2" r:id="rId2"/></sheets></workbook>'
        ),
        "xl/_rels/workbook.xml.rels": (
            '<Relationships><Relationship Id="rId1" Type="ws" Target="worksheets/sheet1.xml"/>'
            '<Relationship Id="rId2" Type="ws" Target="worksheets/sheet2.xml"/></Relationships>'
        ),
        "xl/worksheets/sheet1.xml": f"<f>'{name}'!A1</f>" * toc_links,
        "xl/worksheets/sheet2.xml": f"<v>{name}</v>",
        "docProps/app.xml": f"<vt:lpstr>{name}</vt:lpstr>",
        "xl/sharedStrings.xml": f"<t>{name}</t>",
    }


class SheetNamesTests(ScratchTestCase):
    def test_run_renames_every_part_then_finds_nothing_to_do(self):
        self.write_parts(sheet_name_parts())
        self.assertEqual(sheet_names.run(self.workbook), ["workbook"])
        self.assertEqual(self.text_parts(), sheet_name_parts(name="oz.SumContainsλ"))
        self.assertEqual(sheet_names.run(self.workbook), [])

    def test_run_refuses_a_workbook_without_the_toc_sheet(self):
        parts = sheet_name_parts()
        parts["xl/workbook.xml"] = parts["xl/workbook.xml"].replace('name="TOC"', 'name="Cover"')
        self.write_parts(parts)
        with self.assertRaisesRegex(
            ValueError, "the workbook lacks the TOC sheet or the oz.SumContains sheet"
        ):
            sheet_names.run(self.workbook)

    def test_run_names_a_missing_part_and_a_wrong_anchor_count(self):
        parts = sheet_name_parts(toc_links=3)
        del parts["docProps/app.xml"]
        self.write_parts(parts)
        before = self.workbook.read_bytes()
        with self.assertRaises(ValueError) as caught:
            sheet_names.run(self.workbook)
        self.assertEqual(
            str(caught.exception),
            "xl/worksheets/sheet1.xml: \"'oz.SumContains'!\" expected 4 recognised anchors, "
            "got pre-rename=3 renamed=0; missing part docProps/app.xml",
        )
        self.assertEqual(self.workbook.read_bytes(), before)

    def test_main_rejects_extra_arguments(self):
        code, _ = call_main(sheet_names, "a.xlsx", "b.xlsx")
        self.assertEqual(code, "FAIL: usage: python tools/postbuild/sheet_names.py [workbook]")

    def test_main_rejects_a_missing_workbook(self):
        code, _ = call_main(sheet_names, str(self.missing))
        self.assertEqual(code, f"FAIL: no such workbook: {self.missing}")

    def test_main_reports_a_run_failure(self):
        self.workbook.write_bytes(b"not a zip archive")
        code, _ = call_main(sheet_names, str(self.workbook))
        self.assertEqual(code, "FAIL: File is not a zip file")

    def test_main_reports_the_applied_rename(self):
        self.write_parts(sheet_name_parts())
        code, output = call_main(sheet_names, str(self.workbook))
        self.assertIsNone(code)
        self.assertEqual(output, f"applied sheet names to: workbook\nOK: {self.workbook}\n")


class HelpLinksTests(ScratchTestCase):
    def setUp(self):
        super().setUp()
        self.src = self.directory / "src"
        self.src.mkdir()
        self.ratios = self.src / "Ratios.txt"

    def write_stores(self, state):
        """Both stores holding every anchor in one state: 0 is the wrong link, 1 the fix."""
        book = "".join(pair[state] for pair in help_links._forms("workbook"))
        self.write_parts({"xl/workbook.xml": f"<workbook>{book}</workbook>"})
        self.ratios.write_bytes("\n".join(p[state] for p in help_links._forms("src")).encode())

    def test_main_applies_both_stores(self):
        self.write_stores(0)
        code, output = call_main(help_links, str(self.workbook), str(self.src))
        self.assertIsNone(code)
        self.assertEqual(
            output, f"applied help links to: workbook, src/Ratios.txt\nOK: {self.workbook}\n"
        )
        book = self.text_parts()["xl/workbook.xml"]
        self.assertEqual(book, "<workbook>" + "".join(new for _o, new in help_links._forms("workbook")) + "</workbook>")
        self.assertEqual(
            self.ratios.read_text(encoding="utf-8"),
            "\n".join(new for _old, new in help_links._forms("src")),
        )

    def test_run_refuses_a_missing_ratios_module(self):
        self.write_stores(1)
        self.ratios.unlink()
        with self.assertRaisesRegex(ValueError, "^src: missing .*Ratios.txt$"):
            help_links.run(self.workbook, self.src)

    def test_run_names_every_store_without_its_anchors(self):
        self.write_parts({"xl/workbook.xml": "<workbook/>"})
        self.ratios.write_text("", encoding="utf-8")
        with self.assertRaises(ValueError) as caught:
            help_links.run(self.workbook, self.src)
        failures = str(caught.exception).split("; ")
        self.assertEqual(len(failures), 4)
        for failure, (store, (old, _new)) in zip(
            failures,
            [("workbook", pair) for pair in help_links._forms("workbook")]
            + [("src", pair) for pair in help_links._forms("src")],
        ):
            self.assertEqual(
                failure,
                f"{store}: {old[-60:]!r} expected exactly 1 recognised anchor, "
                "got wrong-link=0 fixed-link=0",
            )

    def test_main_rejects_extra_arguments(self):
        code, _ = call_main(help_links, "a.xlsx", "src", "extra")
        self.assertEqual(
            code, "FAIL: usage: python tools/postbuild/help_links.py [workbook] [src dir]"
        )

    def test_main_rejects_a_missing_workbook(self):
        code, _ = call_main(help_links, str(self.missing), str(self.src))
        self.assertEqual(code, f"FAIL: no such workbook: {self.missing}")

    def test_main_reports_a_run_failure(self):
        self.write_stores(1)
        self.ratios.unlink()
        code, _ = call_main(help_links, str(self.workbook), str(self.src))
        self.assertEqual(code, f"FAIL: src: missing {self.ratios}")


class HelpCorrectionsTests(ScratchTestCase):
    def test_validate_cells_names_each_example_missing_from_the_worksheets(self):
        failures = []
        help_corrections.validate_cells({"xl/worksheets/sheet1.xml": "<worksheet/>"}, failures)
        self.assertEqual(
            failures,
            [
                f"worksheets: {what} expected 1 recognised anchor, got 0 (before=0, after=0)"
                for what, _old, _new in help_corrections.CELL_SWAPS
            ],
        )

    def test_run_refuses_a_workbook_without_shared_strings(self):
        self.write_parts({"xl/workbook.xml": "<workbook/>"})
        with self.assertRaisesRegex(ValueError, "^workbook: xl/sharedStrings.xml is missing$"):
            help_corrections.run(self.workbook, self.directory)

    def test_run_names_each_missing_source_module(self):
        self.write_parts({"xl/workbook.xml": "<workbook/>", "xl/sharedStrings.xml": "<sst/>"})
        before = self.workbook.read_bytes()
        with self.assertRaises(ValueError) as caught:
            help_corrections.run(self.workbook, self.directory)
        failures = str(caught.exception).split("; ")
        self.assertEqual(
            failures[: len(help_corrections.MODULES)],
            [f"src: missing {self.directory / f'{m}.txt'}" for m in help_corrections.MODULES],
        )
        self.assertEqual(self.workbook.read_bytes(), before)

    def test_main_rejects_extra_arguments(self):
        code, _ = call_main(help_corrections, "a.xlsx", "src", "extra")
        self.assertEqual(
            code, "FAIL: usage: python tools/postbuild/help_corrections.py [workbook] [src dir]"
        )

    def test_main_rejects_a_missing_workbook(self):
        code, _ = call_main(help_corrections, str(self.missing))
        self.assertEqual(code, f"FAIL: no such workbook: {self.missing}")


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
