## Getting started

Download the workbook from the [latest release](https://github.com/ryanduguid/Ozzit/releases/latest). Cloning the repository is not required to use the library, and the release asset is a few hundred kilobytes against the repository's tens of megabytes.

The release carries `ozzit.xlsx` alongside `SHA256SUMS` and `provenance.json`. Check the workbook against the published checksum before opening it:

```bash
curl -LO https://github.com/ryanduguid/Ozzit/releases/latest/download/ozzit.xlsx
curl -LO https://github.com/ryanduguid/Ozzit/releases/latest/download/SHA256SUMS
sha256sum --check --ignore-missing SHA256SUMS
```

On Windows, `Get-FileHash ozzit.xlsx -Algorithm SHA256` prints the same digest to compare against the `SHA256SUMS` line by eye. `SHA256SUMS` deliberately does not cover itself, so read it from the release page rather than trusting a local copy alone. `provenance.json` binds that workbook to its version, signed tag and candidate commit; [RELEASING.md](../RELEASING.md) describes what it asserts.

The workbook tracked in this repository is the candidate the gates below run against. The release asset is that same file copied byte-for-byte from the tagged tree, so the two share a SHA-256 at the tag they were released from. Between releases the tracked copy can be ahead of the published one.

1. Open `ozzit.xlsx`.
2. Cell A1 of every visible worksheet links back to the table of contents; every name in the TOC links to its worksheet.
3. For inline help, type a function name with no arguments in an empty cell, for example `=oz.Amortiseλ()`. The help block spills syntax, parameters and worked examples.
4. Grey-shaded cells on each worksheet are inputs. Change them and watch the function respond.
5. To use the functions in your own workbook, copy a green-shaded cell across (Excel brings the named LAMBDA with it), or import the plain-text source from `src/` with the Advanced Formula Environment in the Excel Labs add-in.

   The repository also includes an [Ozzit 13-week cash-flow forecast template](../templates/README.md) for Australian FP&A planning; see the [template overview](cash-flow-template.md).

   Importing `src/` that way recreates the functions under the module container's own
   name, so `Dates.txt` produces `Dates.CountDOWλ` rather than `oz.CountDOWλ`: the
   Advanced Formula Environment takes the prefix from the container, and one flat
   namespace cannot be six containers. The workbook is the authority for the `oz.`
   names. `src/` is for reading, diffing, and pasting a single definition into Name
   Manager, where the name is yours to choose. To change a function, edit its
   definition in `src/` and run `tools/compile_sources.py`, which renders it into the
   stored form and writes it over the defined name that ships.

The workbook opens showing the numbers its own formulas produce, so nothing has to recalculate before it reads correctly and Excel does not ask you to save a file you never edited. That is a property the build cannot give it: the build edits the workbook as XML with no formula engine, so `tools/refresh_cache.py` recalculates it in Excel afterwards and `tools/verify_cache.py` proves every cached value matches.

Functions with a data-validation companion (named with a `DV` suffix, such as `oz.AmortiseλDV`) diagnose argument problems when the parent function returns something unexpected.

## Modern Excel

Excel 365 has gained functions since this library's predecessor release in July 2024, and a few of them do natively what some helpers here were written to work around. Where that is the case the function's own inline help carries a `SEE ALSO` line, so you find out while you are using it rather than after:

| Helper | Native equivalent in Excel 365 |
|---|---|
| `oz.RangeToDAλ`, `oz.RangeToDAEλ`, `oz.RangeToDAUλ` | `TRIMRANGE`, or trim references (`.:.`) |
| `FilterContainsλ` | `REGEXTEST`, `REGEXEXTRACT` |
| `SumPeriodsλ`, `SumContainsλ` | `GROUPBY`, `PIVOTBY` |

The helpers are kept because they still work on the Excel 2024 baseline and inside the library's own composition, and because the native functions are Microsoft 365 only. Prefer the native function when your audience is on 365. Checked against Microsoft's documentation in August 2026.

## Performance and presentation

The workbook is built to stay responsive on modest hardware. No formula in it is volatile except the sheet-name titles and the two `oz.RangeToDAEλ` demonstration cells (that function wraps OFFSET by design, as the changelog explains), so editing a cell recalculates only what depends on it rather than the whole file. That covers the random-number formulas behind the sample data in both their forms. The sample data is fixed rather than randomly generated, which also means the worked examples match their captions every time you open them.

Each module has its own tab colour, gridlines are hidden, and every sheet opens at the top left on the cover.
