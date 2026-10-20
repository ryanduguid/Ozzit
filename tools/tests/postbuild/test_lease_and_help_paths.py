"""Refusal and apply paths of the lease, cover-label and help-text postbuild passes.

The contract tests run these passes as scripts on the committed stores, where they
are no-ops. These tests call them in process: the AASB 16 pass is applied to a copy
reverted to the pre-lease state, and each guard is fed the input it refuses.
"""

import contextlib
import html
import io
import re
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock
from xml.sax.saxutils import escape as xml_escape

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import aasb16_leases  # noqa: E402
import fy27_help_text  # noqa: E402
import gst_help_text  # noqa: E402
from sanitise_workbook import read_text, write_deterministic  # noqa: E402
from verify_sources import NAME, canonical, statements  # noqa: E402
from workbook import BOOK, read_book, read_parts  # noqa: E402

WORKBOOK = ROOT / "ozzit.xlsx"
SRC = ROOT / "src"
INDEX = ROOT / "functions.csv"


def write_zip(path: Path, parts: dict[str, str]) -> None:
    with zipfile.ZipFile(path, "w") as archive:
        for name, text in parts.items():
            archive.writestr(name, text)


def run_main(module, argv):
    """Run module.main() with argv; return (exit code or None, stdout)."""
    out = io.StringIO()
    with mock.patch.object(sys, "argv", argv), contextlib.redirect_stdout(out):
        try:
            module.main()
        except SystemExit as exc:
            return exc.code, out.getvalue()
    return None, out.getvalue()


def shipped_library() -> set[str]:
    library = set()
    for path in sorted(SRC.glob("*.txt")):
        for statement in statements(read_text(path)):
            match = NAME.match(statement)
            # Every statement in the committed src/ modules is a named definition.
            if match:  # pragma: no branch
                library.add(match.group(1))
    return library


def revert_leases(workbook: Path, src_dir: Path, index: Path) -> None:
    """Take the four lease functions back out of every store the pass writes."""
    path = src_dir / "Financial.txt"
    text = read_text(path)
    for _name, _comment, block, _about in aasb16_leases.FUNCTIONS:
        assert text.count("\n\n\n" + block) == 1
        text = text.replace("\n\n\n" + block, "")
    anchor = aasb16_leases.ABOUT_ANCHOR
    line_start = text.rfind("\n", 0, text.index(anchor)) + 1
    indent = text[line_start : text.index(anchor)]
    about = anchor + " &\n" + aasb16_leases.about_block(indent)
    assert text.count(about) == 1
    path.write_bytes(text.replace(about, anchor + " &").encode("utf-8"))

    parts = read_parts(workbook)
    book = read_book(parts)
    stored_anchor = xml_escape(anchor)
    addition = f' &amp; "{xml_escape(aasb16_leases.ABOUT_HEADING)}¶"' + "".join(
        f' &amp; "{xml_escape(f"{name:<19}→{about}")}¶"'
        for name, _comment, _block, about in aasb16_leases.FUNCTIONS
    )
    assert book.count(stored_anchor + addition) == 1
    book = book.replace(stored_anchor + addition, stored_anchor)
    for name in aasb16_leases.QUALIFIED:
        book, count = re.subn(
            rf'<definedName name="{re.escape(name)}"[^>]*>.*?</definedName>', "", book,
            flags=re.DOTALL,
        )
        assert count == 1
    parts[BOOK] = book.encode("utf-8")
    write_deterministic(workbook, parts)

    lines = index.read_bytes().decode("utf-8").split("\n")
    kept = [line for line in lines if line.split(",", 1)[0] not in aasb16_leases.QUALIFIED]
    assert len(lines) - len(kept) == len(aasb16_leases.QUALIFIED)
    index.write_bytes("\n".join(kept).encode("utf-8"))


class StoreCopyMixin:
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-lease-paths-"))

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def copy_stores(self):
        workbook = self.directory / "ozzit.xlsx"
        shutil.copy2(WORKBOOK, workbook)
        src = self.directory / "src"
        shutil.copytree(SRC, src)
        index = self.directory / "functions.csv"
        shutil.copy2(INDEX, index)
        return workbook, src, index


class LeaseApplyTests(StoreCopyMixin, unittest.TestCase):
    def test_a_reverted_copy_gets_all_four_stores_back(self):
        workbook, src, index = self.copy_stores()
        revert_leases(workbook, src, index)
        # A module statement that defines nothing is skipped when the library is read.
        (src / "Notes.txt").write_text("not a definition;\n", encoding="utf-8")
        self.assertEqual(aasb16_leases.src_state(read_text(src / "Financial.txt")), "absent")
        self.assertEqual(aasb16_leases.workbook_state(read_book(read_parts(workbook))), "absent")

        code, out = run_main(
            aasb16_leases, ["aasb16_leases.py", str(workbook), str(src), str(index)]
        )
        self.assertIsNone(code)
        self.assertEqual(
            out.splitlines(),
            [
                "added the AASB 16 lease functions to: src/Financial.txt, workbook, "
                "functions.csv",
                f"OK: {workbook}",
            ],
        )
        self.assertEqual(aasb16_leases.src_state(read_text(src / "Financial.txt")), "applied")
        book = read_book(read_parts(workbook))
        self.assertEqual(aasb16_leases.workbook_state(book), "applied")
        self.assertEqual(index.read_bytes(), INDEX.read_bytes())

        # Every stored definition matches the shipped one, up to the spacing Excel
        # drops on save, and lands in the same place among the defined names.
        shipped = read_book(read_parts(WORKBOOK))
        for name in aasb16_leases.QUALIFIED:
            element = rf'<definedName name="{re.escape(name)}" comment="([^"]*)">(.*?)<'
            ours = re.search(element, book, re.DOTALL)
            theirs = re.search(element, shipped, re.DOTALL)
            self.assertEqual(ours.group(1), theirs.group(1))
            self.assertEqual(
                canonical(html.unescape(ours.group(2))),
                canonical(html.unescape(theirs.group(2))),
            )
        self.assertEqual(
            re.findall(r'<definedName name="([^"]+)"', book),
            re.findall(r'<definedName name="([^"]+)"', shipped),
        )

        # A second run changes nothing, with or without an index to check.
        self.assertEqual(aasb16_leases.run(workbook, src, index), [])
        self.assertEqual(aasb16_leases.run(workbook, src, None), [])
        self.assertEqual(index.read_bytes(), INDEX.read_bytes())

    def test_src_and_workbook_in_different_states_are_refused(self):
        workbook, src, index = self.copy_stores()
        revert_leases(workbook, src, index)
        shutil.copy2(WORKBOOK, workbook)
        before = workbook.read_bytes()
        with self.assertRaisesRegex(
            ValueError,
            "^src/ is absent but the workbook is applied; the two views must not be "
            "applied separately$",
        ):
            aasb16_leases.run(workbook, src, index)
        self.assertEqual(workbook.read_bytes(), before)

    def test_a_missing_financial_module_fails_cleanly(self):
        workbook, src, _index = self.copy_stores()
        (src / "Financial.txt").unlink()
        code, out = run_main(aasb16_leases, ["aasb16_leases.py", str(workbook), str(src)])
        self.assertEqual(code, f"FAIL: src: missing {src / 'Financial.txt'}")
        self.assertEqual(out, "")

    def test_too_many_arguments_print_the_usage(self):
        code, _out = run_main(aasb16_leases, ["aasb16_leases.py", "a", "b", "c", "d"])
        self.assertEqual(
            code,
            "FAIL: usage: python tools/postbuild/aasb16_leases.py "
            "[workbook] [src dir] [functions.csv]",
        )

    def test_a_missing_workbook_is_refused(self):
        missing = self.directory / "absent.xlsx"
        code, _out = run_main(aasb16_leases, ["aasb16_leases.py", str(missing)])
        self.assertEqual(code, f"FAIL: no such workbook: {missing}")


class LeaseGuardTests(StoreCopyMixin, unittest.TestCase):
    def test_a_generated_definition_that_drifts_from_its_source_is_refused(self):
        real_render = aasb16_leases.render
        with mock.patch.object(
            aasb16_leases, "render", side_effect=lambda body, library: real_render(body, library)
            + " + 1"
        ):
            with self.assertRaisesRegex(
                ValueError,
                r"^LeaseLiabilityλ: generated definition does not reproduce its source at "
                r"character \d+\n  src   : .*\n  stored: .*\+1$",
            ):
                aasb16_leases.build_definitions(shipped_library())

    def test_src_state_reads_absent_and_refuses_a_partial_module(self):
        self.assertEqual(aasb16_leases.src_state("Otherλ = LAMBDA(1);"), "absent")
        partial = "\nLeaseLiabilityλ = LAMBDA(1);"
        with self.assertRaisesRegex(
            ValueError,
            r"^src/Financial\.txt holds 1 of 4 lease functions and 0 About heading\(s\); "
            "it is neither state this pass recognises$",
        ):
            aasb16_leases.src_state(partial)

    def test_add_to_src_appends_after_a_final_newline_either_way(self):
        anchor = aasb16_leases.ABOUT_ANCHOR
        blocks = "\n\n\n" + "\n\n\n".join(
            block for _n, _c, block, _a in aasb16_leases.FUNCTIONS
        ) + "\n"
        expected = f"    {anchor} &\n" + aasb16_leases.about_block("    ") + "\n" + blocks
        self.assertEqual(aasb16_leases.add_to_src(f"    {anchor} &"), expected)
        self.assertEqual(aasb16_leases.add_to_src(f"    {anchor} &\n"), expected)
        self.assertEqual(
            aasb16_leases.about_block("  ").splitlines()[:2],
            [
                f'  "{aasb16_leases.ABOUT_HEADING}¶" &',
                '  "LeaseLiabilityλ    →Present value of the lease payments that are not paid '
                'at the commencement date¶" &',
            ],
        )

    def test_add_to_src_refuses_a_module_without_the_about_anchor(self):
        with self.assertRaisesRegex(
            ValueError, "^src/Financial.txt: About anchor found 0 times, expected 1$"
        ):
            aasb16_leases.add_to_src("no anchor here\n")

    def test_add_to_workbook_refuses_a_missing_anchor(self):
        with self.assertRaisesRegex(
            ValueError, "^xl/workbook.xml: About anchor found 2 times, expected 1$"
        ):
            aasb16_leases.add_to_workbook(xml_escape(aasb16_leases.ABOUT_ANCHOR) * 2, {})

    def test_add_to_workbook_refuses_a_name_with_nothing_before_it(self):
        book = xml_escape(aasb16_leases.ABOUT_ANCHOR)
        with self.assertRaisesRegex(
            ValueError,
            "^oz.LeaseLiabilityλ sorts before every shipped name, which is not expected$",
        ):
            aasb16_leases.add_to_workbook(book, {"oz.LeaseLiabilityλ": ("c", "s", "b")})

    def _index(self, text: str) -> Path:
        path = self.directory / "functions.csv"
        path.write_bytes(text.encode("utf-8"))
        return path

    def test_update_index_refuses_unexpected_columns(self):
        path = self._index("function,module\noz.Aλ,Dates\n")
        with self.assertRaisesRegex(
            ValueError, re.escape(f"{path} has unexpected columns: ['function', 'module']")
        ):
            aasb16_leases.update_index(path, {})

    def test_update_index_leaves_a_complete_index_alone(self):
        before = INDEX.read_bytes()
        path = self._index(before.decode("utf-8"))
        definitions = dict.fromkeys(aasb16_leases.QUALIFIED, ("c", "s", "b"))
        self.assertFalse(aasb16_leases.update_index(path, definitions))
        self.assertEqual(path.read_bytes(), before)

    def test_update_index_refuses_a_partial_index(self):
        header = "function,module,previous_name,signature,description\n"
        path = self._index(header + "oz.LeaseLiabilityλ,Financial,,LeaseLiabilityλ(),x\n")
        definitions = dict.fromkeys(aasb16_leases.QUALIFIED, ("c", "s", "b"))
        with self.assertRaisesRegex(
            ValueError, f"^{re.escape(str(path))} holds some of the lease functions but not all$"
        ):
            aasb16_leases.update_index(path, definitions)

    def test_update_index_refuses_a_definition_without_a_function_line(self):
        path = self._index("function,module,previous_name,signature,description\n")
        with self.assertRaisesRegex(
            ValueError, "^oz.Xλ: stored definition has no FUNCTION line$"
        ):
            aasb16_leases.update_index(path, {"oz.Xλ": ("comment", "no help", "body")})


class HelpTextPathTests(unittest.TestCase):
    """Both help-text passes share one shape: modules in src/, swaps in workbook.xml."""

    PASSES = (
        (gst_help_text, "gst_help_text.py"),
        (fy27_help_text, "fy27_help_text.py"),
    )

    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-help-paths-"))
        self.workbook = self.directory / "book.xlsx"
        self.src = self.directory / "src"
        self.src.mkdir()

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def test_missing_src_modules_are_each_reported(self):
        write_zip(self.workbook, {BOOK: "<workbook/>"})
        for module, script in self.PASSES:
            with self.subTest(script=script):
                with self.assertRaises(ValueError) as caught:
                    module.run(self.workbook, self.src)
                failures = str(caught.exception).split("; ")
                self.assertEqual(
                    [f for f in failures if f.startswith("src: missing")],
                    [f"src: missing {self.src / f'{name}.txt'}" for name in module.MODULES],
                )
                code, _out = run_main(module, [script, str(self.workbook), str(self.src)])
                self.assertTrue(code.startswith(f"FAIL: src: missing {self.src}"), code)

    def test_too_many_arguments_print_the_usage(self):
        for module, script in self.PASSES:
            with self.subTest(script=script):
                code, _out = run_main(module, [script, "a", "b", "c"])
                self.assertEqual(
                    code, f"FAIL: usage: python tools/postbuild/{script} [workbook] [src dir]"
                )

    def test_a_missing_workbook_is_refused(self):
        for module, script in self.PASSES:
            with self.subTest(script=script):
                code, _out = run_main(module, [script, str(self.workbook)])
                self.assertEqual(code, f"FAIL: no such workbook: {self.workbook}")


if __name__ == "__main__":
    unittest.main()
