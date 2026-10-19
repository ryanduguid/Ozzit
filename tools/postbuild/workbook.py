"""Workbook reading shared by the postbuild passes.

Three things were copied from pass to pass rather than shared: opening the
archive and decoding xl/workbook.xml, reading whether a pass is already applied
from the defined names it adds, and replacing a table of text swaps. Ten passes
held 9 copies of the first, 2 near-identical copies of the second, and 4
copies of the third under 4 different signatures. One copy each lives here.

The function-adding passes later repeated four more pieces: the applied-or-absent
check on a src module and on functions.csv, the About-row insertion and the command
line. Those live here too.

Reading only. Each pass still owns its own writing, because the order it writes
its stores in and what it restores on failure are decisions the pass makes, not
decisions this module can make for it. Text surgery as before: no COM, no
recalculation.
"""

from __future__ import annotations

import csv
import re
import sys
import zipfile
from collections.abc import Callable, Iterable, Sequence
from pathlib import Path

BOOK = "xl/workbook.xml"

# The shipped names all carry the oz. prefix; Print_Area and friends do not.
OZ_NAME = re.compile(r'<definedName name="(oz\.[^"]+)"')


def read_parts(workbook: Path) -> dict[str, bytes]:
    """Every part of the workbook by name, the shape write_deterministic takes back."""
    with zipfile.ZipFile(workbook) as archive:
        return {name: archive.read(name) for name in archive.namelist()}


def read_book(parts: dict[str, bytes]) -> str:
    return parts[BOOK].decode("utf-8")


def oz_names(book: str) -> list[str]:
    """The oz. defined names the workbook declares, in the order it declares them."""
    return OZ_NAME.findall(book)


def workbook_state(
    book: str,
    names: Sequence[str],
    heading: str | None = None,
    noun: str = "defined names",
) -> str:
    """"absent" or "applied" for a pass that adds `names`, and an About `heading`.

    A workbook holding some of what the pass adds but not all of it is neither
    state, and raises rather than letting the pass write over a partial result.
    """
    present = sum(1 for name in names if f'<definedName name="{name}"' in book)
    about = None if heading is None else book.count(heading)
    if present == 0 and about in (None, 0):
        return "absent"
    if present == len(names) and about in (None, 1):
        return "applied"
    detail = f"{present} of {len(names)} {noun}"
    if about is not None:
        detail += f" and {about} About heading(s)"
    raise ValueError(f"{BOOK} holds {detail}; it is neither state this pass recognises")


def source_state(where: str, text: str, functions: Sequence[tuple[str, str]]) -> str:
    """"absent" or "applied" for a src module that a pass adds functions and About rows to.

    `functions` pairs each function name with the exact About row the pass inserts. A
    module holding part of the change is neither state, and raises.
    """
    present = sum(1 for name, _row in functions if f"\n{name} = LAMBDA(" in text)
    rows = sum(1 for _name, row in functions if row in text)
    if present == 0 and rows == 0:
        return "absent"
    if present == len(functions) and rows == len(functions):
        return "applied"
    raise ValueError(
        f"{where} holds {present} of {len(functions)} functions and {rows} "
        f"About rows; it is neither state this pass recognises"
    )


def index_state(index: Path | None, names: Sequence[str], rows: str) -> str | None:
    """"absent", "applied", or None when no index is supplied, raising on a partial one.

    A supplied index that does not exist is an error, not "no index": treated as absent
    from the check, it let a pass write src/ and the workbook and exit 0 with the index
    left behind. `rows` names the pass's rows in the message.
    """
    if index is None:
        return None
    if not index.is_file():
        raise ValueError(f"{index}: no such index; supply an existing functions.csv or none")
    # Build tool; the path is the repository's own file or the maintainer's argument.
    with index.open(encoding="utf-8-sig", newline="") as handle:  # NOSONAR
        listed = {row.get("function", "") for row in csv.DictReader(handle)}
    present = sum(1 for name in names if name in listed)
    if present == 0:
        return "absent"
    if present == len(names):
        return "applied"
    raise ValueError(
        f"{index.name} holds {present} of {len(names)} {rows} rows; "
        "it is neither state this pass recognises"
    )


def insert_about_rows(
    text: str, where: str, anchor: str, rows: Iterable[str], heading: str | None = None
) -> str:
    """The module with `rows` inserted after the About row `anchor`, at its indent.

    A heading gets the blank spacer row the older groups carry above them.
    """
    hits = text.count(anchor)
    if hits != 1:
        raise ValueError(f"{where}: About anchor found {hits} times, expected 1")
    line_start = text.rfind("\n", 0, text.index(anchor)) + 1
    indent = text[line_start : text.index(anchor)]
    lines = [f'{indent}"→¶" &', f'{indent}"{heading}¶" &'] if heading else []
    lines.extend(f"{indent}{row} &" for row in rows)
    return text.replace(anchor + " &", anchor + " &\n" + "\n".join(lines), 1)


def run_function_pass(
    argv: Sequence[str],
    script: str,
    run: Callable[[Path, Path, Path | None], list[str]],
    what: str,
) -> None:
    """The command line of a function-adding pass: [workbook] [src dir] [functions.csv].

    A named index must exist; the default beside the workbook is used when it does, which
    leaves a bare workbook and src/ (the idempotency test's copy) runnable.
    """
    if len(argv) > 4:
        sys.exit(
            f"FAIL: usage: python tools/postbuild/{script} [workbook] [src dir] [functions.csv]"
        )
    workbook = Path(argv[1] if len(argv) > 1 else "ozzit.xlsx")
    src = Path(argv[2] if len(argv) > 2 else "src")
    default = workbook.parent / "functions.csv"
    index = Path(argv[3]) if len(argv) > 3 else (default if default.is_file() else None)
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changed = run(workbook, src, index)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    if changed:
        print(f"added the {what} to: {', '.join(changed)}")
    else:
        print(f"{what} already applied; no changes")
    print(f"OK: {workbook}")


def apply_swaps(text: str, pairs: Iterable[tuple[str, str]]) -> str:
    """Each (old, new) replaced in turn, in the order the caller lists them.

    The caller decides which pairs a store gets and asserts the hit counts first;
    this is only the replacement, so no pass can vary how a swap is written.
    """
    for old, new in pairs:
        text = text.replace(old, new)
    return text
