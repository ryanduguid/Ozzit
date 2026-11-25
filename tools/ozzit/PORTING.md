# Ozzit Rust Port Inventory

This document records the porting status of non-Rust tooling in `ryanduguid/Ozzit` under `rust-port-2026-10-11`.

## Ported in this crate

| Source script | Rust target | Verification and status |
| --- | --- | --- |
| `tools/build_practice_calculators.mjs` | `tools/ozzit/src/practice_calculators/` and binary `build_practice_calculators` (`tools/ozzit/src/bin/build_practice_calculators.rs`) | Gate A semantic OOXML parity verified across all 22 sheets, 2,766 formulas, styles, formats, widths, heights, conditional rules, data validations, freeze panes, theme colours and internal hyperlinks (`tools/ozzit/tests/practice_calculators_parity.rs`, plus external semantic parity evidence), with zero fabricated cached formula values. `tools/build_practice_calculators.mjs` and `templates/practice-calculators.xlsx` remain intact because `docs/practice-native-evidence.json` and `tests/repository/practice_template.rs` bind their SHA-256 digests to desktop Excel recalculation; `@oai/artifact-tool` is an external authoring host (Constraint 1 / 2). |
| `tools/*.py`, `.github/ci/check_gates.py` (earlier migration) | `tools/ozzit/src/` and binary `ozzit` | Retired and verified by `tools/ozzit/tests/` as documented in [README.md](README.md). |

## Retained non-Rust scripts (constrained)

| Path | Constraint | Reason |
| --- | --- | --- |
| `tools/excel_selftest.ps1`, `tools/selftest_examples.ps1`, `tools/excel_practice_selftest.ps1`, `tools/prepare_practice_template.ps1`, `tools/excel_cash_flow_selftest.ps1`, `tools/excel_retention_selftest.ps1`, `tools/excel_install_selftest.ps1`, `tools/excel_eval_formulas.ps1`, `tools/excel_eval_formulas_verified.ps1`, `tools/dump_values.ps1`, `tools/refresh_cache.ps1`, `tools/excel_process.ps1`, `tools/benchmark_rolling_sum.ps1` | Constraint 1 / 2 | Windows desktop Excel COM Automation (`Excel.Application`), STA COM message filtering (`Add-Type` C#), native hyperlink preparation and Excel-recalculated cached-value verification. |
| `tools/excel_compatibility.ts` | Constraint 1 | Office Scripts compatibility probe executed inside Excel's Automate tab runtime. |
| `tools/ozzit/gen_pyunicode.py` | Constraint 2 | Reads CPython 3.15.0 `unicodedata` (`17.0.0`) and `re` tables to regenerate frozen Rust tables in `tools/ozzit/src/pyunicode.rs`. |
| `tools/pyxirr_comparison.py` | Constraint 2 | Imports the pinned `pyxirr==0.10.8` wheel and invokes `tools/excel_eval_formulas.ps1` in desktop Excel to produce reference comparisons. |
| `tools/tests/postbuild/generate_quantlib_day_count_fixture.py` | Constraint 2 / 3 | Unpacks the pinned Linux `QuantLib==1.43` wheel (`_QuantLib.abi3.so`) as an external reference fixture generator. |
| `tools/tests/test_repository_policy.py`, `tools/tests/test_pyxirr_comparison.py`, `tools/tests/test_compile_and_compare_paths.py`, `tools/tests/test_excel_eval_formulas.py` | Constraint 1 / 2 | Python test suite exercising `ruamel.yaml` YAML/citation validation, Python AST signature enforcement on `tools/**/*.py`, `pyxirr_comparison.py` mocks and Windows PowerShell COM filter checks. |
