"""Check that src/ really is the source of the functions in ozzit.xlsx.

Usage: python tools/verify_sources.py [path/to/ozzit.xlsx] [src dir]

src/ exists so the library can be read, diffed and imported back into Excel. That
only means anything if what is published matches what ships, and if it is written
in the form Excel accepts as input rather than the form the file format stores.

Four differences between the 2 are conventions, not divergences, and are mapped
rather than ignored:

    stored form            typed form         why
    _xlfn. _xlpm. _xlws.   (nothing)          markers for post-2007 functions
    _xlop.Name             [Name]             an OPTIONAL parameter
    [0]!Name               Name               "a name in this workbook"
    SINGLE(x)              @x                 implicit intersection

The parameter one matters most: stripping _xlop. instead of mapping it turns an
optional parameter into a required one, and any function whose body calls
ISOMITTED() on it is then rejected by Excel outright. That shipped once.

Module-local calls are qualified the way the Advanced Formula Environment does on
import, built-in names are upper-cased the way Excel does, and whitespace is
ignored, since help text is padded for alignment and TRIM() removes it.

The sources are also read for 2 defects no comparison can see. A converted date
argument that is never read means the raw one is used instead. An argument default
written IF(OR(ISOMITTED(x), x=""), d, x) reduces a column to one TRUE or FALSE, so
one blank cell gives every row the default; KNOWN_COLLAPSED_DEFAULTS lists the ones
still in src/ and why.
"""
import functools
import html
import re
import sys
import zipfile
from collections.abc import Collection
from pathlib import Path

# Every function name carries a λ, and a Windows console defaults to cp1252, which
# cannot encode it: without this the check dies printing its own result.
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")


WORKBOOK = sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx"
SRC_DIR = sys.argv[2] if len(sys.argv) > 2 else "src"
MODULES = ["Dates", "Essentials", "Financial", "Ratios", "Utilities", "Debt"]
NAMESPACE = "oz"          # every function ships under one prefix, whatever module it lives in

OPENERS, CLOSERS = "({[", ")}]"
NAME = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_.λ]*)\s*=\s*(.+)$", re.DOTALL)
OPTIONAL = re.compile(r"_xlop\.([A-Za-z_][A-Za-z0-9_]*\??)")   # ? is legal in a parameter name
PREFIX = re.compile(r"_xl[a-z]+\.", re.IGNORECASE)
PLACEHOLDER = re.compile("\x00(\\d+)\x00")            # a held-aside string literal

# An argument default that tests its parameter inside OR() or AND() reduces a whole
# column of inputs to one TRUE or FALSE, so a single blank cell gives every row the
# default. GSTAddλ, GSTExtractλ, Movementλ and FinancialYearλ each shipped that way.
# These are the occurrences still in src/, each with the reason it is still there. A new
# occurrence fails the gate, and so does an entry that no longer occurs, so the fix that
# removes one also removes its entry here.
KNOWN_COLLAPSED_DEFAULTS = {
    ("CorkscrewλDV", "Opening"): "scalar: one opening balance, which Corkscrewλ's SCAN needs",
    ("DayCountRateλ", "Convention"): "scalar: one convention code for the whole timeline",
    ("LeaseLiabilityλ", "InAdvance"): "scalar: one lease at a time, so one TRUE or FALSE",
    ("LeaseRemeasureλ", "InAdvance"): "scalar: one lease at a time, so one TRUE or FALSE",
    ("LeaseScheduleλ", "InAdvance"): "scalar: one lease at a time, so one TRUE or FALSE",
    ("AnnualRateλ", "PeriodsPerYear"): "defect: a column of frequencies; fix in a native round",
    ("DateDifλ", "Unit"): "defect: a column of units; fix in a native round",
    ("IsOccurrenceDateλ", "LastOccurrence"): "defect: a table column; fix in a native round",
    ("IsOccurrenceDateλ", "Repeats"): "defect: a table column; fix in a native round",
    ("PeriodRateλ", "PeriodsPerYear"): "defect: a column of frequencies; fix in a native round",
}
# Calls that reduce what they are given to one value, and calls that bind names this check
# does not follow. A parameter read inside either is not being passed through row by row.
# LET binds names too, but the check follows it: see passed_through.
SCALAR_CALLS = frozenset({
    "AND", "AREAS", "AVERAGE", "COLUMNS", "COUNT", "COUNTA", "COUNTBLANK", "ISOMITTED",
    "ISREF", "MAX", "MIN", "OR", "ROWS", "SUM", "SUMPRODUCT", "XOR",
})
OPAQUE_CALLS = frozenset({"BYCOL", "BYROW", "LAMBDA", "MAKEARRAY", "MAP", "REDUCE", "SCAN"})
IF_CALL = re.compile(r"(?<![A-Za-z0-9_.?λ])IF\s*\(", re.IGNORECASE)
COMBINED = re.compile(r"\s*(?:NOT\s*\(\s*)?(?:AND|OR)\s*\(", re.IGNORECASE)
TOKEN = re.compile(r"(?P<call>[A-Za-z_][A-Za-z0-9_.?λ]*)\s*\(|(?P<name>[A-Za-z_][A-Za-z0-9_.?λ]*)"
                   r"|(?P<open>[({])|(?P<close>[)}])")

failures: list[str] = []


def fail(msg: str) -> None:
    failures.append(msg)


def read_string(text: str, i: int) -> tuple[str, int]:
    """Copy a double-quoted literal verbatim, honouring the "" escape."""
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


def statements(text: str) -> list[str]:
    """Split a module into top-level statements, honouring strings and comments."""
    out, buf, depth = [], [], 0
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == '"':
            lit, i = read_string(text, i)
            buf.append(lit)
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                i += 1
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            j = text.find("*/", i + 2)
            i = j + 2 if j != -1 else n
            continue
        if c in OPENERS:
            depth += 1
        elif c in CLOSERS:
            depth -= 1
        elif c == ";" and depth == 0:
            out.append("".join(buf))
            buf = []
            i += 1
            continue
        buf.append(c)
        i += 1
    if "".join(buf).strip():
        out.append("".join(buf))
    return out


def split_literals(formula: str) -> list[tuple[bool, str]]:
    """Alternating (is_string, chunk) pairs."""
    out: list[tuple[bool, str]] = []
    buf: list[str] = []
    i, n = 0, len(formula)
    while i < n:
        if formula[i] == '"':
            if buf:
                out.append((False, "".join(buf)))
                buf = []
            lit, i = read_string(formula, i)
            out.append((True, lit))
            continue
        buf.append(formula[i])
        i += 1
    if buf:
        out.append((False, "".join(buf)))
    return out


def unwrap_single(s: str) -> str:
    """SINGLE(expr) is how the file format stores the @ implicit-intersection operator."""
    while True:
        at = s.find("SINGLE(")
        if at < 0:
            return s
        depth, i = 0, at + len("SINGLE(") - 1
        while i < len(s):
            if s[i] == "(":
                depth += 1
            elif s[i] == ")":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        if i >= len(s):
            return s
        s = s[:at] + "@" + s[at + len("SINGLE("):i] + s[i + 1:]


def canonical(formula: str) -> str:
    """Upper-cased and whitespace-free outside strings, in the typed form.

    A string literal is kept exactly: its whitespace is its own text, so "a b" and
    "ab" are different definitions and a source that differs from the workbook only
    inside a literal is still a mismatch.

    Every normalisation is therefore applied to the code between the literals and to
    nothing else. Each literal is held aside behind a placeholder while the stored
    markers go and SINGLE() is unwrapped, then put back verbatim. Run over the whole
    text instead, the marker strip erases meaningful characters inside a literal, so
    "[0]!FY" reads as "FY" and a changed help string passes as unchanged.
    """
    parts: list[str] = []
    literals: list[str] = []
    for is_string, chunk in split_literals(formula):
        if is_string:
            parts.append(f"\x00{len(literals)}\x00")
            literals.append(chunk)
            continue
        chunk = OPTIONAL.sub(r"[\1]", chunk)          # BEFORE the generic prefix strip
        chunk = PREFIX.sub("", chunk).replace("[0]!", "")
        parts.append("".join(c.upper() for c in chunk if not c.isspace()))
    # SINGLE() can wrap an expression containing a literal, so it is unwrapped over the
    # placeholders rather than per chunk; the placeholders carry no brackets of their own.
    unwrapped = unwrap_single("".join(parts))
    return PLACEHOLDER.sub(lambda m: literals[int(m.group(1))], unwrapped)


@functools.lru_cache(maxsize=8)
def call_pattern(names: frozenset[str]) -> re.Pattern[str]:
    """One alternation over every name, longest first, matched only where it is called.

    A pass per name cost about 362,000 regex calls to compile the library and was most
    of this gate's run time. Longest first means that at any position the longest name
    that is called there wins, which is what the per-name passes produced; scanning once
    also leaves a qualified name alone rather than re-reading it for a shorter one.
    """
    alternatives = "|".join(re.escape(bare) for bare in sorted(names, key=len, reverse=True))
    return re.compile(r"(?<![A-Za-z0-9_.!])(?:" + alternatives + r")(?=\s*\()")


def qualify(formula: str, module: str, names: Collection[str]) -> str:
    """Turn each module-local call into the qualified name AFE compiles it to."""
    if not names:
        return formula
    call = call_pattern(frozenset(names))
    chunks = []
    for is_string, chunk in split_literals(formula):
        if not is_string:
            chunk = call.sub(lambda match: module + "." + match.group(0), chunk)
        chunks.append(chunk)
    return "".join(chunks)


def dead_conversions(body: str) -> list[str]:
    """Names bound to a converted argument that nothing ever reads.

    Several functions take a date that may be written as text, convert it to a serial
    number under a Cvt name, and then compare or subtract the raw argument instead. The
    conversion is computed and thrown away. It is silent: Excel coerces a text date in
    most arithmetic, so the wrong version usually agrees with the right one, and disagrees
    only where text ordering and date ordering differ.

    A Cvt name that appears once in a body is bound and never read, since a reference
    cannot exist without the binding that precedes it. Comments and string literals are
    already out of the way, so a name inside help text does not count as a use.
    """
    code = "".join(chunk for is_string, chunk in split_literals(body) if not is_string)
    seen = re.findall(r"\bCvt\w*\b", code)
    return sorted({n for n in seen if seen.count(n) == 1})


def call_arguments(code: str, opening: int) -> list[str]:
    """The top-level arguments of the call whose "(" is at code[opening].

    String literals must already be emptied, so their brackets and commas cannot count.
    """
    arguments, depth, start = [], 0, opening + 1
    for i in range(opening, len(code)):
        c = code[i]
        if c in "({":
            depth += 1
        elif c in ")}":
            depth -= 1
            if depth == 0:
                arguments.append(code[start:i])
                break
        elif c == "," and depth == 1:
            arguments.append(code[start:i])
            start = i + 1
    return arguments


def passed_through(code: str, name: str, stop: frozenset[str]) -> bool:
    """True when name is read somewhere in code outside every call listed in stop.

    A LET is followed binding by binding: a name bound to a value that passes name
    through is a copy of it, so LET(Copy, Rate, Copy) passes Rate through.
    """
    return reads_any(code, {name.upper()}, stop)


def reads_any(code: str, names: set[str], stop: frozenset[str]) -> bool:
    """True when one of names (upper-cased) is read in code outside every call in stop."""
    calls: list[str] = []
    resume = 0
    for match in TOKEN.finditer(code):
        if match.start() < resume:
            continue
        call = (match.group("call") or "").upper()
        if call == "LET" and not stop.intersection(calls):
            arguments = call_arguments(code, match.end() - 1)
            if len(arguments) >= 3 and len(arguments) % 2:
                if let_reads_any(arguments, names, stop):
                    return True
                resume = match.end() + len(",".join(arguments)) + 1   # just past its ")"
                continue
        if call:
            calls.append(call)
        elif match.group("open"):
            calls.append(match.group("open"))
        elif match.group("close"):
            if calls:
                calls.pop()
        elif match.group("name").upper() in names and not stop.intersection(calls):
            return True
    return False


def let_reads_any(arguments: list[str], names: set[str], stop: frozenset[str]) -> bool:
    """True when the result of LET(name1, value1, ..., result) reads one of names or a copy."""
    live = set(names)
    *bindings, result = arguments
    for bound, value in zip(bindings[::2], bindings[1::2]):
        if reads_any(value, live, stop):
            live.add(bound.strip().upper())
    return reads_any(result, live, stop)


def collapsed_defaults(body: str) -> list[str]:
    """Parameters defaulted by an IF whose test is OR() or AND() of that parameter.

    OR and AND reduce a column of inputs to one TRUE or FALSE, so in
    IF(OR(ISOMITTED(x), x=""), d, x) one blank cell gives every row d. A parameter
    counts when the test reads it row by row and a branch passes it through row by row,
    itself or as a copy a LET binds. A validator that reduces its answer to one flag on
    purpose, as the λDV companions do, passes nothing through and is not reported.
    """
    code = "".join('""' if is_string else chunk for is_string, chunk in split_literals(body))
    head = re.match(r"\s*LAMBDA\s*\(", code, re.IGNORECASE)
    if head is None:
        return []
    parameters = {re.sub(r"[\[\]\s]", "", p) for p in call_arguments(code, head.end() - 1)[:-1]}
    found = set()
    for match in IF_CALL.finditer(code):
        arguments = call_arguments(code, match.end() - 1)
        test = COMBINED.match(arguments[0]) if len(arguments) > 1 else None
        if test is None:
            continue
        tested = call_arguments(arguments[0], test.end() - 1)
        for name in parameters:
            if (any(passed_through(part, name, SCALAR_CALLS) for part in tested)
                    and any(passed_through(branch, name, SCALAR_CALLS | OPAQUE_CALLS)
                            for branch in arguments[1:])):
                found.add(name)
    return sorted(found)


def main() -> int:
    # Two passes: the namespace is flat, so a call in one module may name a function
    # declared in another, and every declaration has to be known before any is qualified.
    raw = {}
    collapsed: set[tuple[str, str]] = set()
    src = Path(SRC_DIR)
    for mod in MODULES:
        path = src / f"{mod}.txt"
        if not path.exists():
            fail("missing source module %s" % path)
    for path in sorted(src.glob("*.txt")):
        text = path.read_text(encoding="utf-8")
        # The published source must be in the form Excel accepts as input.
        for token in ("[0]!", "_xlfn.", "_xlpm.", "_xlop.", "_xlws."):
            if token in text:
                fail("%s contains the stored-form token %r, which Excel will not accept "
                     "as typed input" % (path, token))
        for st in statements(text):
            m = NAME.match(st)
            if m:
                if m.group(1) in raw:
                    fail("%s is declared in more than one module" % m.group(1))
                raw[m.group(1)] = m.group(2).strip()
                for dead in dead_conversions(m.group(2)):
                    fail("%s binds %s and never reads it, so the argument it converts is "
                         "used in its raw form" % (m.group(1), dead))
                collapsed.update((m.group(1), p) for p in collapsed_defaults(m.group(2)))
            elif st.strip():
                fail("%s has an unparsable statement: %r" % (path, st.strip()[:60]))
    for function, parameter in sorted(collapsed - set(KNOWN_COLLAPSED_DEFAULTS)):
        fail("%s defaults %s with an IF that tests it inside OR() or AND(), which reduce a "
             "column to one value, so one blank cell gives every row the default. Test each "
             "row: IF(ISOMITTED(x), d, IF(TRIM(x & \"\")=\"\", d, x))" % (function, parameter))
    for function, parameter in sorted(set(KNOWN_COLLAPSED_DEFAULTS) - collapsed):
        fail("%s no longer defaults %s inside OR() or AND(); remove its entry from "
             "KNOWN_COLLAPSED_DEFAULTS" % (function, parameter))

    every = set(raw)
    parsed = {"%s.%s" % (NAMESPACE, bare): qualify(body, NAMESPACE, every)
              for bare, body in raw.items()}

    z = zipfile.ZipFile(WORKBOOK)
    wbx = z.read("xl/workbook.xml").decode("utf-8")
    shipped = {n: html.unescape(b) for n, b in
               re.findall(r'<definedName name="(%s\.[^"]+)"[^>]*>(.*?)</definedName>' % NAMESPACE,
                          wbx, re.DOTALL)}

    for name in sorted(set(shipped) - set(parsed)):
        fail("%s ships in the workbook but is not in src/" % name)
    for name in sorted(set(parsed) - set(shipped)):
        fail("%s is in src/ but does not ship in the workbook" % name)

    matched = 0
    for name in sorted(set(parsed) & set(shipped)):
        a, b = canonical(parsed[name]), canonical(shipped[name])
        if a == b:
            matched += 1
            continue
        cut = next((i for i in range(min(len(a), len(b))) if a[i] != b[i]), min(len(a), len(b)))
        fail("%s differs from its published source at character %d\n"
             "        src/ : %s\n        xlsx : %s"
             % (name, cut, a[max(0, cut - 30):cut + 50], b[max(0, cut - 30):cut + 50]))

    if failures:
        print("FAIL: %d problem(s) comparing %s with %s/" % (len(failures), WORKBOOK, SRC_DIR))
        for item in failures:
            print("  - %s" % item)
        return 1
    print("OK: %s/ reproduces all %d functions in %s, every date conversion in them is read, "
          "and only the %d listed argument defaults test a column inside OR() or AND()"
          % (SRC_DIR, matched, WORKBOOK, len(KNOWN_COLLAPSED_DEFAULTS)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
