import csv
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
ROOT = TOOLS.parent
WORKBOOK = ROOT / "ozzit.xlsx"
INDEX = ROOT / "functions.csv"


class IndexAndInventoryTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-index-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _run(self, tool, *args):
        return subprocess.run(
            [sys.executable, str(TOOLS / tool), *map(str, args)],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=False,
        )

    def _mutated_index(self, field, value):
        target = self.directory / "functions.csv"
        with INDEX.open(encoding="utf-8-sig", newline="") as source:
            rows = list(csv.DictReader(source))
            fields = list(rows[0])
        rows[0][field] = value
        with target.open("w", encoding="utf-8", newline="") as output:
            writer = csv.DictWriter(output, fieldnames=fields, lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)
        return target

    def _duplicated_index(self, function, **overrides):
        """The index with a second, contradictory row for `function` prepended."""
        target = self.directory / "functions.csv"
        with INDEX.open(encoding="utf-8-sig", newline="") as source:
            rows = list(csv.DictReader(source))
            fields = list(rows[0])
        original = next(row for row in rows if row["function"] == function)
        rows.insert(0, {**original, **overrides})
        with target.open("w", encoding="utf-8", newline="") as output:
            writer = csv.DictWriter(output, fieldnames=fields, lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)
        return target

    def _column_index(self, columns, overrides=(), *, header_only=False, bom=False,
                      quoted=False):
        with INDEX.open(encoding="utf-8-sig", newline="") as source:
            fields, *rows = list(csv.reader(source))
        target = self.directory / "functions.csv"
        with target.open("w", encoding="utf-8-sig" if bom else "utf-8", newline="") as output:
            writer = csv.writer(output, lineterminator="\n",
                                quoting=csv.QUOTE_ALL if quoted else csv.QUOTE_MINIMAL)
            writer.writerow(columns)
            for row in (() if header_only else rows):
                values = [row[fields.index(field)] if field in fields else "extra,\nvalue"
                          for field in columns]
                for position, value in overrides:
                    values[position] = value
                writer.writerow(values)
        return target

    def test_index_gates_reject_repeated_consumed_columns(self):
        fields = ["function", "module", "previous_name", "signature", "description"]
        for tool, consumed in (
            ("verify_index.py", ("function", "module", "signature", "description")),
            ("verify_previous_names.py", ("function", "previous_name", "signature", "description")),
        ):
            for field in consumed:
                for identical in (False, True):
                    with self.subTest(tool=tool, field=field, identical=identical):
                        index = self._column_index([field, *fields],
                                                  () if identical else ((0, ""),))
                        before = index.read_bytes()
                        args = (WORKBOOK, ROOT / "src", index) if tool == "verify_index.py" else (index,)
                        result = self._run(tool, *args)
                        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                        diagnostic = result.stdout + result.stderr
                        self.assertIn("duplicate CSV column", diagnostic)
                        self.assertIn(field, diagnostic)
                        self.assertIn("functions.csv", diagnostic)
                        self.assertNotIn("Traceback", diagnostic)
                        self.assertNotIn("OK:", result.stdout)
                        self.assertEqual(index.read_bytes(), before)

    def test_index_gates_check_parsed_headers_before_mapping_any_rows(self):
        fields = ["function", "module", "previous_name", "signature", "description"]
        cases = (
            (["function", *fields], (), False, False, False),
            ([*fields, "function"], ((5, ""),), False, False, False),
            (["function", "function", *fields], (), False, False, False),
            (["signature", "function", *fields], (), False, False, False),
            ([*fields, "function"], (), True, False, False),
            (["function", *fields], (), False, True, True),
        )
        for tool in ("verify_index.py", "verify_previous_names.py"):
            for number, (columns, overrides, empty, bom, quoted) in enumerate(cases):
                with self.subTest(tool=tool, case=number):
                    index = self._column_index(columns, overrides, header_only=empty,
                                               bom=bom, quoted=quoted)
                    args = (WORKBOOK, ROOT / "src", index) if tool == "verify_index.py" else (index,)
                    result = self._run(tool, *args)
                    self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                    diagnostic = result.stdout + result.stderr
                    self.assertIn("duplicate CSV column", diagnostic)
                    self.assertIn("function", diagnostic)
                    if number == 3:
                        self.assertIn("signature", diagnostic)
                    self.assertNotIn("Traceback", diagnostic)

    def test_index_gates_preserve_ignored_columns_and_csv_parsing(self):
        fields = ["function", "module", "previous_name", "signature", "description"]
        for tool, ignored in (("verify_index.py", "previous_name"),
                              ("verify_previous_names.py", "module")):
            cases = (
                ([ignored, *fields], ((0, "ignored contradiction"),), False),
                (["extra", *fields, "extra"], ((0, "ignored contradiction"),), False),
                ([field for field in fields if field != ignored], (), False),
                ([*reversed(fields), "Function", "extra"], (), False),
                ([*reversed(fields), "Function", "extra"], (), True),
            )
            for number, (columns, overrides, bom) in enumerate(cases):
                with self.subTest(tool=tool, case=number):
                    index = self._column_index(columns, overrides, bom=bom)
                    args = (WORKBOOK, ROOT / "src", index) if tool == "verify_index.py" else (index,)
                    result = self._run(tool, *args)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertIn("OK:", result.stdout)

    def test_index_gates_do_not_treat_uppercase_function_as_the_required_field(self):
        index = self._column_index(["Function", "module", "previous_name", "signature", "description"])
        for tool, args in (("verify_index.py", (WORKBOOK, ROOT / "src", index)),
                           ("verify_previous_names.py", (index,))):
            with self.subTest(tool=tool):
                result = self._run(tool, *args)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertNotIn("duplicate CSV column", result.stdout + result.stderr)
                self.assertNotIn("Traceback", result.stdout + result.stderr)

    def test_index_gate_rejects_a_duplicate_function_row(self):
        # The valid row stays later in the file, so keying the rows by name hid the
        # contradictory copy and every field check passed on the survivor.
        index = self._duplicated_index(
            "oz.AnnualRateλ", module="FabricatedModule", description="Fabricated"
        )
        result = self._run("verify_index.py", WORKBOOK, ROOT / "src", index)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("oz.AnnualRateλ appears 2 times", result.stdout)

    def test_previous_name_gate_rejects_a_duplicate_function_row(self):
        # oz.AnnualRateλ postdates the naming baseline, so its previous_name is blank
        # and the claim checks skipped both copies.
        index = self._duplicated_index("oz.AnnualRateλ", description="Fabricated")
        result = self._run("verify_previous_names.py", index)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("oz.AnnualRateλ appears 2 times", result.stdout)

    def test_previous_name_gate_reports_truncated_rows(self):
        with INDEX.open(encoding="utf-8-sig", newline="") as source:
            rows = list(csv.reader(source))
        index = self.directory / "functions.csv"
        for columns in (1, 3, 4):
            with self.subTest(columns=columns):
                with index.open("w", encoding="utf-8", newline="") as output:
                    writer = csv.writer(output, lineterminator="\n")
                    writer.writerow(rows[0])
                    writer.writerow(rows[1][:columns])
                    writer.writerows(rows[2:])
                result = self._run("verify_previous_names.py", index)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stderr, "")
                field = "description" if columns == 4 else "signature"
                self.assertIn(f"publishes an empty {field}", result.stdout)

    def test_index_gate_reports_a_source_module_that_is_not_utf8(self):
        src = self.directory / "src"
        src.mkdir()
        source = src / "Invalid.txt"
        source.write_bytes(b"\xff")

        result = self._run("verify_index.py", WORKBOOK, src, INDEX)

        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stderr, "")
        self.assertTrue(result.stdout.startswith("FAIL: cannot read index inputs:"))
        self.assertEqual(source.read_bytes(), b"\xff")

    def test_index_gate_reports_a_source_entry_that_is_a_directory(self):
        src = self.directory / "src"
        src.mkdir()
        source = src / "Invalid.txt"
        source.mkdir()

        result = self._run("verify_index.py", WORKBOOK, src, INDEX)

        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stderr, "")
        self.assertTrue(result.stdout.startswith("FAIL: cannot read index inputs:"))
        self.assertTrue(source.is_dir())

    def test_both_index_gates_accept_the_published_index(self):
        for tool, args in (
            ("verify_index.py", (WORKBOOK, ROOT / "src", INDEX)),
            ("verify_previous_names.py", (INDEX,)),
        ):
            with self.subTest(tool=tool):
                result = self._run(tool, *args)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_previous_name_gate_accepts_a_missing_previous_name_for_a_new_function(self):
        with INDEX.open(encoding="utf-8-sig", newline="") as source:
            rows = list(csv.DictReader(source))
            fields = [field for field in rows[0] if field != "previous_name"]
        fields.append("previous_name")
        index = self.directory / "functions.csv"
        with index.open("w", encoding="utf-8", newline="") as output:
            writer = csv.writer(output, lineterminator="\n")
            writer.writerow(fields)
            for row in rows:
                values = [row[field] for field in fields]
                if row["function"] == "oz.AnnualRateλ":
                    values.pop()
                writer.writerow(values)
        result = self._run("verify_previous_names.py", index)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_index_gate_rejects_fabricated_description(self):
        index = self._mutated_index("description", "Fabricated description")
        result = self._run("verify_index.py", WORKBOOK, ROOT / "src", index)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("description differs from workbook", result.stdout)

    def test_index_gate_reports_a_missing_function_in_a_truncated_row(self):
        with INDEX.open(encoding="utf-8-sig", newline="") as source:
            rows = list(csv.DictReader(source))
            fields = [field for field in rows[0] if field != "function"] + ["function"]
        index = self.directory / "functions.csv"
        with index.open("w", encoding="utf-8", newline="") as output:
            writer = csv.writer(output, lineterminator="\n")
            writer.writerow(fields)
            for number, row in enumerate(rows):
                values = [row[field] for field in fields]
                if number == 1:
                    values[-1] = ""
                writer.writerow(values[:-1] if number == 0 else values)
        result = self._run("verify_index.py", WORKBOOK, ROOT / "src", index)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stderr, "")
        self.assertIn(f"{rows[0]['function']} is in the workbook but missing from functions.csv", result.stdout)
        self.assertIn(f"{rows[1]['function']} is in the workbook but missing from functions.csv", result.stdout)

    def test_index_gate_rejects_fabricated_module(self):
        index = self._mutated_index("module", "FabricatedModule")
        result = self._run("verify_index.py", WORKBOOK, ROOT / "src", index)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("module differs from src/", result.stdout)

    def assert_forbidden_part_rejected(self, part):
        target = self.directory / "forbidden.xlsx"
        with zipfile.ZipFile(WORKBOOK) as source, zipfile.ZipFile(
            target, "w", zipfile.ZIP_DEFLATED
        ) as output:
            for info in source.infolist():
                output.writestr(info, source.read(info.filename))
            output.writestr(part, b"forbidden executable or linked content")
        result = self._run("verify_workbook.py", target)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(f"forbidden workbook part {part}", result.stdout)

    def test_workbook_gate_rejects_executable_and_external_parts(self):
        for part in (
            "xl/vbaProject.bin",
            "xl/activeX/activeX1.xml",
            "xl/ctrlProps/ctrlProp1.xml",
            "xl/externalLinks/externalLink1.xml",
            "xl/embeddings/oleObject1.bin",
            "xl/macrosheets/sheet1.xml",
            "xl/dialogsheets/sheet1.xml",
        ):
            with self.subTest(part=part):
                self.assert_forbidden_part_rejected(part)


if __name__ == "__main__":
    unittest.main()
