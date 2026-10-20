"""Refusal and edge paths of the compiler and
the pyxirr comparison, on small fixture sources and workbooks built in a temporary tree.
"""

import csv
import hashlib
import html
import io
import json
import re
import runpy
import shutil
import sys
import tempfile
import unittest
import zipfile
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import compile_sources  # noqa: E402
import pyxirr_comparison as comparison  # noqa: E402

ROOT = TOOLS.parent
WORKBOOK = ROOT / "ozzit.xlsx"
INDEX = ROOT / "functions.csv"
COLUMNS = ["function", "module", "previous_name", "signature", "description"]


def header(bare: str, description: str) -> str:
    return f"/*  FUNCTION NAME:  {bare}\n    DESCRIPTION:*//**{description}*/\n"


def write_src(directory: Path, overrides: dict[str, str] | None = None) -> Path:
    """One small declaration per module, each under its header, unless overridden."""
    directory.mkdir(parents=True, exist_ok=True)
    for module in compile_sources.MODULES:
        bare = f"{module}λ"
        default = header(bare, f"The {module} sample") + f"{bare} = LAMBDA([x], x + 1);\n"
        (directory / f"{module}.txt").write_text((overrides or {}).get(module, default),
                                                 encoding="utf-8")
    return directory


def write_book(path: Path, stored: dict[str, str] | None = None) -> Path:
    """A workbook whose only part is xl/workbook.xml with the fixture's defined names."""
    names = []
    for module in compile_sources.MODULES:
        name = f"oz.{module}λ"
        body = (stored or {}).get(name, "_xlfn.LAMBDA(_xlop.x, _xlpm.x + 1)")
        names.append(f'<definedName name="{name}" comment="The {module} sample">'
                     f"{html.escape(body, quote=False)}</definedName>")
    book = f"<workbook><definedNames>{''.join(names)}</definedNames></workbook>"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("xl/workbook.xml", book)
    return path


def reimport_without_reconfigure(script: str) -> tuple[dict, str]:
    """Run a tool's module code with a stdout that has no reconfigure(), as a pipe can be."""
    with redirect_stdout(io.StringIO()) as printed:
        namespace = runpy.run_path(str(TOOLS / script), run_name="reimported")
    return namespace, printed.getvalue()


class ScratchTestCase(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)


class ImportWithoutReconfigureTests(unittest.TestCase):
    def test_tools_import_when_stdout_cannot_be_reconfigured(self):
        for script, name in [("compile_sources.py", "NAME_LIMIT")]:
            with self.subTest(script=script):
                namespace, printed = reimport_without_reconfigure(script)
                self.assertIn(name, namespace)
                self.assertEqual(printed, "")


class CompileReadingTests(unittest.TestCase):
    def test_line_and_block_comments_are_dropped_and_an_open_block_ends_the_text(self):
        self.assertEqual(compile_sources.strip_comments('a // note\nb /* x */ c "//kept" /* open'),
                         'a \nb  c "//kept" ')

    def test_arguments_of_an_unclosed_call_run_to_the_end(self):
        self.assertEqual(compile_sources.arguments("x, (y, z), w", 0), ["x", " (y, z)", " w"])

    def test_implicit_intersection_tolerates_spacing_and_an_unclosed_call(self):
        self.assertEqual(compile_sources.unwrap_at("@ First + 1"), "SINGLE(First) + 1")
        self.assertEqual(compile_sources.unwrap_at("@INDEX (x, 1)"), "SINGLE(INDEX (x, 1))")
        self.assertEqual(compile_sources.unwrap_at("@INDEX(x"), "SINGLE(INDEX(x)")

    def test_implicit_intersection_without_a_name_is_refused(self):
        with self.assertRaisesRegex(ValueError, re.escape("@ is not followed by a name: '@1 + x'")):
            compile_sources.unwrap_at("@1 + x")

    def test_a_qualified_library_call_is_kept(self):
        self.assertEqual(compile_sources.render("LAMBDA([x], oz.Otherλ(x))", {"Otherλ"}),
                         "_xlfn.LAMBDA(_xlop.x, oz.Otherλ(_xlpm.x))")

    def test_an_unknown_call_is_refused(self):
        with self.assertRaisesRegex(ValueError, "^Mystery reached the stored form without a marker"):
            compile_sources.render("LAMBDA([x], Mystery(x))", {"Otherλ"})


class CompileSourcesTests(ScratchTestCase):
    def test_a_missing_module_is_refused(self):
        with self.assertRaises(ValueError) as failure:
            compile_sources.compile_sources(self.directory)
        self.assertEqual(str(failure.exception),
                         f"missing source module {self.directory / 'Dates.txt'}")

    def test_statements_that_are_not_declarations_are_skipped(self):
        src = write_src(self.directory / "src", {
            "Dates": header("Datesλ", "The Dates sample") + "Datesλ = LAMBDA([x], x + 1);\nBROKEN;\n",
        })
        compiled = compile_sources.compile_sources(src)
        self.assertEqual([c.name for c in compiled], [f"oz.{m}λ" for m in compile_sources.MODULES])
        self.assertEqual(compiled[0].stored, "_xlfn.LAMBDA(_xlop.x, _xlpm.x + 1)")

    def test_a_declaration_without_its_header_is_refused(self):
        src = write_src(self.directory / "src", {"Dates": "Datesλ = LAMBDA([x], x);\n"})
        with self.assertRaisesRegex(ValueError, "^Datesλ: no FUNCTION NAME / DESCRIPTION header "
                                                "in Dates.txt$"):
            compile_sources.compile_sources(src)

    def test_a_rendering_failure_names_the_function(self):
        src = write_src(self.directory / "src", {
            "Dates": header("Datesλ", "d") + "Datesλ = LAMBDA([x], Mystery(x));\n",
        })
        with self.assertRaisesRegex(ValueError, "^Datesλ: Mystery reached the stored form"):
            compile_sources.compile_sources(src)

    def test_a_definition_over_excels_name_limit_is_refused(self):
        literal = "a" * 9000
        src = write_src(self.directory / "src", {
            "Dates": header("Datesλ", "d") + f'Datesλ = LAMBDA([x], "{literal}");\n',
        })
        stored = f'_xlfn.LAMBDA(_xlop.x,"{literal}")'
        with self.assertRaises(ValueError) as failure:
            compile_sources.compile_sources(src)
        self.assertEqual(str(failure.exception),
                         f"Datesλ: the stored definition is {len(stored)} characters and Excel "
                         "refuses to open a workbook with a defined name over 8192")

    def test_a_binding_that_shadows_a_library_function_does_not_reproduce_its_source(self):
        # qualify() reads Essentialsλ( as the library call; the renderer stores the binding.
        src = write_src(self.directory / "src", {
            "Dates": header("Datesλ", "d")
            + "Datesλ = LAMBDA([x], LET(Essentialsλ, LAMBDA(y, y), Essentialsλ(x)));\n",
        })
        with self.assertRaises(ValueError) as failure:
            compile_sources.compile_sources(src)
        message = str(failure.exception)
        self.assertRegex(message, r"^Datesλ: the rendering does not reproduce its source "
                                  r"at character \d+\n  src   : .*OZ\.ESSENTIALSΛ\(X\)")
        self.assertIn("\n  stored: ", message)


class CompileWorkbookTests(ScratchTestCase):
    def test_a_shipped_name_missing_from_src_is_refused(self):
        book = '<definedName name="oz.Goneλ">1</definedName>'
        with self.assertRaisesRegex(ValueError, "^oz.Goneλ ship in the workbook but are not "
                                                "declared in src/$"):
            compile_sources.apply(book, [])

    def test_an_index_with_other_columns_is_refused(self):
        index = self.directory / "functions.csv"
        index.write_text("function,module\noz.Datesλ,Dates\n", encoding="utf-8")
        with self.assertRaises(ValueError) as failure:
            compile_sources.update_index(index, "", {})
        self.assertEqual(str(failure.exception),
                         f"{index} has unexpected columns: ['function', 'module']")


class CompileMainTests(ScratchTestCase):
    def setUp(self):
        super().setUp()
        self.src = write_src(self.directory / "src")
        self.workbook = self.directory / "ozzit.xlsx"
        self.index = self.directory / "functions.csv"

    def main(self, *args: str) -> str:
        printed = io.StringIO()
        with mock.patch.object(sys, "argv", ["compile_sources.py", *args]), \
                redirect_stdout(printed):
            compile_sources.main()
        return printed.getvalue()

    def refusal(self, *args: str) -> object:
        with self.assertRaises(SystemExit) as failure:
            self.main(*args)
        return failure.exception.code

    def test_unknown_flags_and_extra_paths_print_usage(self):
        self.assertEqual(self.refusal("--force"),
                         "FAIL: usage: python tools/compile_sources.py [workbook] [src dir] "
                         "[--check] [--index=functions.csv]")
        self.assertEqual(self.refusal("a", "b", "c"),
                         "FAIL: usage: python tools/compile_sources.py [workbook] [src dir] [--check]")

    def test_a_missing_or_unreadable_workbook_fails_cleanly(self):
        missing = self.directory / "missing.xlsx"
        self.assertEqual(self.refusal(str(missing), str(self.src)),
                         f"FAIL: no such workbook: {missing}")
        self.workbook.write_bytes(b"not a zip archive")
        self.assertEqual(self.refusal(str(self.workbook), str(self.src)),
                         "FAIL: File is not a zip file")
        self.assertEqual(self.workbook.read_bytes(), b"not a zip archive")

    def test_check_reports_a_stale_definition_without_writing(self):
        write_book(self.workbook, {"oz.Debtλ": "_xlfn.LAMBDA(_xlop.x, _xlpm.x + 2)"})
        before = self.workbook.read_bytes()
        printed = self.main(str(self.workbook), str(self.src), "--check")
        self.assertEqual(printed, "would recompile: oz.Debtλ\n"
                                  f"OK: {self.workbook} checked against {self.src}/\n")
        self.assertEqual(self.workbook.read_bytes(), before)

    def test_recompile_writes_the_workbook_and_index_then_has_nothing_to_do(self):
        write_book(self.workbook, {"oz.Debtλ": "_xlfn.LAMBDA(_xlop.x, _xlpm.x + 2)"})
        with self.index.open("w", encoding="utf-8", newline="") as handle:
            csv.writer(handle, lineterminator="\n").writerow(COLUMNS)
        flag = f"--index={self.index}"
        self.assertEqual(self.main(str(self.workbook), str(self.src), flag),
                         f"recompiled: oz.Debtλ, functions.csv\nOK: {self.workbook}\n")
        with zipfile.ZipFile(self.workbook) as archive:
            book = archive.read("xl/workbook.xml").decode("utf-8")
        self.assertIn('<definedName name="oz.Debtλ" comment="The Debt sample">'
                      "_xlfn.LAMBDA(_xlop.x, _xlpm.x + 1)</definedName>", book)
        with self.index.open(encoding="utf-8-sig", newline="") as handle:
            rows = {row["function"]: row for row in csv.DictReader(handle)}
        self.assertEqual(rows["oz.Debtλ"]["module"], "Debt")
        self.assertEqual(rows["oz.Debtλ"]["description"], "The Debt sample")
        self.assertEqual(self.main(str(self.workbook), str(self.src), flag),
                         f"every defined name already matches src/; no changes\nOK: {self.workbook}\n")


class FakeFinance:
    """Stands in for pyxirr with values that identify each call, to pin the row layout."""

    def __init__(self):
        self.rates = set()

    def ipmt(self, rate, k, term, principal):
        self.rates.add(rate)
        return -k

    def ppmt(self, rate, k, term, principal):
        return -10 * k

    def pmt(self, rate, term, principal):
        return -50

    def fv(self, rate, k, payment, principal):
        self.fv_inputs = (payment, principal)
        return -100 * k


class ComparisonReferenceTests(unittest.TestCase):
    def test_schedule_rows_are_nil_outside_the_term(self):
        px = FakeFinance()
        rows = comparison.schedule(1000, 0.12, 2, 5, 2, 3)(px)
        # opening, interest, repayment, closing, principal for periods 1 to 5
        self.assertEqual(rows, [0.0] * 5
                         + [110.0, 1.0, 11.0, 100.0, 10.0]
                         + [220.0, 2.0, 22.0, 200.0, 20.0]
                         + [0.0] * 10)
        self.assertEqual(px.rates, {0.12 / 12 * 3})
        self.assertEqual(px.fv_inputs, (-50, 1000))

    def test_excel_columns_are_read_in_reference_order(self):
        case = {"id": "grid", "columns": 2}
        self.assertEqual(comparison.excel_values(case, [[1, 2], [3, 4]]), [1.0, 3.0, 2.0, 4.0])
        with self.assertRaises(SystemExit) as failure:
            comparison.excel_values(case, [[1, 2, 3]])
        self.assertEqual(failure.exception.code, "grid: 3 Excel columns, expected 2")

    def compare(self, kind: str, reference: list[float], grid: list[list[float]]) -> dict:
        case = {"id": "case", "what": "w", "formula": "=f()", "kind": kind,
                "reference": lambda _px: reference}
        with mock.patch.object(comparison, "CASES", [case]):
            record, = comparison.compare({"results": {"case": grid}}, object())
        return record

    def test_amounts_and_rates_are_compared_within_their_tolerances(self):
        record = self.compare("money", [1.0, 2.0], [[1.0, 2.0000005]])
        self.assertEqual((record["values"], record["ozzit"], record["pyxirr"], record["measure"]),
                         (2, None, None, "amount"))
        self.assertAlmostEqual(record["max_abs_diff"], 5e-7)
        self.assertEqual((record["tolerance"], record["agrees"], record["note"]),
                         (comparison.MONEY_TOLERANCE, True, None))
        record = self.compare("rate", [0.1], [[0.1000002]])
        self.assertEqual((record["ozzit"], record["pyxirr"], record["measure"]),
                         (0.1000002, 0.1, "rate"))
        self.assertEqual((record["tolerance"], record["agrees"]), (comparison.RATE_TOLERANCE, False))

    def test_a_count_mismatch_stops_the_comparison(self):
        with self.assertRaises(SystemExit) as failure:
            self.compare("money", [1.0, 2.0], [[1.0]])
        self.assertEqual(failure.exception.code, "case: 1 Excel values, 2 pyxirr values")


class ComparisonExcelTests(ScratchTestCase):
    def test_only_a_numeric_recorded_pid_is_stopped(self):
        pid_file = self.directory / "out.json.pid"
        with mock.patch.object(comparison.subprocess, "run") as run:
            comparison.stop_recorded_excel(pid_file)
            pid_file.write_text("not a pid\n", encoding="utf-8")
            comparison.stop_recorded_excel(pid_file)
            run.assert_not_called()
            pid_file.write_text("4321\n", encoding="utf-8")
            comparison.stop_recorded_excel(pid_file)
        command, = run.call_args.args
        self.assertEqual(command[:3], ["powershell.exe", "-NoProfile", "-Command"])
        self.assertIn('ProcessId=4321 AND Name=\'EXCEL.EXE\'', command[3])
        self.assertIn("-match '/automation'", command[3])
        self.assertIn("Stop-Process -Id 4321 -Force", command[3])
        self.assertEqual(run.call_args.kwargs, {"check": False, "timeout": 60})

    def test_evaluate_passes_every_case_and_returns_the_recorded_result(self):
        seen = {}

        def evaluator(command, **kwargs):
            seen.update(kwargs, cases=json.loads(Path(command[command.index("-Cases") + 1])
                                                 .read_text(encoding="utf-8")),
                        path=command[command.index("-Path") + 1], file=command[5])
            Path(command[command.index("-Out") + 1]).write_text(
                json.dumps({"excel": "16.0", "results": {}}), encoding="utf-8")
            return SimpleNamespace(args=command, returncode=0)

        workbook = self.directory / "book.xlsx"
        with mock.patch.object(comparison.subprocess, "run", side_effect=evaluator):
            self.assertEqual(comparison.evaluate(workbook), {"excel": "16.0", "results": {}})
        self.assertEqual(seen["cases"], [{"id": c["id"], "formula": c["formula"]}
                                         for c in comparison.CASES])
        self.assertEqual(seen["path"], str(workbook))
        self.assertEqual(seen["file"], str(comparison.ROOT / "tools" / "excel_eval_formulas.ps1"))
        self.assertEqual((seen["check"], seen["timeout"]), (True, 600))

    def test_a_timed_out_evaluation_stops_only_the_recorded_excel(self):
        timeout = comparison.subprocess.TimeoutExpired(["powershell.exe"], 600)
        with mock.patch.object(comparison.subprocess, "run", side_effect=timeout), \
                mock.patch.object(comparison, "stop_recorded_excel") as stop, \
                self.assertRaises(SystemExit) as failure:
            comparison.evaluate(self.directory / "book.xlsx")
        self.assertEqual(failure.exception.code,
                         "FAIL: the Excel evaluation did not finish within 600s. Check for a "
                         "modal dialog before retrying.")
        pid_file, = stop.call_args.args
        self.assertEqual(pid_file.name, "out.json.pid")

    def test_write_table_replaces_only_the_results_block(self):
        document = self.directory / "comparison.md"
        document.write_text(f"intro\n{comparison.TABLE_START}\nold\n{comparison.TABLE_END}\noutro\n",
                            encoding="utf-8")
        evidence = {"cases": [{"id": "c", "measure": "rate", "values": 1, "ozzit": 0.1,
                               "pyxirr": 0.1, "max_abs_diff": 0.0, "agrees": True, "note": None}]}
        with mock.patch.object(comparison, "DOCUMENT", document):
            comparison.write_table(evidence)
        self.assertEqual(document.read_text(encoding="utf-8"),
                         f"intro\n{comparison.TABLE_START}\n{comparison.table(evidence)}\n"
                         f"{comparison.TABLE_END}\noutro\n")


class ComparisonMainTests(ScratchTestCase):
    def setUp(self):
        super().setUp()
        self.workbook = self.directory / "released.xlsx"
        self.workbook.write_bytes(b"synthetic workbook bytes")
        self.digest = hashlib.sha256(self.workbook.read_bytes()).hexdigest()

    def refusal(self, args: list[str]) -> str:
        printed = io.StringIO()
        with mock.patch.object(comparison, "evaluate") as evaluate, redirect_stderr(printed), \
                self.assertRaises(SystemExit) as failure:
            comparison.main(args)
        self.assertEqual(failure.exception.code, 2)
        evaluate.assert_not_called()
        return printed.getvalue()

    def test_a_missing_workbook_or_output_directory_is_refused(self):
        missing = self.directory / "missing.xlsx"
        self.assertIn(f"workbook not found: {missing.resolve()}",
                      self.refusal(["--workbook", str(missing), "--output",
                                    str(self.directory / "out.json"), "--expected-sha256", "0"]))
        self.assertIn("the output directory must already exist",
                      self.refusal(["--workbook", str(self.workbook), "--output",
                                    str(self.directory / "absent" / "out.json"),
                                    "--expected-sha256", self.digest]))

    def test_a_different_pyxirr_version_stops_before_excel(self):
        output = self.directory / "out.json"
        with mock.patch.object(comparison.importlib, "import_module") as load, \
                mock.patch.object(comparison.importlib.metadata, "version", return_value="0.9.0"), \
                mock.patch.object(comparison, "evaluate") as evaluate, \
                redirect_stdout(io.StringIO()) as printed:
            result = comparison.main(["--workbook", str(self.workbook), "--output", str(output),
                                      "--expected-sha256", self.digest])
        self.assertEqual(result, 1)
        self.assertEqual(printed.getvalue(),
                         f"FAIL: pyxirr 0.9.0 installed; this comparison pins {comparison.PYXIRR}\n")
        load.assert_called_once_with("pyxirr")
        evaluate.assert_not_called()
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
