# Recalculate the companion workbook and exercise inputs without saving trials.
param(
    [string]$Path = "$PSScriptRoot\..\templates\practice-calculators.xlsx",
    [string]$Out
)
$ErrorActionPreference = 'Stop'
$Path = (Resolve-Path -LiteralPath $Path).Path
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count) {
    throw 'Excel is already running; this test never attaches to a user session.'
}
$before = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
$xl = $wb = $sheet = $null
$script:checks = 0
function Assert-Cell([string]$Address, $Expected) {
    $xl.CalculateFullRebuild()
    $range = $sheet.Range($Address)
    try {
        $actual = $range.Value2
        if ($Expected -is [double]) {
            if ($actual -isnot [double] -or [Math]::Abs($actual - $Expected) -gt 0.000001) {
                throw "$($sheet.Name)!$Address expected $Expected; got $actual"
            }
        } elseif ($Expected -eq '#N/A') {
            if ($actual -isnot [int] -or $range.Text -ne '#N/A') {
                throw "$($sheet.Name)!$Address expected #N/A; got $actual"
            }
        } elseif ($actual -cne $Expected) {
            throw "$($sheet.Name)!$Address expected $Expected; got $actual"
        }
        $script:checks++
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($range) }
}
function Trial([string]$Address, $Value, [scriptblock]$Checks) {
    $range = $sheet.Range($Address)
    $old = $range.Value2
    try {
        if ($null -eq $Value) { $range.ClearContents() | Out-Null }
        elseif ($Value -is [string]) { $range.Value2 = [string]$Value }
        else { $range.Value2 = [double]$Value }
        & $Checks
    } finally {
        if ($old -is [Array]) {
            for ($r = $old.GetLowerBound(0); $r -le $old.GetUpperBound(0); $r++) {
                for ($c = $old.GetLowerBound(1); $c -le $old.GetUpperBound(1); $c++) {
                    $restore = $range.Cells.Item($r, $c)
                    try { $restore.Value2 = [double]$old.GetValue($r, $c) }
                    finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($restore) }
                }
            }
        } elseif ($null -eq $old) { $range.ClearContents() | Out-Null }
        elseif ($old -is [string]) { $range.Value2 = [string]$old }
        else { $range.Value2 = [double]$old }
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($range)
    }
}
function Select-Sheet([string]$Name) {
    if ($script:sheet) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($script:sheet) }
    $script:sheet = $wb.Worksheets.Item($Name)
}
try {
    $xl = New-Object -ComObject Excel.Application
    $xl.Visible = $false
    $xl.DisplayAlerts = $false
    $xl.EnableEvents = $false
    $xl.AutomationSecurity = 3
    $wb = $xl.Workbooks.Open($Path, 0, $true)
    $version = "{0} build {1}" -f $xl.Version, $xl.Build
    Write-Output "Excel $version"
    $xl.CalculateFullRebuild()
    if ($wb.HasVBProject -or $wb.LinkSources(1)) { throw 'Macros or external workbook links found' }
    if ($wb.Worksheets.Count -ne 14) { throw 'Expected the guide and thirteen calculator tabs' }
    $formulaCount = 0
    foreach ($tab in $wb.Worksheets) {
        $used = $tab.UsedRange
        $values = $used.Value2
        $formulas = $used.Formula2
        if ($values -is [Array]) {
            for ($r = $values.GetLowerBound(0); $r -le $values.GetUpperBound(0); $r++) {
                for ($c = $values.GetLowerBound(1); $c -le $values.GetUpperBound(1); $c++) {
                    if ($values.GetValue($r, $c) -is [int]) { throw "Formula error on $($tab.Name), relative row $r column $c" }
                    if ([string]$formulas.GetValue($r, $c) -like '=*') { $formulaCount++ }
                }
            }
        }
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($used)
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($tab)
    }
    Select-Sheet 'Read me'
    if ($sheet.Hyperlinks.Count -ne 13) { throw 'Expected thirteen native contents links' }
    Select-Sheet 'Allocation'
    Assert-Cell 'D9' 'Ready'
    Assert-Cell 'Q35' 12000.0
    Trial 'B14' 'Example*' { Assert-Cell 'D9' 'Ready' }
    Assert-Cell 'N14' (12000.0 / 1.12 / 1.05 / 6)
    Trial 'D8' 1.0 { Assert-Cell 'N14' (12000.0 / 1.12 / 1.05 / 3); Trial 'D14' $null { Assert-Cell 'D9' 'Ready' } }
    Trial 'D8' 2.0 { Assert-Cell 'N14' (12000.0 / 1.12 / 1.05 / 6); Trial 'D14' $null { Assert-Cell 'D9' 'Ready' } }
    Trial 'D14' $null { Assert-Cell 'D9' 'Check inputs'; Assert-Cell 'N14' '#N/A' }
    Trial 'R14' $null { Assert-Cell 'D9' 'Check inputs' }
    Trial 'B15' 'Example 1' { Assert-Cell 'D9' 'Check inputs' }
    Trial 'R14' 100000.0 { Assert-Cell 'S14' '#N/A' }
    Trial 'J6:Q6' 0.0 { Assert-Cell 'D9' 'Zero allocation weight' }

    Select-Sheet 'Contractor'
    Assert-Cell 'D16' 120000.0
    Assert-Cell 'D17' 124000.0
    Assert-Cell 'D19' (124000.0 / 42 / 35)
    Trial 'D11' 0.0 { Assert-Cell 'D14' 'Check inputs'; Assert-Cell 'D18' '#N/A' }
    Trial 'D5' $null { Assert-Cell 'D14' 'Check inputs' }
    Trial 'D5' 0.0 { Assert-Cell 'D17' 24000.0 }

    Select-Sheet 'Vendor finance'
    Assert-Cell 'K11' 93500.0
    $balance = 100000.0
    1..10 | ForEach-Object { $balance = [Math]::Max(0.0, $balance * 1.06 - 12500.0) }
    Assert-Cell 'K20' $balance
    Trial 'D11' 0.0 { Assert-Cell 'K11' 87500.0 }
    Trial 'H11' 100000.0 { Assert-Cell 'K11' 0.0; Assert-Cell 'L11' 4500.0 }
    Trial 'G11' 15000.0 { Assert-Cell 'M11' 'Check inputs'; Assert-Cell 'K11' '#N/A' }
    Trial 'D5' $null { Assert-Cell 'M11' 'Check inputs' }

    Select-Sheet 'Mixed loan'
    Assert-Cell 'D12' 100000.0
    Assert-Cell 'H12' 'Ready'
    Assert-Cell 'R16' 0.0
    Assert-Cell 'R17' (60000.0 * 0.06 * 10 / 365)
    Assert-Cell 'AJ17' 6000.0
    Assert-Cell 'AK17' 4000.0
    Assert-Cell 'AD17' 60.0
    Assert-Cell 'AE17' 40.0
    Assert-Cell 'X18' 54000.0
    Assert-Cell 'Y18' 46000.0
    Trial 'B17' 46204.0 { Assert-Cell 'K17' 0.0; Assert-Cell 'R17' 0.0 }
    Trial 'C17' 200000.0 { Assert-Cell 'AP17' 100000.0; Assert-Cell 'D12' 10000.0 }
    Trial 'D17' -1.0 { Assert-Cell 'AQ17' 'Check inputs'; Assert-Cell 'X17' '#N/A' }
    Trial 'B17' $null { Assert-Cell 'AQ17' 'Check inputs' }
    Trial 'B17' 1.0 { Assert-Cell 'AQ17' 'Check inputs' }
    Trial 'F6:K6' 0.0 { Trial 'J16' 1.0 { Assert-Cell 'AQ16' 'Check inputs'; Assert-Cell 'H12' 'Check events' } }

    Select-Sheet 'Rental'
    Assert-Cell 'D11' 37500.0
    Assert-Cell 'D12' 27500.0
    Assert-Cell 'D39' 22500.0
    Assert-Cell 'E39' 16500.0
    Trial 'B39' 'Example owner *' { Assert-Cell 'D13' 'Ready' }
    Trial 'D10' 1000.0 { Assert-Cell 'D12' 26500.0 }
    Trial 'D10' -500.0 { Assert-Cell 'D12' 28000.0 }
    Trial 'D10' $null { Assert-Cell 'D11' 37500.0; Assert-Cell 'D12' '#N/A' }
    Trial 'D32' $null { Assert-Cell 'D11' 37500.0; Assert-Cell 'D12' '#N/A' }
    Trial 'C39' 0.7 { Assert-Cell 'D13' 'Check shares'; Assert-Cell 'D39' '#N/A' }
    Trial 'C17' $null { Assert-Cell 'D11' '#N/A' }
    Trial 'C32' $null { Assert-Cell 'D11' '#N/A' }
    Trial 'C32' 0.0 { Assert-Cell 'D11' 41500.0 }
    Trial 'D18' 4000.0 { Assert-Cell 'D11' '#N/A' }
    Trial 'D8' 1000.0 { Assert-Cell 'D12' 30000.0 }

    Select-Sheet 'Packaging'
    Assert-Cell 'D12' 75000.0
    Assert-Cell 'D13' 66000.0
    Assert-Cell 'D14' 120000.0
    Assert-Cell 'D15' 116000.0
    Trial 'D8' $null { Assert-Cell 'D11' 'Check inputs'; Assert-Cell 'D13' '#N/A' }
    Trial 'E19' -1.0 { Assert-Cell 'D11' 'Check inputs' }
    Trial 'D19' 110000.0 { Assert-Cell 'D11' 'Check inputs' }

    Select-Sheet 'Amortisation'
    Assert-Cell 'H9' 1000.0
    Assert-Cell 'L9' 1000.0
    Assert-Cell 'M9' 0.0
    Assert-Cell 'H10' 500.0
    Assert-Cell 'J10' 1000.0
    Assert-Cell 'K10' 0.0
    Assert-Cell 'T9' 0.0
    Trial 'E9' 0.0 { Assert-Cell 'U9' 'Check inputs'; Assert-Cell 'H9' '#N/A' }
    Trial 'D9' 6000.0 { Assert-Cell 'U9' 'Check inputs' }
    Trial 'G9' 2027.0 { Assert-Cell 'I9' 4000.0; Assert-Cell 'J9' 0.0 }

    Select-Sheet 'Timeline'
    Assert-Cell 'E10' 273.0
    Assert-Cell 'F10' 92.0
    Assert-Cell 'E11' 365.0
    Assert-Cell 'F11' 273.0
    Trial 'D10' $null { Assert-Cell 'E10' '#N/A' }
    Trial 'D6' 1.0 { Assert-Cell 'F10' '#N/A' }

    Select-Sheet 'Deed register'
    Assert-Cell 'I9' 'Within five years'
    $review = [DateTime]::FromOADate($sheet.Range('D5').Value2)
    Trial 'H9' ($review.AddYears(5).ToOADate()) { Assert-Cell 'I9' 'Within five years' }
    Trial 'H9' ($review.AddYears(5).AddDays(1).ToOADate()) { Assert-Cell 'I9' 'Later' }
    Trial 'H9' ($review.AddDays(-1).ToOADate()) { Assert-Cell 'I9' 'Past vesting date' }
    Trial 'H9' $null { Assert-Cell 'I9' 'Missing date' }

    Select-Sheet 'Carry forward'
    Assert-Cell 'D12' 10000.0
    Assert-Cell 'F18' 5000.0
    Assert-Cell 'F20' 5000.0
    Assert-Cell 'F21' 0.0
    Assert-Cell 'G21' 5000.0
    Assert-Cell 'D14' 0.0
    Trial 'D8' 40000.0 { Assert-Cell 'D14' 5000.0 }
    Trial 'D5' 'No' { Assert-Cell 'D14' 15000.0; Assert-Cell 'F18' 0.0 }
    Trial 'C18' 2026.0 { Assert-Cell 'E18' 0.0; Assert-Cell 'F21' 5000.0 }
    Trial 'B19' 2022.0 { Assert-Cell 'D11' 'Check inputs' }
    Trial 'D8' $null { Assert-Cell 'D11' 'Check inputs'; Assert-Cell 'D14' '#N/A' }

    Select-Sheet 'Loan offers'
    $periodRate = 0.06 / 12
    $payment = 100000.0 * $periodRate / (1 - [Math]::Pow(1 + $periodRate, -60))
    Assert-Cell 'I10' $payment
    Assert-Cell 'K10' 1100.0
    Trial 'B10' 'Example*' { Assert-Cell 'N10' 'Ready' }
    Assert-Cell 'L10' ($payment * 60 + 1100)
    $shockRate = 0.08 / 12
    Assert-Cell 'M10' (100000.0 * $shockRate / (1 - [Math]::Pow(1 + $shockRate, -60)) + 10)
    Trial 'D10' 0.0 {
        Assert-Cell 'I10' (100000.0 / 60)
        Assert-Cell 'J10' 0.0
        Trial 'F10' 20000.0 { Assert-Cell 'I10' (80000.0 / 60); Assert-Cell 'L10' 101100.0 }
    }
    Trial 'E10' 0.0 { Assert-Cell 'N10' 'Check inputs'; Assert-Cell 'I10' '#N/A' }
    Trial 'E10' 60.5 { Assert-Cell 'N10' 'Check inputs' }
    Trial 'F10' 110000.0 { Assert-Cell 'N10' 'Check inputs' }
    Trial 'D5' -0.07 { Assert-Cell 'N10' 'Check inputs' }
    Trial 'B11' 'Example 1' { Assert-Cell 'N10' 'Check inputs' }
    Assert-Cell 'B1' 'Contents'

    Select-Sheet 'Break even'
    Assert-Cell 'D13' 60.0
    Assert-Cell 'D14' 1000.0
    Assert-Cell 'D15' 1500.0
    Assert-Cell 'D16' 30000.0
    Assert-Cell 'D19' 90.0
    Trial 'D7' 100.0 { Assert-Cell 'D12' 'Check costs or inputs'; Assert-Cell 'D14' '#N/A' }
    Trial 'D10' 1400.0 { Assert-Cell 'D18' 'Above capacity' }
    Trial 'D5' 60001.0 { Assert-Cell 'D14' 1001.0 }
    Trial 'D8' 0.0 { Assert-Cell 'D16' -60000.0 }
    Trial 'D10' 0.0 { Assert-Cell 'D18' 'Above capacity'; Assert-Cell 'D19' '#N/A' }

    Select-Sheet 'Working capital'
    Assert-Cell 'D11' (34800000.0 / 365)
    Assert-Cell 'G15' (18000000.0 / 365)
    Assert-Cell 'G17' (10800000.0 / 365)
    Trial 'D15' 60.0 { Assert-Cell 'G15' (-18000000.0 / 365) }
    Trial 'D17' 0.0 { Assert-Cell 'G17' (-21600000.0 / 365) }
    Trial 'D8' 0.0 { Assert-Cell 'D10' 'Check inputs'; Assert-Cell 'D11' '#N/A' }
    Trial 'D5' $null { Assert-Cell 'D10' 'Check inputs' }
    Trial 'D5' 0.0 { Assert-Cell 'G15' 0.0 }
    Trial 'C15' -1.0 { Assert-Cell 'D10' 'Check inputs' }
    Write-Output "PASS: $script:checks assertions; $formulaCount baseline formulas; zero baseline error cells"
} finally {
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
if ($Out) {
    $runnerHash = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLower()
    $builder = Join-Path $PSScriptRoot 'build_practice_calculators.mjs'
    $builderHash = (Get-FileHash -LiteralPath $builder -Algorithm SHA256).Hash.ToLower()
    $preparerHash = (Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'prepare_practice_template.ps1') -Algorithm SHA256).Hash.ToLower()
    @{excel=$version; assertions=$script:checks; formulas=$formulaCount; baseline_errors=0;
        workbook_sha256=$before.ToLower(); runner_sha256=$runnerHash; builder_sha256=$builderHash; preparer_sha256=$preparerHash;
        workflows=@('Allocation','Contractor','Vendor finance','Mixed loan','Rental','Packaging','Amortisation','Timeline','Deed register','Carry forward','Loan offers','Break even','Working capital'); result='PASS'} |
        ConvertTo-Json | Set-Content -LiteralPath $Out -Encoding UTF8
}
