import sys
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import verify_workbook  # noqa: E402


class WorkbookToolTests(unittest.TestCase):




    def test_privacy_gate_matches_real_windows_user_path(self):
        self.assertIsNotNone(
            verify_workbook.BANNED.search(r"C:\Users\Example\Documents\Ozzit")
        )

    def test_integrity_gate_rejects_xml_doctype_before_parsing(self):
        verify_workbook.failures.clear()
        verify_workbook.check_xml_part(
            "xl/worksheets/sheet1.xml",
            b'<!DOCTYPE x [<!ENTITY a "boom">]><x>&a;</x>',
        )
        self.assertIn(
            "DOCTYPE declaration in xl/worksheets/sheet1.xml",
            verify_workbook.failures,
        )












if __name__ == "__main__":
    unittest.main()
