# Ozzit Rust tooling port

This crate is an unpublished migration checkpoint. The Python commands and CI
remain authoritative while the rest of the tooling is ported and qualified.
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
| `verify_index` | Compare the CSV inventory with sources and the workbook |
| `verify_previous_names` | Check the released-name mapping and rename history |
| `verify_afe` | Check the embedded AFE store against the source files |
| `sync_afe_store` | Synchronise AFE modules and the grouped name index |
| `build_module` | Join the six source modules into the published AFE text |

Normal positional paths and existing flags retain their spelling. Replace
`python tools/<name>.py` with `ozzit <name>`. Usage and help identify the native
command. Corrupt ZIP diagnostics include the native parser's reason.
Some Python tracebacks become a controlled error and exit status 1. Boundary
qualification is still in progress. JSON inputs have a native nesting ceiling
of 1,000 containers; deeper inputs are refused before recursive consumers run.

The default released-name baseline is embedded in the executable. An explicit
baseline path still reads that file. The generated module banner retains its
original text so that `oz.txt` remains byte identical.

AFE commands honour `PYTHONIOENCODING` for UTF-8, ASCII, Latin-1 and CP1252,
escaping unrepresentable characters as the Python tools do. Other codec
overrides fail before a writer runs. Python's other codecs and locale-based
defaults remain a compatibility gap. Other commands write UTF-8, matching their
Python tools' explicit stream configuration.

Writers replace workbooks atomically, preserve opaque parts and use the
repository's deterministic ZIP convention. They do not calculate formulas or
create native recalculation evidence. Run the native Excel gates required by
`RELEASING.md` for any release or workbook change that needs cache validation.

Checks for this crate:

```powershell
cargo fmt --manifest-path tools/ozzit/Cargo.toml --check
cargo clippy --locked --manifest-path tools/ozzit/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/ozzit/Cargo.toml
```

The native tests include 1,117 captured observations from the original parser
tests, archive byte checks and controlled writer failures. Differential scripts
and their receipts are retained in the local Agent Hub evidence for this port.
The remaining verifiers, workbook passes, cache orchestration, release tooling
and callers still use Python or their existing platform runtime.
