"""Lease definitions exercise the same compiler seam as the postbuild pass."""

from pathlib import Path
import runpy
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "tools" / "postbuild" / "aasb16_leases.py"


class Aasb16ParserTests(unittest.TestCase):
    def setUp(self):
        self.build = runpy.run_path(str(SCRIPT))["build_definitions"]

    def render(self, body):
        definitions = [("Fixtureλ", "Synthetic fixture", "Fixtureλ = " + body + ";", "")]
        with patch.dict(self.build.__globals__, FUNCTIONS=definitions):
            return self.build({"Fixtureλ"})["oz.Fixtureλ"][1]

    def test_inline_nested_bindings_get_their_excel_markers(self):
        self.assertEqual(
            self.render("LAMBDA([Values], LET(Result, MAP(Values, LAMBDA(v, v * 2)), Result))"),
            "_xlfn.LAMBDA(_xlop.Values, _xlfn.LET(_xlpm.Result, "
            "_xlfn.MAP(_xlpm.Values, _xlfn.LAMBDA(_xlpm.v, _xlpm.v * 2)), _xlpm.Result))",
        )

    def test_comments_and_escaped_quotes_preserve_literal_text(self):
        self.assertEqual(
            self.render('LAMBDA([Lease], /* comment */ Lease & "a""b//c")'),
            '_xlfn.LAMBDA(_xlop.Lease, _xlpm.Lease & "a""b//c")',
        )

    def test_unknown_names_stop_definition_generation(self):
        with self.assertRaisesRegex(ValueError, "Undeclared"):
            self.render("LAMBDA([Lease], Lease + Undeclared)")


if __name__ == "__main__":
    unittest.main()
