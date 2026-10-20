"""Cross-check fixtures: synthetic cases with independently computed expected values.

Each case is an Excel formula and the grid it should return, worked out here in Python
from the rule the function implements rather than from the LAMBDA's own steps: present
values from the closed-form annuity factor, loan repayments from the level-payment
formula, the ATO ratios and rate lookup from the mirrors in
tools/tests/postbuild/test_ato_benchmarks_rate_lookup.py, and the variance and
financial-year period functions from their definitions with Python dates.

The lease scenarios follow the patterns of the IFRS 16 illustrative examples the AASB
reproduces (Example 13, initial measurement and a change in lease term at a revised
rate; Example 14, a payment linked to an index remeasured at an unchanged rate). Every
input below is invented for these fixtures; none is taken from those examples.

The loan cases mirror what the Moneysmart mortgage calculator and the AussieCalcs
Division 7A calculator compute, with inputs of our own. Neither site's terms were
found, so nothing is taken from them; their results are not evidence here until a
person enters the same inputs by hand and records the outcome.

tools/tests/fixtures/cross-checks-cases.json is this module's CASES, the input
tools/excel_eval_formulas.ps1 reads. cross-checks-results.json is what that native run
returned against the committed workbook, retained by
tools/excel_eval_formulas_verified.ps1. Regenerate the cases with
`python tools/tests/test_cross_checks.py --write`, then rerun the native evaluation.
"""

import hashlib
import json
import math
import sys
import unittest
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = Path(__file__).resolve().parent / "fixtures"
CASES_PATH = FIXTURES / "cross-checks-cases.json"
RESULTS_PATH = FIXTURES / "cross-checks-results.json"
sys.path.insert(0, str(Path(__file__).resolve().parent / "postbuild"))

from test_ato_benchmarks_rate_lookup import ato_labour, ato_turnover, rate_at_date  # noqa: E402

# Excel error values as excel_eval_formulas.ps1 writes them.
NA = "#ERR:-2146826246"
VALUE = "#ERR:-2146826273"
DIV0 = "#ERR:-2146826281"


def annuity(rate, periods):
    """Present value of 1 a period for this many periods, paid in arrears."""
    return periods if rate == 0 else (1 - (1 + rate) ** -periods) / rate


def level_payment(principal, rate, periods):
    return principal / annuity(rate, periods)


def unwind(opening, payments, rate):
    """Opening, payment, interest and closing rows for payments that follow interest."""
    rows = [[], [], [], []]
    for payment in payments:
        interest = opening * rate
        closing = opening + interest - payment
        for row, value in zip(rows, (opening, payment, interest, closing)):
            row.append(value)
        opening = closing
    return rows


def straight_line(cost, periods):
    charge = cost / periods
    return [
        [cost - charge * k for k in range(periods)],
        [charge] * periods,
        [cost - charge * (k + 1) for k in range(periods)],
    ]


def remeasure(revised_liability, paid_now, carrying_liability, carrying_rou):
    """AASB 16 para 39: the adjustment goes to the asset until it is nil, then to profit."""
    adjustment = revised_liability + paid_now - carrying_liability
    asset = carrying_rou + adjustment
    return [[revised_liability], [adjustment], [max(asset, 0.0)], [min(asset, 0.0)]]


def serial(year, month, day):
    """The Excel date serial: days since 30 December 1899."""
    return (date(year, month, day) - date(1899, 12, 30)).days


def level_row(amount, count):
    return f"SEQUENCE(1,{count},{amount},0)"


def lease_cases():
    cases = []

    # L1: 36 monthly payments of 2,400 in arrears at 0.5% a month.
    rate, pay, n = 0.005, 2400, 36
    opening = pay * annuity(rate, n)
    cases += [
        ("lease_l1_liability", f"=oz.LeaseLiabilityλ({level_row(pay, n)},{rate})", [[opening]]),
        ("lease_l1_schedule", f"=oz.LeaseScheduleλ({level_row(pay, n)},{rate})",
         unwind(opening, [pay] * n, rate)),
    ]

    # L2: 12 quarterly payments of 7,500 in advance at 7% a year effective. The asset
    # adds the commencement payment and 3,000 of initial direct costs and deducts a
    # 1,200 incentive (the Example 13 part 1 pattern), then runs straight-line.
    rate_text, rate = "1.07^(1/4)-1", 1.07 ** 0.25 - 1
    pay, n = 7500, 12
    liability = pay * annuity(rate, n - 1)
    cost = liability + pay + 3000 - 1200
    l2 = f"oz.LeaseLiabilityλ({level_row(pay, n)},{rate_text},TRUE)"
    cases += [
        ("lease_l2_liability_in_advance", f"={l2}", [[liability]]),
        ("lease_l2_schedule_in_advance", f"=oz.LeaseScheduleλ({level_row(pay, n)},{rate_text},TRUE)",
         unwind(liability, [pay] * (n - 1), rate)),
        ("lease_l2_rou", f"=oz.ROUScheduleλ({l2}+{pay}+3000-1200,{n})", straight_line(cost, n)),
    ]

    # L3: 7 annual payments of 18,000 in advance at 4.5%. At the start of year 4, before
    # that year's payment, an index moves from 112 to 118.72, so each remaining payment
    # becomes 19,080; the rate is unchanged (the Example 14 pattern).
    rate, pay, n = 0.045, 18000, 7
    initial = pay * annuity(rate, n - 1)
    cost = initial + pay
    carrying = pay + pay * annuity(rate, 3)          # 4 payments left, one due today
    rou = cost * 4 / 7
    revised_pay = pay * 118.72 / 112
    cases.append((
        "lease_l3_index_remeasure",
        f"=oz.LeaseRemeasureλ({level_row(revised_pay, 4)},{rate},{carrying!r},{rou!r},TRUE)",
        remeasure(revised_pay * annuity(rate, 3), revised_pay, carrying, rou),
    ))
    cases.append((
        "lease_l3_carrying_matches_schedule",
        f"=INDEX(oz.LeaseScheduleλ({level_row(pay, n)},{rate},TRUE),4,3)",
        [[carrying - pay]],
    ))

    # L4: 48 monthly payments of 3,100 in arrears at 0.45% a month. After 24 payments the
    # term extends by 12 months at 3,250 and the rate is revised to 0.52% (the Example 13
    # part 2 pattern); the asset then runs straight-line over the 36 months left.
    rate, pay, n = 0.0045, 3100, 48
    cost = pay * annuity(rate, n)
    carrying = pay * annuity(rate, 24)
    rou = cost * 24 / 48
    new_rate = 0.0052
    revised = pay * annuity(new_rate, 24) + 3250 * annuity(new_rate, 12) / (1 + new_rate) ** 24
    l4 = remeasure(revised, 0, carrying, rou)
    revised_payments = f"HSTACK({level_row(pay, 24)},{level_row(3250, 12)})"
    cases += [
        ("lease_l4_term_remeasure",
         f"=oz.LeaseRemeasureλ({revised_payments},{new_rate},{carrying!r},{rou!r})", l4),
        ("lease_l4_carrying_matches_schedule",
         f"=INDEX(oz.LeaseScheduleλ({level_row(pay, n)},{rate}),4,24)", [[carrying]]),
        ("lease_l4_rou_after",
         f"=oz.ROUScheduleλ({l4[2][0]!r},36)", straight_line(l4[2][0], 36)),
    ]

    # L5: payments fall far enough to exhaust the asset: 3 annual payments of 10,000 at
    # 6% against a carried liability of 50,000 and asset of 4,000.
    revised = 10000 * annuity(0.06, 3)
    cases.append((
        "lease_l5_asset_exhausted",
        f"=oz.LeaseRemeasureλ({level_row(10000, 3)},0.06,50000,4000)",
        remeasure(revised, 0, 50000, 4000),
    ))
    return cases


def loan_cases():
    cases = []
    # Monthly principal and interest loans. The same inputs can be entered in a mortgage calculator by hand.
    for label, principal, apr, months in (("m1", 600000, 0.0624, 360), ("m2", 450000, 0.0589, 300)):
        rate = apr / 12
        payment = level_payment(principal, rate, months)
        loan = f"oz.Amortiseλ({principal},{apr},{months},DATE(2026,11,1))"
        cases += [
            (f"loan_{label}_repayment", f"=INDEX({loan},4,1)", [[-payment]]),
            (f"loan_{label}_total_interest", f"=SUM(CHOOSEROWS({loan},3))", [[payment * months - principal]]),
            (f"loan_{label}_first_interest", f"=INDEX({loan},3,1)", [[principal * rate]]),
            (f"loan_{label}_final_closing", f"=ROUND(INDEX({loan},5,{months}),6)", [[0.0]]),
        ]
    # Annual level repayments, the shape of a Division 7A minimum yearly repayment, at
    # the ATO's 8.77% benchmark for 2026-27. No calculator output is recorded here.
    for label, principal, years in (("d1", 85000, 7), ("d2", 240000, 22)):
        payment = level_payment(principal, 0.0877, years)
        loan = f"oz.AmortiseBλ({principal},0.0877,{years},{years},1,12)"
        cases += [
            (f"loan_{label}_yearly_repayment", f"=INDEX({loan},3,1)", [[payment]]),
            (f"loan_{label}_first_interest", f"=INDEX({loan},2,1)", [[principal * 0.0877]]),
            (f"loan_{label}_first_principal", f"=INDEX({loan},5,1)", [[payment - principal * 0.0877]]),
            (f"loan_{label}_final_closing", f"=ROUND(INDEX({loan},4,{years}),6)", [[0.0]]),
        ]
    return cases


def ato_cases():
    table = "DATE(2026,{1;7;10},1),{0.04;0.045;0.05}"
    starts = [serial(2026, 1, 1), serial(2026, 7, 1), serial(2026, 10, 1)]
    rates = [0.04, 0.045, 0.05]
    return [
        ("ato_turnover_keeps_other_sales", "=oz.ATOTurnoverλ(480000,500000)", [[ato_turnover(480000, 500000)]]),
        ("ato_turnover_below_half", "=oz.ATOTurnoverλ(200000,500000)", [[ato_turnover(200000, 500000)]]),
        ("ato_turnover_exactly_half", "=oz.ATOTurnoverλ(250000,500000)", [[ato_turnover(250000, 500000)]]),
        ("ato_turnover_nil", '=oz.ATOTurnoverλ("",500000)', [[ato_turnover(None, 500000)]]),
        ("ato_turnover_text", '=oz.ATOTurnoverλ("x",500000)', [[VALUE]]),
        ("ato_turnover_rows", "=oz.ATOTurnoverλ({480000,200000},{500000,500000})",
         [[ato_turnover(480000, 500000), ato_turnover(200000, 500000)]]),
        ("ato_rent_ratio", "=oz.ATOBenchmarkRatioλ(24000,480000)", [[24000 / 480000]]),
        ("ato_ratio_zero_turnover", "=oz.ATOBenchmarkRatioλ(24000,0)", [[DIV0]]),
        ("ato_total_expenses", "=oz.ATOTotalExpensesRatioλ(410000,480000,30000)", [[380000 / 480000]]),
        ("ato_total_expenses_no_associates", "=oz.ATOTotalExpensesRatioλ(410000,480000)", [[410000 / 480000]]),
        ("ato_cost_of_sales_wages_in", "=oz.ATOCostOfSalesRatioλ(260000,480000,60000)", [[200000 / 480000]]),
        ("ato_cost_of_sales_wages_out", "=oz.ATOCostOfSalesRatioλ(260000,480000)", [[260000 / 480000]]),
        ("ato_labour_w1_higher", "=oz.ATOLabourRatioλ(90000,20000,480000,10000,95000)",
         [[ato_labour(90000, 20000, 480000, 10000, 95000)]]),
        ("ato_labour_w1_lower", "=oz.ATOLabourRatioλ(90000,20000,480000,10000,85000)",
         [[ato_labour(90000, 20000, 480000, 10000, 85000)]]),
        ("ato_labour_minimal", "=oz.ATOLabourRatioλ(90000,20000,480000)",
         [[ato_labour(90000, 20000, 480000)]]),
        ("rate_at_date_example", f"=oz.RateAtDateλ({table},DATE(2026,8,15))",
         [[rate_at_date(starts, rates, serial(2026, 8, 15))]]),
        ("rate_at_date_on_start", f"=oz.RateAtDateλ({table},DATE(2026,10,1))",
         [[rate_at_date(starts, rates, serial(2026, 10, 1))]]),
        ("rate_at_date_before_first", f"=oz.RateAtDateλ({table},DATE(2025,12,31))", [[NA]]),
        ("rate_at_date_column", f"=oz.RateAtDateλ({table},DATE(2026,{{3;7;12}},1))",
         [[rate_at_date(starts, rates, serial(2026, m, 1))] for m in (3, 7, 12)]),
        ("rate_at_date_text_date", f'=oz.RateAtDateλ({table},"2026-08-01")', [[VALUE]]),
        ("rate_at_date_unsorted", "=oz.RateAtDateλ(DATE(2026,{7;1},1),{0.04;0.045},DATE(2026,8,1))",
         [["RateAtDateλ needs EffectiveDates in ascending order. Sort the table by effective date."]]),
        ("rate_at_date_repeated", "=oz.RateAtDateλ(DATE(2026,{7;7},1),{0.04;0.045},DATE(2026,8,1))",
         [["RateAtDateλ found an effective date that repeats. Keep one rate for each date."]]),
        ("rate_at_date_gap", '=oz.RateAtDateλ(DATE(2026,{1;7},1),{0.04;""},DATE(2026,8,1))',
         [["RateAtDateλ needs EffectiveDates and Rates of the same length with a number in every "
           "entry. A blank or text entry leaves a gap in the table."]]),
    ]


def fy_month(day, start=7):
    """Month of the financial year that starts in month `start`, from 1 to 12."""
    return (day.month - start) % 12 + 1


def year_to_date(rows, dates, as_at, start=7):
    """Each row's total over the dates from the first day of as_at's financial year."""
    first = date(as_at.year - (as_at.month < start), start, 1)
    return [[sum(v for v, d in zip(row, dates) if first <= d <= as_at)] for row in rows]


def period_cases():
    rows = [[10, 20, 30, 40], [1, 2, 3, 4]]
    months = [date(2026, m, 1) for m in (5, 6, 7, 8)]
    dates = "DATE(2026,{5,6,7,8},1)"
    grid = "{10,20,30,40;1,2,3,4}"
    return [
        ("variance_income_above", "=oz.Varianceλ(120,100)", [[(120 - 100) * 1]]),
        ("variance_expense_below", "=oz.Varianceλ(90,100,-1)", [[(90 - 100) * -1]]),
        ("variance_types_by_line", "=oz.Varianceλ({120;90;80},{100;100;100},{1;-1;1})",
         [[(a - b) * t] for a, b, t in ((120, 100, 1), (90, 100, -1), (80, 100, 1))]),
        ("variance_type_refused", "=oz.Varianceλ(120,100,2)",
         [["Varianceλ needs Type to be 1 for income or -1 for an expense in every entry."]]),
        ("variance_pct_income", "=oz.VariancePctλ(110,100)", [[(110 - 100) / abs(100)]]),
        ("variance_pct_negative_budget", "=oz.VariancePctλ(-30,-20)", [[(-30 - -20) / abs(-20)]]),
        ("variance_pct_negative_budget_expense", "=oz.VariancePctλ(-30,-20,-1)",
         [[(-30 - -20) * -1 / abs(-20)]]),
        ("variance_pct_zero_budget", "=oz.VariancePctλ(5,0)", [[DIV0]]),
        ("year_to_date_example", f"=oz.YearToDateλ({grid},{dates},DATE(2026,8,31))",
         year_to_date(rows, months, date(2026, 8, 31))),
        ("year_to_date_june", f"=oz.YearToDateλ({grid},{dates},DATE(2026,6,30))",
         year_to_date(rows, months, date(2026, 6, 30))),
        ("year_to_date_calendar", f"=oz.YearToDateλ({grid},{dates},DATE(2026,7,15),1)",
         year_to_date(rows, months, date(2026, 7, 15), 1)),
        ("year_to_date_shape", f"=oz.YearToDateλ({grid},DATE(2026,{{5,6,7}},1),DATE(2026,8,31))",
         [["YearToDateλ needs Dates to be one row with a date for each column of Values."]]),
        ("financial_month_row", "=oz.FinancialMonthλ(DATE(2026,{1,6,7,12},15))",
         [[fy_month(date(2026, m, 15)) for m in (1, 6, 7, 12)]]),
        ("financial_month_april_start", "=oz.FinancialMonthλ(DATE(2026,{3;4},1),4)",
         [[fy_month(date(2026, m, 1), 4)] for m in (3, 4)]),
        ("financial_quarter_row", "=oz.FinancialQuarterλ(DATE(2026,{7,9,10,1,6},1))",
         [[(fy_month(date(2026, m, 1)) - 1) // 3 + 1 for m in (7, 9, 10, 1, 6)]]),
        ("financial_quarter_start_refused", "=oz.FinancialQuarterλ(DATE(2026,7,1),13)",
         [["FinancialQuarterλ needs StartMonth to be one whole number from 1 to 12."]]),
        ("variance_type_row_refused", "=oz.Varianceλ({120;90},{100;100},{1,-1})",
         [["Varianceλ needs Actual and Budget of the same shape, and Type as one value or one for each row."]]),
        ("variance_type_per_row_across_months", "=oz.Varianceλ({120,130;90,80},{100,100;100,100},{1;-1})",
         [[(a - 100) * t for a in row] for row, t in (((120, 130), 1), ((90, 80), -1))]),
        ("variance_text_amount_refused", '=oz.Varianceλ("120",100)',
         [["Varianceλ needs Actual and Budget to hold numbers or blank cells; text and TRUE or FALSE are refused."]]),
        ("year_to_date_start_range_refused", "=oz.YearToDateλ({10,20},DATE(2026,{2,5},1),DATE(2026,8,31),{1,7})",
         [["YearToDateλ needs StartMonth to be one whole number from 1 to 12."]]),
        ("year_to_date_boolean_refused", "=oz.YearToDateλ({TRUE,20},DATE(2026,{7,8},1),DATE(2026,8,31))",
         [["YearToDateλ needs Values to hold numbers or blank cells; text and TRUE or FALSE are refused."]]),
    ]


CASES = [
    {"id": case_id, "formula": formula, "expected": expected}
    for case_id, formula, expected in lease_cases() + loan_cases() + ato_cases() + period_cases()
]


def cases_json():
    return json.dumps(CASES, ensure_ascii=False, indent=2) + "\n"


def close(got, want):
    if isinstance(want, str) or isinstance(got, str):
        return got == want
    return math.isclose(got, want, rel_tol=1e-9, abs_tol=1e-6)


class CrossCheckTests(unittest.TestCase):
    def test_the_committed_cases_are_this_module_s_cases(self):
        self.assertEqual(CASES_PATH.read_text(encoding="utf-8"), cases_json())

    def test_the_hand_checks_behind_the_references(self):
        # 1 a year for 3 years at 6%: 1/1.06 + 1/1.06^2 + 1/1.06^3 = 2.673012
        self.assertAlmostEqual(annuity(0.06, 3), 2.6730119, places=6)
        self.assertEqual(annuity(0, 4), 4)
        # 100 at 10% over 2 years: 100 / 1.735537 = 57.619048
        self.assertAlmostEqual(level_payment(100, 0.1, 2), 57.6190476, places=6)
        # The unwound liability ends at nil and its interest is payments less the opening.
        rows = unwind(100 * annuity(0.05, 3), [100] * 3, 0.05)
        self.assertAlmostEqual(rows[3][-1], 0, places=9)
        self.assertAlmostEqual(sum(rows[2]), 300 - rows[0][0], places=9)
        self.assertEqual(remeasure(30, 0, 50, 4), [[30], [-20], [0.0], [-16]])
        self.assertEqual(serial(2026, 1, 1), 46023)

    def test_every_case_has_a_distinct_id_and_names_an_indexed_function(self):
        index = (ROOT / "functions.csv").read_text(encoding="utf-8")
        ids = [case["id"] for case in CASES]
        self.assertEqual(len(ids), len(set(ids)))
        for case in CASES:
            name = case["formula"].split("oz.", 1)[1].split("(", 1)[0]
            with self.subTest(case=case["id"]):
                self.assertIn(f"oz.{name},", index)

    def test_excel_returned_every_expected_value_on_the_committed_workbook(self):
        results = json.loads(RESULTS_PATH.read_text(encoding="utf-8-sig"))
        base = json.loads((ROOT / "release" / "workbook-base.json").read_text(encoding="utf-8"))
        self.assertEqual(results["workbook_sha256"], base["sha256"])
        self.assertEqual(
            results["workbook_sha256"], hashlib.sha256((ROOT / "ozzit.xlsx").read_bytes()).hexdigest()
        )
        # Retained by tools/excel_eval_formulas_verified.ps1, which records what it ran.
        for field, path in {
            "cases_sha256": CASES_PATH,
            "helper_sha256": ROOT / "tools" / "excel_eval_formulas.ps1",
            "evidence_helper_sha256": ROOT / "tools" / "excel_eval_formulas_verified.ps1",
        }.items():
            self.assertEqual(results[field], hashlib.sha256(path.read_bytes()).hexdigest(), field)
        self.assertRegex(results["excel"], r"^\d+\.\d+ build \d+$")
        self.assertEqual(sorted(results["results"]), sorted(case["id"] for case in CASES))
        for case in CASES:
            got = results["results"][case["id"]]
            with self.subTest(case=case["id"]):
                self.assertEqual(
                    [len(r) for r in got], [len(r) for r in case["expected"]], "shape differs"
                )
                for got_row, want_row in zip(got, case["expected"]):
                    for value, want in zip(got_row, want_row):
                        self.assertTrue(close(value, want), f"got {value!r}, want {want!r}")


if __name__ == "__main__":
    if "--write" in sys.argv:
        CASES_PATH.write_text(cases_json(), encoding="utf-8", newline="\n")
    else:
        unittest.main()
