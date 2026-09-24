"""The recorded Excel-versus-pyxirr comparison matches its cases and its published table.

CI has neither desktop Excel nor pyxirr, so this does not rerun the comparison. It checks
that docs/pyxirr-comparison.json was produced from the cases tools/pyxirr_comparison.py
defines now, that every difference is inside tolerance or carries its documented reason,
and that the table in docs/pyxirr-comparison.md is the one that record renders.
"""

from __future__ import annotations

import json
import re
import sys
import unittest
from pathlib import Path

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


if __name__ == "__main__":
    unittest.main()
