"""Contract for tools/generate_selftest_examples.py and the fragment it writes.

The fragment is a bound view of the help in src/, the way functions.csv is a
bound view of the workbook: it must be regenerable byte for byte, stay pure
ASCII so the self-test's encoding cannot corrupt it, and be dot-sourced by the
self-test. The parser tests pin every layout the help uses for its examples.
"""

import re
import sys
import unittest
from decimal import Decimal
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import generate_selftest_examples as generator  # noqa: E402

ROOT = TOOLS.parent
SRC = ROOT / "src"
FRAGMENT = TOOLS / "selftest_examples.ps1"
SELFTEST = TOOLS / "excel_selftest.ps1"
LIBRARY = {"Thisλ", "Timelineλ"}


def body(*rows: str) -> str:
    """A LAMBDA whose help is the given rows, in the source's own escaping."""
    text = "¶".join(rows).replace('"', '""')
    return f'LAMBDA([x], LET(Help, TRIM(TEXTSPLIT("{text}", "→", "¶")), x))'


class FragmentTests(unittest.TestCase):
    def test_committed_fragment_is_current_and_ascii(self):
        generated = generator.generate(SRC)
        self.assertTrue(generated.isascii())
        self.assertEqual(FRAGMENT.read_text(encoding="ascii"), generated)

    def test_every_function_gets_a_help_assertion(self):
        text = FRAGMENT.read_text(encoding="ascii")
        helps = re.findall(r"^(?:Same|Near) 'help: ([A-Za-z0-9_]+)\$L(DV)?'", text, re.M)
        self.assertEqual(len(helps), 134)
        self.assertIn("Same 'help: CountDOW$L' \"INDEX(oz.CountDOW$L(),1,1)\" 'FUNCTION:'", text)
        self.assertIn("Same 'help: AboutDates$L' \"INDEX(oz.AboutDates$L,1,1)\" 'About:'", text)
        self.assertIn("Near 'help: Amortise$LDV' \"N(oz.Amortise$LDV())\" '1'", text)

    def test_self_test_dot_sources_the_fragment_after_its_helpers(self):
        text = SELFTEST.read_text(encoding="ascii")
        helpers = text.index("function Same(")
        sourced = text.index(". (Join-Path $PSScriptRoot 'selftest_examples.ps1')")
        self.assertGreater(sourced, helpers)

    def test_examples_that_need_undefined_names_are_left_out(self):
        text = FRAGMENT.read_text(encoding="ascii")
        self.assertNotIn("tblRates", text)
        self.assertNotIn("ModelPeriodCount", text)
        self.assertNotIn("AmortisationSchedule", text)


class ParserTests(unittest.TestCase):
    def examples(self, *rows: str):
        return generator.examples_of(body(*rows))

    def test_result_label_before_formula(self):
        examples, bindings = self.examples(
            "FUNCTION:→Thisλ(x)", "EXAMPLES:→", "Result→Formula (oz is assumed to be the module's name)",
            "3→=oz.Thisλ(1)", "TRUE→=Thisλ(2)",
        )
        self.assertEqual(bindings, {})
        self.assertEqual([(e.formula, e.results) for e in examples], [("oz.Thisλ(1)", ["3"]), ("Thisλ(2)", ["TRUE"])])

    def test_formula_then_result_rows(self):
        examples, _ = self.examples(
            "EXAMPLES:→", "→Formula (oz is assumed to be the module's name)", "→=oz.Thisλ(1000, 100, 5)",
            "→Result", "→180,180", "→180,180",
        )
        self.assertEqual([(e.formula, e.results) for e in examples], [("oz.Thisλ(1000, 100, 5)", ["180,180", "180,180"])])

    def test_named_inputs_are_bindings(self):
        examples, bindings = self.examples(
            "EXAMPLES:→", "Result→Formula", "Dates→=EDATE(1, SEQUENCE(,24, 0))", "Values→={1,2}", "→", "Result→Formula",
            "25%→=oz.Thisλ( Values, Dates)",
        )
        self.assertEqual(bindings, {"Dates": "EDATE(1, SEQUENCE(,24, 0))", "Values": "{1,2}"})
        self.assertEqual([(e.formula, e.results) for e in examples], [("oz.Thisλ( Values, Dates)", ["25%"])])

    def test_grid_printed_one_row_per_label_and_result_label_rows(self):
        examples, _ = self.examples(
            "EXAMPLES:→", "Result→Formula", "03, 07→=oz.Thisλ({1,2})", "07, 03→", "30, 70→",
        )
        self.assertEqual(examples[0].results, ["03, 07", "07, 03", "30, 70"])
        examples, _ = self.examples(
            "EXAMPLES:→NOTE! oz is assumed", "Formula:→=oz.Thisλ(0, SEQUENCE(1,5))", "Result:→00, 01", "→01, 02",
        )
        self.assertEqual(examples[0].results, ["00, 01", "01, 02"])


class CheckTests(unittest.TestCase):
    def checks(self, formula: str, *results: str, bindings=None):
        example = generator.Example()
        example.formula = formula
        example.results = list(results)
        return generator.checks_for("Thisλ", example, 1, LIBRARY, bindings or {})

    def test_number_within_half_the_last_digit(self):
        self.assertEqual(self.checks("oz.Thisλ(1)", "222.90"), ["Near 'example: This$L #1' \"oz.This$L(1)\" '222.9' '0.005'"])
        self.assertEqual(self.checks("oz.Thisλ(1)", "25%"), ["Near 'example: This$L #1' \"oz.This$L(1)\" '0.25' '0.005'"])
        self.assertEqual(self.checks("oz.Thisλ(1)", "40.89 days")[0].split("'")[-2], "0.005")

    def test_boolean_shape_date_word_and_list(self):
        self.assertEqual(self.checks("Thisλ(1)", "FALSE"), ["Near 'example: This$L #1' \"N(oz.This$L(1))\" '0'"])
        self.assertEqual(
            self.checks("oz.Thisλ(1)", "5 rows, 60 cols"),
            ["Near 'example: This$L #1' \"ROWS(oz.This$L(1))\" '5'", "Near 'example: This$L #1' \"COLUMNS(oz.This$L(1))\" '60'"],
        )
        self.assertEqual(self.checks("oz.Thisλ(1)", "2026-10-01"), ["Near 'example: This$L #1' \"oz.This$L(1) - DATE(2026,10,1)\" '0' '0.5'"])
        self.assertEqual(self.checks("oz.Thisλ(1)", "FY2027"), ["Same 'example: This$L #1' \"oz.This$L(1)\" 'FY2027'"])
        self.assertEqual(
            self.checks("oz.Thisλ(1)", "1,3", "6,10"),
            ["Near 'example: This$L #1' \"COUNT(oz.This$L(1))\" '4'", "Near 'example: This$L #1' \"SUM(oz.This$L(1))\" '20.0' '2.0'"],
        )

    def test_totals_are_exact_decimals_so_every_python_agrees(self):
        # Floats drift (0.1 + 0.2 + 0.3 is 0.6000000000000001 in binary) and Python 3.12
        # changed how sum() adds them, which made the fragment differ between interpreters.
        # The printed digits are summed exactly and written as the shortest float literal.
        self.assertEqual(
            self.checks("oz.Thisλ(1)", "0.1,0.2", "0.3"),
            ["Near 'example: This$L #1' \"COUNT(oz.This$L(1))\" '3'", "Near 'example: This$L #1' \"SUM(oz.This$L(1))\" '0.6' '0.15'"],
        )
        self.assertEqual(
            self.checks("oz.Thisλ(1)", "12.6122%"),
            ["Near 'example: This$L #1' \"oz.This$L(1)\" '0.126122' '5e-07'"],
        )
        self.assertEqual(generator.number("831.93"), (Decimal("831.93"), Decimal("0.005")))
        self.assertEqual(generator.number("25%"), (Decimal("0.25"), Decimal("0.005")))
        self.assertIsNone(generator.number("%"))

    def test_anything_else_is_checked_for_no_error(self):
        self.assertEqual(
            self.checks("oz.Thisλ(TODAY())", "12 mo, starts"),
            ["Near 'example: This$L #1' \"SUMPRODUCT(--ISERROR(oz.This$L(TODAY())))\" '0'"],
        )

    def test_bindings_are_wrapped_in_let_and_quotes_are_escaped(self):
        checks = self.checks('oz.Thisλ(Values, "a")', "3", bindings={"Values": "{1,2}"})
        self.assertEqual(checks, ["Near 'example: This$L #1' \"LET(Values, {1,2}, oz.This$L(Values, `\"a`\"))\" '3.0' '0.5'"])

    def test_undefined_names_table_references_and_cell_references_are_skipped(self):
        self.assertEqual(self.checks("oz.Thisλ(Array, Labels)", "3"), [])
        self.assertEqual(self.checks("oz.Thisλ(tbl[Col])", "3"), [])
        self.assertEqual(self.checks("oz.Thisλ(A1, 2)", "3"), [])
        self.assertEqual(self.checks("oz.Thisλ(Timelineλ(1), TRUE)", "3")[0].count("oz.Timeline$L("), 1)


if __name__ == "__main__":
    unittest.main()
