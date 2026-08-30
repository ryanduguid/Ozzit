"""Guards on help that promised something the formula did not do.

verify_signatures.py compares a help's FUNCTION line, parameter table and worked
examples with the LAMBDA's own declaration. None of that reads what an example
claims the answer is, or whether the body applies the default the table names, so
each of these shipped with the help and the code contradicting each other and
every gate passing.

Each check reads every store that states the thing. src/ is what a reader diffs
and imports; the defined name in ozzit.xlsx is what Excel actually evaluates, and
the two have drifted apart before. A claim spilled onto a demonstration worksheet
is checked in the cell caching that spill as well. A claim typed into a label or
description column is checked in the shared string table, because such a cell has
no formula and sits under no spill anchor: Excel never revisits it, so correcting
the defined name alone leaves the sentence a reader sees unchanged.
"""

import html
import re
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src"
README = ROOT / "README.md"

# a function name written out on its own, the way a label column carries one
NAME = re.compile(r"(?<![A-Za-z0-9_.])(?:oz\.)?([A-Za-z_][A-Za-z0-9_]*λ[A-Za-z0-9_]*)")


def defined_name(book: str, name: str) -> str:
    match = re.search(
        rf'<definedName name="{re.escape(name)}"[^>]*>(.*?)</definedName>',
        book,
        re.DOTALL,
    )
    if match is None:
        raise AssertionError(f"{name} missing from workbook.xml")
    return match.group(1)


def shared_strings(part: str) -> list[str]:
    """Every entry of the shared string table, as the text a cell displays."""
    return [
        "".join(html.unescape(run) for run in re.findall(r"<t[^>]*>(.*?)</t>", item, re.DOTALL))
        for item in re.findall(r"<si>(.*?)</si>", part, re.DOTALL)
    ]


class HelpMatchesCodeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with zipfile.ZipFile(ROOT / "ozzit.xlsx") as archive:
            cls.book = archive.read("xl/workbook.xml").decode("utf-8")
            cls.sheets = "".join(
                archive.read(name).decode("utf-8")
                for name in archive.namelist()
                if name.startswith("xl/worksheets/")
            )
            cls.strings = shared_strings(
                archive.read("xl/sharedStrings.xml").decode("utf-8")
            )
        cls.modules = {
            path.stem: path.read_text(encoding="utf-8") for path in SRC.glob("*.txt")
        }
        cls.declared = {
            match.group(1)
            for text in cls.modules.values()
            for match in re.finditer(r"^([A-Za-z0-9_]*λ[A-Za-z0-9_]*)\s*=", text, re.MULTILINE)
        }

    def test_is_between_helpers_apply_the_inclusive_default_they_document(self):
        # Both help tables call Inclusive optional with TRUE as the default. An
        # omitted LAMBDA argument evaluates as an empty value, so without this
        # binding IF( Inclusive, ...) takes the exclusive branch and the help's own
        # =IsBetweenEλ(2, 2, 4) example returns FALSE where it prints TRUE.
        binding = "Inclusive,      IF( ISLOGICAL( Inclusive), Inclusive, TRUE),"
        stored = "_xlpm.Inclusive, IF(ISLOGICAL(_xlpm.Inclusive), _xlpm.Inclusive, TRUE)"
        for module, name in (("Essentials", "IsBetweenEλ"), ("Utilities", "IsBetweenUλ")):
            with self.subTest(function=name):
                body = self.modules[module].split(f"{name} = LAMBDA(")[1]
                declaration = body.split("\n);")[0]
                self.assertIn("(Optional) If set to TRUE (default)", declaration)
                self.assertIn(binding, declaration)
                self.assertLess(
                    declaration.index(binding),
                    declaration.index("Result,         IF( Inclusive,"),
                    f"{name} must bind Inclusive before it reads it",
                )
                self.assertIn(stored, defined_name(self.book, f"oz.{name}"))

    def test_movement_example_passes_the_two_arguments_it_declares(self):
        # Movementλ = LAMBDA([BeginningValues], [Values], ...). Excel rejects a
        # LAMBDA call carrying more arguments than the function has parameters,
        # so the printed example could not be evaluated at all.
        stale = "=oz.Movementλ(,{100,110,130,100}, 4)"
        current = "=oz.Movementλ(,{100,110,130,100})"
        self.assertIn(current, self.modules["Financial"])
        self.assertNotIn(stale, self.modules["Financial"])
        self.assertIn(current, defined_name(self.book, "oz.Movementλ"))
        self.assertNotIn(stale, self.book)
        self.assertIn(f"<v>{current}</v>", self.sheets)
        self.assertNotIn(f"<v>{stale}</v>", self.sheets)

    def test_reversal_example_prints_the_row_the_function_returns(self):
        # Column 1 is the opening value and every later column negates the previous
        # input, which is what reversing "in the next period" means. The old example
        # printed the negated input instead, one period early.
        stale = '"-100,-110,-130 →=oz.Reversalλ( , {100,110,130})"'
        current = '"0,-100,-110    →=oz.Reversalλ( , {100,110,130})"'
        self.assertIn(current, self.modules["Financial"])
        self.assertNotIn(stale, self.modules["Financial"])
        self.assertIn(current, defined_name(self.book, "oz.Reversalλ"))
        self.assertIn("<v>0,-100,-110</v>", self.sheets)
        self.assertNotIn("<v>-100,-110,-130</v>", self.sheets)

    def test_period_diff_example_prints_the_number_its_formula_returns(self):
        # 1 Jan to 15 Apr spans Q1 and Q2: Months = 4, Periods = 1, PartialPeriod
        # is TRUE, so Periods + OR(...) is 2. The inclusive count is deliberate;
        # Depreciateλ relies on it for the salvage period.
        example = '→=oz.PeriodDiffλ(""2026-01-01"", ""2026-04-15"", ""Q"")'
        for store in (self.modules["Financial"], self.book):
            with self.subTest(store="src" if store is not self.book else "workbook"):
                self.assertIn('"2              ' + example, store)
                self.assertNotIn('"1              ' + example, store)

    def test_about_dates_lists_no_diagnostic_function_the_library_never_shipped(self):
        # The block told readers to insert 'DV' and type CountDOWλDV( Start, End, 1).
        # The library declares three λDV functions, all in Financial, and no release
        # ever defined a Dates one, so every call it named returned #NAME?.
        for store in (self.modules["Dates"], defined_name(self.book, "oz.AboutDatesλ")):
            self.assertNotIn("DIAGNOSTICS", store)
            self.assertNotIn("λDV", store)
            self.assertIn("FinancialYearλ", store)

    def test_about_ratios_names_the_two_ratios_as_the_library_declares_them(self):
        # The same two rows are typed a second time into the label column of the
        # oz.FinancialRatios sheet, in cells no formula feeds, so both stores and
        # the shared string table have to agree.
        stores = (
            self.modules["Ratios"],
            defined_name(self.book, "oz.AboutRatiosλ"),
            self.sheets,
            "\n".join(self.strings),
        )
        for index, store in enumerate(stores):
            with self.subTest(store=("src", "definedName", "worksheets", "sharedStrings")[index]):
                self.assertNotIn("DSIRatioλ", store)
                self.assertNotIn("DividendPayoutRatioλ", store)
        self.assertIn("DSIλ", self.strings)
        self.assertIn("DPRλ", self.strings)

    def test_no_static_cell_names_a_function_the_library_never_declared(self):
        # A label column is a list of functions to go and type, and every cell in
        # one is a static literal: no formula, no covering spill anchor, so Excel
        # never refreshes it. verify_signatures.py makes this check against src/;
        # nothing made it against the cells, which is how the pre-rename Aboutλ,
        # DSIRatioλ and DividendPayoutRatioλ outlived the names they came from.
        self.assertGreater(len(self.declared), 100, "no declarations were read")
        unknown = {
            (name, text)
            for text in self.strings
            for name in NAME.findall(text)
            if name not in self.declared
        }
        self.assertEqual(unknown, set())

    def test_static_signature_cells_match_the_declaration_they_describe(self):
        # oz.IsOccurrenceDateλ's demonstration sheet writes the signature out in a
        # cell of its own. It listed a fifth argument, [Diagnostics], that the
        # LAMBDA has never declared and its own FUNCTION line has never shown.
        signatures = [text for text in self.strings if text.startswith("IsOccurrenceDateλ(")]
        self.assertEqual(
            signatures,
            ["IsOccurrenceDateλ(Dates, FirstOccurrence, [LastOccurrence], [Repeats])"],
        )
        declaration = self.modules["Dates"].split("IsOccurrenceDateλ = LAMBDA(")[1]
        parameters = declaration.split("//  Help")[0]
        self.assertNotIn("Diagnostics", parameters)
        self.assertIn("→" + signatures[0] + "¶", declaration)

    def test_corkscrew_dv_reports_its_findings_without_a_sixth_argument(self):
        # Errors2Show gated both message arrays on an undocumented Diagnostics
        # argument. Omitted, it evaluated as 0, so a real problem fell through to
        # CHOOSE's third branch and the caller saw #VALUE! rather than the advice.
        # AmortiseλDV and DepreciateλDV never carried the multiplier.
        stored = defined_name(self.book, "oz.CorkscrewλDV")
        self.assertNotIn("Diagnostics", stored)
        self.assertIn(
            "_xlpm.Errors2Show, _xlfn.VSTACK(_xlpm.ErrorsInArgs, _xlpm.DVErrors)", stored
        )
        declaration = self.modules["Financial"].split("CorkscrewλDV = LAMBDA(")[1]
        declaration = declaration.split("\n);")[0]
        self.assertNotIn("Diagnostics", declaration)
        self.assertIn("Errors2Show,    VSTACK( ErrorsInArgs, DVErrors)", declaration)

    def test_sum_depreciate_is_not_described_as_totalling_book_value(self):
        # The block's Book Value row falls to the SWITCH default and stays 0.
        # Totalling a balance row would be meaningless, so the code is right.
        # The same sentence is typed into two static cells, the TOC row for the
        # function and the heading of its own demonstration sheet, and both read
        # the one shared string.
        claim = "CAPEX, Depreciation, Book Value, Salvage Value"
        corrected = "CAPEX, Depreciation, Salvage Value, and Disposal costs"
        self.assertNotIn(claim, self.modules["Financial"])
        self.assertNotIn(claim, self.book)
        self.assertNotIn(claim, self.sheets)
        self.assertNotIn(claim, README.read_text(encoding="utf-8"))
        self.assertIn(corrected, defined_name(self.book, "oz.AboutFinancialλ"))
        described = [text for text in self.strings if "Disposal costs in Depreciateλ" in text]
        self.assertEqual(
            described,
            ["Create row totals for " + corrected + " in Depreciateλ results"],
        )


if __name__ == "__main__":
    unittest.main()
