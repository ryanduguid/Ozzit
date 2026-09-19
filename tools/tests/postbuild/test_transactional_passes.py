"""Recovery-block contracts for the help-link and revision-strip passes.

Both passes rewrite two or more tracked stores that the release gates then
compare. A failure between writes used to leave one store updated and the
rest stale; the gates would then read the stores as out of sync. These tests
fault-inject the second write the way test_rate_date_helpers does and hold
that every store is restored to its pre-run bytes.
"""

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

import help_links  # noqa: E402
import strip_revision_history as strip  # noqa: E402
from sanitise_workbook import read_text, write_deterministic  # noqa: E402
from workbook import BOOK, read_book, read_parts  # noqa: E402

WORKBOOK = ROOT / "ozzit.xlsx"
SRC = ROOT / "src"
MODULES = ("Dates", "Essentials", "Financial", "Ratios", "Utilities", "Debt")

BLOCK = "/*\r\nREVISIONS: synthetic test block\r\n*/\r\n"


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

        real_write = help_links.write_text
        calls = []

        def failing_restore(path, text):
            calls.append(Path(path).name)
            if Path(path).name == "Ratios.txt":
                raise PermissionError("synthetic failure on every Ratios write")
            return real_write(path, text)

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


class StripRecoveryTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-strip-"))
        self.workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, self.workbook)
        self.src = self.directory / "src"
        shutil.copytree(SRC, self.src)
        for module in MODULES:
            path = self.src / f"{module}.txt"
            path.write_bytes(path.read_bytes() + BLOCK.encode("utf-8"))
        self.expected = {"Dates": 1, "Essentials": 1, "Financial": 1,
                         "Ratios": 1, "Utilities": 1, "Debt": 1}

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _before(self) -> dict[str, bytes]:
        return {path.name: path.read_bytes() for path in sorted(self.src.glob("*.txt"))}

    def test_the_pass_strips_every_module(self):
        with mock.patch.dict(strip.EXPECTED_BLOCKS, self.expected):
            changes = strip.apply(self.workbook, self.src)
        self.assertEqual(
            changes[:6],
            [f"stripped 1 REVISIONS blocks from {module}.txt" for module in MODULES],
        )
        for module in MODULES:
            self.assertEqual(
                (self.src / f"{module}.txt").read_bytes(),
                (SRC / f"{module}.txt").read_bytes(),
            )

    def test_a_failed_later_write_restores_every_module(self):
        before = self._before()

        real_write = strip.write_text
        calls = []

        def failing_write(path, text):
            calls.append(Path(path).name)
            if len(calls) == 2:
                raise PermissionError("synthetic failure on the second module write")
            return real_write(path, text)
        with mock.patch.dict(strip.EXPECTED_BLOCKS, self.expected):
            with mock.patch.object(strip, "write_text", side_effect=failing_write):
                with self.assertRaises(PermissionError):
                    strip.apply(self.workbook, self.src)

        # 2 writes attempted, then the restore loop writes every module back:
        # the recovery covers the modules written before the failure and the
        # ones it never reached.
        written = [f"{module}.txt" for module in MODULES]
        self.assertEqual(calls, written[:2] + written)
        self.assertEqual(self._before(), before)

    def test_a_failed_sync_restores_the_sources_and_the_workbook(self) -> None:
        before = self._before()
        before_workbook = self.workbook.read_bytes()

        with mock.patch.dict(strip.EXPECTED_BLOCKS, self.expected):
            with mock.patch.object(
                strip, "sync", side_effect=ValueError("synthetic AFE sync failure")
            ):
                with self.assertRaisesRegex(ValueError, "synthetic AFE sync failure"):
                    strip.apply(self.workbook, self.src)

        self.assertEqual(self._before(), before)
        self.assertEqual(self.workbook.read_bytes(), before_workbook)

    def test_validation_failure_leaves_every_module_untouched(self):
        # A later module with an unexpected block count must not leave the
        # earlier modules already stripped.
        self.expected["Essentials"] = 2
        before = self._before()
        with mock.patch.dict(strip.EXPECTED_BLOCKS, self.expected):
            with self.assertRaisesRegex(ValueError, "Essentials.txt: expected 2"):
                strip.apply(self.workbook, self.src)
        self.assertEqual(self._before(), before)


if __name__ == "__main__":
    unittest.main()
