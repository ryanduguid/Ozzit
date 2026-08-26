import unittest
import xml.etree.ElementTree as ET
import zipfile
from datetime import date, timedelta
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
WORKING_SHEETS = EXPECTED_SHEETS[1:]
EXCEL_EPOCH = date(1899, 12, 30)
START_DATE = date(2026, 8, 31)


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


def formulas_by_address(parts, path):
    root = ET.fromstring(parts[path])
    return {
        cell.attrib["r"]: formula.text
        for cell in root.findall(".//m:c", NS)
        if (formula := cell.find("m:f", NS)) is not None
    }


def excel_serial(value):
    return str((value - EXCEL_EPOCH).days)


def style_for_cell(parts, path, address):
    sheet = ET.fromstring(parts[path])
    cell = sheet.find(f".//m:c[@r='{address}']", NS)
    styles = ET.fromstring(parts["xl/styles.xml"])
    cell_xfs = styles.find("m:cellXfs", NS)
    fonts = styles.find("m:fonts", NS)
    fills = styles.find("m:fills", NS)
    style = cell_xfs[int(cell.attrib.get("s", "0"))]
    font = fonts[int(style.attrib.get("fontId", "0"))]
    fill = fills[int(style.attrib.get("fillId", "0"))]
    return (
        next((node.attrib.get("rgb") for node in font.findall("m:color", NS)), None),
        next((node.attrib.get("rgb") for node in fill.findall(".//m:fgColor", NS)), None),
    )


def row_bounds(reference):
    endpoints = reference.split(":")
    rows = [cell_row(endpoint) for endpoint in endpoints]
    return min(rows), max(rows)


def cell_row(reference):
    return int("".join(character for character in reference if character.isdigit()))


def column_number(reference):
    value = 0
    for character in reference:
        if character.isalpha():
            value = value * 26 + ord(character.upper()) - ord("A") + 1
    return value


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
        expected_week_starts = [excel_serial(START_DATE + timedelta(days=7 * index)) for index in range(13)]
        self.assertEqual([forecast_values.get(f"{column}5") for column in "BCDEFGHIJKLMN"], expected_week_starts)
        self.assertEqual(forecast_values.get("O5"), "13-week total / terminal")
        self.assertEqual(forecast_values.get("O6"), "Week 13 value")
        self.assertEqual(forecast_values.get("O7"), "")
        forecast_root = ET.fromstring(parts[sheet_map["13-Week Forecast"]])
        header_cells_after_terminal = [
            cell.attrib["r"]
            for cell in forecast_root.findall(".//m:c", NS)
            if cell_row(cell.attrib["r"]) in {5, 6, 7}
            and column_number(cell.attrib["r"]) > 15
            and any(cell.find(path, NS) is not None for path in ("m:v", "m:f", "m:is"))
        ]
        self.assertEqual(header_cells_after_terminal, [])

        start_here_values = values_by_address(parts, sheet_map["Start Here"])
        self.assertEqual(start_here_values.get("A1"), "Ozzit | 13-Week Cash-Flow Forecast")
        self.assertEqual(
            start_here_values.get("A2"),
            "A practical weekly liquidity model for Australian finance teams and small-business operators.",
        )
        self.assertIn("ILLUSTRATIVE DATA", start_here_values.get("A4", ""))
        self.assertEqual(start_here_values.get("A6"), "Version")
        self.assertEqual(start_here_values.get("B6"), "1.0 scaffold")
        self.assertEqual(start_here_values.get("A7"), "Prepared date")
        self.assertEqual(start_here_values.get("B7"), excel_serial(date(2026, 8, 26)))
        self.assertEqual(start_here_values.get("A12"), "Five-step weekly update process")
        self.assertEqual([start_here_values.get(f"A{row}") for row in range(13, 18)], ["1", "2", "3", "4", "5"])
        self.assertEqual([start_here_values.get(f"B{row}") for row in range(13, 18)], [
            "Update the Assumptions control panel and selected scenario.",
            "Replace blue cash-receipt and cash-payment inputs with the latest weekly view.",
            "Review closing cash, headroom and liquidity status in the 13-Week Forecast.",
            "Record actual results, actions and owners in Weekly Review.",
            "Resolve any control failure in Checks & Sources before management review.",
        ])
        self.assertEqual(start_here_values.get("A19"), "Colour legend")
        self.assertEqual([start_here_values.get(f"A{row}") for row in range(20, 25)], [
            "Blue text / lavender fill", "Black text", "Green text", "Red text / pale-red fill", "Dark green / pale-green fill",
        ])
        self.assertEqual([start_here_values.get(f"B{row}") for row in range(20, 25)], [
            "Editable user input", "Formula or calculation", "Cross-sheet link", "Warning or exception", "Passing check",
        ])
        self.assertIn("not tax, payroll, legal or financial advice", start_here_values.get("A27", ""))
        start_here_formulas = formulas_by_address(parts, sheet_map["Start Here"])
        for row, sheet_name in enumerate(WORKING_SHEETS, start=7):
            self.assertEqual(
                start_here_formulas.get(f"D{row}"),
                f'HYPERLINK("#\'{sheet_name}\'!A1","{sheet_name}")',
            )

        workbook = ET.fromstring(parts["xl/workbook.xml"])
        defined_names = {
            item.attrib["name"]: item.text
            for item in workbook.findall("m:definedNames/m:definedName", NS)
        }
        self.assertEqual(defined_names, {
            "CashFlowScenario": "'Assumptions'!$B$12",
            "CashFlowSelectedScenario": "'Assumptions'!$B$12",
            "CashFlowScenarioList": "'Assumptions'!$B$15:$B$17",
        })
        assumptions_values = values_by_address(parts, sheet_map["Assumptions"])
        self.assertEqual(
            {address: assumptions_values.get(address) for address in ("B5", "B6", "B7", "B8", "B9", "B10", "B11", "B12")},
            {
                "B5": "Illustrative Australian business",
                "B6": "AUD",
                "B7": "Whole dollars",
                "B8": excel_serial(START_DATE),
                "B9": excel_serial(date(2026, 9, 6)),
                "B10": "400000",
                "B11": "100000",
                "B12": "Base",
            },
        )
        self.assertEqual([assumptions_values.get(f"B{row}") for row in range(15, 18)], ["Base", "Upside", "Downside"])
        self.assertEqual([assumptions_values.get(f"C{row}") for row in range(15, 18)], ["1", "1.08", "0.85"])
        self.assertEqual([assumptions_values.get(f"D{row}") for row in range(15, 18)], ["1", "0.98", "1.05"])
        self.assertEqual(assumptions_values.get("A20"), "Data-quality notes")
        self.assertIn("Monday forecast start", assumptions_values.get("A21", ""))
        input_style = style_for_cell(parts, sheet_map["Assumptions"], "B10")
        link_style = style_for_cell(parts, sheet_map["Start Here"], "D7")
        self.assertEqual(input_style, ("FF0000FF", "FFB1AFAD"))
        self.assertEqual(link_style, ("FF008000", None))
        self.assertNotEqual(link_style, input_style)

        assumptions_root = ET.fromstring(parts[sheet_map["Assumptions"]])
        validations = assumptions_root.findall("m:dataValidations/m:dataValidation", NS)
        self.assertEqual(len(validations), 1)
        self.assertEqual(validations[0].attrib.get("sqref"), "B12")
        self.assertEqual(validations[0].findtext("m:formula1", namespaces=NS), "CashFlowScenarioList")

        for sheet_name, path in sheets:
            root = ET.fromstring(parts[path])
            sheet_view = root.find("m:sheetViews/m:sheetView", NS)
            self.assertEqual(sheet_view.attrib.get("showGridLines"), "0", sheet_name)
            margins = root.find("m:pageMargins", NS)
            self.assertIsNotNone(margins, sheet_name)
            self.assertTrue(all(float(margins.attrib[key]) > 0 for key in ("left", "right", "top", "bottom")), sheet_name)

        calculation_merges = [
            merge.attrib["ref"]
            for merge in forecast_root.findall("m:mergeCells/m:mergeCell", NS)
            if row_bounds(merge.attrib["ref"])[0] <= 51 and row_bounds(merge.attrib["ref"])[1] >= 5
        ]
        self.assertEqual(calculation_merges, [])

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
