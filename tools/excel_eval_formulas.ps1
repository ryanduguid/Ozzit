# Evaluate formulas in desktop Excel against the workbook and write each result as JSON.
#
# Input: a UTF-8 JSON array of {"id": "...", "formula": "=..."}. Each formula is entered with
# Formula2 on a scratch sheet added in memory, so dynamic arrays spill instead of being cut to
# one value by implicit intersection. The workbook opens read-only, closes without saving,
# and its SHA-256 must be unchanged afterwards.
# Output: {"excel": "<version build>", "workbook_sha256": "...", "results": {id: [[...]]}}.
# Numbers use round-trip formatting; an Excel error value is written as "#ERR:<code>".
param([string]$Cases, [string]$Out, [string]$Path = "$PSScriptRoot\..\ozzit.xlsx")

$ErrorActionPreference = 'Stop'
# A Python process launched by PowerShell 7 can inherit its module path. Load the
# module belonging to this PowerShell host so Get-FileHash remains available in 5.1.
Import-Module "$PSHOME\Modules\Microsoft.PowerShell.Utility\Microsoft.PowerShell.Utility.psd1" -ErrorAction Stop
$Path = (Resolve-Path -LiteralPath $Path).Path
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count) {
    throw 'Excel is already running. Close it before running this gate.'
}
$inv = [Globalization.CultureInfo]::InvariantCulture
$before = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLower()
# ForEach-Object unrolls the array: Windows PowerShell 5.1 emits it as one pipeline object.
$caseList = @([IO.File]::ReadAllText($Cases, [Text.Encoding]::UTF8) | ConvertFrom-Json | ForEach-Object { $_ })

# A busy Excel rejects calls with RPC_E_CALL_REJECTED. A method call then throws, as
# Range('A1') did outside Invoke-Excel, but a rejected property read comes back $null:
# on 1 November 2026 HasFormula read null for a formula cell and was right 100 ms later.
# This COM message filter, Microsoft's documented remedy for automation clients, has COM
# retry a rejected call every 100 ms for up to 60 s; Read-Flag below retries a flag that
# still reads null.
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

# A rejected property read comes back $null rather than throwing, so a flag such as
# HasFormula counts only once it reads as true or false.
function Read-Flag([scriptblock]$read) {
    Invoke-Excel { $flag = & $read; if ($flag -isnot [bool]) { throw 'Excel returned no flag.' }; $flag }
}

function Format-Cell($v) {
    if ($null -eq $v) { return 'null' }
    if ($v -is [int]) { return '"#ERR:' + $v + '"' }
    if ($v -is [double]) { return $v.ToString('R', $inv) }
    if ($v -is [bool]) { return $(if ($v) { 'true' } else { 'false' }) }
    return '"' + ([string]$v).Replace('\', '\\').Replace('"', '\"') + '"'
}

# The rows as JSON text, or $null when the first value is null. A string return is safe:
# only an array is flattened on the way out of a function.
function Format-Rows($vals) {
    if ($vals -isnot [Array]) {
        if ($null -eq $vals) { return $null }
        return '[' + (Format-Cell $vals) + ']'
    }
    if ($null -eq $vals.GetValue($vals.GetLowerBound(0), $vals.GetLowerBound(1))) { return $null }
    $rows = @()
    for ($i = $vals.GetLowerBound(0); $i -le $vals.GetUpperBound(0); $i++) {
        $row = @()
        for ($j = $vals.GetLowerBound(1); $j -le $vals.GetUpperBound(1); $j++) { $row += Format-Cell $vals.GetValue($i, $j) }
        $rows += '[' + ($row -join ',') + ']'
    }
    return $rows -join ','
}

$xl = $wb = $ws = $cell = $range = $null
$parts = @()
try {
    [OzzitMessageFilter]::Register()
    $xl = New-Object -ComObject Excel.Application
    # No Excel was running before this line, so the only EXCEL.EXE now is this one. Record
    # its PID beside the output so a caller that times out can stop exactly this process.
    $excelPid = @(Get-Process EXCEL -ErrorAction SilentlyContinue)[0].Id
    [IO.File]::WriteAllText("$Out.pid", [string]$excelPid)
    $xl.Visible = $false
    $xl.DisplayAlerts = $false
    $xl.EnableEvents = $false
    $xl.AutomationSecurity = 3
    $wb = Invoke-Excel { $xl.Workbooks.Open($Path, 0, $true) }
    $ws = Invoke-Excel { $wb.Worksheets.Add() }
    $excel = "{0} build {1}" -f $xl.Version, $xl.Build
    foreach ($case in $caseList) {
        # The comma stops the Range being enumerated into its cells on the way out.
        $cell = Invoke-Excel { , $ws.Range('A1') }
        Invoke-Excel { $ws.Cells.Clear() } | Out-Null
        Invoke-Excel { $cell.Formula2 = $case.formula } | Out-Null
        Invoke-Excel { $xl.Calculate() } | Out-Null
        # Calculate() can return before the result lands. Wait for xlDone; CalculationState
        # comes back as the enum's name, as excel_selftest.ps1 found.
        $deadline = [DateTime]::UtcNow.AddSeconds(60)
        while ((Invoke-Excel { "$($xl.CalculationState)" }) -notin @('xlDone', '0')) {
            if ([DateTime]::UtcNow -gt $deadline) { throw "Excel did not finish calculating $($case.id)." }
            Start-Sleep -Milliseconds 100
        }
        if (-not (Read-Flag { $cell.HasFormula })) { throw "Excel lost the formula for $($case.id)." }
        # A formula's cell is never empty, so a null first value was read before Excel wrote it:
        # on 24 Oct 2026 one run read a blank for a formula that returns text, and the self-test's
        # range read came back null once after xlDone. Keep a result only once 2 reads in a row
        # agree on its shape and every value, so a read taken mid-write is not recorded.
        $text = $null
        for ($try = 1; ; $try++) {
            # Assigned in the branches: an if-expression would enumerate the Range into its cells.
            $range = $cell
            # A null HasSpill read as false would record only the spill's first value.
            if (Read-Flag { $cell.HasSpill }) {
                $range = Invoke-Excel { $spill = $cell.SpillingToRange; if ($null -eq $spill) { throw 'Excel returned no spill range.' }; , $spill }
            }
            # Not through a function: PowerShell flattens a 2-dimensional array on return.
            $vals = $range.Value2
            $now = Format-Rows $vals
            # -ceq: -eq ignores case, so READY and ready would pass as the same read.
            if ($null -ne $now -and $now -ceq $text) { break }
            if ($try -ge 10) { throw "Excel returned no stable value for $($case.id)." }
            $text = $now
            if (-not [object]::ReferenceEquals($range, $cell)) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($range) }
            Start-Sleep -Milliseconds $(if ($null -eq $now) { 200 } else { 50 })
        }
        $parts += '"' + $case.id + '":[' + $text + ']'
        # Every Range left referenced keeps the automation Excel alive after Quit().
        foreach ($com in @($range, $cell)) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com) }
        $range = $cell = $null
    }
    $json = '{"excel":"' + $excel + '","workbook_sha256":"' + $before + '","results":{' + ($parts -join ',') + '}}'
    [IO.File]::WriteAllText($Out, $json, (New-Object Text.UTF8Encoding $false))
}
finally {
    try { if ($wb) { $wb.Close($false) } }
    finally {
        try { if ($xl) { $xl.Quit() } }
        finally {
            foreach ($com in @($range, $cell, $ws, $wb, $xl)) {
                if ($null -ne $com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) {
                    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com)
                }
            }
            $com = $range = $cell = $ws = $wb = $xl = $null
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
            [OzzitMessageFilter]::Revoke()
        }
    }
}
$after = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLower()
if ($after -ne $before) { throw 'Workbook bytes changed during the native evaluation' }
Write-Output "Evaluated $($caseList.Count) formulas in Excel $excel; workbook unchanged: $after"
