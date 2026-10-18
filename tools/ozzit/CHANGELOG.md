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
helpers. The first eight commands are implemented; see README.md for their
scope and remaining encoding limits. Unused inherited dependencies were removed
from this checkpoint; later commands will add dependencies only when needed.
The optional pyxirr comparator's fixed-case reference outputs must be pinned
before replacing that runner.
QuantLib is used only by the optional fixture generator, not workbook tooling.
Excel remains the formula engine, driven by the existing PowerShell scripts.

### Deliberate diagnostic differences

- XML and corrupt ZIP parser diagnostics name the native parser's reason.
  Rejection, exit codes and unchanged destinations remain required.
- A previously uncaught Python exception becomes one error line and exit 1.
- Command dispatch becomes `ozzit <script_stem>`; normal arguments retain
  their spelling. Usage text will identify the native command.

These notes do not claim the port or its differential qualification is complete.
