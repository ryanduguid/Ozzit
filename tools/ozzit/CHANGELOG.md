# Ozzit tooling changes

## 4.0.0 (in progress, unpublished)

The Rust port is being implemented alongside the base Python tools. The Python
tools remain the verification authority until the port passes its acceptance
checks. No workbook, source formula, cached result or release evidence changes
as part of the tooling port.

Dependency qualification: Rust flate2 1.1.10 with zlib-rs 0.6.8 reproduces all
169 deflated members of the base workbook byte for byte on Windows. The writer
also reproduces Python zipfile's sorted directory, fixed date and attributes.
ZIP reading uses zip 9.0.5. JSON, CSV, formula and HTML compatibility use local
helpers. The first twelve commands are implemented; see README.md for their
scope and remaining encoding limits. Unused inherited dependencies were removed
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

These notes do not claim the port or its differential qualification is complete.
