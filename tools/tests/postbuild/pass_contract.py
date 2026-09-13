"""Shared contract for the postbuild text passes.

Each pass rewrites the workbook and src/ in place and must be a byte no-op on
the current stores. The mixin carries the scratch copy of both stores, the
subprocess runner and that no-op test; each test module sets PASS_SCRIPT.
"""

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
WORKBOOK = ROOT / "ozzit.xlsx"


class PassContractMixin:
    """Mix into a unittest.TestCase whose PASS_SCRIPT names the pass under test."""

    PASS_SCRIPT: Path

    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-postbuild-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _run(self, workbook, src_dir):
        return subprocess.run(
            [sys.executable, str(self.PASS_SCRIPT), str(workbook), str(src_dir)],
            capture_output=True,
            text=True,
            check=False,
        )

    def _copy_tree(self):
        workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, workbook)
        src = self.directory / "src"
        shutil.copytree(ROOT / "src", src)
        return workbook, src

    def test_pass_is_byte_noop_on_current_workbook(self):
        workbook, src = self._copy_tree()
        before = workbook.read_bytes()
        src_before = {p.name: p.read_bytes() for p in src.glob("*.txt")}
        result = self._run(workbook, src)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("already", result.stdout.lower())
        self.assertEqual(workbook.read_bytes(), before)
        self.assertEqual({p.name: p.read_bytes() for p in src.glob("*.txt")}, src_before)
