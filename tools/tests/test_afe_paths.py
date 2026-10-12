"""Paths through the AFE store gate and its sync tool that the workbook tests never reach."""

import base64
import contextlib
import io
import json
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import sync_afe_store  # noqa: E402
import verify_afe  # noqa: E402

PADDING = "x" * 80   # keeps a small store's payload over the 100-character blob floor


def encode_store(value) -> bytes:
    return base64.b64encode(json.dumps(value, ensure_ascii=False).encode("utf-16-le"))


def defined_names(*names: str) -> str:
    body = "".join(f'<definedName name="{name}">1</definedName>' for name in names)
    return f"<workbook><definedNames>{body}</definedNames></workbook>"


class AfeFixture(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-afe-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def write_archive(self, name, parts):
        target = self.directory / name
        with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
            for part, data in parts.items():
                archive.writestr(part, data)
        return target

    def write_store(self, value, name="afe.xlsx", **extra):
        parts = {"customXml/item1.xml": b"<store>" + encode_store(value) + b"</store>", **extra}
        return self.write_archive(name, parts)

    def write_src(self, modules):
        src = self.directory / "src"
        src.mkdir(exist_ok=True)
        for module, text in modules.items():
            (src / f"{module}.txt").write_text(text, encoding="utf-8")
        return src


class DecodeStoreTests(AfeFixture):
    def test_a_store_part_without_a_payload_is_named(self):
        with self.assertRaisesRegex(ValueError, "^AFE store has no base64 payload$"):
            verify_afe.find_afe_blob(b"<store>short</store>")
        workbook = self.write_archive("empty.xlsx", {"customXml/item1.xml": b"<store>short</store>"})
        with self.assertRaisesRegex(ValueError, "^AFE store has no base64 payload$"):
            verify_afe.decode_store(workbook)

    def test_an_unreadable_workbook_is_reported_as_unreadable(self):
        not_zip = self.directory / "not-a-zip.xlsx"
        not_zip.write_bytes(b"not a zip archive")
        cases = (
            ("missing file", self.directory / "missing.xlsx", "No such file"),
            ("no store part", self.write_archive("bare.xlsx", {"xl/workbook.xml": "<workbook/>"}),
             "customXml/item1.xml"),
            ("not a zip", not_zip, "File is not a zip file"),
        )
        for label, workbook, detail in cases:
            with self.subTest(label=label):
                with self.assertRaises(ValueError) as caught:
                    verify_afe.decode_store(workbook)
                self.assertTrue(str(caught.exception).startswith("cannot read AFE store: "))
                self.assertIn(detail, str(caught.exception))

    def test_a_payload_that_is_not_utf16_json_cannot_be_decoded(self):
        for size, detail in ((100, "truncated data"), (104, "Expecting value")):
            with self.subTest(detail=detail):
                # 100 characters decode to an odd 75 bytes; 104 to 78 bytes of NULs.
                workbook = self.write_archive(
                    "garbled.xlsx", {"customXml/item1.xml": b"<store>" + b"A" * size + b"</store>"}
                )
                with self.assertRaises(ValueError) as caught:
                    verify_afe.decode_store(workbook)
                self.assertTrue(str(caught.exception).startswith("cannot decode AFE store: "))
                self.assertIn(detail, str(caught.exception))

    def test_each_malformed_store_shape_is_named(self):
        cases = (
            ({"schema": 1, "files": [], "projectNames": []}, "schema must be a string"),
            ({"schema": verify_afe.SCHEMA, "files": {}, "projectNames": []},
             "files must be a list"),
            ({"schema": verify_afe.SCHEMA, "files": ["Dates"], "projectNames": []},
             r"files\[0\] must be an object"),
        )
        for value, message in cases:
            with self.subTest(message=message):
                workbook = self.write_store({**value, "padding": PADDING})
                with self.assertRaisesRegex(ValueError, "^cannot decode AFE store: " + message):
                    verify_afe.decode_store(workbook)


class CheckTests(AfeFixture):
    def test_an_unreadable_store_is_the_only_failure(self):
        missing = self.directory / "missing.xlsx"
        failures = verify_afe.check(missing, self.directory)
        self.assertEqual(len(failures), 1)
        self.assertTrue(failures[0].startswith("cannot read AFE store: "), failures)

    def test_schema_missing_source_and_unknown_name_are_each_reported(self):
        texts = {module: f"{module} source\n" for module in verify_afe.MODULES}
        src = self.write_src({m: t for m, t in texts.items() if m != "Dates"})
        workbook = self.write_store(
            {
                "schema": "http://example.invalid/afeprojects/0.1",
                "files": [{"path": f"/projects/{m}", "text": t} for m, t in texts.items()],
                "projectNames": ["oz.Keptλ", "oz.Ghostλ"],
            },
            **{"xl/workbook.xml": defined_names("oz.Keptλ")},
        )

        self.assertEqual(
            verify_afe.check(workbook, src),
            [
                "AFE schema is 'http://example.invalid/afeprojects/0.1'",
                f"source module {src / 'Dates.txt'} missing",
                "AFE projectNames has unknown name oz.Ghostλ",
            ],
        )


class SyncTests(AfeFixture):
    def sync_fixture(self):
        src = self.write_src({
            "Dates": "Aλ = LAMBDA(x, x);\nCλ = LAMBDA(x, x + 1);\n",
            "Essentials": "Bλ = LAMBDA(x, x);\n",
            "Financial": "",
            "Ratios": "",
            "Utilities": "not a declaration;\n",
            "Debt": "",
        })
        store = {
            "schema": verify_afe.SCHEMA,
            "files": [
                {"path": "/projects/Dates", "text": "stale"},
                {"path": "/projects/Essentials", "text": "Bλ = LAMBDA(x, x);\n"},
            ],
            "projectNames": ["oz.Aλ", "oz.Bλ", "oz.Goneλ"],
        }
        workbook = self.write_store(
            store, name="ozzit.xlsx",
            **{"xl/workbook.xml": defined_names("oz.Aλ", "oz.Bλ", "oz.Cλ", "oz.Zλ")},
        )
        return workbook, src

    def test_module_of_maps_only_declarations(self):
        _workbook, src = self.sync_fixture()
        self.assertEqual(
            sync_afe_store.module_of(src),
            {"oz.Aλ": "Dates", "oz.Cλ": "Dates", "oz.Bλ": "Essentials"},
        )

    def test_sync_adds_missing_modules_and_files_new_names_beside_their_module(self):
        workbook, src = self.sync_fixture()

        changes = sync_afe_store.sync(workbook, src)

        self.assertEqual(
            changes,
            [
                "synced AFE module /projects/Dates",
                "added AFE module /projects/Financial",
                "added AFE module /projects/Ratios",
                "added AFE module /projects/Utilities",
                "added AFE module /projects/Debt",
                "synced AFE projectNames: 2 added, 1 dropped, 4 listed",
            ],
        )
        store = verify_afe.decode_store(workbook)
        # oz.Cλ follows the last Dates name; oz.Zλ has no module, so it goes last.
        self.assertEqual(store["projectNames"], ["oz.Aλ", "oz.Cλ", "oz.Bλ", "oz.Zλ"])
        self.assertEqual(verify_afe.check(workbook, src), [])
        self.assertEqual(sync_afe_store.sync(workbook, src), ["AFE store already matches src/"])

    def run_main(self, *args):
        output = io.StringIO()
        with mock.patch.object(sys, "argv", ["sync_afe_store.py", *map(str, args)]), \
             contextlib.redirect_stdout(output):
            sync_afe_store.main()
        return output.getvalue()

    def test_main_prints_each_change_and_the_synced_size(self):
        workbook, src = self.sync_fixture()

        printed = self.run_main(workbook, src).splitlines()

        self.assertEqual(printed[0], "synced AFE module /projects/Dates")
        self.assertEqual(len(printed), 7)
        self.assertEqual(
            printed[-1], f"OK: {workbook} AFE store synced, {workbook.stat().st_size:,} bytes"
        )

    def test_main_refuses_bad_arguments_and_unreadable_workbooks(self):
        not_zip = self.directory / "not-a-zip.xlsx"
        not_zip.write_bytes(b"not a zip archive")
        missing = self.directory / "missing.xlsx"
        cases = (
            (("a", "b", "c"), "FAIL: usage: python tools/sync_afe_store.py [workbook] [src dir]"),
            ((missing,), f"FAIL: no such workbook: {missing}"),
            ((not_zip, self.directory), "FAIL: cannot sync AFE store: File is not a zip file"),
        )
        for args, message in cases:
            with self.subTest(message=message):
                with self.assertRaises(SystemExit) as caught:
                    self.run_main(*args)
                self.assertEqual(caught.exception.code, message)


if __name__ == "__main__":
    unittest.main()
