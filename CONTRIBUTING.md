# Contributing to Ozzit

`ozzit.xlsx` is the shipped authority. `src/*.txt`, the AFE store and
`functions.csv` are bound publication views and must agree with the same
workbook commit. Do not hand-edit or submit one view alone; follow the
source-owning process in `tools/postbuild/README.md` for a formula change: sync
the AFE store after any `src/` change, and sanitise the workbook last.

Cached formula results may change only with Excel-backed recalculation evidence.
Do not claim a cache refresh from XML tooling: it has no formula engine. Before
the native Excel gates, ensure no user Excel process is running and do not close
or attach to one. Follow the evidence, hash and Excel-version requirements in
`RELEASING.md`.

Run the CI sequence exactly before submitting a change. The first 4 commands
are the `lint` job; the next 2 run on Python 3.14 and 3.15; the rest are the
`rust` job, which builds the native tools in `tools/ozzit` once and runs the
gates with them. Install the git
hooks once with `python -m pip install pre-commit && pre-commit install`; they
run the pinned ruff check on staged files.

```powershell
python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-ruff.txt
python -m ruff check .
python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-mypy.txt
python -m mypy --config-file mypy.ini
python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-citation.txt
python -m unittest discover -s tools/tests -v
rustup toolchain install
cargo fmt --manifest-path tools/ozzit/Cargo.toml --check
cargo clippy --locked --manifest-path tools/ozzit/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/ozzit/Cargo.toml
cargo build --locked --release --manifest-path tools/ozzit/Cargo.toml
tools/ozzit/target/release/ozzit verify_workbook ozzit.xlsx
tools/ozzit/target/release/ozzit verify_sources ozzit.xlsx src
tools/ozzit/target/release/ozzit verify_signatures src
tools/ozzit/target/release/ozzit verify_previous_names functions.csv
tools/ozzit/target/release/ozzit verify_index ozzit.xlsx src functions.csv
tools/ozzit/target/release/ozzit verify_cover ozzit.xlsx functions.csv CHANGELOG.md
tools/ozzit/target/release/ozzit verify_afe ozzit.xlsx src
tools/ozzit/target/release/ozzit verify_help_spills ozzit.xlsx
tools/ozzit/target/release/ozzit verify_native_evidence
```

For a workbook change, also run the native Excel self-test and cached-value
gate named in `RELEASING.md`. Release preparation, tagging and publication are
governed solely by that document and require the stated human approval.

For a change to `templates/13-week-cash-flow-forecast.xlsx`, also run its separate
native gate after confirming no Excel process is running:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\excel_cash_flow_selftest.ps1
```

For `templates/practice-calculators.xlsx`, run
`tools/excel_practice_selftest.ps1` in native Excel and refresh
`docs/practice-native-evidence.json`. Follow the builder, native-link preparation
and sanitising order in [the companion guide](docs/practice-improvements.md#native-preparation-and-checks).
Its portable tests bind the saved workbook, authoring tools and native runner to
that receipt; they do not recalculate formulas without Excel.
