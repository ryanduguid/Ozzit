# Ozzit Rust tooling port

This crate is an unpublished migration checkpoint. CI, `RELEASING.md` and the
postbuild run order call this binary; the Python tools stay as the reference
implementation, and CI fails when a gate and its Python twin disagree.
`build_module` is native only; its former Python assertions now run in
`tests/module_contracts.rs`, including the physical command-line checks.
`generate_selftest_examples` is also native only, with its former assertions in
`tests/selftest_retirement.rs` and exact decimal checks in `src/selftest.rs`.
`cover_label`, `sheet_names` and `strip_revision_history` are native only. Their
former assertions run in `tests/postbuild_retirement.rs`, with transaction
failure checks beside the revision-strip implementation.
`fy27_help_text` and `gst_help_text` are native only. Their former assertions run
in `tests/help_text_retirement.rs`, including character lengths, note formatting,
unchanged descriptions and the separate AFE sync step.
`help_links` is native only. Its former apply and refusal assertions run in
`tests/help_links_retirement.rs`, with partial-write and failed-restore checks
beside the implementation.
`remove_residue` is native only. Whole-workbook restoration and command checks
run in `tests/remove_residue_retirement.rs`; component refusals and binary
content-type checks run beside the implementation.
`help_corrections` is native only. Its workbook/source, example, static-string,
temporary-sibling and command checks run in `tests/help_corrections_retirement.rs`.
Private staging callbacks test a partial source-stage failure before any replacement.
`workbook_palette` is native only. Workbook, template, indexed-background and
command checks run in `tests/workbook_palette_retirement.rs`; colour, theme,
font-sharing and fixed-point assertions run beside the implementation.
`working_capital_days` is native only. Apply, refusal and command checks run in
`tests/working_capital_retirement.rs`; source guards, module endings and all 3
write/compile recovery paths run beside the shared function-pass implementation.
`rate_date_helpers` is native only. Its apply, optional-index and command checks
run in `tests/rate_date_retirement.rs`; two-module recovery, source guards and
newline assertions run beside the shared function-pass implementation.
`ato_benchmarks_rate_lookup` is native only. Apply, required-index, refusal and
command checks run in `tests/ato_retirement.rs`; two-module recovery, insertion
anchors/followers and its source guard run beside the shared implementation.
`aasb16_leases` is native only. Its apply, refusal and final byte-no-op checks
run in `tests/leases_retirement.rs`; exact parser, source-guard, index, shared
helper and CRLF recovery checks run beside the lease implementation. The final
Python workbook helper has retired.
`refresh_cache` is native only. Its former process-launch, timeout, failure,
argument-selection and sanitiser-delegation assertions run beside `src/refresh.rs`.
Diagnostic hints now name `ozzit refresh_cache`; replay adapts only the 16 old
command-hint lines in the unchanged 76-case cache corpus.
`sync_afe_store` is native only. Its 6 former Python test contracts run in
`tests/afe_contracts.rs`, including both physical output encodings, exact command
status/refusals, whole-store restoration, unrelated ZIP parts and idempotency.
`polish_workbook` is native only. Its 13 former Python test contracts run in
`tests/polish_retirement.rs`, reusing the unchanged 31-case corpus input. They
retain exact presentation edits, refusals, path resolution and idempotency.
`sanitise_workbook` is native only. Its 14 former Python test contracts run in
`tests/sanitise_retirement.rs` and a private cell-pattern unit test. The 33-case
replay remains unchanged. Python text and archive helpers remain for callers.
Generated headers retain historical Python filenames to preserve published bytes.
The workbook retains its native Excel formulas and ordinary `.xlsx` format.

Build with the repository's pinned Rust toolchain:

```powershell
cargo build --locked --release --manifest-path tools/ozzit/Cargo.toml
tools/ozzit/target/release/ozzit --help
```

The standalone executable currently provides these commands. Run from the
repository root to use their usual path defaults.

| Command | Purpose |
| --- | --- |
| `compile_sources` | Compile source definitions and optionally rebuild the CSV index |
| `verify_sources` | Compare source definitions with stored workbook names |
| `verify_signatures` | Check signatures, parameter tables and worked examples |
| `function_reference` | Write each function's signature, parameters, notes and worked examples from its help, joined to the CSV index, as JSON. It has no Python twin |
| `verify_index` | Compare the CSV inventory with sources and the workbook |
| `verify_previous_names` | Check the released-name mapping and rename history |
| `verify_afe` | Check the embedded AFE store against the source files |
| `sync_afe_store` | Synchronise AFE modules and the grouped name index |
| `build_module` | Join the six source modules into the published AFE text |
| `verify_cover` | Resolve the displayed version label and compare its date and function count |
| `verify_help_spills` | Report help caches that differ from the definitions |
| `verify_workbook` | Check workbook structure, content, add-ins and references |
| `verify_native_evidence` | Check retained comparison and installation records against their inputs |
| `verify_cache` | Compare the workbook's cached values with native Excel's recalculated values |
| `refresh_cache` | Recalculate and save the workbook in Excel, then sanitise the saved file |
| `sanitise_workbook` | Remove what an Excel save adds and rewrite the archive deterministically |
| `polish_workbook` | Apply the presentation rules to straplines, cover wording and styles |
| `generate_selftest_examples` | Turn the help's worked examples into native self-test assertions |
| `prepare_release_bundle` | Create or verify the copy-only release bundle and its provenance |
| `cover_label`, `sheet_names`, `help_links`, `fy27_help_text`, `gst_help_text`, `strip_revision_history`, `remove_residue`, `workbook_palette`, `help_corrections`, `aasb16_leases`, `rate_date_helpers`, `working_capital_days`, `ato_benchmarks_rate_lookup`, `variance_fy_periods` | The postbuild passes in `tools/postbuild/README.md`, with the same arguments and run order |

Normal positional paths and existing flags retain their spelling. Replace
`python tools/<name>.py` with `ozzit <name>`. Usage and help identify the native
command. XML and corrupt deflate diagnostics include the native parser's reason.
Undefined-reference findings have deterministic token order.
Some Python tracebacks become a controlled error and exit status 1. Boundary
qualification is still in progress. JSON inputs have a native nesting ceiling
of 1,000 containers; deeper inputs are refused before recursive consumers run.

`verify_cache` and `refresh_cache` drive native Excel through
`tools/dump_values.ps1` and `tools/refresh_cache.ps1`, found
beside the executable or in its ancestors, falling back to the working
directory. It needs Windows, Excel and `powershell.exe`, and stays a local gate.

The default released-name baseline is embedded in the executable. An explicit
baseline path still reads that file. The generated module banner retains its
original text so that `oz.txt` remains byte identical.

The retained-evidence command locates repository inputs beside the executable
or in its ancestors, falling back to the working directory. It checks the
recorded runner hashes and fixture results without executing Excel or pyxirr.
Its evidence JSON rejects duplicate keys at every object level. Other JSON
consumers retain their original last-value behaviour.

AFE commands honour `PYTHONIOENCODING` for UTF-8, ASCII, Latin-1 and CP1252,
escaping unrepresentable characters as the Python tools do. Other codec
overrides fail before a writer runs. Python's other codecs and locale-based
defaults remain a compatibility gap. Other commands write UTF-8. The retained
evidence command's Python locale and stream-encoding overrides remain unqualified.

Writers replace workbooks atomically, preserve opaque parts and use the
repository's deterministic ZIP convention. Compressed bytes target the frozen
workbook and the tested zlib-ng baseline. Python builds linked to stock zlib
can produce different deflate bytes for the same parts and ZIP metadata.
Writers do not calculate formulas or create native recalculation evidence.
Run the native Excel gates required by
`RELEASING.md` for any release or workbook change that needs cache validation.

Checks for this crate:

```powershell
cargo fmt --manifest-path tools/ozzit/Cargo.toml --check
cargo clippy --locked --manifest-path tools/ozzit/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/ozzit/Cargo.toml
```

The stable tests replay the shared native parser fuzz harness and committed
seeds. The separate nightly target and bounded Linux campaign are documented
in `fuzz/README.md` at the repository root.

The native tests include 1,117 captured parser observations, 76 recorded
cache-gate findings, 64 recorded sanitiser and presentation-pass results, 834 recorded generator
observations, 146 release-bundle argument and verification cases, 69 workbook and
help-cache observations, 5,461 strict base64 cases, 1,332 retained-evidence/grid
observations, archive byte checks and
controlled writer failures. Large test fixtures are read from `tests/data/`
when tests run. Differential scripts
and their receipts are retained in the local Agent Hub evidence for this port.
CI, `RELEASING.md` and the postbuild run order call this binary. The Python tools
that remain under test are the reference implementations until retired; the
optional pyxirr comparison stays in Python.
