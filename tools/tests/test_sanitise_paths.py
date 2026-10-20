"""Staging permission branches use fakes on every platform."""

import shutil
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import sanitise_workbook  # noqa: E402


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


if __name__ == "__main__":
    unittest.main()
