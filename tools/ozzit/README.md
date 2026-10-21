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
replay remains unchanged. The remaining Python helper module is retired too:
its 13 writer, permission and raw-text contracts map to `src/common_retirement.rs`
and `tests/writer_retirement.rs`. Native tests exercise real temporary files,
partial-write errors, panic unwinding, regular-file replacement refusal and
platform permissions. Native stages remain open; Python's buffered-close fault
injection has no native equivalent, and panic cleanup does not cover OS process
termination. The exact live Python column helper now belongs to
`verify_help_spills`, with its original test retargeted. All 34 unrelated tests
remain; review regression fixtures use a simple standard-library ZIP builder.
`compile_sources` is native only. Its 5 former command test contracts run in
`tests/compiler_cli_retirement.rs`, with exact status, output, no-write and index
selection checks. Its 31 remaining compiler contracts run in `tests/foundation.rs`,
`tests/compiler_retirement.rs` and private formula tests, including exact refusals,
index text and inline idempotency. The shared Python argument parser now belongs
to `verify_sources`; its original test follows it. All 10 unrelated comparison
tests remain. The obsolete Python compiler import lifecycle test is retired.
`verify_cache` and its exclusive OPC helper are native only. Their 26 former
test contracts run in `src/cache_retirement.rs`, with physical command refusals
in `tests/cache_retirement_cli.rs`. Fake runners check the complete launch,
600-second deadline, temporary-dump cleanup and unchanged workbook bytes.
All 8 typed error-code combinations and the 12 original cases at exactly
15,000 cells remain checked. The retained column helper test follows its live
Python owner; the other gate import checks and 26 unrelated tests remain.
Generated headers retain historical Python filenames to preserve published bytes.
`verify_signatures` retains its Python reference and callers. Native signature
tests cover the original mismatch fixture, floor refusal, published help defects,
input failures and command defaults. Existing corpus and foundation cases retain
the helper contracts. A private fixture floor leaves the public floor at 100;
the Python-only fresh import test remains unchanged.
The workbook retains its native Excel formulas and ordinary `.xlsx` format.
`check_gates WORKFLOW GATE [--exempt JOB ...] [--no-results]` checks aggregate
job results, missing needs and declared removals against the first-parent base.
It reads `RESULTS` even with `--no-results`; only `pull_request` checks the base.
The narrow job-key grammar, directory-entry path selection and validation order
retain the former `.github/ci/check_gates.py` contracts. All 39 baseline assertion sets transfer to
private unit tests and physical command tests, with additional real Git cases.
The Python helper and its tests have retired. Both ordinary aggregate jobs
build this crate with the pinned toolchain and `cargo build --locked`, then
run `target/debug/ozzit check_gates` with their existing arguments and RESULTS.
They use full checkouts with fetch-depth 2 so compile-time inputs and the first
parent remain available. Their 20-minute job limit allows toolchain installation
and a cold build; install/build failures fail the required aggregate context.
Job names, needs, conditions, permissions and verify's codecov exemption remain.
Native workflow tests execute both actual workflow invocations on owned copies
with successful, failed and missing results. The command is built within each
unprivileged job; it needs no artifact transfer or Python runtime.
Git uses separate arguments, inherited cwd/environment, captured streams and no
timeout. Both Git streams and workflow files decode strictly as UTF-8 with
universal newlines. Untrimmed Git stdout is retained. This intentionally does
not emulate legacy Python locale/Windows ACP decoding; non-UTF-8 Git output
fails closed. Explicit refusals preserve status and output channels, including
declared-removal stdout before undeclared-removal stderr. Python exceptions
become controlled category-prefixed stderr and exit 1 without tracebacks or
exception-object identity. Git termination without an exit code reports -1.
Release Git tests retain the 6 original checkout-binding contracts beside
the staging tests. They check the shipped workbook against its manifest and
full Git history, query order, all claim mismatches, dirty checkout and bad
HEAD refusals, invocation arguments and failure propagation. A positively
shallow clone logs the original history warning and returns from the Rust
test; qualification requires full history and records the real binding run.
The existing runner tests retain UTF-8 and newline decoding checks.
Release command tests retain the 6 original execution contracts with owned
bundle creation and verification. They cover implicit/explicit HEAD, all
requested-claim refusals, clean-check-first ordering, exact logical output and
the independent semantic rerun. Private callbacks retain production root
discovery and command formatting. All Python release tests and callers remain.
Release static tests and the unchanged replay retain the remaining 18 original
method contracts. Added cases cover exact Boolean claims, malformed files,
canonical values, manifest fields, checksum rows and parser refusals. The replay
retains all 22 provenance mutations with type-strict input bindings. The original
Python subprocess AST check remains; native invocation tests check separate
argument vectors through the existing process runner. Production code is unchanged.
`verify_workbook` retains its Python reference and callers. Native tests cover
the original report fixtures, malformed XML and AFE payloads, all add-in and
forbidden-part cases, scoped names, long names and physical command contracts.
The private loader test adds no production seam. Python-only import coverage
remains unchanged; native XML helper errors adapt its global failure list.
`verify_help_spills` retains its Python reference and callers. The unchanged
corpus checks the original help model, attribute permutations, missing caches
and dimension errors. Physical tests retain stale/current output, argument and
read failures, UTF-8 output and input preservation. Exact trim and private
column-index assertions add no production seam. Python import checks remain.
`verify_native_evidence` retains its Python reference and callers. The unchanged
corpus covers the 43 original validation calls. Additional tests retain the
original installed-file success fixture, supplied-file preservation and physical
default/refusal output. Copied executables exercise root resolution on owned
trees. Production code and the native-execution disclaimer remain unchanged.

Build with the repository's pinned Rust toolchain:

```powershell
cargo build --locked --release --manifest-path tools/ozzit/Cargo.toml
tools/ozzit/target/release/ozzit --help
```

The standalone executable currently provides these commands. Run from the
repository root to use their usual path defaults.

| Command | Purpose |
| --- | --- |
| `check_gates` | Check aggregate CI job results, needs and declared removals |
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
when tests run. Previous-name coverage retains the 13 original Python test
contracts, including all header and read-failure cases and physical command
defaults. Its small diagnostic fixtures use a private floor parameter; public
commands retain the floor of 100. The Python gate and live callers remain.
Cover tests retain all 15 original gate contracts, including exact small-fixture
reference failures, stale labels and dates, decoy strings, CSV parsing and
physical command defaults. Their fixtures write only owned copies.
AFE tests retain all 20 original gate contracts, reusing the helper tests and
adding shipped-store failures, workbook/source read failures and physical
cp1252/UTF-8 command checks. Recursive snapshots protect all owned fixture bytes
and directory entries. Production code, Python tests and live callers remain.
Index tests retain the 10 direct and 8 mixed Python contracts through native
fixtures and physical command checks, with the Python-specific import assertion
kept in Python. They cover every original header/CSV matrix, contradictory
duplicates, missing metadata and unreadable inputs. Production code is unchanged.
Source gate tests retain all applicable assertions from 36 Python contracts,
reusing the exact frozen parser cases and adding private known-list fixtures,
canonical equality/difference pairs, read failures and physical command checks.
The public command still uses the same known list and output. Python-specific
import tests and every live Python caller remain.
Differential scripts
and their receipts are retained in the local Agent Hub evidence for this port.
Release staging tests retain all 10 original staging contracts, using real
copies, checksums, structural verification and cleanup in owned synthetic roots.
All 3 staged assets match the original Python fixture bytes and repeat builds
are byte identical. Private callbacks inject gate failures, structural failures
and a destination race; the public command keeps its real checks and operation
order. Every Python release method, caller and recorded replay remains.
Semantic execution tests add the 4 original independent-rerun, path-spelling
and failed-gate contracts. They check all 7 native argument vectors, absolute
operands, normalised evidence and exact diagnostics, including silent failures.
Private callbacks retain the real public executable, cwd, timeouts and checks.
CI, `RELEASING.md` and the postbuild run order call this binary. The Python tools
that remain under test are the reference implementations until retired; the
optional pyxirr comparison stays in Python.
