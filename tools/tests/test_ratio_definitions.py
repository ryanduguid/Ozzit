"""docs/ratio-definitions.md, held to the ratio formulas in src/Ratios.txt.

The page states each ratio function's arithmetic. This test reads the `Result`
line of every function in the source module and fails when the page names a
function the module does not define, leaves one out, or shows arithmetic the
source no longer performs. Whitespace is ignored on both sides; nothing else is
normalised, so a changed operand or operator fails.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOCUMENT = ROOT / "docs" / "ratio-definitions.md"
SOURCE = ROOT / "src" / "Ratios.txt"

ROW = re.compile(r"^\| `oz\.(\w+λ)` \| `([^`]+)` \|", re.MULTILINE)
BLOCK = re.compile(r"^/\*\s+FUNCTION NAME:\s*(\S+)", re.MULTILINE)
ASSIGNMENT = re.compile(r"^(\S+λ)\s*=", re.MULTILINE)
RESULT = re.compile(r"^\s*Result,\s*(.+?),\s*$", re.MULTILINE)


def compact(text: str) -> str:
    return "".join(text.split())


def source_formulas() -> dict[str, str]:
    text = SOURCE.read_text(encoding="utf-8")
    starts = [(m.start(), m.group(1)) for m in BLOCK.finditer(text)]
    formulas: dict[str, str] = {}
    for index, (start, name) in enumerate(starts):
        end = starts[index + 1][0] if index + 1 < len(starts) else len(text)
        block = text[start:end]
        # The comment is only a label: the name the module defines is the one
        # its formula is assigned to, so a renamed function cannot hide behind it.
        assigned = ASSIGNMENT.search(block)
        if assigned is None or assigned.group(1) != name:
            raise AssertionError(f"the FUNCTION NAME comment {name} does not head a definition of that name")
        if "//  Procedure" not in block:
            continue
        procedure = block.split("//  Procedure", 1)[1].split("//  Return Result", 1)[0]
        found = RESULT.search(procedure)
        if found:
            formulas[name] = compact(found.group(1))
    return formulas


class RatioDefinitionsTest(unittest.TestCase):
    def test_the_page_lists_every_ratio_function_with_its_source_arithmetic(self) -> None:
        rows = ROW.findall(DOCUMENT.read_text(encoding="utf-8"))
        source = source_formulas()
        # A floor, so a parser that finds nothing cannot pass by comparing two empty sets.
        self.assertEqual(len(source), 42)
        # Counted before the rows become a mapping, so a duplicated row cannot hide.
        self.assertEqual(len(rows), len(source))
        documented = {name: compact(formula) for name, formula in rows}
        self.assertEqual(sorted(documented), sorted(source))
        for name, formula in source.items():
            with self.subTest(function=name):
                self.assertEqual(documented[name], formula)

    def test_the_page_states_the_count_it_documents(self) -> None:
        text = DOCUMENT.read_text(encoding="utf-8")
        self.assertIn("Each of the 42 ratio functions", " ".join(text.split()))
        self.assertIn("### The 42 functions", text)


if __name__ == "__main__":
    unittest.main()
