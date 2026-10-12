"""Failure and edge paths through the release gates, run in process on small fixtures."""

import base64
import contextlib
import csv
import importlib.util
import io
import json
import re
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
ROOT = TOOLS.parent
sys.path.insert(0, str(TOOLS))

import verify_cover  # noqa: E402
import verify_previous_names  # noqa: E402
import verify_signatures  # noqa: E402
import verify_sources  # noqa: E402
import verify_workbook  # noqa: E402


def zip_bytes(parts: dict) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for part, data in parts.items():
            archive.writestr(part, data)
    return buffer.getvalue()


class ScratchDirectory(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-verifier-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)


class FreshImportTests(unittest.TestCase):
    """Each gate reconfigures stdout for λ only when the stream supports it."""

    def load_fresh(self, name):
        spec = importlib.util.spec_from_file_location(f"_fresh_{name}", TOOLS / f"{name}.py")
        module = importlib.util.module_from_spec(spec)
        stream = io.StringIO()     # a StringIO has no reconfigure()
        with mock.patch.object(sys, "stdout", stream), \
             mock.patch.object(sys, "argv", [f"{name}.py"]), \
             mock.patch.object(sys, "path", list(sys.path)):
            spec.loader.exec_module(module)
        self.assertEqual(stream.getvalue(), "")
        return module

    def test_gates_import_under_a_stream_that_cannot_be_reconfigured(self):
        self.assertEqual(self.load_fresh("verify_sources").SRC_DIR, "src")
        self.assertEqual(self.load_fresh("verify_workbook").WORKBOOK, "ozzit.xlsx")
        self.assertEqual(self.load_fresh("verify_signatures").SRC, "src")
        self.assertEqual(self.load_fresh("verify_cover").CHANGELOG, Path("CHANGELOG.md"))
        previous = self.load_fresh("verify_previous_names")
        self.assertEqual(previous.CSV, "functions.csv")
        self.assertEqual(Path(previous.BASELINE).name, "released-names-v1.2.6.txt")


class SourceHelperTests(unittest.TestCase):
    def test_an_unterminated_literal_runs_to_the_end(self):
        self.assertEqual(verify_sources.read_string('"abc', 0), ('"abc', 4))

    def test_a_final_statement_without_a_semicolon_is_kept(self):
        self.assertEqual(verify_sources.statements("A = 1; B = 2"), ["A = 1", " B = 2"])

    def test_literals_at_either_end_leave_no_empty_code_chunk(self):
        self.assertEqual(verify_sources.split_literals('"a"b'), [(True, '"a"'), (False, "b")])
        self.assertEqual(verify_sources.split_literals('b"a"'), [(False, "b"), (True, '"a"')])

    def test_an_unclosed_single_is_left_as_written(self):
        self.assertEqual(verify_sources.unwrap_single("1+SINGLE(2"), "1+SINGLE(2")

    def test_qualify_without_names_returns_the_formula(self):
        self.assertEqual(verify_sources.qualify("Fooλ(1)", "oz", []), "Fooλ(1)")

    def test_an_unclosed_call_yields_the_arguments_seen(self):
        self.assertEqual(verify_sources.call_arguments("f(a,b", 1), ["a"])

    def test_a_let_without_a_result_is_read_as_an_ordinary_call(self):
        self.assertTrue(verify_sources.passed_through("LET(x, 1)", "x", frozenset()))
        self.assertFalse(verify_sources.passed_through("LET(x, 1)", "x", frozenset({"LET"})))

    def test_a_stray_closing_bracket_does_not_hide_a_read(self):
        self.assertTrue(verify_sources.reads_any(") x", {"X"}, frozenset({"SUM"})))


class VerifySourcesMainTests(ScratchDirectory):
    def run_main(self, workbook, src):
        output = io.StringIO()
        with mock.patch.object(verify_sources, "failures", []) as failures, \
             mock.patch.object(verify_sources, "WORKBOOK", str(workbook)), \
             mock.patch.object(verify_sources, "SRC_DIR", str(src)), \
             contextlib.redirect_stdout(output):
            result = verify_sources.main()
        return result, list(failures), output.getvalue()

    def test_every_source_defect_is_reported(self):
        src = self.directory / "src"
        src.mkdir()
        (src / "Dates.txt").write_text(
            "Sameλ = LAMBDA(x, x + 1);\n"
            "Diffλ = LAMBDA(x, x + 2);\n"
            "Deadλ = LAMBDA(d, LET(CvtD, DATEVALUE(d), d));\n"
            "not a declaration;\n   ;\n",
            encoding="utf-8",
        )
        (src / "Extra.txt").write_text(
            "Sameλ = LAMBDA(x, x + 1);\nStoredλ = LAMBDA(x, _xlfn.SINGLE(x));\n",
            encoding="utf-8",
        )
        names = {
            "oz.Sameλ": "LAMBDA(_xlpm.x, _xlpm.x + 1)",
            "oz.Diffλ": "LAMBDA(x, x + 3)",
            "oz.Deadλ": "LAMBDA(d, LET(CvtD, DATEVALUE(d), d))",
            "oz.Storedλ": "LAMBDA(x, SINGLE(x))",
            "oz.ShippedOnlyλ": "1",
        }
        body = "".join(f'<definedName name="{n}">{b}</definedName>' for n, b in names.items())
        workbook = self.directory / "ozzit.xlsx"
        workbook.write_bytes(zip_bytes({"xl/workbook.xml": f"<workbook><definedNames>{body}"
                                                          "</definedNames></workbook>"}))

        with mock.patch.object(verify_sources, "KNOWN_COLLAPSED_DEFAULTS", {}):
            result, failures, printed = self.run_main(workbook, src)

        expected = [f"missing source module {src / f'{m}.txt'}"
                    for m in ("Essentials", "Financial", "Ratios", "Utilities", "Debt")]
        expected += [
            "Deadλ binds CvtD and never reads it, so the argument it converts is used in its "
            "raw form",
            f"{src / 'Dates.txt'} has an unparsable statement: 'not a declaration'",
            f"{src / 'Extra.txt'} contains the stored-form token '_xlfn.', which Excel will not "
            "accept as typed input",
            "Sameλ is declared in more than one module",
            "oz.ShippedOnlyλ ships in the workbook but is not in src/",
            "oz.Diffλ differs from its published source at character 11\n"
            "        src/ : LAMBDA(X,X+2)\n        xlsx : LAMBDA(X,X+3)",
        ]
        self.assertEqual(result, 1)
        self.assertEqual(failures, expected)
        self.assertTrue(printed.startswith(
            f"FAIL: {len(expected)} problem(s) comparing {workbook} with {src}/\n"), printed)

    def test_the_committed_sources_reproduce_the_workbook(self):
        src, workbook = ROOT / "src", ROOT / "ozzit.xlsx"
        result, failures, printed = self.run_main(workbook, src)
        self.assertEqual(result, 0, printed)
        self.assertEqual(failures, [])
        self.assertRegex(printed, r"^OK: %s/ reproduces all \d+ functions in %s, " % (
            re.escape(str(src)), re.escape(str(workbook))))
        self.assertIn("only the %d listed argument defaults"
                      % len(verify_sources.KNOWN_COLLAPSED_DEFAULTS), printed)


class VerifyWorkbookTests(unittest.TestCase):
    WEB_EXTENSION = {
        "xl/webextensions/webextension1.xml":
            '<webextension><reference id="wa200003696" storeType="OMEX"/></webextension>',
        "xl/webextensions/taskpanes.xml": '<taskpanes><taskpane visibility="0"/></taskpanes>',
        "xl/webextensions/_rels/taskpanes.xml.rels":
            '<Relationships><Relationship Id="rId1" Target="webextension1.xml"/></Relationships>',
    }

    def run_main(self, parts):
        output = io.StringIO()
        with mock.patch.object(verify_workbook, "failures", []) as failures, \
             mock.patch.object(verify_workbook, "WORKBOOK", io.BytesIO(zip_bytes(parts))), \
             contextlib.redirect_stdout(output), \
             self.assertRaises(SystemExit) as raised:
            verify_workbook.main()
        return raised.exception.code, list(failures), output.getvalue()

    def test_every_content_defect_is_reported(self):
        store = base64.b64encode(
            json.dumps({"note": "dropbox", "padding": "x" * 80}).encode("utf-16-le")
        )
        parts = {
            **self.WEB_EXTENSION,
            "customXml/item1.xml": b"<store>" + store + b"</store>",
            "xl/workbook.xml": (
                '<workbook><sheets><sheet name="Data" sheetId="1"/></sheets><definedNames>'
                '<definedName name="oz.Goodλ">LAMBDA(x, x)</definedName>'
                '<definedName name="oz.Quoteλ">"open</definedName>'
                '<definedName name="oz.Callerλ">oz.Missingλ(1)</definedName>'
                '<definedName name="Slicer_Other">#N/A</definedName>'
                '</definedNames><calcPr fullCalcOnLoad="1"/></workbook>'
            ),
            "xl/worksheets/sheet1.xml": (
                '<worksheet><sheetData><row r="1">'
                '<c r="A1"><f>#REF!+1</f></c>'
                '<c r="B1"><f>oz.Unknownλ(1)</f></c>'
                '<c r="C1"><f>TODAY()</f></c>'
                '<c r="D1" ca="1"><f>1+1</f></c>'
                '<c r="E1" ca="1"><f>2+2</f></c>'
                '</row></sheetData></worksheet>'
            ),
            "xl/tables/table1.xml": (
                '<table><tableColumns><tableColumn name="A"><calculatedColumnFormula>'
                "OFFSET(A1,0,0)</calculatedColumnFormula></tableColumn></tableColumns></table>"
            ),
            "xl/styles.xml": '<styleSheet><font><name val="Calibri"/></font></styleSheet>',
            "xl/printerSettings/printerSettings1.bin": "dropbox".encode("utf-16-le"),
            "xl/sharedStrings.xml": '<sst uniqueCount="many"><si><t>a</t></si></sst>',
            "docProps/app.xml": (
                '<Properties xmlns:vt="urn:vt"><TitlesOfParts><vt:vector>'
                "<vt:lpstr>Other</vt:lpstr></vt:vector></TitlesOfParts></Properties>"
            ),
            "xl/slicerCaches/slicerCache1.xml": '<slicerCacheDefinition name="Slicer_Region"/>',
            "xl/slicerCaches/slicerCache2.xml": '<slicerCacheDefinition name="Slicer_Other"/>',
        }

        code, failures, printed = self.run_main(parts)

        expected = [
            "banned token 'Calibri' in xl/styles.xml",
            "banned token 'dropbox' in xl/printerSettings/printerSettings1.bin",
            "banned token 'dropbox' in the AFE project store",
            "unbalanced quotes in oz.Quoteλ",
            "oz.Callerλ references undefined oz.Missingλ",
            "#REF! present in xl/worksheets/sheet1.xml",
            "xl/worksheets/sheet1.xml uses undefined oz.Unknownλ",
            "volatile TODAY() in a formula in xl/worksheets/sheet1.xml",
            "volatile OFFSET( in a formula in xl/tables/table1.xml",
            # one report per part, however many flagged cells it holds
            "always-calculate flag on a non-volatile cell in xl/worksheets/sheet1.xml",
            "sharedStrings uniqueCount is not an integer: 'many'",
            "sharedStrings uniqueCount -1 != 1 entries",
            "docProps/app.xml lists 1 sheet titles that do not match the workbook's 1, "
            "first difference at 'Other'",
            "slicer cache Slicer_Region has no backing defined name",
        ]
        self.assertEqual(code, 1)
        self.assertCountEqual(failures, expected)
        self.assertIn(f"FAIL: {len(expected)} problem(s) in ", printed)

    def test_main_stops_at_a_missing_or_unreadable_afe_store(self):
        cases = (
            ("missing", {}, "AFE project store missing"),
            ("not UTF-16 JSON", {"customXml/item1.xml": "<store>" + "A" * 104 + "</store>"},
             "invalid AFE project store: Expecting value"),
        )
        for label, extra, message in cases:
            with self.subTest(label=label):
                parts = {**self.WEB_EXTENSION, "xl/workbook.xml": "<workbook/>", **extra}
                code, failures, printed = self.run_main(parts)
                self.assertEqual(code, 1)
                # stopped before the defined-name checks, which would add "no defined names"
                self.assertEqual(len(failures), 1, failures)
                self.assertTrue(failures[0].startswith(message), failures)
                self.assertIn("FAIL: 1 problem(s) in ", printed)


class PreviousNamesTests(ScratchDirectory):
    FIELDS = ("function", "previous_name", "signature", "description")

    def write_index(self, rows, fields=FIELDS):
        target = self.directory / "functions.csv"
        with target.open("w", encoding="utf-8", newline="") as stream:
            writer = csv.writer(stream, lineterminator="\n")
            writer.writerow(fields)
            writer.writerows(rows)
        return target

    def write_baseline(self, names):
        target = self.directory / "baseline.txt"
        target.write_text("# released names\n" + "".join(f"{n}\n" for n in names), encoding="utf-8")
        return target

    def run_main(self, index, baseline, floor=1):
        output = io.StringIO()
        with mock.patch.object(verify_previous_names, "CSV", str(index)), \
             mock.patch.object(verify_previous_names, "BASELINE", str(baseline)), \
             mock.patch.object(verify_previous_names, "FLOOR", floor), \
             contextlib.redirect_stdout(output):
            result = verify_previous_names.main()
        return result, output.getvalue()

    def test_every_index_defect_is_reported(self):
        baseline = self.write_baseline(["ozD.Alphaλ", "ozD.Bareλ", "ozD.Deltaλ", "ozD.Spareλ"])
        index = self.write_index([
            ("oz.AlphaDλ", "ozD.Alphaλ", "AlphaDλ(x)", "Alpha"),
            ("oz.Betaλ", "ozD.Nopeλ", "Betaλ(x)", "Beta"),
            ("oz.AlphaEλ", "ozD.Alphaλ", "AlphaEλ(x)", "Alpha again"),
            ("Bareλ", "ozD.Bareλ", "Bareλ(x)", "Bare"),
            ("oz.Gammaλ", "ozD.Deltaλ", "Gammaλ(x)", "Gamma"),
            ("oz.Sigλ", "", "Sigλ(x", "Sig"),
            ("oz.Escλ", "", "Escλ(x)", "line_x000D_break"),
            ("oz.Arrowλ", "", "Arrowλ(x)", "→ Arrow"),
            ("oz.NoSigλ", "", "", "No signature"),
            ("oz.NoDescλ", "", "NoDescλ(x)", ""),
        ])

        result, printed = self.run_main(index, baseline)

        expected = [
            "oz.Betaλ claims to replace ozD.Nopeλ, which baseline.txt never shipped",
            "ozD.Alphaλ is claimed by both oz.AlphaDλ and oz.AlphaEλ",
            "Bareλ is not a qualified function name",
            "oz.Gammaλ is not a renaming of ozD.Deltaλ: Gammaλ does not begin with Delta",
            "oz.Sigλ publishes a signature with unbalanced brackets: Sigλ(x",
            "oz.Escλ's description carries a raw OOXML escape: line_x000D_break",
            "oz.Arrowλ's description starts with the help table's column delimiter",
            "oz.NoSigλ publishes an empty signature",
            "oz.NoDescλ publishes an empty description",
            "ozD.Spareλ shipped in the baseline and no function claims it",
        ]
        self.assertEqual(result, 1)
        self.assertEqual(
            printed.splitlines(),
            [f"FAIL: {len(expected)} problem(s) in {index}", *(f"  - {e}" for e in expected)],
        )

    def test_an_index_the_gate_cannot_read_stops_it(self):
        baseline = self.write_baseline(["ozD.Alphaλ"])
        row = ("oz.Alphaλ", "ozD.Alphaλ", "Alphaλ(x)", "Alpha")
        cases = (
            ("no rows", self.write_index, ([],), {}, "no rows read from {index}"),
            ("no column", self.write_index, ([row[::2]],), {"fields": ("function", "signature")},
             "{index} has no previous_name column"),
            ("below the floor", self.write_index, ([row],), {},
             "only 1 functions and 1 baseline names read: the check is not reading them"),
        )
        for label, writer, args, kwargs, message in cases:
            with self.subTest(label=label):
                index = writer(*args, **kwargs)
                floor = 100 if label == "below the floor" else 1
                with self.assertRaises(SystemExit) as caught:
                    self.run_main(index, baseline, floor)
                self.assertEqual(caught.exception.code, message.format(index=index))


def help_function(name, params, signature, table, example):
    rows = [f"FUNCTION:→{signature}", "PARAMETERS:→", *(f"{p}→a value" for p in table),
            "EXAMPLES:→", f"EXAMPLE→{example}", "END:→"]
    help_text = " & ".join(f'"{row}¶"' for row in rows)
    declared = ", ".join(f"[{p}]" for p in params)
    return f'{name} = LAMBDA({declared}, LET(Help, TRIM(TEXTSPLIT({help_text}, "→", "¶")), 0));\n'


class SignatureTests(ScratchDirectory):
    def test_an_unterminated_literal_is_read_to_the_end(self):
        self.assertEqual(verify_signatures.literals('a "open'), "open")

    def test_a_lambda_with_only_a_body_takes_nothing(self):
        self.assertEqual(verify_signatures.declared("LAMBDA(42)"), [])

    def write_fixture(self):
        src = self.directory / "src"
        src.mkdir()
        (src / "Fixture.txt").write_text(
            help_function("Goodλ", ["x"], "Goodλ(x)", ["x"], "=Goodλ(1)")
            + help_function("NotSigλ", ["x"], "not a signature", ["x"], "=NotSigλ(1)")
            + help_function("WrongNameλ", ["x"], "Goodλ(x)", ["x"], "=WrongNameλ(1)")
            + help_function("WrongParamsλ", ["x", "y"], "WrongParamsλ(x)", ["x", "y"],
                            "=WrongParamsλ(1, 2)")
            + help_function("WrongExampleλ", ["x"], "WrongExampleλ(x)", ["x"], "=Goodλ(1)")
            + help_function("WrongTableλ", ["x"], "WrongTableλ(x)", ["z"], "=WrongTableλ(1)")
            # an About row for a declared function with no description is skipped
            + 'AboutFixtureλ = TRIM(TEXTSPLIT("Function→Description¶" & "Goodλ→¶", "→", "¶"));\n'
            + "not a declaration;\n",
            encoding="utf-8",
        )
        return src

    def run_main(self, src, floor):
        output = io.StringIO()
        with mock.patch.object(verify_signatures, "SRC", str(src)), \
             mock.patch.object(verify_signatures, "FLOOR", floor), \
             contextlib.redirect_stdout(output):
            result = verify_signatures.main()
        return result, output.getvalue()

    def test_each_help_mismatch_is_reported(self):
        result, printed = self.run_main(self.write_fixture(), 1)

        self.assertEqual(result, 1)
        lines = printed.splitlines()
        self.assertEqual(
            lines[0], "7 declarations read: 6 signatures, 6 parameter tables and 6 example blocks "
            "checked")
        self.assertEqual(lines[3], "   not a LAMBDA (1): AboutFixtureλ")
        self.assertEqual(lines[4:], [
            "FAIL: 5 help problem(s)",
            "  - NotSigλ: the help's FUNCTION line is not a signature: 'not a signature'",
            "  - WrongNameλ: its help names Goodλ",
            "  - WrongParamsλ: its signature says (x) but it takes (x, y)",
            "  - WrongExampleλ's worked examples never call it, they call Goodλ",
            "  - WrongTableλ: its parameter table describes (z) but it takes (x)",
        ])

    def test_too_few_signatures_fail_the_floor(self):
        src = self.write_fixture()
        with self.assertRaises(SystemExit) as caught:
            self.run_main(src, 100)
        self.assertEqual(
            caught.exception.code,
            "only 6 signatures and 6 tables parsed from 1 module(s): the parser is not reading them",
        )

    def test_an_empty_source_directory_is_refused(self):
        with self.assertRaises(SystemExit) as caught:
            self.run_main(self.directory, 1)
        self.assertEqual(caught.exception.code, "no source modules found in %s/" % self.directory)


class CoverTests(ScratchDirectory):
    LABEL = "Version 1 May 2026 - 3 functions - Excel"

    def write_cover(self, *, sheet="Cover", rid="rId1", target="worksheets/sheet1.xml",
                    cell='<c r="A3" t="s"><v>0</v></c>', strings=None):
        strings = strings or f"<sst><si><t>{self.LABEL}</t></si></sst>"
        workbook = self.directory / "ozzit.xlsx"
        workbook.write_bytes(zip_bytes({
            "xl/workbook.xml": f'<workbook><sheets><sheet name="{sheet}" sheetId="1" r:id="rId1"/>'
                               "</sheets></workbook>",
            "xl/_rels/workbook.xml.rels": f'<Relationships><Relationship Id="{rid}" Type="t" '
                                          f'Target="{target}"/></Relationships>',
            "xl/worksheets/sheet1.xml": f"<worksheet><sheetData><row r=\"3\">{cell}</row>"
                                        "</sheetData></worksheet>",
            "xl/sharedStrings.xml": strings,
        }))
        return workbook

    def test_the_label_is_read_through_an_absolute_target_and_rich_text(self):
        strings = ('<sst><si><r><t>Version 1 May 2026</t></r>'
                   '<r><t xml:space="preserve"> - 3 functions - Excel &amp; more</t></r></si></sst>')
        workbook = self.write_cover(target="/xl/worksheets/sheet1.xml", strings=strings)
        self.assertEqual(verify_cover.cover_label(workbook), ("1 May 2026", 3))

    def test_each_unresolvable_cover_cell_is_named(self):
        part = "xl/worksheets/sheet1.xml"
        cases = (
            ({"sheet": "Data"}, "xl/workbook.xml declares no sheet named Cover"),
            ({"rid": "rId9"}, "the Cover sheet has no relationship target"),
            ({"cell": '<c r="A3"><v>0</v></c>'}, f"{part}: Cover!A3 is not a shared-string cell"),
            ({"cell": '<c r="B3" t="s"><v>0</v></c>'},
             f"{part}: Cover!A3 is not a shared-string cell"),
            ({"cell": '<c r="A3" t="s"></c>'}, f"{part}: Cover!A3 carries no shared-string index"),
            ({"cell": '<c r="A3" t="s"><v>5</v></c>'},
             "Cover!A3 points at shared string 5, beyond the 1 stored"),
            ({"strings": "<sst><si><t>Hello</t></si></sst>"},
             "Cover!A3 is not a version label: 'Hello'"),
        )
        for overrides, message in cases:
            with self.subTest(message=message, overrides=overrides):
                workbook = self.write_cover(**overrides)
                with self.assertRaises(ValueError) as caught:
                    verify_cover.cover_label(workbook)
                self.assertEqual(str(caught.exception), message)

    def test_an_empty_index_and_a_headingless_changelog_are_refused(self):
        index = self.directory / "functions.csv"
        index.write_text("function,signature\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "^functions.csv lists no functions$"):
            verify_cover.published_functions(index)
        changelog = self.directory / "CHANGELOG.md"
        changelog.write_text("# Changelog\n\nNothing released yet.\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, r"^CHANGELOG.md has no '## vX\.Y\.Z, <date>,' heading$"):
            verify_cover.current_cut_date(changelog)

    def run_main(self, workbook, index, changelog):
        output = io.StringIO()
        with mock.patch.object(verify_cover, "WORKBOOK", workbook), \
             mock.patch.object(verify_cover, "INDEX", index), \
             mock.patch.object(verify_cover, "CHANGELOG", changelog), \
             contextlib.redirect_stdout(output):
            result = verify_cover.main()
        return result, output.getvalue()

    def test_main_reports_a_matching_cover_and_an_unreadable_one(self):
        index = self.directory / "functions.csv"
        index.write_text("function\noz.Aλ\noz.Bλ\noz.Cλ\noz.AboutDatesλ\n", encoding="utf-8")
        changelog = self.directory / "CHANGELOG.md"
        changelog.write_text("# Changelog\n\n## v1.0.0, 1 May 2026, first cut\n", encoding="utf-8")

        result, printed = self.run_main(self.write_cover(), index, changelog)
        self.assertEqual((result, printed), (0, "OK: cover label states 1 May 2026 and 3 functions\n"))

        missing = self.directory / "missing.xlsx"
        result, printed = self.run_main(missing, index, changelog)
        self.assertEqual(result, 1)
        self.assertTrue(printed.startswith("FAIL: "), printed)
        self.assertIn("missing.xlsx", printed)


if __name__ == "__main__":
    unittest.main()
