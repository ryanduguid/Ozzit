# Ozzit tooling changes

## 4.0.0 (in progress, unpublished)

`sync_afe_store.py` is retired. Rust retains its 6 former test contracts,
including grouped names, inline idempotency, exact status/refusals and both
output encodings. Updated owned copies restore the complete decoded AFE store
and preserve every unrelated archive part. Shared Python AFE helpers remain.

`refresh_cache.py` is retired. Rust preserves its 8 former test contracts,
including the exact PowerShell arguments, 600-second deadline, failure output
and shared sanitiser call. Active diagnostics name `ozzit refresh_cache`; the
frozen cache corpus stays unchanged with an explicit hint adaptation in replay.

`aasb16_leases.py` and the final Python workbook helper are retired. Rust
retains all 20 writer, 7 shared-helper and final idempotency test contracts,
including exact parser output, source guards, index validation and raw CRLF
rollback. Formulas and independent arithmetic checks are unchanged.

`ato_benchmarks_rate_lookup.py` and its 4 unused shared workbook helpers are
retired, along with the unused Python compiler index-update wrapper. Rust retains
the apply, required-index, insertion, refusal, source-guard
and two-module recovery assertions. Formulas and independent arithmetic checks
are unchanged.

`rate_date_helpers.py` is retired. Its assertions now run in Rust, including
Financial/Dates write order, two-module recovery, optional-index notes, source
guards, final newlines and exact command output. Formula blocks and independent
day-count checks are unchanged.

`working_capital_days.py` is retired. Its apply, partial-state, missing-input,
source-guard and command-line assertions now run in Rust. Private failure hooks
verify source, compile and final workbook failures restore every original store.
Source blocks, arithmetic and the separate AFE sync step are unchanged.

`workbook_palette.py` is retired. Its component, workbook, template, refusal and
command-line assertions now run in Rust, including exact change counts, warning
colours, shared fonts, rich text, indexed backgrounds and byte no-ops.

`help_corrections.py` is retired. Its workbook/source, worksheet, shared-string,
refusal, command-line, temporary-sibling and AFE-order assertions now run in Rust.
A private staging callback exercises a partial source-stage failure and verifies
unchanged destination bytes and cleanup before any replacement.

`remove_residue.py` is retired. Its residue-removal, style-renumbering, freeze-pane,
refusal and command-line assertions now run in Rust, including complete restored
workbook parts, absolute sheet targets and preservation of other binary content types.

`help_links.py` is retired. Its apply, validation and command-line assertions now
run in Rust. Private source-write callbacks exercise both rollback paths and
verify that original workbook ZIP bytes are restored before the source restore.

`fy27_help_text.py` and `gst_help_text.py` are retired. Their character-length,
anchor-count, description, note-formatting and command-line assertions now run
in Rust. The GST sequence still requires an explicit AFE sync after text changes.

`cover_label.py`, `sheet_names.py` and `strip_revision_history.py` are retired.
Their rename, validation, command-line and rollback assertions now run in Rust.
The revision-strip transaction accepts private I/O callbacks so tests can fail
between writes and verify restoration of every source and the original ZIP bytes.

`generate_selftest_examples.py` and the Python fuzz reference are retired.
Their parser, exact decimal, fragment and command-line assertions now run in
Rust. The native fuzz harness also has a refusal control for zero tolerance.
The generated PowerShell fragment retains its bytes and historical header.

`build_module.py` is retired. Its assertions now run against the Rust module
builder, including command-line failures and unchanged destinations. The stale
output diagnostic points to `ozzit build_module`. The generated banner keeps
its historical Python filename so that the published `oz.txt` stays byte
identical.

The active parser fuzzing workflow now exercises the Rust implementations with
libFuzzer. Stable Rust tests replay the shared target and seed corpus. The
former Python harness has been retired after its assertions moved to Rust.

The Rust port is implemented alongside the base Python tools, which remain the
reference implementation: CI runs the gates through this binary and fails when
one disagrees with its Python twin. No workbook, source formula, cached result
or release evidence changes as part of the tooling port.

Dependency qualification: Rust flate2 1.1.10 with zlib-rs 0.6.8 reproduces all
169 deflated members of the base workbook byte for byte on Windows. The writer
also reproduces Python zipfile's sorted directory, fixed date and attributes.
ZIP reading is local: the end record, central directory and members are read
as Python's zipfile reads them. JSON, CSV, formula and HTML compatibility use
local helpers. Thirty-two commands are implemented; see README.md for their
scope and remaining encoding limits. `function_reference` is new, with no Python
twin: it reads each definition's help block into signature, description,
parameters, notes and worked examples, checks every name against the CSV index
and writes one JSON file, so a website or an agent can document every function
from the released source. Unused inherited dependencies were removed
from this checkpoint; later commands will add dependencies only when needed.
The optional pyxirr comparator's fixed-case reference outputs must be pinned
before replacing that runner.
QuantLib is used only by the optional fixture generator, not workbook tooling.
Excel remains the formula engine, driven by the existing PowerShell scripts.

### Deliberate compatibility differences

- XML and corrupt deflate diagnostics name the native parser's reason.
  Rejection, exit codes and unchanged destinations remain required.
- A previously uncaught Python exception becomes one error line and exit 1.
- Command dispatch becomes `ozzit <script_stem>`; normal arguments retain
  their spelling. Usage text will identify the native command.
- Undefined-reference tokens use deterministic order instead of Python's
  hash-set iteration order.
- Compressed ZIP bytes retain the frozen workbook's zlib-ng baseline. Stock
  zlib can encode identical parts and ZIP metadata with different deflate bytes.
- The retained-evidence JSON reader refuses a lone surrogate escape such as
  `"\ud800"`, even in a member the verifier ignores; Python keeps the value.
  The refusal fails closed and no retained record contains one.
- On Windows, source discovery reads only `src/*.txt` with a lowercase
  extension; Python's case-insensitive glob would also read `*.TXT`. The
  repository's modules all use the lowercase extension.
- The archive reader decompresses stored and deflated members only; Python's
  zipfile also reads bzip2, LZMA and Zstandard members. The writer emits
  classic ZIP records only: where Python would switch to ZIP64 (more than
  65,535 members, a member whose size times 1.05 exceeds 2,147,483,647 bytes,
  or a size, offset or directory field above that) it refuses instead. Both
  refusals happen before replacement and leave the destination unchanged. The
  committed workbook and recorded Excel saves use neither feature.
- The archive reader refuses a member whose stored or inflated data is
  shorter than its recorded uncompressed size, and a deflate stream that stops
  without its final block by the time it has yielded that many bytes (an empty
  stream included), even when the CRC matches. zipfile accepts these when all
  the recorded compressed bytes are present. Both refuse a required
  compressed-data read that returns no bytes and a CRC mismatch, and both
  ignore bytes after the first completed deflate stream. The native reader
  inflates the rest of that stream and refuses any decoding error in it;
  zipfile inflates at most 2,147,483,647 bytes per read and stops once it has
  the recorded size, so only the native reader refuses corruption beyond that
  much output. In an archive with another fault, this refusal can be the one
  reported. Excel and the deterministic writer never produce such members.
- The CSV readers decode the whole file before parsing it. Python decodes it
  in chunks as it reads (8,192 bytes at a time), so for an invalid byte beyond
  the first chunk it reports a position within the input it was decoding and
  can report a header problem first; the native reader reports the offset from
  the start of the file and the decoding error first. Both refuse the file
  with exit 1.
- Python's two zipfile warnings (an empty Unicode Path field, and an entry
  whose data reaches a record at its own header) are not printed.

The shared CSV reader removes exactly one UTF-8 BOM. Workbook integrity checks
retain duplicate central-directory entries and read only the parts the Python
gate examines. Cover checking loads its referenced parts lazily. Help checking
compares sparse cached cells without allocating a malformed oversized range.
All three checks remain read-only and create no recalculation evidence.

The retained-evidence verifier checks the eight comparison and two installation
cases, their runner hashes, fixture arithmetic and optional supplied assets.
It preserves the distinction between historical evidence and current workbook
validation. SHA-256 hashing uses the restored sha2 dependency and bounded file
buffers; existing locked package versions are unchanged.

The cached-value gate still drives native Excel through `tools/dump_values.ps1`
and refuses to run while Excel is open. It compares the dump with every cached
value under the same tolerance, error-code map, floor and reporting limits as
the Python gate. A dump that is not valid UTF-8 is decoded whole before rows
are checked, so in a dump larger than one read chunk the decoding error can be
reported ahead of an earlier malformed row, counted from the start of the file.
Captured dumper output that is not UTF-8 fails the run with Python's decoding
message, whatever the exit status, where Python ended with a traceback.
`powershell.exe` is not looked up in the working directory.

The Excel-save sanitiser, the cache refresh and the presentation pass write
through the same deterministic archive writer and reproduce the Python tools'
bytes for the committed workbook, an Excel save of it and every recorded
fixture. The refresh still drives Excel through `tools/refresh_cache.ps1` and
then sanitises the saved file. The presentation pass parses the workbook and
shared-string parts natively: a DOCTYPE is refused and parser errors name the
native reason, where ElementTree accepted an internal DTD and reported its own
position. Unexpected Python exceptions in these writers become one error line
and exit 1.

The self-test example generator writes the committed
`tools/selftest_examples.ps1` byte for byte; printed totals use exact
decimals rounded to Python's 28-digit context. The release bundle tool keeps
its argument handling, checks, messages and provenance schema. Its seven
static gates run as native child commands in the checkout but are still
recorded under the `python tools/...` command names the provenance schema
fixes, so bundles from either tool verify with the other. Help and usage text
is fixed at 80 columns where argparse followed the terminal width, and the
Git blob SHA-1 is computed locally rather than through a new dependency.

`tools/pyxirr_comparison.py` stays in Python: it exists to compare Ozzit with
pyxirr, an independent Python library, and a native runner would have to
replace that live computation with pinned reference values.

The thirteen postbuild passes run as `ozzit <pass>` with their usual
arguments. Their text tables (swap anchors, function sources, About rows and
palette maps) are exported once from the Python passes into
`src/data/postbuild/*.json` and embedded, so no anchor is retyped. The palette's
hue remap reproduces Python's `colorsys` arithmetic and half-even rounding. The
lease pass's signature pattern uses atomic groups in place of possessive
quantifiers. Python exceptions these passes did not catch (a missing
relationship, part or list entry) become one FAIL line and exit 1.

A differential fuzzer run against the Python tools on 20 October 2026 found
three differences that are now fixed. The reader takes the central directory
from the `size_cd` bytes before the end record, as zipfile does, so a wrong
directory size is refused where it was accepted; the end record's disk numbers,
entry counts and comment length are ignored as zipfile ignores them. Workbook
XML refuses character references to surrogates and code points above
U+10FFFF, as expat does. Float output breaks an exact tie between two shortest
digit strings to the even digit, as CPython's `repr` does. The source check's
collapsed-default scan now nests only parentheses and braces, as Python's does,
so an unmatched `]` no longer adds a finding. Review of these repairs found two
more inputs the reader accepted and zipfile refuses: a member whose recorded
compressed data lies past the end of the file, and corrupt deflate data after
the bytes a member needs. Both are now refused.

These notes do not claim the port or its differential qualification is complete.
