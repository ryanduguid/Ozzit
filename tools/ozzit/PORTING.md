# Ozzit Rust port inventory

The repository's tooling is ported to Rust in this crate wherever the host and the libraries it needs allow. This file lists what has been ported, how parity is checked, and what stays in another language and why.

## Ported in this crate

| Source script | Rust target | Verification and status |
| --- | --- | --- |
| `tools/build_practice_calculators.mjs` | `tools/ozzit/src/practice_calculators/` and the binary `build_practice_calculators` (`tools/ozzit/src/bin/build_practice_calculators.rs`) | `tools/ozzit/tests/practice_calculators_parity.rs` compares the Rust output with the shipped `templates/practice-calculators.xlsx`, built from the JavaScript builder: all 22 sheets, 2,766 formulas, styles, number formats, column widths, row heights, conditional formatting, data validation, freeze panes, theme colours and internal hyperlinks agree, and no formula carries a cached value. The binary writes to `--out <path>` or `<path>` and needs `--force` to overwrite `templates/practice-calculators.xlsx`. The JavaScript builder and the template stay: `docs/practice-native-evidence.json` and `tests/repository/practice_template.rs` bind their SHA-256 digests to a desktop Excel recalculation, and the builder runs inside an external authoring host. |
| `tools/*.py`, `.github/ci/check_gates.py` (earlier migration) | `tools/ozzit/src/` and the binary `ozzit` | Retired; `tools/ozzit/tests/` covers the behaviour, as [README.md](README.md) describes. |

## Kept in another language

| Path | Why it stays |
| --- | --- |
| `tools/excel_selftest.ps1`, `tools/selftest_examples.ps1`, `tools/excel_practice_selftest.ps1`, `tools/prepare_practice_template.ps1`, `tools/excel_cash_flow_selftest.ps1`, `tools/excel_retention_selftest.ps1`, `tools/excel_install_selftest.ps1`, `tools/excel_eval_formulas.ps1`, `tools/excel_eval_formulas_verified.ps1`, `tools/dump_values.ps1`, `tools/refresh_cache.ps1`, `tools/excel_process.ps1`, `tools/benchmark_rolling_sum.ps1` | They drive desktop Excel through COM Automation (`Excel.Application`), with an STA COM message filter (`Add-Type` C#), to prepare hyperlinks and verify Excel-recalculated cached values. |
| `tools/excel_compatibility.ts` | An Office Script: it runs inside Excel's Automate tab, which accepts only TypeScript. |
| `tools/ozzit/gen_pyunicode.py` | Reads CPython 3.15.0's `unicodedata` (`17.0.0`) and `re` tables to regenerate the frozen tables in `tools/ozzit/src/pyunicode.rs`. |
| `tools/pyxirr_comparison.py` | Imports the pinned `pyxirr==0.10.8` wheel as the independent reference and calls `tools/excel_eval_formulas.ps1` in desktop Excel. |
| `tools/tests/postbuild/generate_quantlib_day_count_fixture.py` | Unpacks the pinned Linux `QuantLib==1.43` wheel (`_QuantLib.abi3.so`) to generate the external day-count reference. |
| `tools/tests/test_repository_policy.py`, `tools/tests/test_pyxirr_comparison.py`, `tools/tests/test_compile_and_compare_paths.py`, `tools/tests/test_excel_eval_formulas.py` | Test the Python scripts above: `ruamel.yaml` citation checks, signature checks on `tools/**/*.py`, `pyxirr_comparison.py` with mocks and the PowerShell COM filter. |
