"""Apply and refusal paths of tools/polish_workbook.py on a small synthetic workbook.

test_workbook_presentation only runs the pass on the committed workbook, which is
already polished. These tests build an unpolished archive with every rule still to
apply, then feed each guard the malformed part it refuses.
"""

import contextlib
import io
import re
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import polish_workbook  # noqa: E402

MAIN = polish_workbook.MAIN_NS
SHEETS = list(polish_workbook.EXPECTED_STRAPLINES)
RATIOS = "oz.FinancialRatios"

STYLES = (
    f'<styleSheet xmlns="{MAIN}">'
    '<fonts count="2"><font><color rgb="FF808080"/></font>'
    '<font><color rgb="FF808080"/></font></fonts>'
    '<fills count="1"><fill><patternFill><fgColor indexed="42"/></patternFill></fill></fills>'
    '<cellXfs count="1"><xf numFmtId="14"/></cellXfs>'
    "</styleSheet>"
)


def sheet_xml(with_a2: bool) -> str:
    row2 = '<row r="2"><c r="A2" s="1" t="s"><v>0</v></c></row>' if with_a2 else ""
    return (
        f'<worksheet xmlns="{MAIN}"><sheetData>'
        f'<row r="1"><c r="A1" t="s"><v>0</v></c></row>{row2}</sheetData>'
        '<mergeCells count="1"><mergeCell ref="A1:K1"/></mergeCells></worksheet>'
    )


def unpolished_parts() -> dict[str, str]:
    sheets = "".join(
        f'<sheet name="{name}" sheetId="{n}" r:id="rId{n}"/>'
        for n, name in enumerate(["Cover", *SHEETS], start=1)
    )
    rels = "".join(
        f'<Relationship Id="rId{n}" Target="worksheets/sheet{n}.xml"/>'
        for n in range(1, len(SHEETS) + 2)
    )
    parts = {
        "xl/workbook.xml": (
            f'<workbook xmlns="{MAIN}" xmlns:r="{polish_workbook.DOC_REL_NS}">'
            f"<sheets>{sheets}</sheets></workbook>"
        ),
        "xl/_rels/workbook.xml.rels": (
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/'
            f'relationships">{rels}</Relationships>'
        ),
        "xl/sharedStrings.xml": (
            f'<sst xmlns="{MAIN}" count="9" uniqueCount="1">'
            f"<si><t>{polish_workbook.COVER_OLD}</t></si></sst>"
        ),
        "xl/styles.xml": STYLES,
        "xl/worksheets/sheet1.xml": sheet_xml(True),
    }
    for n, name in enumerate(SHEETS, start=2):
        parts[f"xl/worksheets/sheet{n}.xml"] = sheet_xml(name != RATIOS)
    return parts


def write_zip(path: Path, parts: dict[str, str]) -> None:
    with zipfile.ZipFile(path, "w") as archive:
        for name, text in parts.items():
            archive.writestr(name, text)


def run_main(argv):
    out = io.StringIO()
    with mock.patch.object(sys, "argv", argv), contextlib.redirect_stdout(out):
        try:
            polish_workbook.main()
        except SystemExit as exc:
            return exc.code, out.getvalue()
    return None, out.getvalue()


class PolishApplyTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-polish-paths-"))
        self.workbook = self.directory / "book.xlsx"
        write_zip(self.workbook, unpolished_parts())

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _part(self, name: str) -> str:
        return polish_workbook.load_parts(self.workbook)[name].decode("utf-8")

    def test_every_rule_applies_once_and_a_second_run_is_a_no_op(self):
        code, out = run_main(["polish_workbook.py", str(self.workbook)])
        self.assertIsNone(code)
        lines = out.splitlines()
        self.assertEqual(
            lines[:-1],
            ["updated Cover formula-highlight wording"]
            + [f"set {name}!A2" for name in SHEETS]
            + [
                "replaced 1 mint fill definition",
                "raised 2 strapline font colour",
                "made 1 date styles locale-safe",
            ],
        )
        self.assertRegex(
            lines[-1], rf"^OK: {re.escape(str(self.workbook))} polished, [\d,]+ bytes$"
        )

        shared = self._part("xl/sharedStrings.xml")
        values = polish_workbook.shared_values(shared)
        self.assertEqual(
            values, [polish_workbook.COVER_NEW, *polish_workbook.EXPECTED_STRAPLINES.values()]
        )
        # 8 new unique strings; only the inserted FinancialRatios cell adds a reference.
        self.assertIn('count="10" uniqueCount="9"', shared)

        paths = polish_workbook.sheet_paths(polish_workbook.load_parts(self.workbook))
        for index, name in enumerate(SHEETS, start=1):
            style = "288" if name == RATIOS else "1"
            self.assertIn(
                f'<c r="A2" s="{style}" t="s"><v>{index}</v></c>', self._part(paths[name])
            )
        self.assertIn(
            '<mergeCells count="2"><mergeCell ref="A1:K1"/><mergeCell ref="A2:K2"/></mergeCells>',
            self._part(paths[RATIOS]),
        )
        styles = self._part("xl/styles.xml")
        self.assertNotIn('indexed="42"', styles)
        self.assertNotIn("FF808080", styles)
        self.assertNotIn('numFmtId="14"', styles)

        before = self.workbook.read_bytes()
        self.assertEqual(polish_workbook.polish(self.workbook), ["already polished"])
        self.assertEqual(self.workbook.read_bytes(), before)

    def test_too_many_arguments_print_the_usage(self):
        code, _out = run_main(["polish_workbook.py", "a", "b"])
        self.assertEqual(code, "FAIL: usage: python tools/polish_workbook.py [workbook]")

    def test_a_missing_workbook_is_refused(self):
        missing = self.directory / "absent.xlsx"
        code, _out = run_main(["polish_workbook.py", str(missing)])
        self.assertEqual(code, f"FAIL: no such workbook: {missing}")

    def test_a_workbook_without_sheets_fails_cleanly(self):
        parts = unpolished_parts()
        parts["xl/workbook.xml"] = f'<workbook xmlns="{MAIN}"/>'
        write_zip(self.workbook, parts)
        code, out = run_main(["polish_workbook.py", str(self.workbook)])
        self.assertEqual(
            code, f"FAIL: cannot polish {self.workbook}: workbook has no sheets collection"
        )
        self.assertEqual(out, "")


class PolishGuardTests(unittest.TestCase):
    def test_a_repointed_ratios_cell_adds_no_reference(self):
        sheet = sheet_xml(True)
        text, changed, inserted = polish_workbook.add_financial_ratios_strapline(sheet, 4)
        self.assertEqual((changed, inserted), (True, False))
        self.assertIn('<c r="A2" s="1" t="s"><v>4</v></c>', text)

    def test_an_existing_ratios_merge_is_not_added_twice(self):
        sheet = sheet_xml(False).replace(
            '<mergeCells count="1"><mergeCell ref="A1:K1"/>',
            '<mergeCells count="1"><mergeCell ref="A2:K2"/>',
        )
        text, changed, inserted = polish_workbook.add_financial_ratios_strapline(sheet, 3)
        self.assertEqual((changed, inserted), (True, True))
        self.assertEqual(text.count("<mergeCell "), 1)
        self.assertIn('<c r="A2" s="288" t="s"><v>3</v></c>', text)

    def test_a_ratios_sheet_without_rows_is_refused(self):
        with self.assertRaisesRegex(ValueError, "^could not insert oz.FinancialRatios row 2$"):
            polish_workbook.add_financial_ratios_strapline("<sheetData/>", 0)

    def test_a_ratios_sheet_without_merges_is_refused(self):
        sheet = '<sheetData><row r="1"></row></sheetData>'
        with self.assertRaisesRegex(ValueError, "^oz.FinancialRatios mergeCells not found$"):
            polish_workbook.add_financial_ratios_strapline(sheet, 0)

    def test_a_sheet_without_a_shared_string_a2_is_refused(self):
        with self.assertRaisesRegex(ValueError, "^shared-string cell A2 not found$"):
            polish_workbook.set_existing_shared_cell('<c r="A2"><v>1</v></c>', "A2", 0)

    def test_styles_without_fonts_or_cell_formats_are_refused(self):
        for missing, message in (
            ("fonts", "styles.xml fonts section missing"),
            ("cellXfs", "styles.xml cellXfs section missing"),
        ):
            with self.subTest(missing=missing):
                styles = STYLES.replace(f"<{missing} ", f"<x{missing} ").replace(
                    f"</{missing}>", f"</x{missing}>"
                )
                with self.assertRaisesRegex(ValueError, f"^{message}$"):
                    polish_workbook.update_styles({"xl/styles.xml": styles.encode("utf-8")})

    def test_polished_styles_are_left_alone(self):
        parts = {"xl/styles.xml": b"<fonts></fonts><cellXfs></cellXfs>"}
        self.assertEqual(polish_workbook.update_styles(parts), [])
        self.assertEqual(parts, {"xl/styles.xml": b"<fonts></fonts><cellXfs></cellXfs>"})


if __name__ == "__main__":
    unittest.main()
