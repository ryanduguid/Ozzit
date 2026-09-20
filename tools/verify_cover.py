"""Check that the Cover sheet's version label matches the index and the changelog.

Usage: python tools/verify_cover.py [workbook] [functions.csv] [CHANGELOG.md]

Cover!A3 is the first thing a user reads and the one published field no other
gate reads: it said "20 August 2026" and "130 functions" three cuts after both
had moved. The label must name the changelog's current cut date and the number
of functions the index publishes, counting every oz. name except the five
About help tables the README describes separately. The gate resolves the cell
itself, through the Cover sheet's part and its shared-string index, so a stale
cell cannot pass on the strength of a current-looking string stored elsewhere.
"""

import csv
import html
import re
import sys
import zipfile
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

WORKBOOK = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
INDEX = Path(sys.argv[2] if len(sys.argv) > 2 else "functions.csv")
CHANGELOG = Path(sys.argv[3] if len(sys.argv) > 3 else "CHANGELOG.md")

LABEL = re.compile(r"^Version (?P<date>.+?)\s+-\s+(?P<count>\d+) functions\s+-\s+.+$")
CUT = re.compile(r"^## v\d+\.\d+\.\d+, (?P<date>\d{1,2} [A-Z][a-z]+ \d{4}),", re.MULTILINE)
COVER_SHEET = re.compile(r'<sheet [^>]*name="Cover"[^>]*r:id="(?P<rid>rId\d+)"')
CELL = re.compile(r'<c r="A3"(?P<attrs>[^>]*)>(?P<body>.*?)</c>', re.DOTALL)


def cover_label(workbook: Path) -> tuple[str, int]:
    with zipfile.ZipFile(workbook) as archive:
        book = archive.read("xl/workbook.xml").decode("utf-8")
        sheet = COVER_SHEET.search(book)
        if sheet is None:
            raise ValueError("xl/workbook.xml declares no sheet named Cover")
        rels = archive.read("xl/_rels/workbook.xml.rels").decode("utf-8")
        target = re.search(
            r'<Relationship [^>]*Id="%s"[^>]*Target="(?P<target>[^"]+)"' % sheet.group("rid"), rels
        )
        if target is None:
            raise ValueError("the Cover sheet has no relationship target")
        part = "xl/" + target.group("target").lstrip("/").removeprefix("xl/")
        cell = CELL.search(archive.read(part).decode("utf-8"))
        if cell is None or 't="s"' not in cell.group("attrs"):
            raise ValueError(f"{part}: Cover!A3 is not a shared-string cell")
        index = re.search(r"<v>(\d+)</v>", cell.group("body"))
        if index is None:
            raise ValueError(f"{part}: Cover!A3 carries no shared-string index")
        items = re.findall(r"<si>(.*?)</si>", archive.read("xl/sharedStrings.xml").decode("utf-8"), re.DOTALL)
    position = int(index.group(1))
    if position >= len(items):
        raise ValueError(f"Cover!A3 points at shared string {position}, beyond the {len(items)} stored")
    text = html.unescape("".join(re.findall(r"<t[^>]*>(.*?)</t>", items[position], re.DOTALL))).strip()
    label = LABEL.match(text)
    if label is None:
        raise ValueError(f"Cover!A3 is not a version label: {text!r}")
    return label.group("date").strip(), int(label.group("count"))


def published_functions(index: Path) -> int:
    # utf-8-sig: the compiler and the index gates accept a byte-order mark, so
    # this one must not let the mark become part of the first column name.
    with index.open(encoding="utf-8-sig", newline="") as stream:
        names = [row["function"] for row in csv.DictReader(stream)]
    if not names:
        raise ValueError("functions.csv lists no functions")
    return sum(1 for name in names if not name.startswith("oz.About"))


def current_cut_date(changelog: Path) -> str:
    match = CUT.search(changelog.read_text(encoding="utf-8-sig"))
    if match is None:
        raise ValueError("CHANGELOG.md has no '## vX.Y.Z, <date>,' heading")
    return match.group("date")


def main() -> int:
    failures = []
    try:
        date, count = cover_label(WORKBOOK)
        functions = published_functions(INDEX)
        cut = current_cut_date(CHANGELOG)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as exc:
        print(f"FAIL: {exc}")
        return 1
    if count != functions:
        failures.append(f"cover says {count} functions; functions.csv publishes {functions}")
    if date != cut:
        failures.append(f"cover is dated {date}; the changelog's current cut is {cut}")
    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        return 1
    print(f"OK: cover label states {cut} and {functions} functions")
    return 0


if __name__ == "__main__":
    sys.exit(main())
