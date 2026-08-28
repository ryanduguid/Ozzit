"""Fixtures for the formula parser used by the AASB 16 post-build pass."""

import ast
from pathlib import Path
import re
from types import SimpleNamespace
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "tools" / "postbuild" / "aasb16_leases.py"


def load_parser_helpers():
    wanted = {
        "_split_literals",
        "_strip_comments",
        "_read_string",
        "declaration_params",
        "local_names",
        "_arguments",
        "xml_escape",
    }
    tree = ast.parse(SCRIPT.read_text(encoding="utf-8"), filename=str(SCRIPT))
    selected = [
        node
        for node in tree.body
        if isinstance(node, ast.FunctionDef) and node.name in wanted
    ]
    loaded = {node.name for node in selected}
    if loaded != wanted:
        raise AssertionError(f"missing parser helpers: {sorted(wanted - loaded)}")
    namespace = {"re": re}
    module = ast.fix_missing_locations(ast.Module(body=selected, type_ignores=[]))
    exec(compile(module, str(SCRIPT), "exec"), namespace)
    return SimpleNamespace(**namespace)


class Aasb16ParserTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parser = load_parser_helpers()

    def test_split_literals_keeps_escaped_quotes_inside_a_string(self):
        self.assertEqual(
            self.parser._split_literals('A & "lease ""term""" & B'),
            [
                (False, "A & "),
                (True, '"lease ""term"""'),
                (False, " & B"),
            ],
        )

    def test_split_literals_preserves_an_unterminated_literal_for_diagnostics(self):
        self.assertEqual(
            self.parser._split_literals('A & "unfinished'),
            [(False, "A & "), (True, '"unfinished')],
        )

    def test_read_string_returns_the_offset_after_the_closing_quote(self):
        fixture = 'prefix "a""b" suffix'
        literal, offset = self.parser._read_string(fixture, fixture.index('"'))

        self.assertEqual(literal, '"a""b"')
        self.assertEqual(fixture[offset:], " suffix")

    def test_strip_comments_removes_comments_but_not_markers_in_strings(self):
        fixture = (
            'LAMBDA([Lease], "https://example.test/a//b", // line comment\n'
            '  /* block comment */ Lease)'
        )
        result = self.parser._strip_comments(fixture)

        self.assertIn('"https://example.test/a//b"', result)
        self.assertNotIn("line comment", result)
        self.assertNotIn("block comment", result)
        self.assertIn("Lease)", result)

    def test_strip_comments_drops_an_unterminated_block_comment_tail(self):
        self.assertEqual(
            self.parser._strip_comments("LAMBDA([A], A) /* unfinished"),
            "LAMBDA([A], A) ",
        )

    def test_declaration_params_reads_optional_outer_lambda_parameters(self):
        body = "LAMBDA([LeasePayments], [DiscountRate], LET(Result, 1, Result))"

        self.assertEqual(
            self.parser.declaration_params(body),
            ["LeasePayments", "DiscountRate"],
        )

    def test_declaration_params_ignores_brackets_inside_strings(self):
        body = 'LAMBDA([LeasePayments], "[not a parameter]", LeasePayments)'

        self.assertEqual(self.parser.declaration_params(body), ["LeasePayments"])

    def test_arguments_respects_nested_calls_and_quoted_commas(self):
        fixture = 'LAMBDA(item, IF(item, "a,b", 0), item + 1)'
        start = fixture.index("(") + 1

        self.assertEqual(
            self.parser._arguments(fixture, start),
            ["item", ' IF(item, "a,b", 0)', " item + 1"],
        )

    def test_local_names_combines_outer_let_and_inner_lambda_bindings(self):
        body = """LAMBDA([LeasePayments],
    DiscountRate, 0.05,
    Result, MAP(LeasePayments, LAMBDA(Payment, Payment * DiscountRate)),
    Result)"""

        self.assertEqual(
            self.parser.local_names(body, ["LeasePayments"]),
            ["LeasePayments", "DiscountRate", "Result", "Payment"],
        )

    def test_local_names_deduplicates_a_reused_binding(self):
        body = """LAMBDA([LeasePayments],
    LeasePayments, LeasePayments,
    LeasePayments)"""

        self.assertEqual(
            self.parser.local_names(body, ["LeasePayments"]),
            ["LeasePayments"],
        )

    def test_xml_escape_escapes_markup_without_touching_quotes(self):
        self.assertEqual(
            self.parser.xml_escape('A&B<C>D "quoted"'),
            'A&amp;B&lt;C&gt;D "quoted"',
        )


if __name__ == "__main__":
    unittest.main()
