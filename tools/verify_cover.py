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
import posixpath
import re
import sys
import zipfile
from pathlib import Path
import xml.etree.ElementTree as ET

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

WORKBOOK = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
INDEX = Path(sys.argv[2] if len(sys.argv) > 2 else "functions.csv")
CHANGELOG = Path(sys.argv[3] if len(sys.argv) > 3 else "CHANGELOG.md")

LABEL = re.compile(r"<t>Version (?P<date>[^<]+?)\s+-\s+(?P<count>\d+) functions\s+-\s+[^<]*</t>")
CUT = re.compile(r"^## v\d+\.\d+\.\d+, (?P<date>\d{1,2} [A-Z][a-z]+ \d{4}),", re.MULTILINE)


def cover_label(workbook: Path) -> tuple[str, int]:
    main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    package_rel = "http://schemas.openxmlformats.org/package/2006/relationships"
    with zipfile.ZipFile(workbook) as archive:
        workbook_xml = ET.fromstring(archive.read("xl/workbook.xml"))
        cover = next(
            sheet for sheet in workbook_xml.findall(f"{{{main}}}sheets/{{{main}}}sheet")
            if sheet.attrib.get("name") == "Cover"
        )
        workbook_rels = ET.fromstring(archive.read("xl/_rels/workbook.xml.rels"))
        relationship = next(
            item for item in workbook_rels
            if item.attrib.get("Id") == cover.attrib[f"{{{rel}}}id"]
        )
        target = relationship.attrib["Target"].lstrip("/")
        sheet_path = posixpath.normpath(
            target if target.startswith("xl/") else posixpath.join("xl", target)
        )
        sheet_xml = ET.fromstring(archive.read(sheet_path))
        cell = next(
            cell for cell in sheet_xml.findall(f".//{{{main}}}c")
            if cell.attrib.get("r") == "A3"
        )
        value = cell.find(f"{{{main}}}v")
        if value is None or cell.attrib.get("t") != "s":
            raise ValueError("Cover!A3 is not a shared string")
        shared = ET.fromstring(archive.read("xl/sharedStrings.xml"))
        items = shared.findall(f"{{{main}}}si")
        index = int(value.text or "-1")
        if not 0 <= index < len(items):
            raise ValueError("Cover!A3 has an invalid shared-string index")
        text = "".join(item.text or "" for item in items[index].iter(f"{{{main}}}t"))
    labels = LABEL.findall(text)
    if len(labels) != 1:
        raise ValueError(f"expected one cover version label, found {len(labels)}")
    date, count = labels[0]
    return html.unescape(date).strip(), int(count)


def published_functions(index: Path) -> int:
    with index.open(encoding="utf-8-sig", newline="") as stream:
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
