"""Refusal and edge paths of the residue, rate and date helper, and revision-strip passes."""

import ast
import contextlib
import csv
import io
import re
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import rate_date_helpers as helpers  # noqa: E402
import remove_residue  # noqa: E402
import strip_revision_history as strip  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402
from workbook import read_book, read_parts  # noqa: E402

WORKBOOK = ROOT / "ozzit.xlsx"
BLOCK = "/*\r\nREVISIONS: synthetic test block\r\n*/\r\n"


def run_main(module, *argv):
    """main() under the given arguments: (exit code or None, stdout)."""
    output = io.StringIO()
    code = None
    with mock.patch.object(sys, "argv", [module.__name__, *map(str, argv)]), \
            contextlib.redirect_stdout(output):
        try:
            module.main()
        except SystemExit as exc:
            code = exc.code
    return code, output.getvalue()


class ScratchDirectory(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-pass-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)


STYLES = (
    '<styleSheet><cellStyleXfs count="1"><xf numFmtId="0"/></cellStyleXfs>'
    '<cellXfs count="1"><xf xfId="0"/></cellXfs>'
    '<cellStyles count="1"><cellStyle name="Normal" xfId="0"/></cellStyles>'
    "%s</styleSheet>"
)


def style_parts(dxfs, sheet="<worksheet/>", styles=STYLES):
    return {"xl/styles.xml": (styles % dxfs).encode("utf-8"),
            "xl/worksheets/sheet1.xml": sheet.encode("utf-8")}


class ResiduePathTests(ScratchDirectory):
    def test_a_residue_sheet_without_its_relationship_is_refused(self):
        parts = {
            "xl/workbook.xml": b'<workbook><sheets><sheet name="FMTs" sheetId="9" r:id="rId9"/></sheets></workbook>',
            "xl/_rels/workbook.xml.rels": b"<Relationships></Relationships>",
        }
        with self.assertRaisesRegex(ValueError, "^FMTs: relationship rId9 missing$"):
            remove_residue.drop_sheet(parts, "FMTs", [])

    def test_a_workbook_without_the_web_extension_is_left_alone(self):
        parts = {"xl/workbook.xml": b"<workbook/>"}
        log = []
        remove_residue.drop_custom_function_declaration(parts, log)
        self.assertEqual((parts, log), ({"xl/workbook.xml": b"<workbook/>"}, []))

    def test_malformed_style_collections_are_refused(self):
        for label, parts, message in (
            ("no dxfs", style_parts(""), "styles.xml has no dxfs collection"),
            ("count disagrees", style_parts('<dxfs count="2"><dxf/></dxfs>'),
             "styles.xml dxfs count disagrees with its entries"),
            ("dxfId out of range", style_parts('<dxfs count="1"><dxf/></dxfs>', '<cfRule dxfId="1"/>'),
             "a dxfId points past the dxfs collection"),
            ("no cell styles", style_parts('<dxfs count="0"></dxfs>', styles="<styleSheet>%s</styleSheet>"),
             "styles.xml lacks a cell style collection"),
            ("xfId out of range",
             style_parts('<dxfs count="0"></dxfs>', styles=STYLES.replace('<xf xfId="0"/>', '<xf xfId="1"/>')),
             "an xfId points past the cellStyleXfs collection"),
        ):
            with self.subTest(label=label), self.assertRaisesRegex(ValueError, f"^{re.escape(message)}$"):
                remove_residue.prune_styles(parts, [])

    def test_unused_formats_are_pruned_without_a_table_style_collection(self):
        parts = style_parts('<dxfs count="2"><dxf><b/></dxf><dxf><i/></dxf></dxfs>', '<cfRule dxfId="1"/>')
        log = []
        remove_residue.prune_styles(parts, log)
        self.assertEqual(log, ["dropped 1 unused differential formats"])
        self.assertEqual(parts["xl/worksheets/sheet1.xml"], b'<cfRule dxfId="0"/>')
        self.assertIn('<dxfs count="1"><dxf><i/></dxf></dxfs>', parts["xl/styles.xml"].decode("utf-8"))

    def freeze_parts(self, sheet_xml, *, drop_first=False):
        sheets = list(remove_residue.FREEZE)[1:] if drop_first else list(remove_residue.FREEZE)
        book = "".join(f'<sheet name="{name}" r:id="rId{i}"/>' for i, name in enumerate(sheets))
        rels = "".join(
            f'<Relationship Id="rId{i}" Target="worksheets/sheet{i}.xml"/>' for i in range(len(sheets))
        )
        parts = {"xl/workbook.xml": book.encode("utf-8"), "xl/_rels/workbook.xml.rels": rels.encode("utf-8")}
        for i in range(len(sheets)):
            parts[f"xl/worksheets/sheet{i}.xml"] = sheet_xml.encode("utf-8")
        return parts

    def test_freeze_panes_refuse_a_missing_sheet_or_sheet_view(self):
        first = next(iter(remove_residue.FREEZE))
        for label, parts, message in (
            ("missing sheet", self.freeze_parts("<worksheet/>", drop_first=True), f"{first} is not in the workbook"),
            ("no sheet view", self.freeze_parts("<worksheet/>"), f"{first} has no sheetView"),
        ):
            with self.subTest(label=label), self.assertRaisesRegex(ValueError, f"^{re.escape(message)}$"):
                remove_residue.freeze_panes(parts, [])

    def test_main_refuses_extra_arguments_a_missing_file_and_a_non_archive(self):
        missing = self.directory / "missing.xlsx"
        not_zip = self.directory / "not-zip.xlsx"
        not_zip.write_bytes(b"not a zip archive")
        self.assertEqual(run_main(remove_residue, "a.xlsx", "b.xlsx"),
                         ("FAIL: usage: python tools/postbuild/remove_residue.py [workbook]", ""))
        self.assertEqual(run_main(remove_residue, missing), (f"FAIL: no such workbook: {missing}", ""))
        self.assertEqual(run_main(remove_residue, not_zip), ("FAIL: File is not a zip file", ""))

    def test_main_prints_each_change_it_applies(self):
        workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, workbook)
        parts = read_parts(workbook)
        target = remove_residue.sheet_targets(parts)[next(iter(remove_residue.FREEZE))]
        text = parts[target].decode("utf-8")
        text = re.sub(r'<selection pane="topRight"[^>]*/>', "", re.sub(r"<pane [^>]*/>", "", text))
        parts[target] = text.encode("utf-8")
        write_deterministic(workbook, parts)

        self.assertEqual(run_main(remove_residue, workbook), (None, (
            "froze the label columns on 1 wide demonstration sheets\n"
            f"OK: {workbook}\n"
        )))
        # Compare the parts: deflate output differs between zlib builds, so the
        # rewritten archive's bytes match the committed one only on some platforms.
        self.assertEqual(read_parts(workbook), read_parts(WORKBOOK))


def guard_loop():
    """The import-time loop that vets FUNCTIONS, compiled from the module's own lines."""
    path = Path(helpers.__file__)
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    loop = next(node for node in tree.body
                if isinstance(node, ast.For) and getattr(node.iter, "id", None) == "FUNCTIONS")
    return compile(ast.Module(body=[loop], type_ignores=[]), str(path), "exec")


class RateDateHelperPathTests(ScratchDirectory):
    def test_the_import_guard_refuses_malformed_source_blocks(self):
        code = guard_loop()
        good = "/* header */\nFλ = LAMBDA(x, x)"
        exec(code, {"FUNCTIONS": [("Financial", "Fλ", "about", good)]})
        for block, message in (
            (good + " &amp; more", "Fλ: &amp; is already escaped in the source"),
            (good + "\t", "Fλ: tabs do not belong in the source"),
            ("/* header */\nGλ = LAMBDA(x, x)", "Fλ: the source block does not declare it"),
        ):
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, f"^{re.escape(message)}$"):
                exec(code, {"FUNCTIONS": [("Financial", "Fλ", "about", block)]})

    def test_a_module_holding_part_of_the_change_is_refused(self):
        functions = helpers.module_functions("Financial")
        text = f"\n{functions[0][0]} = LAMBDA("
        with self.assertRaisesRegex(
            ValueError, f"src/Financial.txt holds 1 of {len(functions)} functions and 0 About rows"
        ):
            helpers.src_state("Financial", text)

    def test_add_to_src_needs_exactly_one_anchor_and_ends_the_module_with_a_newline(self):
        anchor = helpers.ABOUT["Dates"][0]
        with self.assertRaisesRegex(ValueError, "^src/Dates.txt: About anchor found 0 times, expected 1$"):
            helpers.add_to_src("Dates", "no anchor here\n")
        text = f'Help, TEXTSPLIT(\n    {anchor} &\n    "→"'
        self.assertEqual(helpers.add_to_src("Dates", text), helpers.add_to_src("Dates", text + "\n"))

    def test_run_refuses_a_missing_source_module(self):
        src = self.directory / "src"
        src.mkdir()
        with self.assertRaisesRegex(ValueError, "^src: missing .*Financial.txt$"):
            helpers.run(self.directory / "ozzit.xlsx", src, None)

    def copy_stores(self):
        workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, workbook)
        src = self.directory / "src"
        shutil.copytree(ROOT / "src", src)
        index = self.directory / "functions.csv"
        shutil.copy2(ROOT / "functions.csv", index)
        return workbook, src, index

    def unapply(self, workbook, src):
        """Take the 4 helpers back out of the workbook and src/."""
        for module in helpers.MODULES:
            path = src / f"{module}.txt"
            text = path.read_text(encoding="utf-8")
            functions = helpers.module_functions(module)
            blocks = "\n\n\n" + "\n\n\n".join(source for _n, _a, source in functions) + "\n"
            self.assertTrue(text.endswith(blocks))
            text = text[: -len(blocks)]
            anchor, heading, _width = helpers.ABOUT[module]
            indent = text[text.rfind("\n", 0, text.index(anchor)) + 1 : text.index(anchor)]
            rows = [f'{indent}"→¶" &', f'{indent}"{heading}¶" &'] if heading else []
            rows.extend(f"{indent}{helpers.about_row(module, n, a)} &" for n, a, _s in functions)
            text = text.replace(anchor + " &\n" + "\n".join(rows), anchor + " &", 1)
            path.write_text(text, encoding="utf-8", newline="\n")
        parts = read_parts(workbook)
        book = read_book(parts)
        for name in helpers.QUALIFIED:
            book = re.sub(r'<definedName name="%s"[^>]*>.*?</definedName>' % re.escape(name), "", book,
                          count=1, flags=re.DOTALL)
        parts["xl/workbook.xml"] = book.encode("utf-8")
        write_deterministic(workbook, parts)
        self.assertEqual(helpers.workbook_state(book), "absent")

    def test_an_index_without_any_helper_row_disagrees_with_the_applied_stores(self):
        workbook, src, index = self.copy_stores()
        with index.open(encoding="utf-8-sig", newline="") as handle:
            rows = [row for row in csv.DictReader(handle) if row["function"] not in helpers.QUALIFIED]
        with index.open("w", encoding="utf-8", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=list(rows[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)
        with self.assertRaisesRegex(ValueError, (
            r"^the stores disagree \(Financial applied, Dates applied, workbook applied, "
            r"functions.csv absent\); the views must not be applied separately$"
        )):
            helpers.run(workbook, src, index)

    def test_a_missing_or_omitted_index_is_noted_and_left_unwritten(self):
        for label, index_name in (("missing", "absent.csv"), ("omitted", None)):
            with self.subTest(label=label):
                workbook, src, _index = self.copy_stores()
                self.unapply(workbook, src)
                index = None if index_name is None else self.directory / index_name
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    changed = helpers.run(workbook, src, index)
                self.assertEqual(changed, ["src/Financial.txt", "src/Dates.txt", "workbook"])
                expected = "" if index is None else f"note: {index} not found, index not updated\n"
                self.assertEqual(output.getvalue(), expected)
                for module in helpers.MODULES:
                    self.assertEqual((src / f"{module}.txt").read_bytes(),
                                     (ROOT / "src" / f"{module}.txt").read_bytes())
                self.assertEqual(helpers.workbook_state(read_book(read_parts(workbook))), "applied")
                if index is not None:
                    self.assertFalse(index.exists())
                shutil.rmtree(self.directory)
                self.directory.mkdir()

    def test_a_failure_without_an_index_restores_the_sources(self):
        workbook, src, _index = self.copy_stores()
        self.unapply(workbook, src)
        before = {path.name: path.read_bytes() for path in src.glob("*.txt")}
        index = self.directory / "absent.csv"
        with mock.patch.object(helpers, "compile_sources", side_effect=ValueError("synthetic failure")), \
                self.assertRaisesRegex(ValueError, "^synthetic failure$"):
            helpers.run(workbook, src, index)
        self.assertEqual({path.name: path.read_bytes() for path in src.glob("*.txt")}, before)
        self.assertFalse(index.exists())

    def test_main_refuses_extra_arguments_and_a_missing_workbook(self):
        missing = self.directory / "missing.xlsx"
        self.assertEqual(run_main(helpers, "a", "b", "c", "d"), (
            "FAIL: usage: python tools/postbuild/rate_date_helpers.py [workbook] [src dir] [functions.csv]", ""
        ))
        self.assertEqual(run_main(helpers, missing), (f"FAIL: no such workbook: {missing}", ""))


class RevisionStripPathTests(ScratchDirectory):
    def test_strip_module_handles_unterminated_comments_and_line_endings(self):
        self.assertEqual(strip.strip_module("a /* REVISIONS: open"), ("a /* REVISIONS: open", 0))
        self.assertEqual(strip.strip_module("x\n/*\nREVISIONS: y\n*/\nz"), ("x\nz", 1))
        self.assertEqual(strip.strip_module("x\n/*\nREVISIONS: y\n*/"), ("x\n", 1))

    def copy_stores(self, creator):
        workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, workbook)
        parts = read_parts(workbook)
        core = parts["docProps/core.xml"]
        self.assertEqual(core.count(strip.NEW_CREATOR), 1)
        parts["docProps/core.xml"] = core.replace(strip.NEW_CREATOR, creator)
        write_deterministic(workbook, parts)
        src = self.directory / "src"
        shutil.copytree(ROOT / "src", src)
        return workbook, src

    def test_main_rewrites_the_old_creator_credit(self):
        workbook, src = self.copy_stores(strip.OLD_CREATOR)
        code, output = run_main(strip, workbook, src)
        self.assertIsNone(code)
        lines = output.splitlines()
        self.assertEqual(lines[0], "rewrote docProps/core.xml creator")
        self.assertEqual(len(lines), 2)
        self.assertTrue(lines[1].startswith(f"OK: {workbook} stripped, "), lines[1])
        self.assertEqual(read_parts(workbook)["docProps/core.xml"], read_parts(WORKBOOK)["docProps/core.xml"])

    def test_an_unknown_creator_fails_and_restores_the_sources(self):
        workbook, src = self.copy_stores(b"<dc:creator>Someone else</dc:creator>")
        dates = src / "Dates.txt"
        dates.write_bytes(dates.read_bytes() + BLOCK.encode("utf-8"))
        before_dates, before_workbook = dates.read_bytes(), workbook.read_bytes()
        with mock.patch.dict(strip.EXPECTED_BLOCKS, {"Dates": 1}):
            code, output = run_main(strip, workbook, src)
        self.assertEqual(code, "FAIL: cannot strip revision history: "
                               "docProps/core.xml carries neither the old nor the new creator")
        self.assertEqual(output, "")
        self.assertEqual(dates.read_bytes(), before_dates)
        self.assertEqual(workbook.read_bytes(), before_workbook)

    def test_main_refuses_extra_arguments_and_a_missing_workbook(self):
        missing = self.directory / "missing.xlsx"
        self.assertEqual(run_main(strip, "a", "b", "c"), (
            "FAIL: usage: python tools/postbuild/strip_revision_history.py [workbook] [src dir]", ""
        ))
        self.assertEqual(run_main(strip, missing), (f"FAIL: no such workbook: {missing}", ""))


if __name__ == "__main__":
    unittest.main()
