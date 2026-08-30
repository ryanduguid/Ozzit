"""Correction pass: omitted-argument defaults and the help rows that contradict them.

Usage: python tools/postbuild/help_corrections.py [workbook] [src dir]

Nine functions shipped disagreeing with their own inline help. Three of the
corrections below change what a formula computes; the other six change only what
the help claims.

Code, in SWAPS:

    IsBetweenEλ, IsBetweenUλ   never bound Inclusive, so an omitted Inclusive
                               evaluated as FALSE and both returned the exclusive
                               comparison their own help calls the default. The
                               binding added here is the one Dates' IsBetweenλ has
                               carried all along.
    CorkscrewλDV               gated its messages on an undocumented sixth
                               argument, so calling it the way the About tables
                               say returned #VALUE! instead of the diagnosis.
                               Its two siblings never had that multiplier.

Help text, also in SWAPS:

    Movementλ                  its example passed three arguments to a
                               two-parameter LAMBDA, so it could not be evaluated.
    Reversalλ                  its example printed the negated input rather than
                               the reversal, which is one period later.
    PeriodDiffλ                its example printed 1 where the formula returns 2.
    AboutDatesλ                its DIAGNOSTICS block named ten λDV functions the
                               library has never declared.
    AboutRatiosλ               it listed DSIλ and DPRλ under their pre-rename
                               names.
    AboutFinancialλ            it listed Book Value among the rows
                               SumDepreciateλ totals, which it does not.

Every swap carries an asserted hit count in both stores, and no `old` anchor is a
substring of its `new`, so a second run reports "already applied" and writes
nothing. A store whose anchors do not match fails loudly rather than leaving the
workbook and source views out of sync.

The workbook shows the same statements a second way, in cells no formula feeds,
and those copies do not follow the defined names:

    CELL_SWAPS     the two corrected examples are spilled onto the Reversalλ and
                   Movementλ demonstration sheets, and the cells caching that
                   spill hold the old text until Excel next recalculates. Excel
                   does refresh these, and tools/verify_cache.py is the gate that
                   proves it.
    STRING_SWAPS   five shared strings are the whole content of a static literal
                   cell: label and description columns typed out beside the
                   demonstrations, with no formula and under no spill anchor.
                   Excel never refreshes them, so a reader sees the pre-rename
                   name or the withdrawn claim for as long as the string stands.
                   Correcting the defined name alone would leave every one of
                   them saying what this pass has just denied.

Pure text surgery: no COM, no recalculation. Run tools/sync_afe_store.py
afterwards, as the postbuild README's run order requires for any pass that
changes src/.
"""

from __future__ import annotations

import sys
import zipfile
from pathlib import Path
from typing import NamedTuple

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from sanitise_workbook import write_deterministic

MODULES = ("Dates", "Essentials", "Financial", "Ratios", "Utilities", "Debt")


class Swap(NamedTuple):
    """One correction, in each store's own serialisation of the same function."""

    what: str
    workbook: tuple[str, str]
    src: tuple[str, str]
    hits: int = 1


def shared(what: str, old: str, new: str, hits: int = 1) -> Swap:
    """A correction whose text is byte-identical in the workbook and in src/."""
    return Swap(what, (old, new), (old, new), hits)


# The stored form qualifies parameters with _xlpm./_xlop. and post-2007 functions
# with _xlfn., and writes & as &amp;. src/ is the typed form. Neither is derived
# from the other, so both anchors are spelled out.
WORKBOOK_INCLUSIVE_OLD = (
    '"→", "¶")), _xlpm.Help?, OR(_xlfn.ISOMITTED(_xlpm.Value), '
    "_xlfn.ISOMITTED(_xlpm.Low), _xlfn.ISOMITTED(_xlpm.Hi)),"
)
WORKBOOK_INCLUSIVE_NEW = (
    '"→", "¶")), _xlpm.Inclusive, IF(ISLOGICAL(_xlpm.Inclusive), _xlpm.Inclusive, TRUE), '
    "_xlpm.Help?, OR(_xlfn.ISOMITTED(_xlpm.Value), "
    "_xlfn.ISOMITTED(_xlpm.Low), _xlfn.ISOMITTED(_xlpm.Hi)),"
)
# IsInListλ and IsInListUλ open with the same omitted-argument block, so the
# anchor runs on to the Result line that only the two IsBetween copies carry.
SRC_INCLUSIVE_TAIL = (
    "    //  Check inputs - Omitted required arguments\n"
    "        Help?,          OR( ISOMITTED( Value),\n"
    "                            ISOMITTED( Low),\n"
    "                            ISOMITTED( Hi)\n"
    "                        ),\n"
    "    //  Procedure\n"
    "        Result,         IF( Inclusive, \n"
)
SRC_INCLUSIVE_OLD = "                        ),\n" + SRC_INCLUSIVE_TAIL
SRC_INCLUSIVE_NEW = (
    "                        ),\n"
    "    //  Check inputs - Set optional arguments defaults\n"
    "        Inclusive,      IF( ISLOGICAL( Inclusive), Inclusive, TRUE),\n"
    + SRC_INCLUSIVE_TAIL
)

# AboutDatesλ's DIAGNOSTICS block, row by row. The stored defined name collapses
# the label padding src/ writes; nothing else about the two differs. The trailing
# "Timelineλ →" is the block's eleventh entry with its DV suffix missing, not a
# second row for Timelineλ, so it goes with the rest. What replaces the block is
# the empty last row every other About table ends on.
WORKBOOK_DIAGNOSTICS = (
    "&amp; "
    '"→¶" &amp; '
    '"DIAGNOSTICS →These functions diagnose problems in their associate function\'s arguments.¶" &amp; '
    '"→Should a function not work, it may be because there is a problem with the arguments.¶" &amp; '
    '"→Argument problems can include text where numbers should be, invalid dates, etc.¶" &amp; '
    '"→To see if you have argument problems, insert \'DV\' between \'λ\' and \'(\'. ¶" &amp; '
    '"→Here is an example: CountDOWλDV( Start, End, 1). Hit enter to run diagnostics.¶" &amp; '
    '"→When finished, remove \'DV\' to return the function to normal operation¶" &amp; '
    '"CountDOWλDV →¶" &amp; '
    '"IsBetweenλDV →¶" &amp; '
    '"IsOccurrenceDateλDV →¶" &amp; '
    '"OverLapDaysλDV →¶" &amp; '
    '"PeriodsλDV →¶" &amp; '
    '"PeriodLabelλDV →¶" &amp; '
    '"ScheduleRatesλDV →¶" &amp; '
    '"ScheduleRatesByItemsλDV→¶" &amp; '
    '"ScheduleValuesλDV →¶" &amp; '
    '"ScheduleValuesByItemsλDV→¶" &amp; '
    '"Timelineλ →"'
)
SRC_DIAGNOSTICS = (
    '"→¶" &\n        '
    '"DIAGNOSTICS            →These functions diagnose problems in their associate function\'s arguments.¶" & \n                               '
    '"→Should a function not work, it may be because there is a problem with the arguments.¶" & \n                               '
    '"→Argument problems can include text where numbers should be, invalid dates, etc.¶" & \n                               '
    '"→To see if you have argument problems, insert \'DV\' between \'λ\' and \'(\'. ¶" & \n                               '
    '"→Here is an example: CountDOWλDV( Start, End, 1). Hit enter to run diagnostics.¶" & \n                               '
    '"→When finished, remove \'DV\' to return the function to normal operation¶" & \n        '
    '"CountDOWλDV            →¶" &\n        '
    '"IsBetweenλDV           →¶" &\n        '
    '"IsOccurrenceDateλDV    →¶" &\n        '
    '"OverLapDaysλDV         →¶" &\n        '
    '"PeriodsλDV             →¶" &\n        '
    '"PeriodLabelλDV         →¶" &\n        '
    '"ScheduleRatesλDV       →¶" &\n        '
    '"ScheduleRatesByItemsλDV→¶" &\n        '
    '"ScheduleValuesλDV      →¶" &\n        '
    '"ScheduleValuesByItemsλDV→¶" &\n        '
    '"Timelineλ              →"'
)
# The row above the block, which is what makes the replacement unique: every
# other About table also ends on &amp; "→".
DIAGNOSTICS_LEAD = "starting 1 July¶\" "

SWAPS: tuple[Swap, ...] = (
    Swap(
        "IsBetweenEλ and IsBetweenUλ default Inclusive to TRUE",
        (WORKBOOK_INCLUSIVE_OLD, WORKBOOK_INCLUSIVE_NEW),
        (SRC_INCLUSIVE_OLD, SRC_INCLUSIVE_NEW),
        2,
    ),
    shared(
        "Movementλ's example calls it with two arguments",
        "=oz.Movementλ(,{100,110,130,100}, 4)",
        "=oz.Movementλ(,{100,110,130,100})",
    ),
    shared(
        "Reversalλ's example prints the row the function returns",
        '"-100,-110,-130 →=oz.Reversalλ( , {100,110,130})"',
        '"0,-100,-110    →=oz.Reversalλ( , {100,110,130})"',
    ),
    Swap(
        "AboutDatesλ drops the DIAGNOSTICS block for functions it never shipped",
        (
            DIAGNOSTICS_LEAD + WORKBOOK_DIAGNOSTICS,
            DIAGNOSTICS_LEAD + '&amp; "→"',
        ),
        (
            'starting 1 July¶" &\n        ' + SRC_DIAGNOSTICS,
            'starting 1 July¶" &\n        "→"',
        ),
    ),
    shared(
        "AboutRatiosλ names DSIλ as the library declares it",
        '"DSIRatioλ                  →Days Sales in Inventory Ratio',
        '"DSIλ                       →Days Sales in Inventory Ratio',
    ),
    shared(
        "AboutRatiosλ names DPRλ as the library declares it",
        '"DividendPayoutRatioλ       →Dividend Payout Ratio is',
        '"DPRλ                       →Dividend Payout Ratio is',
    ),
    shared(
        "PeriodDiffλ's example prints the 2 its formula returns",
        '"1              →=oz.PeriodDiffλ(""2026-01-01""',
        '"2              →=oz.PeriodDiffλ(""2026-01-01""',
    ),
    Swap(
        "CorkscrewλDV drops the undocumented Diagnostics parameter",
        (
            "_xlop.Flow4,_xlop.Diagnostics, _xlfn.LET(_xlpm.ErrorsInArgs,",
            "_xlop.Flow4, _xlfn.LET(_xlpm.ErrorsInArgs,",
        ),
        (
            "    [Flow4],\n    [Diagnostics],\n\n    LET(\n"
            "    //  Check inputs - Errors in argument values\n"
            "        ErrorsInArgs,   VSTACK( \n                            OR( ISERROR( Opening)),",
            "    [Flow4],\n\n    LET(\n"
            "    //  Check inputs - Errors in argument values\n"
            "        ErrorsInArgs,   VSTACK( \n                            OR( ISERROR( Opening)),",
        ),
    ),
    Swap(
        "CorkscrewλDV shows its messages the way AmortiseλDV and DepreciateλDV do",
        (
            "_xlpm.Errors2Show, _xlfn.VSTACK(_xlpm.Diagnostics * _xlpm.ErrorsInArgs, "
            '_xlpm.Diagnostics * _xlpm.DVErrors), _xlpm.ErrMsgs, {"Opening contains errors',
            "_xlpm.Errors2Show, _xlfn.VSTACK(_xlpm.ErrorsInArgs, _xlpm.DVErrors), "
            '_xlpm.ErrMsgs, {"Opening contains errors',
        ),
        (
            "        Errors2Show,    VSTACK( Diagnostics * ErrorsInArgs, Diagnostics * DVErrors), \n"
            '        ErrMsgs,        {"Opening contains errors',
            "        Errors2Show,    VSTACK( ErrorsInArgs, DVErrors), \n"
            '        ErrMsgs,        {"Opening contains errors',
        ),
    ),
    shared(
        "AboutFinancialλ stops listing Book Value among SumDepreciateλ's totals",
        '"SumDepreciateλ     →Create row totals for CAPEX, Depreciation, Book Value, '
        'Salvage Value, and Disposal costs in Depreciateλ results¶"',
        '"SumDepreciateλ     →Create row totals for CAPEX, Depreciation, '
        'Salvage Value, and Disposal costs in Depreciateλ results¶"',
    ),
)

# The demonstration sheets cache the help each function spills. Only the two
# corrected example rows are cached anywhere, and each is one whole cell value.
CELL_SWAPS: tuple[tuple[str, str, str], ...] = (
    (
        "oz.Reversalλ's cached example row",
        "<v>-100,-110,-130</v>",
        "<v>0,-100,-110</v>",
    ),
    (
        "oz.Movementλ's cached example formula",
        "<v>=oz.Movementλ(,{100,110,130,100}, 4)</v>",
        "<v>=oz.Movementλ(,{100,110,130,100})</v>",
    ),
)

# Static literal cells: typed once, referenced through the shared string table,
# fed by no formula and covered by no spill anchor, so Excel never revisits them.
# Each anchor is a whole <si> element, which is both unique in the part and the
# entire value of every cell pointing at it. Replacing a whole element cannot
# collide with another string or change the table's length, so the sst element's
# count and uniqueCount stay correct; none of the new texts is already an <si>,
# so no two entries become duplicates.
STRING_SWAPS: tuple[tuple[str, str, str], ...] = (
    (
        "oz.FinancialRatios D11 names AboutRatiosλ",
        "<si><t>Aboutλ</t></si>",
        "<si><t>AboutRatiosλ</t></si>",
    ),
    (
        "oz.FinancialRatios D36 names DSIλ",
        "<si><t>DSIRatioλ</t></si>",
        "<si><t>DSIλ</t></si>",
    ),
    (
        "oz.FinancialRatios D58 names DPRλ",
        "<si><t>DividendPayoutRatioλ</t></si>",
        "<si><t>DPRλ</t></si>",
    ),
    (
        "TOC D39 and oz.SumDepreciateλ A2 drop Book Value",
        "<si><t>Create row totals for CAPEX, Depreciation, Book Value, Salvage Value,"
        " and Disposal costs in Depreciateλ results</t></si>",
        "<si><t>Create row totals for CAPEX, Depreciation, Salvage Value,"
        " and Disposal costs in Depreciateλ results</t></si>",
    ),
    (
        "oz.IsOccurrenceDateλ K4 drops the Diagnostics argument",
        "<si><t>IsOccurrenceDateλ(Dates, FirstOccurrence, [LastOccurrence],"
        " [Repeats], [Diagnostics])</t></si>",
        "<si><t>IsOccurrenceDateλ(Dates, FirstOccurrence, [LastOccurrence],"
        " [Repeats])</t></si>",
    ),
)

for _what, _old, _new in CELL_SWAPS + STRING_SWAPS:
    assert _old not in _new and _new not in _old, (
        f"{_what}: an anchor contains its own replacement, so a second run could "
        "not tell the two states apart"
    )

for _swap in SWAPS:
    for _old, _new in (_swap.workbook, _swap.src):
        assert _old not in _new and _new not in _old, (
            f"{_swap.what}: an anchor contains its own replacement, so a second "
            "run could not tell the two states apart"
        )


def validate_store(text: str, store: str, failures: list[str]) -> None:
    """Require every target in this store, in one of the two recognised states."""
    for swap in SWAPS:
        old, new = swap.workbook if store == "workbook" else swap.src
        counts = {"before": text.count(old), "after": text.count(new)}
        hits = sum(counts.values())
        if hits != swap.hits:
            states = ", ".join(f"{state}={count}" for state, count in counts.items())
            failures.append(
                f"{store}: {swap.what} expected {swap.hits} recognised anchor(s), "
                f"got {hits} ({states})"
            )


def validate_cells(sheets: dict[str, str], failures: list[str]) -> None:
    """Each cached example row must be present exactly once across the worksheets."""
    for what, old, new in CELL_SWAPS:
        counts = {
            "before": sum(text.count(old) for text in sheets.values()),
            "after": sum(text.count(new) for text in sheets.values()),
        }
        hits = sum(counts.values())
        if hits != 1:
            states = ", ".join(f"{state}={count}" for state, count in counts.items())
            failures.append(
                f"worksheets: {what} expected 1 recognised anchor, got {hits} ({states})"
            )


def validate_strings(strings: str, failures: list[str]) -> None:
    """Each static literal must be present exactly once, in one of the two states."""
    for what, old, new in STRING_SWAPS:
        counts = {"before": strings.count(old), "after": strings.count(new)}
        hits = sum(counts.values())
        if hits != 1:
            states = ", ".join(f"{state}={count}" for state, count in counts.items())
            failures.append(
                f"sharedStrings: {what} expected 1 recognised anchor, got {hits} ({states})"
            )


def apply_swaps(text: str, store: str) -> str:
    for swap in SWAPS:
        old, new = swap.workbook if store == "workbook" else swap.src
        text = text.replace(old, new)
    return text


def apply_cell_swaps(text: str) -> str:
    for _what, old, new in CELL_SWAPS:
        text = text.replace(old, new)
    return text


def apply_string_swaps(text: str) -> str:
    for _what, old, new in STRING_SWAPS:
        text = text.replace(old, new)
    return text


def _read_text(path: Path) -> str:
    with open(path, encoding="utf-8", newline="") as handle:
        return handle.read()


def _write_text(path: Path, text: str) -> None:
    with open(path, "w", encoding="utf-8", newline="") as handle:
        handle.write(text)


def run(workbook: Path, src_dir: Path) -> list[str]:
    failures: list[str] = []

    src_originals: dict[str, str] = {}
    for module in MODULES:
        path = src_dir / f"{module}.txt"
        if not path.is_file():
            failures.append(f"src: missing {path}")
            continue
        src_originals[module] = _read_text(path)

    with zipfile.ZipFile(workbook) as archive:
        parts = {name: archive.read(name) for name in archive.namelist()}
    book = parts["xl/workbook.xml"].decode("utf-8")
    sheets = {
        name: data.decode("utf-8")
        for name, data in parts.items()
        if name.startswith("xl/worksheets/")
    }
    if "xl/sharedStrings.xml" not in parts:
        raise ValueError("workbook: xl/sharedStrings.xml is missing")
    strings = parts["xl/sharedStrings.xml"].decode("utf-8")

    validate_store(book, "workbook", failures)
    # Per module most anchors are legitimately absent, so the counts are asserted
    # on the whole library the way the FY27 pass asserts its aggregate.
    validate_store("\n".join(src_originals.values()), "src", failures)
    validate_cells(sheets, failures)
    validate_strings(strings, failures)

    if failures:
        raise ValueError("; ".join(failures))

    changed: list[str] = []
    updated_book = apply_swaps(book, "workbook").encode("utf-8")
    rewritten = updated_book != parts["xl/workbook.xml"]
    parts["xl/workbook.xml"] = updated_book
    for name, text in sheets.items():
        updated_sheet = apply_cell_swaps(text).encode("utf-8")
        if updated_sheet != parts[name]:
            parts[name] = updated_sheet
            rewritten = True
    updated_strings = apply_string_swaps(strings).encode("utf-8")
    if updated_strings != parts["xl/sharedStrings.xml"]:
        parts["xl/sharedStrings.xml"] = updated_strings
        rewritten = True
    if rewritten:
        write_deterministic(workbook, parts)
        changed.append("workbook")

    for module, original in src_originals.items():
        text = apply_swaps(original, "src")
        if text != original:
            _write_text(src_dir / f"{module}.txt", text)
            changed.append(f"src/{module}.txt")

    return changed


def main() -> None:
    if len(sys.argv) > 3:
        sys.exit(
            "FAIL: usage: python tools/postbuild/help_corrections.py [workbook] [src dir]"
        )
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    src_dir = Path(sys.argv[2] if len(sys.argv) > 2 else "src")
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changed = run(workbook, src_dir)
    except (OSError, ValueError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    if changed:
        print(f"applied help corrections to: {', '.join(changed)}")
    else:
        print("help corrections already applied; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
