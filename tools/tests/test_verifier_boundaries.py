"""Fixture tests for workbook readers that sit at untyped file boundaries."""

import base64
import json
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import verify_afe
import verify_cache


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

    def cache_parts(self, target="worksheets/sheet1.xml"):
        return {
            "xl/workbook.xml": (
                '<workbook><sheets><sheet name="Data" r:id="rId1"/>'
                "</sheets></workbook>"
            ),
            "xl/_rels/workbook.xml.rels": (
                '<Relationships><Relationship Id="rId1" Target="%s"/>'
                "</Relationships>" % target
            ),
            "xl/sharedStrings.xml": (
                '<sst><si><t>A&amp;B</t></si><si><t>second</t></si></sst>'
            ),
            "xl/worksheets/sheet1.xml": (
                '<worksheet><sheetData><row r="1">'
                '<c r="A1" t="s"><v>0</v></c>'
                '<c r="B2" t="b"><v>1</v></c>'
                '<c r="C3" t="inlineStr"><is><t>inline &lt;value&gt;</t></is></c>'
                '<c r="D4"><v>123.5</v></c>'
                '<c r="E5"><f>1+1</f></c>'
                '<c r="AA6" t="str"><v>cached text</v></c>'
                "</row></sheetData></worksheet>"
            ),
        }

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

    def test_cached_values_reads_each_supported_cell_representation(self):
        workbook = self.write_archive("cache.xlsx", self.cache_parts())

        values, names = verify_cache.cached_values(workbook)

        self.assertEqual(names, {1: "Data"})
        self.assertEqual(
            values,
            {
                (1, 1, 1): "A&B",
                (1, 2, 2): "True",
                (1, 3, 3): "inline <value>",
                (1, 4, 4): "123.5",
                (1, 6, 27): "cached text",
            },
        )

    def test_cached_values_rejects_chart_sheets_before_positions_drift(self):
        parts = self.cache_parts("chartsheets/sheet1.xml")
        workbook = self.write_archive("chart.xlsx", parts)

        with self.assertRaisesRegex(SystemExit, "are not worksheets"):
            verify_cache.cached_values(workbook)


class CacheComparisonTests(unittest.TestCase):
    def test_column_converts_a1_letters_to_one_based_numbers(self):
        self.assertEqual(verify_cache.column("A"), 1)
        self.assertEqual(verify_cache.column("Z"), 26)
        self.assertEqual(verify_cache.column("AA"), 27)
        self.assertEqual(verify_cache.column("ZZ"), 702)

    def test_fold_normalises_all_physical_newline_forms(self):
        self.assertEqual(verify_cache.fold("a\r\nb\rc\nd"), r"a\nb\nc\nd")

    def test_agree_accepts_exact_text_and_negligible_numeric_rounding(self):
        self.assertTrue(verify_cache.agree("plain", "plain"))
        self.assertTrue(verify_cache.agree("1000000", "1000000.0005"))

    def test_agree_rejects_material_numeric_and_text_differences(self):
        self.assertFalse(verify_cache.agree("1", "1.01"))
        self.assertFalse(verify_cache.agree("old", "new"))

    def test_agree_maps_excel_com_error_codes(self):
        self.assertTrue(verify_cache.agree("#DIV/0!", "-2146826281"))
        self.assertFalse(verify_cache.agree("#N/A", "-2146826281"))


if __name__ == "__main__":
    unittest.main()
