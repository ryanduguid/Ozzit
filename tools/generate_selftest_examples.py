"""Turn the help's worked examples and help tables into native self-test assertions.

Usage: python tools/generate_selftest_examples.py [src dir] [output .ps1]

Every function's inline help ends in worked examples a reader is meant to copy,
and every gate that reads them checks their shape rather than their arithmetic.
The native self-test in tools/excel_selftest.ps1 evaluates hand-written
assertions in a real Excel, but named only 20 of the 134 functions, and two
functions shipped for several releases returning the opposite of their own
printed example. This tool closes that gap: it reads every EXAMPLES block in
src/, keeps each example that stands on its own (no table references, no
undefined names), and writes a PowerShell fragment the self-test dot-sources.

What is asserted depends on what the help prints as the result:

    a number, a percentage      the formula evaluates to it, within half a unit
                                of the last printed digit
    TRUE or FALSE               N(formula) is 1 or 0
    a list or grid of numbers   the formula spills that many cells and they sum
                                to the printed total, so no printed format is
                                assumed
    a single word or label      the formula returns exactly that text
    "N rows, M cols"            the shape of the spill
    anything else               the formula evaluates without error

Named inputs (the IRRλ example binds Dates and Values first) are wrapped in
LET(). Every function also gets one help assertion: called with no arguments,
a LAMBDA spills a table whose first cell reads FUNCTION:, an About table's
reads About:, and a data-validation companion returns TRUE.

The output stays pure ASCII, the way the self-test does, so λ is written as
$L. tools/tests checks the committed fragment against a fresh generation.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compile_sources import CONSTANTS, PLAIN, XLFN  # noqa: E402
from verify_sources import NAME, statements  # noqa: E402

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

MODULES = ("Dates", "Essentials", "Financial", "Ratios", "Utilities", "Debt")
NAMESPACE = "oz"
NUMBER = re.compile(r"^(-?\d*(?:\.\d+)?%?)(?:\s*[A-Za-z]+)?$")   # .32x, 40.8949 days
ISO_DATE = re.compile(r"^(\d{4})-(\d{2})-(\d{2})$")
LIST_ITEM = r"-?\d+(?:\.\d+)?"
LIST = re.compile(rf"^\s*{LIST_ITEM}(?:\s*,\s*{LIST_ITEM})+\s*$")
SHAPE = re.compile(r"^(\d+) rows?, (\d+) cols?$")
WORD = re.compile(r"^[A-Za-z0-9:_\-]+$")
IDENT = re.compile(r"(?<![A-Za-z0-9_.λ\"])([A-Za-z_][A-Za-z0-9_.λ]*\??)")
CELL_REF = re.compile(r"^\$?[A-Z]{1,3}\$?\d+$")


def read_literals(body: str) -> str:
    """Every double-quoted literal in order, "" unescaped (verify_signatures' rule)."""
    out, i, n = [], 0, len(body)
    while i < n:
        if body[i] != '"':
            i += 1
            continue
        i += 1
        while i < n:
            if body[i] == '"':
                if i + 1 < n and body[i + 1] == '"':
                    out.append('"')
                    i += 2
                    continue
                i += 1
                break
            out.append(body[i])
            i += 1
    return "".join(out)


def help_rows(body: str) -> list[tuple[str, str]]:
    rows = []
    for raw in read_literals(body).split("¶"):
        label, sep, text = raw.partition("→")
        if sep:
            rows.append((label.strip(), text.rstrip("→").strip()))
    return rows


@dataclass
class Example:
    formula: str = ""
    results: list[str] = field(default_factory=list)


def examples_of(body: str) -> tuple[list[Example], dict[str, str]]:
    """The worked examples of one help, and the named inputs they rely on."""
    rows = help_rows(body)
    start = next((i for i, (label, _t) in enumerate(rows) if label.startswith("EXAMPLE")), None)
    if start is None:
        return [], {}
    out: list[Example] = []
    bindings: dict[str, str] = {}
    current: Example | None = None
    collecting = False           # rows after a Result heading belong to `current`
    for label, text in rows[start:]:
        if text.startswith("="):
            collecting = False
            if label and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", label) and label.upper() not in CONSTANTS \
                    and not label.startswith("Result") and not label.startswith("Formula"):
                bindings[label] = text[1:]
                current = None
                continue
            current = Example()
            current.formula = text[1:]
            if label and not label.startswith("Formula"):
                current.results.append(label)
            out.append(current)
            continue
        if current is not None and text and not label and current.formula.count("(") > current.formula.count(")"):
            current.formula += " " + text          # a formula wrapped onto a second row
            continue
        if text.startswith("Result") or text.startswith("Formula"):
            collecting = text.startswith("Result") and current is not None
            continue
        if label.startswith("Result") and current is not None:
            if text:
                current.results.append(text)
            collecting = True
            continue
        if current is not None and collecting and text:
            current.results.append(text)
            continue
        if current is not None and label and not text and current.results:
            current.results.append(label)      # a grid printed one row per label
            continue
        if not label and not text:
            collecting = False
    return out, bindings


def library_names(src: Path) -> set[str]:
    names = set()
    for module in MODULES:
        for statement in statements((src / f"{module}.txt").read_text(encoding="utf-8")):
            match = NAME.match(statement)
            if match:
                names.add(match.group(1))
    return names


def qualify(formula: str, library: set[str]) -> str:
    for bare in sorted(library, key=len, reverse=True):
        formula = re.sub(r"(?<![A-Za-z0-9_.λ])" + re.escape(bare) + r"(?=\s*\()", f"{NAMESPACE}.{bare}", formula)
    return formula


def stands_alone(formula: str, library: set[str], bound: set[str]) -> bool:
    """True when every name in the formula is a function, a constant or a bound input."""
    if "[" in formula:
        return False
    code = re.sub(r'"(?:[^"]|"")*"', '""', formula)
    for match in IDENT.finditer(code):
        word = match.group(1)
        after = code[match.end():].lstrip()
        if word.startswith(f"{NAMESPACE}.") and word[len(NAMESPACE) + 1:] in library:
            continue
        if after.startswith("("):
            if word.upper() in XLFN or word.upper() in PLAIN or word in library:
                continue
            return False
        if word.upper() in CONSTANTS or word in bound:
            continue
        if CELL_REF.match(word):
            return False
        return False
    return True


def powershell(text: str) -> str:
    """A formula inside a double-quoted PowerShell string, λ written as $L."""
    text = text.replace("`", "``").replace('"', '`"').replace("$", "`$")
    return text.replace("λ", "$L")


def number(text: str) -> tuple[float, float] | None:
    """(value, tolerance) for a printed number, or None."""
    match = NUMBER.match(text.strip())
    if not match or match.group(1) in ("", "-", "%"):
        return None
    text = match.group(1)
    percent = text.endswith("%")
    digits = text.rstrip("%")
    decimals = len(digits.split(".")[1]) if "." in digits else 0
    value = float(digits)
    if percent:
        value /= 100
        decimals += 2
    return value, 0.5 * 10 ** -decimals


def checks_for(name: str, example: Example, index: int, library: set[str], bindings: dict[str, str]) -> list[str]:
    bound = {k for k in bindings}
    if not stands_alone(example.formula, library, bound):
        return []
    for value in bindings.values():
        if not stands_alone(value, library, set()):
            return []
    formula = qualify(example.formula, library)
    if bindings:
        parts = ", ".join(f"{k}, {qualify(v, library)}" for k, v in bindings.items())
        formula = f"LET({parts}, {formula})"
    ident = f"example: {name} #{index}"
    tag = f"'{powershell(ident)}'"
    lines = []
    printed = " ".join(example.results).strip()
    rows = [r for r in example.results if r.strip()]
    single = number(printed) if len(rows) == 1 else None
    shape = SHAPE.match(printed)
    date = ISO_DATE.match(printed)
    if date:
        serial = f"DATE({int(date.group(1))},{int(date.group(2))},{int(date.group(3))})"
        lines.append(f"Near {tag} \"{powershell(formula)} - {serial}\" '0' '0.5'")
    elif single is not None:
        value, tolerance = single
        lines.append(f"Near {tag} \"{powershell(formula)}\" '{value!r}' '{tolerance!r}'")
    elif printed.upper() in CONSTANTS:
        lines.append(f"Near {tag} \"N({powershell(formula)})\" '{1 if printed.upper() == 'TRUE' else 0}'")
    elif shape:
        lines.append(f"Near {tag} \"ROWS({powershell(formula)})\" '{shape.group(1)}'")
        lines.append(f"Near {tag} \"COLUMNS({powershell(formula)})\" '{shape.group(2)}'")
    elif rows and all(LIST.match(r) or number(r) for r in rows):
        parsed = [number(x) for r in rows for x in r.split(",")]
        items = [p for p in parsed if p is not None]
        if len(items) == len(parsed):
            total = sum(v for v, _t in items)
            tolerance = sum(t for _v, t in items)
            lines.append(f"Near {tag} \"COUNT({powershell(formula)})\" '{len(items)}'")
            lines.append(f"Near {tag} \"SUM({powershell(formula)})\" '{total!r}' '{tolerance!r}'")
    elif len(rows) == 1 and WORD.match(printed):
        lines.append(f"Same {tag} \"{powershell(formula)}\" '{printed}'")
    if not lines:
        lines.append(f"Near {tag} \"SUMPRODUCT(--ISERROR({powershell(formula)}))\" '0'")
    return lines


def help_check(name: str, body: str) -> str:
    call = powershell(f"{NAMESPACE}.{name}")
    tag = f"'{powershell('help: ' + name)}'"
    if not body.startswith("LAMBDA("):
        return f"Same {tag} \"INDEX({call},1,1)\" 'About:'"
    if name.endswith("DV"):
        return f"Near {tag} \"N({call}())\" '1'"
    return f"Same {tag} \"INDEX({call}(),1,1)\" 'FUNCTION:'"


def generate(src: Path) -> str:
    library = library_names(src)
    lines = [
        "# Generated by tools/generate_selftest_examples.py from the help in src/. Do not edit;",
        "# rerun the generator after a help change. Dot-sourced by tools/excel_selftest.ps1.",
        "",
        "# --- every function answers a call with no arguments with its help",
    ]
    example_lines: list[str] = []
    counted = 0
    for module in MODULES:
        for statement in statements((src / f"{module}.txt").read_text(encoding="utf-8")):
            match = NAME.match(statement)
            if not match:
                continue
            name, body = match.group(1), match.group(2).strip()
            lines.append(help_check(name, body))
            examples, bindings = examples_of(body)
            for index, example in enumerate(examples, start=1):
                found = checks_for(name, example, index, library, bindings)
                if found:
                    counted += 1
                example_lines.extend(found)
    lines.append("")
    lines.append(f"# --- {counted} worked examples that stand on their own, as the help prints them")
    lines.extend(example_lines)
    return "\n".join(lines) + "\n"


def main() -> None:
    if len(sys.argv) > 3:
        sys.exit("FAIL: usage: python tools/generate_selftest_examples.py [src dir] [output]")
    src = Path(sys.argv[1] if len(sys.argv) > 1 else "src")
    output = Path(sys.argv[2] if len(sys.argv) > 2 else Path(__file__).resolve().parent / "selftest_examples.ps1")
    text = generate(src)
    if not text.isascii():
        sys.exit("FAIL: the generated fragment is not pure ASCII")
    before = output.read_text(encoding="ascii") if output.is_file() else None
    if before == text:
        print(f"already current: {output}")
        return
    output.write_text(text, encoding="ascii", newline="\n")
    print(f"wrote {output}: {text.count(chr(10))} lines")


if __name__ == "__main__":
    main()
