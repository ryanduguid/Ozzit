# Ozzit tooling changes

## 4.0.0 (in progress, unpublished)

The Rust port is implemented alongside the base Python tools, which remain the
reference implementation: CI runs the gates through this binary and fails when
one disagrees with its Python twin. No workbook, source formula, cached result
or release evidence changes as part of the tooling port.

Dependency qualification: Rust flate2 1.1.10 with zlib-rs 0.6.8 reproduces all
169 deflated members of the base workbook byte for byte on Windows. The writer
also reproduces Python zipfile's sorted directory, fixed date and attributes.
ZIP reading uses zip 9.0.5. JSON, CSV, formula and HTML compatibility use local
helpers. Thirty-two commands are implemented; see README.md for their
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

- XML and corrupt ZIP parser diagnostics name the native parser's reason.
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

These notes do not claim the port or its differential qualification is complete.
