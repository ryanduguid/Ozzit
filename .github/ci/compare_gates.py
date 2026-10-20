"""Run each native gate in the verify workflow with its Python reference tool too.

usage: python .github/ci/compare_gates.py

Run from the repository root after the release build. It reads every
"run: tools/ozzit/target/release/ozzit verify_*" step in
.github/workflows/verify.yml, runs that command and "python tools/<gate>.py"
with the same arguments, and fails when no gate is found or when a pair's exit
codes, stdout or stderr differ. Line endings are normalised.

It then repeats every gate that reads a broken input on a copy of the inputs
in a temporary directory, so the pair must also agree on what fails and how it
is reported. The checkout's own files are never changed. Usage text names the
native command, but it appears only on an argument error, which none of these
runs make, so no difference is allowed for it.
"""

import csv
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path

WORKFLOW = ".github/workflows/verify.yml"
GATE = re.compile(r"^\s*run:\s*(tools/ozzit/target/release/ozzit\s+verify_\w+.*?)\s*$")
INPUTS = ("ozzit.xlsx", "src", "functions.csv", "CHANGELOG.md")
# The one documented wording difference: a corrupt ZIP's reason comes from each
# implementation's own parser (tools/ozzit/README.md). Only this exact pair of
# strings is allowed; any other difference in the same output still fails.
ZIP_REASON = ("invalid ZIP: invalid Zip archive: Could not find EOCD", "File is not a zip file")


def drop_function_row(root: Path) -> None:
    # Drop a row that records a previous name and that the cover count includes
    # (it leaves out the About functions), so every gate reading the file sees the loss.
    path = root / "functions.csv"
    lines = path.read_bytes().decode("utf-8").splitlines(keepends=True)
    rows = list(csv.reader(lines))
    if len(rows) != len(lines):
        raise SystemExit("compare_gates: functions.csv has a multi-line row")
    index = next(i for i, row in enumerate(rows) if i and row[2] and not row[0].startswith("oz.About"))
    path.write_bytes("".join(lines[:index] + lines[index + 1 :]).encode("utf-8"))


def rename_parameter(root: Path) -> None:
    # CountDOW's declaration changes; its help, body and functions.csv do not.
    path = root / "src" / "Dates.txt"
    text = path.read_bytes().decode("utf-8")
    if "CountDOWλ = LAMBDA(" not in text or "[Starts]," not in text:
        raise SystemExit("compare_gates: src/Dates.txt no longer declares CountDOWλ's [Starts]")
    path.write_bytes(text.replace("[Starts],", "[Begins],", 1).encode("utf-8"))


def truncate_workbook(root: Path) -> None:
    path = root / "ozzit.xlsx"
    data = path.read_bytes()
    path.write_bytes(data[: len(data) // 2])


BROKEN: dict[str, tuple[str, Callable[[Path], None]]] = {
    "functions.csv without a renamed function's row": ("functions.csv", drop_function_row),
    "src/Dates.txt with CountDOWλ's Starts renamed": ("src", rename_parameter),
    "ozzit.xlsx cut to half its length": ("ozzit.xlsx", truncate_workbook),
}


def run(command: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    # UTF-8 mode makes the Python tools write UTF-8 on Windows, as the binary does.
    env = {**os.environ, "PYTHONUTF8": "1"}
    # The commands are argument lists without a shell: this interpreter or the
    # built binary, a gate name and its arguments from the checked-in workflow.
    # nosemgrep: python.lang.security.audit.dangerous-subprocess-use-audit.dangerous-subprocess-use-audit
    return subprocess.run(  # nosec B603
        command, cwd=cwd, capture_output=True, text=True, encoding="utf-8", env=env, check=False
    )


def compare(label: str, native: str, gate: str, arguments: list[str], cwd: Path) -> bool:
    rust = run([native, gate, *arguments], cwd)
    python = run([sys.executable, str(Path("tools", f"{gate}.py").resolve()), *arguments], cwd)
    if (rust.returncode, rust.stdout, rust.stderr) == (python.returncode, python.stdout, python.stderr):
        print(f"same: {label} (exit {rust.returncode})")
        return True
    if ZIP_REASON[0] in rust.stdout + rust.stderr and (
        rust.returncode,
        rust.stdout.replace(*ZIP_REASON),
        rust.stderr.replace(*ZIP_REASON),
    ) == (python.returncode, python.stdout, python.stderr):
        print(f"same apart from the ZIP parser's reason: {label} (exit {rust.returncode})")
        return True
    print(f"DIFFERENT: {label}", file=sys.stderr)
    for name, result in (("ozzit", rust), ("python", python)):
        print(f"--- {name}: exit {result.returncode}\n{result.stdout}{result.stderr}", file=sys.stderr)
    return False


def main() -> int:
    with open(WORKFLOW, encoding="utf-8") as handle:
        gates = [shlex.split(match.group(1)) for line in handle if (match := GATE.match(line))]
    if not gates:
        print(f"no native verify_* gates found in {WORKFLOW}", file=sys.stderr)
        return 1

    # which() adds the .exe suffix Windows needs for a path with a directory.
    native = str(Path(shutil.which(gates[0][0]) or gates[0][0]).resolve())
    results = [compare(gate, native, gate, arguments, Path.cwd()) for _, gate, *arguments in gates]

    for case, (target, breaker) in BROKEN.items():
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in INPUTS:
                copy = shutil.copytree if Path(name).is_dir() else shutil.copyfile
                copy(name, root / name)
            breaker(root)
            for _, gate, *arguments in gates:
                if target in arguments:
                    results.append(compare(f"{gate} on {case}", native, gate, arguments, root))
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main())
