"""Refusal, edge and import paths of the index gate."""

import contextlib
import csv
import importlib.util
import io
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import verify_index  # noqa: E402


def write_archive(path, parts):
    with zipfile.ZipFile(path, "w") as archive:
        for part, data in parts.items():
            archive.writestr(part, data)
    return path


def load_fresh(name):
    """A second copy of a tool, imported while stdout is a stream with no reconfigure()."""
    spec = importlib.util.spec_from_file_location(f"{name}_fresh_copy", TOOLS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    with mock.patch.object(sys, "argv", [name]), mock.patch.object(sys, "path", list(sys.path)), \
            contextlib.redirect_stdout(io.StringIO()):
        spec.loader.exec_module(module)
    return module


class PlainStdoutImportTests(unittest.TestCase):
    def test_each_gate_imports_when_stdout_cannot_be_reconfigured(self):
        self.assertFalse(hasattr(io.StringIO(), "reconfigure"))
        for name in ("verify_index",):
            with self.subTest(tool=name):
                module = load_fresh(name)
                self.assertTrue(callable(module.main))
                self.assertIsNot(module, sys.modules[name])


class IndexGatePathTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-index-paths-"))
        self.src = self.directory / "src"
        self.src.mkdir()
        (self.src / "Mod.txt").write_text("Aλ = LAMBDA(x, x);\n(1 + 2);\n", encoding="utf-8")
        self.book = write_archive(self.directory / "book.xlsx", {
            "xl/workbook.xml": (
                '<workbook><definedNames><definedName name="oz.Aλ">LAMBDA(x, x)</definedName>'
                '<definedName name="oz.Bλ">LAMBDA(y, y)</definedName></definedNames></workbook>'
            ),
        })

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def write_index(self, rows):
        path = self.directory / "functions.csv"
        with path.open("w", encoding="utf-8", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=["function", "module", "signature", "description"])
            writer.writeheader()
            writer.writerows(rows)
        return path

    def run_gate(self, workbook, index):
        output = io.StringIO()
        with mock.patch.object(verify_index, "WORKBOOK", workbook), \
                mock.patch.object(verify_index, "SRC", self.src), \
                mock.patch.object(verify_index, "INDEX", index), \
                contextlib.redirect_stdout(output):
            result = verify_index.main()
        return result, output.getvalue()

    def test_an_unclosed_signature_stops_at_the_last_help_row(self):
        self.assertEqual(verify_index.index_fields("", "FUNCTION: →Fλ(a, b¶", "Fλ"), ("Fλ(a, b", ""))

    def test_source_modules_skip_statements_that_declare_nothing(self):
        self.assertEqual(verify_index.source_modules(self.src), {"oz.Aλ": "Mod"})

    def test_unreadable_inputs_fail_with_a_message(self):
        index = self.write_index([])
        no_book = write_archive(self.directory / "no-book.xlsx", {"docProps/app.xml": "<Properties/>"})
        for label, workbook, expected in (
            ("missing workbook", self.directory / "missing.xlsx", "No such file"),
            ("no workbook part", no_book, "xl/workbook.xml"),
        ):
            with self.subTest(label=label):
                result, output = self.run_gate(workbook, index)
                self.assertEqual(result, 1)
                self.assertTrue(output.startswith("FAIL: cannot read index inputs: "), output)
                self.assertIn(expected, output)

    def test_names_missing_from_either_side_are_reported(self):
        index = self.write_index([
            {"function": "oz.Aλ", "module": "Mod", "signature": "Aλ", "description": ""},
            {"function": "oz.Cλ", "module": "Mod", "signature": "Cλ", "description": ""},
        ])
        result, output = self.run_gate(self.book, index)
        self.assertEqual(result, 1)
        self.assertEqual(output.splitlines(), [
            "FAIL: 2 functions.csv problem(s)",
            "  - oz.Bλ is in the workbook but missing from functions.csv",
            "  - oz.Cλ is in functions.csv but missing from the workbook",
        ])


if __name__ == "__main__":
    unittest.main()
