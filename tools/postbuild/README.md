# Postbuild passes

The v3.0.0 baseline comes from `tools/transform_from_upstream.py`, which still needs
the uncommitted upstream workbook and stops at v3.0.0. Later passes start from the
committed `ozzit.xlsx` and `src/` recorded in ATTRIBUTION.md. This directory holds
the ones that are deterministic and safe to re-run.

## Run order

```bash
python tools/postbuild/fy27_help_text.py ozzit.xlsx src
python tools/postbuild/workbook_palette.py ozzit.xlsx
python tools/postbuild/gst_help_text.py ozzit.xlsx src
python tools/postbuild/strip_revision_history.py ozzit.xlsx src
python tools/postbuild/help_corrections.py ozzit.xlsx src
python tools/compile_sources.py ozzit.xlsx src --index=functions.csv
python tools/postbuild/refresh_help_spills.py ozzit.xlsx
python tools/postbuild/remove_residue.py ozzit.xlsx
python tools/sync_afe_store.py ozzit.xlsx src
python tools/generate_selftest_examples.py src
python tools/sanitise_workbook.py ozzit.xlsx   # always last, after any Excel save
```

All postbuild passes are idempotent: on a current workbook each reports "already
applied" and writes nothing. A workbook whose anchors do not match the recorded counts
fails loudly rather than writing a partial result.

## Changing a function

Edit its definition in `src/`, then run `tools/compile_sources.py`. It renders every
published definition into the form the workbook stores, proves the rendering
reproduces its source through `verify_sources.py`'s own comparison, writes only the
definitions that changed over the defined names that ship, sets each Name Manager
comment from the source header, and with `--index` regenerates `functions.csv`. The
compiler never touches a cached value: a change that alters what a demonstration cell
computes still needs `tools/refresh_cache.py`, and `tools/verify_cache.py` is the gate
that proves it. `--check` reports what would change without writing. The one result
that needs no formula engine is the help itself: `refresh_help_spills.py` recomputes
the table each demonstration sheet's `=oz.Nameλ()` anchor spills, the way TRIM and
TEXTSPLIT would, and rewrites the cells caching it, so a help change reads correctly
on open and the cached-value gate has nothing to report.

`remove_residue.py` drops what nothing in the workbook reads (the hidden FMTs sheet, the
per-sheet custom properties, the stale custom-function declaration on the Excel Labs
reference, unused differential formats and named styles) and freezes the label columns
on the six wide demonstration sheets. `sync_afe_store.py` then copies all six `src/`
modules into the workbook's Advanced Formula Environment store; this step is required
after any pass that changes `src/`. `generate_selftest_examples.py` rewrites the native
self-test's generated assertions from the help, and the tool tests fail when that
fragment is stale. The FY27 and palette passes
record the v3.0.0 → v3.1.0 swaps. The GST help pass records a later insert against the
committed v3.1.0 workbook and `src/`. The revision-history pass removes the
per-function REVISIONS blocks, the Advanced Formula Environment copies of them and
the workbook's creator credit; it resynchronises the AFE store itself, so it must
run after any text pass that touches `src/`. A build that starts from the upstream
workbook still emits those blocks at v3.0.0, so this pass is what removes them. The
help-corrections pass is the last text pass: it repairs the functions that shipped
disagreeing with their own inline help. Two of the corrected examples are spilled onto
demonstration worksheets, so it rewrites the cells caching that spill, and five of the
corrected statements are also typed into label and description cells that no formula
feeds, so it rewrites those shared strings too.

## What is intentionally not here

**The FY27 cell-date shift** (`tools/postbuild/fy27_dates.ps1` in the v3.1.0 session) is
not re-runnable and is not ported. It computed each sheet's month offset from the sheet's
*current* earliest date and rewrote cells through Excel COM, so its result depends on the
workbook's state at the moment it ran. Two runs from different starting states give
different bytes. That is why ATTRIBUTION.md scopes the reproducible baseline to v3.0.0 and
states that byte-for-byte reproduction of the current workbook is not claimed.

The one-off scripts that produced v3.1.0 remain in the session record; the FY27 and
palette passes here are the parts of that work that are deterministic. The GST help pass
is a later committed-input insert, not a regeneration from upstream.
