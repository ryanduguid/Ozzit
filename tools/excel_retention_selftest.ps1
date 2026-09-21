# Test the retention template in native Excel without saving the trial inputs.
param([string]$Path = "$PSScriptRoot\..\templates\retention-reconciliation.xlsx")
$ErrorActionPreference = 'Stop'
$Path = (Resolve-Path -LiteralPath $Path).Path
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count) {
    throw 'Excel is already running; this gate never closes or attaches to a user session.'
}
$before = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
$xl = $wb = $sheet = $null
$script:checks = 0
function Assert-Cell($address, $expected) {
    $xl.CalculateFullRebuild()
    $actual = $sheet.Range($address).Value2
    if ($expected -is [double]) {
        if ($actual -isnot [double] -or [Math]::Abs($actual - $expected) -gt 0.000001) {
            throw "$address expected $expected; got $actual"
        }
    } elseif ($actual -ne $expected) {
        throw "$address expected $expected; got $actual"
    }
    $script:checks++
}
try {
    $xl = New-Object -ComObject Excel.Application
    $xl.Visible = $false
    $xl.DisplayAlerts = $false
    $xl.EnableEvents = $false
    $xl.AutomationSecurity = 3
    $wb = $xl.Workbooks.Open($Path, 0, $true)
    $sheet = $wb.Worksheets.Item('Retention reconciliation')
    Write-Output ("Excel {0} build {1}" -f $xl.Version, $xl.Build)
    Assert-Cell 'E14' 58000.0
    Assert-Cell 'G10' 250.0
    Assert-Cell 'J4' 1.0
    $sheet.Range('F11').Value2 = 8250.0
    Assert-Cell 'G14' 0.0
    Assert-Cell 'J4' 2.0
    $sheet.Range('F11').Value2 = 8000.0
    $sheet.Range('F10').Value2 = 50000.0
    Assert-Cell 'J10' 'Tied'
    Assert-Cell 'J4' 0.0
    $sheet.Range('C10').ClearContents()
    Assert-Cell 'E10' 'Missing input'
    Assert-Cell 'E14' 'Missing input'
    Assert-Cell 'J5' 1.0
    $sheet.Range('C10').Value2 = 0.0
    Assert-Cell 'E10' 45000.0
    Assert-Cell 'J10' 'Review'
    $sheet.Range('C10').Value2 = 5000.0
    $sheet.Range('H10').ClearContents()
    Assert-Cell 'J10' 'Missing evidence'
    $sheet.Range('H10').Value2 = 'Fabricated cert A'
    $sheet.Range('A11').Value2 = 'JOB-01'
    Assert-Cell 'J10' 'Duplicate contract'
    Assert-Cell 'J11' 'Duplicate contract'
    $sheet.Range('A11').Value2 = 'JOB-02'
    $sheet.Range('B6').ClearContents()
    Assert-Cell 'J10' 'Missing input'
    $sheet.Range('B6').Value2 = 0.0
    Assert-Cell 'J10' 'Tied'
    $sheet.Range('B4').Value2 = 50000.0
    Assert-Cell 'J10' 'Missing input'
    Write-Output ("PASS: {0} retention assertions" -f $script:checks)
}
finally {
    try { if ($wb) { $wb.Close($false) } }
    finally {
        try { if ($xl) { $xl.Quit() } }
        finally {
            foreach ($com in @($sheet, $wb, $xl)) {
                if ($null -ne $com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) {
                    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com)
                }
            }
            $com = $sheet = $wb = $xl = $null
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
        }
    }
}
$after = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
if ($after -ne $before) { throw 'Workbook changed during native tests' }
Write-Output "Workbook unchanged: $after"
