# Finish native navigation after the artifact builder exports ordinary formulas.
# The authoring runtime cannot export HYPERLINK formula results for Excel.
param([string]$Path = "$PSScriptRoot\..\templates\practice-calculators.xlsx")
$ErrorActionPreference = 'Stop'
$Path = (Resolve-Path -LiteralPath $Path).Path
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count) {
    throw 'Excel is already running; preparation never attaches to a user session.'
}
$xl = $wb = $contents = $tab = $null
function Add-InternalLink($Sheet, [string]$Address, [string]$Target, [string]$Label) {
    $anchor = $Sheet.Range($Address)
    $link = $null
    try {
        $anchor.Hyperlinks.Delete()
        $link = $Sheet.Hyperlinks.Add($anchor, '', "'$Target'!B2", '', $Label)
        $anchor.Font.Color = 0x3C0266 # Excel uses BGR: Tyrian purple #66023C.
    } finally {
        foreach ($com in @($link, $anchor)) {
            if ($com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) {
                [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com)
            }
        }
    }
}
try {
    $xl = New-Object -ComObject Excel.Application
    $xl.Visible = $false
    $xl.DisplayAlerts = $false
    $xl.EnableEvents = $false
    $xl.AutomationSecurity = 3
    $wb = $xl.Workbooks.Open($Path, 0, $false)
    if ($wb.Worksheets.Count -ne 22 -or $wb.HasVBProject -or $wb.LinkSources(1)) {
        throw 'Expected the standalone, macro-free practice companion'
    }
    $contents = $wb.Worksheets.Item('Read me')
    if ($contents.Range('B2').Value2 -ne 'Ozzit practice calculators') { throw 'Unexpected workbook title' }
    for ($i = 2; $i -le $wb.Worksheets.Count; $i++) {
        $tab = $wb.Worksheets.Item($i)
        $row = $i + 12
        if ($contents.Range("B$row").Value2 -ne $tab.Name) { throw "Contents row $row does not match the worksheet" }
        Add-InternalLink $contents "B$row" $tab.Name $tab.Name
        Add-InternalLink $tab 'B1' 'Read me' 'Contents'
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($tab)
        $tab = $null
    }
    $xl.CalculateFullRebuild()
    $wb.Save()
    Write-Output 'Prepared 42 internal hyperlinks and recalculated the companion in native Excel'
} finally {
    try { if ($wb) { $wb.Close($false) } }
    finally {
        try { if ($xl) { $xl.Quit() } }
        finally {
            foreach ($com in @($tab, $contents, $wb, $xl)) {
                if ($null -ne $com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) {
                    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com)
                }
            }
            $com = $tab = $contents = $wb = $xl = $null
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
            [GC]::Collect(); [GC]::WaitForPendingFinalizers()
        }
    }
}
