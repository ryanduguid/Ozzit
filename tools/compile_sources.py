"""Compile src/*.txt into the defined names ozzit.xlsx stores.

Usage: python tools/compile_sources.py [workbook] [src dir] [--check] [--index functions.csv]

The workbook is the shipped authority and src/ is its published, typed-form
view. Until now the only way to change a function was to hand-write both the
typed form and the stored form and swap them in together, which is how every
postbuild pass works. This tool closes the loop the other way: it renders each
published definition into the form xl/workbook.xml stores, proves the rendering
reproduces its source through verify_sources.py's own comparison, and writes it
over the defined name that ships. A function whose stored form already matches
is left byte for byte as it was, so the pass is idempotent and touches only what
changed in src/.

The stored form differs from the typed form in the ways verify_sources.py maps:

    _xlfn.NAME( / _xlfn._xlws.NAME(   a post-2007 function
    _xlop.Name                        an optional parameter, declared [Name]
    _xlpm.Name                        a parameter or LET binding, wherever read
    _xlfn.SINGLE(x)                   the @ implicit-intersection operator
    oz.Name(                          a call into the library

Excel also normalises the case of every identifier it recognises, so Switch(
becomes _xlfn.SWITCH( and a binding read as Mpp is stored under the spelling
it was declared with. This tool does the same, and refuses any identifier it
cannot classify: a name that reaches the workbook without its marker is read
as something else entirely, and an unmarked parameter makes Excel reject the
whole definition.

--check reports which functions would change without writing anything.
--index rewrites functions.csv from the compiled workbook, keeping each
function's previous_name, the way verify_index.py derives the other fields.

Cached values are never touched. A formula change that alters what a
demonstration cell computes still needs tools/refresh_cache.py, and
tools/verify_cache.py is the gate that proves it. Run tools/sync_afe_store.py
after this tool, as the postbuild README's run order requires for any change to
src/, and tools/sanitise_workbook.py last.
"""

from __future__ import annotations

import csv
import html
import io
import re
import sys
import zipfile
from pathlib import Path
from typing import NamedTuple

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sanitise_workbook import write_deterministic  # noqa: E402
from verify_index import index_fields  # noqa: E402
from verify_sources import NAME, canonical, qualify, statements  # noqa: E402

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

NAMESPACE = "oz"
MODULES = ("Dates", "Essentials", "Financial", "Ratios", "Utilities", "Debt")

# Functions Excel stores with the _xlfn. marker: everything that arrived after
# the 2007 file format was fixed. SORT and FILTER carry a second, _xlws., marker.
XLWS = frozenset({"SORT", "FILTER"})
XLFN = frozenset(
    {
        "ANCHORARRAY", "ARRAYTOTEXT", "BYCOL", "BYROW", "CHOOSECOLS", "CHOOSEROWS",
        "CONCAT", "DAYS", "DROP", "EXPAND", "FORMULATEXT", "HSTACK", "IFNA", "IFS",
        "ISOMITTED", "ISOWEEKNUM", "LAMBDA", "LET", "MAKEARRAY", "MAP", "MAXIFS",
        "MINIFS", "NUMBERVALUE", "RANDARRAY", "REDUCE", "SCAN", "SEQUENCE", "SINGLE",
        "SORTBY", "SWITCH", "TAKE", "TEXTAFTER", "TEXTBEFORE", "TEXTJOIN", "TEXTSPLIT",
        "TOCOL", "TOROW", "UNIQUE", "VALUETOTEXT", "VSTACK", "WRAPCOLS", "WRAPROWS",
        "XLOOKUP", "XMATCH", "XOR",
    }
    | XLWS
)

# Functions that predate the 2007 format and are stored bare. Any bare identifier
# outside this list, the library and the definition's own names is an error.
PLAIN = frozenset(
    {
        "ABS", "AND", "AVERAGE", "AVERAGEIF", "CEILING", "CELL", "CHOOSE", "COLUMN",
        "COLUMNS", "COUNT", "COUNTA", "COUNTBLANK", "COUNTIF", "COUNTIFS", "CUMIPMT",
        "CUMPRINC", "DATE", "DATEDIF", "DATEVALUE", "DAY", "DB", "DDB", "EDATE",
        "EOMONTH", "EXACT", "EXP", "FIND", "FLOOR", "FV", "HLOOKUP", "IF", "IFERROR",
        "INDEX", "INDIRECT", "INT", "IPMT", "IRR", "ISBLANK", "ISERR", "ISERROR",
        "ISLOGICAL", "ISNA", "ISNONTEXT", "ISNUMBER", "ISTEXT", "LARGE", "LEFT", "LEN",
        "LN", "LOG", "LOOKUP", "LOWER", "MATCH", "MAX", "MEDIAN", "MID", "MIN", "MMULT",
        "MOD", "MONTH", "MROUND", "N", "NA", "NETWORKDAYS", "NOT", "NOW", "NPER",
        "NPV", "OFFSET", "OR", "PMT", "POWER", "PPMT", "PRODUCT", "PROPER", "PV",
        "QUOTIENT", "RATE", "REPLACE", "REPT", "RIGHT", "ROUND", "ROUNDDOWN",
        "ROUNDUP", "ROW", "ROWS", "SEARCH", "SIGN", "SLN", "SMALL", "SQRT",
        "SUBSTITUTE", "SUM", "SUMIF", "SUMIFS", "SUMPRODUCT", "SYD", "TEXT", "TIME",
        "TODAY", "TRANSPOSE", "TRIM", "TRUNC", "UPPER", "VALUE", "VDB", "VLOOKUP",
        "WEEKDAY", "WEEKNUM", "WORKDAY", "XIRR", "XNPV", "YEAR", "YEARFRAC",
    }
)
CONSTANTS = frozenset({"TRUE", "FALSE"})
ERRORS = ("#DIV/0!", "#N/A", "#NAME?", "#NULL!", "#NUM!", "#REF!", "#VALUE!", "#CALC!", "#SPILL!")

IDENT = re.compile(r"(?<![A-Za-z0-9_.λ\0])([A-Za-z_][A-Za-z0-9_.λ]*\??)")
NAME_RE = re.compile(
    r'<definedName name="(oz\.[^"]+)"([^>]*)>(.*?)</definedName>', re.DOTALL
)


class Compiled(NamedTuple):
    """One function as it is published and as it is stored."""

    name: str          # qualified, oz.CountDOWλ
    module: str
    body: str          # typed form, the statement's right-hand side
    stored: str        # unescaped stored form
    comment: str       # the Name Manager comment, from the source header


# The header every published function carries, whose description is the Name
# Manager comment. Excel keeps at most 255 characters of a comment and stores a
# line break as _x000a_.
HEADER = re.compile(
    r"/\*\s*FUNCTION NAME:\s*(?P<name>[^\n]+?)\s*\n\s*DESCRIPTION:\*/\s*/\*\*(?P<text>.*?)\*/",
    re.DOTALL,
)
COMMENT_LIMIT = 255


def header_comments(text: str) -> dict[str, str]:
    """Bare name -> Name Manager comment, for every header in a module."""
    out = {}
    for match in HEADER.finditer(text):
        raw = match.group("text").replace("\r\n", "\n").strip()[:COMMENT_LIMIT]
        out[match.group("name").strip()] = raw.replace("\n", "_x000a_")
    return out


# --------------------------------------------------------------------------- #
# Reading the typed form
# --------------------------------------------------------------------------- #


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


def split_literals(text: str) -> list[tuple[bool, str]]:
    """Alternating (is_string, chunk) pairs, honouring the "" escape."""
    out: list[tuple[bool, str]] = []
    buf: list[str] = []
    i, n = 0, len(text)
    while i < n:
        if text[i] == '"':
            if buf:
                out.append((False, "".join(buf)))
                buf = []
            lit, i = read_string(text, i)
            out.append((True, lit))
            continue
        buf.append(text[i])
        i += 1
    if buf:
        out.append((False, "".join(buf)))
    return out


def strip_comments(text: str) -> str:
    out, i, n = [], 0, len(text)
    while i < n:
        if text[i] == '"':
            lit, i = read_string(text, i)
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


def arguments(text: str, start: int) -> list[str]:
    """The top-level arguments of a call whose opening bracket ends at start."""
    out, buf, depth, i, n = [], [], 0, start, len(text)
    while i < n:
        char = text[i]
        if char == '"':
            lit, i = read_string(text, i)
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


def is_identifier(text: str) -> bool:
    return re.fullmatch(r"[A-Za-z_][A-Za-z0-9_λ]*\??", text.strip()) is not None


def declaration_params(code: str) -> list[str]:
    """The parameters the outer LAMBDA declares, in order, brackets dropped."""
    head = code.index("LAMBDA(") + len("LAMBDA(")
    params = []
    for argument in arguments(code, head)[:-1]:
        params.append(argument.strip().strip("[]").strip())
    return params


def optional_params(code: str) -> set[str]:
    """The parameters the outer LAMBDA declares in brackets."""
    head = code.index("LAMBDA(") + len("LAMBDA(")
    return {
        argument.strip()[1:-1].strip()
        for argument in arguments(code, head)[:-1]
        if argument.strip().startswith("[")
    }


def local_names(code: str) -> list[str]:
    """Every name stored with the _xlpm. marker: LAMBDA parameters and LET bindings.

    Every LAMBDA argument that is a bare identifier is a parameter; the last argument
    is the body. Every even-numbered LET argument before the last is a binding name.
    Both are read structurally, because the source comparison strips the very marker
    that a missed name would lack.
    """
    # Help text may spell out a LAMBDA( or LET( of its own; only code declares names.
    code = "".join('""' if is_string else chunk for is_string, chunk in split_literals(code))
    names: list[str] = []
    for match in re.finditer(r"(?<![A-Za-z0-9_.λ])LAMBDA\s*\(", code, re.IGNORECASE):
        for argument in arguments(code, match.end())[:-1]:
            bare = argument.strip().strip("[]").strip()
            if is_identifier(bare) and bare not in names:
                names.append(bare)
    for match in re.finditer(r"(?<![A-Za-z0-9_.λ])LET\s*\(", code, re.IGNORECASE):
        args = arguments(code, match.end())
        for bare in (a.strip() for a in args[:-1:2]):
            if is_identifier(bare) and bare not in names:
                names.append(bare)
    return names


# --------------------------------------------------------------------------- #
# Rendering the stored form
# --------------------------------------------------------------------------- #


def unwrap_at(code: str) -> str:
    """Rewrite the @ operator as the SINGLE() call the file format stores."""
    out, i, n = [], 0, len(code)
    while i < n:
        if code[i] == '"':
            lit, i = read_string(code, i)
            out.append(lit)
            continue
        if code[i] == "@":
            j = i + 1
            while j < n and code[j].isspace():
                j += 1
            m = re.match(r"[A-Za-z_][A-Za-z0-9_.λ]*\??", code[j:])
            if not m:
                raise ValueError("@ is not followed by a name: %r" % code[i : i + 20])
            k = j + m.end()
            while k < n and code[k].isspace():
                k += 1
            if k < n and code[k] == "(":
                depth = 0
                while k < n:
                    if code[k] == '"':
                        _lit, k = read_string(code, k)
                        continue
                    if code[k] == "(":
                        depth += 1
                    elif code[k] == ")":
                        depth -= 1
                        if depth == 0:
                            k += 1
                            break
                    k += 1
            else:
                k = j + m.end()
            out.append("SINGLE(" + code[j:k] + ")")
            i = k
            continue
        out.append(code[i])
        i += 1
    return "".join(out)


def render(body: str, library: set[str]) -> str:
    """Render a typed-form body in the form xl/workbook.xml stores."""
    code = unwrap_at(strip_comments(body))
    optional = optional_params(code)
    locals_ = {name.lower(): name for name in local_names(code)}
    unknown: list[str] = []

    pieces: list[str] = []
    for is_string, chunk in split_literals(code):
        if is_string:
            pieces.append(chunk)
            continue
        # An optional parameter is declared in brackets, which are not an array here.
        for param in optional:
            chunk = re.sub(r"\[\s*" + re.escape(param) + r"\s*\]", "\0OP\0" + param, chunk)
        # Error literals hold letters that are not names; hide them until the end.
        for position, error in enumerate(ERRORS):
            chunk = re.sub(re.escape(error), "\0E%d\0" % position, chunk, flags=re.IGNORECASE)

        def classify(match: re.Match[str]) -> str:
            word = match.group(1)
            upper = word.upper()
            after = chunk[match.end():].lstrip()
            called = after.startswith("(")
            if word.startswith(f"{NAMESPACE}.") and word[len(NAMESPACE) + 1:] in library:
                return word
            if word.lower() in locals_ and not (called and (upper in XLFN or upper in PLAIN)):
                return "\0PM\0" + locals_[word.lower()]
            if called:
                if upper in XLWS:
                    return "\0FN\0\0WS\0" + upper
                if upper in XLFN:
                    return "\0FN\0" + upper
                if upper in PLAIN:
                    return upper
                if word in library:
                    return "\0LIB\0" + word
            if upper in CONSTANTS:
                return upper
            unknown.append(word)
            return word

        chunk = IDENT.sub(classify, chunk)
        for position, error in enumerate(ERRORS):
            chunk = chunk.replace("\0E%d\0" % position, error)
        chunk = re.sub(r"\s+", " ", chunk)
        pieces.append(chunk)

    if unknown:
        raise ValueError(
            "%s reached the stored form without a marker; Excel will not read the "
            "definition" % ", ".join(sorted(set(unknown)))
        )
    stored = "".join(pieces)
    stored = (
        stored.replace("\0OP\0", "_xlop.")
        .replace("\0FN\0", "_xlfn.")
        .replace("\0WS\0", "_xlws.")
        .replace("\0PM\0", "_xlpm.")
        .replace("\0LIB\0", f"{NAMESPACE}.")
    )
    return stored.strip()


def xml_escape(text: str) -> str:
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def tight(formula: str) -> str:
    """The stored form with no whitespace and no workbook-scope markers.

    Runs of whitespace inside help literals are what TRIM() removes when the help is
    read, and [0]! is how Excel marks a name it resolved in this workbook; neither
    is a difference worth rewriting a definition for.
    """
    return re.sub(r"\s+", "", formula).replace("[0]!", "")


# --------------------------------------------------------------------------- #
# Compiling a module set
# --------------------------------------------------------------------------- #


def read_text(path: Path) -> str:
    with open(path, encoding="utf-8", newline="") as handle:
        return handle.read()


def compile_sources(src: Path) -> list[Compiled]:
    """Every function in src/, rendered and proven to reproduce its source."""
    declared: list[tuple[str, str, str, str]] = []
    for module in MODULES:
        path = src / f"{module}.txt"
        if not path.is_file():
            raise ValueError(f"missing source module {path}")
        text = read_text(path)
        comments = header_comments(text)
        for statement in statements(text):
            match = NAME.match(statement)
            if match:
                bare = match.group(1)
                if bare not in comments:
                    raise ValueError(f"{bare}: no FUNCTION NAME / DESCRIPTION header in {path.name}")
                declared.append((bare, module, match.group(2).strip(), comments[bare]))
    library = {bare for bare, _module, _body, _comment in declared}

    out: list[Compiled] = []
    for bare, module, body, comment in declared:
        try:
            stored = render(body, library) if "LAMBDA(" in body else render_plain(body, library)
        except ValueError as exc:
            raise ValueError(f"{bare}: {exc}") from exc
        want = canonical(qualify(body, NAMESPACE, library))
        got = canonical(stored)
        if want != got:
            cut = next(
                (i for i in range(min(len(want), len(got))) if want[i] != got[i]),
                min(len(want), len(got)),
            )
            raise ValueError(
                f"{bare}: the rendering does not reproduce its source at character {cut}\n"
                f"  src   : {want[max(0, cut - 40):cut + 60]}\n"
                f"  stored: {got[max(0, cut - 40):cut + 60]}"
            )
        out.append(Compiled(f"{NAMESPACE}.{bare}", module, body, stored, comment))
    return out


def render_plain(body: str, library: set[str]) -> str:
    """Render a definition that is an expression rather than a LAMBDA (the About tables)."""
    return render("LAMBDA(" + body + ")", library)[len("_xlfn.LAMBDA(") : -1].strip()


# --------------------------------------------------------------------------- #
# The workbook
# --------------------------------------------------------------------------- #


def shipped_names(book: str) -> dict[str, tuple[str, str]]:
    """Qualified name -> (attribute text, unescaped body) for every oz. defined name."""
    return {
        name: (attrs, html.unescape(body))
        for name, attrs, body in NAME_RE.findall(book)
    }


def apply(book: str, compiled: list[Compiled]) -> tuple[str, list[str]]:
    """Write each changed rendering over its defined name; report what changed."""
    shipped = shipped_names(book)
    missing = sorted(set(shipped) - {c.name for c in compiled})
    if missing:
        raise ValueError(
            "%s ship in the workbook but are not declared in src/" % ", ".join(missing)
        )
    changed: list[str] = []
    for item in compiled:
        if item.name not in shipped:
            raise ValueError(f"{item.name} is declared in src/ but does not ship; add it first")
        attrs, current = shipped[item.name]
        old_comment = re.search(r' comment="([^"]*)"', attrs)
        same_comment = old_comment is not None and html.unescape(old_comment.group(1)) == item.comment
        if tight(current) == tight(item.stored) and same_comment:
            continue
        element = f'<definedName name="{item.name}"{attrs}>'
        start = book.index(element)
        end = book.index("</definedName>", start)
        new_attrs = re.sub(
            r' comment="[^"]*"', "", attrs
        ) + f' comment="{xml_escape(item.comment).replace(chr(34), "&quot;")}"'
        body = xml_escape(item.stored) if tight(current) != tight(item.stored) else book[start + len(element):end]
        book = book[:start] + f'<definedName name="{item.name}"{new_attrs}>' + body + book[end:]
        changed.append(item.name)
    return book, changed


def update_index(path: Path, book: str, modules: dict[str, str]) -> bool:
    """Rewrite functions.csv from the compiled workbook, keeping previous_name."""
    with path.open(encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle)
        fieldnames = list(reader.fieldnames or [])
        previous = {row["function"]: row.get("previous_name", "") for row in reader}
    if fieldnames != ["function", "module", "previous_name", "signature", "description"]:
        raise ValueError(f"{path} has unexpected columns: {fieldnames}")
    rows = []
    for name, (attrs, body) in shipped_names(book).items():
        signature, description = index_fields(attrs, body, name.rsplit(".", 1)[1])
        rows.append(
            {
                "function": name,
                "module": modules.get(name, ""),
                "previous_name": previous.get(name, ""),
                "signature": signature,
                "description": description,
            }
        )
    rows.sort(key=lambda row: row["function"])
    with path.open(encoding="utf-8-sig", newline="") as handle:
        before = handle.read()
    buffer = io.StringIO()
    writer = csv.DictWriter(buffer, fieldnames=fieldnames, lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    after = buffer.getvalue()
    if after == before:
        return False
    with path.open("w", encoding="utf-8", newline="") as handle:
        handle.write(after)
    return True


def run(workbook: Path, src: Path, check: bool, index: Path | None) -> list[str]:
    compiled = compile_sources(src)
    with zipfile.ZipFile(workbook) as archive:
        parts = {name: archive.read(name) for name in archive.namelist()}
    book = parts["xl/workbook.xml"].decode("utf-8")
    new_book, changed = apply(book, compiled)
    if check:
        return changed
    if changed:
        parts["xl/workbook.xml"] = new_book.encode("utf-8")
        write_deterministic(workbook, parts)
    if index is not None:
        modules = {c.name: c.module for c in compiled}
        if update_index(index, new_book, modules):
            changed.append(index.name)
    return changed


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    flags = [a for a in sys.argv[1:] if a.startswith("--")]
    index: Path | None = None
    check = False
    for flag in flags:
        if flag == "--check":
            check = True
        elif flag.startswith("--index="):
            index = Path(flag[len("--index="):])
        else:
            sys.exit(
                "FAIL: usage: python tools/compile_sources.py [workbook] [src dir] "
                "[--check] [--index=functions.csv]"
            )
    if len(args) > 2:
        sys.exit("FAIL: usage: python tools/compile_sources.py [workbook] [src dir] [--check]")
    workbook = Path(args[0] if args else "ozzit.xlsx")
    src = Path(args[1] if len(args) > 1 else "src")
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changed = run(workbook, src, check, index)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    if check:
        if changed:
            print("would recompile: " + ", ".join(changed))
        else:
            print("every defined name already matches src/")
        print(f"OK: {workbook} checked against {src}/")
        return
    if changed:
        print("recompiled: " + ", ".join(changed))
    else:
        print("every defined name already matches src/; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
