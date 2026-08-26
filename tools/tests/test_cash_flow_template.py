import unittest
import xml.etree.ElementTree as ET
import zipfile
import re
from datetime import date, timedelta
from pathlib import Path
from posixpath import dirname, join, normpath


ROOT = Path(__file__).resolve().parents[2]
WORKBOOK = ROOT / "templates" / "13-week-cash-flow-forecast.xlsx"
TEMPLATE_README = ROOT / "templates" / "README.md"
MAIN_README = ROOT / "README.md"
MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PACKAGE_REL = "http://schemas.openxmlformats.org/package/2006/relationships"
CONTENT_TYPES = "http://schemas.openxmlformats.org/package/2006/content-types"
NS = {"m": MAIN, "r": REL}
DRAWING = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
CHART = "http://schemas.openxmlformats.org/drawingml/2006/chart"
DRAWING_NS = {"xdr": DRAWING, "c": CHART, "r": REL, "a": "http://schemas.openxmlformats.org/drawingml/2006/main"}
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
SCENARIO_CLOSING_CASH = {
    "Base": BASE_CLOSING_CASH,
    "Upside": [358000, 364380, 354920, 336680, 257940, 203320, 300000, 256060, 226480, 260600, 237020, 234980, 199320],
    "Downside": [358000, 318800, 266150, 206900, 88550, -6050, 52600, -30500, -100300, -108350, -174900, -221400, -302550],
}
FORECAST_COLUMNS = "BCDEFGHIJKLMN"
FLOW_ROWS = (*range(10, 20), *range(22, 44), 47)
TERMINAL_ROWS = (46, 48, 49, 50, 51)
PALETTE_ANCHORS = ("5C2D91", "04001F", "2B2733", "B1AFAD", "DED9E8", "F3F1F6", "7A5AB5", "C00000")
BANNED_SOURCE_TEXT = ("tradepoint", "1099", "federal tax", "state tax", "tradepointcfo")
WORKING_SHEETS = EXPECTED_SHEETS[1:]
EXCEL_EPOCH = date(1899, 12, 30)
START_DATE = date(2026, 8, 31)
AUD_FORMAT = '"$"#,##0;[Red]("$"#,##0);-'
DATE_FORMAT = "dd mmm yyyy"
PERCENT_FORMAT = "0.0%"
ERROR_TOKENS = ("#REF!", "#DIV/0!", "#VALUE!", "#NAME?", "#N/A", "#NUM!", "#NULL!")
VOLATILE_PATTERN = re.compile(
    r"(?<![A-Z0-9_.])(?:_xlfn\.)?(?:TODAY|NOW|RAND|RANDARRAY|RANDBETWEEN|OFFSET|INDIRECT|CELL|INFO|RTD)\s*\(",
    re.IGNORECASE,
)
PROHIBITED_PART_MARKERS = (
    "vbaproject", "/activex/", "/oleobjects/", "/ctrlprops/", "/embeddings/",
    "customui/", "attachedtemplate", "/externallinks/",
)
PROHIBITED_CONTENT_TYPE_MARKERS = (
    "vba", "activex", "oleobject", "macroenabled", "attachedtemplate",
    "control", "customui", "embedded", "externallink",
)
PROHIBITED_RELATIONSHIP_MARKERS = (
    "/vbaproject", "/activex", "/oleobject", "/control", "/customui",
    "/attachedtemplate", "/externallink",
)
FORMULA_ERROR_COUNT_FORMULA = (
    "SUMPRODUCT(--ISERROR('Assumptions'!$B$5:$E$19))"
    "+SUMPRODUCT(--ISERROR('13-Week Forecast'!$B$5:$O$51))"
    "+SUMPRODUCT(--ISERROR('Weekly Review'!$A$4:$N$18))"
    "+SUMPRODUCT(--ISERROR('Dashboard'!$A$6:$J$30))"
    "+SUMPRODUCT(--ISERROR('Dashboard'!$P$4:$S$49))"
)


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


def markdown_section(markdown, heading, next_heading=None):
    start_marker = f"## {heading}\n"
    start = markdown.index(start_marker) + len(start_marker)
    end = len(markdown) if next_heading is None else markdown.index(f"## {next_heading}\n", start)
    return markdown[start:end]


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


def font_and_number_format(parts, path, address):
    sheet = ET.fromstring(parts[path])
    cell = sheet.find(f".//m:c[@r='{address}']", NS)
    styles = ET.fromstring(parts["xl/styles.xml"])
    style = styles.find("m:cellXfs", NS)[int(cell.attrib.get("s", "0"))]
    font = styles.find("m:fonts", NS)[int(style.attrib.get("fontId", "0"))]
    name = font.find("m:name", NS)
    number_format_id = int(style.attrib.get("numFmtId", "0"))
    custom_formats = {
        int(item.attrib["numFmtId"]): item.attrib["formatCode"]
        for item in styles.findall("m:numFmts/m:numFmt", NS)
    }
    return name.attrib.get("val") if name is not None else None, custom_formats.get(number_format_id, "General")


def relationship_records(parts):
    for relationships_path, payload in parts.items():
        if not relationships_path.endswith(".rels"):
            continue
        root = ET.fromstring(payload)
        if relationships_path == "_rels/.rels":
            source_part = ""
        else:
            prefix, relationship_file = relationships_path.rsplit("/_rels/", 1)
            source_part = join(prefix, relationship_file[:-5])
        for relationship in root.findall(f"{{{PACKAGE_REL}}}Relationship"):
            target = relationship.attrib.get("Target", "")
            if target.startswith("/"):
                resolved_target = normpath(target.lstrip("/"))
            else:
                resolved_target = normpath(join(dirname(source_part), target)).lstrip("/")
            yield {
                "source": source_part,
                "type": relationship.attrib.get("Type", ""),
                "target": target,
                "target_mode": relationship.attrib.get("TargetMode", "Internal"),
                "resolved_target": resolved_target,
            }


def formulas_and_cached_values(parts):
    for sheet_name, path in sheet_paths(parts):
        root = ET.fromstring(parts[path])
        for cell in root.findall(".//m:c", NS):
            formula = cell.find("m:f", NS)
            cached = cell.find("m:v", NS)
            yield {
                "sheet": sheet_name,
                "address": cell.attrib["r"],
                "formula": "" if formula is None else formula.text or "",
                "cached": "" if cached is None else cached.text or "",
                "type": cell.attrib.get("t", ""),
            }


def worksheet_references(formula):
    unquoted_formula = re.sub(r'"(?:[^"]|"")*"', '""', formula)
    return {
        (quoted.replace("''", "'") if quoted else bare).strip()
        for quoted, bare in re.findall(r"(?:'((?:[^']|'')+)'|([A-Za-z0-9 _&-]+))!", unquoted_formula)
    }


def assert_package_and_formula_safe(test_case, parts):
    lower_names = [name.lower() for name in parts]
    test_case.assertFalse([
        name for name in lower_names
        if any(marker in name for marker in PROHIBITED_PART_MARKERS)
    ])

    content_types = ET.fromstring(parts["[Content_Types].xml"])
    declared_types = [item.attrib.get("ContentType", "").lower() for item in content_types]
    test_case.assertFalse([
        content_type for content_type in declared_types
        if any(marker in content_type for marker in PROHIBITED_CONTENT_TYPE_MARKERS)
    ])

    relationships = list(relationship_records(parts))
    test_case.assertFalse([item for item in relationships if item["target_mode"].lower() == "external"])
    test_case.assertFalse([
        item for item in relationships
        if any(marker in item["type"].lower() for marker in PROHIBITED_RELATIONSHIP_MARKERS)
        or item["type"].lower().endswith("/package")
    ])
    unresolved = [
        item for item in relationships
        if item["target_mode"].lower() != "external" and item["resolved_target"] not in parts
    ]
    test_case.assertEqual(unresolved, [])

    sheets = {name for name, _ in sheet_paths(parts)}
    for cell in formulas_and_cached_values(parts):
        location = f"{cell['sheet']}!{cell['address']}"
        test_case.assertNotEqual(cell["type"], "e", location)
        test_case.assertFalse([token for token in ERROR_TOKENS if token in cell["formula"] or token in cell["cached"]], location)
        test_case.assertEqual(worksheet_references(cell["formula"]) - sheets, set(), location)
        formula_without_strings = re.sub(r'"(?:[^"]|"")*"', '""', cell["formula"])
        test_case.assertIsNone(VOLATILE_PATTERN.search(formula_without_strings), location)


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


def column_width(parts, path, reference):
    target = column_number(reference)
    root = ET.fromstring(parts[path])
    for column in root.findall("m:cols/m:col", NS):
        if int(column.attrib["min"]) <= target <= int(column.attrib["max"]):
            return float(column.attrib["width"])
    return None


def relationship_target(parts, source_path, relationship_id):
    rels_path = join(dirname(source_path), "_rels", Path(source_path).name + ".rels")
    relationships = ET.fromstring(parts[rels_path])
    target = next(item.attrib["Target"] for item in relationships if item.attrib["Id"] == relationship_id)
    return normpath(join(dirname(source_path), target)).lstrip("/")


def dashboard_charts(parts, dashboard_path):
    dashboard = ET.fromstring(parts[dashboard_path])
    drawing = dashboard.find("m:drawing", NS)
    drawing_path = relationship_target(parts, dashboard_path, drawing.attrib[f"{{{REL}}}id"])
    drawing_root = ET.fromstring(parts[drawing_path])
    charts = []
    for anchor in drawing_root.findall("xdr:twoCellAnchor", DRAWING_NS):
        chart_ref = anchor.find(".//c:chart", DRAWING_NS)
        if chart_ref is None:
            continue
        chart_path = relationship_target(parts, drawing_path, chart_ref.attrib[f"{{{REL}}}id"])
        chart_root = ET.fromstring(parts[chart_path])
        chart_element = next(child for child in chart_root.find("c:chart/c:plotArea", DRAWING_NS))
        bar_direction = chart_element.find("c:barDir", DRAWING_NS)
        grouping = chart_element.find("c:grouping", DRAWING_NS)
        charts.append({
            "type": chart_element.tag.rsplit("}", 1)[-1],
            "barDir": bar_direction.attrib.get("val") if bar_direction is not None else None,
            "grouping": grouping.attrib.get("val") if grouping is not None else None,
            "title": "".join(node.text or "" for node in chart_root.findall(".//a:t", DRAWING_NS)),
            "categories": [node.text for node in chart_root.findall(".//c:cat//c:f", DRAWING_NS)],
            "series": [node.text for node in chart_root.findall(".//c:val//c:f", DRAWING_NS)],
            "anchor": tuple(
                int(anchor.findtext(f"xdr:{edge}/xdr:{coordinate}", namespaces=DRAWING_NS))
                for edge, coordinate in (("from", "col"), ("from", "row"), ("to", "col"), ("to", "row"))
            ),
        })
    return charts


def sum_abs_differences(left_sheet, left_cells, right_sheet, right_cells):
    return "+".join(
        f"ABS('{left_sheet}'!${left_column}${left_row}-'{right_sheet}'!${right_column}${right_row})"
        for (left_column, left_row), (right_column, right_row) in zip(left_cells, right_cells)
    )


def max_abs_formula(expressions):
    return "MAX(" + ",".join(f"ABS({expression})" for expression in expressions) + ")"


def cash_roll_forward_check_formula():
    equations = [
        f"'13-Week Forecast'!${column}$48-'13-Week Forecast'!${column}$46-'13-Week Forecast'!${column}$47"
        for column in FORECAST_COLUMNS
    ]
    continuity = [
        f"'13-Week Forecast'!${column}$46-'13-Week Forecast'!${previous}$48"
        for previous, column in zip(FORECAST_COLUMNS, FORECAST_COLUMNS[1:])
    ]
    return max_abs_formula([*equations, *continuity])


def weekly_total_check_formula(total_row, component_start, component_end):
    return max_abs_formula([
        f"'13-Week Forecast'!${column}${total_row}-SUM('13-Week Forecast'!${column}${component_start}:${column}${component_end})"
        for column in FORECAST_COLUMNS
    ])


def conditional_rule_styles(parts, path):
    sheet = ET.fromstring(parts[path])
    styles = ET.fromstring(parts["xl/styles.xml"])
    dxfs = styles.find("m:dxfs", NS)
    rules = {}
    for rule in sheet.findall(".//m:conditionalFormatting/m:cfRule", NS):
        dxf = dxfs[int(rule.attrib["dxfId"])]
        font_colour = next((node.attrib.get("rgb") for node in dxf.findall("m:font/m:color", NS)), None)
        fill_colour = next((node.attrib.get("rgb") for node in dxf.findall("m:fill/m:patternFill/m:bgColor", NS)), None)
        rules[rule.findtext("m:formula", namespaces=NS)] = (font_colour, fill_colour)
    return rules


def formulas_without_string_literals(parts):
    for path in dict(sheet_paths(parts)).values():
        yield from formulas_by_address(parts, path).values()


class CashFlowTemplateContractTests(unittest.TestCase):
    def test_package_contract_rejects_internal_external_link_parts(self):
        """An internal externalLink package must fail even without TargetMode=External."""
        parts = workbook_parts()
        external_link_path = "xl/externalLinks/externalLink1.xml"
        parts[external_link_path] = b"<externalLink/>"

        content_types = ET.fromstring(parts["[Content_Types].xml"])
        ET.SubElement(content_types, f"{{{CONTENT_TYPES}}}Override", {
            "PartName": f"/{external_link_path}",
            "ContentType": "application/vnd.openxmlformats-officedocument.spreadsheetml.externalLink+xml",
        })
        parts["[Content_Types].xml"] = ET.tostring(content_types)

        workbook_relationships = ET.fromstring(parts["xl/_rels/workbook.xml.rels"])
        ET.SubElement(workbook_relationships, f"{{{PACKAGE_REL}}}Relationship", {
            "Id": "rIdSyntheticExternalLink",
            "Type": "http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLink",
            "Target": "/xl/externalLinks/externalLink1.xml",
        })
        parts["xl/_rels/workbook.xml.rels"] = ET.tostring(workbook_relationships)

        with self.assertRaises(AssertionError):
            assert_package_and_formula_safe(self, parts)

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
        self.assertEqual(start_here_values.get("D6"), "Workbook sheets")
        self.assertEqual(start_here_values.get("A6"), "Version")
        self.assertEqual(start_here_values.get("B6"), "Release 1.1")
        self.assertEqual(start_here_values.get("A7"), "Prepared date")
        self.assertEqual(start_here_values.get("B7"), excel_serial(date(2026, 8, 26)))
        self.assertEqual(start_here_values.get("A12"), "Five-step weekly update process")
        self.assertEqual([start_here_values.get(f"A{row}") for row in range(13, 18)], ["1", "2", "3", "4", "5"])
        self.assertEqual([start_here_values.get(f"B{row}") for row in range(13, 18)], [
            "Update the Assumptions control panel and selected scenario.",
            "Replace blue cash-receipt and cash-payment inputs with the latest weekly view.",
            "Review closing cash, headroom and liquidity status in the 13-Week Forecast.",
            "Snapshot the original forecast as values, then record actuals and owner commentary in Weekly Review.",
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
        for row, sheet_name in enumerate(WORKING_SHEETS, start=7):
            self.assertEqual(start_here_values.get(f"D{row}"), sheet_name)

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
        self.assertEqual(assumptions_values.get("A19"), "Liquidity action lead time (weeks)")
        self.assertEqual(assumptions_values.get("B19"), "2")
        self.assertEqual(assumptions_values.get("A20"), "Data-quality notes")
        self.assertIn("Monday forecast start", assumptions_values.get("A21", ""))
        input_style = style_for_cell(parts, sheet_map["Assumptions"], "B10")
        link_style = style_for_cell(parts, sheet_map["Start Here"], "D7")
        self.assertEqual(input_style, ("FF0000FF", "FFB1AFAD"))
        self.assertEqual(link_style, ("FF2B2733", None))
        self.assertNotEqual(link_style, input_style)

        assumptions_root = ET.fromstring(parts[sheet_map["Assumptions"]])
        self.assertGreaterEqual(
            column_width(parts, sheet_map["Assumptions"], "D"),
            22,
            "Assumptions!D14 must show 'Variable-payment factor' without clipping into Description.",
        )
        validations = assumptions_root.findall("m:dataValidations/m:dataValidation", NS)
        self.assertEqual({validation.attrib.get("sqref") for validation in validations}, {"B12", "B19"})
        scenario_validation = next(validation for validation in validations if validation.attrib.get("sqref") == "B12")
        lead_time_validation = next(validation for validation in validations if validation.attrib.get("sqref") == "B19")
        self.assertEqual(scenario_validation.findtext("m:formula1", namespaces=NS), "CashFlowScenarioList")
        self.assertEqual(lead_time_validation.attrib.get("type"), "whole")
        self.assertEqual(lead_time_validation.attrib.get("operator"), "between")
        self.assertEqual(lead_time_validation.findtext("m:formula1", namespaces=NS), "0")
        self.assertEqual(lead_time_validation.findtext("m:formula2", namespaces=NS), "13")

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

        assert_package_and_formula_safe(self, parts)
        self.assertIsNotNone(VOLATILE_PATTERN.search("=_xlfn.RANDARRAY(2,2)"))
        all_xml = b" ".join(payload.lower() for name, payload in parts.items() if name.endswith((".xml", ".rels")))
        for banned in BANNED_SOURCE_TEXT:
            self.assertNotIn(banned.encode(), all_xml)
        styles = parts["xl/styles.xml"].upper()
        for colour in PALETTE_ANCHORS:
            self.assertIn(colour.encode(), styles)

        for sheet_name, path in sheets:
            self.assertEqual(font_and_number_format(parts, path, "A1")[0], "Aptos Display", sheet_name)
            self.assertEqual(font_and_number_format(parts, path, "A2")[0], "Aptos", sheet_name)
        self.assertEqual(font_and_number_format(parts, sheet_map["Start Here"], "B7")[1], DATE_FORMAT)
        self.assertEqual(font_and_number_format(parts, sheet_map["Assumptions"], "B8")[1], DATE_FORMAT)
        self.assertEqual(font_and_number_format(parts, sheet_map["Assumptions"], "B10")[1], AUD_FORMAT)
        self.assertEqual(font_and_number_format(parts, sheet_map["Assumptions"], "C15")[1], PERCENT_FORMAT)

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
        self.assertEqual(forecast_formulas.get("O47"), "SUM(B47:N47)")
        self.assertEqual(forecast_values.get("O47"), "-369000")

        self.assertEqual(style_for_cell(parts, forecast_path, "B10"), ("FF0000FF", "FFF3F1F6"))
        self.assertEqual(style_for_cell(parts, forecast_path, "B18")[0], "FF000000")
        self.assertEqual(style_for_cell(parts, forecast_path, "B46")[0], "FF008000")
        self.assertEqual(font_and_number_format(parts, forecast_path, "B5")[1], DATE_FORMAT)
        self.assertEqual(font_and_number_format(parts, forecast_path, "B47")[1], AUD_FORMAT)
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
            [review_values.get(f"{column}5") for column in "ABCDEFGHIJKLMN"],
            [
                "Snapshot week start", "Snapshot week end", "Forecast snapshot receipts", "Actual receipts",
                "Receipt variance $", "Receipt variance %", "Forecast snapshot payments",
                "Actual payments", "Payment variance $", "Payment variance %",
                "Forecast snapshot closing cash", "Actual closing cash", "Closing-cash variance",
                "Owner commentary",
            ],
        )
        self.assertIn("as values", review_values.get("A4", "").lower())
        self.assertIn("must not link", review_values.get("A4", "").lower())
        for row, column in enumerate(FORECAST_COLUMNS, start=6):
            expected_formulas = {
                "E": f'IF(D{row}="","",D{row}-C{row})',
                "F": f'IF(OR(D{row}="",C{row}=0),"",(D{row}-C{row})/C{row})',
                "I": f'IF(H{row}="","",G{row}-H{row})',
                "J": f'IF(OR(H{row}="",G{row}=0),"",(G{row}-H{row})/G{row})',
                "M": f'IF(L{row}="","",L{row}-K{row})',
            }
            for review_column, expected_formula in expected_formulas.items():
                self.assertEqual(review_formulas.get(f"{review_column}{row}"), expected_formula)
            self.assertEqual(review_values.get(f"A{row}"), forecast_values.get(f"{column}5"))
            self.assertEqual(review_values.get(f"B{row}"), forecast_values.get(f"{column}6"))
            self.assertEqual(review_values.get(f"C{row}"), forecast_values.get(f"{column}19"))
            self.assertEqual(review_values.get(f"G{row}"), forecast_values.get(f"{column}43"))
            self.assertEqual(review_values.get(f"K{row}"), forecast_values.get(f"{column}48"))
            for snapshot_column in "ABCGK":
                self.assertNotIn(f"{snapshot_column}{row}", review_formulas)
            for input_column in "DHLN":
                self.assertEqual(review_values.get(f"{input_column}{row}"), "")
            for variance_column in "EFIJM":
                self.assertIsNone(cached_value(parts, review_path, f"{variance_column}{row}"))
        for address in ("A6", "B6", "C6", "G6", "K6"):
            self.assertEqual(style_for_cell(parts, review_path, address), ("FF0000FF", "FFF3F1F6"))
        self.assertEqual(style_for_cell(parts, review_path, "D6"), ("FF0000FF", "FFF3F1F6"))
        self.assertEqual(style_for_cell(parts, review_path, "H6"), ("FF0000FF", "FFF3F1F6"))
        self.assertEqual(style_for_cell(parts, review_path, "L6"), ("FF0000FF", "FFF3F1F6"))
        self.assertEqual(style_for_cell(parts, review_path, "N6"), ("FF0000FF", "FFF3F1F6"))
        self.assertEqual(font_and_number_format(parts, review_path, "A6")[1], DATE_FORMAT)
        self.assertEqual(font_and_number_format(parts, review_path, "C6")[1], AUD_FORMAT)
        self.assertEqual(font_and_number_format(parts, review_path, "F6")[1], PERCENT_FORMAT)
        review_status_styles = conditional_rule_styles(parts, review_path)
        for formula in ("E6>0", "I6>0", "M6>0"):
            self.assertEqual(review_status_styles[formula], ("FF008000", "FFE2F0D9"))
        for formula in ("E6<0", "I6<0", "M6<0"):
            self.assertEqual(review_status_styles[formula], ("FFC00000", "FFFCE4D6"))
        review_root = ET.fromstring(parts[review_path])
        self.assertEqual(review_root.findall("m:dataValidations/m:dataValidation", NS), [])

    def test_dashboard_controls_charts_and_sources_contract(self):
        """The dashboard-and-controls generator change is required for this contract."""
        parts = workbook_parts()
        sheet_map = dict(sheet_paths(parts))
        dashboard_path = sheet_map["Dashboard"]
        checks_path = sheet_map["Checks & Sources"]
        dashboard_values = values_by_address(parts, dashboard_path)
        dashboard_formulas = formulas_by_address(parts, dashboard_path)
        self.assertEqual(
            [dashboard_values.get(f"{column}6") for column in "ABCDEFGHIJ"],
            [
                "Selected scenario", "Week 13 closing cash", "Lowest closing cash",
                "Minimum cash buffer", "Minimum headroom", "Weeks below buffer",
                "13-week net cash change", "Week of lowest cash", "MODEL STATUS",
                "LIQUIDITY STATUS",
            ],
        )
        expected_kpis = {
            "A7": "'Assumptions'!$B$12",
            "B7": "'13-Week Forecast'!$N$48",
            "C7": "MIN('13-Week Forecast'!$B$48:$N$48)",
            "D7": "'Assumptions'!$B$11",
            "E7": "MIN('13-Week Forecast'!$B$50:$N$50)",
            "F7": "COUNTIF('13-Week Forecast'!$B$50:$N$50,\"<0\")",
            "G7": "SUM('13-Week Forecast'!$B$47:$N$47)",
            "H7": "INDEX('13-Week Forecast'!$B$6:$N$6,1,MATCH(C7,'13-Week Forecast'!$B$48:$N$48,0))",
            "I7": "'Checks & Sources'!$B$4",
            "J7": "'Checks & Sources'!$E$4",
        }
        self.assertEqual({address: dashboard_formulas.get(address) for address in expected_kpis}, expected_kpis)
        self.assertEqual(
            [dashboard_values.get(f"{column}7") for column in "ABCDEFGHIJ"],
            ["Base", "31000", "31000", "100000", "-69000", "3", "-369000", excel_serial(date(2026, 11, 29)), "PASS", "ACTION REQUIRED"],
        )
        for address in ("B7", "C7", "D7", "E7", "G7"):
            self.assertEqual(font_and_number_format(parts, dashboard_path, address)[1], AUD_FORMAT)
        self.assertEqual(font_and_number_format(parts, dashboard_path, "H7")[1], DATE_FORMAT)
        self.assertEqual(
            [dashboard_values.get(f"{column}9") for column in "ABCD"],
            ["First buffer breach", "Weeks until breach", "Funding required", "Action deadline"],
        )
        expected_action_formulas = {
            "A10": 'IF(COUNTIF(\'13-Week Forecast\'!$B$51:$N$51,"BELOW BUFFER")=0,"None",INDEX(\'13-Week Forecast\'!$B$6:$N$6,1,MATCH("BELOW BUFFER",\'13-Week Forecast\'!$B$51:$N$51,0)))',
            "B10": 'IF(A10="None","13+",MAX(0,ROUNDUP((A10-\'Assumptions\'!$B$9)/7,0)))',
            "C10": "MAX(0,-MIN('13-Week Forecast'!$B$50:$N$50))",
            "D10": 'IF(A10="None","None",A10-7*\'Assumptions\'!$B$19)',
        }
        self.assertEqual({address: dashboard_formulas.get(address) for address in expected_action_formulas}, expected_action_formulas)
        self.assertEqual(
            [dashboard_values.get(f"{column}10") for column in "ABCD"],
            [excel_serial(date(2026, 11, 15)), "10", "69000", excel_serial(date(2026, 11, 1))],
        )
        self.assertEqual(font_and_number_format(parts, dashboard_path, "A10")[1], DATE_FORMAT)
        self.assertEqual(font_and_number_format(parts, dashboard_path, "C10")[1], AUD_FORMAT)
        self.assertEqual(font_and_number_format(parts, dashboard_path, "D10")[1], DATE_FORMAT)
        self.assertEqual(
            [dashboard_values.get(f"{column}11") for column in "ABCDE"],
            ["Week", "Week ending", "Closing cash", "Headroom", "Liquidity status"],
        )
        for index, column in enumerate(FORECAST_COLUMNS, start=12):
            self.assertEqual(dashboard_formulas.get(f"A{index}"), f"ROW()-11")
            self.assertEqual(dashboard_formulas.get(f"B{index}"), f"'13-Week Forecast'!{column}$6")
            self.assertEqual(dashboard_formulas.get(f"C{index}"), f"'13-Week Forecast'!{column}$48")
            self.assertEqual(dashboard_formulas.get(f"D{index}"), f"'13-Week Forecast'!{column}$50")
            self.assertEqual(dashboard_formulas.get(f"E{index}"), f"'13-Week Forecast'!{column}$51")

        self.assertEqual([dashboard_values.get(f"{column}4") for column in "PQR"], ["Week ending", "Closing cash", "Minimum buffer"])
        self.assertEqual([dashboard_values.get(f"{column}21") for column in "PQR"], ["Week ending", "Total cash receipts", "Total cash payments"])
        for index, column in enumerate(FORECAST_COLUMNS, start=5):
            self.assertEqual(dashboard_formulas.get(f"P{index}"), f"TEXT('13-Week Forecast'!{column}$6,\"dd mmm\")")
            self.assertEqual(dashboard_formulas.get(f"Q{index}"), f"'13-Week Forecast'!{column}$48")
            self.assertEqual(dashboard_formulas.get(f"R{index}"), f"'13-Week Forecast'!{column}$49")
        for index, column in enumerate(FORECAST_COLUMNS, start=22):
            self.assertEqual(dashboard_formulas.get(f"P{index}"), f"TEXT('13-Week Forecast'!{column}$6,\"dd mmm\")")
            self.assertEqual(dashboard_formulas.get(f"Q{index}"), f"'13-Week Forecast'!{column}$19")
            self.assertEqual(dashboard_formulas.get(f"R{index}"), f"'13-Week Forecast'!{column}$43")

        self.assertEqual(
            [dashboard_values.get(f"{column}27") for column in "ABCDE"],
            ["Scenario", "Week 13 cash", "Minimum cash", "First buffer breach", "Funding required"],
        )
        scenario_expected = {
            28: ("Base", 31000, 31000, date(2026, 11, 15), 69000),
            29: ("Upside", 199320, 199320, "None", 0),
            30: ("Downside", -302550, -302550, date(2026, 10, 4), 402550),
        }
        for row, (name, week_13, minimum, breach, funding) in scenario_expected.items():
            self.assertEqual(dashboard_values.get(f"A{row}"), name)
            self.assertIn(f"A{row}", dashboard_formulas)
            self.assertEqual(int(float(dashboard_values[f"B{row}"])), week_13)
            self.assertEqual(int(float(dashboard_values[f"C{row}"])), minimum)
            expected_breach = excel_serial(breach) if isinstance(breach, date) else breach
            self.assertEqual(dashboard_values.get(f"D{row}"), expected_breach)
            self.assertEqual(int(float(dashboard_values[f"E{row}"])), funding)
            for column in "BCDE":
                self.assertIn(f"{column}{row}", dashboard_formulas)

        self.assertEqual([dashboard_values.get(f"{column}36") for column in "PQRS"], ["Week ending", "Base closing cash", "Upside closing cash", "Downside closing cash"])
        for scenario_column, scenario_name in zip("QRS", ("Base", "Upside", "Downside")):
            actual_values = [int(float(dashboard_values[f"{scenario_column}{row}"])) for row in range(37, 50)]
            self.assertEqual(actual_values, SCENARIO_CLOSING_CASH[scenario_name])
            self.assertTrue(all(f"{scenario_column}{row}" in dashboard_formulas for row in range(37, 50)))
        self.assertEqual([dashboard_values.get(f"{column}37") for column in "QRS"], ["358000", "358000", "358000"])

        charts = dashboard_charts(parts, dashboard_path)
        self.assertEqual(len(charts), 2)
        line_chart = next(chart for chart in charts if chart["title"] == "Closing cash vs minimum buffer (AUD)")
        receipts_chart = next(chart for chart in charts if chart["title"] == "Weekly receipts vs payments (AUD)")
        self.assertEqual(line_chart["type"], "lineChart")
        self.assertEqual(line_chart["categories"], ["'Dashboard'!$P$5:$P$17", "'Dashboard'!$P$5:$P$17"])
        self.assertEqual(line_chart["series"], ["'Dashboard'!$Q$5:$Q$17", "'Dashboard'!$R$5:$R$17"])
        self.assertEqual(line_chart["anchor"], (5, 9, 15, 25))
        self.assertEqual(receipts_chart["type"], "barChart")
        self.assertEqual(receipts_chart["barDir"], "col")
        self.assertEqual(receipts_chart["grouping"], "clustered")
        self.assertEqual(receipts_chart["categories"], ["'Dashboard'!$P$22:$P$34", "'Dashboard'!$P$22:$P$34"])
        self.assertEqual(receipts_chart["series"], ["'Dashboard'!$Q$22:$Q$34", "'Dashboard'!$R$22:$R$34"])
        self.assertEqual(receipts_chart["anchor"], (5, 26, 15, 42))

        checks_values = values_by_address(parts, checks_path)
        checks_formulas = formulas_by_address(parts, checks_path)
        self.assertEqual(checks_values.get("A4"), "MODEL STATUS")
        self.assertEqual(checks_formulas.get("B4"), 'IF(COUNTIF(F8:F16,"PASS")=9,"PASS","FAIL")')
        self.assertEqual(checks_values.get("B4"), "PASS")
        self.assertEqual(checks_values.get("D4"), "LIQUIDITY STATUS")
        self.assertEqual(checks_formulas.get("E4"), 'IF(COUNTIF(\'13-Week Forecast\'!$B$51:$N$51,"BELOW BUFFER")>0,"ACTION REQUIRED",IF(COUNTIF(\'13-Week Forecast\'!$B$51:$N$51,"WATCH")>0,"WATCH","OK"))')
        self.assertEqual(checks_values.get("E4"), "ACTION REQUIRED")
        self.assertEqual([checks_values.get(f"{column}7") for column in "ABCDEFGH"], ["Check", "Actual", "Expected", "Difference", "Tolerance", "Status", "Where to fix", "Notes"])
        expected_checks = [
            ("Exactly 13 forecast weeks", "COLUMNS('13-Week Forecast'!$B$5:$N$5)", "13", "B8-C8", "13-Week Forecast!B5:N5"),
            ("Forecast starts Monday", "WEEKDAY('Assumptions'!$B$8,2)", "1", "B9-C9", "Assumptions!B8"),
            ("Selected scenario recognised", "COUNTIF('Assumptions'!$B$15:$B$17,'Assumptions'!$B$12)", "1", "B10-C10", "Assumptions!B12"),
            ("Week 1 opening cash ties", "'13-Week Forecast'!$B$46", "'Assumptions'!$B$10", "B11-C11", "13-Week Forecast!B46"),
            ("Weekly cash roll-forward equations", cash_roll_forward_check_formula(), "0", "B12-C12", "13-Week Forecast!B46:N48"),
            ("Cash receipt totals reconcile", weekly_total_check_formula(19, 10, 18), "0", "B13-C13", "13-Week Forecast!B19:N19"),
            ("Cash payment totals reconcile", weekly_total_check_formula(43, 22, 42), "0", "B14-C14", "13-Week Forecast!B43:N43"),
            ("Week 13 closing cash equation", "'13-Week Forecast'!$N$48", "'Assumptions'!$B$10+SUM('13-Week Forecast'!$B$47:$N$47)", "B15-C15", "13-Week Forecast!N48"),
            ("Formula-error count", FORMULA_ERROR_COUNT_FORMULA, "0", "B16-C16", "Assumptions, Forecast, Weekly Review and Dashboard"),
        ]
        for row, (label, actual, expected, difference, where_to_fix) in enumerate(expected_checks, start=8):
            self.assertEqual(checks_values.get(f"A{row}"), label)
            self.assertEqual(checks_formulas.get(f"B{row}"), actual)
            self.assertEqual(checks_formulas.get(f"C{row}"), expected)
            self.assertEqual(checks_formulas.get(f"D{row}"), difference)
            self.assertEqual(checks_values.get(f"E{row}"), "0")
            self.assertEqual(checks_formulas.get(f"F{row}"), f'IF(ABS(D{row})<=E{row},"PASS","FAIL")')
            self.assertEqual(checks_values.get(f"F{row}"), "PASS")
            self.assertEqual(checks_values.get(f"G{row}"), where_to_fix)

        source_rows = {
            checks_values[f"A{row}"]: {column: checks_values.get(f"{column}{row}", "") for column in "BCDEF"}
            for row in range(22, 27)
        }
        self.assertEqual(source_rows["GST standard rate"]["B"], "https://www.legislation.gov.au/C2004A00446/latest/text")
        self.assertIn("GST default: 10%", source_rows["GST standard rate"]["C"])
        self.assertIn("taxable supply", source_rows["GST standard rate"]["C"])
        self.assertIn("does not decide", source_rows["GST standard rate"]["E"])
        self.assertEqual(source_rows["BAS due dates"]["B"], "https://www.ato.gov.au/businesses-and-organisations/preparing-lodging-and-paying/business-activity-statements-bas/due-dates-for-lodging-and-paying-your-bas")
        self.assertIn("planning pattern", source_rows["BAS due dates"]["C"].lower())
        self.assertIn("issued or entity-specific BAS due date controls", source_rows["BAS due dates"]["E"])
        self.assertNotIn("all entities", " ".join(source_rows["BAS due dates"].values()).lower())
        self.assertEqual(source_rows["PAYG withholding"]["B"], "https://www.ato.gov.au/businesses-and-organisations/hiring-and-paying-your-workers/payg-withholding")
        payg_text = " ".join(source_rows["PAYG withholding"].values()).lower()
        self.assertIn("business size", payg_text)
        self.assertIn("circumstances", payg_text)
        self.assertIn("user-entered", payg_text)
        self.assertIn("confirmed", payg_text)
        self.assertEqual(source_rows["Super guarantee"]["B"], "https://www.ato.gov.au/tax-rates-and-codes/key-superannuation-rates-and-thresholds/super-guarantee")
        self.assertIn("12.0%", source_rows["Super guarantee"]["C"])
        self.assertIn("worker eligibility", source_rows["Super guarantee"]["E"].lower())
        self.assertEqual(source_rows["Payday Super"]["B"], "https://softwaredevelopers.ato.gov.au/PaydaySuper")
        payday_text = " ".join(source_rows["Payday Super"].values()).lower()
        self.assertIn("from 1 july 2026", payday_text)
        self.assertIn("generally receive it by the seventh business day after payday", payday_text)
        self.assertIn("eligible longer period", payday_text)
        self.assertNotIn("must be received by the seventh business day after payday", payday_text)
        self.assertTrue(all(row["D"] == "26 August 2026" for row in source_rows.values()))
        disclaimer = checks_values.get("A28", "").lower()
        for phrase in ("illustrative fp&a cash-planning model only", "not tax, bas, payroll, superannuation or legal advice", "entity-specific lodgement dates and tax classifications must be confirmed by the user or adviser"):
            self.assertIn(phrase, disclaimer)

        self.assertEqual(style_for_cell(parts, dashboard_path, "A1"), ("FFFFFFFF", "FF5C2D91"))
        self.assertEqual(style_for_cell(parts, dashboard_path, "A6"), ("FFFFFFFF", "FF04001F"))
        self.assertEqual(style_for_cell(parts, dashboard_path, "B7"), ("FF04001F", "FFFFFFFF"))
        self.assertEqual(style_for_cell(parts, checks_path, "A21"), ("FFFFFFFF", "FF04001F"))
        dashboard_status_styles = conditional_rule_styles(parts, dashboard_path)
        checks_status_styles = conditional_rule_styles(parts, checks_path)
        self.assertEqual(dashboard_status_styles['I7="PASS"'], ("FF008000", "FFE2F0D9"))
        self.assertEqual(dashboard_status_styles['J7="ACTION REQUIRED"'], ("FFC00000", "FFFCE4D6"))
        self.assertEqual(checks_status_styles['B4="PASS"'], ("FF008000", "FFE2F0D9"))
        self.assertEqual(checks_status_styles['E4="ACTION REQUIRED"'], ("FFC00000", "FFFCE4D6"))
        self.assertEqual(checks_status_styles['E4="OK"'], ("FF008000", "FFE2F0D9"))
        self.assertEqual(font_and_number_format(parts, checks_path, "B11")[1], AUD_FORMAT)

    def test_template_documentation_contract(self):
        """The repository documentation change is the production change that makes this pass."""
        template_readme = TEMPLATE_README.read_text(encoding="utf-8")
        main_readme = MAIN_README.read_text(encoding="utf-8")
        workflow = markdown_section(template_readme, "Weekly workflow", "Scenario behaviour")
        scenario = markdown_section(template_readme, "Scenario behaviour", "Checks and limitations")
        limitations = markdown_section(template_readme, "Checks and limitations")
        getting_started = markdown_section(main_readme, "Getting started", "Australian conventions")
        repository_layout = markdown_section(main_readme, "Repository layout", "Checks")

        self.assertIn("13-week-cash-flow-forecast.xlsx", template_readme)
        self.assertIn("Excel 2024 and later", template_readme)
        self.assertIn("illustrative data", template_readme.lower())
        self.assertEqual(len(re.findall(r"(?m)^\d+\. \*\*[^*]+\*\*", workflow)), 5)
        for action in (
            "replace the blue input cells",
            "enter the business name, forecast start date, as-at date, opening cash and minimum cash buffer",
            "select `Base`, `Upside` or `Downside`",
            "refresh the forecast inputs and enter available actual receipts, actual payments and actual closing cash",
            "before refreshing the live forecast",
            "paste the original period dates, forecast receipts, forecast payments and forecast closing cash into the blue snapshot cells",
            "as values",
            "must not link to the live forecast",
            "use `Dashboard`",
            "action deadline",
            "three-scenario comparison",
            "use `Weekly Review`",
            "compare receipt, payment and closing-cash variances",
            "record owner commentary",
            "review `Checks & Sources`",
            "save the reviewed file as the period's archive",
            "copy it for the next cycle",
        ):
            self.assertIn(action.lower(), workflow.lower())

        self.assertIn("Scenario factors apply only to weeks labelled `Forecast`.", scenario)
        self.assertIn("Weeks labelled `Actual` remain unchanged.", scenario)
        for scenario_name, receipt_factor, payment_factor in (
            ("Base", "100%", "100%"),
            ("Upside", "108%", "98%"),
            ("Downside", "85%", "105%"),
        ):
            self.assertIn(
                f"`{scenario_name}` uses {receipt_factor} receipts and {payment_factor} selected variable payments.",
                scenario,
            )
        self.assertIn(
            "The scenario receipt adjustment is calculated from customer receipts, overdue receipts and cash / EFTPOS sales.",
            scenario,
        )
        self.assertIn(
            "The scenario variable-payment adjustment is calculated from suppliers and inventory, marketing, freight / vehicles / travel and other operating payments.",
            scenario,
        )
        for excluded in (
            "GST refunds or credits, other operating receipts, asset sale proceeds, equity or owner funding, or loan proceeds",
            "wages, PAYG withholding, superannuation, payroll tax",
            "GST / BAS payments, income-tax instalments",
            "FBT, interest, loan principal, capital expenditure or dividends",
        ):
            self.assertIn(excluded, scenario)

        self.assertIn("not tax advice", limitations.lower())
        self.assertIn("statutory", limitations.lower())
        self.assertIn("exactly 13 weeks", limitations.lower())
        self.assertIn("formula errors", limitations.lower())
        self.assertIn("[Ozzit 13-week cash-flow forecast template](templates/README.md)", getting_started)
        self.assertIn("| `templates/` | `13-week-cash-flow-forecast.xlsx` and its user guide |", repository_layout)


if __name__ == "__main__":
    unittest.main()
