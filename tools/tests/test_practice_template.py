"""Bind the companion's saved native result to its workbook and test inputs.

This checks retained evidence, not Excel calculation on this host. Run the
PowerShell test in Excel to refresh evidence after a material input changes.
"""

import hashlib
import json
import unittest
import zipfile
from pathlib import Path
from xml.etree import ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
NS = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}


def luminance(rgb):
    channels = [int(rgb[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    linear = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in channels]
    return sum(c * weight for c, weight in zip(linear, (0.2126, 0.7152, 0.0722)))


def contrast(foreground, background):
    light, dark = sorted((luminance(foreground), luminance(background)), reverse=True)
    return (light + 0.05) / (dark + 0.05)


class PracticeTemplateTests(unittest.TestCase):
    def test_native_receipt_matches_saved_inputs(self):
        receipt = json.loads((ROOT / "docs/practice-native-evidence.json").read_text("utf-8-sig"))
        for relative, field in (
            ("templates/practice-calculators.xlsx", "workbook_sha256"),
            ("tools/excel_practice_selftest.ps1", "runner_sha256"),
            ("tools/build_practice_calculators.mjs", "builder_sha256"),
            ("tools/prepare_practice_template.ps1", "preparer_sha256"),
        ):
            with self.subTest(path=relative):
                actual = hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()
                self.assertEqual(actual, receipt[field], "rerun native Excel verification")
        self.assertEqual(receipt["result"], "PASS")
        self.assertEqual(receipt["baseline_errors"], 0)
        self.assertGreater(receipt["assertions"], 0)
        self.assertIn("build", receipt["excel"])
        with zipfile.ZipFile(ROOT / "templates/practice-calculators.xlsx") as archive:
            workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        names = {s.attrib["name"] for s in workbook.findall("s:sheets/s:sheet", NS)}
        self.assertEqual(names - {"Read me"}, set(receipt["workflows"]))

    def test_companion_has_no_executable_or_external_workbook_parts(self):
        with zipfile.ZipFile(ROOT / "templates/practice-calculators.xlsx") as archive:
            names = archive.namelist()
            self.assertFalse(any("vbaproject" in p.lower() for p in names))
            self.assertFalse(any(p.startswith("xl/externalLinks/") for p in names))
            content_types = archive.read("[Content_Types].xml").decode()
            self.assertNotIn("macroEnabled", content_types)
            # Images and hyperlinks would be legitimate, but this companion
            # deliberately needs no external relationships at all.
            for name in names:
                if name.endswith(".rels"):
                    relations = ET.fromstring(archive.read(name))
                    self.assertFalse(any(r.attrib.get("TargetMode") == "External" for r in relations))

    def test_native_navigation_targets_existing_calculators(self):
        relationship_ns = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
        with zipfile.ZipFile(ROOT / "templates/practice-calculators.xlsx") as archive:
            book = ET.fromstring(archive.read("xl/workbook.xml"))
            rels = ET.fromstring(archive.read("xl/_rels/workbook.xml.rels"))
            targets = {r.attrib["Id"]: r.attrib["Target"] for r in rels}
            sheets = book.findall("s:sheets/s:sheet", NS)
            for sheet in sheets:
                name = sheet.attrib["name"]
                target = targets[sheet.attrib[f"{{{relationship_ns}}}id"]]
                part = target.lstrip("/") if target.startswith("/") else "xl/" + target
                xml = ET.fromstring(archive.read(part))
                links = xml.findall("s:hyperlinks/s:hyperlink", NS)
                actual = {(link.attrib["ref"], link.attrib["location"]) for link in links}
                if name == "Read me":
                    expected = {(f"B{14 + i}", f"'{s.attrib['name']}'!B2")
                                for i, s in enumerate(sheets[1:])}
                else:
                    expected = {("B1", "'Read me'!B2")}
                self.assertEqual(actual, expected, name)

    def test_text_contrast_on_every_populated_cell_and_conditional_style(self):
        with zipfile.ZipFile(ROOT / "templates/practice-calculators.xlsx") as archive:
            styles = ET.fromstring(archive.read("xl/styles.xml"))
            fonts = styles.findall("s:fonts/s:font", NS)
            fills = styles.findall("s:fills/s:fill", NS)
            formats = styles.findall("s:cellXfs/s:xf", NS)

            def colour(node, fallback):
                if node is None:
                    return fallback
                # The companion deliberately exports explicit colours. Unknown
                # theme/indexed colours need review rather than a guessed pass.
                self.assertIn("rgb", node.attrib)
                return node.attrib["rgb"][-6:]

            for part in archive.namelist():
                if not part.startswith("xl/worksheets/sheet") or not part.endswith(".xml"):
                    continue
                sheet = ET.fromstring(archive.read(part))
                for cell in sheet.findall(".//s:c", NS):
                    value = cell.find("s:v", NS)
                    if value is None or value.text in (None, ""):
                        continue
                    style = formats[int(cell.attrib.get("s", "0"))]
                    font = fonts[int(style.attrib.get("fontId", "0"))]
                    fill = fills[int(style.attrib.get("fillId", "0"))]
                    foreground = colour(font.find("s:color", NS), "000000")
                    background = colour(fill.find("s:patternFill/s:fgColor", NS), "FFFFFF")
                    size = font.find("s:sz", NS)
                    points = float(size.attrib["val"]) if size is not None else 11
                    large = points >= 18 or (points >= 14 and font.find("s:b", NS) is not None)
                    with self.subTest(cell=f"{part}!{cell.attrib['r']}"):
                        self.assertGreaterEqual(contrast(foreground, background), 3 if large else 4.5)
            for index, style in enumerate(styles.findall("s:dxfs/s:dxf", NS)):
                font = style.find("s:font/s:color", NS)
                fill = style.find("s:fill/s:patternFill/s:fgColor", NS)
                if fill is None:
                    fill = style.find("s:fill/s:patternFill/s:bgColor", NS)
                if font is not None and fill is not None:
                    with self.subTest(conditional_style=index):
                        self.assertGreaterEqual(contrast(colour(font, "000000"), colour(fill, "FFFFFF")), 4.5)


if __name__ == "__main__":
    unittest.main()
