"""Recovery-block contracts for the help-link pass.

The pass rewrites two tracked stores that the release gates then
compare. A failure between writes used to leave one store updated and the
rest stale; the gates would then read the stores as out of sync. These tests
fault-inject the second write the way test_rate_date_helpers does and hold
that every store is restored to its pre-run bytes.
"""

import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import help_links  # noqa: E402
from sanitise_workbook import read_text, write_deterministic  # noqa: E402
from workbook import BOOK, read_book, read_parts  # noqa: E402

WORKBOOK = ROOT / "ozzit.xlsx"
SRC = ROOT / "src"


def revert_links(workbook: Path, src_dir: Path) -> None:
    """Put both stores back into the wrong-link state the pass fixes."""
    parts = read_parts(workbook)
    book = read_book(parts)
    for old, new in help_links._forms("workbook"):
        assert book.count(new) == 1
        book = book.replace(new, old)
    parts[BOOK] = book.encode("utf-8")
    write_deterministic(workbook, parts)

    ratios_path = src_dir / "Ratios.txt"
    ratios = read_text(ratios_path)
    for old, new in help_links._forms("src"):
        assert ratios.count(new) == 1
        ratios = ratios.replace(new, old)
    ratios_path.write_bytes(ratios.encode("utf-8"))


class HelpLinkRecoveryTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-help-links-"))
        self.workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, self.workbook)
        self.src = self.directory / "src"
        shutil.copytree(SRC, self.src)
        revert_links(self.workbook, self.src)
        # Rollback must preserve archive bytes, even when they are not canonical.
        with zipfile.ZipFile(self.workbook, "a") as archive:
            archive.comment = b"original archive"
        self.ratios_path = self.src / "Ratios.txt"

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def test_the_pass_applies_both_stores(self):
        tracked_book = read_book(read_parts(WORKBOOK))
        tracked_ratios = read_text(SRC / "Ratios.txt")
        changed = help_links.run(self.workbook, self.src)
        self.assertEqual(changed, ["workbook", "src/Ratios.txt"])
        self.assertEqual(read_book(read_parts(self.workbook)), tracked_book)
        self.assertEqual(read_text(self.ratios_path), tracked_ratios)
        # a second run is a byte-stable no-op
        self.assertEqual(help_links.run(self.workbook, self.src), [])

    def test_a_failed_ratios_restore_still_restores_the_workbook(self) -> None:
        before_workbook = self.workbook.read_bytes()
        before_ratios = self.ratios_path.read_bytes()

        calls = []

        def failing_restore(path, text):
            # Ratios.txt is the only store help_links writes with write_text.
            calls.append(Path(path).name)
            raise PermissionError("synthetic failure on every Ratios write")

        with mock.patch.object(help_links, "write_text", side_effect=failing_restore):
            with self.assertRaises(PermissionError):
                help_links.run(self.workbook, self.src)

        # The workbook restore happens before the source restore, so the
        # workbook is back to its pre-run bytes even though the restore of
        # the source write also failed.
        self.assertEqual(calls, ["Ratios.txt", "Ratios.txt"])
        self.assertEqual(self.workbook.read_bytes(), before_workbook)
        self.assertEqual(self.ratios_path.read_bytes(), before_ratios)

    def test_a_failed_second_write_restores_both_stores(self):
        before_workbook = self.workbook.read_bytes()
        before_ratios = self.ratios_path.read_bytes()

        real_write = help_links.write_text
        calls = []

        def failing_write(path, text):
            calls.append(Path(path).name)
            if Path(path).name == "Ratios.txt" and len(calls) == 1:
                raise PermissionError("synthetic failure on the Ratios write")
            return real_write(path, text)

        with mock.patch.object(help_links, "write_text", side_effect=failing_write):
            with self.assertRaises(PermissionError):
                help_links.run(self.workbook, self.src)

        # the first Ratios write fails, then the restore writes it back
        self.assertEqual(calls, ["Ratios.txt", "Ratios.txt"])
        self.assertEqual(self.workbook.read_bytes(), before_workbook)
        self.assertEqual(self.ratios_path.read_bytes(), before_ratios)


if __name__ == "__main__":
    unittest.main()
