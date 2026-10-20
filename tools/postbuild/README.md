# Postbuild passes

The v3.0.0 baseline came from a one-shot transform that needed the uncommitted earlier
workbook and stopped at v3.0.0; commit 96b9939 keeps the
script and the byte length and SHA-256 of the input it expects. Later passes start from the
committed `ozzit.xlsx` and `src/`. This directory holds
the ones that are deterministic and safe to re-run.

FY27 help text and the workbook palette are the tracked successors of the one-off
session scripts that produced v3.1.0. Later passes, including GST help text, start
from the committed `ozzit.xlsx` and `src/`. They do not read the earlier workbook.

Each pass is a standalone script, run by path and never imported as a package.
`workbook.py` is the one exception: it is the shared reader the passes import, and
it holds the archive reading, the applied-or-absent check and the swap replacement
that used to be copied from pass to pass.

The shared writers create a unique temporary file beside each destination and
close it before replacement. Existing `.tmp` siblings are left alone. On POSIX,
existing permission bits are preserved and new files start with private permissions.
An exception cleans up the writer's temporary file, but a hard process stop may
leave one behind.

The help-correction pass stages all changed files before replacing any of them.
A staging failure leaves the destinations unchanged. Replacements still happen
one file at a time, so a later replacement failure can leave earlier changes in
place. Run postbuild passes one at a time; unique staging does not prevent lost
updates from concurrent edits.

## Run order

The passes run through the native tools. Build them once with
`cargo build --locked --release --manifest-path tools/ozzit/Cargo.toml`; each command
below keeps the arguments of the Python pass it replaces. The Python passes in this
directory stay as the reference implementation until they are retired.

```bash
tools/ozzit/target/release/ozzit fy27_help_text ozzit.xlsx src
tools/ozzit/target/release/ozzit workbook_palette ozzit.xlsx
tools/ozzit/target/release/ozzit gst_help_text ozzit.xlsx src
tools/ozzit/target/release/ozzit strip_revision_history ozzit.xlsx src
tools/ozzit/target/release/ozzit help_corrections ozzit.xlsx src
tools/ozzit/target/release/ozzit aasb16_leases ozzit.xlsx src functions.csv
tools/ozzit/target/release/ozzit rate_date_helpers ozzit.xlsx src functions.csv
tools/ozzit/target/release/ozzit working_capital_days ozzit.xlsx src functions.csv
tools/ozzit/target/release/ozzit ato_benchmarks_rate_lookup ozzit.xlsx src functions.csv
tools/ozzit/target/release/ozzit variance_fy_periods ozzit.xlsx src functions.csv
tools/ozzit/target/release/ozzit cover_label ozzit.xlsx
tools/ozzit/target/release/ozzit compile_sources ozzit.xlsx src --index=functions.csv
tools/ozzit/target/release/ozzit build_module src oz.txt
tools/ozzit/target/release/ozzit remove_residue ozzit.xlsx
tools/ozzit/target/release/ozzit sync_afe_store ozzit.xlsx src
tools/ozzit/target/release/ozzit generate_selftest_examples src
tools/ozzit/target/release/ozzit help_links ozzit.xlsx src
tools/ozzit/target/release/ozzit sheet_names ozzit.xlsx
tools/ozzit/target/release/ozzit sanitise_workbook ozzit.xlsx   # always last, after any Excel save
```

The templates take the colour roles alone, without the Ozzit-specific folds:

```bash
tools/ozzit/target/release/ozzit workbook_palette --template templates/13-week-cash-flow-forecast.xlsx
tools/ozzit/target/release/ozzit workbook_palette --template templates/retention-reconciliation.xlsx
```

After `ozzit refresh_cache` has saved the workbook through Excel, run
`ozzit remove_residue` again and then `ozzit sanitise_workbook`:
an Excel save leaves named cell styles unused, renumbers the worksheet parts and
splits long string literals, and those 2 passes put the file back into the
state every gate expects.

All postbuild passes are idempotent: on a current workbook each reports "already
applied" and writes nothing. A workbook whose anchors do not match the recorded counts
fails loudly rather than writing a partial result.

## Changing a function

Edit its definition in `src/`, then run `ozzit compile_sources`. It renders every
published definition into the form the workbook stores, proves the rendering
reproduces its source through `verify_sources`'s own comparison, writes only the
definitions that changed over the defined names that ship, sets each Name Manager
comment from the source header, and with `--index` regenerates `functions.csv`. The
compiler never touches a cached value: a change that alters what a demonstration cell
computes still needs `ozzit refresh_cache`, and `ozzit verify_cache` is the gate
that proves it. `--check` reports what would change without writing. A help change is a
cached-value change too: the table each demonstration sheet's `=oz.Nameλ()` anchor spills
is cached on the sheet, and the compiler leaves that cache as Excel last wrote it.
`ozzit verify_help_spills` reports, without Excel, which cached helps no longer match
their definitions; `ozzit refresh_cache` in Excel is what refreshes them, and
`ozzit verify_cache` proves the result.

`remove_residue` drops what nothing in the workbook reads (the hidden FMTs sheet, the
per-sheet custom properties, the stale custom-function declaration on the Excel Labs
reference, unused differential formats and named styles) and freezes the label columns
on the 6 wide demonstration sheets. `sync_afe_store` then copies all 6 `src/`
modules into the workbook's Advanced Formula Environment store; this step is required
after any pass that changes `src/`. `generate_selftest_examples` rewrites the native
self-test's generated assertions from the help, and the tool tests fail when that
fragment is stale. The FY27 and palette passes
record the v3.0.0 → v3.1.0 swaps. The GST help pass records a later insert against the
committed v3.1.0 workbook and `src/`. The revision-history pass removes the
per-function REVISIONS blocks, the Advanced Formula Environment copies of them and
the workbook's creator credit; it resynchronises the AFE store itself, so it must
run after any text pass that touches `src/`. A build that starts from the earlier
workbook still emits those blocks at v3.0.0, so this pass is what removes them. The
help-corrections pass is the last of the repair passes: it repairs the functions that shipped
disagreeing with their own inline help. Two of the corrected examples are spilled onto
demonstration worksheets, so it rewrites the cells caching that spill. It also
corrects the retention-ratio demonstration formula in `oz.FinancialRatios!A61`,
leaving its value and B61's formula-text cache for native Excel to refresh. Five of the
corrected statements are also typed into label and description cells that no formula
feeds, so it rewrites those shared strings too. The rate and date helpers pass runs
after it: it adds 4 functions (`oz.PeriodRateλ`, `oz.AnnualRateλ`,
`oz.DayCountRateλ` and `oz.DateDifλ`) on the same pattern as the AASB 16 pass, except
that it renders the stored forms through `compile_sources.py` rather than carrying a
renderer of its own. The working-capital days pass follows it on the same pattern: it adds 4 ratio
functions (`oz.ReceivableDaysλ`, `oz.PayableDaysλ`, `oz.WIPDaysλ` and
`oz.CashConversionCycleλ`) and their rows in the Ratios About table. The ATO benchmark
and rate pass adds 5 ratio functions that restate the ATO's
tax return benchmark ratios (`oz.ATOTurnoverλ`, `oz.ATOBenchmarkRatioλ`,
`oz.ATOTotalExpensesRatioλ`, `oz.ATOCostOfSalesRatioλ` and `oz.ATOLabourRatioλ`) under
a new heading in the Ratios About table, and `oz.RateAtDateλ` to the Dates group. Its
blocks go before the ones the two earlier passes appended, so their own contract tests
still find them at the end of each module. The variance and financial-year period pass
is the last text pass and exists only as the native `ozzit variance_fy_periods`: it adds
`oz.Varianceλ`, `oz.VariancePctλ` and `oz.YearToDateλ` under a new REPORTING heading in
the Financial About table, before the rate helpers' blocks, and `oz.FinancialMonthλ` and
`oz.FinancialQuarterλ` to the Dates group after `oz.DateDifλ`, before `oz.RateAtDateλ`'s
block. Its Dates rows go above `oz.FinancialYearλ`'s, because the help-corrections pass
recognises its own result by that row closing the table. The cover-label pass is one swap in `xl/sharedStrings.xml`:
Cover!A3 kept reading 20 August 2026 and 130 functions after three cuts, and
`ozzit verify_cover` now keeps that label equal to `functions.csv` and the
changelog's current cut. The pass accepts any earlier label it records, so it also
carries the count from 133 to 137 for the working-capital functions, to 143 with the
v3.5.0 cut date for the ATO benchmark and rate functions, and to 148 for the variance
and financial-year period functions.

## What is intentionally not here

**The FY27 cell-date shift** (`tools/postbuild/fy27_dates.ps1` in the v3.1.0 session) is
not re-runnable and is not ported. It computed each sheet's month offset from the sheet's
*current* earliest date and rewrote cells through Excel COM, so its result depends on the
workbook's state at the moment it ran. Two runs from different starting states give
different bytes. That is why the reproducible baseline stops at v3.0.0 and
why byte-for-byte reproduction of the current workbook is not claimed.

The one-off scripts that produced v3.1.0 remain in the session record; the FY27 and
palette passes here are the parts of that work that are deterministic. The GST help pass
is a later committed-input insert, not a regeneration from the earlier workbook.
