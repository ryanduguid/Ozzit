"""ATO benchmark ratios and a dated rate lookup: 6 functions added in October 2026.

Usage: python tools/postbuild/ato_benchmarks_rate_lookup.py [workbook] [src dir] [functions.csv]

This pass starts from the committed ozzit.xlsx and src/. It adds 5 functions to the
Ratios group (oz.ATOTurnoverλ, oz.ATOBenchmarkRatioλ, oz.ATOTotalExpensesRatioλ,
oz.ATOCostOfSalesRatioλ and oz.ATOLabourRatioλ) and oz.RateAtDateλ to the Dates group,
in every store at once: the src module, its About table, the defined names in
xl/workbook.xml and functions.csv. The Advanced Formula Environment store is not touched
here; tools/sync_afe_store.py copies src/ into it afterwards and verify_afe gates it.

The ratio functions restate the tax return benchmark ratios in the ATO's "How we
calculate benchmark ratios" page (last updated 16 March 2026, read 19 October 2026),
re-derived from its text: each is an amount divided by turnover excluding GST, with the
turnover fallback and the wages, associate-payment and W1 adjustments that page states.
They return a fraction, as every Ozzit ratio does; the ATO prints the same quotient
multiplied by 100. The activity statement ratios are left out: the ATO has not produced
them since 1 July 2017. No benchmark range or rate ships with them.

It follows tools/postbuild/rate_date_helpers.py, which added 4 functions across 2
modules the same way. The stored form of each definition is rendered by
tools/compile_sources.py from the published source, and that tool's own round-trip
through verify_sources.py's comparison proves the rendering before anything is written.

Every insertion carries an asserted count. A second run reports "already applied" and
writes nothing. A store that already holds some of the six but not all of them fails
loudly rather than leaving the views out of step.

Pure text surgery: no COM, no recalculation. No worksheet is added, but both About
tables' help changes, so a cached spill of either needs tools/refresh_cache.py in Excel.
"""

from __future__ import annotations

import csv
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compile_sources import (  # noqa: E402
    apply,
    compile_sources,
    insert_names,
    update_index,
)
from sanitise_workbook import read_text, write_deterministic, write_text  # noqa: E402
from workbook import read_book, read_parts  # noqa: E402
from workbook import workbook_state as book_state  # noqa: E402

NAMESPACE = "oz"
ATO_PAGE = (
    "https://www.ato.gov.au/businesses-and-organisations/income-deductions-and-concessions/"
    "small-business-benchmarks/small-business-benchmarks-methodology-and-ratio-calculations/"
    "how-we-calculate-benchmark-ratios"
)


ATO_TURNOVER_SRC = '''/*  FUNCTION NAME:  ATOTurnoverλ
    DESCRIPTION:*//**Turnover as the ATO small business benchmarks measure it: other sales unless under half of total business income*/

ATOTurnoverλ = LAMBDA(
//  Parameter Declaration
    [OtherSales],
    [TotalBusinessIncome],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →ATOTurnoverλ(OtherSales, TotalBusinessIncome)¶" &
                            "DESCRIPTION:   →Turnover excluding GST, the denominator of every ATO tax return benchmark ratio.¶NOTES!         →Returns OtherSales, unless it is zero, blank or less than half of¶               →TotalBusinessIncome, when it returns TotalBusinessIncome instead. That is¶               →the ATO's rule. OtherSales is the other sales of goods and services¶               →amount on the return. Where total business income is blank or¶               →incomplete, the ATO adds up the business income items it lists for the¶               →return type: pass that total as TotalBusinessIncome. Pass both amounts¶               →excluding GST. Text in either argument returns #VALUE!. Arithmetic only:¶               →the reviewer decides what belongs in each amount. ATO benchmark ratio¶               →calculations checked 19 October 2026.¶" &
                            "WEBPAGE:       →__ATO_PAGE__¶" &
                            "VERSION:       →19 Oct 2026¶" &
                            "PARAMETERS:    →¶" &
                            "OtherSales     →(Required) Other sales of goods and services, excluding GST.¶" &
                            "TotalBusinessIncome→(Required) Total business income, excluding GST.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.ATOTurnoverλ(480000, 500000)¶" &
                            "→Result¶" &
                            "→480000¶" &
                            "→=oz.ATOTurnoverλ(200000, 500000)¶" &
                            "→Result¶" &
                            "→500000",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(OtherSales), ISOMITTED(TotalBusinessIncome)),
    //  Procedure
    //  A blank OtherSales cell or an empty string is nil, row by row, which the rule
    //  replaces with total business income; any other text is #VALUE!
        Sales,          IF(OtherSales = "", 0, IF(ISNUMBER(OtherSales), OtherSales, #VALUE!)),
        Income,         IF(ISNUMBER(TotalBusinessIncome), TotalBusinessIncome, #VALUE!),
        Result,         IF((Sales = 0) + (Sales < Income / 2), Income, Sales),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


ATO_BENCHMARK_RATIO_SRC = '''/*  FUNCTION NAME:  ATOBenchmarkRatioλ
    DESCRIPTION:*//**ATO tax return benchmark ratio: an amount divided by turnover, as for rent or motor vehicle expenses to turnover*/

ATOBenchmarkRatioλ = LAMBDA(
//  Parameter Declaration
    [Amount],
    [Turnover],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →ATOBenchmarkRatioλ(Amount, Turnover)¶" &
                            "DESCRIPTION:   →ATO tax return benchmark ratio: Amount / Turnover.¶NOTES!         →The ATO's basic formula for a tax return benchmark ratio, and the whole¶               →of its rent expenses to turnover and motor vehicle expenses to turnover¶               →ratios: pass the rent or motor vehicle expenses amount from the return.¶               →Returns a fraction; format it as a percentage, as the ATO's figure is the¶               →same quotient multiplied by 100. Take Turnover from ATOTurnoverλ(). Use¶               →ATOTotalExpensesRatioλ(), ATOCostOfSalesRatioλ() and ATOLabourRatioλ()¶               →for the 3 ratios whose amounts the ATO adjusts. Turnover of 0 returns¶               →#DIV/0!. Arithmetic only. ATO benchmark ratio calculations checked¶               →19 October 2026.¶" &
                            "WEBPAGE:       →__ATO_PAGE__¶" &
                            "VERSION:       →19 Oct 2026¶" &
                            "PARAMETERS:    →¶" &
                            "Amount         →(Required) The expense amount, excluding GST.¶" &
                            "Turnover       →(Required) Turnover excluding GST, for example from ATOTurnoverλ().¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.ATOBenchmarkRatioλ(24000, 480000)¶" &
                            "→Result¶" &
                            "→5%",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(Amount), ISOMITTED(Turnover)),
    //  Procedure
        Result,         Amount / Turnover,
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


ATO_TOTAL_EXPENSES_RATIO_SRC = '''/*  FUNCTION NAME:  ATOTotalExpensesRatioλ
    DESCRIPTION:*//**ATO total expenses to turnover: total expenses less payments to associated persons, over turnover*/

ATOTotalExpensesRatioλ = LAMBDA(
//  Parameter Declaration
    [TotalExpenses],
    [Turnover],
    [AssociatePayments],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →ATOTotalExpensesRatioλ(TotalExpenses, Turnover, [AssociatePayments])¶" &
                            "DESCRIPTION:   →ATO total expenses to turnover benchmark ratio.¶NOTES!         →(TotalExpenses - AssociatePayments) / Turnover. The ATO deducts payments¶               →to associated persons from total expenses before dividing. Where total¶               →expenses is blank or incomplete, it adds up the expense items it lists¶               →for the return type: pass that total as TotalExpenses. OperatingRatioλ()¶               →gives the same quotient only when its expenses already exclude those¶               →payments and its net sales equal this Turnover. Returns a fraction, as¶               →ATOBenchmarkRatioλ() does. An omitted or blank AssociatePayments counts¶               →as 0. Turnover of 0 returns #DIV/0!. Arithmetic only. ATO benchmark ratio¶               →calculations checked 19 October 2026.¶" &
                            "WEBPAGE:       →__ATO_PAGE__¶" &
                            "VERSION:       →19 Oct 2026¶" &
                            "PARAMETERS:    →¶" &
                            "TotalExpenses  →(Required) Total expenses from the return, excluding GST.¶" &
                            "Turnover       →(Required) Turnover excluding GST, for example from ATOTurnoverλ().¶" &
                            "AssociatePayments→(Optional: Default = 0) Payments to associated persons.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.ATOTotalExpensesRatioλ(410000, 480000, 30000)¶" &
                            "→Result¶" &
                            "→0.7917",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(TotalExpenses), ISOMITTED(Turnover)),
    //  Procedure
    //  An omitted AssociatePayments, a blank cell or an empty string is 0, row by row;
    //  any other text is #VALUE!
        Associates,     IF(ISOMITTED(AssociatePayments), 0,
                            IF(AssociatePayments = "", 0,
                                IF(ISNUMBER(AssociatePayments), AssociatePayments, #VALUE!))),
        Result,         (TotalExpenses - Associates) / Turnover,
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


ATO_COST_OF_SALES_RATIO_SRC = '''/*  FUNCTION NAME:  ATOCostOfSalesRatioλ
    DESCRIPTION:*//**ATO cost of sales to turnover, excluding salary and wages included in cost of sales*/

ATOCostOfSalesRatioλ = LAMBDA(
//  Parameter Declaration
    [CostOfSales],
    [Turnover],
    [IncludedWages],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →ATOCostOfSalesRatioλ(CostOfSales, Turnover, [IncludedWages])¶" &
                            "DESCRIPTION:   →ATO cost of sales to turnover benchmark ratio, excluding labour.¶NOTES!         →(CostOfSales - IncludedWages) / Turnover. The ATO's cost of sales excludes¶               →salary and wages: where the return's total salary and wages code is C,¶               →meaning they are included in cost of sales, it deducts the total salary¶               →and wages expenses. Pass that total as IncludedWages only when the code is¶               →C, and leave it out otherwise. GrossMarginλ() is a different measure:¶               →it keeps wages in cost of sales. Returns a fraction, as¶               →ATOBenchmarkRatioλ() does. An omitted or blank IncludedWages counts as 0.¶               →Turnover of 0 returns #DIV/0!. Arithmetic only. ATO benchmark ratio¶               →calculations checked 19 October 2026.¶" &
                            "WEBPAGE:       →__ATO_PAGE__¶" &
                            "VERSION:       →19 Oct 2026¶" &
                            "PARAMETERS:    →¶" &
                            "CostOfSales    →(Required) Cost of sales from the return, excluding GST.¶" &
                            "Turnover       →(Required) Turnover excluding GST, for example from ATOTurnoverλ().¶" &
                            "IncludedWages  →(Optional: Default = 0) Total salary and wages expenses, only when the return's code is C.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.ATOCostOfSalesRatioλ(260000, 480000, 60000)¶" &
                            "→Result¶" &
                            "→0.4167",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(CostOfSales), ISOMITTED(Turnover)),
    //  Procedure
    //  An omitted IncludedWages, a blank cell or an empty string is 0, row by row;
    //  any other text is #VALUE!
        Wages,          IF(ISOMITTED(IncludedWages), 0,
                            IF(IncludedWages = "", 0,
                                IF(ISNUMBER(IncludedWages), IncludedWages, #VALUE!))),
        Result,         (CostOfSales - Wages) / Turnover,
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


ATO_LABOUR_RATIO_SRC = '''/*  FUNCTION NAME:  ATOLabourRatioλ
    DESCRIPTION:*//**ATO labour to turnover: salary and wages plus contractors less payments to associated persons, over turnover*/

ATOLabourRatioλ = LAMBDA(
//  Parameter Declaration
    [SalaryWages],
    [Contractors],
    [Turnover],
    [AssociatePayments],
    [W1],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →ATOLabourRatioλ(SalaryWages, Contractors, Turnover, [AssociatePayments], [W1])¶" &
                            "DESCRIPTION:   →ATO labour to turnover benchmark ratio.¶NOTES!         →(Wages + Contractors - AssociatePayments) / Turnover, where Wages is¶               →SalaryWages, or W1 when W1 is greater. The ATO adds total salary and¶               →wages expenses to contractor, subcontractor and commission expenses and¶               →deducts payments to associated persons. Where the activity statements'¶               →label W1 (total salary, wages and other payments) for the year exceeds¶               →the return's salary and wages, it uses W1. Returns a fraction, as¶               →ATOBenchmarkRatioλ() does. An omitted or blank AssociatePayments or W1¶               →counts as 0. Turnover of 0 returns #DIV/0!. Arithmetic only. ATO benchmark¶               →ratio calculations checked 19 October 2026.¶" &
                            "WEBPAGE:       →__ATO_PAGE__¶" &
                            "VERSION:       →19 Oct 2026¶" &
                            "PARAMETERS:    →¶" &
                            "SalaryWages    →(Required) Total salary and wages expenses from the return.¶" &
                            "Contractors    →(Required) Contractor, subcontractor and commission expenses.¶" &
                            "Turnover       →(Required) Turnover excluding GST, for example from ATOTurnoverλ().¶" &
                            "AssociatePayments→(Optional: Default = 0) Payments to associated persons.¶" &
                            "W1             →(Optional: Default = 0) The year's total at activity statement label W1.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.ATOLabourRatioλ(90000, 20000, 480000, 10000, 95000)¶" &
                            "→Result¶" &
                            "→0.21875",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(SalaryWages), ISOMITTED(Contractors), ISOMITTED(Turnover)),
    //  Procedure
    //  An omitted optional argument, a blank cell or an empty string is 0, row by row;
    //  any other text is #VALUE!
        Associates,     IF(ISOMITTED(AssociatePayments), 0,
                            IF(AssociatePayments = "", 0,
                                IF(ISNUMBER(AssociatePayments), AssociatePayments, #VALUE!))),
        Activity,       IF(ISOMITTED(W1), 0, IF(W1 = "", 0, IF(ISNUMBER(W1), W1, #VALUE!))),
    //  The larger of the 2 wage figures, row by row: MAX() would reduce a column to one
        Wages,          IF(Activity > SalaryWages, Activity, SalaryWages),
        Result,         (Wages + Contractors - Associates) / Turnover,
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


RATE_AT_DATE_SRC = '''/*  FUNCTION NAME:  RateAtDateλ
    DESCRIPTION:*//**Returns the rate in effect on each date from a table of effective dates and rates, refusing gaps, unsorted or repeated dates*/

RateAtDateλ = LAMBDA(
//  Parameter Declaration
    [EffectiveDates],
    [Rates],
    [Dates],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →RateAtDateλ(EffectiveDates, Rates, Dates)¶" &
                            "DESCRIPTION:   →Returns the rate in effect on each date from your table of effective dates and rates.¶NOTES!         →Each rate applies from its effective date until the next one. Dates may¶               →be one date or a row or column of them, and the result takes their shape.¶               →A date before the first effective date has no rate in effect and returns¶               →#N/A. The table must be complete and in order, so instead of a result it¶               →returns a message when EffectiveDates and Rates differ in length or hold¶               →a blank or text entry, when the dates are not in ascending order, or when¶               →a date repeats. A text or blank date in Dates returns #VALUE!. The¶               →library ships no rate table: supply your own, with its source and the¶               →date you checked it. ScheduleRatesλ() lays rates across a timeline¶               →without these checks.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶" &
                            "VERSION:       →19 Oct 2026¶" &
                            "PARAMETERS:    →¶" &
                            "EffectiveDates →(Required) A row or column of dates from which each rate applies, in ascending order.¶" &
                            "Rates          →(Required) The rate that applies from each effective date, the same length as EffectiveDates.¶" &
                            "Dates          →(Required) The date, or a row or column of dates, to look up.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.RateAtDateλ(DATE(2026, {1;7;10}, 1), {0.04;0.045;0.05}, DATE(2026, 8, 15))¶" &
                            "→Result¶" &
                            "→0.045",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(EffectiveDates), ISOMITTED(Rates), ISOMITTED(Dates)),
    //  The table laid down one column, so a row and a column read the same
        Starts,         TOCOL(EffectiveDates),
        Values,         TOCOL(Rates),
        Count,          ROWS(Starts),
    //  Read on the arguments, not the columns: TOCOL turns a blank cell into 0, which
    //  would pass as a date in 1900 or a nil rate
        Complete?,      AND(Count = ROWS(Values), ISNUMBER(EffectiveDates), ISNUMBER(Rates)),
        Steps,          IF(Count = 1, 1, DROP(Starts, 1) - DROP(Starts, -1)),
        Unsorted?,      OR(Steps < 0),
        Repeated?,      OR(Steps = 0),
        GapMessage,     "RateAtDateλ needs EffectiveDates and Rates of the same length with a number "
                        & "in every entry. A blank or text entry leaves a gap in the table.",
        SortMessage,    "RateAtDateλ needs EffectiveDates in ascending order. Sort the table by "
                        & "effective date.",
        RepeatMessage,  "RateAtDateλ found an effective date that repeats. Keep one rate for each "
                        & "date.",
    //  Binary search is safe once the order is proven. No date at or before the lookup
    //  date means no rate was yet in effect: #N/A.
        Result,         IF(ISNUMBER(Dates), XLOOKUP(Dates, Starts, Values, NA(), -1, 2), #VALUE!),
    //  Return Result, Help, or the message
        Return,         IF(Help?, 2, IF(NOT(Complete?), 3, IF(Unsorted?, 4, IF(Repeated?, 5, 1)))),
        CHOOSE( Return, Result, Help, GapMessage, SortMessage, RepeatMessage)
    )
);'''


# module -> (the About-table row the additions follow, a heading to add first or None,
# the width of the table's label column, and the function block the additions precede)
ABOUT: dict[str, tuple[str, str | None, int, str]] = {
    # After the last market value row, so the new group sits above the Key Ratios note.
    "Ratios": (
        '"RetentionRatioλ            →Retention Ratio is the proportion of earnings kept back '
        "in the business as retained earnings¶\"",
        "ATO Small Business Benchmark Ratios→",
        27,
        "/*  FUNCTION NAME:  ReceivableDaysλ",
    ),
    # Beside the other rate schedulers. help_corrections.py recognises its own AboutDatesλ
    # swap by the rows around FinancialYearλ, which this leaves alone.
    "Dates": (
        '"ScheduleRatesByItemsλ  →Schedule rates that persist until replaced in a timeline '
        'for each item in a list.¶"',
        None,
        23,
        "/*  FUNCTION NAME:  DateDifλ",
    ),
}

# module, name, About-table description, source block
FUNCTIONS: list[tuple[str, str, str, str]] = [
    (
        "Ratios",
        "ATOTurnoverλ",
        "Turnover excluding GST as the ATO benchmarks measure it",
        ATO_TURNOVER_SRC,
    ),
    (
        "Ratios",
        "ATOBenchmarkRatioλ",
        "An amount over turnover, as for ATO rent or motor vehicle expenses to turnover",
        ATO_BENCHMARK_RATIO_SRC,
    ),
    (
        "Ratios",
        "ATOTotalExpensesRatioλ",
        "ATO total expenses to turnover, less payments to associated persons",
        ATO_TOTAL_EXPENSES_RATIO_SRC,
    ),
    (
        "Ratios",
        "ATOCostOfSalesRatioλ",
        "ATO cost of sales to turnover, excluding labour",
        ATO_COST_OF_SALES_RATIO_SRC,
    ),
    (
        "Ratios",
        "ATOLabourRatioλ",
        "ATO labour to turnover: wages and contractors less payments to associated persons",
        ATO_LABOUR_RATIO_SRC,
    ),
    (
        "Dates",
        "RateAtDateλ",
        "Return the rate in effect on each date from a checked table of effective dates",
        RATE_AT_DATE_SRC,
    ),
]

FUNCTIONS = [
    (module, name, about, block.replace("__ATO_PAGE__", ATO_PAGE))
    for module, name, about, block in FUNCTIONS
]
MODULES = list(ABOUT)
QUALIFIED = [f"{NAMESPACE}.{name}" for _module, name, _about, _src in FUNCTIONS]

# No raw & < > may reach xl/workbook.xml as text, so the help must not contain escaped
# forms either. Checked with raise rather than assert, so python -O cannot skip it.
for _module, _name, _about, _block in FUNCTIONS:
    for _bad in ("&amp;", "&lt;", "&gt;", "__ATO_PAGE__"):
        if _bad in _block:
            raise ValueError(f"{_name}: {_bad} does not belong in the source")
    if "\t" in _block:
        raise ValueError(f"{_name}: tabs do not belong in the source")
    if f"\n{_name} = LAMBDA(" not in _block:
        raise ValueError(f"{_name}: the source block does not declare it")


def module_functions(module: str) -> list[tuple[str, str, str]]:
    return [(n, a, s) for m, n, a, s in FUNCTIONS if m == module]


def about_row(module: str, name: str, about: str) -> str:
    width = ABOUT[module][2]
    return f'"{name:<{width}}→{about}¶"'


def src_state(module: str, text: str) -> str:
    """"absent", "applied", or an error when the module holds only part of the change."""
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


def index_state(index: Path | None) -> str | None:
    """"absent", "applied", or None when no index is supplied, raising on a partial one.

    A supplied index that does not exist is an error, not "no index", as in
    working_capital_days.py: otherwise the pass could write src/ and the workbook and
    exit 0 with the index left behind.
    """
    if index is None:
        return None
    if not index.is_file():
        raise ValueError(f"{index}: no such index; supply an existing functions.csv or none")
    # Build tool; the path is the repository's own file or the maintainer's argument.
    with index.open(encoding="utf-8-sig", newline="") as handle:  # NOSONAR
        listed = {row.get("function", "") for row in csv.DictReader(handle)}
    present = sum(1 for name in QUALIFIED if name in listed)
    if present == 0:
        return "absent"
    if present == len(QUALIFIED):
        return "applied"
    raise ValueError(
        f"{index.name} holds {present} of {len(QUALIFIED)} benchmark and rate rows; "
        "it is neither state this pass recognises"
    )


def add_to_src(module: str, text: str) -> str:
    """The module with its About rows after the anchor and its blocks before the follower."""
    anchor, heading, _width, follower = ABOUT[module]
    hits = text.count(anchor)
    if hits != 1:
        raise ValueError(f"src/{module}.txt: About anchor found {hits} times, expected 1")
    line_start = text.rfind("\n", 0, text.index(anchor)) + 1
    indent = text[line_start : text.index(anchor)]
    # A heading gets the blank spacer row the older groups carry above them.
    rows = [f'{indent}"→¶" &', f'{indent}"{heading}¶" &'] if heading else []
    rows.extend(
        f"{indent}{about_row(module, name, about)} &"
        for name, about, _src in module_functions(module)
    )
    text = text.replace(anchor + " &", anchor + " &\n" + "\n".join(rows), 1)

    # The blocks go before the first one an earlier pass appended (working_capital_days.py
    # in Ratios, rate_date_helpers.py in Dates), followed by the same run of blank lines
    # that preceded it, so those passes still find their own blocks laid out as they
    # wrote them at the end of the module.
    hits = text.count(follower)
    if hits != 1:
        name = follower.split()[-1]
        raise ValueError(f"src/{module}.txt: {name} block found {hits} times, expected 1")
    blocks = "\n\n\n".join(src for _name, _about, src in module_functions(module))
    return text.replace(follower, blocks + gap_before(text, follower) + follower, 1)


def gap_before(text: str, follower: str) -> str:
    """The newlines between the follower block and the statement before it."""
    start = text.index(follower)
    return text[len(text[:start].rstrip("\n")) : start]


def run(workbook: Path, src: Path, index: Path | None) -> list[str]:
    texts: dict[str, str] = {}
    for module in MODULES:
        path = src / f"{module}.txt"
        if not path.is_file():
            raise ValueError(f"src: missing {path}")
        texts[module] = read_text(path)

    parts = read_parts(workbook)
    book = read_book(parts)

    states = {module: src_state(module, text) for module, text in texts.items()}
    states["workbook"] = book_state(book, QUALIFIED)
    supplied_index = index_state(index)
    if index is not None and supplied_index is not None:
        states[index.name] = supplied_index
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
        raise ValueError("functions.csv is required to apply the benchmark and rate functions")

    # The compiler reads src/ from disk, so src/ is written first and the index next;
    # the workbook, which is what marks the pass applied, is written last. Every write,
    # the first included, is inside the recovery block, so a failure anywhere restores
    # all the stores.
    changed: list[str] = []
    # Build tool; the path is the repository's own file or the maintainer's argument.
    index_before = index.read_bytes()  # NOSONAR
    try:
        for module in MODULES:
            write_text(src / f"{module}.txt", add_to_src(module, texts[module]))
            changed.append(f"src/{module}.txt")
        compiled = compile_sources(src)
        book = insert_names(book, [c for c in compiled if c.name in QUALIFIED])
        book, _recompiled = apply(book, compiled)
        update_index(index, book, {c.name: c.module for c in compiled})
        parts["xl/workbook.xml"] = book.encode("utf-8")
        write_deterministic(workbook, parts)
    except Exception:
        for module in MODULES:
            write_text(src / f"{module}.txt", texts[module])
        # Build tool; the path is the repository's own file or the maintainer's argument.
        index.write_bytes(index_before)  # NOSONAR
        raise
    return [*changed, "workbook", index.name]


def main() -> None:
    if len(sys.argv) > 4:
        sys.exit(
            "FAIL: usage: python tools/postbuild/ato_benchmarks_rate_lookup.py "
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
        print(f"added the benchmark and rate functions to: {', '.join(changed)}")
    else:
        print("benchmark and rate functions already applied; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
