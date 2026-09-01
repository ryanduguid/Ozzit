"""Contract for tools/postbuild/remove_residue.py.

The tracked workbook carries none of the residue any more, so the pass must be
a byte no-op on it (test_idempotency covers that too). These tests put each kind
of residue back into a copy and prove the pass removes it, renumbers what
referenced it, and refuses a workbook whose file-properties list disagrees.
"""

import re
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import remove_residue  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402

WORKBOOK = ROOT / "ozzit.xlsx"


def parts_of(path: Path) -> dict[str, bytes]:
    with zipfile.ZipFile(path) as archive:
        return {name: archive.read(name) for name in archive.namelist()}


class ResidueTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-residue-"))
        self.workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, self.workbook)

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def test_tracked_workbook_carries_no_residue(self):
        parts = parts_of(WORKBOOK)
        self.assertFalse([n for n in parts if n.startswith("xl/customProperty")])
        book = parts["xl/workbook.xml"].decode("utf-8")
        self.assertNotIn('name="FMTs"', book)
        self.assertNotIn("<we:extLst>", parts["xl/webextensions/webextension1.xml"].decode("utf-8"))
        styles = parts["xl/styles.xml"].decode("utf-8")
        dxf_count = int(re.search(r'<dxfs count="(\d+)"', styles).group(1))
        xf_count = int(re.search(r'<cellStyleXfs count="(\d+)"', styles).group(1))
        referenced = set()
        for name, data in parts.items():
            if name.startswith("xl/worksheets/sheet"):
                referenced |= {int(x) for x in re.findall(r'dxfId="(\d+)"', data.decode("utf-8"))}
        referenced |= {int(x) for x in re.findall(r'dxfId="(\d+)"', re.search(r"<tableStyles.*?</tableStyles>", styles, re.S).group(0))}
        self.assertEqual(referenced, set(range(dxf_count)), "every differential format is used")
        used_xf = {0} | {int(x) for x in re.findall(r'xfId="(\d+)"', re.search(r"<cellXfs.*?</cellXfs>", styles, re.S).group(0))}
        self.assertEqual(used_xf, set(range(xf_count)), "every named cell style is used")
        self.assertEqual(int(re.search(r'<cellStyles count="(\d+)"', styles).group(1)), xf_count)
        targets = remove_residue.sheet_targets(parts)
        for sheet, column in remove_residue.FREEZE.items():
            text = parts[targets[sheet]].decode("utf-8")
            self.assertIn(f'<pane xSplit="{remove_residue.column_number(column) - 1}" topLeftCell="{column}1"', text)
        self.assertEqual(remove_residue.run(self.workbook), [])
        self.assertEqual(self.workbook.read_bytes(), WORKBOOK.read_bytes())

    def test_residue_put_back_is_removed_and_references_renumbered(self):
        parts = parts_of(self.workbook)
        # a hidden sheet in the middle of the book, with a table and a print area after it
        book = parts["xl/workbook.xml"].decode("utf-8")
        rels = parts["xl/_rels/workbook.xml.rels"].decode("utf-8")
        print_areas = re.findall(r'localSheetId="(\d+)"', book)
        sheet = parts["xl/worksheets/sheet3.xml"]
        parts["xl/worksheets/sheet99.xml"] = sheet
        parts["xl/worksheets/_rels/sheet99.xml.rels"] = (
            b'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            b'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            b'<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table99.xml"/>'
            b"</Relationships>"
        )
        parts["xl/tables/table99.xml"] = parts["xl/tables/table2.xml"]
        book = book.replace(
            '<sheet name="Data Validation"',
            '<sheet name="FMTs" sheetId="999" state="hidden" r:id="rId999"/><sheet name="Data Validation"',
            1,
        )
        rels = rels.replace(
            "</Relationships>",
            '<Relationship Id="rId999" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet99.xml"/></Relationships>',
        )
        book = re.sub(r'localSheetId="(\d+)"', lambda m: 'localSheetId="%d"' % (int(m.group(1)) + 1), book)
        app = parts["docProps/app.xml"].decode("utf-8")
        app = app.replace("<vt:lpstr>Data Validation</vt:lpstr>", "<vt:lpstr>FMTs</vt:lpstr><vt:lpstr>Data Validation</vt:lpstr>", 1)
        app = re.sub(r"(<vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>)(\d+)", lambda m: f"{m.group(1)}{int(m.group(2)) + 1}", app)
        app = re.sub(r'(<TitlesOfParts><vt:vector size=")(\d+)', lambda m: f"{m.group(1)}{int(m.group(2)) + 1}", app)
        content_types = parts["[Content_Types].xml"].decode("utf-8").replace(
            "</Types>",
            '<Override PartName="/xl/worksheets/sheet99.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
            '<Override PartName="/xl/tables/table99.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>'
            '<Default Extension="bin" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.customProperty"/></Types>',
        )
        # a custom property on the cover sheet
        parts["xl/customProperty1.bin"] = "Cover".encode("utf-16-le")
        parts["xl/worksheets/_rels/sheet1.xml.rels"] = (
            b'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            b'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            b'<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customProperty" Target="../customProperty1.bin"/>'
            b"</Relationships>"
        )
        cover = parts["xl/worksheets/sheet1.xml"].decode("utf-8")
        cover = cover.replace("</worksheet>", '<customProperties><customPr name="Description" r:id="rId2"/></customProperties></worksheet>')
        parts["xl/worksheets/sheet1.xml"] = cover.encode("utf-8")
        # the custom-function declaration, an unused differential format and an unused style
        web = parts["xl/webextensions/webextension1.xml"].decode("utf-8")
        parts["xl/webextensions/webextension1.xml"] = web.replace(
            "</we:webextension>",
            '<we:extLst><a:ext xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" uri="{D87F86FE-615C-45B5-9D79-34F1136793EB}"><we:containsCustomFunctions/></a:ext></we:extLst></we:webextension>',
        ).encode("utf-8")
        styles = parts["xl/styles.xml"].decode("utf-8")
        dxf_count = int(re.search(r'<dxfs count="(\d+)"', styles).group(1))
        styles = re.sub(r'<dxfs count="(\d+)">', lambda m: f'<dxfs count="{dxf_count + 1}"><dxf><font><b/></font></dxf>', styles, count=1)
        # every existing reference now points one entry later
        for name in list(parts):
            if name.startswith("xl/worksheets/sheet"):
                text = parts[name].decode("utf-8")
                parts[name] = re.sub(r'dxfId="(\d+)"', lambda m: 'dxfId="%d"' % (int(m.group(1)) + 1), text).encode("utf-8")
        table_styles = re.search(r"<tableStyles.*?</tableStyles>", styles, re.S).group(0)
        styles = styles.replace(table_styles, re.sub(r'dxfId="(\d+)"', lambda m: 'dxfId="%d"' % (int(m.group(1)) + 1), table_styles))
        xf_count = int(re.search(r'<cellStyleXfs count="(\d+)"', styles).group(1))
        styles = styles.replace("</cellStyleXfs>", '<xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>', 1)
        styles = styles.replace(f'<cellStyleXfs count="{xf_count}"', f'<cellStyleXfs count="{xf_count + 1}"', 1)
        names_count = int(re.search(r'<cellStyles count="(\d+)"', styles).group(1))
        styles = styles.replace("</cellStyles>", f'<cellStyle name="Unused" xfId="{xf_count}"/></cellStyles>', 1)
        styles = styles.replace(f'<cellStyles count="{names_count}"', f'<cellStyles count="{names_count + 1}"', 1)
        parts["xl/styles.xml"] = styles.encode("utf-8")
        parts["xl/workbook.xml"] = book.encode("utf-8")
        parts["xl/_rels/workbook.xml.rels"] = rels.encode("utf-8")
        parts["docProps/app.xml"] = app.encode("utf-8")
        parts["[Content_Types].xml"] = content_types.encode("utf-8")
        # the freeze panes, unfrozen again
        targets = remove_residue.sheet_targets(parts)
        for sheet in remove_residue.FREEZE:
            text = parts[targets[sheet]].decode("utf-8")
            text = re.sub(r"<pane [^>]*/>", "", text)
            text = re.sub(r'<selection pane="topRight"[^>]*/>', "", text)
            parts[targets[sheet]] = text.encode("utf-8")
        write_deterministic(self.workbook, parts)

        log = remove_residue.run(self.workbook)
        self.assertEqual(len(log), 6, log)
        after = parts_of(self.workbook)
        self.assertNotIn("xl/worksheets/sheet99.xml", after)
        self.assertNotIn("xl/tables/table99.xml", after)
        self.assertNotIn("xl/customProperty1.bin", after)
        self.assertNotIn("xl/worksheets/_rels/sheet1.xml.rels", after)
        self.assertNotIn("<customProperties>", after["xl/worksheets/sheet1.xml"].decode("utf-8"))
        self.assertNotIn("<we:extLst>", after["xl/webextensions/webextension1.xml"].decode("utf-8"))
        self.assertNotIn("FMTs", after["docProps/app.xml"].decode("utf-8"))
        self.assertNotIn('Extension="bin"', after["[Content_Types].xml"].decode("utf-8"))
        book = after["xl/workbook.xml"].decode("utf-8")
        self.assertNotIn('name="FMTs"', book)
        self.assertEqual(re.findall(r'localSheetId="(\d+)"', book), print_areas)
        styles = after["xl/styles.xml"].decode("utf-8")
        self.assertEqual(int(re.search(r'<dxfs count="(\d+)"', styles).group(1)), dxf_count)
        self.assertNotIn('name="Unused"', styles)
        # the renumbered references are the original ones again
        self.assertEqual(after["xl/worksheets/sheet2.xml"], parts_of(WORKBOOK)["xl/worksheets/sheet2.xml"])
        self.assertEqual(self.workbook.read_bytes(), WORKBOOK.read_bytes(), "the pass restores the tracked bytes exactly")

    def test_pass_fails_when_the_properties_list_disagrees(self):
        parts = parts_of(self.workbook)
        book = parts["xl/workbook.xml"].decode("utf-8").replace(
            "<sheets>", '<sheets><sheet name="FMTs" sheetId="998" state="hidden" r:id="rId1"/>', 1
        )
        parts["xl/workbook.xml"] = book.encode("utf-8")
        write_deterministic(self.workbook, parts)
        with self.assertRaisesRegex(ValueError, "docProps/app.xml lists FMTs 0 times"):
            remove_residue.run(self.workbook)


if __name__ == "__main__":
    unittest.main()
