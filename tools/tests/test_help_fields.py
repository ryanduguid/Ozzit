"""Every function help table carries the same WEBPAGE and VERSION rows.

A function copied into a model takes only its own help with it, so that help is
the one place a reader can see where the function came from and how old it is.
Up to v3.5.0, 17 help tables had no VERSION row, 4 had no link, and 10 labelled
the link WEBSITE where the rest say WEBPAGE. The module About tables are
a different layout, with their own REPOSITORY and WEBSITE rows, and are not read
here: they are assigned with TRIM(TEXTSPLIT(...)), not LAMBDA.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src"

FUNCTION = re.compile(r"^([A-Za-z0-9_]*λ[A-Za-z0-9_]*)\s*=\s*LAMBDA\(", re.MULTILINE)
ROW = re.compile(r'^\s*"([A-Z][A-Z ]*):\s*→\s*([^"]*?)\s*¶?"', re.MULTILINE)
DATE = re.compile(r"[1-9]\d? (Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) 20\d\d")

# The diagnostic companions take the same arguments as the function they check and
# return a table of findings; they have no help of their own.
WITHOUT_HELP = {"AmortiseλDV", "CorkscrewλDV", "DepreciateλDV"}


def functions() -> dict[str, str]:
    """Each LAMBDA's text, from its assignment to the next one in the module."""
    found: dict[str, str] = {}
    for path in sorted(SRC.glob("*.txt")):
        text = path.read_text(encoding="utf-8")
        starts = [(m.start(), m.group(1)) for m in FUNCTION.finditer(text)]
        for index, (start, name) in enumerate(starts):
            end = starts[index + 1][0] if index + 1 < len(starts) else len(text)
            found[name] = text[start:end]
    return found


class HelpFieldTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.functions = functions()
        cls.rows = {
            name: {label: value for label, value in ROW.findall(body)}
            for name, body in cls.functions.items()
        }

    def test_the_library_was_read(self):
        self.assertGreaterEqual(len(self.functions), 140)

    def test_only_the_diagnostic_companions_have_no_help(self):
        without = {name for name, rows in self.rows.items() if "FUNCTION" not in rows}
        self.assertEqual(without, WITHOUT_HELP)

    def test_every_help_names_its_webpage_and_version(self):
        for name, rows in self.rows.items():
            if name in WITHOUT_HELP:
                continue
            with self.subTest(function=name):
                self.assertNotIn("WEBSITE", rows, "label the link WEBPAGE, as every other help does")
                self.assertTrue(rows.get("WEBPAGE", "").startswith("https://"), rows.get("WEBPAGE"))
                self.assertRegex(rows.get("VERSION", ""), rf"^{DATE.pattern}$")


if __name__ == "__main__":
    unittest.main()
