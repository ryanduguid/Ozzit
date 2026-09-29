"""Working-capital days: 4 ratio functions added in September 2026.

Usage: python tools/postbuild/working_capital_days.py [workbook] [src dir] [functions.csv]

This pass starts from the committed ozzit.xlsx and src/. It adds oz.ReceivableDaysλ,
oz.PayableDaysλ, oz.WIPDaysλ and oz.CashConversionCycleλ to the Ratios group, in every
store at once: src/Ratios.txt, its About table, the defined names in xl/workbook.xml and
functions.csv. The Advanced Formula Environment store is not touched here;
tools/sync_afe_store.py copies src/ into it afterwards and verify_afe gates it.

It follows tools/postbuild/rate_date_helpers.py, which added 4 functions the same way.
The stored form of each definition is rendered by tools/compile_sources.py from the
published source, and that tool's own round-trip through verify_sources.py's comparison
proves the rendering before anything is written. The About table is then recompiled the
same way, so the 2 views cannot drift.

Every insertion carries an asserted count. A second run reports "already applied" and
writes nothing. A store that already holds some of the four but not all of them fails
loudly rather than leaving the views out of step.

Pure text surgery: no COM, no recalculation. No worksheet is added, but the About table's
help changes, so a cached spill of it needs tools/refresh_cache.py in Excel afterwards.
"""

from __future__ import annotations

import csv
import sys
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape as xml_escape

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compile_sources import (  # noqa: E402
    Compiled,
    apply,
    compile_sources,
    update_index,
)
from sanitise_workbook import read_text, write_deterministic, write_text  # noqa: E402
from workbook import oz_names, read_book, read_parts  # noqa: E402
from workbook import workbook_state as book_state  # noqa: E402

NAMESPACE = "oz"
MODULE = "Ratios"


RECEIVABLE_DAYS_SRC = '''/*  FUNCTION NAME:  ReceivableDaysλ
    DESCRIPTION:*//**Receivable days, or days sales outstanding (DSO): Receivables / Sales x Days*/

ReceivableDaysλ = LAMBDA(
//  Parameter Declaration
    [Receivables],
    [Sales],
    [Days],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →ReceivableDaysλ(Receivables, Sales, [Days])¶" &
                            "DESCRIPTION:   →Receivable days, or days sales outstanding (DSO): the days of sales not yet collected.¶NOTES!         →Receivables / Sales * Days. Days is the number of days the Sales figure¶               →covers: 365 with a year's sales, 90 with the last three months' sales for¶               →a rolling figure, or a month's days with that month's sales. Pass closing¶               →receivables, or the average of opening and closing for an average-balance¶               →figure. Put the balance and the sales on the same basis: where one includes¶               →GST and the other does not, adjust the inputs first, because this function¶               →makes no GST adjustment. Sales of 0 returns #DIV/0!, and Days of 0 or less¶               →returns #NUM!.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit/blob/main/docs/ratio-definitions.md¶" &
                            "VERSION:       →29 Sep 2026¶" &
                            "PARAMETERS:    →¶" &
                            "Receivables    →(Required) Trade receivables: the closing balance, or the average of opening and closing.¶" &
                            "Sales          →(Required) Credit sales over the same Days.¶" &
                            "Days           →(Optional: Default = 365) Days the Sales figure covers.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.ReceivableDaysλ(125000, 1200000)¶" &
                            "→Result¶" &
                            "→38.0208¶" &
                            "→=oz.ReceivableDaysλ(125000, 330000, 90)¶" &
                            "→Result¶" &
                            "→34.0909",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(Receivables), ISOMITTED(Sales)),
    //  Procedure
    //  An omitted Days, a blank cell or an empty string is 365, row by row. Anything else
    //  that is not a number is #VALUE!: text compares greater than every number, so "0"
    //  would pass the check below and arithmetic would then read it as 0.
        Period,         IF(ISOMITTED(Days), 365, IF(Days = "", 365, IF(ISNUMBER(Days), Days, #VALUE!))),
        Result,         IF(Period <= 0, #NUM!, Receivables / Sales * Period),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


PAYABLE_DAYS_SRC = '''/*  FUNCTION NAME:  PayableDaysλ
    DESCRIPTION:*//**Payable days, or days payable outstanding (DPO): Payables / Purchases x Days*/

PayableDaysλ = LAMBDA(
//  Parameter Declaration
    [Payables],
    [Purchases],
    [Days],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →PayableDaysλ(Payables, Purchases, [Days])¶" &
                            "DESCRIPTION:   →Payable days, or days payable outstanding (DPO): the days of purchases not yet paid.¶NOTES!         →Payables / Purchases * Days. Days is the number of days the Purchases¶               →figure covers, as for ReceivableDaysλ(). Credit purchases are the matching¶               →flow; cost of sales is the common substitute where purchases are not¶               →reported. Pass closing payables, or the average of opening and closing. Put¶               →the balance and the purchases on the same basis: where one includes GST and¶               →the other does not, adjust the inputs first, because this function makes¶               →no GST adjustment. Purchases of 0 returns #DIV/0!, and Days of 0 or less¶               →returns #NUM!.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit/blob/main/docs/ratio-definitions.md¶" &
                            "VERSION:       →29 Sep 2026¶" &
                            "PARAMETERS:    →¶" &
                            "Payables       →(Required) Trade payables: the closing balance, or the average of opening and closing.¶" &
                            "Purchases      →(Required) Credit purchases, or cost of sales, over the same Days.¶" &
                            "Days           →(Optional: Default = 365) Days the Purchases figure covers.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.PayableDaysλ(64000, 780000)¶" &
                            "→Result¶" &
                            "→29.9487",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(Payables), ISOMITTED(Purchases)),
    //  Procedure
    //  An omitted Days, a blank cell or an empty string is 365, row by row; anything else
    //  that is not a number is #VALUE!, as in ReceivableDaysλ()
        Period,         IF(ISOMITTED(Days), 365, IF(Days = "", 365, IF(ISNUMBER(Days), Days, #VALUE!))),
        Result,         IF(Period <= 0, #NUM!, Payables / Purchases * Period),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


WIP_DAYS_SRC = '''/*  FUNCTION NAME:  WIPDaysλ
    DESCRIPTION:*//**Work in progress days: WIP / RelatedFlow x Days, with both on one valuation basis*/

WIPDaysλ = LAMBDA(
//  Parameter Declaration
    [WIP],
    [RelatedFlow],
    [Days],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →WIPDaysλ(WIP, RelatedFlow, [Days])¶" &
                            "DESCRIPTION:   →Work in progress days: the days of work done but not yet billed or completed.¶NOTES!         →WIP / RelatedFlow * Days. WIP and RelatedFlow must use the same valuation¶               →basis: fee revenue with WIP at charge-out value, as a professional services¶               →firm records it, or cost of sales with WIP at cost. Days is the number of¶               →days the RelatedFlow figure covers. WIP days plus ReceivableDaysλ() gives¶               →the lock-up days a services firm tracks. RelatedFlow of 0 returns #DIV/0!,¶               →and Days of 0 or less returns #NUM!.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit/blob/main/docs/ratio-definitions.md¶" &
                            "VERSION:       →29 Sep 2026¶" &
                            "PARAMETERS:    →¶" &
                            "WIP            →(Required) Work in progress: the closing balance, or the average of opening and closing.¶" &
                            "RelatedFlow    →(Required) Fee revenue or cost of sales over the same Days, on the valuation basis of WIP.¶" &
                            "Days           →(Optional: Default = 365) Days the RelatedFlow figure covers.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.WIPDaysλ(42000, 510000)¶" &
                            "→Result¶" &
                            "→30.0588",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(WIP), ISOMITTED(RelatedFlow)),
    //  Procedure
    //  An omitted Days, a blank cell or an empty string is 365, row by row; anything else
    //  that is not a number is #VALUE!, as in ReceivableDaysλ()
        Period,         IF(ISOMITTED(Days), 365, IF(Days = "", 365, IF(ISNUMBER(Days), Days, #VALUE!))),
        Result,         IF(Period <= 0, #NUM!, WIP / RelatedFlow * Period),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


CASH_CONVERSION_CYCLE_SRC = '''/*  FUNCTION NAME:  CashConversionCycleλ
    DESCRIPTION:*//**Cash conversion cycle in days: inventory days + WIP days + receivable days - payable days*/

CashConversionCycleλ = LAMBDA(
//  Parameter Declaration
    [InventoryDays],
    [ReceivableDays],
    [PayableDays],
    [WIPDays],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →CashConversionCycleλ(InventoryDays, ReceivableDays, PayableDays, [WIPDays])¶" &
                            "DESCRIPTION:   →Cash conversion cycle: the days between paying suppliers and collecting from customers.¶NOTES!         →InventoryDays + WIPDays + ReceivableDays - PayableDays. Pass day counts,¶               →for example from DSIλ(), ReceivableDaysλ(), PayableDaysλ() and WIPDaysλ(),¶               →with every component on the same period basis. DSIλ() multiplies by 365¶               →and expects a full year's cost of goods sold, so for a shorter window¶               →annualise that window's cost of goods sold before passing it. A negative¶               →result means suppliers are paid after customers pay. An omitted or blank¶               →WIPDays counts as 0. Pass WIPDays only where WIP is not already in the¶               →inventory behind InventoryDays, or it is counted twice.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit/blob/main/docs/ratio-definitions.md¶" &
                            "VERSION:       →29 Sep 2026¶" &
                            "PARAMETERS:    →¶" &
                            "InventoryDays  →(Required) Inventory days, for example from DSIλ().¶" &
                            "ReceivableDays →(Required) Receivable days, for example from ReceivableDaysλ().¶" &
                            "PayableDays    →(Required) Payable days, for example from PayableDaysλ().¶" &
                            "WIPDays        →(Optional: Default = 0) Work in progress days, for example from WIPDaysλ().¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.CashConversionCycleλ(40.8949, 38.0208, 29.9487)¶" &
                            "→Result¶" &
                            "→48.967",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(InventoryDays), ISOMITTED(ReceivableDays), ISOMITTED(PayableDays)),
    //  Procedure
    //  An omitted WIPDays, a blank cell or an empty string is 0, row by row
        WIPPart,        IF(ISOMITTED(WIPDays), 0, IF(WIPDays = "", 0, WIPDays)),
        Result,         InventoryDays + WIPPart + ReceivableDays - PayableDays,
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


# The About-table row the additions follow, and the width of the table's label column.
# They join the Efficiency Ratios group, after its last row, so DSIλ sits beside them.
ABOUT_ANCHOR = (
    '"OperatingRatioλ            →Shows how efficient management is at keeping costs low '
    'while generating revenue¶"'
)
ABOUT_WIDTH = 27

# name, About-table description, source block
FUNCTIONS: list[tuple[str, str, str]] = [
    (
        "ReceivableDaysλ",
        "Receivable days (DSO): receivables as days of sales",
        RECEIVABLE_DAYS_SRC,
    ),
    (
        "PayableDaysλ",
        "Payable days (DPO): payables as days of purchases",
        PAYABLE_DAYS_SRC,
    ),
    (
        "WIPDaysλ",
        "Work in progress days: WIP as days of related revenue or cost",
        WIP_DAYS_SRC,
    ),
    (
        "CashConversionCycleλ",
        "Cash conversion cycle: inventory, WIP and receivable days less payable days",
        CASH_CONVERSION_CYCLE_SRC,
    ),
]

QUALIFIED = [f"{NAMESPACE}.{name}" for name, _about, _src in FUNCTIONS]

# No raw & < > may reach xl/workbook.xml as text, so the help must not contain escaped
# forms either. Checked with raise rather than assert, so python -O cannot skip it.
for _name, _about, _block in FUNCTIONS:
    for _bad in ("&amp;", "&lt;", "&gt;"):
        if _bad in _block:
            raise ValueError(f"{_name}: {_bad} is already escaped in the source")
    if "\t" in _block:
        raise ValueError(f"{_name}: tabs do not belong in the source")
    if f"\n{_name} = LAMBDA(" not in _block:
        raise ValueError(f"{_name}: the source block does not declare it")


def about_row(name: str, about: str) -> str:
    return f'"{name:<{ABOUT_WIDTH}}→{about}¶"'


def src_state(text: str) -> str:
    """"absent", "applied", or an error when the module holds only part of the change."""
    present = sum(1 for name, _about, _src in FUNCTIONS if f"\n{name} = LAMBDA(" in text)
    rows = sum(1 for name, about, _src in FUNCTIONS if about_row(name, about) in text)
    if present == 0 and rows == 0:
        return "absent"
    if present == len(FUNCTIONS) and rows == len(FUNCTIONS):
        return "applied"
    raise ValueError(
        f"src/{MODULE}.txt holds {present} of {len(FUNCTIONS)} functions and {rows} "
        f"About rows; it is neither state this pass recognises"
    )


def workbook_state(book: str) -> str:
    return book_state(book, QUALIFIED)


def index_state(index: Path | None) -> str | None:
    """"absent", "applied", or None when no index is supplied, raising on a partial one.

    A supplied index that does not exist is an error, not "no index": treated as absent
    from the check, it let the pass write src/ and the workbook and exit 0 with the
    index left behind.
    """
    if index is None:
        return None
    if not index.is_file():
        raise ValueError(f"{index}: no such index; supply an existing functions.csv or none")
    with index.open(encoding="utf-8-sig", newline="") as handle:
        listed = {row.get("function", "") for row in csv.DictReader(handle)}
    present = sum(1 for name in QUALIFIED if name in listed)
    if present == 0:
        return "absent"
    if present == len(QUALIFIED):
        return "applied"
    raise ValueError(
        f"{index.name} holds {present} of {len(QUALIFIED)} working-capital rows; "
        "it is neither state this pass recognises"
    )


def add_to_src(text: str) -> str:
    """The module with its About rows inserted after the anchor and its blocks appended."""
    hits = text.count(ABOUT_ANCHOR)
    if hits != 1:
        raise ValueError(f"src/{MODULE}.txt: About anchor found {hits} times, expected 1")
    line_start = text.rfind("\n", 0, text.index(ABOUT_ANCHOR)) + 1
    indent = text[line_start : text.index(ABOUT_ANCHOR)]
    rows = [f"{indent}{about_row(name, about)} &" for name, about, _src in FUNCTIONS]
    text = text.replace(ABOUT_ANCHOR + " &", ABOUT_ANCHOR + " &\n" + "\n".join(rows), 1)

    # The module keeps its own ending. Ratios.txt ends without a final newline, which
    # .editorconfig exempts and tools/tests/test_repository_policy.py expects of one module.
    blocks = "\n\n\n" + "\n\n\n".join(src for _name, _about, src in FUNCTIONS)
    return text.rstrip("\n") + blocks + ("\n" if text.endswith("\n") else "")


def insert_names(book: str, compiled: list[Compiled]) -> str:
    """Each new defined name after its case-insensitive alphabetical predecessor."""
    existing = sorted(oz_names(book), key=str.lower)
    for item in sorted(compiled, key=lambda c: c.name.lower()):
        comment = xml_escape(item.comment).replace('"', "&quot;")
        element = (
            f'<definedName name="{xml_escape(item.name)}" comment="{comment}">'
            f"{xml_escape(item.stored)}</definedName>"
        )
        before = [n for n in existing if n.lower() < item.name.lower()]
        if not before:
            raise ValueError(f"{item.name} sorts before every shipped name, which is not expected")
        marker = f'<definedName name="{xml_escape(before[-1])}"'
        start = book.index(marker)
        end = book.index("</definedName>", start) + len("</definedName>")
        book = book[:end] + element + book[end:]
        existing = sorted(existing + [item.name], key=str.lower)
    return book


def run(workbook: Path, src: Path, index: Path | None) -> list[str]:
    path = src / f"{MODULE}.txt"
    if not path.is_file():
        raise ValueError(f"src: missing {path}")
    text = read_text(path)

    parts = read_parts(workbook)
    book = read_book(parts)

    states = {MODULE: src_state(text), "workbook": workbook_state(book)}
    supplied_index = index_state(index)
    if supplied_index is not None:
        states[index.name if index is not None else "index"] = supplied_index
    if len(set(states.values())) != 1:
        raise ValueError(
            "the stores disagree (%s); the views must not be applied separately"
            % ", ".join(f"{store} {state}" for store, state in states.items())
        )
    if states["workbook"] == "applied":
        return []
    # Without an index the pass can only confirm the applied state, as the idempotency
    # test's bare copy needs; applying it would leave functions.csv behind.
    if index is None:
        raise ValueError("functions.csv is required to apply the working-capital days functions")

    # The compiler reads src/ from disk, so src/ is written first and the index next;
    # the workbook, which is what marks the pass applied, is written last. Every write,
    # the first included, is inside the recovery block, so a failure anywhere restores
    # all three stores.
    changed: list[str] = []
    index_before = index.read_bytes() if index is not None else None
    indexed = False
    try:
        write_text(path, add_to_src(text))
        changed.append(f"src/{MODULE}.txt")
        compiled = compile_sources(src)
        book = insert_names(book, [c for c in compiled if c.name in QUALIFIED])
        book, _recompiled = apply(book, compiled)
        if index is not None:
            indexed = update_index(index, book, {c.name: c.module for c in compiled})
        parts["xl/workbook.xml"] = book.encode("utf-8")
        write_deterministic(workbook, parts)
    except Exception:
        write_text(path, text)
        if index is not None and index_before is not None:
            index.write_bytes(index_before)
        raise
    changed.append("workbook")
    if indexed and index is not None:
        changed.append(index.name)
    return changed


def main() -> None:
    if len(sys.argv) > 4:
        sys.exit(
            "FAIL: usage: python tools/postbuild/working_capital_days.py "
            "[workbook] [src dir] [functions.csv]"
        )
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    src = Path(sys.argv[2] if len(sys.argv) > 2 else "src")
    # A named index must exist; the default beside the workbook is used when it does,
    # which leaves a bare workbook and src/ (the idempotency test's copy) runnable.
    default = workbook.parent / "functions.csv"
    index = Path(sys.argv[3]) if len(sys.argv) > 3 else (default if default.is_file() else None)
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changed = run(workbook, src, index)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    if changed:
        print(f"added the working-capital days functions to: {', '.join(changed)}")
    else:
        print("working-capital days functions already applied; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
