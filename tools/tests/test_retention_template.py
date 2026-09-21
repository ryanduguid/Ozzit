import unittest
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKBOOK = ROOT / "templates" / "retention-reconciliation.xlsx"
MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PKG_REL = "http://schemas.openxmlformats.org/package/2006/relationships"
NS = {"m": MAIN, "r": REL}


class RetentionTemplateStructureTests(unittest.TestCase):
    def setUp(self):
        with zipfile.ZipFile(WORKBOOK) as archive:
            self.parts = {name: archive.read(name) for name in archive.namelist()}

    def sheet(self):
        workbook = ET.fromstring(self.parts["xl/workbook.xml"])
        relationships = ET.fromstring(self.parts["xl/_rels/workbook.xml.rels"])
        targets = {item.attrib["Id"]: item.attrib["Target"] for item in relationships}
        sheet = workbook.find("m:sheets/m:sheet", NS)
        self.assertIsNotNone(sheet)
        self.assertEqual(sheet.attrib["name"], "Retention reconciliation")
        target = targets[sheet.attrib[f"{{{REL}}}id"]].lstrip("/")
        return target if target.startswith("xl/") else "xl/" + target

    def test_package_is_safe_and_has_expected_sheet(self):
        self.assertTrue(WORKBOOK.is_file())
        self.assertIn("[Content_Types].xml", self.parts)
        self.assertIn("xl/workbook.xml", self.parts)
        self.assertIn("xl/_rels/workbook.xml.rels", self.parts)
        forbidden = ("vbaProject", "externalLinks", "oleObjects", "embeddings", "activeX")
        self.assertFalse(any(any(marker.lower() in name.lower() for marker in forbidden)
                             for name in self.parts))
        relationships = ET.fromstring(self.parts["xl/_rels/workbook.xml.rels"])
        self.assertFalse(any("externalLink" in item.attrib.get("Type", "")
                             for item in relationships))
        self.sheet()

    def test_formulas_fixed_ranges_and_cached_examples(self):
        path = self.sheet()
        root = ET.fromstring(self.parts[path])
        cells = {cell.attrib["r"]: cell for cell in root.findall(".//m:c", NS)}

        # These are the calculation and status outputs exercised by the native test.
        for address in ("E10", "E14", "G10", "G14", "J4", "J5", "J10", "J11", "J12"):
            self.assertIn(address, cells)
            self.assertIsNotNone(cells[address].find("m:f", NS), address)
        self.assertEqual(cells["E14"].findtext("m:v", namespaces=NS), "58000")
        self.assertEqual(cells["G10"].findtext("m:v", namespaces=NS), "250")
        self.assertEqual(cells["J4"].findtext("m:v", namespaces=NS), "1")

        formulas = " ".join(cell.findtext("m:f", default="", namespaces=NS) or "" for cell in cells.values())
        self.assertIn("10:12", formulas)
        self.assertNotIn("13", formulas)

    def test_inputs_and_data_validation_are_present(self):
        path = self.sheet()
        root = ET.fromstring(self.parts[path])
        validations = root.find("m:dataValidations", NS)
        self.assertIsNotNone(validations)
        self.assertGreaterEqual(int(validations.attrib.get("count", "0")), 1)
        sqrefs = " ".join(item.attrib.get("sqref", "") for item in validations)
        self.assertTrue(sqrefs)
        self.assertIn("B4", sqrefs)
        self.assertIn("B6", sqrefs)
        cells = {cell.attrib["r"] for cell in root.findall(".//m:c", NS)}
        for row in (10, 11, 12):
            for column in ("A", "B", "C", "D", "F", "H"):
                self.assertIn(f"{column}{row}", cells)
        self.assertNotIn("13", sqrefs)


if __name__ == "__main__":
    unittest.main()
