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
FORECAST_ROW_MAP = {
    5: "Week commencing", 6: "Week ending", 7: "Actual / Forecast",
    9: "CASH RECEIPTS", 10: "Customer receipts", 11: "Overdue receipts",
    12: "Cash / EFTPOS sales", 13: "GST refunds / credits", 14: "Other operating",
    15: "Asset sale proceeds", 16: "Equity / owner funding", 17: "Loan proceeds",
    18: "Scenario receipt adjustment", 19: "Total cash receipts",
    21: "CASH PAYMENTS", 22: "Suppliers and inventory", 23: "Net wages",
    24: "PAYG withholding", 25: "Superannuation", 26: "Payroll tax / workers compensation",
    27: "Rent", 28: "Utilities", 29: "Insurance", 30: "Software and subscriptions",
    31: "Marketing", 32: "Professional services", 33: "Freight, vehicles and travel",
    34: "GST / BAS payments", 35: "PAYG income tax instalments", 36: "FBT / other tax",
    37: "Interest and bank fees", 38: "Loan principal", 39: "Capital expenditure",
    40: "Dividends / distributions", 41: "Other operating", 42: "Scenario payment adjustment",
    43: "Total cash payments", 45: "CASH POSITION", 46: "Opening cash balance",
    47: "Net cash movement", 48: "Closing cash balance", 49: "Minimum cash buffer",
    50: "Headroom / (gap)", 51: "Liquidity status",
}
BASE_CLOSING_CASH = [358000, 349000, 325000, 293000, 201000, 133000, 217000, 160000, 117000, 137000, 99000, 82000, 31000]
FORECAST_COLUMNS = "BCDEFGHIJKLMN"
FLOW_ROWS = (*range(10, 20), *range(22, 44))
TERMINAL_ROWS = (46, 47, 48, 49, 50, 51)
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


def cached_value(parts, path, address):
    root = ET.fromstring(parts[path])
    cell = root.find(f".//m:c[@r='{address}']", NS)
    value = cell.find("m:v", NS)
    return None if value is None else value.text or ""


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


def comments_text(parts):
    return b" ".join(payload.lower() for name, payload in parts.items() if "comments" in name and name.endswith(".xml"))


def comment_text(parts, address):
    for name, payload in parts.items():
        if not name.startswith("xl/comments") or not name.endswith(".xml"):
            continue
        root = ET.fromstring(payload)
        for comment in root.findall("m:commentList/m:comment", NS):
            if comment.attrib.get("ref", "").upper() == address.upper():
                return "".join(comment.itertext()).lower().encode()
    return b""


def conditional_formulas(parts, path):
    root = ET.fromstring(parts[path])
    return [formula.text for formula in root.findall(".//m:conditionalFormatting/m:cfRule/m:formula", NS)]


def forecast_formula_contract(column, previous_column):
    return {
        7: f'IF({column}6<=\'Assumptions\'!$B$9,"Actual","Forecast")',
        18: f'IF({column}$7="Actual",0,SUM({column}10:{column}12)*(INDEX(Assumptions!$C$15:$C$17,MATCH(Assumptions!$B$12,Assumptions!$B$15:$B$17,0))-1))',
        19: f"SUM({column}10:{column}18)",
        42: f'IF({column}$7="Actual",0,SUM({column}22,{column}31,{column}33,{column}41)*(INDEX(Assumptions!$D$15:$D$17,MATCH(Assumptions!$B$12,Assumptions!$B$15:$B$17,0))-1))',
        43: f"SUM({column}22:{column}42)",
        46: "'Assumptions'!$B$10" if previous_column is None else f"{previous_column}48",
        47: f"{column}19-{column}43",
        48: f"{column}46+{column}47",
        49: "'Assumptions'!$B$11",
        50: f"{column}48-{column}49",
        51: f'IF({column}48<{column}49,"BELOW BUFFER",IF({column}48<={column}49*1.25,"WATCH","OK"))',
    }


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

    def test_forecast_engine_and_weekly_review_contract(self):
        """The formula-driven forecast generator is the production change that makes this pass."""
        parts = workbook_parts()
        sheet_map = dict(sheet_paths(parts))
        forecast_path = sheet_map["13-Week Forecast"]
        forecast_values = values_by_address(parts, forecast_path)
        forecast_formulas = formulas_by_address(parts, forecast_path)

        for row, label in FORECAST_ROW_MAP.items():
            self.assertEqual(forecast_values.get(f"A{row}"), label)
        self.assertEqual([forecast_values.get(f"{column}7") for column in FORECAST_COLUMNS], ["Actual", *["Forecast"] * 12])
        self.assertEqual([int(float(forecast_values[f"{column}48"])) for column in FORECAST_COLUMNS], BASE_CLOSING_CASH)
        for index, column in enumerate(FORECAST_COLUMNS):
            previous_column = FORECAST_COLUMNS[index - 1] if index else None
            for row, expected_formula in forecast_formula_contract(column, previous_column).items():
                self.assertEqual(forecast_formulas.get(f"{column}{row}"), expected_formula)

        for row in FLOW_ROWS:
            self.assertEqual(forecast_formulas.get(f"O{row}"), f"SUM(B{row}:N{row})")
            self.assertEqual(int(float(forecast_values[f"O{row}"])), sum(int(float(forecast_values[f"{column}{row}"])) for column in FORECAST_COLUMNS))
        for row in TERMINAL_ROWS:
            self.assertEqual(forecast_formulas.get(f"O{row}"), f"N{row}")
            self.assertEqual(forecast_values.get(f"O{row}"), forecast_values.get(f"N{row}"))

        self.assertEqual(style_for_cell(parts, forecast_path, "B10"), ("FF0000FF", "FFF3F1F6"))
        self.assertEqual(style_for_cell(parts, forecast_path, "B18")[0], "FF000000")
        self.assertEqual(style_for_cell(parts, forecast_path, "B46")[0], "FF008000")
        self.assertTrue(any("BELOW BUFFER" in formula for formula in conditional_formulas(parts, forecast_path)))
        self.assertTrue(any("WATCH" in formula for formula in conditional_formulas(parts, forecast_path)))
        self.assertTrue(any("OK" in formula for formula in conditional_formulas(parts, forecast_path)))
        comment_payload = comments_text(parts)
        for phrase in (b"scenario receipt", b"scenario payment", b"gst / bas", b"payg income-tax"):
            self.assertIn(phrase, comment_payload)
        self.assertIn(b"current customer receipts, overdue receipts and cash / eftpos sales", comment_text(parts, "B18"))

        review_path = sheet_map["Weekly Review"]
        review_values = values_by_address(parts, review_path)
        review_formulas = formulas_by_address(parts, review_path)
        self.assertEqual(
            [review_values.get(f"{column}5") for column in "ABCDEFGHI"],
            ["Week", "Week ending", "Forecast closing cash", "Actual closing cash", "Variance", "Rolling forecast note", "Owner", "Action", "Status"],
        )
        self.assertEqual([review_values.get(f"A{row}") for row in range(6, 19)], [str(index) for index in range(1, 14)])
        for index, column in enumerate(FORECAST_COLUMNS, start=6):
            self.assertEqual(review_formulas.get(f"B{index}"), f"'13-Week Forecast'!{column}$6")
            self.assertEqual(review_formulas.get(f"C{index}"), f"'13-Week Forecast'!{column}$48")
            self.assertEqual(review_formulas.get(f"E{index}"), f'IF(D{index}="","",D{index}-C{index})')
            self.assertEqual(review_values.get(f"B{index}"), forecast_values.get(f"{column}6"))
            self.assertEqual(review_values.get(f"C{index}"), forecast_values.get(f"{column}48"))
            self.assertEqual(review_values.get(f"D{index}"), "")
            self.assertIsNone(cached_value(parts, review_path, f"E{index}"))
        self.assertEqual(style_for_cell(parts, review_path, "B6")[0], "FF008000")
        self.assertEqual(style_for_cell(parts, review_path, "D6"), ("FF0000FF", "FFF3F1F6"))
        review_root = ET.fromstring(parts[review_path])
        validations = review_root.findall("m:dataValidations/m:dataValidation", NS)
        self.assertEqual([(item.attrib.get("sqref"), item.findtext("m:formula1", namespaces=NS)) for item in validations], [("I6:I18", '"Open,In progress,Complete"')])


if __name__ == "__main__":
    unittest.main()
