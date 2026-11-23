# Rebuild the tracked workbook

The native builder reads the 6 formula modules in `src/` and the workbook layout in `release/workbook-source.json`. It runs without Excel, an input workbook or Git history. Both inputs and the pinned toolchain must come from the same revision. The baseline begins with the 552,608-byte workbook at commit `4db8068`.

From the repository root, choose an unused output path, build the pinned tools and compare the result with the release lock.

```powershell
cargo build --locked --release --manifest-path tools/ozzit/Cargo.toml
if ($LASTEXITCODE) { throw 'Native build failed' }
$output = Join-Path ([IO.Path]::GetTempPath()) ("ozzit-" + [guid]::NewGuid() + ".xlsx")
& .\tools\ozzit\target\release\ozzit workbook_source build release/workbook-source.json src $output
if ($LASTEXITCODE) { throw 'Workbook build failed' }
$expected = Get-Content -Raw release/workbook-base.json | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $output).Hash -ne $expected.sha256 -or
    (Get-Item -LiteralPath $output).Length -ne $expected.size) { throw 'Workbook differs from release lock' }
```

The layout stores the worksheet cells, styles, relationships, date values and Excel-calculated caches as XML text. Each worksheet cell occupies one JSON line, so its address, formula and cached result appear together in a diff. For each library name the layout stores the compiled formula digest and the spacing and local workbook qualifiers needed to reproduce Excel's spelling. The compiler supplies the formula itself. The same modules populate the Advanced Formula Environment store. The pinned ZIP writer fixes member order, timestamps and compression.

The clean-build regression test runs the command in a fresh directory containing only these text inputs, compares every output byte with the tracked workbook and checks the lock digest. This tests the present baseline. The missing input to the historical v3.0.0 builder and the original FY27 date shift are documented in the [postbuild history](../tools/postbuild/README.md#what-is-intentionally-not-here).

## Refresh the layout after a workbook change

Follow the source and native Excel gates in the [release procedure](../RELEASING.md). The capture command preserves the values Excel calculated. It never recalculates a formula. Capture the layout after final sanitisation and native evidence checks.

```powershell
& .\tools\ozzit\target\release\ozzit workbook_source capture ozzit.xlsx src release/workbook-source.next.json
if ($LASTEXITCODE) { throw 'Capture failed' }
```

Capture writes a new file only after an in-memory rebuild matches the entire input workbook. Review the layout diff against `release/workbook-source.json`, replace it with the reviewed capture and include it with the workbook change. Update `release/workbook-base.json` and the README digest in the required separate commit. Then run the Rust tests against the updated lock.
