"""Workbook palette pass: theme, styles, sheets and drawings to the Mermaid palette.

Usage: python tools/postbuild/workbook_palette.py [--template] [workbook]

Palette (the repository's Mermaid diagram and badge colours) and roles:
 #000000 black - section headings, emphasis labels, header bands, dates tabs
 #FFFFF0 ivory - text on dark fills, help-block and header-label fills
 #E5E4E2 platinum - input cells, reference fills, structure tabs (Cover, TOC)
 #66023C Tyrian purple - the accent: titles, links, function cells, financial tabs
 #990024 crimson - warnings and failed checks, followed links, essentials tabs
 #0F4B32 Rhodesian green - passed checks only
Function cells, warnings and passed checks use 15% tints of their colour as fills
(E8D9E2, F0D9DE, DBE4E0). Neutral greys (6E6862 straplines, A6A6A6, D9D9D9) and
the modelling font conventions in the templates (blue inputs, green cross-sheet
links) are kept: the palette has no mid grey for text on white, and the font
conventions carry meaning that a palette colour would lose.

Three sources merged:
 phase_c2 - explicit colour map, teal/salmon hue remap, font-family consolidation
 phase_e - help-label greens/blues/maroon folded to the accent / warning crimson
 phase_f - mint help-block fill folded to ivory
The v3.1.0 purple palette (#5C2D91 and its family) maps onto the roles above.

`--template` applies only the colour roles, for the workbooks in templates/: it
skips the Ozzit-specific font folds, input fill and TOC guard.

Idempotent on the current workbook: a second run finds nothing to do and writes
nothing. A fixed-point guard refuses to write if any colour in the result is still
remappable, so a future in-band addition to the palette cannot drift on reruns.

One honest limitation: the band remap is the v3.1.0 formula, which is not a fixed
point for high-saturation salmon inputs (the output can stay above the band's
saturation gate). The guard runs inside recolour, before the workbook is written, so
such an input is rejected on its first run and nothing is written at all, rather than
being converted once and refused on a later pass. No v3.0.0 workbook is available to
show how that input behaved there, so this is documented rather than asserted.

Pure XML surgery: no COM, no recalculation.
"""

from __future__ import annotations

import colorsys
import re
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from sanitise_workbook import write_deterministic
from workbook import read_parts

BLACK = "000000"
IVORY = "FFFFF0"
PLATINUM = "E5E4E2"
TYRIAN = "66023C"
CRIMSON = "990024"
GREEN = "0F4B32"
TYRIAN_TINT = "E8D9E2"
CRIMSON_TINT = "F0D9DE"
GREEN_TINT = "DBE4E0"

# Each v3.1.0 palette colour, and each colour the templates used, to its role.
MERMAID = {
    "5C2D91": TYRIAN,
    "38206B": TYRIAN,
    "4F485E": TYRIAN,  # secondary text; as a tab colour, TAB_COLOURS wins
    "DED9E8": TYRIAN_TINT,
    "04001F": BLACK,
    "2B2733": BLACK,
    "2B1A54": BLACK,
    "243747": BLACK,
    "7A5AB5": CRIMSON,
    "C00000": CRIMSON,
    "FF0000": CRIMSON,
    "9B2925": CRIMSON,
    "7B4B00": CRIMSON,
    "FFCCCC": CRIMSON_TINT,
    "FCE4D6": CRIMSON_TINT,
    "FDE9E7": CRIMSON_TINT,
    "FFF0D4": CRIMSON_TINT,
    "E2F0D9": GREEN_TINT,
    "F3F1F6": IVORY,
    "F3F1F5": IVORY,
    "B1AFAD": PLATINUM,
    "E7E4EB": PLATINUM,
    "E8E4EC": PLATINUM,
    "DDDBE0": PLATINUM,
    "D0CAD7": PLATINUM,
    "E2E0DF": PLATINUM,
    "DAD9D9": PLATINUM,
    "ECECEC": PLATINUM,
    "71808C": "6E6862",
}

# The legacy v3.0.0 colours, each sent straight to its final role.
LEGACY = {
    "3F656F": "5C2D91",
    "66A1AF": "4F485E",
    "B1D2DA": "DED9E8",
    "92563E": "6E6862",
    "D7876B": "B1AFAD",
    "EFBEAB": "DCDAD7",
    "44546A": "04001F",
    "2E4950": "2B2733",
    "0563C1": "5C2D91",
    "954F72": "7A5AB5",
    "595959": "6E6862",
    "1F4E79": "2B1A54",
    "157A5F": "5C2D91",
    "2E75B6": "7A5AB5",
    "375623": "38206B",
}
EXPLICIT = {old: MERMAID.get(new, new) for old, new in LEGACY.items()} | MERMAID

# Tab colours by module: structure platinum, dates black, financial and ratios
# Tyrian purple, array essentials crimson.
TAB_COLOURS = {
    "4F485E": PLATINUM,
    "2B1A54": BLACK,
    "5C2D91": TYRIAN,
    "38206B": TYRIAN,
    "7A5AB5": CRIMSON,
}

# Theme slots, so Excel's colour picker, built-in styles, hyperlinks and charts
# offer the same palette. dk1 and lt1 stay the system window colours.
THEME = {
    "dk2": BLACK,
    "lt2": IVORY,
    "accent1": TYRIAN,
    "accent2": CRIMSON,
    "accent3": PLATINUM,
    "accent4": "6E6862",
    "accent5": "B1AFAD",
    "accent6": "DCDAD7",
    "hlink": TYRIAN,
    "folHlink": CRIMSON,
}

# Colours the hue remap must never touch: the palette and its tints, neutrals,
# the template font conventions, and the legacy accents that phase e/f own.
KEEP = {
    BLACK, IVORY, PLATINUM, TYRIAN, CRIMSON, GREEN, TYRIAN_TINT, CRIMSON_TINT, GREEN_TINT,
    "FFFFFF", "A6A6A6", "D9D9D9", "F8F8F8", "FA7D00", "6E6862", "DCDAD7", "F5F5F5",
    "CCCCCC", "008000", "2455A4",
    "006600", "00C000", "0000FF", "0000C0", "800000", "CCFFCC", "FFFFCC",
}

# phase_e: help-label fonts folded to the accent (or the warning crimson).
FONT_REMAP = {
    "FF006600": f"FF{TYRIAN}",
    "FF00C000": f"FF{TYRIAN}",
    "FF0000FF": f"FF{TYRIAN}",
    "FF0000C0": f"FF{TYRIAN}",
    "FF800000": f"FF{CRIMSON}",
}

# phase_f: legacy fills folded to the help-block ivory.
FILL_REMAP = {
    "FFCCFFCC": f"FF{IVORY}",
    "FFFFFFCC": f"FF{IVORY}",
}

# The Ozzit input cells: a 5% shade of the theme background becomes platinum.
INPUT_FILL = '<fgColor theme="0" tint="-4.9989318521683403E-2"/>'

FONT_FAMILIES = {
    '<name val="Roboto Condensed"/>': '<name val="Aptos Display"/>',
    '<name val="Bahnschrift SemiCondensed"/>': '<name val="Aptos"/>',
    '<name val="Arial"/>': '<name val="Aptos"/>',
    '<name val="Aptos Narrow"/>': '<name val="Aptos"/>',
}

# Rich-text runs in sharedStrings.xml use rFont, not name.
RFONT_FAMILIES = {
    '<rFont val="Roboto Condensed"/>': '<rFont val="Aptos Display"/>',
    '<rFont val="Bahnschrift SemiCondensed"/>': '<rFont val="Aptos"/>',
    '<rFont val="Arial"/>': '<rFont val="Aptos"/>',
    '<rFont val="Aptos Narrow"/>': '<rFont val="Aptos"/>',
}


def hue_map(hexrgb: str) -> str | None:
    r, g, b = (int(hexrgb[i : i + 2], 16) / 255 for i in (0, 2, 4))
    h, lightness, s = colorsys.rgb_to_hls(r, g, b)
    deg = h * 360
    if s < 0.03:
        return None
    if 150 <= deg <= 225 and s > 0.05:
        h2, s2 = 325 / 360, min(0.35, s * 0.45)  # Tyrian purple's hue
    elif 5 <= deg <= 40 and s > 0.10:
        h2, s2 = 28 / 360, s * 0.22
    else:
        return None
    r2, g2, b2 = colorsys.hls_to_rgb(h2, lightness, s2)
    return f"{round(r2 * 255):02X}{round(g2 * 255):02X}{round(b2 * 255):02X}"


def map_colour(hexrgb: str) -> str | None:
    up = hexrgb.upper()
    if up in KEEP or up in EXPLICIT.values():
        return None
    if up in EXPLICIT:
        return EXPLICIT[up]
    return hue_map(up)


def recolour(text: str) -> tuple[str, int]:
    count = 0

    def rep8(match: re.Match[str]) -> str:
        nonlocal count
        pre, argb = match.group(1), match.group(2)
        alpha, rgb = argb[:2], argb[2:]
        new = map_colour(rgb)
        if new is None:
            return match.group(0)
        count += 1
        return f'{pre}"{alpha}{new}"'

    def rep6(match: re.Match[str]) -> str:
        nonlocal count
        pre, rgb = match.group(1), match.group(2)
        new = map_colour(rgb)
        if new is None:
            return match.group(0)
        count += 1
        return f'{pre}"{new}"'

    text = re.sub(r'(rgb=)"([0-9A-Fa-f]{8})"', rep8, text)
    text = re.sub(r'(val=)"([0-9A-Fa-f]{6})"(?=/>|")', rep6, text)
    # Fixed-point guard: nothing in the result may still be remappable. The band
    # remap lowers saturation below the input gate by design, so a colour that is
    # still remappable after one pass would drift on every subsequent run.
    residual = re.findall(r'rgb="[0-9A-Fa-f]{8}"', text) + re.findall(
        r'val="[0-9A-Fa-f]{6}"(?=/>|")', text
    )  # invariant: these 2 shapes are exactly what rep8/rep6 write. Add a
    # substitution shape elsewhere and this scan must grow the same pattern.
    for token in residual:
        hexv = token.split('"')[1]
        rgb = hexv[2:] if len(hexv) == 8 else hexv
        if map_colour(rgb) is not None:
            raise ValueError(
                f"remap is not a fixed point: {rgb} is still remappable after one pass"
            )
    return text, count


def _section(text: str, tag: str) -> tuple[str, str, str]:
    start = text.index(f"<{tag}")
    end = text.index(f"</{tag}>") + len(f"</{tag}>")
    return text[:start], text[start:end], text[end:]


def theme_slots(text: str) -> str:
    for slot, rgb in THEME.items():
        new = f'<a:{slot}><a:srgbClr val="{rgb}"/></a:{slot}>'
        text = re.sub(rf"<a:{slot}>.*?</a:{slot}>", new, text, flags=re.S)
    # dk1 and lt1 keep the system window colours; a fixed text colour becomes black.
    return re.sub(
        r'<a:dk1><a:srgbClr val="[0-9A-Fa-f]{6}"/></a:dk1>', f'<a:dk1><a:srgbClr val="{BLACK}"/></a:dk1>', text
    )


def tab_colours(text: str) -> tuple[str, int]:
    def rep(match: re.Match[str]) -> str:
        return f'{match.group(1)}{TAB_COLOURS[match.group(2).upper()]}"'

    return re.subn(r'(<(?:\w+:)?tabColor rgb="FF)(' + "|".join(TAB_COLOURS) + ')"', rep, text, flags=re.I)


# Element shapes with or without a namespace prefix: the retention template's
# writer prefixes every SpreadsheetML element with x:.
P = r"(?:\w+:)?"


def ivory_text(styles: str) -> tuple[str, int]:
    """White text only ever sits on a dark fill; make it ivory, in fonts and dxfs."""
    count = 0

    def rep(match: re.Match[str]) -> str:
        nonlocal count
        count += match.group(0).count('rgb="FFFFFFFF"')
        return match.group(0).replace('rgb="FFFFFFFF"', f'rgb="FF{IVORY}"')

    return re.sub(rf"<{P}font>.*?</{P}font>", rep, styles, flags=re.S), count


def passed_green(styles: str) -> tuple[str, int]:
    """Show passed checks in Rhodesian green.

    In the templates, plain green 008000 text marks cross-sheet links and is kept.
    Green text on the passed tint is a passed check: in conditional formats it is
    recoloured in place, and a cell style whose green font is shared with links
    gets its own copy of that font.
    """
    count = 0
    old, new = 'rgb="FF008000"', f'rgb="FF{GREEN}"'

    def dxf(match: re.Match[str]) -> str:
        nonlocal count
        if f"FF{GREEN_TINT}" not in match.group(0):
            return match.group(0)
        count += match.group(0).count(old)
        return match.group(0).replace(old, new)

    styles = re.sub(rf"<{P}dxf>.*?</{P}dxf>", dxf, styles, flags=re.S)

    fonts_match = re.search(rf"<{P}fonts\b.*?</{P}fonts>", styles, re.S)
    fills_match = re.search(rf"<{P}fills\b.*?</{P}fills>", styles, re.S)
    xfs_match = re.search(rf"<{P}cellXfs\b.*?</{P}cellXfs>", styles, re.S)
    if not (fonts_match and fills_match and xfs_match):
        return styles, count
    fonts = re.findall(rf"<{P}font\s*/>|<{P}font>.*?</{P}font>", fonts_match.group(0), re.S)
    fills = re.findall(rf"<{P}fill\s*/>|<{P}fill>.*?</{P}fill>", fills_match.group(0), re.S)
    passed_fills = {i for i, fill in enumerate(fills) if f"FF{GREEN_TINT}" in fill}
    copies: dict[int, int] = {}

    def xf(match: re.Match[str]) -> str:
        nonlocal count
        font = re.search(r'fontId="(\d+)"', match.group(0))
        fill = re.search(r'fillId="(\d+)"', match.group(0))
        if not (font and fill) or int(fill.group(1)) not in passed_fills:
            return match.group(0)
        font_id = int(font.group(1))
        if old not in fonts[font_id]:
            return match.group(0)
        if font_id not in copies:
            copies[font_id] = len(fonts)
            fonts.append(fonts[font_id].replace(old, new))
        count += 1
        return match.group(0).replace(font.group(0), f'fontId="{copies[font_id]}"')

    xfs = re.sub(rf"<{P}xf\b[^>]*?(?:/>|>.*?</{P}xf>)", xf, xfs_match.group(0), flags=re.S)
    if not copies:
        return styles, count
    head = re.match(rf"<{P}fonts\b[^>]*>", fonts_match.group(0))
    assert head is not None
    tag = head.group(0)[1:].split()[0].rstrip(">")
    opening = re.sub(r'count="\d+"', f'count="{len(fonts)}"', head.group(0))
    new_fonts = opening + "".join(fonts) + f"</{tag}>"
    return (
        styles[: fonts_match.start()]
        + new_fonts
        + styles[fonts_match.end() : xfs_match.start()]
        + xfs
        + styles[xfs_match.end() :],
        count,
    )


def run(workbook: Path, template: bool = False) -> list[str]:
    parts = read_parts(workbook)
    order = list(parts)

    changes = []

    # Tab colours first: a tab's role differs from the same colour as text.
    tabs = 0
    for name in [n for n in order if re.match(r"xl/worksheets/sheet\d+\.xml$", n)]:
        text, count = tab_colours(parts[name].decode("utf-8"))
        if count:
            parts[name] = text.encode("utf-8")
            tabs += count
    if tabs:
        changes.append(f"tab colours set ({tabs} sheets)")

    for name in [n for n in order if re.match(r"xl/theme/theme\d+\.xml$", n)]:
        text = parts[name].decode("utf-8")
        slotted = theme_slots(text)
        if slotted != text:
            parts[name] = slotted.encode("utf-8")
            changes.append(f"{name}: theme colours set")

    # Main palette: styles, every worksheet, drawing and chart, and shared strings
    # (rich-text runs carry colour and rFont inside sharedStrings.xml).
    for name in ["xl/styles.xml", "xl/sharedStrings.xml"] + [
        n
        for n in order
        if re.match(r"xl/(worksheets/sheet|drawings/drawing|(drawings/)?charts/chart)\d+\.xml$", n)
    ]:
        if name not in parts:
            continue
        text = parts[name].decode("utf-8")
        recoloured, count = recolour(text)
        if recoloured != text:
            parts[name] = recoloured.encode("utf-8")
            changes.append(f"{name}: {count} colours remapped")

    styles = parts["xl/styles.xml"].decode("utf-8")
    for step, label in ((ivory_text, "white text made ivory"), (passed_green, "passed checks made green")):
        styles, count = step(styles)
        if count:
            changes.append(f"{label} ({count} entries)")
    parts["xl/styles.xml"] = styles.encode("utf-8")

    if template:
        if not changes:
            return []
        write_deterministic(workbook, parts)
        return changes

    # One drawing outlines its text box in preset green; give it the platinum
    # outline the other text boxes take from accent3.
    for name in [n for n in order if re.match(r"xl/drawings/drawing\d+\.xml$", n)]:
        text = parts[name].decode("utf-8")
        if '<a:prstClr val="green"/>' in text:
            parts[name] = text.replace('<a:prstClr val="green"/>', '<a:schemeClr val="accent3"/>').encode("utf-8")
            changes.append(f"{name}: green outline made platinum")

    # Font families + help-label fonts + help-block fill, in styles.xml only.

    for old, new in FONT_FAMILIES.items():
        if old in styles:
            styles = styles.replace(old, new)
            changes.append(f"font family {old} -> {new}")
    # Rich-text font names live in sharedStrings.xml as rFont.
    shared = parts["xl/sharedStrings.xml"].decode("utf-8")
    for old, new in RFONT_FAMILIES.items():
        if old in shared:
            shared = shared.replace(old, new)
            changes.append(f"rich-text font {old} -> {new}")
    parts["xl/sharedStrings.xml"] = shared.encode("utf-8")

    pre, fonts, post = _section(styles, "fonts")
    folded = 0
    for old, new in FONT_REMAP.items():
        hits = fonts.count(f'rgb="{old}"')
        if hits:
            fonts = fonts.replace(f'rgb="{old}"', f'rgb="{new}"')
            folded += hits
    if folded:
        changes.append(f"help-label fonts folded to palette ({folded} entries)")
    styles = pre + fonts + post

    pre, fills, post = _section(styles, "fills")
    folded = 0
    for old, new in FILL_REMAP.items():
        hits = fills.count(f'rgb="{old}"')
        if hits:
            fills = fills.replace(f'rgb="{old}"', f'rgb="{new}"')
            folded += hits
    # phase_f also caught the indexed mint fill left by earlier passes.
    hits = fills.count('<fgColor indexed="42"/>')
    if hits:
        fills = fills.replace('<fgColor indexed="42"/>', f'<fgColor rgb="FF{IVORY}"/>')
        folded += hits
    if folded:
        changes.append(f"help-block fills folded to ivory ({folded} entries)")
    hits = fills.count(INPUT_FILL)
    if hits:
        fills = fills.replace(INPUT_FILL, f'<fgColor rgb="FF{PLATINUM}"/>')
        changes.append(f"input fills made platinum ({hits} entries)")
    styles = pre + fills + post

    parts["xl/styles.xml"] = styles.encode("utf-8")

    # Guard: the TOC length warning keeps its warning role, now in crimson.
    dxfs = styles[styles.index("<dxfs") : styles.index("</dxfs>")]
    assert f"FF{CRIMSON_TINT}" in dxfs and f"FF{CRIMSON}" in dxfs, "TOC warning colours are missing"

    if not changes:
        return []
    write_deterministic(workbook, parts)
    return changes


def main() -> None:
    args = sys.argv[1:]
    template = args[:1] == ["--template"]
    if template:
        args = args[1:]
    if len(args) > 1:
        sys.exit("FAIL: usage: python tools/postbuild/workbook_palette.py [--template] [workbook]")
    workbook = Path(args[0] if args else "ozzit.xlsx")
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changes = run(workbook, template)
    except (OSError, ValueError, AssertionError, zipfile.BadZipFile) as exc:
        sys.exit(f"FAIL: {exc}")
    if changes:
        for change in changes:
            print(change)
    else:
        print("Palette already applied; no changes")
    print(f"OK: {workbook}")


if __name__ == "__main__":
    main()
