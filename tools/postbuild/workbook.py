"""Workbook reading shared by the postbuild passes.

Three things were copied from pass to pass rather than shared: opening the
archive and decoding xl/workbook.xml, reading whether a pass is already applied
from the defined names it adds, and replacing a table of text swaps. Ten passes
held nine copies of the first, two near-identical copies of the second, and four
copies of the third under four different signatures. One copy each lives here.

Reading only. Each pass still owns its own writing, because the order it writes
its stores in and what it restores on failure are decisions the pass makes, not
decisions this module can make for it. Text surgery as before: no COM, no
recalculation.
"""

from __future__ import annotations

import re
import zipfile
from collections.abc import Iterable, Sequence
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


def apply_swaps(text: str, pairs: Iterable[tuple[str, str]]) -> str:
    """Each (old, new) replaced in turn, in the order the caller lists them.

    The caller decides which pairs a store gets and asserts the hit counts first;
    this is only the replacement, so no pass can vary how a swap is written.
    """
    for old, new in pairs:
        text = text.replace(old, new)
    return text
