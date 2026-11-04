# Copyright 2026 Ryan Duguid
# SPDX-License-Identifier: LicenseRef-Duguid-Public-Attribution-1.0

# Write every calculated cell value in a workbook to a tab-separated file, so a checker can
# compare what Excel produces against what the file has cached. Columns are
# sheet, row, column, value kind (error/value), then the newline-folded value.
#
# Cells are keyed by tab position rather than by $ws.Index, which intermittently returns an
# empty value over COM and silently attributes a sheet's cells to the wrong sheet.
param([string]$Path, [string]$Out)

$ErrorActionPreference = 'Stop'
$Path = (Resolve-Path $Path).Path

# A busy Excel rejects calls with RPC_E_CALL_REJECTED. A method call then throws, but a
# rejected property read comes back $null, and a null row or column turns the cell's
# position into 0, which `ozzit verify_cache` refuses. This is the COM message filter in
# excel_eval_formulas.ps1: COM retries a rejected call every 100 ms for up to 60 s.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

[ComImport, Guid("00000016-0000-0000-C000-000000000046"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IOzzitOleMessageFilter {
    [PreserveSig] int HandleInComingCall(int callType, IntPtr caller, int tickCount, IntPtr interfaceInfo);
    [PreserveSig] int RetryRejectedCall(IntPtr callee, int tickCount, int rejectType);
    [PreserveSig] int MessagePending(IntPtr callee, int tickCount, int pendingType);
}

public class OzzitMessageFilter : IOzzitOleMessageFilter {
    [DllImport("ole32.dll")]
    static extern int CoRegisterMessageFilter(IOzzitOleMessageFilter filter, out IOzzitOleMessageFilter previous);

    static IOzzitOleMessageFilter previous;
    static bool registered;

    // Fails outside a single-threaded apartment, where a filter cannot apply.
    public static void Register() {
        Marshal.ThrowExceptionForHR(CoRegisterMessageFilter(new OzzitMessageFilter(), out previous));
        registered = true;
    }

    // Restores the thread's earlier filter, if any. Not checked: it runs last in the cleanup,
    // where a throw would hide the error that ended the run.
    public static void Revoke() {
        if (!registered) return;
        IOzzitOleMessageFilter ours;
        CoRegisterMessageFilter(previous, out ours);
        previous = null;
        registered = false;
    }

    // SERVERCALL_ISHANDLED
    public int HandleInComingCall(int callType, IntPtr caller, int tickCount, IntPtr interfaceInfo) { return 0; }

    // Retry SERVERCALL_RETRYLATER after 100 ms until 60 s have passed; -1 cancels the call.
    public int RetryRejectedCall(IntPtr callee, int tickCount, int rejectType) {
        return rejectType == 2 && tickCount < 60000 ? 100 : -1;
    }

    // PENDINGMSG_WAITDEFPROCESS
    public int MessagePending(IntPtr callee, int tickCount, int pendingType) { return 2; }
}
'@

function Invoke-Excel([scriptblock]$op, [int]$tries = 12) {
    for ($i = 1; $i -le $tries; $i++) {
        try { return & $op }
        catch { if ($i -eq $tries) { throw }; Start-Sleep -Milliseconds (250 * $i) }
    }
}

# A rejected property read comes back $null rather than throwing, so a row or column
# counts only once it reads as a number.
function Read-Position([scriptblock]$read) {
    Invoke-Excel { $position = & $read; if ($position -isnot [int]) { throw 'Excel returned no position.' }; $position }
}

for ($w = 0; $w -lt 10 -and @(Get-Process EXCEL -ErrorAction SilentlyContinue).Count -gt 0; $w++) {
    Start-Sleep -Milliseconds 500
}
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count -gt 0) {
    Write-Output 'Excel is already running. Close it first: this drives Excel over COM and quits it when done.'
    exit 2
}

[OzzitMessageFilter]::Register()
$xl = New-Object -ComObject Excel.Application
# msoAutomationSecurityForceDisable (3), not Low (1): reading values needs no macros.
$xl.Visible = $false; $xl.DisplayAlerts = $false; $xl.AutomationSecurity = 3
$wb = $null; $sw = $null; $exit = 0
try {
    # Inside the try: a bad -Out path must still reach the finally that quits Excel.
    $sw = [System.IO.StreamWriter]::new($Out, $false, (New-Object System.Text.UTF8Encoding $false))
    $wb = Invoke-Excel { $xl.Workbooks.Open($Path, 0, $false) }
    Invoke-Excel { $xl.CalculateFullRebuild() }
    # CalculationState comes back over COM as the enum's NAME, "xlDone", not as 0, so a
    # test against 0 is true forever and the wait below never ends early. Excel very
    # occasionally reports itself still busy after a rebuild that has in fact finished, so
    # ask a second time before giving up rather than failing a run for a transient state.
    $settled = $false
    for ($attempt = 1; $attempt -le 2 -and -not $settled; $attempt++) {
        for ($k = 0; $k -lt 300 -and "$($xl.CalculationState)" -notin @('xlDone', '0'); $k++) {
            Start-Sleep -Milliseconds 50
        }
        $settled = "$($xl.CalculationState)" -in @('xlDone', '0')
        if (-not $settled) { Invoke-Excel { $xl.CalculateFullRebuild() } }
    }
    if (-not $settled) { throw 'Excel did not finish calculating within 30 seconds' }
    $pos = 0
    foreach ($ws in $wb.Worksheets) {
        $pos++
        $r0 = Read-Position { $ws.UsedRange.Row }
        $c0 = Read-Position { $ws.UsedRange.Column }
        $ur = $ws.UsedRange
        # Not Invoke-Excel: PowerShell flattens a 2-dimensional array returned through a
        # function, which turns the whole sheet into one row and hides every difference.
        $vals = $ur.Value2
        if ($null -eq $vals) { continue }
        if ($vals -isnot [object[,]]) {
            $kind = if ($vals -is [int]) { 'error' } else { 'value' }
            $sw.WriteLine("$pos`t$r0`t$c0`t$kind`t" + (([string]$vals) -replace "`r`n", '\n' -replace "`n", '\n' -replace "`r", '\n'))
            continue
        }
        for ($i = 1; $i -le $vals.GetLength(0); $i++) {
            for ($j = 1; $j -le $vals.GetLength(1); $j++) {
                $v = $vals[$i, $j]
                if ($null -eq $v) { continue }
                # COM errors are Int32; an ordinary number with the same text is Double.
                $kind = if ($v -is [int]) { 'error' } else { 'value' }
                # A cell may hold newlines. Fold them so one cell stays one line.
                $s = ([string]$v) -replace "`r`n", '\n' -replace "`n", '\n' -replace "`r", '\n'
                $sw.WriteLine("$pos`t" + ($r0 + $i - 1) + "`t" + ($c0 + $j - 1) + "`t$kind`t" + $s)
            }
        }
    }
    Invoke-Excel { $wb.Close($false) }; $wb = $null
}
catch { Write-Output ('FAIL: ' + $_.Exception.Message); $exit = 1 }
finally {
    if ($sw) { $sw.Close() }
    if ($wb) { try { $wb.Close($false) } catch {} }
    try { $xl.Quit() } catch {}
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($xl)
    [GC]::Collect(); [GC]::WaitForPendingFinalizers()
    [OzzitMessageFilter]::Revoke()
}
exit $exit
