## Getting started

Download the workbook from the [latest release](https://github.com/ryanduguid/Ozzit/releases/latest). Cloning the repository is not required to use the library, and the release asset is a few hundred kilobytes against the repository's tens of megabytes.

The release carries `ozzit.xlsx` alongside `SHA256SUMS` and `provenance.json`. Check the workbook against the published checksum before opening it:

```bash
curl -LO https://github.com/ryanduguid/Ozzit/releases/latest/download/ozzit.xlsx
curl -LO https://github.com/ryanduguid/Ozzit/releases/latest/download/SHA256SUMS
sha256sum --check --ignore-missing SHA256SUMS
```

On Windows, `Get-FileHash ozzit.xlsx -Algorithm SHA256` prints the same digest to compare against the `SHA256SUMS` line by eye. `SHA256SUMS` deliberately does not cover itself, so read it from the release page rather than trusting a local copy alone. `provenance.json` binds that workbook to its version, signed tag and candidate commit; [RELEASING.md](../RELEASING.md) describes what it asserts.

The workbook tracked in this repository is the candidate the gates below run against. The release asset is that same file copied byte-for-byte from the tagged tree, so the 2 share a SHA-256 at the tag they were released from. Between releases the tracked copy can be ahead of the published one.

1. Open `ozzit.xlsx`.
2. Cell A1 of every visible worksheet links back to the table of contents; every name in the TOC links to its worksheet.
3. For inline help, type a function name with no arguments in an empty cell, for example `=oz.Amortiseλ()`. The help block spills syntax, parameters and worked examples.
4. Grey-shaded cells on each worksheet are inputs. Change them and watch the function respond.
5. To use the functions in your own workbook, copy a pale purple shaded cell across (Excel brings the named LAMBDA with it), or import the plain-text source with the Advanced Formula Environment in the Excel Labs add-in: `oz.txt` at the repository root is the whole library as one module.

   The repository also includes an [Ozzit 13-week cash-flow forecast template](../templates/README.md) for Australian FP&A planning; see the [template overview](cash-flow-template.md).

   Use `oz.txt`, `functions.csv` and `ozzit.xlsx` from the same approved tag or
   commit. A current `main` text file may differ from an older release workbook;
   do not combine those versions. Record the revision and workbook checksum
   alongside an installation check.

   The Advanced Formula Environment takes a function's prefix from the module
   container it is imported into, so importing one `src/` file as a module named
   after it produces `Dates.CountDOWλ` rather than `oz.CountDOWλ`, and a call from
   one module into another does not resolve. `oz.txt` joins the 6 module texts,
   verbatim and in build order, so that one import into a new module named `oz`
   recreates every shipped `oz.` name with its cross-module calls intact.
   `ozzit build_module` writes it from `src/`, and the native module tests in
   `tools/ozzit/tests/module_contracts.rs` hold the tracked file to the current
   `src/`, to every `oz.` name the workbook
   ships (the 5 help tables included) and to `functions.csv`. Those checks bind the
   file to the workbook commit it ships with; they do not exercise the add-in's
   import route, which has not yet been run against `oz.txt` as a native gate, so
   import into a disposable workbook first and check the names it creates against
   `functions.csv`. Vertex42's LAMBDA Library and BigSpill publish their libraries
   the same single-module way. The workbook is the authority for the `oz.` names.
   `src/` is for reading, diffing, and pasting a single definition into Name
   Manager, where the name is yours to choose. A pasted definition stands alone
   except for 6 that call other functions by name; define their callees first,
   under the names the definition uses: `Depreciateλ` calls `Allocateλ`, `DBλ`,
   `DDBλ`, `DiminishingValueλ`, `PeriodDiffλ`, `PrimeCostλ`, `SLNλ`, `SYDλ` and
   `VDBλ`; `LeaseScheduleλ` and `LeaseRemeasureλ` call `LeaseLiabilityλ`;
   `HPScheduleλ` calls `PeriodRateλ` and `LevelPaymentλ`; `FinanceCompareλ` calls
   `PeriodRateλ`; and `DebtSculptVariableLRVλ` calls `InterestLRVλ`. To change a function, edit its
   definition in `src/` and follow the run order in `tools/postbuild/README.md`:
   `ozzit compile_sources` renders it into the stored form and writes it over the
   defined name that ships, `ozzit build_module` rebuilds `oz.txt`,
   `ozzit sync_afe_store` copies the source into the workbook's AFE store, and
   `ozzit sanitise_workbook` runs last.

   Imported names can collide with names a workbook already holds, and a module
   import brings every function at once. Try the import in a blank workbook first,
   then read Name Manager. To remove the library from a workbook, delete the `oz.`
   names in Name Manager, or delete the `oz` module in the add-in and save; both
   leave formulas that used the functions showing `#NAME?` until they are replaced.

The workbook opens showing the numbers its own formulas produce, so nothing has to recalculate before it reads correctly and Excel does not ask you to save a file you never edited. That is a property the build cannot give it: the build edits the workbook as XML with no formula engine, so `ozzit refresh_cache` recalculates it in Excel afterwards and `ozzit verify_cache` proves every cached value matches.

Functions with a data-validation companion (named with a `DV` suffix, such as `oz.AmortiseλDV`) diagnose argument problems when the parent function returns something unexpected.

### Check installation in a fresh workbook (v3.4.2)

The desktop Excel check below copies existing result cells for `oz.RollingSumλ`
and `oz.Amortiseλ` into a new workbook. It replaces their example references
with self-contained inputs, closes the source, checks the required names and
helpers, then saves and reopens the destination. A second set of inputs checks
that the reopened formulas recalculate. No external workbook link may remain.

This runner uses the source cell addresses and helper dependencies of v3.4.2. Use the v3.4.2 workbook for this historical check. Close Excel first. Use its SHA-256 and an
unused output directory outside this checkout, with an existing parent:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\excel_install_selftest.ps1 -Path C:\downloads\ozzit.xlsx -ExpectedSha256 <verified-sha256> -OutDirectory C:\checks\ozzit-install
```

The source opens read-only and must retain its original bytes. The output
contains `installed.xlsx`, `evidence.json` and the automation process ID.
The saved workbook uses the initial test inputs; the changed inputs are tested
in memory. Existing output directories are refused.

On 25 September 2026, both cases passed for the released v3.4.2 workbook
(`0306793a7e473ce70e78149fea1e107fc0f714d61ab50960c16f6fd528878f6f`)
in Windows desktop Excel 16.0 build 20430. The [installation record](install-v3.4.2.json)
retains the runner and workbook hashes, names, spill shapes and values.
This checks the copy-cell route and
those examples only. It does not test Excel for Mac, Excel for the web, the AFE
import route or every function. Numerical agreement with another engine is
recorded separately in the [pyxirr comparison](pyxirr-comparison.md).

The normal verification sequence runs `ozzit verify_native_evidence`.
It checks the retained v3.4.2 records, runner byte hashes, case inventories,
reported differences and installation values. It fails on runner drift or
contradictory evidence. If the checkout workbook differs from v3.4.2, it labels
the records as historical; they do not validate the changed workbook.

To bind the records to locally retained files, add `--workbook` with the exact
release workbook and `--destination` with the installed workbook. Supplied
files must match their recorded hashes. CI has neither file and verifies only
the retained records and runners. This check does not run Excel, recalculate
the workbook or replace the native gates required for a workbook change.

## Check recalculation on Windows, Mac or the web

Use a disposable copy of the workbook. Record its SHA-256 before opening or uploading it. Record the Excel build on desktop, or the browser version and date on the web. Use the same workbook digest when comparing hosts.

In Excel's **Automate** tab, create a script from [excel_compatibility.ts](../tools/excel_compatibility.ts). Run it with your host and build and the workbook digest. It includes the same [4 cases](../tools/compatibility-cases.json) used by the Windows runner. The script creates a scratch worksheet, enters each formula, forces recalculation, reads each spill and removes the scratch sheet. Save the JSON from the output pane, including any failed cases. Check that all 4 case IDs and both spill dimensions match the case file before recording a pass for that host. Availability depends on the account and Excel version. [Microsoft's Office Scripts requirements](https://learn.microsoft.com/en-us/office/dev/scripts/testing/platform-limits)

For a repeat run with Windows desktop Excel closed, use an unused evidence file. The Windows runner opens the workbook read-only. Compare its result grids with the case file.

```powershell
$output = Join-Path ([IO.Path]::GetTempPath()) ("ozzit-compat-" + [guid]::NewGuid() + ".json")
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\excel_eval_formulas_verified.ps1 -Cases (Resolve-Path tools\compatibility-cases.json) -Out $output -Path (Resolve-Path ozzit.xlsx)
```

Without the Automate tab, enter the same 4 formulas in A1 on a blank sheet in the copy, one at a time. Record each spill's dimensions and every cell value. RollingSum produces one row of 4 values. LeaseSchedule calls LeaseLiability and produces one column of 4 values. Compare each numeric cell with its expected value using an absolute tolerance of 0.000001. Record formula errors and blocked spills as observed.

On 23 November 2026, all 4 cases passed on the tracked 552,608-byte workbook in Windows Excel 16.0 build 20625 using the existing Windows evaluator. The [Windows record](compatibility-windows-2026-11-23.json) includes the case and workbook digests, runner digests and actual spill grids. Mac and web await native runs. These cases cover recalculation in the open workbook. Copying names between workbooks and importing `oz.txt` require separate tests.

## Evaluate formulas from a script or agent

A script or coding agent on Windows with desktop Excel installed can look up a function and get Excel's own result for a formula without a person at the keyboard. Close Excel first: the evaluator refuses to start while Excel runs. From the repository root, write every function's signature, parameters, notes and worked examples as JSON:

```powershell
cargo build --locked --release --manifest-path tools/ozzit/Cargo.toml
if ($LASTEXITCODE) { throw 'Native build failed' }
$reference = Join-Path ([IO.Path]::GetTempPath()) ("ozzit-reference-" + [guid]::NewGuid() + ".json")
& .\tools\ozzit\target\release\ozzit function_reference src functions.csv $reference
if ($LASTEXITCODE) { throw 'Function reference failed' }
```

Then write the formulas as a UTF-8 JSON array of `id` and `formula` pairs, as in [compatibility-cases.json](../tools/compatibility-cases.json), and evaluate them:

```powershell
$cases = Join-Path ([IO.Path]::GetTempPath()) ("ozzit-cases-" + [guid]::NewGuid() + ".json")
$results = Join-Path ([IO.Path]::GetTempPath()) ("ozzit-results-" + [guid]::NewGuid() + ".json")
[IO.File]::WriteAllText($cases, '[{"id":"rolling","formula":"=oz.RollingSum' + [char]0x03BB + '({1,2,3,4},2)"}]', [Text.UTF8Encoding]::new($false))
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\excel_eval_formulas_verified.ps1 -Cases $cases -Out $results -Path (Resolve-Path ozzit.xlsx)
if ($LASTEXITCODE) { throw 'Evaluation failed' }
(Get-Content -Raw $results | ConvertFrom-Json).results.rolling
```

Excel opens the workbook read-only in a hidden window, enters each formula on a scratch sheet so arrays spill, and closes without saving. The results file holds each spill as rows of values under its `id`, an Excel error as `#ERR:<code>`, the Excel build and the SHA-256 of the workbook and case file. The run fails if the workbook's bytes change. Mac and web Excel use the Office Script above instead.

## Audit a workbook

`ozzit audit <workbook>` reads any `.xlsx` file without changing it and lists what it finds, one line each; it exits 1 when it finds anything. It reads the formulas the file records, so it needs neither Excel nor a recalculation:

```powershell
& .\tools\ozzit\target\release\ozzit audit .\model.xlsx
```

- **Hard-coded number:** a plain number between 2 formula cells of one pattern in a row or column.
- **External link:** a linked workbook that does not exist, that Excel marked missing or that is remote. A Mac volume-colon path is checked as a Mac path.
- **Hidden sheet or name.**
- **Unused name:** a defined name for a range or constant that no formula, formatting or validation rule, chart or other part uses. Built-in and hidden names, and names holding a formula that calls a function, `LAMBDA` included, are not reported.
- **Buried check:** a row whose label, the nearest text to its left, contains the word 'check', whose formulas no formula outside that row reads, and which reads no other check. A roll-up that reads the check, on any sheet, clears it; a formatting rule over the check itself does not.
- **Unlinked row:** a number or formula between a `SUM` of one range and its total, in the total's own column or row, which the total leaves out and no formula outside that row reads. This is the row added just below a range and above its total.
- **Circular reference:** formula cells that read one another, directly or through a reference-only name; a reference that only `ROW`, `COLUMN`, `ROWS`, `COLUMNS`, `ISREF`, `ISFORMULA`, `FORMULATEXT` (including `_xlfn.` XML forms) or `CELL` with `"filename"`, `"address"`, `"row"`, `"col"` or `"width"` reads does not count. Iterative calculation being on is reported too, because Excel then gives no warning.

A reference built whilst Excel calculates, through `INDIRECT` or `OFFSET`, cannot be followed from the file. A workbook whose cells use either is not checked for buried checks or unlinked rows, and the audit names the first such cell.

## Roll checks up into one status

`oz.CheckRollUpλ(Checks, [Labels], [Tolerance])` turns a model's check cells into one text value for a banner or summary cell. In each cell of `Checks`, TRUE passes and FALSE fails. A number is a difference: it passes when its size is at most `Tolerance` (0 when omitted) and fails above it. An error or any other text fails. Blank cells and empty text are not checks. The result names the first failure in reading order, across each row in turn, by its label from `Labels` (the same shape as `Checks`), or by its position when there is no label. A range with no check result fails, so an empty roll-up never reads as OK.

| Checks | Formula | Result |
|---|---|---|
| All pass within the tolerance | `=oz.CheckRollUpλ({TRUE,0,0.004},{"Balance","Cash","GST"},0.005)` | OK: 3 checks pass |
| 2 fail | `=oz.CheckRollUpλ({TRUE,FALSE,12.5},{"Balance","Cash","GST"})` | FAIL: 2 of 3 checks fail, first: Cash |
| No labels | `=oz.CheckRollUpλ({0,0.001})` | FAIL: 1 of 2 checks fail, first: check 2 |
| Nothing to check | `=oz.CheckRollUpλ({"",""})` | FAIL: no check results |

For a banner, put the roll-up in a cell near the top of the cover or summary sheet, here named `ModelStatus`, over the check column and its labels:

```text
=oz.CheckRollUpλ(Checks!C5:C20, Checks!A5:A20, 0.005)
```

Then add a conditional formatting rule to that cell with the formula `=LEFT(ModelStatus,3)<>"OK:"` and a fill that stands out. The rule also catches the refusal messages, which do not start with `OK:`. Because the roll-up reads every check in its range, `ozzit audit` does not report those rows as buried checks.

## Modern Excel

Excel 365 has gained functions since the earlier workbook of July 2024. Several helpers point to native options in their inline help's `SEE ALSO` line. Native functions can have different inputs and behaviour, so compare their contracts before replacing a helper. The table also includes a newer Beta preview:

| Helper | Native option in Excel 365 |
|---|---|
| `oz.RangeToDAλ`, `oz.RangeToDAEλ`, `oz.RangeToDAUλ` | `TRIMRANGE`, or trim references (`.:.`) |
| `FilterContainsλ` | `REGEXTEST`, `REGEXEXTRACT` |
| `SumPeriodsλ`, `SumContainsλ` | `GROUPBY`, `PIVOTBY` |
| `oz.IsInListλ`, `oz.IsInListUλ` | `HAS` ([Beta preview](#lists-and-nested-arrays-in-beta)); argument order differs |

The helpers remain available on the Excel 2024 baseline and inside the library's own composition. Choose a native option only when its behaviour fits and every recipient's Excel build supports it. A Microsoft 365 subscription alone does not establish availability. The original native options were checked against Microsoft's documentation in August 2026; the Beta information below was checked on 25 September 2026.

### Exact membership and any/all checks

`oz.IsInListλ(Value, List)` checks whether a whole value occurs in a row, column or grid. It ignores case and treats `*` and `?` as ordinary characters. `oz.IsInListUλ` has the same behaviour. Combine the returned TRUE/FALSE values with `OR` or `AND` to check several values:

| Question | Formula | Result |
|---|---|---|
| Does this grid contain 1? | `=oz.IsInListλ(1,{2,3;4,1})` | TRUE |
| Does the list contain either 1 or 9? | `=OR(oz.IsInListλ({1;9},{1,2,3}))` | TRUE |
| Does the list contain both 1 and 9? | `=AND(oz.IsInListλ({1;9},{1,2,3}))` | FALSE |

For named ranges, `=AND(oz.IsInListλ(RequiredValues,AvailableValues))` checks that every required value is present. Use clean, non-empty inputs and validate missing values, errors and data types separately. Membership does not prove uniqueness or valid data: an error-valued lookup such as `oz.IsInListλ(NA(),{1,2})` returns FALSE.

Membership differs from substring matching. `oz.FilterContainsλ` and `oz.SumContainsλ` use SEARCH or FIND to look within text, with an optional case setting. For example, `oz.FilterContainsλ({10;20},{"CAPEX plant";"OPEX rent"},"CAPEX")` returns 10. A whole-value membership check for `"CAPEX"` against those two labels would not match either label.

The preview `HAS` takes its arguments in the opposite order: `HAS(List,Value)`. Test its edge cases before substituting it for `oz.IsInListλ`; the names alone do not establish equivalent behaviour.

### Return several rows and columns

For exact lookups, `CHOOSEROWS` with `XMATCH` can return several complete rows without a stacking loop or a preview function:

```excel
=CHOOSEROWS({10,20,30;40,50,60},XMATCH({"B";"A"},{"A";"B"},0))
```

The result is a 2-row, 3-column array in the requested key order:

```text
40  50  60
10  20  30
```

With named ranges, use `=CHOOSEROWS(ReturnTable,XMATCH(Keys,KeyColumn,0))`. Each row of `ReturnTable` must align with the corresponding key in `KeyColumn`. Use unique keys when each key should identify one row. Duplicate source keys select the first match; a missing key returns an error. This recipe does not implement XLOOKUP's approximate matching, reverse search or not-found options.

The concrete membership, substring and lookup examples above were evaluated in Excel 16.0 build 20430 on 25 September 2026. These checks do not certify every supported Excel platform.

### Lists and nested arrays in Beta

Microsoft's [24 September 2026 announcement](https://techcommunity.microsoft.com/blog/microsoft365insiderblog/put-multiple-values-in-one-cell-with-lists-and-arrays-in-excel/4559395) introduces lists and arrays stored in cells, nested arrays, and four functions:

- `HAS` checks membership, while `HASANY` and `HASALL` check whether any or all requested values occur.
- `FLATTEN` removes levels of nesting. `TOCOL` reshapes a flat row, column or grid into one column; the two operations serve different purposes.

The preview is rolling out to the Beta Channel on Windows Version 2610 (Build 20520.20000 or later) and Mac Version 16.114 (Build 26092111 or later). Features may not appear immediately. Microsoft advises against using them in important workbooks until general availability.

Most nested-array calculations require Compatibility Version 3, a workbook setting that can change existing formula results. Test on a separate copy and check the recipient workbook's setting when copying functions. Excel 2024 remains on Compatibility Version 1 according to Microsoft's [compatibility guidance](https://support.microsoft.com/excel/compatibility-versions). That general guidance still lists Version 2 for Beta; the newer announcement describes the Version 3 preview.

The preview has limits: Power Query cannot load or emit array-valued columns, PivotTables do not read array values as source data, charts do not expand them into data points, and data validation cannot use them as dropdown items. Keep structured input tables and flat outputs where those tools need them. These features do not justify replacing validated financial inputs with manually typed lists.

The four preview functions returned `#NAME?` on the build used for the examples above. The existing membership and lookup recipes do not need them.

## Performance and presentation

No formula in the workbook is volatile except the sheet-name titles and the 2 `oz.RangeToDAEλ` demonstration cells (that function wraps OFFSET by design, as the changelog explains), so editing a cell recalculates only what depends on it rather than the whole file. That covers the random-number formulas behind the sample data in both their forms. The sample data is fixed rather than randomly generated, which also means the worked examples match their captions every time you open them.

Each module has its own tab colour, gridlines are hidden, and every sheet opens at the top left on the cover.

The [rolling-sum benchmark](rolling-sum-benchmark.md) records calculation timings
and the numerical checks behind retaining the current `oz.RollingSumλ` formula.
