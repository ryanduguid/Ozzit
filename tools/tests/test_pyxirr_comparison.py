"""The recorded Excel-versus-pyxirr comparison matches its cases and its published table.

CI has neither desktop Excel nor pyxirr, so this does not rerun the comparison. It checks
that docs/pyxirr-comparison.json was produced from the cases tools/pyxirr_comparison.py
defines now, that every difference is inside tolerance or carries its documented reason,
and that the table in docs/pyxirr-comparison.md is the one that record renders.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

import pyxirr_comparison as comparison  # noqa: E402


class PyxirrComparisonTests(unittest.TestCase):
    def setUp(self) -> None:
        self.evidence = json.loads(comparison.EVIDENCE.read_text(encoding="utf-8"))

    def test_record_covers_exactly_the_current_cases(self) -> None:
        recorded = [(c["id"], c["formula"]) for c in self.evidence["cases"]]
        defined = [(c["id"], c["formula"]) for c in comparison.CASES]
        self.assertEqual(recorded, defined, "rerun tools/pyxirr_comparison.py after "
                                            "changing a case")

    def test_record_identifies_what_was_compared(self) -> None:
        self.assertRegex(self.evidence["workbook_sha256"], r"^[0-9a-f]{64}$")
        self.assertRegex(self.evidence["commit"], r"^[0-9a-f]{40}$")
        current = hashlib.sha256((ROOT / self.evidence["workbook"]).read_bytes()).hexdigest()
        self.assertEqual(self.evidence["workbook_sha256"], current,
                         "the workbook changed since the recorded comparison; rerun "
                         "tools/pyxirr_comparison.py in desktop Excel")
        self.assertEqual(self.evidence["pyxirr"], comparison.PYXIRR)
        self.assertTrue(self.evidence["excel"])

    def test_every_difference_is_within_tolerance_or_documented(self) -> None:
        notes = {c["id"]: c.get("expected_difference") for c in comparison.CASES}
        for case in self.evidence["cases"]:
            with self.subTest(case=case["id"]):
                if case["agrees"]:
                    self.assertLessEqual(case["max_abs_diff"], case["tolerance"])
                    self.assertIsNone(case["note"])
                else:
                    self.assertTrue(notes[case["id"]], "an undocumented difference")
                    self.assertEqual(case["note"], notes[case["id"]])

    def test_a_reported_root_is_a_root(self) -> None:
        for case in self.evidence["cases"]:
            if case.get("pyxirr_npv") is not None:
                with self.subTest(case=case["id"]):
                    self.assertLessEqual(case["pyxirr_npv"], comparison.MONEY_TOLERANCE)

    def test_document_table_is_the_rendered_record(self) -> None:
        text = comparison.DOCUMENT.read_text(encoding="utf-8")
        match = re.search(re.escape(comparison.TABLE_START) + r"\n(.*)\n"
                          + re.escape(comparison.TABLE_END), text, re.S)
        self.assertIsNotNone(match)
        assert match is not None
        self.assertEqual(match.group(1), comparison.table(self.evidence))

    def test_a_missing_error_note_fails_the_rendered_table(self) -> None:
        """Mutation guard: a difference without its reason must render as NO."""
        broken = json.loads(json.dumps(self.evidence))
        for case in broken["cases"]:
            if not case["agrees"]:
                case["note"] = None
        if any(not c["agrees"] for c in broken["cases"]):
            self.assertIn("| NO |", comparison.table(broken))


class ComparisonFailureTests(unittest.TestCase):
    def test_only_num_error_with_a_verified_reference_root_is_expected(self) -> None:
        case = next(c for c in comparison.CASES if c["id"] == "irr-two-roots")
        for code, npv, expected in [(-2146826252, 0.0, True),
                                    (-2146826259, 0.0, False),
                                    (-2146826273, 0.0, False),
                                    (-2146826252, 1.0, False),
                                    (-2146826252, float("nan"), False)]:
            with self.subTest(code=code, npv=npv), patch.object(comparison, "CASES", [case]):
                px = Mock(xirr=Mock(return_value=0.1), xnpv=Mock(return_value=npv))
                excel = {"results": {case["id"]: [[f"#ERR:{code}"]]}}
                record, = comparison.compare(excel, px)
                self.assertFalse(record["agrees"])
                self.assertEqual(bool(record["note"]), expected)

    def test_a_numeric_non_root_is_not_an_expected_difference(self) -> None:
        case = next(c for c in comparison.CASES if c["id"] == "irr-two-roots")
        with patch.object(comparison, "CASES", [case]):
            px = Mock(xirr=Mock(return_value=0.1), xnpv=Mock(side_effect=[1.0, 0.0]))
            record, = comparison.compare({"results": {case["id"]: [[0.2]]}}, px)
        self.assertFalse(record["agrees"])
        self.assertIsNone(record["note"])


class ReleasedWorkbookTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.workbook = self.root / "released.xlsx"
        self.workbook.write_bytes(b"synthetic workbook bytes; Excel is mocked")
        self.digest = hashlib.sha256(self.workbook.read_bytes()).hexdigest()
        self.output = self.root / "comparison.json"
        self.args = ["--workbook", str(self.workbook), "--output", str(self.output),
                     "--expected-sha256", self.digest]

    def test_external_input_requires_separate_output_and_expected_hash(self) -> None:
        for args in [["--workbook", str(self.workbook)],
                     ["--workbook", str(self.workbook), "--output", str(self.output)]]:
            with self.subTest(args=args), patch.object(comparison, "evaluate") as evaluate:
                with self.assertRaises(SystemExit):
                    comparison.main(args)
                evaluate.assert_not_called()

    def test_wrong_hash_and_existing_output_refuse_before_excel(self) -> None:
        with patch.object(comparison, "evaluate") as evaluate:
            with self.assertRaises(SystemExit):
                comparison.main(self.args[:-1] + ["0" * 64])
            self.output.write_text("previous evidence", encoding="utf-8")
            with self.assertRaises(SystemExit):
                comparison.main(self.args)
            evaluate.assert_not_called()
        self.assertEqual(self.output.read_text(encoding="utf-8"), "previous evidence")

    def run_mocked(self, side_effect=None, excel_hash=None) -> int:
        excel = {"excel": "test Excel", "workbook_sha256": excel_hash or self.digest}
        with (patch.object(comparison, "evaluate", return_value=excel,
                           side_effect=side_effect) as evaluate,
              patch.object(comparison.importlib, "import_module"),
              patch.object(comparison.importlib.metadata, "version", return_value=comparison.PYXIRR),
              patch.object(comparison.subprocess, "run", return_value=Mock(stdout="a" * 40)),
              patch.object(comparison, "compare", return_value=[]),
              patch.object(comparison, "write_table") as write_table):
            result = comparison.main(self.args)
            evaluate.assert_called_once_with(self.workbook.resolve())
            write_table.assert_not_called()
            return result

    def test_evidence_identifies_downloaded_bytes_without_rewriting_history(self) -> None:
        old = {p: p.read_bytes() for p in [comparison.EVIDENCE, comparison.DOCUMENT]}
        self.assertEqual(self.run_mocked(), 0)
        evidence = json.loads(self.output.read_text(encoding="utf-8"))
        self.assertEqual(evidence["workbook"], self.workbook.name)
        self.assertEqual(evidence["workbook_sha256"], self.digest)
        self.assertEqual(evidence["expected_sha256"], self.digest)
        self.assertEqual(evidence["runner_commit"], "a" * 40)
        for relative, digest in evidence["runner_files"].items():
            self.assertEqual(digest, hashlib.sha256((ROOT / relative).read_bytes()).hexdigest())
        for path, content in old.items():
            self.assertEqual(path.read_bytes(), content)

    def test_changed_workbook_or_wrong_evaluated_hash_produces_no_evidence(self) -> None:
        with self.assertRaises(SystemExit):
            self.run_mocked(excel_hash="0" * 64)
        self.assertFalse(self.output.exists())

        def mutate(_):
            self.workbook.write_bytes(b"changed")
            return {"excel": "test Excel", "workbook_sha256": self.digest}

        with self.assertRaises(SystemExit):
            self.run_mocked(side_effect=mutate)
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
