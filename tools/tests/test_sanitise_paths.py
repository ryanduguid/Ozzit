"""Excel save residue, command-line paths and the Excel refresh wrapper's failures.

Uses a small synthetic workbook built with zipfile; Excel and PowerShell are
always replaced with fakes.
"""

import importlib.util
import io
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
import zipfile
from contextlib import redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import refresh_cache  # noqa: E402
import sanitise_workbook  # noqa: E402

PRINTER_CT = "application/vnd.openxmlformats-officedocument.spreadsheetml.printerSettings"
TYPES_OPEN = '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
XML_DEFAULT = '<Default Extension="xml" ContentType="application/xml"/>'
CONTENT_TYPES = (
    TYPES_OPEN
    + f'<Default Extension="bin" ContentType="{PRINTER_CT}"/>'
    + XML_DEFAULT
    + '<Override PartName="/xl/customProperty1.bin" '
    + f'ContentType="{sanitise_workbook.CUSTOMPROP_CT}"/>'
    + "</Types>"
)
CLEAN_CONTENT_TYPES = (
    TYPES_OPEN
    + f'<Default Extension="bin" ContentType="{sanitise_workbook.CUSTOMPROP_CT}"/>'
    + XML_DEFAULT
    + "</Types>"
)
BOOK_VIEWS = f"<bookViews><workbookView {sanitise_workbook.FIXED_WINDOW}/></bookViews>"
SAVE_PATH = (
    '<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">'
    '<mc:Choice Requires="x15">'
    '<x15ac:absPath url="/home/saver/books/" '
    'xmlns:x15ac="http://schemas.microsoft.com/office/spreadsheetml/2010/11/ac"/>'
    "</mc:Choice></mc:AlternateContent>"
)
WORKBOOK_XML = f"<workbook>{SAVE_PATH}{BOOK_VIEWS}</workbook>"
SHEET_XML = (
    "<worksheet><sheetData><row>"
    '<c r="A1" ca="1"><f>A2+1</f><v>2</v></c>'
    '<c r="B1" aca="1" ca="1"><f>A1*2</f><v>4</v></c>'
    '<c r="C1" ca="1"><f>CELL("filename")</f></c>'
    '<c r="D1"><v>1</v></c>'
    "</row></sheetData></worksheet>"
)
CLEAN_SHEET_XML = (
    "<worksheet><sheetData><row>"
    '<c r="A1"><f>A2+1</f><v>2</v></c>'
    '<c r="B1"><f>A1*2</f><v>4</v></c>'
    '<c r="C1" ca="1"><f>CELL("filename")</f></c>'
    '<c r="D1"><v>1</v></c>'
    "</row></sheetData></worksheet>"
)
CORE_XML = (
    "<cp:coreProperties><dc:creator>Ozzit project</dc:creator>"
    "<cp:lastModifiedBy>Ozzit project</cp:lastModifiedBy>"
    f'<dcterms:modified xsi:type="dcterms:W3CDTF">{sanitise_workbook.FIXED_STAMP}'
    "</dcterms:modified></cp:coreProperties>"
)


def excel_saved_parts():
    return {
        "[Content_Types].xml": CONTENT_TYPES.encode(),
        "docProps/core.xml": CORE_XML.encode(),
        "xl/workbook.xml": WORKBOOK_XML.encode(),
        "xl/worksheets/sheet1.xml": SHEET_XML.encode(),
    }


class SanitiseResidueTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-sanitise-paths-"))
        self.workbook = self.directory / "saved.xlsx"
        with zipfile.ZipFile(self.workbook, "w") as archive:
            for name, data in excel_saved_parts().items():
                archive.writestr(name, data)

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def parts(self):
        with zipfile.ZipFile(self.workbook) as archive:
            return {name: archive.read(name) for name in archive.namelist()}

    def test_excel_save_residue_is_removed(self):
        log = sanitise_workbook.sanitise(self.workbook)

        self.assertEqual(
            log,
            [
                f"[Content_Types].xml {len(CONTENT_TYPES)} -> {len(CLEAN_CONTENT_TYPES)} bytes",
                "removed 1 x15ac:absPath",
                "cleared always-calculate flags on 2 cells",
            ],
        )
        self.assertEqual(
            self.parts(),
            {
                "[Content_Types].xml": CLEAN_CONTENT_TYPES.encode(),
                "docProps/core.xml": CORE_XML.encode(),
                "xl/workbook.xml": f"<workbook>{BOOK_VIEWS}</workbook>".encode(),
                "xl/worksheets/sheet1.xml": CLEAN_SHEET_XML.encode(),
            },
        )
        self.assertEqual(sanitise_workbook.sanitise(self.workbook), ["already clean"])


class SanitiseCommandLineTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-sanitise-cli-"))
        self.workbook = self.directory / "saved.xlsx"
        sanitise_workbook.write_deterministic(self.workbook, excel_saved_parts())

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def run_main(self, *arguments):
        printed = io.StringIO()
        argv = ["sanitise_workbook.py", *arguments]
        with mock.patch.object(sys, "argv", argv), redirect_stdout(printed):
            sanitise_workbook.main()
        return printed.getvalue()

    def test_usage_is_required(self):
        for arguments in ((), ("a.xlsx", "b.xlsx")):
            with self.subTest(arguments=arguments), self.assertRaises(SystemExit) as caught:
                self.run_main(*arguments)
            self.assertEqual(
                caught.exception.code,
                "FAIL: usage: python tools/sanitise_workbook.py ozzit.xlsx",
            )

    def test_missing_and_unreadable_workbooks_fail_cleanly(self):
        missing = self.directory / "missing.xlsx"
        with self.assertRaises(SystemExit) as caught:
            self.run_main(str(missing))
        self.assertEqual(caught.exception.code, f"FAIL: no such workbook: {missing}")

        broken = self.directory / "broken.xlsx"
        broken.write_bytes(b"not a zip")
        with self.assertRaises(SystemExit) as caught:
            self.run_main(str(broken))
        self.assertTrue(
            caught.exception.code.startswith(f"FAIL: cannot sanitise {broken}: "),
            caught.exception.code,
        )

    def test_success_prints_each_change_and_the_final_size(self):
        printed = self.run_main(str(self.workbook))

        size = self.workbook.stat().st_size
        self.assertEqual(
            printed.splitlines(),
            [
                f"[Content_Types].xml {len(CONTENT_TYPES)} -> {len(CLEAN_CONTENT_TYPES)} bytes",
                "removed 1 x15ac:absPath",
                "cleared always-calculate flags on 2 cells",
                f"OK: {self.workbook} sanitised, {size:,} bytes",
            ],
        )


class StagingPermissionTests(unittest.TestCase):
    """stage_bytes copies permission bits only where os.name is not "nt".

    The module's os is replaced with a stand-in so both branches run on every
    platform; chmod is observed rather than applied.
    """

    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-stage-"))
        self.destination = self.directory / "book.xlsx"

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def stage(self, os_name):
        with (
            mock.patch.object(sanitise_workbook, "os", SimpleNamespace(name=os_name)),
            mock.patch.object(sanitise_workbook.Path, "chmod", autospec=True) as chmod,
        ):
            staged = sanitise_workbook.stage_bytes(self.destination, b"new bytes")
        self.addCleanup(staged.unlink, missing_ok=True)
        self.assertEqual(staged.parent, self.directory)
        self.assertEqual(staged.read_bytes(), b"new bytes")
        return staged, chmod

    def test_posix_staging_copies_the_destination_mode(self):
        self.destination.write_bytes(b"old bytes")
        staged, chmod = self.stage("posix")
        chmod.assert_called_once_with(staged, stat.S_IMODE(self.destination.stat().st_mode))

    def test_posix_staging_for_a_new_destination_keeps_the_default_mode(self):
        _, chmod = self.stage("posix")
        chmod.assert_not_called()

    def test_windows_staging_leaves_permissions_alone(self):
        self.destination.write_bytes(b"old bytes")
        _, chmod = self.stage("nt")
        chmod.assert_not_called()


class RefreshCacheTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-refresh-"))
        self.workbook = self.directory / "ozzit.xlsx"
        self.workbook.write_bytes(b"workbook")

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def refresh(self, workbook=None, **run):
        printed = io.StringIO()
        with (
            mock.patch.object(refresh_cache, "WORKBOOK", str(workbook or self.workbook)),
            mock.patch.object(refresh_cache.subprocess, "run", **run) as excel,
            mock.patch.object(refresh_cache, "sanitise", return_value=["sanitised"]) as clean,
            redirect_stdout(printed),
        ):
            code = refresh_cache.main()
        return code, printed.getvalue(), excel, clean

    def test_missing_workbook_fails_before_excel_runs(self):
        missing = self.directory / "missing.xlsx"
        code, printed, excel, clean = self.refresh(missing)
        self.assertEqual((code, printed), (1, f"FAIL: no such workbook: {missing}\n"))
        excel.assert_not_called()
        clean.assert_not_called()

    def test_missing_powershell_is_reported(self):
        code, printed, _, clean = self.refresh(side_effect=FileNotFoundError("powershell.exe"))
        self.assertEqual(
            (code, printed),
            (1, "FAIL: powershell.exe was not found, so Excel could not be driven\n"),
        )
        clean.assert_not_called()

    def test_hung_refresh_is_reported(self):
        timeout = subprocess.TimeoutExpired("powershell.exe", 600)
        code, printed, _, clean = self.refresh(side_effect=timeout)
        self.assertEqual(code, 1)
        self.assertEqual(
            printed,
            "FAIL: the Excel refresh did not finish within 600s; check for a modal dialog "
            "and close any Excel process before retrying\n",
        )
        clean.assert_not_called()

    def test_failed_refresh_prints_its_exit_code_and_trimmed_error(self):
        failed = SimpleNamespace(returncode=5, stdout="", stderr="  " + "x" * 2500 + "\n")
        code, printed, _, clean = self.refresh(return_value=failed)
        self.assertEqual(code, 1)
        self.assertEqual(
            printed,
            "FAIL: the Excel refresh did not complete (exit 5)\n" + "x" * 2000 + "\n",
        )
        clean.assert_not_called()

    def test_failed_refresh_without_error_text_prints_only_the_exit_code(self):
        failed = SimpleNamespace(returncode=2, stdout="opening\n", stderr=" \n")
        code, printed, _, _ = self.refresh(return_value=failed)
        self.assertEqual(code, 1)
        self.assertEqual(printed, "opening\nFAIL: the Excel refresh did not complete (exit 2)\n")

    def test_silent_successful_refresh_goes_straight_to_the_sanitiser(self):
        done = SimpleNamespace(returncode=0, stdout=None, stderr="")
        code, printed, excel, clean = self.refresh(return_value=done)
        self.assertEqual(code, 0)
        self.assertEqual(
            printed,
            f"sanitised\nOK: {self.workbook} holds the values its own formulas produce\n",
        )
        script = str(refresh_cache.HERE / "refresh_cache.ps1")
        self.assertEqual(
            excel.call_args.args[0],
            ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script,
             "-Path", str(self.workbook.resolve())],
        )
        self.assertEqual(excel.call_args.kwargs["timeout"], refresh_cache.REFRESH_TIMEOUT_SECONDS)
        clean.assert_called_once_with(self.workbook)

    def test_import_reads_the_workbook_argument_and_tolerates_a_plain_stdout(self):
        # A captured or redirected stdout may have no reconfigure(); importing must still work.
        for argv, expected in ((["refresh_cache.py"], "ozzit.xlsx"),
                               (["refresh_cache.py", "other.xlsx"], "other.xlsx")):
            with self.subTest(argv=argv):
                spec = importlib.util.spec_from_file_location(
                    "refresh_cache_plain_stdout", TOOLS / "refresh_cache.py"
                )
                module = importlib.util.module_from_spec(spec)
                stream = io.StringIO()
                with mock.patch.object(sys, "argv", argv), redirect_stdout(stream):
                    spec.loader.exec_module(module)
                self.assertEqual(module.WORKBOOK, expected)
                self.assertIs(module.sanitise, sanitise_workbook.sanitise)
                self.assertEqual(stream.getvalue(), "")


if __name__ == "__main__":
    unittest.main()
