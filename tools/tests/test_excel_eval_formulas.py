"""The formula helper has COM retry calls a busy Excel rejects.

A rejected method call throws RPC_E_CALL_REJECTED, but a rejected property read comes
back $null in PowerShell. On 1 November 2026 that made HasFormula read null for a formula
cell, so 3 of 4 runs on an unchanged workbook failed. The helper registers a COM message
filter that retries a rejected call every 100 ms for up to 60 s, and reads HasFormula and
HasSpill again until they come back true or false.
"""

import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HELPER = Path(__file__).resolve().parents[1] / "excel_eval_formulas.ps1"
WINDOWS = sys.platform == "win32" and shutil.which("powershell.exe")


def filter_source(text: str) -> str:  # pragma: no cover - Windows only; the coverage job runs on Ubuntu
    start = text.index("Add-Type -TypeDefinition @'\n") + len("Add-Type -TypeDefinition @'\n")
    return text[start:text.index("\n'@", start)]


def function(text: str, name: str) -> str:  # pragma: no cover - Windows only; the coverage job runs on Ubuntu
    """A top-level function's definition, from its first line to its closing brace."""
    found = re.search(rf"^function {name}\b.*?^\}}$", text, re.MULTILINE | re.DOTALL)
    if found is None:
        raise AssertionError(f"{name} is not defined")
    return found.group(0)


def powershell(script: str) -> subprocess.CompletedProcess[str]:  # pragma: no cover - Windows only; the coverage job runs on Ubuntu
    return subprocess.run(
        ["powershell.exe", "-NoProfile", "-NonInteractive", "-STA", "-Command", script],
        capture_output=True, text=True, timeout=120, check=False,
    )


class MessageFilterTests(unittest.TestCase):
    def test_the_filter_covers_every_excel_call_and_is_revoked_last(self):
        text = HELPER.read_text(encoding="utf-8")
        # The main try opens with the filter, so the finally that quits Excel also revokes it.
        main = text.index("try {\n    [OzzitMessageFilter]::Register()\n")
        self.assertLess(main, text.index("New-Object -ComObject Excel.Application"))
        self.assertLess(text.rindex("finally {"), text.index("[OzzitMessageFilter]::Revoke()"))
        for flag in ("HasFormula", "HasSpill"):
            with self.subTest(flag=flag):
                # Every read of the flag goes through Read-Flag.
                self.assertGreater(text.count(f"Read-Flag {{ $cell.{flag} }}"), 0)
                self.assertEqual(
                    text.count(f"$cell.{flag}"), text.count(f"Read-Flag {{ $cell.{flag} }}")
                )

    @unittest.skipUnless(WINDOWS, "needs Windows PowerShell")
    def test_only_a_retry_later_rejection_is_retried_within_60_seconds(self):  # pragma: no cover - Windows only; the coverage job runs on Ubuntu
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / "filter.cs"
            source.write_text(filter_source(HELPER.read_text(encoding="utf-8")), encoding="utf-8")
            run = powershell(
                f"Add-Type -TypeDefinition ([IO.File]::ReadAllText('{source}'));"
                "[OzzitMessageFilter]::Revoke();"  # before Register: must leave the thread alone
                "[OzzitMessageFilter]::Register();"
                "$f = New-Object OzzitMessageFilter;"
                "$z = [IntPtr]::Zero;"
                "($f.RetryRejectedCall($z, 0, 2), $f.RetryRejectedCall($z, 59999, 2),"
                " $f.RetryRejectedCall($z, 60000, 2), $f.RetryRejectedCall($z, 0, 1)) -join ',';"
                "[OzzitMessageFilter]::Revoke()"
            )
        self.assertEqual(run.returncode, 0, run.stderr)
        # 100 retries after 100 ms; -1 cancels a call rejected outright or after 60 s.
        self.assertEqual(run.stdout.strip(), "100,100,-1,-1")

    @unittest.skipUnless(WINDOWS, "needs Windows PowerShell")
    def test_a_null_flag_is_read_again_rather_than_taken_as_false(self):  # pragma: no cover - Windows only; the coverage job runs on Ubuntu
        text = HELPER.read_text(encoding="utf-8")
        run = powershell(
            function(text, "Invoke-Excel") + "\n" + function(text, "Read-Flag") + "\n"
            "$script:reads = 0;"
            "$late = Read-Flag { $script:reads++; if ($script:reads -le 2) { $null } else { $true } };"
            "$no = Read-Flag { $false };"
            "\"$late,$($script:reads),$no\""
        )
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(run.stdout.strip(), "True,3,False")


if __name__ == "__main__":
    unittest.main()
