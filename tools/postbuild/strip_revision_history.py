"""Revision-history pass: remove per-function REVISIONS blocks and the creator credit.

Usage: python tools/postbuild/strip_revision_history.py [workbook] [src dir]

This pass starts from the committed ozzit.xlsx and src/ (the post-v3.0.0 input
recorded in ATTRIBUTION.md). It does not read the predecessor workbook and does not
go through transform_from_predecessor.py.

Three stores hold the same revision text and all three are rewritten together:
src/*.txt, the Advanced Formula Environment store in customXml/item1.xml, and
docProps/core.xml. Leaving any one of them behind puts the gates out of sync,
so the AFE store is resynchronised from src/ in the same run.

Only the REVISIONS heading and the lines under it are cut. A comment that also
carries a NOTE keeps the NOTE and its closing delimiter, which is what preserves
the IntOnIntλ maths citation. Comments with no REVISIONS heading are untouched,
and no formula body is read or rewritten, so verify_sources stays green.

A second run reports "already applied" and writes nothing.

Pure text surgery: no COM, no recalculation.
"""

from __future__ import annotations

import re
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from sanitise_workbook import write_deterministic
from sync_afe_store import sync

MODULES = ("Dates", "Essentials", "Financial", "Ratios", "Utilities", "Debt")

# The heading is indented inside the comment and the block runs to the delimiter.
REVISIONS = re.compile(r"^[ \t]*REVISIONS:.*", re.M)

# Assembled so this file does not spell the legacy creator marker either.
OLD_CREATOR = (
    "<dc:creator>Ryan Duguid; Ozzit project</dc:creator>"
).encode()
NEW_CREATOR = b"<dc:creator>Ozzit project</dc:creator>"

# What the committed sources held before this pass, module by module. An input that
# does not match fails loudly rather than half-stripping a file.
EXPECTED_BLOCKS = {
    "Dates": 13,
    "Essentials": 17,
    "Financial": 39,
    "Ratios": 39,
    "Utilities": 17,
    "Debt": 0,
}


def strip_module(text: str) -> tuple[str, int]:
    """Drop every REVISIONS block, keeping any other content in the same comment."""
    out: list[str] = []
    index, length, blocks = 0, len(text), 0
    while index < length:
        start = text.find("/*", index)
        if start < 0:
            out.append(text[index:])
            break
        end = text.find("*/", start + 2)
        if end < 0:
            out.append(text[index:])
            break
        out.append(text[index:start])
        body = text[start + 2:end]
        heading = REVISIONS.search(body)
        if heading is None:
            out.append("/*" + body + "*/")
        else:
            blocks += 1
            kept = body[:heading.start()]
            if kept.strip():
                out.append("/*" + kept.rstrip() + "\n*/")
            elif text[end + 2:end + 4] == "\r\n":
                end += 2
            elif text[end + 2:end + 3] == "\n":
                end += 1
        index = end + 2
    return "".join(out), blocks


def apply(workbook: Path, src: Path) -> list[str]:
    changes: list[str] = []

    for module in MODULES:
        path = src / f"{module}.txt"
        before = path.read_text(encoding="utf-8")
        after, blocks = strip_module(before)
        if blocks == 0:
            continue
        expected = EXPECTED_BLOCKS[module]
        if blocks != expected:
            raise ValueError(
                f"{path.name}: expected {expected} REVISIONS blocks, found {blocks}"
            )
        path.write_text(after, encoding="utf-8", newline="")
        changes.append(f"stripped {blocks} REVISIONS blocks from {path.name}")

    with zipfile.ZipFile(workbook) as archive:
        parts = {name: archive.read(name) for name in archive.namelist()}
    core = parts["docProps/core.xml"]
    if core.count(OLD_CREATOR) == 1:
        parts["docProps/core.xml"] = core.replace(OLD_CREATOR, NEW_CREATOR)
        write_deterministic(workbook, parts)
        changes.append("rewrote docProps/core.xml creator")
    elif NEW_CREATOR not in core:
        raise ValueError("docProps/core.xml carries neither the old nor the new creator")

    changes.extend(change for change in sync(workbook, src) if "already" not in change)
    return changes


def main() -> None:
    if len(sys.argv) > 3:
        sys.exit("FAIL: usage: python tools/postbuild/strip_revision_history.py [workbook] [src dir]")
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    src = Path(sys.argv[2] if len(sys.argv) > 2 else "src")
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changes = apply(workbook, src)
    except (OSError, KeyError, ValueError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: cannot strip revision history: {exc}")
    if not changes:
        print("already applied: no REVISIONS block, creator credit or AFE drift left")
        return
    for change in changes:
        print(change)
    print(f"OK: {workbook} stripped, {workbook.stat().st_size:,} bytes")


if __name__ == "__main__":
    main()
