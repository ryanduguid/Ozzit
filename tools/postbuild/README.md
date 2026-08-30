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
python tools/sync_afe_store.py ozzit.xlsx src
python tools/sanitise_workbook.py ozzit.xlsx   # always last, after any Excel save
```

All postbuild passes are idempotent: on a current workbook each reports "already
applied" and writes nothing. A workbook whose anchors do not match the recorded counts
fails loudly rather than writing a partial result. `sync_afe_store.py` then copies the
five non-recursive `src/` modules into the workbook's Advanced Formula Environment
store; this step is required after a text pass changes `src/`. The FY27 and palette passes
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
