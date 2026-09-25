"""Retained native evidence must identify its inputs and refuse contradictory records."""

import copy
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

import verify_native_evidence as evidence  # noqa: E402


class NativeEvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in (evidence.COMPARISON, evidence.INSTALLATION, *evidence.RUNNERS, evidence.INSTALL_RUNNER):
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        (self.root / "ozzit.xlsx").write_bytes(b"Different synthetic workbook")

    def write(self, name, value):
        (self.root / name).write_text(json.dumps(value), encoding="utf-8")

    def test_valid_records_are_historical_when_the_checkout_differs(self):
        messages = evidence.verify(self.root)
        self.assertIn("Historical release evidence", "\n".join(messages))
        self.assertIn("No native execution", messages[-1])

    def test_runner_drift_or_unexpected_paths_are_refused(self):
        for name in (*evidence.RUNNERS, evidence.INSTALL_RUNNER):
            path = self.root / name
            original = path.read_bytes()
            path.write_bytes(original + b"\n")
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "stale"):
                evidence.verify(self.root)
            path.write_bytes(original)
        record = evidence.read_record(self.root / evidence.COMPARISON)
        record["runner_files"]["../unexpected"] = "0" * 64
        self.write(evidence.COMPARISON, record)
        with self.assertRaisesRegex(ValueError, "inventory"):
            evidence.verify(self.root)

    def test_missing_cases_false_flags_and_nonfinite_values_are_refused(self):
        for name in (evidence.COMPARISON, evidence.INSTALLATION):
            original = evidence.read_record(self.root / name)
            mutations = []
            missing = copy.deepcopy(original)
            missing["cases"].pop()
            mutations.append(missing)
            duplicate = copy.deepcopy(original)
            duplicate["cases"].append(duplicate["cases"][0])
            mutations.append(duplicate)
            if name == evidence.COMPARISON:
                for value in (float("nan"), -1, 100):
                    bad = copy.deepcopy(original)
                    bad["cases"][0]["max_abs_diff"] = value
                    mutations.append(bad)
                bad = copy.deepcopy(original)
                bad["cases"][2]["note"] = "an invented reason"
                mutations.append(bad)
            else:
                for key, value in (("passed", False), ("passed", "true"),
                                   ("source_unchanged", False), ("external_links", True)):
                    bad = copy.deepcopy(original)
                    bad[key] = value
                    mutations.append(bad)
                for value in (float("nan"), True, "#NUM!", 0):
                    bad = copy.deepcopy(original)
                    bad["cases"][0]["changed_input"]["values"][0] = value
                    mutations.append(bad)
            for index, bad in enumerate(mutations):
                with self.subTest(name=name, index=index):
                    self.write(name, bad)
                    with self.assertRaises(ValueError):
                        evidence.verify(self.root)
            self.write(name, original)

    def test_supplied_assets_must_match_and_are_not_modified(self):
        path = self.root / "provided.xlsx"
        original = b"synthetic mismatched workbook"
        path.write_bytes(original)
        for key in ("workbook", "destination"):
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "hash does not match"):
                evidence.verify(self.root, **{key: path})
        self.assertEqual(path.read_bytes(), original)

    def test_case_counts_and_installation_inputs_cannot_change(self):
        trial = evidence.read_record(self.root / evidence.COMPARISON)
        for index in (0, 6, 7):
            changed = copy.deepcopy(trial)
            changed["cases"][index]["values"] += 1
            self.write(evidence.COMPARISON, changed)
            with self.subTest(comparison=index), self.assertRaises(ValueError):
                evidence.verify(self.root)
        self.write(evidence.COMPARISON, trial)
        install = evidence.read_record(self.root / evidence.INSTALLATION)
        for index in (0, 1):
            for key in ("formula", "changed_formula", "copied_from"):
                changed = copy.deepcopy(install)
                changed["cases"][index][key] = "changed input"
                self.write(evidence.INSTALLATION, changed)
                with self.subTest(installation=index, field=key), self.assertRaises(ValueError):
                    evidence.verify(self.root)
            changed = copy.deepcopy(install)
            for key in ("reopened", "changed_input"):
                grid = changed["cases"][index][key]
                grid["rows"], grid["columns"] = grid["columns"], grid["rows"]
            self.write(evidence.INSTALLATION, changed)
            with self.subTest(installation=index, field="shape"), self.assertRaises(ValueError):
                evidence.verify(self.root)

    def test_duplicate_json_keys_are_refused(self):
        path = self.root / evidence.INSTALLATION
        path.write_text('{"passed":false,"passed":true}', encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "duplicate"):
            evidence.verify(self.root)

    def test_the_retained_two_root_exception_cannot_be_relabelled_as_agreement(self):
        trial = evidence.read_record(self.root / evidence.COMPARISON)
        trial["cases"][2].update(agrees=True, ozzit=0.1, pyxirr=0.1,
                                  max_abs_diff=0, tolerance=evidence.comparison.MONEY_TOLERANCE,
                                  note=None)
        self.write(evidence.COMPARISON, trial)
        with self.assertRaises(ValueError):
            evidence.verify(self.root)

    def test_doubled_installation_values_must_still_match_the_fixture(self):
        install = evidence.read_record(self.root / evidence.INSTALLATION)
        for index in (0, 1):
            changed = copy.deepcopy(install)
            for key in ("reopened", "changed_input"):
                grid = changed["cases"][index][key]
                grid["values"] = [value * 3 for value in grid["values"]]
            self.write(evidence.INSTALLATION, changed)
            with self.subTest(installation=index), self.assertRaises(ValueError):
                evidence.verify(self.root)

    def test_the_recorded_reference_rate_must_produce_the_reported_npv(self):
        trial = evidence.read_record(self.root / evidence.COMPARISON)
        for rate in (0.0, 0.5, -1.0, -2.0):
            changed = copy.deepcopy(trial)
            changed["cases"][2]["pyxirr"] = rate
            self.write(evidence.COMPARISON, changed)
            with self.subTest(rate=rate), self.assertRaises(ValueError):
                evidence.verify(self.root)

    def test_the_recorded_npv_must_be_a_non_negative_magnitude(self):
        trial = evidence.read_record(self.root / evidence.COMPARISON)
        trial["cases"][2]["pyxirr_npv"] *= -1
        self.write(evidence.COMPARISON, trial)
        with self.assertRaises(ValueError):
            evidence.verify(self.root)


if __name__ == "__main__":
    unittest.main()
