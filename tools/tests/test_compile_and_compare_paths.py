"""pyxirr comparison refusal paths on owned fixtures.
"""

import hashlib
import io
import json
import shutil
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import pyxirr_comparison as comparison  # noqa: E402


class ScratchTestCase(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)


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
