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

    def _run(self, *args, cwd=None):
        return subprocess.run(
            [sys.executable, str(TOOLS / "build_module.py"), *map(str, args)],
            cwd=cwd,
            capture_output=True,
            text=True,
            check=False,
        )

    def _column_index(self, columns, overrides=(), *, header_only=False, bom=False):
        with INDEX.open(encoding="utf-8-sig", newline="") as source:
            fields, *rows = list(csv.reader(source))
        target = self.directory / "functions.csv"
        with target.open("w", encoding="utf-8-sig" if bom else "utf-8", newline="") as output:
            writer = csv.writer(output, lineterminator="\n")
            writer.writerow(columns)
            for row in (() if header_only else rows):
                values = [row[fields.index(field)] if field in fields else "extra,\nvalue"
                          for field in columns]
                for position, value in overrides:
                    values[position] = value
                writer.writerow(values)
        return target

    def test_repeated_function_columns_fail_before_destination_handling(self):
        for identical in (False, True):
            for checking in (False, True):
                for destination in ("absent", "current", "stale"):
                    with self.subTest(identical=identical, checking=checking,
                                      destination=destination):
                        index = self._column_index(["function", "function"],
                                                  () if identical else ((0, ""),))
                        before = index.read_bytes()
                        output = self.directory / f"{identical}-{checking}-{destination}.txt"
                        original = (MODULE.read_bytes() if destination == "current"
                                    else b"/* keep temporary output */\n")
                        if destination != "absent":
                            output.write_bytes(original)
                        result = self._run(SRC, output, WORKBOOK, index,
                                           *(["--check"] if checking else []))
                        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                        self.assertIn("duplicate CSV column", result.stdout)
                        self.assertIn("function", result.stdout)
                        self.assertIn("functions.csv", result.stdout)
                        self.assertNotIn("Traceback", result.stderr)
                        self.assertNotIn("wrote ", result.stdout)
                        self.assertEqual(index.read_bytes(), before)
                        if destination == "absent":
                            self.assertFalse(output.exists())
                        else:
                            self.assertEqual(output.read_bytes(), original)

    def test_direct_check_reports_header_failures_and_retains_text_findings(self):
        text = build(SRC)
        for columns, overrides, empty in (
            (["function", "function"], (), False),
            (["function", "module", "function"], ((2, ""),), False),
            (["function", "function", "function"], (), False),
            (["function", "function"], (), True),
        ):
            with self.subTest(columns=columns, empty=empty):
                index = self._column_index(columns, overrides, header_only=empty)
                findings = check(text + "BROKEN;\n", WORKBOOK, index)
                self.assertTrue(any("duplicate CSV column" in finding for finding in findings),
                                findings)
                self.assertTrue(any("not a declaration" in finding and "BROKEN" in finding
                                    for finding in findings), findings)

    def test_ignored_columns_and_minimal_reordered_indexes_remain_usable(self):
        text = build(SRC)
        for columns, overrides, bom in (
            (["module", "function"], (), False),
            (["module", "function"], (), True),
            (["description", "function", "description"], ((0, "ignored contradiction"),), False),
            (["extra", "function", "extra"], ((0, "ignored contradiction"),), False),
            (["Function", "extra", "function"], (), True),
        ):
            with self.subTest(columns=columns, bom=bom):
                index = self._column_index(columns, overrides, bom=bom)
                self.assertEqual(check(text, WORKBOOK, index), [])

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
        self.assertEqual(stale.read_bytes(), b"/* stale */\n")
        result = self._run(SRC, stale, WORKBOOK, INDEX)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(stale.read_bytes(), MODULE.read_bytes())
        result = self._run(SRC, stale, WORKBOOK, INDEX, "--check")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("OK", result.stdout)

    def test_cli_rejects_unknown_options_and_extra_paths_without_writing(self):
        output = self.directory / "oz.txt"
        sentinel = b"/* keep this output */\n"
        for extra in ("--chek", "--chec", "--unknown", "extra.txt"):
            with self.subTest(argument=extra):
                output.write_bytes(sentinel)
                result = self._run(SRC, output, WORKBOOK, INDEX, extra)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("unrecognized arguments", result.stderr)
                self.assertEqual(output.read_bytes(), sentinel)

    def test_cli_rejects_invalid_index_names_without_writing(self):
        with INDEX.open(encoding="utf-8-sig", newline="") as handle:
            names = [row["function"] for row in csv.DictReader(handle)]
        index = self.directory / "functions.csv"
        output = self.directory / "oz.txt"
        sentinel = b"/* keep this output */\n"
        for name in (None, "", " \t", "xx." + names[0][3:], "ozx" + names[0][3:]):
            for checking in (False, True):
                with self.subTest(name=name, checking=checking):
                    with index.open("w", encoding="utf-8-sig", newline="") as handle:
                        writer = csv.writer(handle)
                        writer.writerow(["module", "function"])
                        writer.writerow(["fixture"] if name is None else ["fixture", name])
                        writer.writerows(["fixture", value] for value in names[1:])
                    output.write_bytes(MODULE.read_bytes() if checking else sentinel)
                    before = output.read_bytes()
                    result = self._run(SRC, output, WORKBOOK, index,
                                       *(["--check"] if checking else []))
                    self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                    self.assertIn("FAIL:", result.stdout)
                    self.assertIn("functions.csv", result.stdout)
                    self.assertNotIn("Traceback", result.stderr)
                    self.assertEqual(output.read_bytes(), before)

    def test_cli_rejects_an_index_without_function_column(self):
        index = self.directory / "functions.csv"
        index.write_text("module\nfixture\n", encoding="utf-8")
        output = self.directory / "oz.txt"
        sentinel = b"/* keep this output */\n"
        output.write_bytes(sentinel)
        result = self._run(SRC, output, WORKBOOK, index)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("FAIL:", result.stdout)
        self.assertIn("function", result.stdout)
        self.assertNotIn("Traceback", result.stderr)
        self.assertEqual(output.read_bytes(), sentinel)

    def test_cli_help_does_not_read_inputs_or_write_output(self):
        output = self.directory / "oz.txt"
        sentinel = b"/* keep this output */\n"
        output.write_bytes(sentinel)
        result = self._run(self.directory / "missing-src", output,
                           self.directory / "missing.xlsx", self.directory / "missing.csv", "--help")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        for argument in ("src", "output", "workbook", "index", "--check"):
            self.assertIn(argument, result.stdout)
        self.assertEqual(output.read_bytes(), sentinel)

    def test_cli_check_preserves_optional_path_defaults(self):
        shutil.copytree(SRC, self.directory / "src")
        for path in (MODULE, WORKBOOK, INDEX):
            shutil.copyfile(path, self.directory / path.name)
        paths = ("src", "oz.txt", "ozzit.xlsx", "functions.csv")
        for count in range(5):
            with self.subTest(path_count=count):
                result = self._run(*paths[:count], "--check", cwd=self.directory)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn("OK", result.stdout)
                self.assertEqual((self.directory / "oz.txt").read_bytes(), MODULE.read_bytes())

    def test_cli_check_accepts_options_between_paths_without_writing(self):
        output = self.directory / "oz.txt"
        sentinel = b"/* keep this output */\n"
        output.write_bytes(sentinel)
        paths = [SRC, output, WORKBOOK, INDEX]
        for position in range(len(paths) + 1):
            with self.subTest(option_position=position):
                result = self._run(*paths[:position], "--check", *paths[position:])
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn("stale", result.stdout)
                self.assertEqual(output.read_bytes(), sentinel)


if __name__ == "__main__":
    unittest.main()
