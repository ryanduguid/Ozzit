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
function Assert-Colours([string]$Address, [int]$Font, [int]$Fill) {
    $xl.CalculateFullRebuild()
    $range = $sheet.Range($Address)
    $display = $fontObject = $fillObject = $null
    try {
        $display = $range.DisplayFormat
        $fontObject = $display.Font; $fillObject = $display.Interior
        if ([int]$fontObject.Color -ne $Font -or [int]$fillObject.Color -ne $Fill) {
            throw "$($sheet.Name)!$Address has incorrect displayed warning/completion colours"
        }
        $script:checks += 2
    } finally {
        foreach ($com in @($fontObject, $fillObject, $display, $range)) {
            if ($com) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com) }
        }
    }
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
                    try { Write-TrialInput $restore $old.GetValue($r, $c) }
                    finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($restore) }
                }
            }
        } elseif ($null -eq $old) { $range.ClearContents() | Out-Null }
        elseif ($old -is [string]) { $range.Value2 = [string]$old }
        else { $range.Value2 = [double]$old }
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($range)
    }
}
# Writes only unsaved inputs in the read-only test workbook.
function Write-TrialInput($Range, $Value) {
    if ($null -eq $Value) { $Range.ClearContents() | Out-Null }
    elseif ($Value -is [string]) { $Range.Value2 = [string]$Value }
    else { $Range.Value2 = [double]$Value }
}
function Trial-Cells([hashtable]$Values, [scriptblock]$Checks) {
    $saved = @{}
    try {
        foreach ($address in $Values.Keys) {
            $range = $sheet.Range($address)
            try { $saved[$address] = $range.Value2; Write-TrialInput $range $Values[$address] }
            finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($range) }
        }
        & $Checks
    } finally {
        foreach ($address in $saved.Keys) {
            $range = $sheet.Range($address)
            try { Write-TrialInput $range $saved[$address] }
            finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($range) }
        }
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
    if ($wb.Worksheets.Count -ne 22) { throw 'Expected the guide and twenty-one calculator tabs' }
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
    if ($sheet.Hyperlinks.Count -ne 21) { throw 'Expected twenty-one native contents links' }
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
    Trial 'AR16' 1000.0 { Assert-Cell 'D12' 101000.0 }
    Trial-Cells @{AR16=1000.0;AS16=-1000.0} {
        Assert-Cell 'D12' 100000.0
        Assert-Cell 'R17' (61000.0 * 0.06 * 10 / 365)
        Assert-Cell 'S17' (39000.0 * 0.06 * 10 / 365)
        Assert-Cell 'AJ17' 6100.0
        Assert-Cell 'AK17' 3900.0
    }
    Trial 'AR16' -60001.0 { Assert-Cell 'AQ16' 'Check inputs'; Assert-Cell 'AQ17' 'Check inputs'; Assert-Cell 'D12' '#N/A' }
    Trial 'AR16' $null { Assert-Cell 'AQ16' 'Check inputs' }
    Trial 'AR16' 'invalid' { Assert-Cell 'AQ16' 'Check inputs'; Assert-Cell 'H12' 'Check events' }
    Trial 'AR51' -1000.0 { Assert-Cell 'D12' 99000.0 }

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
    $packEnd = [DateTime]::new(2027,6,30)
    Assert-Cell 'F27' 'Active'
    Assert-Cell 'E27' (($packEnd - [DateTime]::new(2026,10,19)).TotalDays)
    Trial 'H11' ([DateTime]::new(2026,6,30).ToOADate()) { Assert-Cell 'F27' 'Not started' }
    Trial 'H11' ([DateTime]::new(2026,7,1).ToOADate()) { Assert-Cell 'F27' 'Active' }
    Trial 'H11' ($packEnd.AddDays(-31).ToOADate()) { Assert-Cell 'F27' 'Active' }
    Trial 'H11' ($packEnd.AddDays(-30).ToOADate()) { Assert-Cell 'F27' 'Ends within 30 days' }
    Trial 'H11' ($packEnd.ToOADate()) { Assert-Cell 'F27' 'Ends within 30 days'; Assert-Cell 'E27' 0.0 }
    Trial 'H11' ($packEnd.AddDays(1).ToOADate()) { Assert-Cell 'F27' 'Cease date passed'; Assert-Cell 'D15' 116000.0 }
    Trial 'C27' $null { Assert-Cell 'F27' 'Check dates'; Assert-Cell 'E27' '#N/A' }
    Trial 'C27' 'invalid' { Assert-Cell 'F27' 'Check dates' }
    Trial 'C27' ($packEnd.AddDays(1).ToOADate()) { Assert-Cell 'F27' 'Check dates' }

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
    Trial 'V9' 0.5 { Assert-Cell 'H9' 500.0; Assert-Cell 'L9' 1000.0; Assert-Cell 'M9' 500.0; Assert-Cell 'N9' 0.0; Assert-Cell 'T9' 0.0 }
    Trial 'V9' 0.0 { Assert-Cell 'H9' 0.0; Assert-Cell 'M9' 1000.0; Assert-Cell 'N9' 0.0 }
    Trial-Cells @{V9=0.5;G9=2027.0} { Assert-Cell 'H9' 500.0; Assert-Cell 'I9' 4500.0; Assert-Cell 'T9' 0.0 }
    Trial-Cells @{V9=0.5;D9=1000.0} { Assert-Cell 'H9' 400.0; Assert-Cell 'M9' 400.0; Assert-Cell 'T9' 1000.0 }
    Trial 'V9' 1.01 { Assert-Cell 'U9' 'Check inputs'; Assert-Cell 'H9' '#N/A' }
    Trial 'V9' $null { Assert-Cell 'U9' 'Check inputs' }
    Trial-Cells @{C19=6000.0;F19=2036.0;V19=0.5} { Assert-Cell 'R19' 1000.0; Assert-Cell 'T19' 5000.0 }

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
    # The new record tables must reject missing names, blanks, text and literal duplicates.
    foreach ($case in @(
        @('Construction WIP','D5','Q14'), @('KPI awards','D8','L14'),
        @('Asset disposal','D5','P14'), @('Exit pay','D5','T14'), @('Tax cash','D5','L14')
    )) {
        Select-Sheet $case[0]
        $statusAddress = $case[1]; $rowStatusAddress = $case[2]
        Assert-Cell $statusAddress 'Ready'
        Trial 'C14' $null { Assert-Cell $rowStatusAddress 'Check inputs'; Assert-Cell $statusAddress 'Check rows' }
        Trial 'C14' 'invalid' { Assert-Cell $rowStatusAddress 'Check inputs' }
        Trial 'B14' $null { Assert-Cell $rowStatusAddress 'Missing name'; Assert-Cell $statusAddress 'Check rows' }
        Trial-Cells @{B14='Example*';B15='Example second'} { Assert-Cell $rowStatusAddress 'Ready' }
        Trial 'B15' ($sheet.Range('B14').Value2) { Assert-Cell $rowStatusAddress 'Check inputs' }
        Assert-Cell $statusAddress 'Ready'
    }
    Select-Sheet 'Construction WIP'
    Assert-Cell 'D6' 100000.0; Assert-Cell 'D7' 20000.0; Assert-Cell 'D8' 0.0; Assert-Cell 'D9' 20000.0
    Assert-Cell 'M14' 70000.0
    Trial 'F14' 0.0 { Assert-Cell 'M14' -50000.0; Assert-Cell 'O14' 100000.0; Assert-Cell 'N14' 0.0 }
    Trial 'F14' 1.0 { Assert-Cell 'L14' 200000.0; Assert-Cell 'N14' 100000.0 }
    Trial 'G14' 150000.0 { Assert-Cell 'M14' -30000.0 }
    Trial 'E14' 200000.0 { Assert-Cell 'K14' -60000.0 }
    Trial 'F14' 1.01 { Assert-Cell 'Q14' 'Check inputs'; Assert-Cell 'D7' '#N/A' }
    Trial 'I14' 100001.0 { Assert-Cell 'Q14' 'Check inputs' }
    Trial-Cells @{B25='Last project';C25=100000.0;D25=20000.0;E25=50000.0;F25=0.25;G25=10000.0;H25=10000.0;I25=5000.0} {
        Assert-Cell 'D5' 'Ready'; Assert-Cell 'D6' 130000.0; Assert-Cell 'D7' 35000.0; Assert-Cell 'D9' 25000.0
    }
    Assert-Cell 'D5' 'Ready'

    Select-Sheet 'KPI awards'
    Assert-Cell 'H14' 12000.0; Assert-Cell 'I14' 2112.0; Assert-Cell 'D9' 14112.0; Assert-Cell 'D10' 10000.0
    Trial-Cells @{D14=0.79;G14=0.0} { Assert-Cell 'H14' 0.0; Assert-Cell 'D9' 0.0 }
    Trial 'D14' 0.8 { Assert-Cell 'H14' 8000.0; Assert-Cell 'D9' 9408.0 }
    Trial 'D14' 2.0 { Assert-Cell 'H14' 15000.0; Assert-Cell 'D9' 17640.0 }
    Trial 'F14' 0.5 { Assert-Cell 'L14' 'Check inputs' }
    Trial 'G14' 12001.0 { Assert-Cell 'L14' 'Check inputs'; Assert-Cell 'D10' '#N/A' }
    Trial-Cells @{C14=0.0;G14=0.0} { Assert-Cell 'D10' 0.0 }
    Trial 'D5' $null { Assert-Cell 'H9' 'Check cost rates'; Assert-Cell 'D9' '#N/A'; Assert-Cell 'D10' 10000.0 }
    Trial 'D6' -1.0 { Assert-Cell 'H9' 'Check cost rates'; Assert-Cell 'D9' '#N/A' }
    Trial-Cells @{B33='Last employee';C33=5000.0;D33=1.0;E33=1.0;F33=1.0;G33=0.0} {
        Assert-Cell 'D8' 'Ready'; Assert-Cell 'D9' 19992.0; Assert-Cell 'D10' 15000.0
    }
    Assert-Cell 'D8' 'Ready'

    Select-Sheet 'Asset disposal'
    Assert-Cell 'K14' 18000.0; Assert-Cell 'D6' 1000.0; Assert-Cell 'D7' -12100.0
    Assert-Cell 'D8' -1100.0; Assert-Cell 'D9' -11000.0
    Trial-Cells @{E14=0.0;F14=0.0} { Assert-Cell 'L14' -19000.0 }
    Trial 'D14' 20000.0 { Assert-Cell 'K14' 0.0; Assert-Cell 'L14' 19000.0 }
    Trial 'H14' 1101.0 { Assert-Cell 'P14' 'Check inputs' }
    Trial 'F14' 22001.0 { Assert-Cell 'P14' 'Check inputs' }
    Trial-Cells @{I14=0.0;J14=0.0} { Assert-Cell 'M14' 20900.0; Assert-Cell 'N14' 1900.0; Assert-Cell 'O14' 19000.0 }
    Trial 'C14' -1.0 { Assert-Cell 'P14' 'Check inputs'; Assert-Cell 'D6' '#N/A' }
    Trial-Cells @{B25='Last asset';C25=0.0;D25=0.0;E25=0.0;F25=0.0;G25=0.0;H25=0.0;I25=0.0;J25=0.0} {
        Assert-Cell 'D5' 'Ready'; Assert-Cell 'L25' 0.0; Assert-Cell 'D9' -11000.0
    }

    Select-Sheet 'Exit pay'
    Assert-Cell 'N14' 4700.0; Assert-Cell 'O14' 2000.0; Assert-Cell 'P14' 3800.0
    Assert-Cell 'D6' 21000.0; Assert-Cell 'D7' 17000.0; Assert-Cell 'D8' 21300.0
    Trial 'C14' 12.5 { Assert-Cell 'N14' 734.375; Assert-Cell 'Q14' 17034.375 }
    Trial 'C14' 0.0 { Assert-Cell 'N14' 0.0; Assert-Cell 'Q14' 16300.0 }
    Trial 'E14' 0.0 { Assert-Cell 'N14' 4000.0; Assert-Cell 'Q14' 20300.0 }
    Trial 'L14' 21000.0 { Assert-Cell 'R14' 0.0 }
    Trial 'L14' 21001.0 { Assert-Cell 'T14' 'Check inputs'; Assert-Cell 'Q14' '#N/A' }
    Trial 'F14' -1.0 { Assert-Cell 'T14' 'Check inputs' }
    Trial 'M14' $null { Assert-Cell 'T14' 'Check inputs' }
    Trial-Cells @{B23='Last employee';C23=0.0;D23=0.0;E23=0.0;F23=0.0;G23=0.0;H23=0.0;I23=0.0;J23=0.0;K23=0.0;L23=0.0;M23=0.0} {
        Assert-Cell 'D5' 'Ready'; Assert-Cell 'Q23' 0.0; Assert-Cell 'D6' 21000.0
    }

    Select-Sheet 'Tax cash'
    Assert-Cell 'D6' 4500.0; Assert-Cell 'D7' 0.0; Assert-Cell 'D8' 4500.0
    Trial 'F14' 30000.0 { Assert-Cell 'I14' -5500.0; Assert-Cell 'D6' 0.0; Assert-Cell 'D7' 5500.0 }
    Trial-Cells @{F14=30000.0;G14=5500.0} { Assert-Cell 'I14' 0.0 }
    Trial 'C14' -1000.0 { Assert-Cell 'I14' 2500.0 }
    Trial 'D14' -25000.0 { Assert-Cell 'I14' -45500.0 }
    Trial 'H14' -1.0 { Assert-Cell 'L14' 'Check inputs' }
    Trial 'F14' $null { Assert-Cell 'L14' 'Check inputs'; Assert-Cell 'D8' '#N/A' }
    Trial-Cells @{B25='Separate credit account';C25=0.0;D25=-1000.0;E25=0.0;F25=0.0;G25=0.0;H25=0.0} {
        Assert-Cell 'D5' 'Ready'; Assert-Cell 'D6' 4500.0; Assert-Cell 'D7' 1000.0; Assert-Cell 'D8' 3500.0
    }

    Select-Sheet 'Investment'
    Assert-Cell 'H14' 110000.0; Assert-Cell 'K14' 3600.0; Assert-Cell 'N14' 57000.0
    Assert-Cell 'P14' -3600.0; Assert-Cell 'Q14' 53000.0; Assert-Cell 'B23' 2036.0
    $asset=100000.0; $debt=60000.0; $cash=0.0
    1..10 | ForEach-Object { $cash += 6000.0-2000.0-$debt*0.06-3000.0-1000.0; $asset=$asset*1.08+2000.0; $debt-=3000.0 }
    Assert-Cell 'D9' ($asset-$debt); Assert-Cell 'D10' $cash; Assert-Cell 'D11' ($cash+$asset-$debt-40000.0)
    Trial 'J14' 0.0 { Assert-Cell 'K14' 0.0; Assert-Cell 'P14' 0.0 }
    Trial 'M14' 10000.0 { Assert-Cell 'N14' 67000.0; Assert-Cell 'P14' 6400.0; Assert-Cell 'I15' 67000.0 }
    Trial 'F14' 5000.0 { Assert-Cell 'H14' 105000.0; Assert-Cell 'P14' 1400.0 }
    Trial 'O14' -500.0 { Assert-Cell 'P14' -2100.0 }
    Trial 'D14' -1.0 { Assert-Cell 'H14' 2000.0 }
    Trial 'D14' -1.01 { Assert-Cell 'R14' 'Check inputs'; Assert-Cell 'R15' 'Check prior year'; Assert-Cell 'D9' '#N/A' }
    Trial 'L14' 60000.0 { Assert-Cell 'N14' 0.0; Assert-Cell 'R15' 'Check inputs' }
    Trial 'L14' 60001.0 { Assert-Cell 'R14' 'Check inputs' }
    Trial 'F14' 110000.0 { Assert-Cell 'H14' 0.0 }
    Trial 'F14' 110001.0 { Assert-Cell 'R14' 'Check inputs' }
    Trial 'G14' -1.0 { Assert-Cell 'R14' 'Check inputs' }
    Trial 'D14' 'invalid' { Assert-Cell 'R14' 'Check inputs' }
    Trial 'D20' 0.0 {
        $laterAsset=100000.0
        1..10 | ForEach-Object { $laterAsset=$laterAsset*$(if ($_ -eq 7) {1.0} else {1.08})+2000.0 }
        Assert-Cell 'D9' ($laterAsset-30000.0)
    }

    Select-Sheet 'Action register'
    $reviewDate=[DateTime]::new(2026,10,19)
    Assert-Cell 'D7' 'Ready'; Assert-Cell 'D8' 1.0; Assert-Cell 'D9' 0.0; Assert-Cell 'D10' 1.0
    Assert-Cell 'K14' 'Due soon'; Assert-Cell 'L14' 0.0
    # Excel colour integers are BGR: crimson on its tint, then Rhodesian green.
    Assert-Colours 'K14' 0x240099 0xDED9F0
    Trial 'F14' ($reviewDate.AddDays(30).ToOADate()) { Assert-Cell 'K14' 'Due soon' }
    Trial 'F14' ($reviewDate.AddDays(31).ToOADate()) { Assert-Cell 'K14' 'Open' }
    Trial 'F14' ($reviewDate.ToOADate()) { Assert-Cell 'K14' 'Due soon'; Assert-Cell 'L14' 0.0 }
    Trial 'F14' ($reviewDate.AddDays(-1).ToOADate()) { Assert-Cell 'K14' 'Overdue'; Assert-Cell 'L14' 1.0 }
    Trial 'G14' 'Yes' { Assert-Cell 'J14' 'Check inputs'; Assert-Cell 'D7' 'Check rows' }
    Trial-Cells @{G14='Yes';H14=$reviewDate.ToOADate()} { Assert-Cell 'J14' 'Check inputs' }
    Trial-Cells @{G14='Yes';H14=$reviewDate.ToOADate();I14='Example evidence'} {
        Assert-Cell 'K14' 'Recorded complete'; Assert-Cell 'D8' 0.0; Assert-Cell 'D10' 0.0
        Assert-Colours 'K14' 0x324B0F 0xE0E4DB
    }
    Trial-Cells @{G14='Yes';H14=$reviewDate.AddDays(1).ToOADate();I14='Example evidence'} { Assert-Cell 'J14' 'Check inputs' }
    Trial 'H14' ($reviewDate.ToOADate()) { Assert-Cell 'J14' 'Check inputs' }
    Trial 'H14' 'invalid' { Assert-Cell 'J14' 'Check inputs' }
    Trial 'F14' 'invalid' { Assert-Cell 'J14' 'Check inputs' }
    Trial 'E14' $null { Assert-Cell 'J14' 'Check inputs' }
    Trial 'B14' $null { Assert-Cell 'J14' 'Missing name'; Assert-Cell 'D7' 'Check rows' }
    Trial 'B15' 'EXAMPLE-01' { Assert-Cell 'J14' 'Check inputs' }
    Trial-Cells @{B33='LAST-01';C33='Corporate action';D33='Example deadline';E33='Example reviewer';F33=$reviewDate.AddDays(-18).ToOADate();G33='No'} {
        Assert-Cell 'D7' 'Ready'; Assert-Cell 'D8' 2.0; Assert-Cell 'D9' 1.0; Assert-Cell 'D10' 1.0; Assert-Cell 'L33' 18.0
    }
    Assert-Cell 'D7' 'Ready'
    Select-Sheet 'Benefit totals'
    Assert-Cell 'D9' 'Ready'; Assert-Cell 'D10' 'Ready'
    Assert-Cell 'E14' 20802.0; Assert-Cell 'F14' 9434.0; Assert-Cell 'G14' 30236.0; Assert-Cell 'D11' 14210.92
    Trial 'D7' 0.2 { Assert-Cell 'D11' 6047.2 }
    Trial 'D7' 0.0 { Assert-Cell 'D11' 0.0 }
    Trial-Cells @{D5=1.0;D6=1.0} { Assert-Cell 'G14' 15000.0; Assert-Cell 'D11' 7050.0 }
    Trial 'D5' 0.99 { Assert-Cell 'D9' 'Check rates'; Assert-Cell 'D11' '#N/A' }
    Trial 'D7' $null { Assert-Cell 'D9' 'Check rates'; Assert-Cell 'D11' '#N/A' }
    Trial 'D7' 1.01 { Assert-Cell 'D9' 'Check rates' }
    Trial 'C14' $null { Assert-Cell 'I14' 'Check inputs'; Assert-Cell 'D10' 'Check rows' }
    Trial 'C14' 'invalid' { Assert-Cell 'I14' 'Check inputs' }
    Trial-Cells @{C14=0.0;D14=0.0} { Assert-Cell 'D11' 0.0 }
    Trial 'C14' -1.0 { Assert-Cell 'I14' 'Check inputs' }
    Trial 'B14' $null { Assert-Cell 'I14' 'Missing name'; Assert-Cell 'D10' 'Check rows' }
    Trial 'B15' 'Example benefits' { Assert-Cell 'I14' 'Check inputs' }
    Trial-Cells @{B14='Example*';B15='Example second'} { Assert-Cell 'I14' 'Ready' }
    Trial-Cells @{B33='Last benefit';C33=1000.0;D33=0.0} {
        Assert-Cell 'D10' 'Ready'; Assert-Cell 'H33' 977.694; Assert-Cell 'D11' 15188.614
    }
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
        workflows=@('Allocation','Contractor','Vendor finance','Mixed loan','Rental','Packaging','Amortisation','Timeline','Deed register','Carry forward','Loan offers','Break even','Working capital','Construction WIP','KPI awards','Asset disposal','Exit pay','Tax cash','Investment','Action register','Benefit totals'); result='PASS'} |
        ConvertTo-Json | Set-Content -LiteralPath $Out -Encoding UTF8
}
