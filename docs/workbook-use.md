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
5. To use the functions in your own workbook, copy a green-shaded cell across (Excel brings the named LAMBDA with it), or import the plain-text source from `src/` with the Advanced Formula Environment in the Excel Labs add-in.

   The repository also includes an [Ozzit 13-week cash-flow forecast template](../templates/README.md) for Australian FP&A planning; see the [template overview](cash-flow-template.md).

   Importing `src/` that way recreates the functions under the module container's own
   name, so `Dates.txt` produces `Dates.CountDOWλ` rather than `oz.CountDOWλ`: the
   Advanced Formula Environment takes the prefix from the container, and one flat
   namespace cannot be 6 containers. The workbook is the authority for the `oz.`
   names. `src/` is for reading, diffing, and pasting a single definition into Name
   Manager, where the name is yours to choose. To change a function, edit its
   definition in `src/` and run `tools/compile_sources.py`, which renders it into the
   stored form and writes it over the defined name that ships.

The workbook opens showing the numbers its own formulas produce, so nothing has to recalculate before it reads correctly and Excel does not ask you to save a file you never edited. That is a property the build cannot give it: the build edits the workbook as XML with no formula engine, so `tools/refresh_cache.py` recalculates it in Excel afterwards and `tools/verify_cache.py` proves every cached value matches.

Functions with a data-validation companion (named with a `DV` suffix, such as `oz.AmortiseλDV`) diagnose argument problems when the parent function returns something unexpected.

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
