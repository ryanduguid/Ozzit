"""Check that the Cover sheet's version label matches the index and the changelog.

Usage: python tools/verify_cover.py [workbook] [functions.csv] [CHANGELOG.md]

Cover!A3 is the first thing a user reads and the one published field no other
gate reads: it said "20 August 2026" and "130 functions" three cuts after both
had moved. The label must name the changelog's current cut date and the number
of functions the index publishes, counting every oz. name except the five
About help tables the README describes separately.
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

LABEL = re.compile(r"<t>Version (?P<date>[^<]+?)\s+-\s+(?P<count>\d+) functions\s+-\s+[^<]*</t>")
CUT = re.compile(r"^## v\d+\.\d+\.\d+, (?P<date>\d{1,2} [A-Z][a-z]+ \d{4}),", re.MULTILINE)


def cover_label(workbook: Path) -> tuple[str, int]:
    with zipfile.ZipFile(workbook) as archive:
        strings = archive.read("xl/sharedStrings.xml").decode("utf-8")
    labels = LABEL.findall(strings)
    if len(labels) != 1:
        raise ValueError(f"expected one cover version label, found {len(labels)}")
    date, count = labels[0]
    return html.unescape(date).strip(), int(count)


def published_functions(index: Path) -> int:
    with index.open(encoding="utf-8", newline="") as stream:
        names = [row["function"] for row in csv.DictReader(stream)]
    if not names:
        raise ValueError("functions.csv lists no functions")
    return sum(1 for name in names if not name.startswith("oz.About"))


def current_cut_date(changelog: Path) -> str:
    match = CUT.search(changelog.read_text(encoding="utf-8"))
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
