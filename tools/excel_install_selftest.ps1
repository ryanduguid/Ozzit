# Test the documented copy-cell route in a fresh workbook using desktop Excel.
# The source opens read-only. Only the new destination workbook is saved.
param(
    [Parameter(Mandatory=$true)][string]$Path,
    [Parameter(Mandatory=$true)][ValidatePattern('^[a-f0-9]{64}$')][string]$ExpectedSha256,
    [Parameter(Mandatory=$true)][string]$OutDirectory
)

$ErrorActionPreference = 'Stop'
Import-Module "$PSHOME\Modules\Microsoft.PowerShell.Utility\Microsoft.PowerShell.Utility.psd1" -ErrorAction Stop
$Path = (Resolve-Path -LiteralPath $Path).Path
$before = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
if ($before -ne $ExpectedSha256) { throw 'Source workbook hash does not match ExpectedSha256.' }
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count) { throw 'Excel is already running. Close it before running this gate.' }
$OutDirectory = [IO.Path]::GetFullPath($OutDirectory)
if (Test-Path -LiteralPath $OutDirectory) { throw 'OutDirectory must not already exist.' }
$repo = [IO.Path]::GetFullPath("$PSScriptRoot\..")
if ($OutDirectory.StartsWith($repo + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'OutDirectory must be outside the source checkout.'
}
$parent = Split-Path -Parent $OutDirectory
if (-not (Test-Path -LiteralPath $parent -PathType Container)) { throw 'The output parent must exist.' }
New-Item -ItemType Directory -Path $OutDirectory | Out-Null
$destination = Join-Path $OutDirectory 'installed.xlsx'
$lambda = [char]0x3bb
$cases = @(
    @{ id='rolling-sum'; sheet="oz.RollingSum$lambda"; cell='H21'; target='A1';
       formula="=oz.RollingSum$lambda({1,2,3,4},2)"; changed="=oz.RollingSum$lambda({2,4,6,8},2)";
       required=@("oz.RollingSum$lambda") },
    @{ id='amortise-helpers'; sheet="oz.Amortise$lambda"; cell='I24'; target='A10';
       formula="=oz.Amortise$lambda(1000,0.06,4,DATE(2026,7,1))";
       changed="=oz.Amortise$lambda(2000,0.06,4,DATE(2026,7,1))";
       required=@("oz.Amortise$lambda", "oz.TimelineOffset$lambda", "oz.TimelinePosition$lambda") }
)

function Read-Grid($sheet, [string]$address) {
    $cell = $range = $null
    try {
        $cell = $sheet.Range($address)
        $range = $cell
        if ($cell.HasSpill) { $range = $cell.SpillingToRange }
        $raw = $range.Value2
        $values = @($raw)
        foreach ($value in $values) {
            if ($null -eq $value -or $value -isnot [double] -or [double]::IsNaN($value) -or [double]::IsInfinity($value)) {
                throw "Unexpected non-numeric or error value at $address`: $value"
            }
        }
        return @{ rows=$range.Rows.Count; columns=$range.Columns.Count; values=$values }
    }
    finally {
        foreach ($com in @($range, $cell)) { if ($com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com) } }
    }
}

function Assert-Grid($actual, $expected, [string]$label) {
    if ($actual.rows -ne $expected.rows -or $actual.columns -ne $expected.columns) { throw "$label`: spill shape changed." }
    for ($i = 0; $i -lt $expected.values.Count; $i++) {
        if ([Math]::Abs($actual.values[$i] - $expected.values[$i]) -gt 0.000001) { throw "$label`: value $i differs." }
    }
}

$xl = $source = $fresh = $scratch = $sheet = $copySheet = $from = $to = $null
$records = @()
try {
    $xl = New-Object -ComObject Excel.Application
    $xl.Visible = $false
    $xl.DisplayAlerts = $false
    $xl.EnableEvents = $false
    $xl.AutomationSecurity = 3
    $excelPid = @(Get-Process EXCEL -ErrorAction SilentlyContinue)[0].Id
    [IO.File]::WriteAllText((Join-Path $OutDirectory 'excel.pid'), [string]$excelPid)
    $excel = "$($xl.Version) build $($xl.Build)"
    $source = $xl.Workbooks.Open($Path, 0, $true)
    $scratch = $source.Worksheets.Add()
    $fresh = $xl.Workbooks.Add()
    $sheet = $fresh.Worksheets.Item(1)
    foreach ($case in $cases) {
        $copySheet = $source.Worksheets.Item($case.sheet)
        $from = $copySheet.Range($case.cell)
        $to = $sheet.Range($case.target)
        $from.Copy($to) | Out-Null
        foreach ($com in @($from, $to, $copySheet)) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com) }
        $from = $to = $copySheet = $null
        foreach ($variant in @('formula', 'changed')) {
            $scratch.Cells.Clear() | Out-Null
            $scratch.Range('A1').Formula2 = $case[$variant]
            $xl.CalculateFullRebuild()
            $case["expected_$variant"] = Read-Grid $scratch 'A1'
        }
        $sheet.Range($case.target).Formula2 = $case.formula
    }
    $source.Close($false)
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($scratch)
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($source)
    $scratch = $source = $null
    $xl.CalculateFullRebuild()
    foreach ($case in $cases) { Assert-Grid (Read-Grid $sheet $case.target) $case.expected_formula "$($case.id) source closed" }
    $fresh.SaveAs($destination, 51)
    $fresh.Close($false)
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($sheet)
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($fresh)
    $fresh = $sheet = $null
    $fresh = $xl.Workbooks.Open($destination, 0, $false)
    $sheet = $fresh.Worksheets.Item(1)
    $xl.CalculateFullRebuild()
    foreach ($case in $cases) {
        $reopened = Read-Grid $sheet $case.target
        Assert-Grid $reopened $case.expected_formula "$($case.id) reopened"
        foreach ($name in $case.required) {
            $defined = $fresh.Names.Item($name)
            if ($defined.RefersTo -match '#REF!|\[.*\.xlsx\]') { throw "$name retains an invalid or external reference." }
            [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($defined)
        }
        $sheet.Range($case.target).Formula2 = $case.changed
        $xl.CalculateFullRebuild()
        $changed = Read-Grid $sheet $case.target
        Assert-Grid $changed $case.expected_changed "$($case.id) changed input"
        $records += @{id=$case.id; copied_from="$($case.sheet)!$($case.cell)"; formula=$case.formula;
                      changed_formula=$case.changed; required_names=$case.required;
                      reopened=$reopened; changed_input=$changed; passed=$true}
    }
    $links = @($fresh.LinkSources(1) | Where-Object { $null -ne $_ })
    if ($links.Count) { throw 'The destination retains an external workbook link.' }
    # Keep the saved workbook at the verified initial inputs.
    $fresh.Close($false)
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($fresh)
    $fresh = $null
}
finally {
    try { if ($source) { $source.Close($false) }; if ($fresh) { $fresh.Close($false) } }
    finally {
        try { if ($xl) { $xl.Quit() } }
        finally {
            foreach ($com in @($from, $to, $copySheet, $sheet, $scratch, $fresh, $source, $xl)) {
                if ($com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com) }
            }
            $com = $from = $to = $copySheet = $sheet = $scratch = $fresh = $source = $xl = $null
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
        }
    }
}
$after = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
if ($after -ne $before) { throw 'Source workbook bytes changed.' }
$result = @{excel=$excel; source_sha256=$before; source_unchanged=$true;
            runner_sha256=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant();
            destination_sha256=(Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant();
            external_links=0; cases=$records; passed=$true}
$result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutDirectory 'evidence.json') -Encoding UTF8
Write-Output "PASS: $($records.Count) copy-cell cases, helpers, recalculation and save/reopen; Excel $excel; source unchanged."
