"""Day-count arithmetic for oz.DayCountRateλ, checked without Excel.

tools/postbuild/rate_date_helpers.py carries the published source of the four
rate and date helpers, and test_idempotency.py proves only that re-running the
pass writes nothing. Nothing asserted what the day counts come to, which is how
the Actual/Actual defect that the Unreleased CHANGELOG records reached a release.

The arithmetic itself lives in an Excel LAMBDA, so it cannot be executed here.
This module closes the gap in two halves that have to agree:

1. `fraction()` restates each convention in Python, and the cases below assert it
   against year fractions worked out by hand in the comments beside them.
2. `FormulaPinTests` asserts that the shipped LAMBDA still spells those same four
   rules, comparing whitespace-normalised text against the published source. The
   builder's constants are byte-identical to `src/Financial.txt`, which is what
   `verify_sources.py` proves against the workbook, so pinning the constant pins
   the shipped function.

Change the LAMBDA and the pins fail, which is the prompt to re-derive the mirror
and its hand calculations rather than to edit the pin.

Dates are period boundaries the way the LAMBDA reads them: `start` is the first
day of the period and `following` is the first day of the next one, the LAMBDA's
`Starts` and `Next`. Pure Python: no Excel, no dependencies.
"""

import re
import sys
import unittest
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "postbuild"))

import rate_date_helpers  # noqa: E402

# The Convention argument's numbers, as the shipped PARAMETERS row states them.
THIRTY_360 = 1
ACTUAL_360 = 2
ACTUAL_365 = 3
ACTUAL_ACTUAL = 4


def days_in_year(year: int) -> int:
    """366 in a leap year, 365 otherwise, counted the way the LAMBDA counts it."""
    return (date(year + 1, 1, 1) - date(year, 1, 1)).days


def fraction(start: date, following: date, convention: int) -> float:
    """The year fraction one period contributes, restating the shipped LAMBDA."""
    if convention == THIRTY_360:
        # European rule: a 31st counts as the 30th at either end.
        days = (
            360 * (following.year - start.year)
            + 30 * (following.month - start.month)
            + min(following.day, 30)
            - min(start.day, 30)
        )
        return days / 360
    if convention == ACTUAL_360:
        return (following - start).days / 360
    if convention == ACTUAL_365:
        return (following - start).days / 365
    if convention == ACTUAL_ACTUAL:
        # ISDA: each calendar year is divided by its own length, and every whole
        # year in between contributes exactly one.
        boundary = date(start.year + 1, 1, 1)
        split = min(following, boundary)
        rest = 0.0
        if following.year > start.year:
            last_year_start = date(following.year, 1, 1)
            rest = (following.year - start.year - 1) + (
                (following - last_year_start).days / days_in_year(following.year)
            )
        return (split - start).days / days_in_year(start.year) + rest
    raise ValueError(f"convention {convention} is not one the helper accepts")


class ThirtyThreeSixtyTests(unittest.TestCase):
    def test_a_whole_month_is_thirty_days(self):
        # 1 Jul 2026 to 1 Aug 2026: 360*0 + 30*(8-7) + 1 - 1 = 30 days; 30/360 = 1/12.
        self.assertAlmostEqual(
            fraction(date(2026, 7, 1), date(2026, 8, 1), THIRTY_360), 1 / 12, places=12
        )

    def test_month_end_days_clamp_to_the_thirtieth_at_both_ends(self):
        # 31 Jan 2026 to 31 Mar 2026: 30*(3-1) + min(31,30) - min(31,30) = 60 days.
        # 60/360 = 1/6. The European rule is why February's short month does not show.
        self.assertAlmostEqual(
            fraction(date(2026, 1, 31), date(2026, 3, 31), THIRTY_360), 1 / 6, places=12
        )

    def test_february_in_a_leap_year_is_still_thirty_days(self):
        # 1 Feb 2024 to 1 Mar 2024: 30*(3-2) + 1 - 1 = 30 days, although February
        # actually ran 29 days. 30/360 = 1/12.
        self.assertAlmostEqual(
            fraction(date(2024, 2, 1), date(2024, 3, 1), THIRTY_360), 1 / 12, places=12
        )

    def test_three_whole_years_are_three_hundred_and_sixty_days_each(self):
        # 1 Jul 2023 to 1 Jul 2026: 360*3 + 30*0 + 1 - 1 = 1,080 days; 1080/360 = 3.
        self.assertAlmostEqual(
            fraction(date(2023, 7, 1), date(2026, 7, 1), THIRTY_360), 3.0, places=12
        )


class ActualThreeSixtyTests(unittest.TestCase):
    def test_july_is_thirty_one_actual_days_over_a_three_hundred_and_sixty_day_year(self):
        # 1 Jul 2026 to 1 Aug 2026 is 31 days. 31/360 = 0.086111111...
        self.assertAlmostEqual(
            fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_360), 31 / 360, places=12
        )
        self.assertAlmostEqual(
            fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_360), 0.0861111111, places=9
        )

    def test_a_leap_year_runs_past_one_on_a_three_hundred_and_sixty_day_year(self):
        # 1 Jan 2024 to 1 Jan 2025 is 366 actual days. 366/360 = 1.016666...
        self.assertAlmostEqual(
            fraction(date(2024, 1, 1), date(2025, 1, 1), ACTUAL_360), 366 / 360, places=12
        )


class ActualThreeSixtyFiveTests(unittest.TestCase):
    def test_july_is_thirty_one_actual_days_over_a_three_hundred_and_sixty_five_day_year(self):
        # 1 Jul 2026 to 1 Aug 2026 is 31 days. 31/365 = 0.084931506...
        self.assertAlmostEqual(
            fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_365), 31 / 365, places=12
        )
        self.assertAlmostEqual(
            fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_365), 0.0849315068, places=9
        )

    def test_a_leap_year_exceeds_one_because_the_denominator_is_fixed(self):
        # 1 Jan 2024 to 1 Jan 2025 is 366 days; the denominator stays 365, so the
        # year comes to 366/365 = 1.002739726..., not 1. That is the convention,
        # and it is the difference Actual/Actual removes.
        self.assertAlmostEqual(
            fraction(date(2024, 1, 1), date(2025, 1, 1), ACTUAL_365), 366 / 365, places=12
        )
        self.assertAlmostEqual(
            fraction(date(2024, 1, 1), date(2025, 1, 1), ACTUAL_365), 1.0027397260, places=9
        )

    def test_three_years_including_a_leap_year(self):
        # 1 Jul 2023 to 1 Jul 2026 is 365 + 366 + 365 = 1,096 days (2024 is the leap
        # year). 1096/365 = 3.002739726...
        self.assertEqual((date(2026, 7, 1) - date(2023, 7, 1)).days, 1096)
        self.assertAlmostEqual(
            fraction(date(2023, 7, 1), date(2026, 7, 1), ACTUAL_365), 1096 / 365, places=12
        )


class ActualActualTests(unittest.TestCase):
    def test_a_period_inside_one_calendar_year_uses_that_year_only(self):
        # 1 Jul 2026 to 1 Aug 2026: Next is before 1 Jan 2027, so Split is Next and
        # nothing is added. 31/365 = 0.084931506...
        self.assertAlmostEqual(
            fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_ACTUAL), 31 / 365, places=12
        )

    def test_a_leap_year_is_exactly_one(self):
        # 1 Jan 2024 to 1 Jan 2025: the first year contributes 366/366 = 1, and
        # YearLast (2025) exceeds YearOne (2024) by one, so the whole-year term is
        # 2025 - 2024 - 1 = 0 and the last part is 0/365. Total exactly 1.
        self.assertEqual(fraction(date(2024, 1, 1), date(2025, 1, 1), ACTUAL_ACTUAL), 1.0)

    def test_a_non_leap_year_is_exactly_one(self):
        # 1 Jan 2026 to 1 Jan 2027: 365/365 = 1, with nothing added.
        self.assertEqual(fraction(date(2026, 1, 1), date(2027, 1, 1), ACTUAL_ACTUAL), 1.0)

    def test_a_period_spanning_several_years_counts_each_year_by_its_own_length(self):
        # 1 Jul 2023 to 1 Jul 2026, the defect the CHANGELOG records.
        # First part:  1 Jul 2023 to 1 Jan 2024 is 31+31+30+31+30+31 = 184 days,
        #              over 2023's 365, so 184/365.
        # Whole years: 2026 - 2023 - 1 = 2 (2024 and 2025), one each regardless of
        #              2024 being a leap year.
        # Last part:   1 Jan 2026 to 1 Jul 2026 is 31+28+31+30+31+30 = 181 days,
        #              over 2026's 365, so 181/365.
        # Total:       184/365 + 2 + 181/365 = 2 + 365/365 = 3 exactly.
        self.assertEqual((date(2024, 1, 1) - date(2023, 7, 1)).days, 184)
        self.assertEqual((date(2026, 7, 1) - date(2026, 1, 1)).days, 181)
        self.assertAlmostEqual(
            fraction(date(2023, 7, 1), date(2026, 7, 1), ACTUAL_ACTUAL), 3.0, places=12
        )

    def test_a_period_crossing_into_a_leap_year_splits_at_new_year(self):
        # 1 Dec 2023 to 1 Feb 2024, two months straddling 1 January.
        # First part: 1 Dec 2023 to 1 Jan 2024 is 31 days over 2023's 365 = 31/365.
        # Last part:  1 Jan 2024 to 1 Feb 2024 is 31 days over 2024's 366 = 31/366,
        #             because 2024 is a leap year. No whole years in between.
        # Total:      31/365 + 31/366 = 0.0849315068 + 0.0846994536 = 0.1696309604.
        self.assertEqual(days_in_year(2024), 366)
        self.assertAlmostEqual(
            fraction(date(2023, 12, 1), date(2024, 2, 1), ACTUAL_ACTUAL),
            31 / 365 + 31 / 366,
            places=12,
        )
        self.assertAlmostEqual(
            fraction(date(2023, 12, 1), date(2024, 2, 1), ACTUAL_ACTUAL),
            0.1696309604,
            places=9,
        )

    def test_the_leap_day_itself_is_carried_by_the_leap_year_denominator(self):
        # 1 Feb 2024 to 1 Mar 2024 is 29 days, over 2024's 366: 29/366 = 0.0792349727.
        self.assertEqual((date(2024, 3, 1) - date(2024, 2, 1)).days, 29)
        self.assertAlmostEqual(
            fraction(date(2024, 2, 1), date(2024, 3, 1), ACTUAL_ACTUAL), 29 / 366, places=12
        )


class PeriodRateTests(unittest.TestCase):
    def test_the_period_rate_is_the_apr_times_the_year_fraction(self):
        # The LAMBDA's final step is Rates = Rate * Fraction. At the help's own
        # example, APR 0.073 over 1 Jul 2026 to 1 Aug 2026 on the Actual/365
        # default: 0.073 * 31/365 = 0.0062 exactly, which is what the help prints.
        rate = 0.073 * fraction(date(2026, 7, 1), date(2026, 8, 1), ACTUAL_365)
        self.assertAlmostEqual(rate, 0.0062, places=12)

    def test_an_unrecognised_convention_is_rejected(self):
        with self.assertRaises(ValueError):
            fraction(date(2026, 7, 1), date(2026, 8, 1), 5)


def normalised(text: str) -> str:
    return re.sub(r"\s+", " ", text)


class FormulaPinTests(unittest.TestCase):
    """The shipped LAMBDA must still spell the rules the mirror above restates."""

    def setUp(self):
        self.source = normalised(rate_date_helpers.DAY_COUNT_RATE_SRC)

    def test_the_builder_carries_the_shipped_definition(self):
        financial = (ROOT / "src" / "Financial.txt").read_text(encoding="utf-8")
        self.assertIn(rate_date_helpers.DAY_COUNT_RATE_SRC, financial)

    def test_the_convention_numbers_match_the_constants_used_here(self):
        self.assertIn(
            "(Optional: Default = 3) 1 = 30/360, 2 = Actual/360, 3 = Actual/365, "
            "4 = Actual/Actual.",
            self.source,
        )
        self.assertEqual(
            (THIRTY_360, ACTUAL_360, ACTUAL_365, ACTUAL_ACTUAL), (1, 2, 3, 4)
        )

    def test_the_thirty_three_sixty_day_count_is_the_european_rule(self):
        self.assertIn(
            "Days, IF(Method = 1, 360 * (YEAR(Next) - YEAR(Starts)) "
            "+ 30 * (MONTH(Next) - MONTH(Starts)) "
            "+ IF(DAY(Next) > 30, 30, DAY(Next)) - IF(DAY(Starts) > 30, 30, DAY(Starts)), "
            "Next - Starts)",
            self.source,
        )

    def test_the_actual_actual_terms_split_at_new_year_on_each_year_length(self):
        self.assertIn("Boundary, DATE(YearOne + 1, 1, 1)", self.source)
        self.assertIn("Split, IF(Next < Boundary, Next, Boundary)", self.source)
        self.assertIn("DaysOne, IF(DAY(DATE(YearOne, 2, 29)) = 29, 366, 365)", self.source)
        self.assertIn("DaysLast, DATE(YearLast + 1, 1, 1) - LastYearStart", self.source)

    def test_each_convention_keeps_its_denominator(self):
        self.assertIn(
            "Fraction, SWITCH(Method, 1, Days / 360, 2, Days / 360, "
            "4, (Split - Starts) / DaysOne + IF(YearLast > YearOne, "
            "YearLast - YearOne - 1 + (Next - LastYearStart) / DaysLast, 0), "
            "Days / 365)",
            self.source,
        )
        self.assertIn("Rates, Rate * Fraction", self.source)


if __name__ == "__main__":
    unittest.main()
