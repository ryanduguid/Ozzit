# Ozzit agent guidance

## Authority and scope

`ozzit.xlsx` is the shipped authority. `src/*.txt`, the AFE store and
`functions.csv` are bound publication views: do not edit or approve them in
isolation. The workbook contains native Excel LAMBDA functions only; retain the
ordinary `.xlsx` and no macros. Do not make tax classifications,
individual-tax or Division 7A decisions.

Never fabricate Excel recalculation or cached-value evidence. Cached formula
results may change only with Excel-backed recalculation evidence. XML tooling
has no formula engine; use the documented native Excel gates when a workbook
change requires recalculation or cache validation.

## Verification

Run the CI sequence exactly:

```powershell
python -m pip install "mypy==2.3.1"
python -m mypy --config-file mypy.ini
python tools/verify_workbook.py ozzit.xlsx
python tools/verify_sources.py ozzit.xlsx src
python tools/verify_signatures.py src
python tools/verify_previous_names.py functions.csv
python tools/verify_index.py ozzit.xlsx src functions.csv
python tools/verify_afe.py ozzit.xlsx src
python -m unittest discover -s tools/tests -v
```

For native cache evidence, first ensure no user Excel process is running; never
close or attach to one. Then use `tools/excel_selftest.ps1` and
`tools/verify_cache.py` as specified in `RELEASING.md`, recording the required
Excel and hash evidence.

## Releases

Route all release work through `RELEASING.md`. It governs the clean candidate,
human approval, signed tag, independent verification and release publication;
these instructions do not authorise any publication action.
