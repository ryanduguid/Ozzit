"""The shared postbuild reader, tested where two passes now depend on one copy."""

import sys
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[3] / "tools"
sys.path.insert(0, str(TOOLS / "postbuild"))

import workbook  # noqa: E402


def book(*names: str) -> str:
    declared = "".join(f'<definedName name="{name}">1</definedName>' for name in names)
    return f"<workbook><definedNames>{declared}</definedNames></workbook>"


class WorkbookStateTests(unittest.TestCase):
    def test_none_of_the_names_is_absent(self):
        self.assertEqual(workbook.workbook_state(book("oz.Other"), ["oz.A", "oz.B"]), "absent")

    def test_all_of_the_names_is_applied(self):
        self.assertEqual(workbook.workbook_state(book("oz.A", "oz.B"), ["oz.A", "oz.B"]), "applied")

    def test_some_of_the_names_is_neither_state(self):
        with self.assertRaisesRegex(ValueError, "1 of 2 defined names"):
            workbook.workbook_state(book("oz.A"), ["oz.A", "oz.B"])

    def test_the_about_heading_is_counted_with_the_names(self):
        # A pass that adds an About heading is applied only when both arrived.
        names = ["oz.A"]
        self.assertEqual(workbook.workbook_state(book("oz.A") + "Leases", names, "Leases"), "applied")
        with self.assertRaisesRegex(ValueError, "About heading"):
            workbook.workbook_state(book("oz.A"), names, "Leases")


class OzNamesTests(unittest.TestCase):
    def test_only_the_prefixed_names_are_returned_in_document_order(self):
        self.assertEqual(workbook.oz_names(book("oz.B", "Print_Area", "oz.A")), ["oz.B", "oz.A"])


class ApplySwapsTests(unittest.TestCase):
    def test_pairs_are_applied_in_the_order_given(self):
        self.assertEqual(workbook.apply_swaps("ab", [("a", "b"), ("bb", "c")]), "c")

    def test_no_pairs_leaves_the_text_alone(self):
        self.assertEqual(workbook.apply_swaps("ab", []), "ab")


if __name__ == "__main__":
    unittest.main()
