import csv
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
ROOT = TOOLS.parent
WORKBOOK = ROOT / "ozzit.xlsx"
INDEX = ROOT / "functions.csv"
SRC = ROOT / "src"
MODULE = ROOT / "oz.txt"

from build_module import build, check, declared_names  # noqa: E402
from verify_afe import workbook_names  # noqa: E402


class BuildModuleTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-module-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _run(self, *args):
        return subprocess.run(
            [sys.executable, str(TOOLS / "build_module.py"), *map(str, args)],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_tracked_module_is_the_current_join_of_src(self):
        # The tracked file is a publication view of src/, like functions.csv: a src/
        # edit that skipped the rebuild would ship a module that no longer matches
        # the workbook the AFE store is held to.
        self.assertEqual(MODULE.read_bytes(), build(SRC).encode("utf-8"))
        self.assertEqual(check(MODULE.read_text(encoding="utf-8"), WORKBOOK, INDEX), [])

    def test_join_declares_every_shipped_name_once_and_nothing_else(self):
        text = MODULE.read_text(encoding="utf-8")
        declared = declared_names(text)
        with INDEX.open(encoding="utf-8-sig", newline="") as handle:
            indexed = [row["function"] for row in csv.DictReader(handle)]
        self.assertEqual(len(declared), len(set(declared)))
        self.assertEqual({f"oz.{name}" for name in declared}, set(indexed))
        self.assertEqual({f"oz.{name}" for name in declared}, workbook_names(WORKBOOK))
        # The five help tables are named formulas, not LAMBDAs; a LAMBDA-only
        # inventory would miss them.
        self.assertEqual(sorted(name for name in declared if name.startswith("About")),
                         ["AboutDatesλ", "AboutEssentialsλ", "AboutFinancialλ",
                          "AboutRatiosλ", "AboutUtilitiesλ"])
        self.assertGreaterEqual(len(declared), 130)

    def test_join_is_lf_only_and_ends_with_a_newline(self):
        raw = MODULE.read_bytes()
        self.assertNotIn(b"\r", raw)
        self.assertTrue(raw.endswith(b"\n"))
        self.assertNotIn(b"oz.oz.", raw)
        self.assertTrue(raw.startswith(b"/*  Ozzit:"))

    def test_check_is_not_vacuous(self):
        # Positive controls: a duplicated declaration, a case-folded duplicate, a
        # missing declaration and a carriage return each produce a finding.
        text = build(SRC)
        first = next(statement for statement in text.split(";\n") if "= LAMBDA(" in statement)
        duplicated = text.replace(first + ";\n", first + ";\n" + first + ";\n", 1)
        self.assertTrue(any("more than once" in f for f in check(duplicated, WORKBOOK, INDEX)))
        name = declared_names(text)[-1]
        folded = text.replace(f"\n{name} =", f"\n{name.upper()} =", 1)
        findings = check(folded, WORKBOOK, INDEX)
        self.assertTrue(any(name in f and "does not declare" in f for f in findings), findings)
        self.assertTrue(any("\r" in f or "carriage return" in f
                            for f in check(text.replace("\n", "\r\n", 1), WORKBOOK, INDEX)))
        # A stray top-level statement that is not a declaration must fail the
        # check even though the name inventory is untouched.
        stray = check(text + "BROKEN;\n", WORKBOOK, INDEX)
        self.assertTrue(any("not a declaration" in f and "BROKEN" in f for f in stray), stray)

    def test_cli_check_reports_a_stale_tracked_file(self):
        stale = self.directory / "oz.txt"
        stale.write_bytes(b"/* stale */\n")
        result = self._run(SRC, stale, WORKBOOK, INDEX, "--check")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("stale", result.stdout)
        result = self._run(SRC, stale, WORKBOOK, INDEX)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(stale.read_bytes(), MODULE.read_bytes())
        result = self._run(SRC, stale, WORKBOOK, INDEX, "--check")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("OK", result.stdout)


if __name__ == "__main__":
    unittest.main()
