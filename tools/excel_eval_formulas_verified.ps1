# Retain native evidence only after the existing Excel helper completes successfully.
# The input bytes are frozen before evaluation. Output identifies both helper versions.
param([string]$Cases, [string]$Out, [string]$Path = "$PSScriptRoot\..\ozzit.xlsx")
$ErrorActionPreference = 'Stop'
Import-Module "$PSHOME\Modules\Microsoft.PowerShell.Utility\Microsoft.PowerShell.Utility.psd1" -ErrorAction Stop
if (Test-Path -LiteralPath $Out) { throw 'Output already exists; choose a new path.' }
$Path = (Resolve-Path -LiteralPath $Path).Path
$Out = [IO.Path]::GetFullPath($Out)
$helper = Join-Path $PSScriptRoot 'excel_eval_formulas.ps1'
$helperHash = (Get-FileHash -LiteralPath $helper -Algorithm SHA256).Hash.ToLower()
$selfHash = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLower()
$scratch = Join-Path ([IO.Path]::GetDirectoryName($Out)) ('.excel-evidence-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($scratch)
$frozen = Join-Path $scratch 'cases.json'
$pending = Join-Path $scratch 'result.json'
try {
    [IO.File]::WriteAllBytes($frozen, [IO.File]::ReadAllBytes($Cases))
    $casesHash = (Get-FileHash -LiteralPath $frozen -Algorithm SHA256).Hash.ToLower()
    Write-Output "Native scratch directory: $scratch; the helper records its Excel PID in result.json.pid"
    & $helper -Cases $frozen -Out $pending -Path $Path
    $record = [IO.File]::ReadAllText($pending, [Text.Encoding]::UTF8) | ConvertFrom-Json
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLower() -ne $record.workbook_sha256) {
        throw 'Workbook bytes changed during the native evaluation'
    }
    if ((Get-FileHash -LiteralPath $helper -Algorithm SHA256).Hash.ToLower() -ne $helperHash) {
        throw 'Excel helper changed during the native evaluation'
    }
    $record | Add-Member cases_sha256 $casesHash
    $record | Add-Member helper_sha256 $helperHash
    $record | Add-Member evidence_helper_sha256 $selfHash
    $record | Add-Member evaluated_at_utc ([DateTime]::UtcNow.ToString('o'))
    [IO.File]::WriteAllText($pending, ($record | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding $false))
    [IO.File]::Move($pending, $Out)
    Write-Output 'VERIFIED_NATIVE_EVIDENCE_RETAINED'
}
finally {
    foreach ($file in @($frozen, $pending, "$pending.pid")) { [IO.File]::Delete($file) }
    [IO.Directory]::Delete($scratch)
}
