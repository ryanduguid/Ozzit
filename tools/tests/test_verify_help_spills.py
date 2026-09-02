"""Contract for tools/verify_help_spills.py.

The tool models TRIM(TEXTSPLIT(literal, "→", "¶")) and compares the result with
the cells a help anchor spills into. It only reads: a stale help is reported for
tools/refresh_cache.py to refresh in Excel, never rewritten here. The model is
proven against the workbook itself: every help cache Excel wrote for a function
whose help has not changed must equal what the model computes, and the only
helps allowed to differ are the ones this release changed and Excel has not yet
refreshed.
"""

import re
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))

import verify_help_spills as spills  # noqa: E402

WORKBOOK = ROOT / "ozzit.xlsx"
# The helps this release changed. Until tools/refresh_cache.py has run in Excel their
# caches show the previous table; once it has, this set may be emptied.
CHANGED_HELPS = {
    "oz.Depreciateλ", "oz.IsOccurrenceDateλ", "oz.MinColsλ", "oz.Movementλ", "oz.OverLapDaysλ",
    "oz.Periodsλ", "oz.RollingSumλ", "oz.ScheduleRatesByItemsλ", "oz.ScheduleValuesλ",
    "oz.ScheduleValuesByItemsλ",
}

BOOK = (
    "<workbook><definedNames>"
    '<definedName name="oz.Fλ">_xlfn.LAMBDA(_xlop.x, _xlfn.LET(_xlpm.Help, TRIM(_xlfn.TEXTSPLIT('
    '"FUNCTION:      →Fλ(x)¶" &amp; "NOTE!  →say ""hi""¶" &amp; "→", "→", "¶")), _xlpm.x))</definedName>'
    '<definedName name="oz.Gλ">_xlfn.LAMBDA(_xlop.x, _xlfn.LET(_xlpm.Help, TRIM(_xlfn.TEXTSPLIT('
    '"FUNCTION:→Gλ(x)¶" &amp; "→", "→", "¶")), _xlpm.x))</definedName>'
    "</definedNames></workbook>"
)
CURRENT = (
    "<worksheet><sheetData>"
    '<row r="4"><c r="A4" t="str"><f t="array" ref="A4:B6">oz.Fλ()</f><v>FUNCTION:</v></c>'
    '<c r="B4" t="str"><v>Fλ(x)</v></c></row>'
    '<row r="5"><c r="A5" t="str"><v>NOTE!</v></c><c r="B5" t="str"><v>say "hi"</v></c></row>'
    '<row r="6"><c r="A6" t="str"><v/></c><c r="B6" t="str"><v/></c></row>'
    "</sheetData></worksheet>"
)
STALE = (
    "<worksheet><sheetData>"
    '<row r="4"><c r="A4" t="str"><f t="array" ref="A4:B5">oz.Gλ()</f><v>FUNCTION:</v></c>'
    '<c r="B4" t="str"><v>Gλ(y)</v></c></row>'
    '<row r="5"><c r="A5" t="str"><v/></c><c r="B5" t="str"><v/></c></row>'
    "</sheetData></worksheet>"
)
STALE_LINE = "xl/worksheets/sheet2.xml: the oz.Gλ help is stale, 2 cached rows against 2 in the definition"


def parts_for(*sheets: str) -> dict[str, bytes]:
    parts = {"xl/workbook.xml": BOOK.encode("utf-8")}
    for number, sheet in enumerate(sheets, start=1):
        parts[f"xl/worksheets/sheet{number}.xml"] = sheet.encode("utf-8")
    return parts


class HelpSpillCheckTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-help-spills-"))

    def tearDown(self):
        for path in self.directory.iterdir():
            path.unlink()
        self.directory.rmdir()

    def write(self, name: str, parts: dict[str, bytes]) -> Path:
        path = self.directory / name
        with zipfile.ZipFile(path, "w") as archive:
            for part, data in parts.items():
                archive.writestr(part, data)
        return path

    def test_trim_and_split_follow_excel(self):
        self.assertEqual(spills.trim("  a   b  "), "a b")
        self.assertEqual(
            spills.help_table(spills.defined_names(BOOK)["oz.Fλ"], "oz.Fλ"),
            [["FUNCTION:", "Fλ(x)"], ["NOTE!", 'say "hi"'], ["", ""]],
        )
        with self.assertRaisesRegex(ValueError, "would pad it with #N/A"):
            spills.help_table('TRIM(_xlfn.TEXTSPLIT("A→b→c", "→", "¶"))', "oz.Fλ")
        with self.assertRaisesRegex(ValueError, "has no TEXTSPLIT help"):
            spills.help_table("_xlfn.LAMBDA(_xlpm.x, _xlpm.x)", "oz.Fλ")

    def test_a_stale_help_is_reported_and_a_current_one_is_not(self):
        self.assertEqual(spills.check(parts_for(CURRENT)), ([], 1))
        self.assertEqual(spills.check(parts_for(CURRENT, STALE)), ([STALE_LINE], 2))
        # a spill that shrank or grew is stale too, whatever its cells hold
        grown = CURRENT.replace('ref="A4:B6"', 'ref="A4:B7"')
        self.assertEqual(spills.check(parts_for(grown))[0], [
            "xl/worksheets/sheet1.xml: the oz.Fλ help is stale, 4 cached rows against 3 in the definition"
        ])

    def test_an_anchor_without_a_definition_is_refused(self):
        with self.assertRaisesRegex(ValueError, "oz.Hλ is not a defined name"):
            spills.check(parts_for(CURRENT.replace("oz.Fλ()", "oz.Hλ()")))

    def test_cli_reports_stale_helps_and_never_writes(self):
        tool = TOOLS / "verify_help_spills.py"
        stale = self.write("stale.xlsx", parts_for(CURRENT, STALE))
        before = stale.read_bytes()
        result = subprocess.run([sys.executable, str(tool), str(stale)], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(STALE_LINE, result.stdout)
        self.assertIn("FAIL: 1 of 2 cached helps", result.stderr)
        self.assertIn("run tools/refresh_cache.py in Excel", result.stderr)
        self.assertEqual(stale.read_bytes(), before)

        current = self.write("current.xlsx", parts_for(CURRENT))
        result = subprocess.run([sys.executable, str(tool), str(current)], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("OK: every cached help", result.stdout)
        self.assertIn("1 anchors", result.stdout)

    def test_tracked_workbook_matches_the_model_except_the_helps_this_release_changed(self):
        stale, anchors = spills.run(WORKBOOK)
        self.assertGreaterEqual(anchors, 40)
        names = {re.search(r" the (oz\.\S+) help is stale", line).group(1) for line in stale}
        self.assertLessEqual(
            names, CHANGED_HELPS,
            "a help Excel cached for an unchanged function does not match the model: " + ", ".join(sorted(names)),
        )


if __name__ == "__main__":
    unittest.main()
