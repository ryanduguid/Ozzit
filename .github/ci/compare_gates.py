"""Run each native gate in the verify workflow with its Python reference tool too.

usage: python .github/ci/compare_gates.py

Run from the repository root after the release build. It reads every
"run: tools/ozzit/target/release/ozzit verify_*" step in
.github/workflows/verify.yml, runs that command and "python tools/<gate>.py"
with the same arguments, and fails when no gate is found or when a pair's exit
codes or stdout differ. Line endings are normalised. Usage text names the
native command, but it appears only on an argument error, which these runs
never make, so no difference is allowed for it.
"""

import os
import re
import shlex
import shutil
import subprocess
import sys

WORKFLOW = ".github/workflows/verify.yml"
GATE = re.compile(r"^\s*run:\s*(tools/ozzit/target/release/ozzit\s+verify_\w+.*?)\s*$")


def run(command: list[str]) -> subprocess.CompletedProcess[str]:
    # UTF-8 mode makes the Python tools write UTF-8 on Windows, as the binary does.
    env = {**os.environ, "PYTHONUTF8": "1"}
    # The commands are argument lists without a shell: this interpreter or the
    # built binary, a gate name and its arguments from the checked-in workflow.
    # nosemgrep: python.lang.security.audit.dangerous-subprocess-use-audit.dangerous-subprocess-use-audit
    return subprocess.run(  # nosec B603
        command, capture_output=True, text=True, encoding="utf-8", env=env, check=False
    )


def main() -> int:
    with open(WORKFLOW, encoding="utf-8") as handle:
        gates = [shlex.split(match.group(1)) for line in handle if (match := GATE.match(line))]
    if not gates:
        print(f"no native verify_* gates found in {WORKFLOW}", file=sys.stderr)
        return 1

    failed = 0
    for native, gate, *arguments in gates:
        # which() adds the .exe suffix Windows needs for a path with a directory.
        rust = run([shutil.which(native) or native, gate, *arguments])
        python = run([sys.executable, f"tools/{gate}.py", *arguments])
        if (rust.returncode, rust.stdout) == (python.returncode, python.stdout):
            print(f"same: {gate} (exit {rust.returncode})")
            continue
        failed += 1
        print(f"DIFFERENT: {gate}", file=sys.stderr)
        for label, result in (("ozzit", rust), ("python", python)):
            print(f"--- {label}: exit {result.returncode}\n{result.stdout}{result.stderr}", file=sys.stderr)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
