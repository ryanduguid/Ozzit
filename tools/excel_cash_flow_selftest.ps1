# Exercise the cash-flow template in Excel, closing without saving any test inputs.
param([string]$Path = "$PSScriptRoot\..\templates\13-week-cash-flow-forecast.xlsx")

$ErrorActionPreference = 'Stop'
$Path = (Resolve-Path -LiteralPath $Path).Path
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count) {
    throw 'Excel is already running. Close it before running this gate.'
}
$before = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
$xl = $wb = $review = $dashboard = $null
$checks = 0
function Check-Bias($label, $count, $bias) {
    $xl.CalculateFullRebuild()
    $actualCount = $dashboard.Range('A33').Value2
    $actualBias = $dashboard.Range('D33').Value2
    if ($actualCount -ne $count -or
        ($null -eq $bias -and $actualBias -ne '') -or
        ($null -ne $bias -and ($actualBias -isnot [double] -or [Math]::Abs($actualBias - $bias) -gt 0.000001))) {
        throw "$label`: expected count $count and bias [$bias], got $actualCount and [$actualBias]"
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
    $review = $wb.Worksheets.Item('Weekly Review')
    $dashboard = $wb.Worksheets.Item('Dashboard')
    Write-Output ("Excel {0} build {1}" -f $xl.Version, $xl.Build)
    $review.Range('D6:D18').ClearContents()
    $review.Range('H6:H18').ClearContents()
    $review.Range('L6:L18').ClearContents()
    Check-Bias 'No actuals' 0 $null

    $review.Range('K6').Value2 = 100.0
    $review.Range('L6').Value2 = 110.0
    Check-Bias 'Only a closing actual' 0 $null
    $review.Range('D6').Value2 = 0.0
    $review.Range('H6').Value2 = 0.0
    Check-Bias 'Zero receipts and payments are valid actuals' 1 10.0

    $review.Range('K7').Value2 = 100.0
    $review.Range('L7').Value2 = 130.0
    Check-Bias 'A partial second week must not enter the average' 1 10.0
    $review.Range('D7').Value2 = 100.0
    Check-Bias 'Payments still missing' 1 10.0
    $review.Range('H7').Value2 = 100.0
    Check-Bias 'Two complete actual weeks' 2 20.0
    $review.Range('L7').Value2 = 70.0
    Check-Bias 'Positive and negative variances' 2 -10.0
    $review.Range('D7').ClearContents()
    Check-Bias 'Removing an actual removes its variance' 1 10.0
    Write-Output ("PASS: {0} cash-flow assertions" -f $checks)
}
finally {
    try { if ($wb) { $wb.Close($false) } }
    finally {
        try { if ($xl) { $xl.Quit() } }
        finally {
            foreach ($com in @($dashboard, $review, $wb, $xl)) {
                if ($null -ne $com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) {
                    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com)
                }
            }
            $com = $dashboard = $review = $wb = $xl = $null
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
        }
    }
    $after = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
    if ($after -ne $before) { throw 'Workbook bytes changed during the native test' }
    Write-Output "Workbook unchanged: $after"
}
