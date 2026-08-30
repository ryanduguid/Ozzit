"""Contract for tools/postbuild/help_corrections.py.

The pass repairs the functions that shipped disagreeing with their own inline
help: in the defined names, in src/, in the two cells caching a corrected example
and in the five shared strings that are the whole content of a static literal
cell. On the current workbook it must be a byte no-op; on a workbook reverted to
any one of the pre-correction texts it must apply that swap exactly once; and a
store whose anchors do not match must fail rather than write a partial result.
"""

import importlib.util
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
WORKBOOK = ROOT / "ozzit.xlsx"
PASS_SCRIPT = TOOLS / "postbuild" / "help_corrections.py"
POSTBUILD_README = TOOLS / "postbuild" / "README.md"

_spec = importlib.util.spec_from_file_location("help_corrections", PASS_SCRIPT)
_mod = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_mod)
SWAPS = _mod.SWAPS
CELL_SWAPS = _mod.CELL_SWAPS
STRING_SWAPS = _mod.STRING_SWAPS


class HelpCorrectionsTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-help-corrections-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def _run(self, workbook, src_dir):
        return subprocess.run(
            [sys.executable, str(PASS_SCRIPT), str(workbook), str(src_dir)],
            capture_output=True,
            text=True,
            check=False,
        )

    def _copy_tree(self):
        workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, workbook)
        src = self.directory / "src"
        shutil.copytree(ROOT / "src", src)
        return workbook, src

    def _rewrite_workbook(self, workbook, parts):
        with zipfile.ZipFile(workbook, "w", zipfile.ZIP_DEFLATED) as archive:
            for name, data in parts.items():
                archive.writestr(name, data)

    def _revert(self, workbook, src, swap):
        """Put both stores back into the state this swap was written to repair."""
        with zipfile.ZipFile(workbook) as archive:
            parts = {n: archive.read(n) for n in archive.namelist()}
        book = parts["xl/workbook.xml"].decode("utf-8")
        old, new = swap.workbook
        self.assertEqual(book.count(new), swap.hits, "precondition: corrected workbook")
        parts["xl/workbook.xml"] = book.replace(new, old).encode("utf-8")
        self._rewrite_workbook(workbook, parts)

        old, new = swap.src
        reverted = 0
        for path in src.glob("*.txt"):
            text = path.read_text(encoding="utf-8")
            reverted += text.count(new)
            path.write_text(text.replace(new, old), encoding="utf-8")
        self.assertEqual(reverted, swap.hits, "precondition: corrected src/")

    def test_every_anchor_can_tell_the_two_states_apart(self):
        # A pass whose replacement contains its own anchor cannot report "already
        # applied", and one whose anchor contains its replacement applies twice.
        for swap in SWAPS:
            for store, (old, new) in (("workbook", swap.workbook), ("src", swap.src)):
                with self.subTest(what=swap.what, store=store):
                    self.assertNotIn(old, new)
                    self.assertNotIn(new, old)
        for what, old, new in CELL_SWAPS + STRING_SWAPS:
            with self.subTest(what=what, store="cells"):
                self.assertNotIn(old, new)
                self.assertNotIn(new, old)

    def test_pass_corrects_the_static_literal_cells_no_formula_refreshes(self):
        # These are whole shared strings, not spilled values: reverting one puts
        # the workbook back into the state a reader of the file actually saw.
        workbook, src = self._copy_tree()
        with zipfile.ZipFile(workbook) as archive:
            parts = {n: archive.read(n) for n in archive.namelist()}
        strings = parts["xl/sharedStrings.xml"].decode("utf-8")
        for what, old, new in STRING_SWAPS:
            self.assertEqual(strings.count(new), 1, f"precondition: {what} is present once")
            self.assertEqual(strings.count(old), 0, f"precondition: {what} is corrected")
            strings = strings.replace(new, old)
        parts["xl/sharedStrings.xml"] = strings.encode("utf-8")
        self._rewrite_workbook(workbook, parts)

        result = self._run(workbook, src)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        with zipfile.ZipFile(workbook) as archive:
            corrected = archive.read("xl/sharedStrings.xml").decode("utf-8")
        for what, old, new in STRING_SWAPS:
            with self.subTest(what=what):
                self.assertEqual(corrected.count(new), 1)
                self.assertNotIn(old, corrected)

    def test_pass_fails_when_a_static_literal_holds_neither_state(self):
        workbook, src = self._copy_tree()
        with zipfile.ZipFile(workbook) as archive:
            parts = {n: archive.read(n) for n in archive.namelist()}
        strings = parts["xl/sharedStrings.xml"].decode("utf-8")
        _what, _old, new = STRING_SWAPS[0]
        self.assertIn(new, strings, "precondition: the corrected string is present")
        parts["xl/sharedStrings.xml"] = strings.replace(
            new, "<si><t>Removedλ</t></si>"
        ).encode("utf-8")
        self._rewrite_workbook(workbook, parts)

        result = self._run(workbook, src)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("sharedStrings", result.stdout + result.stderr)

    def test_pass_is_byte_noop_on_current_workbook(self):
        workbook, src = self._copy_tree()
        before = workbook.read_bytes()
        src_before = {p.name: p.read_bytes() for p in src.glob("*.txt")}
        result = self._run(workbook, src)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("already", result.stdout.lower())
        self.assertEqual(workbook.read_bytes(), before)
        self.assertEqual({p.name: p.read_bytes() for p in src.glob("*.txt")}, src_before)

    def test_each_swap_is_reapplied_to_a_reverted_pair_of_stores(self):
        for swap in SWAPS:
            with self.subTest(what=swap.what):
                workbook, src = self._copy_tree()
                try:
                    self._revert(workbook, src, swap)
                    result = self._run(workbook, src)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

                    with zipfile.ZipFile(workbook) as archive:
                        book = archive.read("xl/workbook.xml").decode("utf-8")
                    old, new = swap.workbook
                    self.assertEqual(book.count(new), swap.hits)
                    self.assertNotIn(old, book)

                    old, new = swap.src
                    text = "\n".join(
                        path.read_text(encoding="utf-8") for path in sorted(src.glob("*.txt"))
                    )
                    self.assertEqual(text.count(new), swap.hits)
                    self.assertNotIn(old, text)
                finally:
                    shutil.rmtree(self.directory, ignore_errors=True)
                    self.directory.mkdir()

    def test_pass_refreshes_the_cached_spill_on_the_demonstration_sheets(self):
        workbook, src = self._copy_tree()
        with zipfile.ZipFile(workbook) as archive:
            parts = {n: archive.read(n) for n in archive.namelist()}
        for _what, old, new in CELL_SWAPS:
            hits = 0
            for name, data in list(parts.items()):
                if not name.startswith("xl/worksheets/"):
                    continue
                text = data.decode("utf-8")
                hits += text.count(new)
                parts[name] = text.replace(new, old).encode("utf-8")
            self.assertEqual(hits, 1, f"precondition: {_what} is cached once")
        self._rewrite_workbook(workbook, parts)

        result = self._run(workbook, src)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        with zipfile.ZipFile(workbook) as archive:
            sheets = "".join(
                archive.read(name).decode("utf-8")
                for name in archive.namelist()
                if name.startswith("xl/worksheets/")
            )
        for _what, old, new in CELL_SWAPS:
            self.assertIn(new, sheets)
            self.assertNotIn(old, sheets)

    def test_pass_fails_loudly_when_an_anchor_is_seen_twice(self):
        workbook, src = self._copy_tree()
        target = src / "Financial.txt"
        text = target.read_text(encoding="utf-8")
        example = '"0,-100,-110    →=oz.Reversalλ( , {100,110,130})"'
        self.assertIn(example, text, "precondition: the corrected example is in src/")
        target.write_text(text.replace(example, example + " & " + example), encoding="utf-8")

        result = self._run(workbook, src)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("FAIL", result.stdout + result.stderr)
        self.assertIn("src", result.stdout + result.stderr)

    def test_pass_fails_when_a_store_holds_neither_state(self):
        workbook, src = self._copy_tree()
        target = src / "Ratios.txt"
        text = target.read_text(encoding="utf-8")
        row = '"DSIλ                       →Days Sales in Inventory Ratio'
        self.assertIn(row, text, "precondition: the corrected row is in src/")
        target.write_text(text.replace(row, '"Removedλ                   →Days'), encoding="utf-8")

        result = self._run(workbook, src)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("FAIL", result.stdout + result.stderr)

    def test_documented_run_order_syncs_afe_after_this_pass(self):
        instructions = POSTBUILD_README.read_text(encoding="utf-8")
        command = "python tools/postbuild/help_corrections.py ozzit.xlsx src"
        self.assertIn(command, instructions, "the pass is missing from the run order")
        corrections = instructions.index(command)
        afe = instructions.index("python tools/sync_afe_store.py ozzit.xlsx src")
        sanitise = instructions.index("python tools/sanitise_workbook.py ozzit.xlsx")
        self.assertLess(corrections, afe)
        self.assertLess(afe, sanitise)


if __name__ == "__main__":
    unittest.main()
