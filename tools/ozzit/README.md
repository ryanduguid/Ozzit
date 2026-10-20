# Ozzit Rust tooling port

This crate is an unpublished migration checkpoint. CI, `RELEASING.md` and the
postbuild run order call this binary; the Python tools stay as the reference
implementation, and CI fails when a gate and its Python twin disagree.
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

The native tests include 1,117 captured parser observations, 76 recorded
cache-gate findings, 64 recorded sanitiser and presentation-pass results, 834 recorded generator
observations, 146 release-bundle argument and verification cases, 69 workbook and
help-cache observations, 5,461 strict base64 cases, 1,332 retained-evidence/grid
observations, archive byte checks and
controlled writer failures. Large test fixtures are read from `tests/data/`
when tests run. Differential scripts
and their receipts are retained in the local Agent Hub evidence for this port.
CI, `RELEASING.md` and the postbuild run order call this binary. The Python tools
remain under test as the reference implementation until they are retired; the
optional pyxirr comparison stays in Python.
