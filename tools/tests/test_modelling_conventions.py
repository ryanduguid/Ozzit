"""docs/modelling-conventions.md, held to the facts it states about src/ and ozzit.xlsx.

The page records where Ozzit follows or departs from the FAST Standard and the
ICAEW Financial Modelling Code. Each check below recomputes a figure or fact the
page states and fails when it changes, so the page is revisited rather than left
out of date. Help-text lines (those holding the help table's arrows or opening
with a quotation mark) are excluded, so an example inside a help table does not
count as a function calling the function it shows.
"""

from __future__ import annotations

import html
import re
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOCUMENT = ROOT / "docs" / "modelling-conventions.md"
WORKBOOK = ROOT / "ozzit.xlsx"
DEFINITION = re.compile(r"^([A-Za-z][\w.]*λ\w*)\s*=\s*LAMBDA\(", re.MULTILINE)


def function_bodies() -> dict[str, list[str]]:
    """Each function's source lines, without its help text."""
    bodies: dict[str, list[str]] = {}
    for path in sorted((ROOT / "src").glob("*.txt")):
        text = path.read_text(encoding="utf-8")
        starts = [(m.start(), m.group(1)) for m in DEFINITION.finditer(text)]
        for index, (start, name) in enumerate(starts):
            end = starts[index + 1][0] if index + 1 < len(starts) else len(text)
            bodies[name] = [
                line for line in text[start:end].splitlines()
                if "→" not in line and not line.lstrip().startswith('"')
            ]
    return bodies


def merged_formulas() -> list[str]:
    """The formula, if any, in the first cell of every merged range."""
    formulas: list[str] = []
    with zipfile.ZipFile(WORKBOOK) as book:
        for name in book.namelist():
            if not re.fullmatch(r"xl/worksheets/sheet\d+\.xml", name):
                continue
            sheet = book.read(name).decode("utf-8")
            for first in re.findall(r'<mergeCell ref="([A-Z]+\d+):', sheet):
                cell = re.search(r'<c r="' + first + r'"[^>]*>(.*?)</c>', sheet, re.DOTALL)
                formula = re.search(r"<f[^>]*>(.*?)</f>", cell.group(1), re.DOTALL) if cell else None
                if formula:
                    formulas.append("=" + html.unescape(formula.group(1)))
    return formulas


def calling(pattern: str) -> set[str]:
    found = re.compile(pattern)
    return {name for name, lines in function_bodies().items() if any(found.search(line) for line in lines)}


class ModellingConventionsTest(unittest.TestCase):
    page = " ".join(DOCUMENT.read_text(encoding="utf-8").split())

    def test_the_function_and_help_table_counts(self) -> None:
        self.assertEqual(len(function_bodies()), 133)
        self.assertIn("a library of 133 LAMBDA functions and 5 help tables", self.page)
        self.assertIn("Ozzit's 133 functions exist to be called from cells", self.page)
        tables = set()
        for path in (ROOT / "src").glob("*.txt"):
            tables |= set(re.findall(r"^(About\w+λ)\s*=", path.read_text(encoding="utf-8"), re.MULTILINE))
        self.assertEqual(len(tables), 5)
        self.assertIn("5 About tables list each module's functions", self.page)

    def test_utilities_repeats_the_essentials_functions(self) -> None:
        def defined(module: str) -> set[str]:
            return set(DEFINITION.findall((ROOT / "src" / f"{module}.txt").read_text(encoding="utf-8")))

        essentials, utilities = defined("Essentials"), defined("Utilities")
        self.assertEqual(len(essentials), 16)
        # Each Essentials name reappears in Utilities with a U before the λ; the
        # E-tagged range function is Essentials' own copy of RangeToDAλ.
        self.assertEqual({name.replace("Eλ", "λ").replace("λ", "Uλ") for name in essentials}, utilities)
        self.assertIn("Utilities repeats the 16 Essentials functions with a `U` suffix", self.page)

    def test_if_and_nested_if(self) -> None:
        self.assertEqual(len(calling(r"\bIF\(")), 59)
        nested = {n for n, lines in function_bodies().items() if any(len(re.findall(r"\bIF\(", line)) > 1 for line in lines)}
        self.assertGreaterEqual(len(nested), 18)
        self.assertIn("59 functions call `IF`", self.page)
        self.assertIn("At least 18 functions nest one `IF` inside another", self.page)

    def test_help_tables_and_the_closing_choose(self) -> None:
        validators = {"AmortiseλDV", "CorkscrewλDV", "DepreciateλDV"}
        helped = set(function_bodies()) - validators
        self.assertEqual(calling(r"CHOOSE\(\s*(Help\?\s*\+\s*1|Return)\s*,\s*Result\s*,\s*Help\s*[,)]"), helped)
        self.assertEqual(calling(r"CHOOSE\(\s*Return\s*,\s*TRUE\s*,\s*Messages\s*,\s*#VALUE!\s*\)"), validators)
        # Every parameter of a function with a help table has a Required or Optional row.
        for path in sorted((ROOT / "src").glob("*.txt")):
            text = path.read_text(encoding="utf-8")
            starts = [(m.start(), m.group(1)) for m in DEFINITION.finditer(text)]
            for index, (start, name) in enumerate(starts):
                if name in validators:
                    continue
                block = text[start:starts[index + 1][0] if index + 1 < len(starts) else len(text)]
                for parameter in re.findall(r"^\s*\[(\w+)\]", block.split("LET(", 1)[0], re.MULTILINE):
                    with self.subTest(function=name, parameter=parameter):
                        self.assertRegex(block, r'"\s*' + parameter + r"\s*→\s*\((Required|Optional)")
        self.assertEqual(len(calling(r"\bINDEX\(")), 15)
        self.assertIn("use `INDEX` (15 functions)", self.page)

    def test_offset_indirect_and_npv(self) -> None:
        self.assertEqual(calling(r"\bOFFSET\("), {"RangeToDAλ", "RangeToDAEλ", "RangeToDAUλ"})
        self.assertEqual(calling(r"\b(INDIRECT|X?NPV)\("), set())

    def test_rounding_and_the_days_a_month_constant(self) -> None:
        self.assertEqual(
            calling(r"\bROUND(UP|DOWN)?\("),
            {"Allocateλ", "Amortiseλ", "DayCountRateλ", "Depreciateλ", "DiminishingValueλ", "PrimeCostλ", "TimelineOffsetλ"},
        )
        self.assertEqual(calling(r"30\.5"), {"DayCountRateλ", "Depreciateλ", "TimelineOffsetλ"})

    def test_the_embedded_gst_rate_and_the_days_a_year_convention(self) -> None:
        self.assertEqual(calling(r"IF\(ISOMITTED\(Rate\), 0\.1,"), {"GSTAddλ", "GSTExtractλ"})
        self.assertIn("fall back to a rate of 0.1 written into the formula", self.page)
        self.assertEqual(calling(r"DpY,\s*365"), {"DSIλ"})
        self.assertIn("`oz.DSIλ` names its 365 days a year in a `LET` step", self.page)

    def test_names_merged_cells_and_macros(self) -> None:
        with zipfile.ZipFile(WORKBOOK) as book:
            workbook = book.read("xl/workbook.xml").decode("utf-8")
            sheets = [n for n in book.namelist() if re.fullmatch(r"xl/worksheets/sheet\d+\.xml", n)]
            merged = [n for n in sheets if b"<mergeCell " in book.read(n)]
            members = book.namelist()
        local = re.findall(r'<definedName name="([^"]+)"[^>]*localSheetId', workbook)
        self.assertTrue(local)
        self.assertEqual(set(local), {"_xlnm.Print_Area"})
        self.assertEqual((len(merged), len(sheets)), (48, 49))
        titles = [f for f in merged_formulas() if f == '=_xlfn.TEXTAFTER(CELL("filename",A1),"]")']
        self.assertEqual((len(titles), len(merged_formulas())), (46, 51))
        self.assertIn("Of the 51 formulas in merged ranges, 46 are a title", self.page)
        self.assertIn("48 of the workbook's 49 sheets", self.page)
        self.assertFalse([n for n in members if "vbaproject" in n.lower()])


if __name__ == "__main__":
    unittest.main()
