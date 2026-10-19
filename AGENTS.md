# Ozzit agent guidance

## Authority and scope

`ozzit.xlsx` is the shipped authority. `src/*.txt`, the AFE store, `oz.txt` and
`functions.csv` are bound publication views: do not edit or approve them in
isolation. The library has 143 native Excel LAMBDA functions and 5 named help
tables. Retain the ordinary `.xlsx` and no macros. Do not make tax classifications,
individual-tax or Division 7A decisions. Agent-facing guidance for this estate follows
DrDebits, https://github.com/ryanduguid/llm-tax-guardrails, guide version 0.3.3.
Functions and help may document rates with sources and check dates, and show
worked arithmetic results. The reviewer decides the applicable tax treatment.

For formula changes, follow the required run order in `tools/postbuild/README.md`:
sync the AFE store after any `src/` change, and sanitise the workbook last.

Never fabricate Excel recalculation or cached-value evidence. Cached formula
results may change only with Excel-backed recalculation evidence. XML tooling
has no formula engine; use the documented native Excel gates when a workbook
change requires recalculation or cache validation.

## Verification

Run the CI sequence exactly. The first 4 commands are the `lint` job; the next 2
run on Python 3.14 and 3.15; the rest are the `rust` job, which builds the native
tools in `tools/ozzit` once and runs the gates with them. Its last command reruns
each gate with its Python reference tool, on the real inputs and on broken copies,
and fails if exit code or output differ:

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
python .github/ci/compare_gates.py
```

For native cache evidence, first ensure no user Excel process is running; never
close or attach to one. Then use `tools/excel_selftest.ps1` and
`ozzit verify_cache` as specified in `RELEASING.md`, recording the required
Excel and hash evidence.

## Releases

Route all release work through `RELEASING.md`. It governs the clean candidate,
human approval, signed tag, independent verification and release publication;
these instructions do not authorise any publication action.
