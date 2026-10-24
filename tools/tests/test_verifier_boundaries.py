"""The Excel value dump opens its output only inside its Excel cleanup."""

import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]


class WorkbookFixtureTests(unittest.TestCase):
    def test_value_dump_opens_its_output_inside_the_excel_cleanup(self):
        text = (TOOLS / "dump_values.ps1").read_text(encoding="utf-8")
        # a bad -Out path thrown before the try would skip Quit() and leave Excel running
        cleanup = text.index("try {", text.index("New-Object -ComObject Excel.Application"))
        self.assertLess(cleanup, text.index("[System.IO.StreamWriter]::new("))


if __name__ == "__main__":
    unittest.main()
