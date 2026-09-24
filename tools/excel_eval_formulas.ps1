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
$Path = (Resolve-Path -LiteralPath $Path).Path
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count) {
    throw 'Excel is already running. Close it before running this gate.'
}
$inv = [Globalization.CultureInfo]::InvariantCulture
$before = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLower()
# ForEach-Object unrolls the array: Windows PowerShell 5.1 emits it as one pipeline object.
$caseList = @([IO.File]::ReadAllText($Cases, [Text.Encoding]::UTF8) | ConvertFrom-Json | ForEach-Object { $_ })

function Invoke-Excel([scriptblock]$op, [int]$tries = 12) {
    for ($i = 1; $i -le $tries; $i++) {
        try { return & $op }
        catch { if ($i -eq $tries) { throw }; Start-Sleep -Milliseconds (250 * $i) }
    }
}

function Format-Cell($v) {
    if ($null -eq $v) { return 'null' }
    if ($v -is [int]) { return '"#ERR:' + $v + '"' }
    if ($v -is [double]) { return $v.ToString('R', $inv) }
    if ($v -is [bool]) { return $(if ($v) { 'true' } else { 'false' }) }
    return '"' + ([string]$v).Replace('\', '\\').Replace('"', '\"') + '"'
}

$xl = $wb = $ws = $cell = $range = $null
$parts = @()
try {
    $xl = New-Object -ComObject Excel.Application
    $xl.Visible = $false
    $xl.DisplayAlerts = $false
    $xl.EnableEvents = $false
    $xl.AutomationSecurity = 3
    $wb = Invoke-Excel { $xl.Workbooks.Open($Path, 0, $true) }
    $ws = Invoke-Excel { $wb.Worksheets.Add() }
    $excel = "{0} build {1}" -f $xl.Version, $xl.Build
    foreach ($case in $caseList) {
        $cell = $ws.Range('A1')
        $ws.Cells.Clear() | Out-Null
        Invoke-Excel { $cell.Formula2 = $case.formula } | Out-Null
        Invoke-Excel { $xl.Calculate() } | Out-Null
        # Assigned in the branches: an if-expression would enumerate the Range into its cells.
        $range = $cell
        if ($cell.HasSpill) { $range = $cell.SpillingToRange }
        # Not through a function: PowerShell flattens a 2-dimensional array on return.
        $vals = $range.Value2
        $rows = @()
        if ($vals -is [Array]) {
            for ($i = $vals.GetLowerBound(0); $i -le $vals.GetUpperBound(0); $i++) {
                $row = @()
                for ($j = $vals.GetLowerBound(1); $j -le $vals.GetUpperBound(1); $j++) { $row += Format-Cell $vals.GetValue($i, $j) }
                $rows += '[' + ($row -join ',') + ']'
            }
        }
        else { $rows += '[' + (Format-Cell $vals) + ']' }
        $parts += '"' + $case.id + '":[' + ($rows -join ',') + ']'
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
        }
    }
}
$after = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLower()
if ($after -ne $before) { throw 'Workbook bytes changed during the native evaluation' }
Write-Output "Evaluated $($caseList.Count) formulas in Excel $excel; workbook unchanged: $after"
