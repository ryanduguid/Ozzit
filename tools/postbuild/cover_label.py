"""Cover-label pass: the Cover sheet states the current cut and function count.

Usage: python tools/postbuild/cover_label.py [workbook]

Cover!A3 is a typed shared string, "Version <date>  -  <n> functions  -
<compatibility>". It read "20 August 2026" and "130 functions" after three
cuts had added functions and moved the date, because no gate read the cell.
This pass records the swap to the v3.4.2 wording, then to the count after the
four working-capital days functions of September 2026; `tools/verify_cover.py`
then keeps the cell equal to functions.csv and the changelog. Pure text
surgery on xl/sharedStrings.xml: no COM, no recalculation, no cached value
depends on the cell.
"""

from __future__ import annotations

import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from sanitise_workbook import write_deterministic
from workbook import read_parts

STRINGS = "xl/sharedStrings.xml"
# Earlier labels, oldest first; any one of them is swapped for NEW.
OLD = (
    "Version 20 August 2026    -    130 functions    -    Microsoft 365 or Excel 2024 and later",
    "Version 16 September 2026    -    133 functions    -    Microsoft 365 or Excel 2024 and later",
)
NEW = "Version 16 September 2026    -    137 functions    -    Microsoft 365 or Excel 2024 and later"


def run(workbook: Path) -> list[str]:
    parts = read_parts(workbook)
    if STRINGS not in parts:
        raise ValueError(f"missing part {STRINGS}")
    text = parts[STRINGS].decode("utf-8")
    counts = {label: text.count(f"<t>{label}</t>") for label in (*OLD, NEW)}
    if sum(counts.values()) != 1:
        found = ", ".join(f"{n} of {label!r}" for label, n in counts.items() if n) or "none"
        raise ValueError(f"{STRINGS}: expected exactly one cover label, got {found}")
    if counts[NEW]:
        return []
    old = next(label for label in OLD if counts[label])
    parts[STRINGS] = text.replace(f"<t>{old}</t>", f"<t>{NEW}</t>").encode("utf-8")
    write_deterministic(workbook, parts)
    return ["workbook"]


def main() -> None:
    if len(sys.argv) > 2:
        sys.exit("FAIL: usage: python tools/postbuild/cover_label.py [workbook]")
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changed = run(workbook)
    except (OSError, ValueError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    if changed:
        print("applied cover label to: workbook")
    else:
        print("cover label already applied; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
