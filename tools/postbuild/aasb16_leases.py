"""AASB 16 lease pass: four lessee functions added to the Financial group.

Usage: python tools/postbuild/aasb16_leases.py [workbook] [src dir] [functions.csv]

This pass starts from the committed ozzit.xlsx and src/ (the post-v3.0.0 input
recorded in ATTRIBUTION.md). It does not read the predecessor workbook and does not
go through transform_from_predecessor.py.

It adds oz.LeaseLiabilityλ, oz.LeaseScheduleλ, oz.ROUScheduleλ and
oz.LeaseRemeasureλ to four stores at once: src/Financial.txt, the defined names
in xl/workbook.xml, the AboutFinancialλ function table in both of those, and
functions.csv. The Advanced Formula Environment store is not touched here;
tools/sync_afe_store.py copies src/ into it afterwards and verify_afe gates it.

One spec per function generates both stores. The stored form is derived from the
published source rather than written beside it, because the two hand-written
lists drifted the last time this library kept them in parallel. Every generated
definition is then round-tripped through the comparison verify_sources.py itself
uses, before anything is written.

Every insertion carries an asserted count. A second run reports "already applied"
and writes nothing. A store that already holds some of the four but not all of
them fails loudly rather than leaving the views out of sync.

Pure text surgery: no COM, no recalculation. No worksheet is added, so no cached
value moves and verify_cache.py is unaffected.
"""

from __future__ import annotations

import csv
import re
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from sanitise_workbook import write_deterministic  # noqa: E402
from verify_sources import canonical, qualify, statements, NAME  # noqa: E402

MODULE = "Financial"
NAMESPACE = "oz"

# Post-2007 functions carry a marker in the stored form. This is the set the
# library already ships, narrowed to the ones these four definitions call.
XLFN = frozenset(
    {"LAMBDA", "LET", "ISOMITTED", "TEXTSPLIT", "SEQUENCE", "SCAN", "HSTACK", "VSTACK", "DROP"}
)

# The About table row each function adds, and the heading they sit under.
ABOUT_HEADING = "LEASES (AASB 16)   →"
ABOUT_ANCHOR = (
    '"GSTExtractλ        →Returns the GST contained in one or more '
    'GST-inclusive amounts.¶"'
)


LEASE_LIABILITY_SRC = '''/*  FUNCTION NAME:  LeaseLiabilityλ
    DESCRIPTION:*//**Present value of the lease payments that are not paid at the commencement date*/

LeaseLiabilityλ = LAMBDA(
//  Parameter Declaration
    [Payments],
    [Rate],
    [InAdvance],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →LeaseLiabilityλ(Payments, Rate, [InAdvance])¶" &
                            "DESCRIPTION:   →Present value of the lease payments that are not paid at the commencement date.¶NOTES!         →AASB 16 paragraph 26 measures the lease liability at the present value¶               →of the lease payments not paid at the commencement date, discounted at¶               →the interest rate implicit in the lease where that rate can be readily¶               →determined and at the lessee's incremental borrowing rate where it¶               →cannot. When InAdvance is TRUE, the first supplied payment is paid at¶               →the measurement date and excluded from the liability. Add a payment¶               →made at or before commencement to the right-of-use asset cost instead.¶               →Rate is the rate per period, so divide an annual rate by the number of¶               →periods in a year. Include an amount expected to be payable under a¶               →residual value guarantee, or the exercise price of a purchase option,¶               →in the final period where paragraph 27 brings it into the lease¶               →payments. Derive an implicit rate with IRRλ() over the same cash flows.¶               →Arithmetic only: this does not determine the lease term, decide what is¶               →a lease payment, or decide whether the arrangement contains a lease.¶               →AASB 16 compilation checked 24 August 2026.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶" &
                            "VERSION:       →24 Aug 2026¶" &
                            "PARAMETERS:    →¶" &
                            "Payments       →(Required) One row or one column of lease payments, one per period, entered positive. Include the measurement-date payment first when InAdvance is TRUE.¶" &
                            "Rate           →(Required) Discount rate per period, not per year.¶" &
                            "InAdvance      →(Optional: Default = FALSE) TRUE when the first supplied payment is paid at the measurement date.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.LeaseLiabilityλ({100,100,100}, 0.05)¶" &
                            "→Result¶" &
                            "→272.32",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(Payments), ISOMITTED(Rate)),
    //  A blank InAdvance cell is not an omitted argument, so test for both
        Advance?,       IF(OR(ISOMITTED(InAdvance), TRIM(InAdvance & "")=""), FALSE, InAdvance),
    //  One lease at a time, laid along a row the way the timeline helpers expect
        Flat,           IF(ROWS(Payments) = 1, Payments, TRANSPOSE(Payments)),
        Count,          COLUMNS(Flat),
    //  The first in-advance payment is made at the measurement date, not left unpaid
        LiabilityPayments, IF(AND(Advance?, Count > 1),
                                DROP(Flat, , 1),
                                IF(Advance?, 0, Flat)),
        Exponents,      SEQUENCE(1, COLUMNS(LiabilityPayments)),
        Result,         SUM(LiabilityPayments / (1 + Rate) ^ Exponents),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


LEASE_SCHEDULE_SRC = '''/*  FUNCTION NAME:  LeaseScheduleλ
    DESCRIPTION:*//**Unwinds an AASB 16 lease liability: opening, payment, interest and closing for each period*/

LeaseScheduleλ = LAMBDA(
//  Parameter Declaration
    [Payments],
    [Rate],
    [InAdvance],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →LeaseScheduleλ(Payments, Rate, [InAdvance])¶" &
                            "DESCRIPTION:   →Unwinds an AASB 16 lease liability over its term.¶NOTES!         →Returns four rows: opening liability, payment, interest, closing¶               →liability. When InAdvance is TRUE, the first supplied payment is paid¶               →at the measurement date and is excluded from both the opening liability¶               →and the payment row. Each remaining payment follows one period of¶               →interest, so closing is opening plus interest less payment and the last¶               →closing balance is nil. The result has one fewer column for payments in¶               →advance; a sole measurement-date payment returns one zero column. The¶               →opening balance is LeaseLiabilityλ() on the same arguments. Pair this¶               →with ROUScheduleλ() for the asset that sits opposite it: the two¶               →together give the front-loaded expense profile AASB 16 produces.¶               →Arithmetic only, on the payments and rate you supply.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶" &
                            "VERSION:       →24 Aug 2026¶" &
                            "PARAMETERS:    →¶" &
                            "Payments       →(Required) One row or one column of lease payments, one per period, entered positive. Include the measurement-date payment first when InAdvance is TRUE.¶" &
                            "Rate           →(Required) Discount rate per period, not per year.¶" &
                            "InAdvance      →(Optional: Default = FALSE) TRUE when the first supplied payment is paid at the measurement date.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.LeaseScheduleλ({100,100,100}, 0.05)¶" &
                            "→Result¶" &
                            "→272.32, 185.94,  95.24¶" &
                            "→100.00, 100.00, 100.00¶" &
                            "→ 13.62,   9.30,   4.76¶" &
                            "→185.94,  95.24,   0.00",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(Payments), ISOMITTED(Rate)),
    //  A blank InAdvance cell is not an omitted argument, so test for both
        Advance?,       IF(OR(ISOMITTED(InAdvance), TRIM(InAdvance & "")=""), FALSE, InAdvance),
    //  One lease at a time, laid along a row the way the timeline helpers expect
        Flat,           IF(ROWS(Payments) = 1, Payments, TRANSPOSE(Payments)),
        InputCount,     COLUMNS(Flat),
        LiabilityPayments, IF(AND(Advance?, InputCount > 1),
                                DROP(Flat, , 1),
                                IF(Advance?, 0, Flat)),
        Count,          COLUMNS(LiabilityPayments),
        Opening1,       LeaseLiabilityλ(Flat, Rate, Advance?),
    //  Every payment left in the liability vector follows one period of interest
        Closings,       SCAN(Opening1, LiabilityPayments, LAMBDA(Balance, Payment,
                            Balance * (1 + Rate) - Payment)),
    //  Each opening is the closing before it. A one-period lease has no closing to shift.
        Openings,       IF(Count = 1, Opening1, HSTACK(Opening1, DROP(Closings, , -1))),
        Interest,       Openings * Rate,
        Result,         VSTACK(Openings, LiabilityPayments, Interest, Closings),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


ROU_SCHEDULE_SRC = '''/*  FUNCTION NAME:  ROUScheduleλ
    DESCRIPTION:*//**Straight-line right-of-use asset schedule: opening, depreciation and closing for each period*/

ROUScheduleλ = LAMBDA(
//  Parameter Declaration
    [Cost],
    [Periods],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →ROUScheduleλ(Cost, Periods)¶" &
                            "DESCRIPTION:   →Depreciates a right-of-use asset in a straight line over the lease term.¶NOTES!         →Returns three rows: opening carrying amount, depreciation, closing¶               →carrying amount. AASB 16 paragraph 24 measures the asset at cost,¶               →comprising the initial lease liability, lease payments made at or¶               →before the commencement date less incentives received, initial direct¶               →costs, and an estimate of dismantling and restoration costs. Add those¶               →up yourself and pass the total as Cost. Depreciate over the lease term¶               →where ownership does not transfer, and over the useful life where it¶               →does. Cost and Periods are single values, not arrays. Arithmetic only.¶               →AASB 16 compilation checked 24 August 2026.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶" &
                            "VERSION:       →24 Aug 2026¶" &
                            "PARAMETERS:    →¶" &
                            "Cost           →(Required) Initial carrying amount of the right-of-use asset.¶" &
                            "Periods        →(Required) Number of periods to depreciate over, at least 1.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.ROUScheduleλ(300, 3)¶" &
                            "→Result¶" &
                            "→300, 200, 100¶" &
                            "→100, 100, 100¶" &
                            "→200, 100,   0",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(Cost), ISOMITTED(Periods)),
    //  Refuse rather than build a schedule from an argument that cannot make one
        Valid?,         AND(ISNUMBER(Cost), ISNUMBER(Periods), Periods >= 1),
        Count,          IF(Valid?, INT(Periods), 1),
        Charge,         IF(Valid?, Cost / Count, 0),
        Index,          SEQUENCE(1, Count),
        Result,         IF(Valid?,
                            VSTACK(
                                Cost - Charge * (Index - 1),
                                SEQUENCE(1, Count, Charge, 0),
                                Cost - Charge * Index),
                            "ROUScheduleλ needs a numeric Cost and Periods of at least 1"),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


LEASE_REMEASURE_SRC = '''/*  FUNCTION NAME:  LeaseRemeasureλ
    DESCRIPTION:*//**Remeasures an AASB 16 lease liability for revised payments and adjusts the right-of-use asset*/

LeaseRemeasureλ = LAMBDA(
//  Parameter Declaration
    [RevisedPayments],
    [Rate],
    [CarryingLiability],
    [CarryingROU],
    [InAdvance],
    LET(
    //  Help
        Help,           TRIM(TEXTSPLIT(
                            "FUNCTION:      →LeaseRemeasureλ(RevisedPayments, Rate, CarryingLiability, CarryingROU, [InAdvance])¶" &
                            "DESCRIPTION:   →Remeasures a lease liability for revised payments and adjusts the asset.¶NOTES!         →Returns four rows: revised lease liability, adjustment, revised¶               →right-of-use asset, amount recognised in profit or loss. AASB 16¶               →paragraph 42(b) remeasures the liability where future payments change¶               →because an index or a rate used to determine them has changed, and¶               →only when the cash flows change. Paragraph 43 keeps the discount rate¶               →unchanged for that revision, and uses a revised rate only where the¶               →change arises from a floating interest rate, so pass the original rate¶               →unless it is the rate itself that moved. When InAdvance is TRUE, the¶               →first revised payment is paid at the remeasurement date and excluded¶               →from the revised liability. Paragraph 39 takes the remeasurement to the¶               →right-of-use asset, and where that asset is reduced to nil any further¶               →reduction goes to profit or loss: row four carries that remainder,¶               →negative, because it is a credit. Arithmetic only.¶               →AASB 16 compilation checked 24 August 2026.¶" &
                            "WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶" &
                            "VERSION:       →24 Aug 2026¶" &
                            "PARAMETERS:    →¶" &
                            "RevisedPayments→(Required) Revised lease payments for the remaining term, one per period, positive. Include a payment made at the remeasurement date first when InAdvance is TRUE.¶" &
                            "Rate           →(Required) Discount rate per period. Unchanged, unless a floating rate moved.¶" &
                            "CarryingLiability→(Required) Lease liability carried immediately before the remeasurement.¶" &
                            "CarryingROU    →(Required) Right-of-use asset carried immediately before the remeasurement.¶" &
                            "InAdvance      →(Optional: Default = FALSE) TRUE when the first revised payment is paid at the remeasurement date.¶" &
                            "EXAMPLES:      →¶" &
                            "→Formula (oz is assumed to be the module's name)¶" &
                            "→=oz.LeaseRemeasureλ({110,110}, 0.05, 185.94, 181.55)¶" &
                            "→Result¶" &
                            "→204.54¶" &
                            "→ 18.60¶" &
                            "→200.15¶" &
                            "→  0.00",
                            "→", "¶"
                        )),
    //  Check inputs - Omitted required arguments
        Help?,          OR(ISOMITTED(RevisedPayments), ISOMITTED(Rate),
                            ISOMITTED(CarryingLiability), ISOMITTED(CarryingROU)),
    //  A blank InAdvance cell is not an omitted argument, so test for both
        Advance?,       IF(OR(ISOMITTED(InAdvance), TRIM(InAdvance & "")=""), FALSE, InAdvance),
        Revised,        LeaseLiabilityλ(RevisedPayments, Rate, Advance?),
        Adjustment,     Revised - CarryingLiability,
    //  The asset absorbs the adjustment until it is exhausted, and no further
        RawAsset,       CarryingROU + Adjustment,
        Result,         VSTACK(Revised, Adjustment, MAX(RawAsset, 0), MIN(RawAsset, 0)),
    //  Return Result or Help
        CHOOSE( Help? + 1, Result, Help)
    )
);'''


# name, Name Manager comment (the description functions.csv publishes), source block,
# About-table description.
FUNCTIONS = [
    (
        "LeaseLiabilityλ",
        "Present value of the lease payments that are not paid at the commencement date",
        LEASE_LIABILITY_SRC,
        "Present value of the lease payments that are not paid at the commencement date",
    ),
    (
        "LeaseScheduleλ",
        "Unwinds an AASB 16 lease liability: opening, payment, interest and closing for each period",
        LEASE_SCHEDULE_SRC,
        "Unwinds a lease liability into opening, payment, interest and closing rows",
    ),
    (
        "LeaseRemeasureλ",
        "Remeasures an AASB 16 lease liability for revised payments and adjusts the right-of-use asset",
        LEASE_REMEASURE_SRC,
        "Remeasures a lease liability for revised payments and adjusts the right-of-use asset",
    ),
    (
        "ROUScheduleλ",
        "Straight-line right-of-use asset schedule: opening, depreciation and closing for each period",
        ROU_SCHEDULE_SRC,
        "Depreciates a right-of-use asset in a straight line over the lease term",
    ),
]

NAMES = [name for name, _comment, _src, _about in FUNCTIONS]
QUALIFIED = [f"{NAMESPACE}.{name}" for name in NAMES]

# No raw & < > may reach xl/workbook.xml as text, so the help must not contain them.
for _name, _comment, _block, _about in FUNCTIONS:
    for _bad in ("&amp;", "&lt;", "&gt;"):
        assert _bad not in _block, f"{_name}: {_bad} is already escaped in the source"
    assert "\t" not in _block, f"{_name}: tabs do not belong in the source"


# --------------------------------------------------------------------------- #
# Stored form
# --------------------------------------------------------------------------- #


def _split_literals(text: str) -> list[tuple[bool, str]]:
    """Alternating (is_string, chunk) pairs, honouring the "" escape."""
    out: list[tuple[bool, str]] = []
    buf: list[str] = []
    i, n = 0, len(text)
    while i < n:
        if text[i] == '"':
            if buf:
                out.append((False, "".join(buf)))
                buf = []
            lit = ['"']
            i += 1
            while i < n:
                if text[i] == '"':
                    if i + 1 < n and text[i + 1] == '"':
                        lit.append('""')
                        i += 2
                        continue
                    lit.append('"')
                    i += 1
                    break
                lit.append(text[i])
                i += 1
            out.append((True, "".join(lit)))
            continue
        buf.append(text[i])
        i += 1
    if buf:
        out.append((False, "".join(buf)))
    return out


def _strip_comments(text: str) -> str:
    out, i, n = [], 0, len(text)
    while i < n:
        if text[i] == '"':
            lit, i = _read_string(text, i)
            out.append(lit)
            continue
        if text[i] == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                i += 1
            continue
        if text[i] == "/" and i + 1 < n and text[i + 1] == "*":
            end = text.find("*/", i + 2)
            i = end + 2 if end != -1 else n
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def _read_string(text: str, i: int) -> tuple[str, int]:
    lit, n = ['"'], len(text)
    i += 1
    while i < n:
        if text[i] == '"':
            if i + 1 < n and text[i + 1] == '"':
                lit.append('""')
                i += 2
                continue
            lit.append('"')
            i += 1
            break
        lit.append(text[i])
        i += 1
    return "".join(lit), i


def declaration_params(body: str) -> list[str]:
    """The [Name] parameters the outer LAMBDA declares, in order."""
    head = _strip_comments(body[body.index("LAMBDA(") + len("LAMBDA(") :])
    depth, out, i = 0, [], 0
    buf: list[str] = []
    while i < len(head):
        char = head[i]
        if char == '"':
            _lit, i = _read_string(head, i)
            break
        if char in "({[":
            if char == "[" and depth == 0:
                close = head.index("]", i)
                out.append(head[i + 1 : close])
                i = close + 1
                continue
            depth += 1
        elif char in ")}]":
            if depth == 0:
                break
            depth -= 1
        elif head[i : i + 4] == "LET(":
            break
        buf.append(char)
        i += 1
    return out


def local_names(body: str, params: list[str]) -> list[str]:
    """Every name Excel stores with the _xlpm. marker: parameters and LET bindings."""
    code = _strip_comments(body)
    names = list(params)
    # LET binding names: the identifier at the start of each "name, value," pair. The
    # library writes one per line, name first, so the line start is a reliable anchor.
    for match in re.finditer(r"^\s{4,}([A-Za-z_][A-Za-z0-9_]*\??)\s*,", code, re.MULTILINE):
        if match.group(1) not in names:
            names.append(match.group(1))
    # Inner LAMBDA parameters, stored the same way. Every argument that is a bare
    # identifier is a parameter; the last argument is the body and is an expression.
    # Missing one of these is invisible to the source comparison, because that
    # comparison strips the very marker being missed, so it is parsed properly here
    # and checked again by unmarked_names() once the stored form exists.
    for match in re.finditer(r"\bLAMBDA\s*\(", code):
        for argument in _arguments(code, match.end()):
            argument = argument.strip()
            if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*\??", argument) and argument not in names:
                names.append(argument)
    return names


def _arguments(text: str, start: int) -> list[str]:
    """The top-level arguments of a call whose opening bracket ends at start."""
    out, buf, depth, i, n = [], [], 0, start, len(text)
    while i < n:
        char = text[i]
        if char == '"':
            lit, i = _read_string(text, i)
            buf.append(lit)
            continue
        if char in "({[":
            depth += 1
        elif char in ")}]":
            if depth == 0:
                break
            depth -= 1
        elif char == "," and depth == 0:
            out.append("".join(buf))
            buf = []
            i += 1
            continue
        buf.append(char)
        i += 1
    out.append("".join(buf))
    return out


# Functions Excel stores without a marker, which is every name that predates the
# 2007 file format. Anything else left bare in a stored definition is a mistake.
PLAIN = frozenset(
    {
        "AND", "CHOOSE", "COLUMNS", "FALSE", "IF", "INT", "ISNUMBER", "MAX", "MIN",
        "OR", "ROWS", "SUM", "TRANSPOSE", "TRIM", "TRUE",
    }
)


def unmarked_names(stored: str, library: set[str]) -> list[str]:
    """Identifiers in a stored definition that carry no marker and are not built in.

    Excel marks every parameter, LET binding and post-2007 function. A name that
    reaches the workbook without its marker is read as something else entirely: an
    unmarked LAMBDA parameter makes Excel reject the whole definition. The source
    comparison cannot see this, because it strips the markers before comparing.
    """
    bare = []
    for is_string, chunk in _split_literals(stored):
        if is_string:
            continue
        for match in re.finditer(r"(?<![A-Za-z0-9_.])([A-Za-z_][A-Za-z0-9_.]*\??)", chunk):
            name = match.group(1)
            if name.startswith(("_xlfn.", "_xlpm.", "_xlop.", "_xlws.", f"{NAMESPACE}.")):
                continue
            if name in PLAIN:
                continue
            bare.append(name)
    return sorted(set(bare))


def to_stored(body: str, params: list[str], locals_: list[str], library: set[str]) -> str:
    """Render a published source body in the form xl/workbook.xml stores."""
    pieces = []
    for is_string, chunk in _split_literals(_strip_comments(body)):
        if is_string:
            pieces.append(chunk)
            continue
        # Declaration markers first: [Name] is an optional parameter, not an array.
        for param in params:
            chunk = chunk.replace(f"[{param}]", f"\0OP\0{param}")
        # Library calls take the namespace the Advanced Formula Environment compiles in.
        for bare in sorted(library, key=len, reverse=True):
            chunk = re.sub(
                r"(?<![A-Za-z0-9_.!])" + re.escape(bare) + r"(?=\s*\()",
                f"\0LIB\0{bare}",
                chunk,
            )
        # Post-2007 functions carry their marker.
        for fn in sorted(XLFN, key=len, reverse=True):
            chunk = re.sub(
                r"(?<![A-Za-z0-9_.\0])" + fn + r"(?=\s*\()", f"\0FN\0{fn}", chunk
            )
        # Everything the function binds itself is a _xlpm. name.
        for local in sorted(locals_, key=len, reverse=True):
            chunk = re.sub(
                r"(?<![A-Za-z0-9_.\0])" + re.escape(local) + r"(?![A-Za-z0-9_?])",
                f"\0PM\0{local}",
                chunk,
            )
        chunk = re.sub(r"\s+", " ", chunk)
        pieces.append(chunk)
    stored = "".join(pieces)
    stored = (
        stored.replace("\0OP\0", "_xlop.")
        .replace("\0FN\0", "_xlfn.")
        .replace("\0PM\0", "_xlpm.")
        .replace("\0LIB\0", f"{NAMESPACE}.")
    )
    return stored.strip()


def xml_escape(text: str) -> str:
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def build_definitions(library: set[str]) -> dict[str, tuple[str, str, str]]:
    """Qualified name -> (comment, stored body, published body), self-checked."""
    out = {}
    for name, comment, block, _about in FUNCTIONS:
        match = NAME.match(statements(block)[0])
        assert match and match.group(1) == name, f"{name}: source block does not declare it"
        body = match.group(2).strip()
        params = declaration_params(body)
        stored = to_stored(body, params, local_names(body, params), library)
        loose = unmarked_names(stored, library)
        if loose:
            raise ValueError(
                f"{name}: {', '.join(loose)} reached the stored form without a marker; "
                "Excel will not read the definition"
            )
        # The gate's own comparison, run before anything is written. This is what
        # stopped the two hand-written lists drifting the last time.
        want = canonical(qualify(body, NAMESPACE, library))
        got = canonical(stored)
        if want != got:
            cut = next(
                (i for i in range(min(len(want), len(got))) if want[i] != got[i]),
                min(len(want), len(got)),
            )
            raise ValueError(
                f"{name}: generated definition does not reproduce its source at "
                f"character {cut}\n  src   : {want[max(0, cut - 40):cut + 60]}\n"
                f"  stored: {got[max(0, cut - 40):cut + 60]}"
            )
        out[f"{NAMESPACE}.{name}"] = (comment, stored, body)
    return out


# --------------------------------------------------------------------------- #
# Stores
# --------------------------------------------------------------------------- #


def about_block(indent: str) -> str:
    rows = [f'{indent}"{ABOUT_HEADING}¶" &']
    for name, _comment, _block, about in FUNCTIONS:
        rows.append(f'{indent}"{name:<19}→{about}¶" &')
    return "\n".join(rows)


def read_text(path: Path) -> str:
    with open(path, encoding="utf-8", newline="") as handle:
        return handle.read()


def write_text(path: Path, text: str) -> None:
    with open(path, "w", encoding="utf-8", newline="") as handle:
        handle.write(text)


def src_state(text: str) -> str:
    present = sum(1 for name in NAMES if f"\n{name} = LAMBDA(" in text)
    about = text.count(ABOUT_HEADING)
    if present == 0 and about == 0:
        return "absent"
    if present == len(NAMES) and about == 1:
        return "applied"
    raise ValueError(
        f"src/{MODULE}.txt holds {present} of {len(NAMES)} lease functions and "
        f"{about} About heading(s); it is neither state this pass recognises"
    )


def workbook_state(book: str) -> str:
    present = sum(1 for name in QUALIFIED if f'<definedName name="{name}"' in book)
    about = book.count(ABOUT_HEADING)
    if present == 0 and about == 0:
        return "absent"
    if present == len(QUALIFIED) and about == 1:
        return "applied"
    raise ValueError(
        f"xl/workbook.xml holds {present} of {len(QUALIFIED)} lease defined names and "
        f"{about} About heading(s); it is neither state this pass recognises"
    )


def add_to_src(text: str) -> str:
    anchor = ABOUT_ANCHOR
    hits = text.count(anchor)
    if hits != 1:
        raise ValueError(f"src/{MODULE}.txt: About anchor found {hits} times, expected 1")
    line_start = text.rfind("\n", 0, text.index(anchor)) + 1
    indent = text[line_start : text.index(anchor)]
    text = text.replace(anchor + " &", anchor + " &\n" + about_block(indent), 1)

    blocks = "\n\n\n" + "\n\n\n".join(block for _n, _c, block, _a in FUNCTIONS) + "\n"
    if not text.endswith("\n"):
        text += "\n"
    return text + blocks


def add_to_workbook(book: str, definitions: dict[str, tuple[str, str, str]]) -> str:
    # About table: the same anchor, escaped the way the workbook stores it.
    stored_anchor = xml_escape(ABOUT_ANCHOR)
    hits = book.count(stored_anchor)
    if hits != 1:
        raise ValueError(f"xl/workbook.xml: About anchor found {hits} times, expected 1")
    addition = f' &amp; "{xml_escape(ABOUT_HEADING)}¶"' + "".join(
        f' &amp; "{xml_escape(f"{name:<19}→{about}")}¶"'
        for name, _comment, _block, about in FUNCTIONS
    )
    book = book.replace(stored_anchor, stored_anchor + addition, 1)

    # Defined names, each in its case-insensitive alphabetical place.
    existing = re.findall(r'<definedName name="(oz\.[^"]+)"', book)
    for name in sorted(definitions, key=str.lower):
        comment, stored, _body = definitions[name]
        element = (
            f'<definedName name="{xml_escape(name)}" '
            f'comment="{xml_escape(comment)}">{xml_escape(stored)}</definedName>'
        )
        after = [n for n in existing if n.lower() < name.lower()]
        if not after:
            raise ValueError(f"{name} sorts before every shipped name, which is not expected")
        previous = after[-1]
        marker = f'<definedName name="{xml_escape(previous)}"'
        start = book.index(marker)
        end = book.index("</definedName>", start) + len("</definedName>")
        book = book[:end] + element + book[end:]
        existing = sorted(existing + [name], key=str.lower)
    return book


def update_index(path: Path, definitions: dict[str, tuple[str, str, str]]) -> bool:
    """Add a row per function, deriving every field the way verify_index does."""
    with path.open(encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle)
        fieldnames = list(reader.fieldnames or [])
        rows = list(reader)
    if set(fieldnames) != {"function", "module", "previous_name", "signature", "description"}:
        raise ValueError(f"{path} has unexpected columns: {fieldnames}")
    have = {row["function"] for row in rows}
    if set(definitions) <= have:
        return False
    if set(definitions) & have:
        raise ValueError(f"{path} holds some of the lease functions but not all")

    signature_re = re.compile(r"FUNCTION:\s*→?\s*(.*?)¶")
    for name in sorted(definitions):
        comment, stored, _body = definitions[name]
        found = signature_re.search(stored)
        if not found:
            raise ValueError(f"{name}: stored definition has no FUNCTION line")
        rows.append(
            {
                "function": name,
                "module": MODULE,
                # Nothing before the v1.2.6 baseline, so it records nothing.
                "previous_name": "",
                "signature": found.group(1).strip(),
                "description": re.sub(r"\s{2,}", " ", comment),
            }
        )
    # functions.csv is sorted case-sensitively, unlike the workbook's defined names.
    # Sorting it the other way moves a dozen untouched rows and buries the addition.
    rows.sort(key=lambda row: row["function"])
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    return True


def run(workbook: Path, src_dir: Path, index: Path | None) -> list[str]:
    src_path = src_dir / f"{MODULE}.txt"
    if not src_path.is_file():
        raise ValueError(f"src: missing {src_path}")
    src_text = read_text(src_path)

    with zipfile.ZipFile(workbook) as archive:
        parts = {name: archive.read(name) for name in archive.namelist()}
    book = parts["xl/workbook.xml"].decode("utf-8")

    src_status, book_status = src_state(src_text), workbook_state(book)
    if src_status != book_status:
        raise ValueError(
            f"src/ is {src_status} but the workbook is {book_status}; the two views "
            "must not be applied separately"
        )

    # The library every generated body is qualified against, this pass included.
    library = set()
    for path in sorted(src_dir.glob("*.txt")):
        for statement in statements(read_text(path)):
            match = NAME.match(statement)
            if match:
                library.add(match.group(1))
    library.update(NAMES)
    definitions = build_definitions(library)

    changed: list[str] = []
    if src_status == "absent":
        write_text(src_path, add_to_src(src_text))
        changed.append(f"src/{MODULE}.txt")
        parts["xl/workbook.xml"] = add_to_workbook(book, definitions).encode("utf-8")
        write_deterministic(workbook, parts)
        changed.append("workbook")

    if index is not None and index.is_file():
        if update_index(index, definitions):
            changed.append(index.name)
    elif index is not None:
        print(f"note: {index} not found, index not updated")

    return changed


def main() -> None:
    if len(sys.argv) > 4:
        sys.exit(
            "FAIL: usage: python tools/postbuild/aasb16_leases.py "
            "[workbook] [src dir] [functions.csv]"
        )
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    src_dir = Path(sys.argv[2] if len(sys.argv) > 2 else "src")
    index = Path(sys.argv[3]) if len(sys.argv) > 3 else workbook.parent / "functions.csv"
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changed = run(workbook, src_dir, index)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    if changed:
        print(f"added the AASB 16 lease functions to: {', '.join(changed)}")
    else:
        print("AASB 16 lease functions already applied; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
