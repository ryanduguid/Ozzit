"""Contract for tools/postbuild/refresh_help_spills.py.

The pass models TRIM(TEXTSPLIT(literal, "→", "¶")) and writes the result over the
cells a help anchor spills into. Its model is proven against the workbook itself:
every help cache Excel wrote for a function whose help has not changed must equal
what the model computes. The rest of the contract is that a changed help is
rewritten, a grown spill gets its rows, and a help that would spill #N/A is refused.
"""

import re
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import refresh_help_spills as spills  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402

WORKBOOK = ROOT / "ozzit.xlsx"


def parts_of(path: Path) -> dict[str, bytes]:
    with zipfile.ZipFile(path) as archive:
        return {name: archive.read(name) for name in archive.namelist()}


class HelpSpillTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-spills-"))
        self.workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, self.workbook)

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def test_every_cached_help_equals_the_model(self):
        parts = parts_of(WORKBOOK)
        names = spills.defined_names(parts["xl/workbook.xml"].decode("utf-8"))
        anchors = 0
        for part in parts:
            if not re.fullmatch(r"xl/worksheets/sheet\d+\.xml", part):
                continue
            sheet = parts[part].decode("utf-8")
            for match in spills.ANCHOR.finditer(sheet):
                anchors += 1
                left, top = spills.column_number(match.group(1)), int(match.group(2))
                last = match.group(4).split(":")[1]
                rows = int(re.sub(r"[A-Z]", "", last)) - top + 1
                cols = spills.column_number(re.sub(r"\d", "", last)) - left + 1
                self.assertEqual(
                    spills.cached_table(sheet, top, left, rows, cols),
                    spills.help_table(names[match.group(6)], match.group(6)),
                    f"{part} caches a help that {match.group(6)} no longer spills",
                )
        self.assertGreaterEqual(anchors, 40)
        self.assertEqual(spills.run(self.workbook), [])

    def test_trim_and_split_follow_excel(self):
        self.assertEqual(spills.trim("  a   b  "), "a b")
        stored = (
            '_xlfn.LAMBDA(_xlop.x, _xlfn.LET(_xlpm.Help, TRIM(_xlfn.TEXTSPLIT('
            '"FUNCTION:      →Fλ(x)¶" & "NOTE!  →say ""hi""¶" & "→", "→", "¶")), _xlpm.x))'
        )
        self.assertEqual(
            spills.help_table(stored, "oz.Fλ"),
            [["FUNCTION:", "Fλ(x)"], ["NOTE!", 'say "hi"'], ["", ""]],
        )
        with self.assertRaisesRegex(ValueError, "would pad it with #N/A"):
            spills.help_table('TRIM(_xlfn.TEXTSPLIT("A→b→c", "→", "¶"))', "oz.Fλ")

    def test_a_changed_help_is_rewritten_and_the_spill_grows(self):
        parts = parts_of(self.workbook)
        book = parts["xl/workbook.xml"].decode("utf-8")
        old = '"Starts         →(Required) One of more start dates (text or numeric) in a row or column¶"'
        self.assertEqual(book.count(old), 1)
        book = book.replace(old, '"Starts         →(Required) Start dates¶" &amp; "NOTE!          →One more row¶"')
        parts["xl/workbook.xml"] = book.encode("utf-8")
        write_deterministic(self.workbook, parts)

        log = spills.run(self.workbook)
        self.assertEqual(log, ["xl/worksheets/sheet6.xml: refreshed the oz.CountDOWλ help, 11 rows"])
        sheet = parts_of(self.workbook)["xl/worksheets/sheet6.xml"].decode("utf-8")
        self.assertIn('<f t="array" ref="A4:B14">oz.CountDOWλ()</f><v>FUNCTION:</v>', sheet)
        self.assertIn('<c r="B8" t="str"><v>(Required) Start dates</v></c>', sheet)
        self.assertIn('<c r="A9" t="str"><v>NOTE!</v></c>', sheet)
        # TRIM collapses the double space the literal carries before the last argument
        self.assertIn('<c r="B14" t="str"><v>=oz.CountDOWλ("23/7/2026", "11/8/2026", 1)</v></c>', sheet)
        # the row the spill grew into is created after the row before it
        self.assertLess(sheet.index('<row r="13"'), sheet.index('<row r="14"'))
        self.assertEqual(spills.run(self.workbook), [])


if __name__ == "__main__":
    unittest.main()
