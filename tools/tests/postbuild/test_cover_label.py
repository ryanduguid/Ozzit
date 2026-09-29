"""The cover-label pass moves any label it records to the current one, and only that.

Cover!A3 has read "130 functions" (before v3.4.2) and "133 functions" (v3.4.2); the
pass must carry either to the current wording, leave the current one alone, and refuse
a workbook holding none or more than one of them.
"""

import shutil
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import cover_label  # noqa: E402
from sanitise_workbook import write_deterministic  # noqa: E402
from workbook import read_parts  # noqa: E402

STRINGS = cover_label.STRINGS


class CoverLabelTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-cover-"))
        self.workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(ROOT / "ozzit.xlsx", self.workbook)

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _strings(self):
        return read_parts(self.workbook)[STRINGS].decode("utf-8")

    def _set_strings(self, text):
        parts = read_parts(self.workbook)
        parts[STRINGS] = text.encode("utf-8")
        write_deterministic(self.workbook, parts)

    def _swap(self, old, new):
        text = self._strings()
        self.assertEqual(text.count(f"<t>{old}</t>"), 1)
        self._set_strings(text.replace(f"<t>{old}</t>", f"<t>{new}</t>"))

    def test_each_earlier_label_moves_to_the_current_one(self):
        for old in cover_label.OLD:
            with self.subTest(label=old):
                self._swap(cover_label.NEW, old)
                self.assertEqual(cover_label.run(self.workbook), ["workbook"])
                text = self._strings()
                self.assertEqual(text.count(f"<t>{cover_label.NEW}</t>"), 1)
                self.assertEqual(sum(text.count(f"<t>{label}</t>") for label in cover_label.OLD), 0)

    def test_the_current_label_is_left_alone(self):
        before = self.workbook.read_bytes()
        self.assertEqual(cover_label.run(self.workbook), [])
        self.assertEqual(self.workbook.read_bytes(), before)

    def test_two_labels_are_refused(self):
        text = self._strings()
        self._set_strings(text.replace("</sst>", f"<si><t>{cover_label.OLD[1]}</t></si></sst>"))
        with self.assertRaises(ValueError):
            cover_label.run(self.workbook)

    def test_no_recognised_label_is_refused(self):
        self._swap(cover_label.NEW, cover_label.NEW.replace("137", "999"))
        with self.assertRaises(ValueError):
            cover_label.run(self.workbook)


if __name__ == "__main__":
    unittest.main()
