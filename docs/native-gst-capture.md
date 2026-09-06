# Native GST screenshot, 6 September 2026

This records one synthetic example in the released v3.2.0 workbook. It is not the full native acceptance gate for the different workbook on main.

Source: [v3.2.0 ozzit.xlsx](https://github.com/ryanduguid/Ozzit/releases/download/v3.2.0/ozzit.xlsx).

- Source SHA-256: `13df5eb0e2e7a3d1b17a743a990c30adfd187d409be133996ec154543e78ff28`.
- Excel: Microsoft 365, version 16.0, build 20326, Windows.
- Result after `CalculateFullRebuild()`: numeric `100`, displayed `$100.00`.
- Formula read back through `Range("B19").Formula2`: `=oz.GSTExtractλ(1100)`.
- Source workbook was copied before opening. No shipped workbook or function definition changed.
- [Screenshot](../assets/ozzit-gst-extract-synthetic.png) SHA-256: `899c54a4c238f3d4abca5cda5b38ca250e68f349f384a4ee2a03a47191bc9b7b`.

## Reproduce the calculation and capture

Close your own Excel session first. Run this from a directory containing the downloaded release asset. The script refuses to attach to any existing Excel process, verifies the source hash, works on a disposable copy and leaves Excel visible for the capture. Use a fresh directory for each run.

```powershell
$ErrorActionPreference = 'Stop'
if (Get-Process EXCEL -ErrorAction SilentlyContinue) { throw 'Close your Excel session first' }
$sourcePath = (Resolve-Path './ozzit.xlsx').Path
$expected = '13df5eb0e2e7a3d1b17a743a990c30adfd187d409be133996ec154543e78ff28'
if ((Get-FileHash -LiteralPath $sourcePath).Hash.ToLowerInvariant() -ne $expected) { throw 'Release hash mismatch' }
$copyPath = Join-Path (Get-Location) 'ozzit-capture.xlsx'
if (Test-Path -LiteralPath $copyPath) { throw 'Use a fresh capture directory' }
Copy-Item -LiteralPath $sourcePath -Destination $copyPath
$ozExcel = New-Object -ComObject Excel.Application
try {
    $ozExcel.Visible = $true
    $ozExcel.DisplayAlerts = $true
    $book = $ozExcel.Workbooks.Open($copyPath, 0, $false)
    $sheet = $book.Worksheets.Item('Australian tax')
    $sheet.Activate()
    $sheet.Range('A14').Value2 = 'GST | SYNTHETIC EXAMPLE'
    $sheet.Range('A19').Value2 = 1100
    $sheet.Range('B19').Formula2 = '=oz.GSTExtractλ(1100)'
    $sheet.Range('A19:B19').NumberFormat = '$#,##0.00'
    $sheet.Columns.Item('A').ColumnWidth = 32
    $sheet.Columns.Item('B').ColumnWidth = 22
    $ozExcel.ActiveWindow.Zoom = 125
    $ozExcel.ActiveWindow.ScrollRow = 14
    $sheet.Range('B19').Select()
    $ozExcel.CalculateFullRebuild()
    Write-Output "Excel=$($ozExcel.Version) build=$($ozExcel.Build); formula=$($sheet.Range('B19').Formula2); value=$($sheet.Range('B19').Value2); text=$($sheet.Range('B19').Text)"
    if ($sheet.Range('B19').Value2 -ne 100) { throw 'Unexpected GST result: do not capture as success' }
    Read-Host 'Capture the visible example, then press Enter to close this Excel session'
} finally {
    $ozExcel.DisplayAlerts = $false
    $ozExcel.Quit()
}
```

The recorded run printed:

```text
Excel=16.0 build=20326; formula==oz.GSTExtractλ(1100); value=100; text=$100.00
```

Set the window to 1280 by 900 pixels. Keep B19 selected and the formula bar visible. Capture the contiguous rectangle from the name box and formula bar down through row 19, ending immediately after column B. The published capture used window-local x=8, y=185, width=594, height=214 pixels; the ribbon, account name and other applications are outside that crop. Different display scaling may require a different rectangle. No views were stitched and no displayed values were altered.

If opening requires repair, calculation raises an error or the value differs from $100.00, retain the actual diagnostic and do not publish a success image. Do not hard-code the result or mask an error. Follow [RELEASING.md](../RELEASING.md) for native acceptance of a corrected workbook; this small calculation does not replace those gates.
