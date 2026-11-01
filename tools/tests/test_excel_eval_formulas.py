"""The formula helper has COM retry calls a busy Excel rejects.

A rejected method call throws RPC_E_CALL_REJECTED, but a rejected property read comes
back $null in PowerShell. On 1 November 2026 that made HasFormula read null for a formula
cell, so 3 of 4 runs on an unchanged workbook failed. The helper registers a COM message
filter that retries a rejected call every 100 ms for up to 60 s.
"""

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HELPER = Path(__file__).resolve().parents[1] / "excel_eval_formulas.ps1"


def filter_source(text: str) -> str:
    start = text.index("Add-Type -TypeDefinition @'\n") + len("Add-Type -TypeDefinition @'\n")
    return text[start:text.index("\n'@", start)]


class MessageFilterTests(unittest.TestCase):
    def test_the_filter_covers_every_excel_call_and_is_revoked(self):
        text = HELPER.read_text(encoding="utf-8")
        register = text.index("[OzzitMessageFilter]::Register()")
        # Registered inside the try, so the finally that quits Excel also revokes it.
        self.assertLess(text.index("try {"), register)
        self.assertLess(register, text.index("New-Object -ComObject Excel.Application"))
        self.assertLess(text.index("finally {"), text.index("[OzzitMessageFilter]::Revoke()"))

    @unittest.skipUnless(
        sys.platform == "win32" and shutil.which("powershell.exe"), "needs Windows PowerShell"
    )
    def test_only_a_retry_later_rejection_is_retried_within_60_seconds(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / "filter.cs"
            source.write_text(filter_source(HELPER.read_text(encoding="utf-8")), encoding="utf-8")
            script = (
                f"Add-Type -TypeDefinition ([IO.File]::ReadAllText('{source}'));"
                "[OzzitMessageFilter]::Register();"
                "$f = New-Object OzzitMessageFilter;"
                "$z = [IntPtr]::Zero;"
                "($f.RetryRejectedCall($z, 0, 2), $f.RetryRejectedCall($z, 59999, 2),"
                " $f.RetryRejectedCall($z, 60000, 2), $f.RetryRejectedCall($z, 0, 1)) -join ',';"
                "[OzzitMessageFilter]::Revoke()"
            )
            run = subprocess.run(
                ["powershell.exe", "-NoProfile", "-NonInteractive", "-STA", "-Command", script],
                capture_output=True, text=True, timeout=120, check=False,
            )
        self.assertEqual(run.returncode, 0, run.stderr)
        # 100 retries after 100 ms; -1 cancels a call rejected outright or after 60 s.
        self.assertEqual(run.stdout.strip(), "100,100,-1,-1")


if __name__ == "__main__":
    unittest.main()
