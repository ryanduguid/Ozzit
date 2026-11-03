"""Every function help table carries the same WEBPAGE and VERSION rows.

A function copied into a model takes only its own help with it, so that help is
the one place a reader can see where the function came from and how old it is.
Up to v3.5.0, 17 help tables had no VERSION row, 4 had no link, and 10 labelled
the link WEBSITE where the rest say WEBPAGE. The module About tables are
a different layout, with their own REPOSITORY and WEBSITE rows, and are not read
here: they are assigned with TRIM(TEXTSPLIT(...)), not LAMBDA.

VERSION is the date the function's formula last changed. Up to v3.5.0 nothing
checked that, and 29 functions changed after the date their help gave: Amortiseλ
gained FinalPeriodEnd and still read 20 Aug 2026. The formula here is the LAMBDA
without its comments, whitespace or help text, so a help-only edit leaves VERSION
alone. The gate compares it with VERSION_BASE, which CI sets on a push to main to
the commit main pointed at before it, or else with HEAD's first parent, which in a
pull request is the base it merges into.
"""

from __future__ import annotations

import os
import re
import subprocess
import unittest
from datetime import date, datetime
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src"
REPOSITORY = "https://github.com/ryanduguid/Ozzit"

FUNCTION = re.compile(r"^([A-Za-z0-9_]*λ[A-Za-z0-9_]*)\s*=\s*LAMBDA\(", re.MULTILINE)
ROW = re.compile(r'^\s*"([A-Z][A-Z ]*):\s*→\s*([^"]*?)\s*¶?"', re.MULTILINE)
DATE = re.compile(r"[1-9]\d? (Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) 20\d\d")

# The diagnostic companions take the same arguments as the function they check and
# return a table of findings; they have no help of their own.
WITHOUT_HELP = {"AmortiseλDV", "CorkscrewλDV", "DepreciateλDV"}


def split(text: str) -> dict[str, str]:
    """Each LAMBDA's text in one module, from its assignment to the next one."""
    starts = [(m.start(), m.group(1)) for m in FUNCTION.finditer(text)] + [(len(text), "")]
    return {name: text[start:end] for (start, name), (end, _) in zip(starts, starts[1:])}


def functions() -> dict[str, str]:
    """Each LAMBDA's text in src/."""
    found: dict[str, str] = {}
    for path in sorted(SRC.glob("*.txt")):
        found.update(split(path.read_text(encoding="utf-8")))
    return found


def committed_functions(revision: str) -> dict[str, str] | None:
    """Each LAMBDA's text in src/ at a commit, or None when Git cannot show it."""
    found: dict[str, str] = {}
    try:
        names = subprocess.run(["git", "ls-tree", "--name-only", revision, "src/"], cwd=ROOT,
                               capture_output=True, text=True, check=True).stdout.split()
        for name in [n for n in names if n.endswith(".txt")]:
            shown = subprocess.run(["git", "show", f"{revision}:{name}"], cwd=ROOT,
                                   capture_output=True, check=True).stdout
            found.update(split(shown.decode("utf-8")))
    except (OSError, subprocess.CalledProcessError):
        return None
    return found


# A literal (group 1), a comment or a run of whitespace; only literals are kept.
TOKEN = re.compile(r'("(?:[^"]|"")*")|//[^\n]*|/\*.*?\*/|\s+', re.DOTALL)
# The help: the Help binding's TEXTSPLIT up to its "→" and "¶" delimiters.
HELP = re.compile(r'\bHelp,\s*TRIM\(\s*TEXTSPLIT\(.*?"→",\s*"¶"', re.DOTALL)


def formula(body: str) -> str:
    """The LAMBDA without comments, whitespace outside literals or help text.

    The help becomes one marker whatever its rows say; every other literal, message
    text and its spacing included, is part of the formula.
    """
    return TOKEN.sub(lambda m: m[1] or "", HELP.sub("Help,¶", body))


def version(body: str) -> date:
    return datetime.strptime(dict(ROW.findall(body))["VERSION"], "%d %b %Y").date()


def unmoved(current: dict[str, str], base: dict[str, str], today: date) -> list[str]:
    """Functions whose formula changed since base while VERSION stayed put.

    A function changed again on the day its VERSION was set keeps that date, so a
    VERSION within a day of today passes; the day allows for time zones.
    """
    return [
        name for name, body in current.items()
        if name in base and name not in WITHOUT_HELP and formula(body) != formula(base[name])
        and version(body) <= version(base[name]) and abs((version(body) - today).days) > 1
    ]


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

    def test_every_webpage_in_this_repository_is_a_file_on_main(self):
        # A WEBPAGE inside the repository names the page that explains the function; the
        # repository root is kept only where no page does. A renamed page breaks the link.
        on_main = REPOSITORY + "/blob/main/"
        for name, rows in self.rows.items():
            link = rows.get("WEBPAGE", "")
            if link == REPOSITORY or not link.startswith(REPOSITORY + "/"):
                continue
            with self.subTest(function=name):
                self.assertTrue(link.startswith(on_main), link)
                self.assertTrue((ROOT / link.removeprefix(on_main)).is_file(), link)

    def test_a_changed_formula_moves_its_version(self):
        named = os.environ.get("VERSION_BASE", "")
        base = committed_functions(named or "HEAD^1")
        if base is None and not named:  # pragma: no cover - CI checks out the full history
            self.skipTest("needs HEAD's first parent; a shallow clone or archive has none")
        self.assertTrue(base, f"no functions under src/ at {named or 'HEAD^1'}")
        self.assertEqual(unmoved(self.functions, base, date.today()), [],
                         "the formula changed: set VERSION to the date of the change")

    def test_the_version_gate_reads_formulas_not_help(self):
        def body(version: str, rows: str, code: str) -> str:
            return (f'Fλ = LAMBDA([X], LET(\n    Help, TRIM(TEXTSPLIT("FUNCTION:→Fλ(X)¶" &\n'
                    f'        "VERSION:   →{version}¶" &\n{rows}        "→", "¶")),\n'
                    f'    // the arithmetic\n    {code}));\n')

        base = {"Fλ": body("1 Sep 2026", "", "X + 1")}
        today = date(2026, 11, 3)
        cases = {
            "formula changed, VERSION kept": (body("1 Sep 2026", "", "X + 2"), ["Fλ"]),
            "formula changed, VERSION moved": (body("2 Nov 2026", "", "X + 2"), []),
            "formula changed on the VERSION's day": (body("4 Nov 2026", "", "X+2"), []),
            "help row added": (body("1 Sep 2026", '        "NOTE!→More.¶" &\n', "X + 1"), []),
            "comment and spacing": (body("1 Sep 2026", "", "X  +\n        1").replace(
                "// the arithmetic", "/* add one */"), []),
            "message text changed": (body("1 Sep 2026", "", 'IF(X, "Bad.", 1)'), ["Fλ"]),
        }
        for case, (current, expected) in cases.items():
            with self.subTest(case=case):
                self.assertEqual(unmoved({"Fλ": current}, base, today), expected)
        self.assertEqual(unmoved({"Fλ": base["Fλ"], "Gλ": base["Fλ"]}, base, today), [])
        self.assertIsNone(committed_functions("no-such-revision"))
        # Help separators in a message outside the help, and the spacing inside any
        # literal, are formula text.
        for before, after in [('"A→B¶"', '"A→C¶"'), ('"A B"', '"AB"')]:
            message = {"Fλ": body("1 Sep 2026", "", f"IF(X, {before}, 1)")}
            changed = {"Fλ": body("1 Sep 2026", "", f"IF(X, {after}, 1)")}
            self.assertEqual(unmoved(changed, message, today), ["Fλ"], after)

    def test_a_named_base_is_read_and_one_that_cannot_be_read_fails(self):
        with mock.patch.dict(os.environ, {"VERSION_BASE": "HEAD"}):
            self.test_a_changed_formula_moves_its_version()
        with mock.patch.dict(os.environ, {"VERSION_BASE": "no-such-revision"}):
            with self.assertRaisesRegex(AssertionError, "no functions under src/ at no-such-revision"):
                self.test_a_changed_formula_moves_its_version()


if __name__ == "__main__":
    unittest.main()
