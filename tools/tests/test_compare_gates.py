import importlib.util
import io
import subprocess
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("compare_gates", ROOT / ".github" / "ci" / "compare_gates.py")
assert SPEC is not None and SPEC.loader is not None
compare_gates = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(compare_gates)


def result(code: int, stdout: str, stderr: str) -> subprocess.CompletedProcess[str]:
    return subprocess.CompletedProcess([], code, stdout, stderr)


class CompareGatesTests(unittest.TestCase):
    def compare(self, rust: subprocess.CompletedProcess[str], python: subprocess.CompletedProcess[str]) -> bool:
        with mock.patch.object(compare_gates, "run", side_effect=[rust, python]):
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                return bool(compare_gates.compare("gate", "ozzit", "verify_workbook", [], ROOT))

    def test_identical_results_are_the_same(self):
        self.assertTrue(self.compare(result(0, "OK\n", ""), result(0, "OK\n", "")))

    def test_a_stderr_difference_is_reported(self):
        self.assertFalse(self.compare(result(0, "OK\n", ""), result(0, "OK\n", "different diagnostic\n")))

    def test_the_zip_reason_allowance_still_compares_stderr(self):
        rust_reason, python_reason = compare_gates.ZIP_REASON
        rust = result(1, f"FAIL: {rust_reason}\n", "")
        self.assertTrue(self.compare(rust, result(1, f"FAIL: {python_reason}\n", "")))
        rust = result(1, f"FAIL: {rust_reason}\n", "")
        self.assertFalse(self.compare(rust, result(1, f"FAIL: {python_reason}\n", "extra\n")))
        rust = result(1, "", f"FAIL: {rust_reason}\n")
        self.assertTrue(self.compare(rust, result(1, "", f"FAIL: {python_reason}\n")))


if __name__ == "__main__":
    unittest.main()
