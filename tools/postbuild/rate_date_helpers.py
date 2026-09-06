"""Rate and date helpers: four functions added in September 2026.

Usage: python tools/postbuild/rate_date_helpers.py [workbook] [src dir] [functions.csv]

This pass starts from the committed ozzit.xlsx and src/ (the post-v3.0.0 input
recorded in ATTRIBUTION.md). It adds oz.PeriodRateλ, oz.AnnualRateλ and
oz.DayCountRateλ to the Financial group and oz.DateDifλ to the Dates group, in
every store at once: the src module, its About table, the defined names in
xl/workbook.xml and functions.csv. The Advanced Formula Environment store is not
touched here; tools/sync_afe_store.py copies src/ into it afterwards and
verify_afe gates it.

The stored form of each definition is rendered by tools/compile_sources.py from
the published source, and that tool's own round-trip through verify_sources.py's
comparison proves the rendering before anything is written. The About tables are
then recompiled the same way, so the two views cannot drift.

Every insertion carries an asserted count. A second run reports "already applied"
and writes nothing. A store that already holds some of the four but not all of
them fails loudly rather than leaving the views out of step.

Pure text surgery: no COM, no recalculation. No worksheet is added, so no cached
value moves and verify_cache.py is unaffected.
"""

from __future__ import annotations

import re
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compile_sources import (  # noqa: E402
    Compiled,
    apply,
    compile_sources,
    read_text,
    update_index,
    xml_escape,
)
from sanitise_workbook import write_deterministic  # noqa: E402

NAMESPACE = "oz"


PERIOD_RATE_SRC = '''/*  FUNCTION NAME:  PeriodRateλ
    DESCRIPTION:*//**Converts an effective annual rate to the equivalent rate per period, the rate the lease functions take*/

PeriodRateλ = LAMBDA(
//  Parameter Declaration
    [AnnualRate],
    [PeriodsPerYear],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →PeriodRateλ(AnnualRate, [PeriodsPerYear])¶" &
                            "DESCRIPTION:   →Converts an effective annual rate to the equivalent rate per period.¶NOTES!         →(1 + AnnualRate) ^ (1 / PeriodsPerYear) - 1, so compounding the result¶               →PeriodsPerYear times gives the annual rate back. This is the rate per¶               →period that LeaseLiabilityλ(), LeaseScheduleλ() and LeaseRemeasureλ()¶               →take. It is for an effective annual rate. A nominal annual percentage¶               →rate that compounds monthly is already APR / 12 per month, so do not¶               →pass it here; EFFECT() turns one into the effective rate this takes.¶               →AnnualRateλ() is the inverse.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶" &
                            "VERSION:       →6 Sep 2026¶" &
                            "PARAMETERS:    →¶" &
                            "AnnualRate     →(Required) Effective annual rate, or a row or column of them.¶" &
                            "PeriodsPerYear →(Optional: Default = 12) Periods in a year: 12 for months, 4 for quarters, 1 for years.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.PeriodRateλ(0.05)¶" &
                            "→Result¶" &
                            "→0.004074",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          ISOMITTED(AnnualRate),
    //  A blank PeriodsPerYear cell is not an omitted argument, so test for both
        Periods,        IF(OR(ISOMITTED(PeriodsPerYear), NOT(ISNUMBER(PeriodsPerYear))), 12, PeriodsPerYear),
        Result,         (1 + AnnualRate) ^ (1 / Periods) - 1,
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


ANNUAL_RATE_SRC = '''/*  FUNCTION NAME:  AnnualRateλ
    DESCRIPTION:*//**Converts a rate per period to the effective annual rate, the inverse of PeriodRateλ*/

AnnualRateλ = LAMBDA(
//  Parameter Declaration
    [PeriodRate],
    [PeriodsPerYear],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →AnnualRateλ(PeriodRate, [PeriodsPerYear])¶" &
                            "DESCRIPTION:   →Converts a rate per period to the effective annual rate.¶NOTES!         →(1 + PeriodRate) ^ PeriodsPerYear - 1, the inverse of PeriodRateλ().¶               →Use it to state a rate per period, such as a lease rate solved per¶               →month, as the annual figure a reader expects.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶" &
                            "VERSION:       →6 Sep 2026¶" &
                            "PARAMETERS:    →¶" &
                            "PeriodRate     →(Required) Rate per period, or a row or column of them.¶" &
                            "PeriodsPerYear →(Optional: Default = 12) Periods in a year: 12 for months, 4 for quarters, 1 for years.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.AnnualRateλ(0.01)¶" &
                            "→Result¶" &
                            "→0.126825",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          ISOMITTED(PeriodRate),
    //  A blank PeriodsPerYear cell is not an omitted argument, so test for both
        Periods,        IF(OR(ISOMITTED(PeriodsPerYear), NOT(ISNUMBER(PeriodsPerYear))), 12, PeriodsPerYear),
        Result,         (1 + PeriodRate) ^ Periods - 1,
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


DAY_COUNT_RATE_SRC = '''/*  FUNCTION NAME:  DayCountRateλ
    DESCRIPTION:*//**Interest rate for each period of a timeline under a day count convention: 30/360, Actual/360, Actual/365 or Actual/Actual*/

DayCountRateλ = LAMBDA(
//  Parameter Declaration
    [Timeline],
    [APR],
    [Convention],
    [EndDates],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →DayCountRateλ(Timeline, APR, [Convention], [EndDates])¶" &
                            "DESCRIPTION:   →Interest rate for each period of a timeline under a day count convention.¶NOTES!         →Returns a row with one rate per timeline period, whichever way the¶               →timeline runs: APR multiplied by the days in the period and divided by¶               →the days in the year, as the convention counts them. Pass the result to¶               →DebtSculptVariableλ() or DebtSculptVariableLRVλ() as PeriodRates in place¶               →of a flat twelfth of the APR. With start dates each period runs from its¶               →date to the day before the next, and the last period takes its length¶               →from the one before it; with end dates each runs from the day after the¶               →previous date to its own, and the first period is the one inferred. A¶               →gap of 28 days or more is read as whole calendar months, a shorter one¶               →as days. 30/360 counts every month as 30 days on the European rule.¶               →Under Actual/Actual a period that straddles 31 December splits its days¶               →between the two years, each over that year's own length (the ISDA rule).¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶" &
                            "VERSION:       →6 Sep 2026¶" &
                            "PARAMETERS:    →¶" &
                            "Timeline       →(Required) A row or column of period dates, at least two, evenly spaced.¶" &
                            "APR            →(Required) Annual percentage rate, one value or one per period.¶" &
                            "Convention     →(Optional: Default = 3) 1 = 30/360, 2 = Actual/360, 3 = Actual/365, 4 = Actual/Actual. Any other number is #VALUE!.¶" &
                            "EndDates       →(Optional: Default = FALSE) TRUE, or a number other than 0, when the timeline shows period end dates rather than start dates.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.DayCountRateλ(EDATE(DATE(2026,7,1), {0,1,2}), 0.073)¶" &
                            "→Result¶" &
                            "→0.0062, 0.0062, 0.0060",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(Timeline), ISOMITTED(APR)),
    //  A blank cell is not an omitted argument, so test for both
        Method,         IF(OR(ISOMITTED(Convention), NOT(ISNUMBER(Convention))), 3, Convention),
        EndFlag,        IF(ISLOGICAL(EndDates), EndDates, IF(ISNUMBER(EndDates), EndDates <> 0, FALSE)),
    //  One timeline laid along a row, text dates read, and a column of rates laid the same way
        Row,            IF(ROWS(Timeline) = 1, Timeline, TRANSPOSE(Timeline)),
        Dates,          IF(ISNUMBER(Row), Row, DATEVALUE(Row)),
        Rate,           IF(ROWS(APR) = 1, APR, TRANSPOSE(APR)),
        Count,          COLUMNS(Dates),
    //  The period length is read off the first gap for the first period and the last gap
    //  for the last, in whole months from 28 days up and in days below that. A month-end
    //  date steps back through EOMONTH, because EDATE(28 Feb, -1) is 28 Jan, not 31 Jan.
        FirstGap,       INDEX(Dates, 1, 2) - INDEX(Dates, 1, 1),
        LastGap,        INDEX(Dates, 1, Count) - INDEX(Dates, 1, Count - 1),
        FirstDate,      INDEX(Dates, 1, 1),
        FirstMonths,    ROUND(FirstGap / 30.5, 0),
        FirstStart,     IF(FirstGap < 28,
                            FirstDate - FirstGap,
                            IF(DAY(FirstDate + 1) = 1,
                                EOMONTH(FirstDate, -FirstMonths),
                                EDATE(FirstDate, -FirstMonths))) + 1,
        LastDate,       INDEX(Dates, 1, Count),
        LastMonths,     ROUND(LastGap / 30.5, 0),
        LastEnd,        IF(LastGap < 28,
                            LastDate + LastGap,
                            IF(DAY(LastDate + 1) = 1,
                                EOMONTH(LastDate, LastMonths),
                                EDATE(LastDate, LastMonths))) - 1,
    //  Every period runs from its start to the day before the next start
        Starts,         IF(EndFlag, HSTACK(FirstStart, DROP(Dates, , -1) + 1), Dates),
        Ends,           IF(EndFlag, Dates, HSTACK(DROP(Dates, , 1) - 1, LastEnd)),
        Next,           Ends + 1,
    //  30/360 on the European rule: a 31st counts as the 30th at either end
        Days,           IF(Method = 1,
                            360 * (YEAR(Next) - YEAR(Starts)) + 30 * (MONTH(Next) - MONTH(Starts))
                                + IF(DAY(Next) > 30, 30, DAY(Next)) - IF(DAY(Starts) > 30, 30, DAY(Starts)),
                            Next - Starts),
    //  Actual/Actual splits a period at 1 January, each part over its own year's length.
    //  IF rather than MIN, because MIN would collapse the row to one value.
        YearOne,        YEAR(Starts),
        Boundary,       DATE(YearOne + 1, 1, 1),
        Split,          IF(Next < Boundary, Next, Boundary),
        DaysOne,        IF(DAY(DATE(YearOne, 2, 29)) = 29, 366, 365),
        DaysTwo,        IF(DAY(DATE(YearOne + 1, 2, 29)) = 29, 366, 365),
        Fraction,       SWITCH(Method,
                            1, Days / 360,
                            2, Days / 360,
                            4, (Split - Starts) / DaysOne + (Next - Split) / DaysTwo,
                            Days / 365),
        Rates,          Rate * Fraction,
        Result,         IF(OR(Count < 2, NOT(OR(Method = {1,2,3,4}))), #VALUE!, Rates),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


DATE_DIF_SRC = '''/*  FUNCTION NAME:  DateDifλ
    DESCRIPTION:*//**Whole years, months or days between two dates, and the remainders DATEDIF gets wrong*/

DateDifλ = LAMBDA(
//  Parameter Declaration
    [StartDate],
    [EndDate],
    [Unit],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →DateDifλ(StartDate, EndDate, [Unit])¶" &
                            "DESCRIPTION:   →Whole years, months or days between two dates, and the remainders.¶NOTES!         →Excel's DATEDIF is undocumented and its MD unit can return a negative¶               →or wrong day count around month ends. This counts a month as complete¶               →when EDATE() of the start date has arrived, so a month after 31 January¶               →is the last day of February, and takes every remainder from that same¶               →anniversary. A reversed range or an unknown unit returns #NUM!.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶" &
                            "VERSION:       →6 Sep 2026¶" &
                            "PARAMETERS:    →¶" &
                            "StartDate      →(Required) The earlier date, numeric or text.¶" &
                            "EndDate        →(Required) The later date, numeric or text.¶" &
                            "Unit           →(Optional: Default = D) Y whole years, M whole months, D days, YM months left after the whole years, YD days left after the whole years, MD days left after the whole months.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.DateDifλ(DATE(2026,1,31), DATE(2026,3,1), ""MD"")¶" &
                            "→Result¶" &
                            "→1",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(StartDate), ISOMITTED(EndDate)),
    //  Text dates are read; a blank Unit cell is not an omitted argument, so test for both
        Start,          IF(ISNUMBER(StartDate), StartDate, DATEVALUE(StartDate)),
        Finish,         IF(ISNUMBER(EndDate), EndDate, DATEVALUE(EndDate)),
        Code,           IF(OR(ISOMITTED(Unit), Unit = ""), "D", UPPER(Unit)),
    //  Calendar months crossed, less one when the last has not yet completed
        RawMonths,      12 * (YEAR(Finish) - YEAR(Start)) + MONTH(Finish) - MONTH(Start),
        Months,         RawMonths - (EDATE(Start, RawMonths) > Finish),
        Years,          INT(Months / 12),
        Result,         IF(Finish < Start,
                            #NUM!,
                            SWITCH(Code,
                                "Y", Years,
                                "M", Months,
                                "D", Finish - Start,
                                "YM", Months - 12 * Years,
                                "YD", Finish - EDATE(Start, 12 * Years),
                                "MD", Finish - EDATE(Start, Months),
                                #NUM!)),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


# module -> (the About-table row the additions follow, a heading to add first or
# None, and the width of the table's label column)
ABOUT: dict[str, tuple[str, str | None, int]] = {
    "Financial": (
        '"ROUScheduleλ       →Depreciates a right-of-use asset in a straight line over the lease term¶"',
        "RATES              →",
        19,
    ),
    # Before FinancialYearλ, not after it: help_corrections.py recognises its own
    # AboutDatesλ swap by the row that follows the FinancialYearλ line.
    "Dates": (
        '"Timelineλ              →Creates a horizontal list of start or end dates for a timeline¶"',
        None,
        23,
    ),
}

# module, name, About-table description, source block
FUNCTIONS: list[tuple[str, str, str, str]] = [
    (
        "Financial",
        "PeriodRateλ",
        "Converts an effective annual rate to the equivalent rate per period",
        PERIOD_RATE_SRC,
    ),
    (
        "Financial",
        "AnnualRateλ",
        "Converts a rate per period to the effective annual rate",
        ANNUAL_RATE_SRC,
    ),
    (
        "Financial",
        "DayCountRateλ",
        "Interest rate for each timeline period under a day count convention",
        DAY_COUNT_RATE_SRC,
    ),
    (
        "Dates",
        "DateDifλ",
        "Whole years, months or days between two dates, and the remainders",
        DATE_DIF_SRC,
    ),
]

MODULES = list(ABOUT)
QUALIFIED = [f"{NAMESPACE}.{name}" for _module, name, _about, _src in FUNCTIONS]

# No raw & < > may reach xl/workbook.xml as text, so the help must not contain them.
# Checked with raise rather than assert, so python -O cannot skip it.
for _module, _name, _about, _block in FUNCTIONS:
    for _bad in ("&amp;", "&lt;", "&gt;"):
        if _bad in _block:
            raise ValueError(f"{_name}: {_bad} is already escaped in the source")
    if "\t" in _block:
        raise ValueError(f"{_name}: tabs do not belong in the source")
    if f"\n{_name} = LAMBDA(" not in _block:
        raise ValueError(f"{_name}: the source block does not declare it")


def module_functions(module: str) -> list[tuple[str, str, str]]:
    """(name, About description, source block) for one module, in order."""
    return [(n, a, s) for m, n, a, s in FUNCTIONS if m == module]


def about_row(module: str, name: str, about: str) -> str:
    width = ABOUT[module][2]
    return f'"{name:<{width}}→{about}¶"'


def src_state(module: str, text: str) -> str:
    """"absent", "applied", or an error when a module holds only part of the change."""
    functions = module_functions(module)
    present = sum(1 for name, _about, _src in functions if f"\n{name} = LAMBDA(" in text)
    rows = sum(1 for name, about, _src in functions if about_row(module, name, about) in text)
    if present == 0 and rows == 0:
        return "absent"
    if present == len(functions) and rows == len(functions):
        return "applied"
    raise ValueError(
        f"src/{module}.txt holds {present} of {len(functions)} functions and {rows} "
        f"About rows; it is neither state this pass recognises"
    )


def workbook_state(book: str) -> str:
    present = sum(1 for name in QUALIFIED if f'<definedName name="{name}"' in book)
    if present == 0:
        return "absent"
    if present == len(QUALIFIED):
        return "applied"
    raise ValueError(
        f"xl/workbook.xml holds {present} of {len(QUALIFIED)} defined names; it is "
        f"neither state this pass recognises"
    )


def add_to_src(module: str, text: str) -> str:
    """The module with its About rows inserted after the anchor and its blocks appended."""
    anchor, heading, _width = ABOUT[module]
    hits = text.count(anchor)
    if hits != 1:
        raise ValueError(f"src/{module}.txt: About anchor found {hits} times, expected 1")
    line_start = text.rfind("\n", 0, text.index(anchor)) + 1
    indent = text[line_start : text.index(anchor)]
    # A heading gets the blank spacer row the older suites carry above them.
    rows = [f'{indent}"→¶" &', f'{indent}"{heading}¶" &'] if heading else []
    rows.extend(f"{indent}{about_row(module, name, about)} &" for name, about, _src in module_functions(module))
    text = text.replace(anchor + " &", anchor + " &\n" + "\n".join(rows), 1)

    blocks = "\n\n\n" + "\n\n\n".join(src for _name, _about, src in module_functions(module)) + "\n"
    if not text.endswith("\n"):
        text += "\n"
    return text + blocks


def insert_names(book: str, compiled: list[Compiled]) -> str:
    """Each new defined name after its case-insensitive alphabetical predecessor."""
    existing = sorted(re.findall(r'<definedName name="(oz\.[^"]+)"', book), key=str.lower)
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


def write_text(path: Path, text: str) -> None:
    with open(path, "w", encoding="utf-8", newline="") as handle:
        handle.write(text)


def run(workbook: Path, src: Path, index: Path | None) -> list[str]:
    texts: dict[str, str] = {}
    for module in MODULES:
        path = src / f"{module}.txt"
        if not path.is_file():
            raise ValueError(f"src: missing {path}")
        texts[module] = read_text(path)

    with zipfile.ZipFile(workbook) as archive:
        parts = {name: archive.read(name) for name in archive.namelist()}
    book = parts["xl/workbook.xml"].decode("utf-8")

    states = {module: src_state(module, text) for module, text in texts.items()}
    states["workbook"] = workbook_state(book)
    if len(set(states.values())) != 1:
        raise ValueError(
            "the stores disagree (%s); the views must not be applied separately"
            % ", ".join(f"{store} {state}" for store, state in states.items())
        )
    if states["workbook"] == "applied":
        return []

    changed: list[str] = []
    for module in MODULES:
        write_text(src / f"{module}.txt", add_to_src(module, texts[module]))
        changed.append(f"src/{module}.txt")

    # Rendered from the published source and proven against it, the way every
    # compiled definition is; then the About tables catch up through the same tool.
    # The compiler reads src/ from disk, so src/ is written first and the index next;
    # the workbook, which is what marks the pass applied, is written last. If anything
    # before that fails, src/ and the index go back to what they were rather than
    # staying half-applied.
    index_before = index.read_bytes() if index is not None and index.is_file() else None
    indexed = False
    try:
        compiled = compile_sources(src)
        book = insert_names(book, [c for c in compiled if c.name in QUALIFIED])
        book, _recompiled = apply(book, compiled)
        if index_before is not None and index is not None:
            indexed = update_index(index, book, {c.name: c.module for c in compiled})
        elif index is not None:
            print(f"note: {index} not found, index not updated")
        parts["xl/workbook.xml"] = book.encode("utf-8")
        write_deterministic(workbook, parts)
    except Exception:
        for module in MODULES:
            write_text(src / f"{module}.txt", texts[module])
        if index_before is not None and index is not None:
            index.write_bytes(index_before)
        raise
    changed.append("workbook")
    if indexed and index is not None:
        changed.append(index.name)
    return changed


def main() -> None:
    if len(sys.argv) > 4:
        sys.exit(
            "FAIL: usage: python tools/postbuild/rate_date_helpers.py "
            "[workbook] [src dir] [functions.csv]"
        )
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    src = Path(sys.argv[2] if len(sys.argv) > 2 else "src")
    index = Path(sys.argv[3]) if len(sys.argv) > 3 else workbook.parent / "functions.csv"
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changed = run(workbook, src, index)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    if changed:
        print(f"added the rate and date helpers to: {', '.join(changed)}")
    else:
        print("rate and date helpers already applied; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
