"""Fixture tests for workbook readers that sit at untyped file boundaries."""

import base64
import csv
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import verify_afe  # noqa: E402
import verify_help_spills  # noqa: E402


class WorkbookFixtureTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-boundary-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def write_archive(self, name, parts):
        target = self.directory / name
        with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
            for part, data in parts.items():
                archive.writestr(part, data)
        return target

    def write_afe_store(self, value):
        encoded = base64.b64encode(
            json.dumps(value, ensure_ascii=False).encode("utf-16-le")
        )
        return self.write_archive(
            "afe.xlsx",
            {"customXml/item1.xml": b"<store>" + encoded + b"</store>"},
        )


    def test_decode_store_accepts_a_typed_store_fixture(self):
        value = {
            "schema": verify_afe.SCHEMA,
            "files": [{"path": "/projects/Dates", "text": "source"}],
            "projectNames": ["oz.CountDOWλ"],
        }
        workbook = self.write_afe_store(value)

        self.assertEqual(verify_afe.decode_store(workbook), value)

    def test_decode_store_rejects_a_non_object_root(self):
        workbook = self.write_afe_store(["not", "an", "object", "x" * 80])

        with self.assertRaisesRegex(ValueError, "root must be an object"):
            verify_afe.decode_store(workbook)

    def test_decode_store_rejects_malformed_file_records(self):
        value = {
            "schema": verify_afe.SCHEMA,
            "files": [{"path": 7, "text": "source"}],
            "projectNames": [],
            "padding": "x" * 80,
        }
        workbook = self.write_afe_store(value)

        with self.assertRaisesRegex(ValueError, r"files\[0\].*strings"):
            verify_afe.decode_store(workbook)

    def test_decode_store_rejects_a_non_list_project_name_collection(self):
        value = {
            "schema": verify_afe.SCHEMA,
            "files": [],
            "projectNames": "oz.CountDOWλ",
            "padding": "x" * 80,
        }
        workbook = self.write_afe_store(value)

        with self.assertRaisesRegex(ValueError, "projectNames must be a list of strings"):
            verify_afe.decode_store(workbook)

    def test_workbook_names_decodes_entities_and_keeps_only_project_names(self):
        workbook = self.write_archive(
            "names.xlsx",
            {
                "xl/workbook.xml": (
                    '<workbook><definedNames><definedName name="oz.A&amp;B">1</definedName>'
                    '<definedName name="Print_Area">2</definedName></definedNames></workbook>'
                )
            },
        )

        self.assertEqual(verify_afe.workbook_names(workbook), {"oz.A&B"})

    def test_afe_check_reports_a_workbook_part_it_cannot_read(self):
        store = self.write_afe_store(
            {"schema": verify_afe.SCHEMA, "files": [], "projectNames": [], "padding": "x" * 80}
        )
        undecodable = self.write_archive("undecodable.xlsx", {"xl/workbook.xml": b"<workbook>\xff</workbook>"})
        for label, workbook in (("missing part", store), ("not UTF-8", undecodable)):
            with self.subTest(label=label):
                with self.assertRaisesRegex(ValueError, "cannot read workbook names"):
                    verify_afe.workbook_names(workbook)

        self.assertTrue(verify_afe.check(store, self.directory)[-1].startswith("cannot read workbook names"))

    def test_afe_workbook_readers_reject_a_directory(self):
        workbook = self.directory / "directory.xlsx"
        workbook.mkdir()
        for reader, message in ((verify_afe.decode_store, "cannot read AFE store"),
                                (verify_afe.workbook_names, "cannot read workbook names")):
            with self.subTest(reader=reader.__name__):
                with self.assertRaisesRegex(ValueError, message):
                    reader(workbook)
        self.assertTrue(verify_afe.check(workbook, self.directory)[0].startswith("cannot read AFE store"))
        result = self.run_tool(TOOLS / "verify_afe.py", workbook, self.directory)
        self.assertEqual(result.returncode, 1)
        self.assertIn("FAIL: 1 AFE problem(s)", result.stdout)
        self.assertEqual(result.stderr, "")

    def test_afe_reports_unreadable_source_modules(self):
        for label in ("not-utf8", "directory"):
            with self.subTest(source=label):
                src = self.directory / label
                shutil.copytree(TOOLS.parent / "src", src)
                source = src / "Dates.txt"
                if label == "not-utf8":
                    source.write_bytes(b"\xff")
                else:
                    source.unlink()
                    source.mkdir()
                failures = verify_afe.check(TOOLS.parent / "ozzit.xlsx", src)
                self.assertEqual(len(failures), 1)
                self.assertIn(f"cannot read source module {source}", failures[0])
                result = self.run_tool(TOOLS / "verify_afe.py", TOOLS.parent / "ozzit.xlsx", src)
                self.assertEqual(result.returncode, 1)
                self.assertIn("FAIL: 1 AFE problem(s)", result.stdout)
                self.assertIn(f"cannot read source module {source}", result.stdout)
                self.assertEqual(result.stderr, "")

    def run_tool(self, *args):
        return subprocess.run(
            [sys.executable, *map(str, args)],
            capture_output=True, text=True, encoding="utf-8", errors="replace", check=False, timeout=120,
        )

    def test_previous_name_inputs_fail_with_a_message(self):
        script = TOOLS / "verify_previous_names.py"
        current_csv = TOOLS.parent / "functions.csv"
        current_baseline = TOOLS / "released-names-v1.2.6.txt"
        missing = self.directory / "missing"
        directory = self.directory / "directory"
        directory.mkdir()
        undecodable = self.directory / "not-utf8"
        undecodable.write_bytes(b"\xff")
        oversized = self.directory / "oversized.csv"
        oversized.write_text(
            "function,previous_name,signature,description\n"
            + "x" * (csv.field_size_limit() + 1) + ",,,\n",
            encoding="utf-8",
        )
        cases = [
            ("CSV missing", missing, current_baseline, missing),
            ("CSV directory", directory, current_baseline, directory),
            ("CSV not UTF-8", undecodable, current_baseline, undecodable),
            ("CSV parse error", oversized, current_baseline, oversized),
            ("baseline missing", current_csv, missing, missing),
            ("baseline directory", current_csv, directory, directory),
            ("baseline not UTF-8", current_csv, undecodable, undecodable),
        ]
        for label, index, baseline, failed_path in cases:
            with self.subTest(input=label):
                before = {path: path.read_bytes() for path in (index, baseline) if path.is_file()}
                result = self.run_tool(script, index, baseline)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn(f"FAIL: cannot read {failed_path}:", result.stdout)
                self.assertNotIn("OK:", result.stdout)
                self.assertEqual(result.stderr, "")
                for path, original in before.items():
                    self.assertEqual(path.read_bytes(), original)

    def test_index_and_cover_csv_parse_errors_fail_with_a_message(self):
        workbook = TOOLS.parent / "ozzit.xlsx"
        changelog = TOOLS.parent / "CHANGELOG.md"
        oversized = "x" * (csv.field_size_limit() + 1)
        cases = {
            "header": oversized + ",function\n",
            "row": "function,module,signature,description\n" + oversized + ",,,\n",
        }
        original_inputs = {path: path.read_bytes() for path in (workbook, changelog)}
        for label, text in cases.items():
            index = self.directory / f"{label}.csv"
            index.write_text(text, encoding="utf-8")
            original_index = index.read_bytes()
            for tool, args in (
                ("verify_index.py", (workbook, TOOLS.parent / "src", index)),
                ("verify_cover.py", (workbook, index, changelog)),
            ):
                with self.subTest(tool=tool, CSV=label):
                    result = self.run_tool(TOOLS / tool, *args)
                    self.assertEqual(result.returncode, 1, result.stderr)
                    self.assertIn("FAIL:", result.stdout)
                    self.assertIn("field larger than field limit", result.stdout)
                    self.assertNotIn("OK:", result.stdout)
                    self.assertEqual(result.stderr, "")
                    self.assertEqual(index.read_bytes(), original_index)
        for path, original in original_inputs.items():
            self.assertEqual(path.read_bytes(), original)

    def test_verification_commands_reject_surplus_inputs(self):
        root = TOOLS.parent
        workbook = root / "ozzit.xlsx"
        source = root / "src"
        index = root / "functions.csv"
        cases = [
            ("verify_workbook.py", [workbook]),
            ("verify_sources.py", [workbook, source]),
            ("verify_signatures.py", [source]),
            ("verify_previous_names.py", [index, TOOLS / "released-names-v1.2.6.txt"]),
            ("verify_index.py", [workbook, source, index]),
            ("verify_cover.py", [workbook, index, root / "CHANGELOG.md"]),
            ("verify_afe.py", [workbook, source]),
        ]
        for name, inputs in cases:
            with self.subTest(tool=name):
                result = self.run_tool(
                    TOOLS / name, *inputs, self.directory / "unrequested.xlsx"
                )
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn("FAIL: usage:", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_unreadable_workbooks_fail_with_a_message_not_a_traceback(self):
        not_zip = self.directory / "not-a-zip.xlsx"
        not_zip.write_bytes(b"not a zip archive")
        missing = self.directory / "missing.xlsx"
        no_book = self.write_archive("no-book.xlsx", {"docProps/app.xml": "<Properties/>"})
        undecodable = self.write_archive("undecodable.xlsx", {"xl/workbook.xml": b"<workbook>\xff</workbook>"})
        runs = [(TOOLS / "verify_workbook.py", book) for book in (not_zip, missing)]
        runs += [(TOOLS / "verify_sources.py", book, TOOLS.parent / "src")
                 for book in (not_zip, missing, no_book, undecodable)]
        for script, workbook, *rest in runs:
            with self.subTest(tool=script.name, workbook=workbook.name):
                result = self.run_tool(script, workbook, *rest)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn(f"FAIL: cannot read {workbook}", result.stdout)
                self.assertNotIn("Traceback", result.stderr)

    def test_undecodable_source_modules_fail_with_a_message(self):
        source = self.directory / "Invalid.txt"
        source.write_bytes(b"\xff")
        runs = [(TOOLS / "verify_sources.py", TOOLS.parent / "ozzit.xlsx", self.directory),
                (TOOLS / "verify_signatures.py", self.directory)]
        for script, *args in runs:
            with self.subTest(tool=script.name):
                result = self.run_tool(script, *args)
                self.assertEqual(result.returncode, 1)
                self.assertIn(f"FAIL: cannot read {source}", result.stdout)
                self.assertEqual(result.stderr, "")

    def test_source_directory_entries_fail_with_a_message(self):
        source = self.directory / "Invalid.txt"
        source.mkdir()
        runs = [(TOOLS / "verify_sources.py", TOOLS.parent / "ozzit.xlsx", self.directory),
                (TOOLS / "verify_signatures.py", self.directory)]
        for script, *args in runs:
            with self.subTest(tool=script.name):
                result = self.run_tool(script, *args)
                self.assertEqual(result.returncode, 1)
                self.assertIn(f"FAIL: cannot read {source}", result.stdout)
                self.assertEqual(result.stderr, "")

    def test_workbook_check_skips_parts_already_reported_as_malformed(self):
        encoded = base64.b64encode(json.dumps({"padding": "x" * 80}).encode("utf-16-le"))
        store = b"<store>" + encoded + b"</store>"
        for part in ("docProps/app.xml", "xl/slicerCaches/slicerCache1.xml"):
            with self.subTest(part=part):
                workbook = self.write_archive("parts.xlsx", {"customXml/item1.xml": store, part: b"<p>\xff</p>"})
                result = self.run_tool(TOOLS / "verify_workbook.py", workbook)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn(f"malformed XML in {part}", result.stdout)
                self.assertNotIn("Traceback", result.stderr)


    def test_value_dump_opens_its_output_inside_the_excel_cleanup(self):
        text = (TOOLS / "dump_values.ps1").read_text(encoding="utf-8")
        # a bad -Out path thrown before the try would skip Quit() and leave Excel running
        cleanup = text.index("try {", text.index("New-Object -ComObject Excel.Application"))
        self.assertLess(cleanup, text.index("[System.IO.StreamWriter]::new("))


class CacheComparisonTests(unittest.TestCase):
    def test_column_converts_a1_letters_to_one_based_numbers(self):
        self.assertEqual(verify_help_spills.column_number("A"), 1)
        self.assertEqual(verify_help_spills.column_number("Z"), 26)
        self.assertEqual(verify_help_spills.column_number("AA"), 27)
        self.assertEqual(verify_help_spills.column_number("ZZ"), 702)


if __name__ == "__main__":
    unittest.main()
