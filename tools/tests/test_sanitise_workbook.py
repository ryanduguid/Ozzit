import os
import shutil
import stat
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import sanitise_workbook  # noqa: E402
import verify_workbook  # noqa: E402

ROOT = TOOLS.parent
WORKBOOK = ROOT / "ozzit.xlsx"


class WorkbookToolTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-sanitise-"))
        self.workbook = self.directory / "dirty.xlsx"
        shutil.copy2(WORKBOOK, self.workbook)

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _parts(self):
        with zipfile.ZipFile(self.workbook) as z:
            return {name: z.read(name) for name in z.namelist()}


    def test_privacy_gate_matches_real_windows_user_path(self):
        self.assertIsNotNone(
            verify_workbook.BANNED.search(r"C:\Users\Example\Documents\Ozzit")
        )

    def test_integrity_gate_rejects_xml_doctype_before_parsing(self):
        verify_workbook.failures.clear()
        verify_workbook.check_xml_part(
            "xl/worksheets/sheet1.xml",
            b'<!DOCTYPE x [<!ENTITY a "boom">]><x>&a;</x>',
        )
        self.assertIn(
            "DOCTYPE declaration in xl/worksheets/sheet1.xml",
            verify_workbook.failures,
        )


    def test_atomic_writer_preserves_original_and_cleans_temp_if_replace_fails(self):
        original = self.workbook.read_bytes()
        original_paths = set(self.directory.iterdir())
        parts = self._parts()
        parts["docProps/core.xml"] += b" "

        writer = sanitise_workbook.write_deterministic
        with (
            mock.patch("sanitise_workbook.os.replace", side_effect=OSError("blocked")),
            self.assertRaisesRegex(OSError, "blocked"),
        ):
            writer(self.workbook, parts)

        self.assertEqual(self.workbook.read_bytes(), original)
        self.assertEqual(set(self.directory.iterdir()), original_paths)

    def test_atomic_writer_preserves_unrelated_temporary_sibling(self):
        sibling = self.workbook.with_name(self.workbook.name + ".tmp")
        sibling.write_bytes(b"unrelated work")
        original_paths = set(self.directory.iterdir())

        sanitise_workbook.replace_atomically(self.workbook, b"replacement")

        self.assertEqual(self.workbook.read_bytes(), b"replacement")
        self.assertTrue(sibling.is_file())
        self.assertEqual(sibling.read_bytes(), b"unrelated work")
        self.assertEqual(set(self.directory.iterdir()), original_paths)

    def test_failed_atomic_writer_preserves_unrelated_temporary_sibling(self):
        sibling = self.workbook.with_name(self.workbook.name + ".tmp")
        sibling.write_bytes(b"unrelated work")
        original = self.workbook.read_bytes()
        original_paths = set(self.directory.iterdir())

        with (
            mock.patch("sanitise_workbook.os.replace", side_effect=OSError("blocked")),
            self.assertRaisesRegex(OSError, "blocked"),
        ):
            sanitise_workbook.replace_atomically(self.workbook, b"replacement")

        self.assertEqual(self.workbook.read_bytes(), original)
        self.assertTrue(sibling.is_file())
        self.assertEqual(sibling.read_bytes(), b"unrelated work")
        self.assertEqual(set(self.directory.iterdir()), original_paths)

    def test_staging_cleans_partial_files_after_write_close_or_interrupt_failure(self):
        create = sanitise_workbook.tempfile.NamedTemporaryFile
        original = self.workbook.read_bytes()
        original_paths = set(self.directory.iterdir())
        cases = (("write", OSError), ("close", OSError), ("write", KeyboardInterrupt))
        for phase, error in cases:
            with self.subTest(phase=phase, error=error.__name__):
                def fail_stage(*args, **kwargs):
                    handle = create(*args, **kwargs)
                    wrapped = mock.MagicMock()
                    wrapped.name = handle.name
                    wrapped.__enter__.return_value = wrapped

                    def write(data):
                        if phase == "write":
                            handle.write(data[:2])
                            raise error("injected staging failure")
                        return handle.write(data)

                    def close(*exc):
                        handle.__exit__(*exc)
                        if phase == "close":
                            raise error("injected staging failure")

                    wrapped.write.side_effect = write
                    wrapped.__exit__.side_effect = close
                    return wrapped

                with (
                    mock.patch.object(sanitise_workbook.tempfile, "NamedTemporaryFile", fail_stage),
                    self.assertRaisesRegex(error, "injected staging failure"),
                ):
                    sanitise_workbook.replace_atomically(self.workbook, b"replacement")

                self.assertEqual(self.workbook.read_bytes(), original)
                self.assertEqual(set(self.directory.iterdir()), original_paths)

    def test_overlapping_stages_have_distinct_closed_files(self):
        original = self.workbook.read_bytes()
        original_paths = set(self.directory.iterdir())
        first = sanitise_workbook.stage_bytes(self.workbook, b"first")
        try:
            second = sanitise_workbook.stage_bytes(self.workbook, b"second")
            try:
                self.assertNotEqual(first, second)
                self.assertEqual(first.parent, self.workbook.parent)
                self.assertEqual(second.parent, self.workbook.parent)
                self.assertEqual(self.workbook.read_bytes(), original)
                self.assertEqual(first.read_bytes(), b"first")
                self.assertEqual(second.read_bytes(), b"second")
                os.replace(first, self.workbook)
                self.assertEqual(self.workbook.read_bytes(), b"first")
                self.assertEqual(second.read_bytes(), b"second")
                os.replace(second, self.workbook)
                self.assertEqual(self.workbook.read_bytes(), b"second")
            finally:
                second.unlink(missing_ok=True)
        finally:
            first.unlink(missing_ok=True)
        self.assertEqual(set(self.directory.iterdir()), original_paths)

    def test_atomic_text_writer_preserves_utf8_and_line_endings(self):
        target = self.directory / "text.txt"
        text = "one\r\ntwo\nλ\rthree"
        sanitise_workbook.write_text(target, text)
        self.assertEqual(target.read_bytes(), text.encode("utf-8"))

    @unittest.skipIf(os.name == "nt", "POSIX permission bits do not apply on Windows")
    def test_atomic_writer_preserves_existing_posix_permissions(self):
        for mode in (0o640, 0o664):
            with self.subTest(mode=oct(mode)):
                self.workbook.chmod(mode)
                sanitise_workbook.replace_atomically(self.workbook, b"replacement")
                self.assertEqual(stat.S_IMODE(self.workbook.stat().st_mode), mode)

    def test_deterministic_writer_matches_explicit_level_9_reference(self):
        parts = self._parts()
        actual = self.directory / "actual.xlsx"
        expected = self.directory / "expected.xlsx"

        sanitise_workbook.write_deterministic(actual, parts)
        with zipfile.ZipFile(
            expected, "w", zipfile.ZIP_DEFLATED, compresslevel=9
        ) as archive:
            for name in sorted(parts):
                info = zipfile.ZipInfo(name, date_time=sanitise_workbook.FIXED_DATE)
                info.compress_type = zipfile.ZIP_DEFLATED
                info.create_system = 3
                info.external_attr = 0o644 << 16
                archive.writestr(info, parts[name], compresslevel=9)

        self.assertEqual(actual.read_bytes(), expected.read_bytes())

    def test_deterministic_writer_uses_portable_file_metadata(self):
        parts = self._parts()
        output = self.directory / "metadata.xlsx"
        sanitise_workbook.write_deterministic(output, parts)
        with zipfile.ZipFile(output) as archive:
            self.assertTrue(all(info.create_system == 3 for info in archive.infolist()))
            self.assertTrue(
                all((info.external_attr >> 16) == 0o644 for info in archive.infolist())
            )


if __name__ == "__main__":
    unittest.main()
