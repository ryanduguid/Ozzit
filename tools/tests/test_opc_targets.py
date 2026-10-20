"""Absolute relationship targets resolve from the package root in every tool that follows one.

Excel writes relative targets ("worksheets/sheet1.xml") and the committed workbook holds
only those, so the tools' other tests never see the absolute form another producer can
write ("/xl/worksheets/sheet1.xml"). Each workbook here mixes the 2 forms.
"""

import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import polish_workbook  # noqa: E402
import verify_cache  # noqa: E402
from opc import resolve_target  # noqa: E402

RELS_NS = "http://schemas.openxmlformats.org/package/2006/relationships"


def book_parts(*names: str) -> dict[str, bytes]:
    """A workbook whose first sheet has an absolute target and every other a relative one."""
    sheets = "".join(
        f'<sheet name="{name}" sheetId="{n}" r:id="rId{n}"/>' for n, name in enumerate(names, start=1)
    )
    rels = "".join(
        f'<Relationship Id="rId{n}" Type="ws" Target="{"/xl/" if n == 1 else ""}worksheets/sheet{n}.xml"/>'
        for n in range(1, len(names) + 1)
    )
    return {
        "xl/workbook.xml": (
            f'<workbook xmlns="{polish_workbook.MAIN_NS}" xmlns:r="{polish_workbook.DOC_REL_NS}">'
            f"<sheets>{sheets}</sheets></workbook>"
        ).encode("utf-8"),
        "xl/_rels/workbook.xml.rels": f'<Relationships xmlns="{RELS_NS}">{rels}</Relationships>'.encode("utf-8"),
    }


class ResolveTargetTests(unittest.TestCase):
    def test_targets_resolve_per_opc(self):
        for source, target, expected in (
            ("xl/workbook.xml", "/xl/worksheets/sheet1.xml", "xl/worksheets/sheet1.xml"),
            ("xl/workbook.xml", "worksheets/sheet1.xml", "xl/worksheets/sheet1.xml"),
            ("xl/worksheets/sheet1.xml", "../tables/table1.xml", "xl/tables/table1.xml"),
            ("xl/worksheets/sheet1.xml", "/xl/tables/table1.xml", "xl/tables/table1.xml"),
            ("", "xl/workbook.xml", "xl/workbook.xml"),
        ):
            with self.subTest(source=source, target=target):
                self.assertEqual(resolve_target(source, target), expected)


class AbsoluteTargetTests(unittest.TestCase):
    expected = {"Data": "xl/worksheets/sheet1.xml", "Notes": "xl/worksheets/sheet2.xml"}

    def test_polish_workbook_sheet_paths(self):
        self.assertEqual(polish_workbook.sheet_paths(book_parts("Data", "Notes")), self.expected)


    def test_verify_cache_reads_an_absolute_target(self):
        directory = Path(tempfile.mkdtemp(prefix="ozzit-opc-"))
        self.addCleanup(shutil.rmtree, directory, ignore_errors=True)
        path = directory / "absolute.xlsx"
        parts = book_parts("Data", "Notes")
        for n in (1, 2):
            parts[f"xl/worksheets/sheet{n}.xml"] = (
                f'<worksheet><sheetData><c r="A1"><v>{n}</v></c></sheetData></worksheet>'.encode("utf-8")
            )
        with zipfile.ZipFile(path, "w") as archive:
            for name, data in parts.items():
                archive.writestr(name, data)
        self.assertEqual(
            verify_cache.cached_values(path),
            ({(1, 1, 1): verify_cache.CellValue("1"), (2, 1, 1): verify_cache.CellValue("2")},
             {1: "Data", 2: "Notes"})
        )


if __name__ == "__main__":
    unittest.main()
