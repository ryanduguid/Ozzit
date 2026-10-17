"""Refusal and edge paths of the cache, native evidence, index and help-spill gates."""

import contextlib
import csv
import hashlib
import importlib.util
import io
import json
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
ROOT = TOOLS.parent
sys.path.insert(0, str(TOOLS))

import verify_cache  # noqa: E402
import verify_help_spills as spills  # noqa: E402
import verify_index  # noqa: E402
import verify_native_evidence as evidence  # noqa: E402


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
        for name in ("verify_cache", "verify_index", "verify_help_spills"):
            with self.subTest(tool=name):
                module = load_fresh(name)
                self.assertTrue(callable(module.main))
                self.assertIsNot(module, sys.modules[name])


class CacheGatePathTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-cache-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def workbook(self, cells):
        """One worksheet, no shared strings part, holding the given cell XML."""
        return write_archive(self.directory / "cache.xlsx", {
            "xl/workbook.xml": '<workbook><sheets><sheet name="Data" r:id="rId1"/></sheets></workbook>',
            "xl/_rels/workbook.xml.rels": (
                '<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>'
            ),
            "xl/worksheets/sheet1.xml": "<worksheet><sheetData>%s</sheetData></worksheet>" % cells,
        })

    def numbers(self, count, value="1"):
        return self.workbook("".join(f'<c r="A{row}"><v>{value}</v></c>' for row in range(1, count + 1)))

    def run_gate(self, workbook, dump="", returncode=0, stdout="", stderr="", floor=None):
        dumps = []

        def dump_values(args, **kwargs):
            target = Path(args[args.index("-Out") + 1])
            dumps.append(target)
            if returncode:
                target.unlink()
            else:
                target.write_text(dump, encoding="utf-8")
            return SimpleNamespace(args=args, returncode=returncode, stdout=stdout,
                                   stderr=stderr)

        output = io.StringIO()
        patches = [mock.patch.object(verify_cache, "WORKBOOK", str(workbook)),
                   mock.patch.object(verify_cache.subprocess, "run", side_effect=dump_values)]
        if floor is not None:
            patches.append(mock.patch.object(verify_cache, "FLOOR", floor))
        with contextlib.ExitStack() as stack:
            for item in patches:
                stack.enter_context(item)
            stack.enter_context(contextlib.redirect_stdout(output))
            result = verify_cache.main()
        for target in dumps:
            self.assertFalse(target.exists(), "the gate left its value dump behind")
        return result, output.getvalue()

    def test_a_missing_workbook_fails_before_excel_is_driven(self):
        missing = self.directory / "missing.xlsx"
        output = io.StringIO()
        with mock.patch.object(verify_cache, "WORKBOOK", str(missing)), \
                mock.patch.object(verify_cache.subprocess, "run") as run, \
                contextlib.redirect_stdout(output):
            self.assertEqual(verify_cache.main(), 1)
        self.assertEqual(output.getvalue(), f"FAIL: no such workbook: {missing}\n")
        run.assert_not_called()

    def test_an_excel_failure_reports_its_exit_code_and_output(self):
        # The dumper exits before writing, so the cleanup finds no file to remove.
        result, output = self.run_gate(self.numbers(1), returncode=3, stderr="  COM refused \n")
        self.assertEqual(result, 1)
        self.assertEqual(
            output, "FAIL: could not read the workbook in Excel (exit 3)\nCOM refused\n"
        )

    def test_cached_values_without_shared_strings_skip_empty_inline_cells(self):
        workbook = self.workbook('<c r="A1"><v>7</v></c><c r="B1" t="inlineStr"><is/></c>')
        self.assertEqual(verify_cache.cached_values(workbook),
                         ({(1, 1, 1): verify_cache.CellValue("7")}, {1: "Data"}))

    def test_malformed_dump_lines_fail_with_a_diagnostic(self):
        workbook = self.workbook('<c r="A1"><v>7</v></c><c r="B1" t="inlineStr"><is/></c>')
        result, output = self.run_gate(workbook, dump="1\t1\t1\tvalue\t7\nnot a dump row\n1\t2\n")
        self.assertEqual(result, 1)
        self.assertIn("FAIL: cannot read Excel dump: row 2", output)

    def test_a_complete_but_thin_dump_still_fails_the_floor(self):
        result, output = self.run_gate(self.numbers(1), dump="1\t1\t1\tvalue\t1\n")
        self.assertEqual(result, 1)
        self.assertIn("FAIL: only 1 cells carried both a cached and a calculated value, "
                      "under the floor of 15000.", output)

    def test_value_tabs_remain_inside_the_final_dump_field(self):
        workbook = self.workbook('<c r="A1" t="str"><v>a\tb</v></c>')
        result, output = self.run_gate(workbook, dump="1\t1\t1\tvalue\ta\tb\n", floor=1)
        self.assertEqual(result, 0, output)

    def test_more_than_twenty_missing_cells_are_summarised(self):
        result, output = self.run_gate(self.numbers(25))
        self.assertEqual(result, 1)
        lines = output.splitlines()
        self.assertEqual(lines[0], "FAIL: 25 cached value(s) are missing from the Excel dump")
        self.assertEqual(len(lines), 22)
        self.assertEqual(lines[-1], "  ... and 5 more")

    def test_more_than_twenty_stale_cells_are_summarised(self):
        dump = "".join(f"1\t{row}\t1\tvalue\t2\n" for row in range(1, 26))
        result, output = self.run_gate(self.numbers(25), dump=dump, floor=1)
        self.assertEqual(result, 1)
        lines = output.splitlines()
        self.assertEqual(
            lines[0], "FAIL: 25 of 25 cached values are not what the formula produces, on 1 sheet(s)"
        )
        self.assertEqual(lines[1], "  - Data row 1 col 1: cached 1 (value), calculates to 2 (value)")
        self.assertEqual(lines[-2:], ["  ... and 5 more", "Run: python tools/refresh_cache.py"])


class NativeEvidencePathTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-evidence-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def test_a_supplied_installed_workbook_that_matches_is_confirmed(self):
        root = self.directory / "root"
        for name in (evidence.COMPARISON, evidence.INSTALLATION, *evidence.RUNNERS, evidence.INSTALL_RUNNER):
            (root / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, root / name)
        (root / "ozzit.xlsx").write_bytes(b"synthetic checkout workbook")
        installed = self.directory / "installed.xlsx"
        installed.write_bytes(b"synthetic installed workbook")
        record = evidence.read_record(root / evidence.INSTALLATION)
        record["destination_sha256"] = hashlib.sha256(installed.read_bytes()).hexdigest()
        (root / evidence.INSTALLATION).write_text(json.dumps(record), encoding="utf-8")

        messages = evidence.verify(root, destination=installed)

        self.assertIn("Supplied installed workbook bytes match the retained evidence.", messages)
        self.assertNotIn("Supplied release workbook bytes match the retained evidence.", messages)

    def run_main(self, *argv):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = evidence.main(list(argv))
        return result, output.getvalue()

    def test_main_prints_each_message_for_the_checkout(self):
        result, output = self.run_main()
        self.assertEqual(result, 0, output)
        self.assertEqual(output.splitlines(), evidence.verify())

    def test_main_reports_a_mismatched_release_workbook(self):
        supplied = self.directory / "release.xlsx"
        supplied.write_bytes(b"not the release workbook")
        result, output = self.run_main("--workbook", str(supplied))
        self.assertEqual(result, 1)
        self.assertEqual(
            output, "FAIL: native evidence: release workbook hash does not match the retained evidence\n"
        )


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


HELP_BOOK = (
    '<workbook><definedNames><definedName name="oz.Fλ">LAMBDA(x, TRIM(TEXTSPLIT('
    '"FUNCTION:→Fλ(x)¶NOTE:→none", "→", "¶")))</definedName></definedNames></workbook>'
)
HELP_SHEET = (
    "<worksheet><sheetData>"
    '<row r="4"><c r="A4" t="str"><f t="array" ref="A4:C6">oz.Fλ()</f><v>FUNCTION:</v></c>'
    '<c r="B4" t="str"><v>Fλ(x)</v></c></row>'
    '<row r="5"><c r="A5" t="str"><v>NOTE:</v></c><c r="B5" t="str"><v>none</v></c></row>'
    "</sheetData></worksheet>"
)


class HelpSpillPathTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-help-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def test_a_help_split_on_other_delimiters_is_refused(self):
        for stored in ('TRIM(TEXTSPLIT("a→b", ",", "¶"))', 'TRIM(TEXTSPLIT("a→b", "→"))'):
            with self.subTest(stored=stored):
                with self.assertRaisesRegex(ValueError, "oz.Fλ: the help is not split on → and ¶"):
                    spills.help_table(stored, "oz.Fλ")

    def test_a_spill_of_the_wrong_width_reports_both_dimensions(self):
        parts = {"xl/workbook.xml": HELP_BOOK.encode("utf-8"),
                 "xl/worksheets/sheet1.xml": HELP_SHEET.encode("utf-8")}
        self.assertEqual(spills.check(parts), ([
            "xl/worksheets/sheet1.xml: the oz.Fλ help is stale, "
            "3 cached rows against 2 in the definition, 3 cached columns against 2"
        ], 1))

    def run_main(self, *argv):
        with mock.patch.object(sys, "argv", ["verify_help_spills.py", *map(str, argv)]), \
                self.assertRaises(SystemExit) as raised:
            spills.main()
        return raised.exception.code

    def test_main_refuses_extra_arguments_a_missing_file_and_a_non_archive(self):
        missing = self.directory / "missing.xlsx"
        not_zip = self.directory / "not-zip.xlsx"
        not_zip.write_bytes(b"not a zip archive")
        self.assertEqual(self.run_main("a.xlsx", "b.xlsx"),
                         "FAIL: usage: python tools/verify_help_spills.py [workbook]")
        self.assertEqual(self.run_main(missing), f"FAIL: no such workbook: {missing}")
        self.assertEqual(self.run_main(not_zip), "FAIL: File is not a zip file")


if __name__ == "__main__":
    unittest.main()
