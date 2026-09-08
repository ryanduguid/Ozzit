# Disposable comparison. Opens the reviewed workbook read-only and never saves it.
param([string]$Path = "$PSScriptRoot\..\ozzit.xlsx")
$ErrorActionPreference = 'Stop'
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count) { throw 'Excel is already running' }
$Path = (Resolve-Path $Path).Path
$before = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
$xl = $null; $wb = $null
$lambda = [char]0x03BB
$candidate = '=LAMBDA(values,size,LET(totals,SCAN(0,values,LAMBDA(acc,value,acc+value)),cols,SEQUENCE(,COLUMNS(values)),totals-IF(cols>size,CHOOSECOLS(totals,IF(cols>size,cols-size,1)),0)))'
try {
    $xl = New-Object -ComObject Excel.Application
    $xl.Visible = $false; $xl.DisplayAlerts = $false
    $xl.AutomationSecurity = 3; $xl.EnableEvents = $false
    $wb = $xl.Workbooks.Open($Path, 0, $true)
    $xl.Calculation = -4135
    $ws = $wb.Worksheets.Add()
    [void]$wb.Names.Add('bench_prefix', $candidate)
    Write-Output "Excel $($xl.Version) build $($xl.Build)"
    Write-Output "Measured: $([DateTimeOffset]::Now.ToString('o'))"
    Write-Output "Workbook SHA-256: $($before.ToLowerInvariant())"
    Write-Output "Candidate: $candidate"
    Write-Output '| Columns | Window | Current, ms | Cumulative candidate, ms | Maximum absolute difference |'
    Write-Output '|---:|---:|---:|---:|---:|'
    foreach ($case in @(@(120,12), @(1200,120), @(10000,120), @(10000,1000))) {
        $n = $case[0]; $size = $case[1]
        [void]$ws.UsedRange.ClearContents()
        $data = New-Object 'object[,]' 1,$n
        for ($i = 0; $i -lt $n; $i++) { $data[0,$i] = [double]((($i * 17) % 997) / 100.0) }
        $inputRange = $ws.Range($ws.Cells.Item(1,1), $ws.Cells.Item(1,$n))
        $inputRange.Value2 = $data
        $address = $inputRange.Address($false,$false)
        $old = $ws.Range('A3'); $new = $ws.Range('A5')
        $old.Formula2 = "=oz.RollingSum$lambda($address,$size)"
        $new.Formula2 = "=bench_prefix($address,$size)"
        for ($i=0; $i -lt 3; $i++) { [void]$old.Calculate(); [void]$new.Calculate() }
        $oldTimes=@(); $newTimes=@()
        for ($i=0; $i -lt 9; $i++) {
            # Alternate order to reduce a systematic warm-cache advantage.
            foreach ($name in $(if ($i % 2) { @('new','old') } else { @('old','new') })) {
                $range = if ($name -eq 'old') { $old } else { $new }
                $watch = [Diagnostics.Stopwatch]::StartNew()
                [void]$range.Calculate()
                $watch.Stop()
                if ($name -eq 'old') { $oldTimes += $watch.Elapsed.TotalMilliseconds }
                else { $newTimes += $watch.Elapsed.TotalMilliseconds }
            }
        }
        $a = $ws.Range($ws.Cells.Item(3,1),$ws.Cells.Item(3,$n)).Value2
        $b = $ws.Range($ws.Cells.Item(5,1),$ws.Cells.Item(5,$n)).Value2
        $maxError = 0.0
        for ($i=1; $i -le $n; $i++) {
            if ($a.GetValue(1,$i) -isnot [double] -or $b.GetValue(1,$i) -isnot [double]) {
                throw "Expected numeric outputs at column $i for $n columns, window $size"
            }
            $maxError = [Math]::Max($maxError,[Math]::Abs($a.GetValue(1,$i) - $b.GetValue(1,$i)))
        }
        $currentMs = (($oldTimes | Sort-Object)[4]).ToString('F3',[Globalization.CultureInfo]::InvariantCulture)
        $prefixMs = (($newTimes | Sort-Object)[4]).ToString('F3',[Globalization.CultureInfo]::InvariantCulture)
        $difference = $maxError.ToString('E2',[Globalization.CultureInfo]::InvariantCulture)
        Write-Output "| $n | $size | $currentMs | $prefixMs | $difference |"
    }
    Write-Output '| Input | Window | Current result | Candidate result |'
    Write-Output '|---|---:|---|---|'
    foreach ($case in @(
        @{ Name='large value leaves window'; Values='{1E16,1,1}'; Size=1 },
        @{ Name='error leaves window'; Values='HSTACK(1,NA(),3,4,5)'; Size=2 },
        @{ Name='text in input'; Values='{1,"x",3,4}'; Size=2 }
    )) {
        [void]$ws.UsedRange.ClearContents()
        $ws.Range('A3').Formula2 = "=oz.RollingSum$lambda($($case.Values),$($case.Size))"
        $ws.Range('A5').Formula2 = "=bench_prefix($($case.Values),$($case.Size))"
        [void]$ws.Calculate()
        $count = if ($case.Name -eq 'large value leaves window') { 3 } elseif ($case.Name -eq 'error leaves window') { 5 } else { 4 }
        $oldText = for ($i=1; $i -le $count; $i++) { $cell = $ws.Cells.Item(3,$i); if ($cell.Value2 -is [double]) { $cell.Value2.ToString('R',[Globalization.CultureInfo]::InvariantCulture) } else { $cell.Text } }
        $newText = for ($i=1; $i -le $count; $i++) { $cell = $ws.Cells.Item(5,$i); if ($cell.Value2 -is [double]) { $cell.Value2.ToString('R',[Globalization.CultureInfo]::InvariantCulture) } else { $cell.Text } }
        Write-Output "| $($case.Values) | $($case.Size) | [$($oldText -join ', ')] | [$($newText -join ', ')] |"
    }
}
finally {
    if ($wb) { [void]$wb.Close($false) }
    if ($xl) { [void]$xl.Quit(); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($xl) }
    [GC]::Collect(); [GC]::WaitForPendingFinalizers()
}
$after = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
if ($after -ne $before) { throw 'Workbook bytes changed' }
Write-Output "Workbook unchanged: $after"
