# Index factor and bullet loan

`oz.IndexFactorλ` and `oz.BulletLoanλ` are modelling schedules, one period in each column.
Neither makes a tax or accounting classification, and neither rounds its result.

## IndexFactorλ

`oz.IndexFactorλ(Rates, [Periods])` returns the cumulative index factor at the end of each
period: the product of 1 plus each period's rate up to and including that period. Multiply a
base amount by the factor to index it, for example by an inflation or escalation rate.

- `Rates` is one row with a rate for each period, or one rate with `Periods`, a whole number
  from 1 to 1,000.
- Each rate is per period, above -1 and below 2. Convert an annual rate first, for example
  with `oz.PeriodRateλ`.
- `=oz.IndexFactorλ(0.03, 3)` returns 1.03, 1.0609 and 1.092727.

## BulletLoanλ

`oz.BulletLoanλ(Principal, Rates, [CashInterest])` returns 6 rows for a loan repaid in one
amount at the end of the last period: opening balance, interest, cash interest, capitalised
interest, repayment and closing balance.

- Interest is the opening balance times the period's rate.
- `CashInterest` is the share of each period's interest paid in cash, from 0 to 1: one for
  every period, or one for each. Omitted, it is 1, a plain bullet loan. At 0 all interest is
  capitalised (payment in kind) and added to the balance; between them it is split.
- The last period repays the opening balance and that period's capitalised interest.
- With all interest capitalised at a constant rate $r$ over $n$ periods, the repayment is
  $P(1 + r)^n$; the self-test checks this on random inputs.
- `=oz.BulletLoanλ(1000, {0.1,0.1}, 0)` opens at 1,000 and 1,100, capitalises 100 and 110, and
  repays 1,210.
