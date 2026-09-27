"""Row-by-row defaults in GSTAddλ, GSTExtractλ and Movementλ, and the Life guards.

OR() reduced a column of rates or openings to one TRUE, so a single blank gave every
row the default: zero-rated GST lines were taxed and every opening balance read as 0.
A negative life wrote the whole cost off in one period and a zero life divided by zero.
The native self-test runs these cases in Excel; this checks that the source and the
names the workbook ships both carry the fix.
"""

import html
import re
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PREFIXES = re.compile(r"_xl(?:fn|pm|op)\.")

EXPECTED = {
    "GSTAddλ": ['IF(ISOMITTED(Rate), 0.1, IF(TRIM(Rate & "")="", 0.1, Rate))'],
    "GSTExtractλ": ['IF(ISOMITTED(Rate), 0.1, IF(TRIM(Rate & "")="", 0.1, Rate))'],
    "Movementλ": ['IF(ISOMITTED(BeginningValues), 0,', 'IF(TRIM(BeginningValues & "")="", 0, BeginningValues))'],
    "DiminishingValueλ": ["AND(ISNUMBER(Life), Life > 0)", '"DiminishingValueλ needs a Life greater than 0"'],
    "PrimeCostλ": ["AND(ISNUMBER(Life), Life > 0)", '"PrimeCostλ needs a Life greater than 0"'],
}


def flat(text: str) -> str:
    return re.sub(r"\s+", " ", text)


class RowDefaultsAndLifeGuardTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = (ROOT / "src" / "Financial.txt").read_text(encoding="utf-8")
        with zipfile.ZipFile(ROOT / "ozzit.xlsx") as archive:
            cls.book = archive.read("xl/workbook.xml").decode("utf-8")

    def definition(self, name: str) -> str:
        start = self.source.index(f"\n{name} = LAMBDA(")
        end = self.source.index("\n);", start)
        return flat(self.source[start:end])

    def shipped(self, name: str) -> str:
        match = re.search(rf'<definedName name="oz\.{re.escape(name)}"[^>]*>(.*?)</definedName>',
                          self.book, re.DOTALL)
        self.assertIsNotNone(match, name)
        assert match is not None
        return flat(PREFIXES.sub("", html.unescape(match.group(1))))

    def test_the_source_and_the_shipped_names_carry_the_fix(self):
        for name, fragments in EXPECTED.items():
            for store, text in (("src", self.definition(name)), ("definedName", self.shipped(name))):
                for fragment in fragments:
                    with self.subTest(name=name, store=store, fragment=fragment):
                        self.assertIn(flat(fragment), text)

    def test_no_default_is_collapsed_by_or(self):
        for name in ("GSTAddλ", "GSTExtractλ", "Movementλ"):
            with self.subTest(name=name):
                self.assertNotRegex(self.definition(name), r"OR\(ISOMITTED\((Rate|BeginningValues)\)")
                self.assertNotRegex(self.shipped(name), r"OR\(ISOMITTED\((Rate|BeginningValues)\)")


if __name__ == "__main__":
    unittest.main(verbosity=2)
