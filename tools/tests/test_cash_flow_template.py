import unittest
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKBOOK = ROOT / "templates" / "13-week-cash-flow-forecast.xlsx"
MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
NS = {"m": MAIN, "r": REL}
EXPECTED_SHEETS = [
    "Start Here",
    "Dashboard",
    "Assumptions",
    "13-Week Forecast",
    "Weekly Review",
    "Checks & Sources",
]
FORECAST_LABELS = {
    5: "Week commencing",
    6: "Week ending",
    7: "Actual / Forecast",
    9: "CASH RECEIPTS",
    19: "Total cash receipts",
    21: "CASH PAYMENTS",
    43: "Total cash payments",
    45: "CASH POSITION",
    48: "Closing cash balance",
    51: "Liquidity status",
}
PALETTE_ANCHORS = ("5C2D91", "04001F", "2B2733", "B1AFAD", "DED9E8", "F3F1F6", "7A5AB5", "C00000")
BANNED_SOURCE_TEXT = ("tradepoint", "1099", "federal tax", "state tax", "tradepointcfo")


def workbook_parts():
    with zipfile.ZipFile(WORKBOOK) as archive:
        return {name: archive.read(name) for name in archive.namelist()}


def sheet_paths(parts):
    workbook = ET.fromstring(parts["xl/workbook.xml"])
    relationships = ET.fromstring(parts["xl/_rels/workbook.xml.rels"])
    targets = {item.attrib["Id"]: item.attrib["Target"] for item in relationships}
    relationship_id = f"{{{REL}}}id"
    sheets = workbook.find("m:sheets", NS)
    return [
        (
            item.attrib["name"],
            targets[item.attrib[relationship_id]].lstrip("/")
            if targets[item.attrib[relationship_id]].lstrip("/").startswith("xl/")
            else "xl/" + targets[item.attrib[relationship_id]].lstrip("/"),
        )
        for item in sheets
    ]


def shared_strings(parts):
    fallback = f"<sst xmlns='{MAIN}'/>".encode()
    root = ET.fromstring(parts.get("xl/sharedStrings.xml", fallback))
    return ["".join(item.itertext()) for item in root.findall("m:si", NS)]


def values_by_address(parts, path):
    strings = shared_strings(parts)
    root = ET.fromstring(parts[path])
    values = {}
    for cell in root.findall(".//m:c", NS):
        reference = cell.attrib["r"]
        value = cell.find("m:v", NS)
        if value is None:
            values[reference] = "".join(cell.itertext())
        elif cell.attrib.get("t") == "s":
            values[reference] = strings[int(value.text)]
        else:
            values[reference] = value.text
    return values


class CashFlowTemplateContractTests(unittest.TestCase):
    def test_scaffold_package_contract(self):
        """The artifact-tool scaffold is the production change that makes this pass."""
        self.assertTrue(
            WORKBOOK.is_file(),
            "Production change needed: run the artifact-tool scaffold generator to create templates/13-week-cash-flow-forecast.xlsx",
        )

        parts = workbook_parts()
        sheets = sheet_paths(parts)
        self.assertEqual([name for name, _ in sheets], EXPECTED_SHEETS)
        sheet_map = dict(sheets)

        forecast_values = values_by_address(parts, sheet_map["13-Week Forecast"])
        for row, label in FORECAST_LABELS.items():
            self.assertEqual(forecast_values.get(f"A{row}"), label)
        self.assertEqual(
            [forecast_values.get(f"{column}5") for column in "BCDEFGHIJKLMN"].count(None),
            0,
        )

        workbook = ET.fromstring(parts["xl/workbook.xml"])
        defined_names = {
            item.attrib["name"] for item in workbook.findall("m:definedNames/m:definedName", NS)
        }
        self.assertTrue({"CashFlowSelectedScenario", "CashFlowScenarioList"} <= defined_names)
        assumptions_xml = parts[sheet_map["Assumptions"]]
        self.assertIn(b"dataValidation", assumptions_xml)
        self.assertIn(b"CashFlowScenarioList", assumptions_xml)

        names = set(parts)
        self.assertFalse(any("vbaProject" in name or "externalLink" in name for name in names))
        rel_parts = [payload for name, payload in parts.items() if name.endswith(".rels")]
        self.assertFalse(any(b'TargetMode="External"' in payload for payload in rel_parts))
        all_xml = b" ".join(payload.lower() for name, payload in parts.items() if name.endswith((".xml", ".rels")))
        for banned in BANNED_SOURCE_TEXT:
            self.assertNotIn(banned.encode(), all_xml)
        styles = parts["xl/styles.xml"].upper()
        for colour in PALETTE_ANCHORS:
            self.assertIn(colour.encode(), styles)


if __name__ == "__main__":
    unittest.main()
