"""The helpers other test modules rely on, run on input that takes their other paths.

Against the committed workbook and sources each helper only ever sees the shapes it
was written for, so its refusal or fallback never runs there. These call the
helpers from their own modules with small synthetic input and hold the outcome.
"""

import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

TESTS = Path(__file__).resolve().parent
sys.path.insert(0, str(TESTS))

import test_cash_flow_template as cash_flow  # noqa: E402
import test_help_matches_code as help_matches  # noqa: E402
import test_ratio_definitions as ratio_definitions  # noqa: E402
import test_repository_policy as policy  # noqa: E402

from postbuild import test_rate_date_helpers as rate_date  # noqa: E402

MAIN = cash_flow.MAIN
BOOK = '<definedName name="oz.Aλ" comment="c">=LAMBDA(x, x)</definedName>'


class WorkflowCommandTests(unittest.TestCase):
    def test_a_block_run_at_the_end_of_the_file_is_empty(self):
        self.assertEqual(policy.workflow_commands("      - run: |\n\n"), ("",))

    def test_a_block_run_without_an_indented_body_is_empty(self):
        workflow = "      - run: |\n      - name: next\n        run: python b.py\n"
        self.assertEqual(policy.workflow_commands(workflow), ("", "python b.py"))

    def test_a_block_run_ends_at_the_first_line_indented_less(self):
        workflow = (
            "      - run: |\n          echo a\n\n          echo b\n"
            "      - run: >-\n          python c.py\n          --check\n"
            "      - name: next\n"
        )
        self.assertEqual(
            policy.workflow_commands(workflow), ("echo a\necho b", "python c.py --check")
        )


class DefinedNameTests(unittest.TestCase):
    def test_each_copy_returns_its_part_of_a_present_name(self):
        self.assertEqual(help_matches.defined_name(BOOK, "oz.Aλ"), "=LAMBDA(x, x)")

    def test_each_copy_fails_on_a_missing_name(self):
        for module in (help_matches,):
            with self.subTest(module=module.__name__):
                with self.assertRaises(AssertionError) as caught:
                    module.defined_name(BOOK, "oz.Bλ")
                self.assertEqual(str(caught.exception), "oz.Bλ missing from workbook.xml")


class RatioSourceTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-ratio-source-"))
        self.source = self.directory / "Ratios.txt"

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def formulas(self, text):
        self.source.write_text(text, encoding="utf-8")
        with mock.patch.object(ratio_definitions, "SOURCE", self.source):
            return ratio_definitions.source_formulas()

    def test_a_procedure_without_a_result_line_is_left_out(self):
        text = (
            "/* FUNCTION NAME: Aλ */\nAλ = LAMBDA(x,\n//  Procedure\n"
            "    LET(\n    Result,  x + 1,\n//  Return Result\n"
            "/* FUNCTION NAME: Bλ */\nBλ = LAMBDA(x,\n//  Procedure\n    x\n//  Return Result\n"
        )
        self.assertEqual(self.formulas(text), {"Aλ": "x+1"})

    def test_a_comment_heading_another_definition_fails(self):
        with self.assertRaises(AssertionError) as caught:
            self.formulas("/* FUNCTION NAME: Cλ */\nDλ = LAMBDA(x, x)\n")
        self.assertEqual(
            str(caught.exception),
            "the FUNCTION NAME comment Cλ does not head a definition of that name",
        )


class UnapplySourceTests(unittest.TestCase):

    def test_rate_date_leaves_an_absent_module_unchanged(self):
        module = rate_date.helpers.MODULES[0]
        text = rate_date.helpers.ABOUT[module][0] + " &\n"
        unapply = rate_date.RateDateHelperContractTests._unapply_src
        self.assertEqual(unapply(module, text), text)


def worksheet(body):
    return f"<worksheet xmlns='{MAIN}' xmlns:r='{cash_flow.REL}'>{body}</worksheet>".encode()


class CashFlowHelperTests(unittest.TestCase):
    def test_values_by_address_reads_shared_inline_and_number_cells(self):
        parts = {
            "xl/sharedStrings.xml": f"<sst xmlns='{MAIN}'><si><t>Opening</t></si></sst>".encode(),
            "s.xml": worksheet(
                "<sheetData><row><c r='A1' t='s'><v>0</v></c><c r='B1'><v>5</v></c>"
                "<c r='C1' t='inlineStr'><is><t>Note</t></is></c></row></sheetData>"
            ),
        }
        self.assertEqual(
            cash_flow.values_by_address(parts, "s.xml"), {"A1": "Opening", "B1": "5", "C1": "Note"}
        )

    def test_relationship_records_resolve_a_relative_target(self):
        rels = (
            f"<Relationships xmlns='{cash_flow.PACKAGE_REL}'>"
            "<Relationship Id='rId1' Type='t' Target='../drawings/drawing1.xml'/></Relationships>"
        )
        parts = {"xl/workbook.xml": b"<x/>", "xl/worksheets/_rels/sheet1.xml.rels": rels.encode()}
        self.assertEqual(
            list(cash_flow.relationship_records(parts)),
            [
                {
                    "source": "xl/worksheets/sheet1.xml",
                    "type": "t",
                    "target": "../drawings/drawing1.xml",
                    "target_mode": "Internal",
                    "resolved_target": "xl/drawings/drawing1.xml",
                }
            ],
        )

    def test_comment_text_matches_the_address_and_is_empty_without_one(self):
        comments = (
            f"<comments xmlns='{MAIN}'><commentList>"
            "<comment ref='B2'><text><t>Read Me</t></text></comment></commentList></comments>"
        )
        parts = {"xl/styles.xml": b"<x/>", "xl/comments1.xml": comments.encode()}
        self.assertEqual(cash_flow.comment_text(parts, "b2"), b"read me")
        self.assertEqual(cash_flow.comment_text(parts, "C3"), b"")

    def test_column_width_is_none_outside_every_declared_column(self):
        parts = {"s.xml": worksheet("<cols><col min='1' max='2' width='12.5'/></cols>")}
        self.assertEqual(cash_flow.column_width(parts, "s.xml", "B"), 12.5)
        self.assertIsNone(cash_flow.column_width(parts, "s.xml", "D"))

    def test_dashboard_charts_skip_an_anchor_without_a_chart(self):
        drawing = (
            f"<xdr:wsDr xmlns:xdr='{cash_flow.DRAWING}'>"
            "<xdr:twoCellAnchor><xdr:pic/></xdr:twoCellAnchor></xdr:wsDr>"
        )
        rels = (
            f"<Relationships xmlns='{cash_flow.PACKAGE_REL}'>"
            "<Relationship Id='rId1' Type='d' Target='../drawings/drawing1.xml'/></Relationships>"
        )
        parts = {
            "xl/worksheets/sheet2.xml": worksheet("<drawing r:id='rId1'/>"),
            "xl/worksheets/_rels/sheet2.xml.rels": rels.encode(),
            "xl/drawings/drawing1.xml": drawing.encode(),
        }
        self.assertEqual(cash_flow.dashboard_charts(parts, "xl/worksheets/sheet2.xml"), [])


if __name__ == "__main__":
    unittest.main()
