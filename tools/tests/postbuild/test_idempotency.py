"""Every XML-only postbuild pass must be a byte no-op on the committed workbook.

This is the reproducibility property testable without the earlier workbook: a
second run either reports "already applied" and changes nothing, or fails on an
assertion. A silent mutation means the pass is not idempotent and must not ship.
"""

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
WORKBOOK = ROOT / "ozzit.xlsx"

XML_PASSES = [
    (TOOLS / "postbuild" / "fy27_help_text.py", True),
    (TOOLS / "postbuild" / "workbook_palette.py", False),
    (TOOLS / "postbuild" / "gst_help_text.py", True),
    (TOOLS / "postbuild" / "help_links.py", True),
    (TOOLS / "postbuild" / "sheet_names.py", False),
    (TOOLS / "postbuild" / "strip_revision_history.py", True),
    (TOOLS / "postbuild" / "aasb16_leases.py", True),
    (TOOLS / "postbuild" / "rate_date_helpers.py", True),
    (TOOLS / "postbuild" / "cover_label.py", False),
    (TOOLS / "postbuild" / "help_corrections.py", True),
    (TOOLS / "postbuild" / "remove_residue.py", False),
]


class PostbuildIdempotencyTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-idem-"))
        self.workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, self.workbook)
        self.src = self.directory / "src"
        shutil.copytree(ROOT / "src", self.src)

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def test_each_xml_pass_is_a_byte_noop(self):
        # Compared with the committed bytes, not with the result of a first run: a pass
        # that rewrote the committed workbook once and then held still would otherwise
        # pass as a no-op.
        committed = WORKBOOK.read_bytes()
        src_committed = {p.name: p.read_bytes() for p in (ROOT / "src").glob("*.txt")}
        for script, takes_src in XML_PASSES:
            with self.subTest(script=script.name):
                command = [sys.executable, str(script), str(self.workbook)]
                if takes_src:
                    command.append(str(self.src))
                for run in ("first", "second"):
                    result = subprocess.run(command, capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertIn("already", result.stdout.lower())
                    self.assertEqual(
                        self.workbook.read_bytes(),
                        committed,
                        f"{script.name} mutated the workbook on its {run} run",
                    )
                    self.assertEqual(
                        {p.name: p.read_bytes() for p in self.src.glob("*.txt")},
                        src_committed,
                        f"{script.name} mutated src/ on its {run} run",
                    )


if __name__ == "__main__":
    unittest.main()
