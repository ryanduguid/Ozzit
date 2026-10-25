"""Independent arithmetic and formula pins for the ATO benchmark and rate helpers.

The arithmetic lives in Excel LAMBDAs, so it cannot run here. FormulaPinTests
holds each published procedure line, and the Python mirrors below restate the
ATO rules from the ATO's "How we calculate benchmark ratios" page (last updated
16 March 2026, read 19 October 2026). tools/tests/test_cross_checks.py compares
those mirrors with values a native Excel run returned.
"""

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def ato_turnover(other_sales, total_business_income):
    """Other sales, unless nil, blank or under half of total business income."""
    if not other_sales or other_sales < total_business_income / 2:
        return total_business_income
    return other_sales


def ato_labour(salary_wages, contractors, turnover, associates=0, w1=0):
    """Wages (the larger of the return's and W1) plus contractors less associates."""
    return (max(salary_wages, w1) + contractors - associates) / turnover


def rate_at_date(effective_dates, rates, date):
    """The last rate whose effective date is on or before the date; None before the first."""
    in_effect = [rate for start, rate in zip(effective_dates, rates) if start <= date]
    return in_effect[-1] if in_effect else None


class MirrorTests(unittest.TestCase):
    """The mirrors against amounts worked by hand in the comments beside them."""

    def test_turnover_keeps_other_sales_at_half_or_more(self):
        self.assertEqual(ato_turnover(480_000, 500_000), 480_000)
        self.assertEqual(ato_turnover(250_000, 500_000), 250_000)  # exactly half

    def test_turnover_falls_back_below_half_or_when_nil(self):
        self.assertEqual(ato_turnover(249_999, 500_000), 500_000)
        self.assertEqual(ato_turnover(0, 500_000), 500_000)
        self.assertEqual(ato_turnover(None, 500_000), 500_000)

    def test_labour_takes_the_larger_wage_figure(self):
        # (95,000 + 20,000 - 10,000) / 480,000 = 105,000 / 480,000
        self.assertAlmostEqual(ato_labour(90_000, 20_000, 480_000, 10_000, 95_000), 0.21875)
        # W1 below the return's wages: (90,000 + 20,000 - 10,000) / 480,000
        self.assertAlmostEqual(ato_labour(90_000, 20_000, 480_000, 10_000, 85_000), 100 / 480)

    def test_rate_at_date_applies_each_rate_until_the_next(self):
        starts, rates = [1, 10, 20], [0.04, 0.045, 0.05]
        self.assertIsNone(rate_at_date(starts, rates, 0))
        self.assertEqual(rate_at_date(starts, rates, 10), 0.045)
        self.assertEqual(rate_at_date(starts, rates, 19), 0.045)
        self.assertEqual(rate_at_date(starts, rates, 99), 0.05)


def normalised(text):
    return re.sub(r"\s+", " ", text)


class FormulaPinTests(unittest.TestCase):
    """Each LAMBDA still spells the rule its mirror restates; the pins fail on a change."""

    @classmethod
    def setUpClass(cls):
        table = ROOT / "tools/ozzit/src/data/postbuild/ato_benchmarks_rate_lookup.json"
        cls.functions = json.loads(table.read_text(encoding="utf-8"))["functions"]

    PINS = {
        "ATOTurnoverλ": [
            'Sales, IF(OtherSales = "", 0, IF(ISNUMBER(OtherSales), OtherSales, #VALUE!)),',
            "Result, IF((Sales = 0) + (Sales < Income / 2), Income, Sales),",
        ],
        "ATOBenchmarkRatioλ": ["Result, Amount / Turnover,"],
        "ATOTotalExpensesRatioλ": ["Result, (TotalExpenses - Associates) / Turnover,"],
        "ATOCostOfSalesRatioλ": ["Result, (CostOfSales - Wages) / Turnover,"],
        "ATOLabourRatioλ": [
            "Wages, IF(Activity > SalaryWages, Activity, SalaryWages),",
            "Result, (Wages + Contractors - Associates) / Turnover,",
        ],
        "RateAtDateλ": [
            "Complete?, AND(Count = ROWS(Values), ISNUMBER(EffectiveDates), ISNUMBER(Rates)),",
            "Steps, IF(Count = 1, 1, DROP(Starts, 1) - DROP(Starts, -1)),",
            "Result, IF(ISNUMBER(Dates), XLOOKUP(Dates, Starts, Values, NA(), -1, 2), #VALUE!),",
            "Return, IF(Help?, 2, IF(NOT(Complete?), 3, IF(Unsorted?, 4, IF(Repeated?, 5, 1)))),",
        ],
    }

    def test_every_function_is_pinned(self):
        self.assertEqual(sorted(self.PINS), sorted(name for _m, name, _a, _s in self.functions))

    def test_each_published_procedure_is_the_one_the_mirror_restates(self):
        blocks = {name: normalised(block) for _m, name, _a, block in self.functions}
        for name, lines in self.PINS.items():
            for line in lines:
                with self.subTest(name=name, line=line):
                    self.assertIn(normalised(line), blocks[name])

    def test_the_committed_modules_carry_the_published_blocks(self):
        for module, name, _about, block in self.functions:
            with self.subTest(name=name):
                self.assertIn(block, (ROOT / "src" / f"{module}.txt").read_text(encoding="utf-8"))

    def test_no_rate_table_ships_in_the_help(self):
        # llms.txt says the library carries no rate card: the only rates in the help
        # are the synthetic example's.
        source, = [source for _m, name, _a, source in self.functions if name == "RateAtDateλ"]
        help_rates = re.findall(r"0\.0\d+", source)
        self.assertEqual(sorted(set(help_rates)), ["0.04", "0.045", "0.05"])


if __name__ == "__main__":
    unittest.main()
