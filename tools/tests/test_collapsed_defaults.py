"""An argument default must not test a column inside OR() or AND().

OR and AND reduce a whole column of inputs to one TRUE or FALSE. Written as
IF(OR(ISOMITTED(x), x=""), d, x), a default gives every row d as soon as one
cell is blank. GSTAddλ, GSTExtractλ and Movementλ shipped that way in v3.4.2.
verify_sources.py reports each occurrence that KNOWN_COLLAPSED_DEFAULTS does not
list, and each listed one that has gone.
"""

import io
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import verify_sources  # noqa: E402

ROOT = TOOLS.parent


def found(body: str) -> list[str]:
    return verify_sources.collapsed_defaults(body)


class CollapsedDefaultTests(unittest.TestCase):
    def test_the_shipped_gst_and_movement_defaults_are_reported(self):
        # The v3.4.2 definitions, reduced to the lines that defaulted each argument.
        gst = ('LAMBDA([Amounts], [Rate], LET('
               'GSTRate, IF(OR(ISOMITTED(Rate), TRIM(Rate & "")=""), 0.1, Rate), '
               'Amounts * GSTRate))')
        movement = ('LAMBDA([Values], [BeginningValues], LET('
                    'BeginningValues, IF(OR(ISOMITTED(BeginningValues), BeginningValues=""), '
                    '0, BeginningValues), Values - BeginningValues))')
        self.assertEqual(found(gst), ["Rate"])
        self.assertEqual(found(movement), ["BeginningValues"])

    def test_and_and_a_negated_test_are_reported(self):
        self.assertEqual(
            found('LAMBDA([Dates], IF(AND(ISNUMBER(Dates), Dates > 0), Dates, ""))'), ["Dates"]
        )
        self.assertEqual(
            found("LAMBDA([Unit], IF(NOT(OR(ISOMITTED(Unit), Unit = \"\")), UPPER(Unit), \"D\"))"),
            ["Unit"],
        )

    def test_a_default_tested_row_by_row_passes(self):
        # The form the fix for GST and Movementλ took.
        self.assertEqual(
            found('LAMBDA([Rate], IF(ISOMITTED(Rate), 0.1, IF(TRIM(Rate & "")="", 0.1, Rate)))'),
            [],
        )

    def test_a_validator_that_reduces_to_one_flag_passes(self):
        # The λDV companions reduce each argument to one flag on purpose.
        self.assertEqual(
            found("LAMBDA([Principal], IF(OR(ISOMITTED(Principal), ISERROR(Principal)), "
                  "FALSE, OR(IFERROR(VALUE(Principal) < 1, TRUE))))"),
            [],
        )
        self.assertEqual(
            found("LAMBDA([Life], IF(OR(ISERROR(Life)), FALSE, "
                  "LET(Lives, IFERROR(VALUE(Life), -1), OR(MIN(Lives) <= 0))))"),
            [],
        )

    def test_a_shape_test_passes(self):
        # ROWS and COLUMNS answer once for the whole input, so nothing is collapsed.
        self.assertEqual(
            found("LAMBDA([Timeline], IF(OR(COLUMNS(Timeline) > 1, ROWS(Timeline) = 1), "
                  "Timeline, TRANSPOSE(Timeline)))"),
            [],
        )

    def test_only_parameters_are_read(self):
        self.assertEqual(
            found("LAMBDA([x], LET(Errors, ISERROR(x), IF(OR(Errors), 3, x)))"), []
        )

    def test_help_text_and_help_tables_are_ignored(self):
        # The same formula inside a string literal is help text, not code.
        self.assertEqual(
            found('LAMBDA([Rate], LET(Help, "IF(OR(ISOMITTED(Rate), Rate=""""), 0.1, Rate)", '
                  'IF(ISOMITTED(Rate), Help, Rate)))'),
            [],
        )
        self.assertEqual(found('TRIM(TEXTSPLIT("A→B¶C→D", "→", "¶"))'), [])

    def test_the_known_list_is_exactly_what_src_holds(self):
        held = set()
        for path in sorted((ROOT / "src").glob("*.txt")):
            for statement in verify_sources.statements(path.read_text(encoding="utf-8")):
                match = verify_sources.NAME.match(statement)
                if match:
                    held.update((match.group(1), p) for p in found(match.group(2)))
        self.assertEqual(held, set(verify_sources.KNOWN_COLLAPSED_DEFAULTS))

    def test_the_gate_reports_a_new_occurrence_and_a_stale_entry(self):
        known = dict(verify_sources.KNOWN_COLLAPSED_DEFAULTS)
        del known[("DateDifλ", "Unit")]
        known[("Fixtureλ", "Rate")] = "an entry whose occurrence has gone"
        verify_sources.failures.clear()
        output = io.StringIO()
        try:
            with (
                mock.patch.object(verify_sources, "KNOWN_COLLAPSED_DEFAULTS", known),
                mock.patch.object(verify_sources, "SRC_DIR", str(ROOT / "src")),
                mock.patch.object(verify_sources, "WORKBOOK", str(ROOT / "ozzit.xlsx")),
                redirect_stdout(output),
            ):
                result = verify_sources.main()
        finally:
            verify_sources.failures.clear()
        self.assertEqual(result, 1)
        self.assertIn("DateDifλ defaults Unit with an IF that tests it inside OR() or AND()",
                      output.getvalue())
        self.assertIn("Fixtureλ no longer defaults Rate", output.getvalue())


if __name__ == "__main__":
    unittest.main()
