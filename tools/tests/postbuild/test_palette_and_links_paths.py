"""Refusal and apply paths of 3 postbuild passes, on small synthetic workbooks.

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
        "xl/theme/theme1.xml": '<a:theme><a:accent1><a:srgbClr val="3F656F"/></a:accent1></a:theme>',
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
        self.assertEqual(palette.hue_map("40A0A0"), "865A74")
        self.assertEqual(palette.hue_map("C08060"), "9B8F85")

    def test_recolour_remaps_a_six_digit_theme_value(self):
        self.assertEqual(
            palette.recolour('<a:srgbClr val="3F656F"/>'), ('<a:srgbClr val="66023C"/>', 1)
        )

    def test_tab_colours_follow_the_module_roles(self):
        sheet = '<sheetPr><tabColor rgb="FF{}"/></sheetPr>'
        for old, new in (("4F485E", "E5E4E2"), ("2B1A54", "000000"), ("5C2D91", "66023C"), ("7A5AB5", "990024")):
            self.assertEqual(palette.tab_colours(sheet.format(old)), (sheet.format(new), 1))

    def test_theme_slots_keep_the_system_window_colours(self):
        theme = (
            '<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>'
            '<a:accent1><a:srgbClr val="5C2D91"/></a:accent1><a:hlink><a:srgbClr val="0563C1"/></a:hlink>'
        )
        self.assertEqual(
            palette.theme_slots(theme),
            '<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>'
            '<a:accent1><a:srgbClr val="66023C"/></a:accent1><a:hlink><a:srgbClr val="66023C"/></a:hlink>',
        )

    def test_white_text_becomes_ivory_but_white_borders_stay(self):
        styles = '<font><color rgb="FFFFFFFF"/></font><border><top><color rgb="FFFFFFFF"/></top></border>'
        self.assertEqual(
            palette.ivory_text(styles),
            ('<font><color rgb="FFFFFFF0"/></font><border><top><color rgb="FFFFFFFF"/></top></border>', 1),
        )

    def test_passed_checks_get_their_own_green_font_and_links_keep_theirs(self):
        styles = (
            '<styleSheet><fonts count="1"><font><color rgb="FF008000"/></font></fonts>'
            '<fills count="2"><fill><patternFill patternType="none"/></fill>'
            '<fill><patternFill patternType="solid"><fgColor rgb="FFDBE4E0"/></patternFill></fill></fills>'
            '<cellXfs count="2"><xf fontId="0" fillId="0"/><xf fontId="0" fillId="1"/></cellXfs>'
            '<dxfs count="2"><dxf><font><color rgb="FF008000"/></font>'
            '<fill><patternFill><bgColor rgb="FFDBE4E0"/></patternFill></fill></dxf>'
            '<dxf><font><color rgb="FF008000"/></font></dxf></dxfs></styleSheet>'
        )
        result, count = palette.passed_green(styles)
        self.assertEqual(count, 2)
        self.assertIn(
            '<fonts count="2"><font><color rgb="FF008000"/></font><font><color rgb="FF0F4B32"/></font></fonts>', result
        )
        self.assertIn('<xf fontId="0" fillId="0"/><xf fontId="1" fillId="1"/>', result)
        self.assertIn('<dxf><font><color rgb="FF008000"/></font></dxf>', result)
        self.assertEqual(palette.passed_green(result), (result, 0))

    def test_passed_checks_sharing_a_font_share_one_copy(self):
        styles = (
            '<styleSheet><fonts count="2"><font><color rgb="FF008000"/></font><font><b/></font></fonts>'
            '<fills count="1"><fill><patternFill patternType="solid"><fgColor rgb="FFDBE4E0"/></patternFill></fill></fills>'
            '<cellXfs count="4"><xf fontId="0" fillId="0"/><xf fontId="0" fillId="0"/>'
            '<xf fontId="1" fillId="0"/><xf numFmtId="0"/></cellXfs></styleSheet>'
        )
        result, count = palette.passed_green(styles)
        self.assertEqual(count, 2)
        self.assertIn('<font><b/></font><font><color rgb="FF0F4B32"/></font></fonts>', result)
        self.assertIn(
            '<xf fontId="2" fillId="0"/><xf fontId="2" fillId="0"/><xf fontId="1" fillId="0"/><xf numFmtId="0"/>',
            result,
        )

    def test_passed_green_leaves_styles_without_cell_formats(self):
        styles = "<styleSheet><fonts/></styleSheet>"
        self.assertEqual(palette.passed_green(styles), (styles, 0))

    def test_hue_map_leaves_greys_alone(self):
        self.assertIsNone(palette.hue_map("808080"))

    def test_run_sets_tabs_the_green_outline_and_the_input_fill(self):
        parts = palette_parts()
        parts["xl/worksheets/sheet1.xml"] = '<worksheet><sheetPr><tabColor rgb="FF5C2D91"/></sheetPr></worksheet>'
        parts["xl/worksheets/sheet2.xml"] = "<worksheet/>"
        parts["xl/drawings/drawing1.xml"] = '<a:ln><a:solidFill><a:prstClr val="green"/></a:solidFill></a:ln>'
        parts["xl/drawings/drawing2.xml"] = "<xdr:wsDr/>"
        parts["xl/styles.xml"] = parts["xl/styles.xml"].replace(
            "</fills>", f"<fill><patternFill>{palette.INPUT_FILL}</patternFill></fill></fills>"
        )
        self.write_parts(parts)
        changes = palette.run(self.workbook)
        self.assertEqual(changes[0], "tab colours set (1 sheets)")
        self.assertIn("xl/drawings/drawing1.xml: green outline made platinum", changes)
        self.assertIn("input fills made platinum (1 entries)", changes)
        written = self.text_parts()
        self.assertIn('<tabColor rgb="FF66023C"/>', written["xl/worksheets/sheet1.xml"])
        self.assertIn('<a:schemeClr val="accent3"/>', written["xl/drawings/drawing1.xml"])
        self.assertIn('<fgColor rgb="FFE5E4E2"/>', written["xl/styles.xml"])
        self.assertEqual(palette.run(self.workbook), [])

    def test_template_run_without_shared_strings_then_nothing_to_do(self):
        parts = palette_parts(dxfs="<dxfs></dxfs>")
        del parts["xl/sharedStrings.xml"]
        parts["xl/styles.xml"] = parts["xl/styles.xml"].replace(
            "</fonts>", '<font><color rgb="FFFFFFFF"/></font></fonts>'
        )
        self.write_parts(parts)
        self.assertEqual(
            palette.run(self.workbook, template=True),
            ["xl/theme/theme1.xml: theme colours set", "white text made ivory (1 entries)"],
        )
        self.assertEqual(palette.run(self.workbook, template=True), [])

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
                "xl/theme/theme1.xml: theme colours set",
                "xl/styles.xml: 2 colours remapped",
                'font family <name val="Arial"/> -> <name val="Aptos"/>',
                'rich-text font <rFont val="Arial"/> -> <rFont val="Aptos"/>',
                "help-label fonts folded to palette (1 entries)",
                "help-block fills folded to ivory (1 entries)",
            ],
        )
        parts = self.text_parts()
        self.assertIn('val="66023C"', parts["xl/theme/theme1.xml"])
        styles = parts["xl/styles.xml"]
        self.assertIn('<name val="Aptos"/><color rgb="FF66023C"/>', styles)
        self.assertIn('<fgColor rgb="FFFFFFF0"/>', styles)
        self.assertIn('<color rgb="FF990024"/>', styles)
        self.assertIn('<bgColor rgb="FFF0D9DE"/>', styles)
        self.assertIn('<rFont val="Aptos"/>', parts["xl/sharedStrings.xml"])
        self.assertEqual(palette.run(self.workbook), [])

    def test_main_rejects_extra_arguments(self):
        code, _ = call_main(palette, "a.xlsx", "b.xlsx")
        self.assertEqual(code, "FAIL: usage: python tools/postbuild/workbook_palette.py [--template] [workbook]")

    def test_template_mode_skips_the_ozzit_folds_and_guard(self):
        self.write_parts(palette_parts(dxfs="<dxfs></dxfs>"))
        code, output = call_main(palette, "--template", str(self.workbook))
        self.assertIsNone(code)
        self.assertIn("theme colours set", output)
        styles = self.text_parts()["xl/styles.xml"]
        self.assertIn('<name val="Arial"/><color rgb="FF006600"/>', styles)
        self.assertIn('<fgColor rgb="FFCCFFCC"/>', styles)

    def test_main_rejects_a_missing_workbook(self):
        code, _ = call_main(palette, str(self.missing))
        self.assertEqual(code, f"FAIL: no such workbook: {self.missing}")

    def test_main_reports_a_remapped_toc_warning_without_writing(self):
        self.write_parts(palette_parts(dxfs="<dxfs></dxfs>"))
        before = self.workbook.read_bytes()
        code, _ = call_main(palette, str(self.workbook))
        self.assertEqual(code, "FAIL: TOC warning colours are missing")
        self.assertEqual(self.workbook.read_bytes(), before)

    def test_main_reports_a_file_that_is_not_a_workbook(self):
        self.workbook.write_bytes(b"not a zip archive")
        code, _ = call_main(palette, str(self.workbook))
        self.assertEqual(code, "FAIL: File is not a zip file")


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
