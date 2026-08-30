# Contributing to Ozzit

`ozzit.xlsx` is the shipped authority. `src/*.txt`, the AFE store and
`functions.csv` are bound publication views and must agree with the same
workbook commit. Do not hand-edit or submit one view alone; follow the
documented source-owning process for a formula change.

Cached formula results may change only with Excel-backed recalculation evidence.
Do not claim a cache refresh from XML tooling: it has no formula engine. Before
the native Excel gates, ensure no user Excel process is running and do not close
or attach to one. Follow the evidence, hash and Excel-version requirements in
`RELEASING.md`.

Run the CI sequence exactly before submitting a change:

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

For a workbook change, also run the native Excel self-test and cached-value
gate named in `RELEASING.md`. Release preparation, tagging and publication are
governed solely by that document and require the stated human approval.
