"""Residue pass: parts the workbook carries that nothing in it reads.

Usage: python tools/postbuild/remove_residue.py [workbook]

Five kinds of residue, all inherited from the upstream workbook or from the
add-ins that once edited it, and one presentation rule:

    FMTs           a hidden worksheet holding a 134-row table named Skin. Its
                   validation prompts describe a format set that a VBA styler
                   read, and there is no VBA here: no formula, name, hyperlink
                   or table references the sheet.
    customProperty 38 per-sheet Description properties, one .bin part each,
                   readable only from VBA and repeating the strapline every
                   sheet already shows in A2.
    custom functions
                   the Excel Labs web-extension part declares that the workbook
                   contains custom functions and names the LABS.GENERATIVEAI
                   id. No cell calls it. The reference itself stays, because it
                   is what binds the Advanced Formula Environment store.
    dxfs           differential formats that no conditional format or table
                   style uses, and named cell styles that no cell format uses.
                   Both are renumbered where they are referenced.
    freeze panes   the six demonstration sheets that run to hundreds of columns
                   freeze the columns up to their first data column, so the row
                   labels stay in view while the timeline scrolls.

Every edit is anchored and counted, so a second run reports "already applied"
and writes nothing, and a workbook in neither recognised state fails loudly.
Removing a worksheet renumbers the print areas that follow it, drops the
worksheet from the file-properties list, and leaves every cached value alone.
Excel keeps its calculation chain keyed by sheetId, which does not change.
Pure text surgery: no COM, no recalculation.
"""

from __future__ import annotations

import re
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from sanitise_workbook import write_deterministic  # noqa: E402

RESIDUE_SHEET = "FMTs"
CUSTOM_FUNCTIONS = re.compile(r"<we:extLst>.*?</we:extLst>", re.DOTALL)
# sheet name -> first data column, one past the frozen block
FREEZE = {
    "oz.IsOccurrenceDateλ": "H",
    "oz.Amortiseλ": "I",
    "oz.SumAmortiseλ": "I",
    "oz.Depreciateλ": "L",
    "oz.SumPeriodsλ": "I",
    "oz.TimelineOffsetλ": "C",
}


def column_number(letters: str) -> int:
    n = 0
    for ch in letters:
        n = n * 26 + ord(ch) - 64
    return n


def sheet_targets(parts: dict[str, bytes]) -> dict[str, str]:
    """Sheet name -> worksheet part, from the workbook and its relationships."""
    book = parts["xl/workbook.xml"].decode("utf-8")
    rels = parts["xl/_rels/workbook.xml.rels"].decode("utf-8")
    targets = dict(re.findall(r'<Relationship Id="([^"]+)"[^>]*Target="([^"]+)"', rels))
    out = {}
    for name, rid in re.findall(r'<sheet name="([^"]+)"[^>]*r:id="([^"]+)"', book):
        out[name] = "xl/" + targets[rid].lstrip("/")
    return out


# --------------------------------------------------------------------------- #
# The hidden sheet
# --------------------------------------------------------------------------- #


def drop_sheet(parts: dict[str, bytes], name: str, log: list[str]) -> None:
    book = parts["xl/workbook.xml"].decode("utf-8")
    sheets = re.findall(r"<sheet\b[^>]*/>", book)
    position = next((i for i, s in enumerate(sheets) if f'name="{name}"' in s), None)
    if position is None:
        return
    element = sheets[position]
    rid = re.search(r'r:id="([^"]+)"', element).group(1)  # type: ignore[union-attr]
    book = book.replace(element, "", 1)
    # Print areas and other sheet-scoped names index sheets by position.
    book = re.sub(
        r'localSheetId="(\d+)"',
        lambda m: 'localSheetId="%d"' % (int(m.group(1)) - (1 if int(m.group(1)) > position else 0)),
        book,
    )
    parts["xl/workbook.xml"] = book.encode("utf-8")

    rels = parts["xl/_rels/workbook.xml.rels"].decode("utf-8")
    relationship = re.search(rf'<Relationship Id="{rid}"[^>]*/>', rels)
    if relationship is None:
        raise ValueError(f"{name}: relationship {rid} missing")
    target = "xl/" + re.search(r'Target="([^"]+)"', relationship.group(0)).group(1).lstrip("/")  # type: ignore[union-attr]
    parts["xl/_rels/workbook.xml.rels"] = rels.replace(relationship.group(0), "", 1).encode("utf-8")

    sheet_rels = target.replace("worksheets/", "worksheets/_rels/") + ".rels"
    tables = []
    if sheet_rels in parts:
        tables = re.findall(r'Target="\.\./tables/([^"]+)"', parts[sheet_rels].decode("utf-8"))
        del parts[sheet_rels]
    del parts[target]
    for table in tables:
        del parts["xl/tables/" + table]

    content_types = parts["[Content_Types].xml"].decode("utf-8")
    content_types = re.sub(rf'<Override PartName="/{re.escape(target)}"[^>]*/>', "", content_types)
    for table in tables:
        content_types = re.sub(rf'<Override PartName="/xl/tables/{re.escape(table)}"[^>]*/>', "", content_types)
    parts["[Content_Types].xml"] = content_types.encode("utf-8")

    app = parts["docProps/app.xml"].decode("utf-8")
    needle = f"<vt:lpstr>{name}</vt:lpstr>"
    if app.count(needle) != 1:
        raise ValueError(f"docProps/app.xml lists {name} {app.count(needle)} times, expected 1")
    app = app.replace(needle, "", 1)
    app = re.sub(
        r"(<vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>)(\d+)(</vt:i4>)",
        lambda m: f"{m.group(1)}{int(m.group(2)) - 1}{m.group(3)}",
        app,
    )
    app = re.sub(
        r'(<TitlesOfParts><vt:vector size=")(\d+)(")',
        lambda m: f"{m.group(1)}{int(m.group(2)) - 1}{m.group(3)}",
        app,
    )
    parts["docProps/app.xml"] = app.encode("utf-8")
    log.append(f"removed the hidden {name} sheet, its table and its {len(tables)} table part(s)")


# --------------------------------------------------------------------------- #
# Custom properties and the custom-function declaration
# --------------------------------------------------------------------------- #


def drop_custom_properties(parts: dict[str, bytes], log: list[str]) -> None:
    bins = [n for n in parts if re.fullmatch(r"xl/customProperty\d+\.bin", n)]
    if not bins:
        return
    for name in bins:
        del parts[name]
    for name in [n for n in parts if n.startswith("xl/worksheets/_rels/")]:
        text = parts[name].decode("utf-8")
        text = re.sub(r'<Relationship [^>]*Target="\.\./customProperty\d+\.bin"[^>]*/>', "", text)
        if re.search(r"<Relationships[^>]*>\s*</Relationships>", text):
            del parts[name]
        else:
            parts[name] = text.encode("utf-8")
    for name in [n for n in parts if re.fullmatch(r"xl/worksheets/sheet\d+\.xml", n)]:
        text = parts[name].decode("utf-8")
        new = re.sub(r"<customProperties>\s*(?:<customPr [^>]*/>\s*)+</customProperties>", "", text)
        if new != text:
            parts[name] = new.encode("utf-8")
    content_types = parts["[Content_Types].xml"].decode("utf-8")
    content_types = re.sub(r'<Default Extension="bin"[^>]*/>', "", content_types)
    parts["[Content_Types].xml"] = content_types.encode("utf-8")
    log.append(f"removed {len(bins)} per-sheet custom properties")


def drop_custom_function_declaration(parts: dict[str, bytes], log: list[str]) -> None:
    name = "xl/webextensions/webextension1.xml"
    if name not in parts:
        return
    text = parts[name].decode("utf-8")
    new, hits = CUSTOM_FUNCTIONS.subn("", text)
    if hits:
        parts[name] = new.encode("utf-8")
        log.append("removed the stale custom-function declaration from the Excel Labs reference")


# --------------------------------------------------------------------------- #
# Styles
# --------------------------------------------------------------------------- #


def prune_styles(parts: dict[str, bytes], log: list[str]) -> None:
    styles = parts["xl/styles.xml"].decode("utf-8")
    sheets = {n: parts[n].decode("utf-8") for n in parts if re.fullmatch(r"xl/worksheets/sheet\d+\.xml", n)}

    # Differential formats: referenced by conditional formats and table styles.
    dxfs_match = re.search(r'<dxfs count="(\d+)">(.*?)</dxfs>', styles, re.DOTALL)
    if dxfs_match is None:
        raise ValueError("styles.xml has no dxfs collection")
    entries = re.findall(r"<dxf>.*?</dxf>|<dxf/>", dxfs_match.group(2), re.DOTALL)
    if len(entries) != int(dxfs_match.group(1)):
        raise ValueError("styles.xml dxfs count disagrees with its entries")
    table_styles = re.search(r"<tableStyles\b.*?</tableStyles>", styles, re.DOTALL)
    used = set()
    for text in list(sheets.values()) + ([table_styles.group(0)] if table_styles else []):
        used |= {int(x) for x in re.findall(r'dxfId="(\d+)"', text)}
    if used and max(used) >= len(entries):
        raise ValueError("a dxfId points past the dxfs collection")
    keep = sorted(used)
    if len(keep) != len(entries):
        renumber = {old: new for new, old in enumerate(keep)}

        def rewrite(text: str) -> str:
            return re.sub(r'dxfId="(\d+)"', lambda m: 'dxfId="%d"' % renumber[int(m.group(1))], text)

        for name, text in sheets.items():
            new = rewrite(text)
            if new != text:
                parts[name] = new.encode("utf-8")
                sheets[name] = new
        if table_styles:
            styles = styles.replace(table_styles.group(0), rewrite(table_styles.group(0)), 1)
        styles = styles.replace(
            dxfs_match.group(0),
            f'<dxfs count="{len(keep)}">' + "".join(entries[i] for i in keep) + "</dxfs>",
            1,
        )
        log.append(f"dropped {len(entries) - len(keep)} unused differential formats")

    # Named cell styles: referenced by the xfId of each cell format.
    xfs_match = re.search(r'<cellStyleXfs count="(\d+)">(.*?)</cellStyleXfs>', styles, re.DOTALL)
    names_match = re.search(r'<cellStyles count="(\d+)">(.*?)</cellStyles>', styles, re.DOTALL)
    cell_xfs = re.search(r"<cellXfs\b.*?</cellXfs>", styles, re.DOTALL)
    if xfs_match is None or names_match is None or cell_xfs is None:
        raise ValueError("styles.xml lacks a cell style collection")
    style_xfs = re.findall(r"<xf\b[^>]*/>|<xf\b[^>]*>.*?</xf>", xfs_match.group(2), re.DOTALL)
    style_names = re.findall(r"<cellStyle\b[^>]*/>", names_match.group(2))
    used_xf = {0} | {int(x) for x in re.findall(r'xfId="(\d+)"', cell_xfs.group(0))}
    if max(used_xf) >= len(style_xfs):
        raise ValueError("an xfId points past the cellStyleXfs collection")
    keep_xf = sorted(used_xf)
    if len(keep_xf) != len(style_xfs):
        renumber_xf = {old: new for new, old in enumerate(keep_xf)}
        kept_names = []
        for element in style_names:
            xf_id = int(re.search(r'xfId="(\d+)"', element).group(1))  # type: ignore[union-attr]
            if xf_id in renumber_xf:
                kept_names.append(element.replace(f'xfId="{xf_id}"', f'xfId="{renumber_xf[xf_id]}"', 1))
        new_cell_xfs = re.sub(
            r'xfId="(\d+)"', lambda m: 'xfId="%d"' % renumber_xf[int(m.group(1))], cell_xfs.group(0)
        )
        styles = styles.replace(cell_xfs.group(0), new_cell_xfs, 1)
        styles = styles.replace(
            xfs_match.group(0),
            f'<cellStyleXfs count="{len(keep_xf)}">' + "".join(style_xfs[i] for i in keep_xf) + "</cellStyleXfs>",
            1,
        )
        styles = styles.replace(
            names_match.group(0),
            f'<cellStyles count="{len(kept_names)}">' + "".join(kept_names) + "</cellStyles>",
            1,
        )
        log.append(f"dropped {len(style_xfs) - len(keep_xf)} unused named cell styles")
    parts["xl/styles.xml"] = styles.encode("utf-8")


# --------------------------------------------------------------------------- #
# Freeze panes
# --------------------------------------------------------------------------- #


def freeze_panes(parts: dict[str, bytes], log: list[str]) -> None:
    targets = sheet_targets(parts)
    frozen = 0
    for sheet, column in FREEZE.items():
        part = targets.get(sheet)
        if part is None:
            raise ValueError(f"{sheet} is not in the workbook")
        text = parts[part].decode("utf-8")
        if "<pane " in text:
            continue
        view = re.search(r"<sheetView\b[^>]*>", text)
        if view is None:
            raise ValueError(f"{sheet} has no sheetView")
        pane = (
            f'<pane xSplit="{column_number(column) - 1}" topLeftCell="{column}1" '
            'activePane="topRight" state="frozen"/>'
        )
        selection = f'<selection pane="topRight" activeCell="{column}1" sqref="{column}1"/>'
        text = text.replace(view.group(0), view.group(0) + pane, 1)
        text = text.replace("</sheetView>", selection + "</sheetView>", 1)
        parts[part] = text.encode("utf-8")
        frozen += 1
    if frozen:
        log.append(f"froze the label columns on {frozen} wide demonstration sheets")


# --------------------------------------------------------------------------- #


def run(workbook: Path) -> list[str]:
    with zipfile.ZipFile(workbook) as archive:
        parts = {n: archive.read(n) for n in archive.namelist()}
    log: list[str] = []
    drop_sheet(parts, RESIDUE_SHEET, log)
    drop_custom_properties(parts, log)
    drop_custom_function_declaration(parts, log)
    prune_styles(parts, log)
    freeze_panes(parts, log)
    if log:
        write_deterministic(workbook, parts)
    return log


def main() -> None:
    if len(sys.argv) > 2:
        sys.exit("FAIL: usage: python tools/postbuild/remove_residue.py [workbook]")
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        log = run(workbook)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    for line in log:
        print(line)
    if not log:
        print("residue already removed; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
