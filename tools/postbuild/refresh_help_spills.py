"""Refresh the help tables the demonstration sheets cache.

Usage: python tools/postbuild/refresh_help_spills.py [workbook]

Cell A4 of every single-function demonstration sheet holds `=oz.Nameλ()`, the
call with no arguments that spills the function's help, and the file caches
what that spilled when Excel last saved. Nothing keeps that cache in step with
the defined name: change a function's help in src/, compile it, and the sheet
still shows the old table until Excel recalculates, which is exactly the
disagreement tools/verify_cache.py exists to catch.

The help is the one result in the workbook that needs no formula engine to
produce. It is TRIM(TEXTSPLIT(text, "→", "¶")) over a string literal, so this
pass reads the literal out of the stored definition, splits it the way
TEXTSPLIT does, trims each cell the way TRIM does (leading and trailing spaces
off, runs of spaces to one), and writes the table over the cells the anchor's
spill range covers, growing or shrinking the range as the help did. A help
whose rows do not all carry exactly one arrow is refused, because Excel would
pad it with #N/A.

Every anchor whose cache already matches is left alone, so a second run reports
"already current" and writes nothing; on the tracked workbook every help the
pass recomputes for an unchanged function reproduces Excel's own cache, which is
what proves the model of TRIM and TEXTSPLIT here. Pure text surgery: no COM, no
recalculation, and no other cell is read or written.
"""

from __future__ import annotations

import html
import re
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compile_sources import arguments, read_string  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402

ANCHOR = re.compile(
    r'<c r="([A-Z]+)(\d+)"([^>]*)><f t="array" ref="([A-Z]+\d+:[A-Z]+\d+)"([^>]*)>'
    r"(oz\.[A-Za-z0-9_]+λ(?:DV)?)\(\)</f><v>[^<]*</v></c>"
)
ROW = re.compile(r'<row r="(\d+)"([^>]*)>(.*?)</row>', re.DOTALL)
CELL = re.compile(r'<c r="([A-Z]+)(\d+)"([^>]*?)(?:/>|>(.*?)</c>)', re.DOTALL)


def column_number(letters: str) -> int:
    n = 0
    for ch in letters:
        n = n * 26 + ord(ch) - 64
    return n


def column_letters(n: int) -> str:
    out = ""
    while n:
        n, remainder = divmod(n - 1, 26)
        out = chr(65 + remainder) + out
    return out


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


def escape(text: str) -> str:
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def rewrite_range(sheet: str, top: int, left: int, old_rows: int, table: list[list[str]], name: str) -> str:
    """Write the table over the spill range, growing or shrinking it as needed."""
    rows, cols = len(table), len(table[0])
    touched = range(top, top + max(rows, old_rows))
    row_elements = {int(m.group(1)): m for m in ROW.finditer(sheet)}
    # A row the spill grows into is created after its predecessor, with that row's
    # attributes; every touched row keeps its other cells and gets its two rewritten.
    pieces: list[tuple[int, str]] = []
    for r in touched:
        match = row_elements.get(r)
        attrs = match.group(2) if match else re.sub(r' spans="[^"]*"', "", row_elements[max(k for k in row_elements if k < r)].group(2))
        cells = list(CELL.finditer(match.group(3))) if match else []
        kept: list[tuple[int, str]] = []
        for cell in cells:
            column = column_number(cell.group(1))
            inner = cell.group(4) or ""
            if left <= column < left + cols:
                if "<f" in inner and not (r == top and column == left):
                    raise ValueError(f"{name}: {cell.group(1)}{r} holds a formula inside the help spill")
                continue
            kept.append((column, cell.group(0)))
        if r < top + rows:
            for c in range(cols):
                column = left + c
                text = escape(table[r - top][c])
                if r == top and c == 0:
                    anchor = next((cell for cell in cells if column_number(cell.group(1)) == column), None)
                    if anchor is None:
                        raise ValueError(f"{name}: the anchor cell is missing")
                    head = re.match(r'<c r="[A-Z]+\d+"[^>]*>', anchor.group(0)).group(0)  # type: ignore[union-attr]
                    formula = re.search(r"<f [^>]*>[^<]*</f>", anchor.group(0)).group(0)  # type: ignore[union-attr]
                    last = f"{column_letters(left + cols - 1)}{top + rows - 1}"
                    formula = re.sub(r'ref="[^"]*"', f'ref="{column_letters(left)}{top}:{last}"', formula, count=1)
                    kept.append((column, f"{head}{formula}<v>{text}</v></c>"))
                else:
                    style = next((cell.group(3) for cell in cells if column_number(cell.group(1)) == column), "")
                    style = re.sub(r' (?:t|cm)="[^"]*"', "", style)
                    value = f"<v>{text}</v>" if text else "<v/>"      # Excel's own spelling of a spilled ""
                    kept.append((column, f'<c r="{column_letters(column)}{r}"{style} t="str">{value}</c>'))
        kept.sort()
        pieces.append((r, f'<row r="{r}"{attrs}>' + "".join(text for _c, text in kept) + "</row>"))

    for r, element in pieces:
        # Re-read the rows each time: the previous row was rewritten a moment ago, and a
        # row the spill grows into is inserted after that rewritten text.
        current = {int(m.group(1)): m for m in ROW.finditer(sheet)}
        match = current.get(r)
        if match:
            sheet = sheet.replace(match.group(0), element, 1)
        else:
            previous = current[max(k for k in current if k < r)].group(0)
            sheet = sheet.replace(previous, previous + element, 1)
    return sheet


def refresh(parts: dict[str, bytes]) -> list[str]:
    names = defined_names(parts["xl/workbook.xml"].decode("utf-8"))
    log: list[str] = []
    for part in sorted(n for n in parts if re.fullmatch(r"xl/worksheets/sheet\d+\.xml", n)):
        sheet = parts[part].decode("utf-8")
        for match in ANCHOR.finditer(sheet):
            left, top = column_number(match.group(1)), int(match.group(2))
            first, last = match.group(4).split(":")
            old_rows = int(re.sub(r"[A-Z]", "", last)) - top + 1
            old_cols = column_number(re.sub(r"\d", "", last)) - left + 1
            name = match.group(6)
            if name not in names:
                raise ValueError(f"{part}: {name} is not a defined name")
            table = help_table(names[name], name)
            if old_cols == len(table[0]) and cached_table(sheet, top, left, old_rows, old_cols) == table:
                continue
            sheet = rewrite_range(sheet, top, left, old_rows, table, name)
            log.append(f"{part}: refreshed the {name} help, {len(table)} rows")
        if log and log[-1].startswith(part):
            parts[part] = sheet.encode("utf-8")
    return log


def run(workbook: Path) -> list[str]:
    with zipfile.ZipFile(workbook) as archive:
        parts = {n: archive.read(n) for n in archive.namelist()}
    log = refresh(parts)
    if log:
        write_deterministic(workbook, parts)
    return log


def main() -> None:
    if len(sys.argv) > 2:
        sys.exit("FAIL: usage: python tools/postbuild/refresh_help_spills.py [workbook]")
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        log = run(workbook)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    for line in log:
        print(line)
    if not log:
        print("help spills already current; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
