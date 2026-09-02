"""Report the demonstration sheets whose cached help no longer matches its function.

Usage: python tools/verify_help_spills.py [workbook]

Cell A4 of every single-function demonstration sheet holds `=oz.Nameλ()`, the
call with no arguments that spills the function's help, and the file caches what
that spilled when Excel last saved. A help change in src/ compiled into the
workbook leaves that cache showing the old table until Excel recalculates, and
nothing in the CI sequence looks at it: verify_sources.py compares definitions,
and tools/verify_cache.py needs Excel.

The help is the one result that can be predicted without a formula engine. It
is TRIM(TEXTSPLIT(text, "→", "¶")) over a string literal, so this tool reads the
literal out of each stored definition, splits it the way TEXTSPLIT does, trims
each cell the way TRIM does (leading and trailing spaces off, runs of spaces to
one), and compares the table with the cells the anchor's spill range covers. On
the tracked workbook the model reproduces every cache Excel wrote for a help
that has not changed, which is what proves it. A help whose rows do not all
carry exactly one arrow is refused, because Excel would pad it with #N/A.

This tool only reads. AGENTS.md reserves cached values for Excel-backed
evidence, so a stale help is reported, never rewritten: run
tools/refresh_cache.py in Excel, then tools/sanitise_workbook.py, and
tools/verify_cache.py proves the result. Run this first to know whether that
round is needed at all.
"""

from __future__ import annotations

import html
import re
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compile_sources import arguments, read_string  # noqa: E402

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

ANCHOR = re.compile(
    r'<c r="([A-Z]+)(\d+)"([^>]*)><f t="array" ref="([A-Z]+\d+:[A-Z]+\d+)"([^>]*)>'
    r"(oz\.[A-Za-z0-9_]+λ(?:DV)?)\(\)</f><v>[^<]*</v></c>"
)
CELL = re.compile(r'<c r="([A-Z]+)(\d+)"([^>]*?)(?:/>|>(.*?)</c>)', re.DOTALL)
SHEET = re.compile(r"xl/worksheets/sheet\d+\.xml")


def column_number(letters: str) -> int:
    n = 0
    for ch in letters:
        n = n * 26 + ord(ch) - 64
    return n


def literal_text(code: str) -> str:
    """Every string literal in a code fragment, concatenated, "" unescaped."""
    out, i, n = [], 0, len(code)
    while i < n:
        if code[i] == '"':
            lit, i = read_string(code, i)
            out.append(lit[1:-1].replace('""', '"'))
            continue
        i += 1
    return "".join(out)


def trim(text: str) -> str:
    return re.sub(" {2,}", " ", text).strip(" ")


def help_table(stored: str, name: str) -> list[list[str]]:
    """The table TRIM(TEXTSPLIT(literal, "→", "¶")) spills for one definition."""
    at = stored.find("TEXTSPLIT(")
    if at < 0:
        raise ValueError(f"{name}: the definition has no TEXTSPLIT help")
    args = arguments(stored, at + len("TEXTSPLIT("))
    if len(args) < 3 or literal_text(args[1]) != "→" or literal_text(args[2]) != "¶":
        raise ValueError(f"{name}: the help is not split on → and ¶")
    rows = []
    for raw in literal_text(args[0]).split("¶"):
        cells = raw.split("→")
        if len(cells) != 2:
            raise ValueError(
                f"{name}: a help row has {len(cells) - 1} arrows, so Excel would pad it "
                f"with #N/A: {raw[:60]!r}"
            )
        rows.append([trim(cell) for cell in cells])
    return rows


def defined_names(book: str) -> dict[str, str]:
    return {
        name: html.unescape(body)
        for name, body in re.findall(r'<definedName name="(oz\.[^"]+)"[^>]*>(.*?)</definedName>', book, re.DOTALL)
    }


def cached_table(sheet: str, top: int, left: int, rows: int, cols: int) -> list[list[str]]:
    """What the sheet caches over a range, "" for a cell that is absent or empty."""
    cells: dict[tuple[int, int], str] = {}
    for match in CELL.finditer(sheet):
        column, row = column_number(match.group(1)), int(match.group(2))
        if top <= row < top + rows and left <= column < left + cols:
            inner = match.group(4) or ""
            value = re.search(r"<v>(.*?)</v>", inner, re.DOTALL)
            cells[(row, column)] = html.unescape(value.group(1)) if value else ""
    return [[cells.get((top + r, left + c), "") for c in range(cols)] for r in range(rows)]


def check(parts: dict[str, bytes]) -> tuple[list[str], int]:
    """(one line per stale help, number of anchors examined)."""
    names = defined_names(parts["xl/workbook.xml"].decode("utf-8"))
    stale: list[str] = []
    anchors = 0
    for part in sorted(n for n in parts if SHEET.fullmatch(n)):
        sheet = parts[part].decode("utf-8")
        for match in ANCHOR.finditer(sheet):
            anchors += 1
            left, top = column_number(match.group(1)), int(match.group(2))
            last = match.group(4).split(":")[1]
            rows = int(re.sub(r"[A-Z]", "", last)) - top + 1
            cols = column_number(re.sub(r"\d", "", last)) - left + 1
            name = match.group(6)
            if name not in names:
                raise ValueError(f"{part}: {name} is not a defined name")
            table = help_table(names[name], name)
            if cols == len(table[0]) and cached_table(sheet, top, left, rows, cols) == table:
                continue
            stale.append(
                f"{part}: the {name} help is stale, {rows} cached rows against "
                f"{len(table)} in the definition"
            )
    return stale, anchors


def run(workbook: Path) -> tuple[list[str], int]:
    with zipfile.ZipFile(workbook) as archive:
        parts = {n: archive.read(n) for n in archive.namelist()}
    return check(parts)


def main() -> None:
    if len(sys.argv) > 2:
        sys.exit("FAIL: usage: python tools/verify_help_spills.py [workbook]")
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        stale, anchors = run(workbook)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    for line in stale:
        print(line)
    if stale:
        sys.exit(
            f"FAIL: {len(stale)} of {anchors} cached helps in {workbook} no longer match "
            "their definitions; run tools/refresh_cache.py in Excel, then "
            "tools/sanitise_workbook.py, and tools/verify_cache.py proves the result"
        )
    print(f"OK: every cached help in {workbook} matches its definition, {anchors} anchors")


if __name__ == "__main__":
    main()
