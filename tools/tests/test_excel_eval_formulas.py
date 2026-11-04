# Copyright 2026 Ryan Duguid
# SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

"""The formula helper has COM retry calls a busy Excel rejects.

A rejected method call throws RPC_E_CALL_REJECTED, but a rejected property read comes
back $null in PowerShell. On 1 November 2026 that made HasFormula read null for a formula
cell, so 3 of 4 runs on an unchanged workbook failed. The helper registers a COM message
filter that retries a rejected call every 100 ms for up to 60 s, and reads HasFormula and
HasSpill again until they come back true or false.

The check that every flag read goes through the filter runs in Rust, in
tools/ozzit/tests/repository/excel_eval_formulas.rs. These 2 run the filter itself
and need Windows PowerShell.
"""

import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HELPER = Path(__file__).resolve().parents[1] / "excel_eval_formulas.ps1"
DUMPER = Path(__file__).resolve().parents[1] / "dump_values.ps1"
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

    @unittest.skipUnless(WINDOWS, "needs Windows PowerShell")
    def test_a_block_is_read_again_until_it_fits_and_fails_if_it_never_does(self):  # pragma: no cover - Windows only; the coverage job runs on Ubuntu
        text = DUMPER.read_text(encoding="utf-8")
        run = powershell(
            function(text, "Invoke-Excel") + "\n" + function(text, "Read-Block") + "\n"
            "function Fake($row, $rows, $columns, $values, $formula) {"
            " [pscustomobject]@{ Row = $row; Column = 1; Rows = [pscustomobject]@{ Count = $rows };"
            " Columns = [pscustomobject]@{ Count = $columns }; Value2 = $values; Formula = $formula } };"
            "function Sheet($answers) {"
            " $script:answers = $answers; $script:reads = 0;"
            " $ws = [pscustomobject]@{};"
            " $ws | Add-Member ScriptProperty UsedRange { $script:reads++; $script:answers[[Math]::Min($script:reads, $script:answers.Count) - 1] };"
            " $ws };"
            "$grid = New-Object 'object[,]' 2,2;"
            "$bad = @($null, (Fake $null 2 2 $grid), (Fake 5 2 2 $null), (Fake 5 2 2 7), (Fake 5 2 3 $grid));"
            "$outside = @(0, -1) | ForEach-Object { Read-Block (Sheet @((Fake $_ 2 2 $grid), (Fake 5 2 2 $grid))); $script:reads };"
            "$offside = Fake 5 2 2 $grid; $offside.Column = 0;"
            "$null = Read-Block (Sheet @($offside, (Fake 5 2 2 $grid))); $columnReads = $script:reads;"
            "$unshaped = @((Fake 5 0 2 $grid), (Fake 5 2 -2 $grid)) | ForEach-Object { $script:reads = 0 ; Read-Block (Sheet @($_, (Fake 5 2 2 $grid))); $script:reads };"
            "$block = Read-Block (Sheet ($bad + (Fake 5 2 2 $grid)));"
            "$took = $script:reads;"
            "$empty = Read-Block (Sheet @(Fake 1 1 1 $null ''));"
            "$stuck = try { Read-Block (Sheet @(Fake 5 2 2 $null)) } catch { $_.Exception.Message };"
            "\"$($block.Row),$($block.Values.GetLength(1)),$took,$($null -eq $empty.Values),$stuck,$(@($outside)[1]),$(@($outside)[3]),$columnReads,$(@($unshaped)[1]),$(@($unshaped)[3])\""
        )
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(run.stdout.strip(), "5,2,6,True,Excel returned values that do not fit the range.,2,2,2,2,2")


if __name__ == "__main__":
    unittest.main()
