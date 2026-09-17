"""The arithmetic in docs/depreciation-comparison.md, checked independently.

The figures in that document are recomputed here from the two conventions it
describes, in Decimal, and compared with the numbers written in its tables. The
schedules are not read out of the workbook and no Excel recalculation is
claimed or implied: this proves the document's arithmetic is right, and the
document says so.
"""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from decimal import ROUND_HALF_UP, Decimal
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOCUMENT = ROOT / "docs" / "depreciation-comparison.md"
COST = Decimal("120000.00")
LIFE = 5
CENTS = Decimal("0.01")


def money(value: Decimal) -> Decimal:
    return value.quantize(CENTS, rounding=ROUND_HALF_UP)


def flat(text: str) -> str:
    """Collapse whitespace, so a sentence wrapped across lines still matches."""
    return " ".join(text.split())


def prime_cost(cost: Decimal, life: int) -> list[Decimal]:
    """Straight line over `life` periods, the shape oz.PrimeCostλ produces."""
    return [money(cost / life) for _ in range(life)]


def diminishing_value(cost: Decimal, life: int) -> list[Decimal]:
    """200% of the straight-line rate, residual written off in the last period.

    The final period is the one that matters: the function reconciles its
    schedule to cost, so the closing balance is nil rather than a stub.
    """
    rate = Decimal(2) / Decimal(life)
    remaining = cost
    schedule: list[Decimal] = []
    for period in range(1, life + 1):
        charge = remaining if period == life else money(remaining * rate)
        schedule.append(money(charge))
        remaining -= charge
    return schedule


def unwritten_residual(cost: Decimal, life: int) -> tuple[Decimal, Decimal]:
    """The final-period charge and leftover if the factor kept being applied."""
    rate = Decimal(2) / Decimal(life)
    remaining = cost
    for _ in range(life):
        charge = money(remaining * rate)
        remaining -= charge
    return charge, remaining


def table_figures(heading: str) -> list[Decimal]:
    """Every first money column under a table whose heading contains `heading`."""
    text = DOCUMENT.read_text(encoding="utf-8")
    start = text.index(heading)
    rows = re.findall(
        r"^\| \d+ \| ([\d,]+\.\d{2}) \| ([\d,]+\.\d{2}) \|$",
        text[start:], flags=re.MULTILINE,
    )
    return [Decimal(first.replace(",", "")) for first, _ in rows[:LIFE]]


def table_balances(heading: str) -> list[Decimal]:
    text = DOCUMENT.read_text(encoding="utf-8")
    start = text.index(heading)
    rows = re.findall(
        r"^\| \d+ \| ([\d,]+\.\d{2}) \| ([\d,]+\.\d{2}) \|$",
        text[start:], flags=re.MULTILINE,
    )
    return [Decimal(second.replace(",", "")) for _, second in rows[:LIFE]]


class DepreciationComparisonTests(unittest.TestCase):
    def test_the_document_claims_no_excel_evidence_and_adds_nothing(self) -> None:
        text = DOCUMENT.read_text(encoding="utf-8")
        self.assertIn("NOT_RUN", text)
        self.assertIn("no recalculation is claimed", text)
        self.assertIn(
            "adds a function or introduces an add-in, a macro or a remote formula",
            flat(text),
        )

    def test_the_prime_cost_table_matches_independent_arithmetic(self) -> None:
        expected = prime_cost(COST, LIFE)
        self.assertEqual(expected, [Decimal("24000.00")] * LIFE)
        self.assertEqual(table_figures("### Prime cost"), expected)

    def test_the_prime_cost_balances_run_down_to_nil(self) -> None:
        balances = table_balances("### Prime cost")
        remaining = COST
        for charge, shown in zip(prime_cost(COST, LIFE), balances):
            remaining -= charge
            self.assertEqual(shown, remaining)
        self.assertEqual(balances[-1], Decimal("0.00"))

    def test_the_diminishing_value_table_matches_independent_arithmetic(self) -> None:
        expected = diminishing_value(COST, LIFE)
        self.assertEqual(
            expected,
            [Decimal("48000.00"), Decimal("28800.00"), Decimal("17280.00"),
             Decimal("10368.00"), Decimal("15552.00")],
        )
        self.assertEqual(table_figures("### Diminishing value"), expected)

    def test_the_diminishing_value_schedule_reconciles_to_cost(self) -> None:
        self.assertEqual(sum(diminishing_value(COST, LIFE)), COST)
        self.assertEqual(table_balances("### Diminishing value")[-1], Decimal("0.00"))

    def test_the_documented_alternative_final_period_is_correct(self) -> None:
        """The figures the document uses to name the convention difference."""
        charge, residual = unwritten_residual(COST, LIFE)
        self.assertEqual(charge, Decimal("6220.80"))
        self.assertEqual(residual, Decimal("9331.20"))
        text = DOCUMENT.read_text(encoding="utf-8")
        self.assertIn("6,220.80", text)
        self.assertIn("9,331.20", text)

    def test_the_unwritten_balance_is_not_called_acceptable(self) -> None:
        """A surviving balance is unallocated depreciable amount, not a choice.

        The document used to say neither treatment was wrong. On the stated
        facts, a five-year life with no residual value, AASB 116 leaves the
        9,331.20 nowhere to go, so saying so is the point of the comparison.
        """
        text = DOCUMENT.read_text(encoding="utf-8")
        self.assertNotIn("Neither is wrong", text)
        self.assertIn("no residual value", text)
        self.assertIn("depreciable amount", text)

    def test_a_method_change_is_not_offered_as_a_way_to_clear_the_balance(self) -> None:
        """AASB 116 changes the method only where the consumption pattern changed.

        The document once listed a switch to straight line beside a residual
        value and a longer life as ways to account for the closing balance.
        A method changed to force a balance to nil, with the pattern
        unchanged, is not a permitted change.
        """
        text = DOCUMENT.read_text(encoding="utf-8")
        self.assertNotIn("or a switch to straight line", text)
        self.assertIn("changed significantly", text)
        self.assertIn("change in estimate", text)

    def test_the_documented_movement_closes(self) -> None:
        opening, additions, depreciation = COST, Decimal("0.00"), Decimal("24000.00")
        self.assertEqual(opening + additions - depreciation, Decimal("96000.00"))
        self.assertIn("120,000.00 + 0.00 - 24,000.00 = 96,000.00",
                      DOCUMENT.read_text(encoding="utf-8"))

    def test_the_document_keeps_accounting_and_tax_apart(self) -> None:
        text = flat(DOCUMENT.read_text(encoding="utf-8"))
        self.assertIn("It is not a deduction under ITAA 1997 Division 40", text)
        self.assertIn("an AASB 116 carrying amount is not one either", text)

    def test_the_committed_workbook_is_untouched_by_this_document(self) -> None:
        """This example must not have edited the shipped authority."""
        base = json.loads((ROOT / "release" / "workbook-base.json").read_text(encoding="utf-8"))
        digest = hashlib.sha256((ROOT / base["path"]).read_bytes()).hexdigest()
        self.assertEqual(digest, base["sha256"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
