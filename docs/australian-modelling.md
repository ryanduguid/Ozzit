## Australian conventions

The library is Australian-only: the foreign tax regimes and references the earlier library carried have been removed rather than relabelled, and the conventions below are Australian throughout.

- Australian English throughout (amortise, modelling, and so on), including function names.
- Dates are day-first, in text examples and in cell formats. Worksheets print on A4.
- Sample data uses AUD.
- Depreciation, GST, PAYG withholding and financial-year helpers:

| Function | Purpose |
|---|---|
| `oz.DiminishingValueλ(Cost, Life)` | Diminishing balance at 200% of the straight-line rate, writing the residual off in the final period |
| `oz.PrimeCostλ(Cost, Life)` | Straight line over `ROUNDUP(Life, 0)` periods, with any part-year remainder in the final period |
| `oz.GSTAddλ(Amounts, [Rate])` | Adds GST to GST-exclusive amounts, 10% by default |
| `oz.GSTExtractλ(Amounts, [Rate])` | Returns the GST inside GST-inclusive amounts |
| `oz.PAYGWithholdingλ(Earnings, Coefficients, [Period])` | Amount to withhold from a payment under the ATO's Schedule 1 formulas (see [PAYG withholding](#payg-withholding)) |
| `oz.FinancialYearλ(Dates, [StartMonth])` | Labels dates with their financial year, starting 1 July |

The 10% default and one-eleventh extraction reflect the basic rule for a taxable supply in [*A New Tax System (Goods and Services Tax) Act 1999*](https://www.legislation.gov.au/C2004A00446/latest/text) ss 9-70 and 9-75 (source checked 20 August 2026). Recheck the current Act and any applicable special rule at the time of use. These helpers apply arithmetic only: they do not decide whether a supply is taxable, GST-free, input taxed, outside the GST system or subject to a special rule.

The same scope note is embedded as NOTES! rows in the `oz.GSTAddλ` and `oz.GSTExtractλ` inline help. The About table and Name Manager comments keep the one-line descriptions. `ozzit gst_help_text` applies it to the committed `ozzit.xlsx` and `src/`; the one-shot v3.0.0 transform predates it and is no longer in the tree.

`oz.Depreciateλ` accepts the method codes `SLN`, `SYD`, `DB`, `DDB`, `VDB`, `DV` (diminishing value) and `PC` (prime cost).

**Modelling parameters, not tax calculations.** The depreciation helpers compute multi-period asset amortisation from cost and effective life inputs and nothing else. `oz.DiminishingValueλ` amortises the remaining balance in the final period so the multi-year schedule reconciles exactly to initial cost (for example, a cost of 1,000 over 5 years produces 400, 240, 144, 86.40 and 129.60). `oz.Depreciateλ` allocates those annual values across a monthly timeline from each in-service date and writes the remaining book value off at disposal. None of that is a tax balancing adjustment or a deduction the ATO would accept, so apportion and adjust for tax in your own model.

## PAYG withholding

`oz.PAYGWithholdingλ(Earnings, Coefficients, [Period])` applies the formulas in the ATO's [Schedule 1 – Statement of formulas for calculating amounts to be withheld](https://www.ato.gov.au/tax-rates-and-codes/payg-withholding-schedule-1-statement-of-formulas-for-calculating-amounts-to-be-withheld) (NAT 1004), the version for payments made from 1 July 2026, published 17 June 2026 and read on 16 November 2026. Period is `"W"` (the default), `"F"`, `"M"` or `"Q"`.

- Weekly earnings are converted as the schedule sets out: fortnightly earnings are halved; monthly earnings gain a cent when they end in 33 cents and are multiplied by 3 and divided by 13; quarterly earnings are divided by 13. Cents are dropped and 99 cents added, giving x.
- The weekly amount is `a * x - b` from the first row of Coefficients whose 'less than' bound is above x, or the last row, rounded to the nearest dollar with 50 cents rounded up and no rounding to the cent first.
- That weekly amount is doubled for a fortnight, multiplied by 13 and divided by 3 and rounded to the nearest dollar for a month, and multiplied by 13 for a quarter.

Coefficients is the table for the payee's scale, copied from the schedule's coefficients page: 3 columns of weekly earnings 'less than', a and b, with the bounds rising and only the last, '& over' row repeating the bound above it, and a row the schedule gives no coefficients for entered as 0 and 0. Each payment must be a number from 0 to under 10,000,000,000 in whole cents: a fraction of a cent, which the schedule does not deal with, returns a message. A payment within a thousandth of a cent of a whole cent, as binary arithmetic leaves a sum of cents, counts as that cent; a payment further off returns a message even when it comes from arithmetic on whole cents, such as a small difference of 2 very large sums, so round such a payment to the cent first. The function does not choose the scale, apply tax offsets, the Medicare levy adjustment or the extra withholding for 53 or 27 pays, and does not do scale 4 (no TFN) or payments to working holiday makers (Schedule 15). The native self-test checks it against every row of the schedule's withholding amounts sample data for scales 1, 2, 3, 5 and 6, and against the schedule's 3 worked examples before their offsets and levy adjustments. Recheck the schedule each year: the ATO reissues it when rates change.

## AASB 16 leases

Four functions cover lessee accounting. `oz.ROUScheduleλ` takes the asset's `Cost` directly: the initial liability plus prepayments net of incentives, initial direct costs and applicable restoration costs under [AASB 16 paragraph 24](https://standards.aasb.gov.au/aasb-16-nov-2022). A later index review feeds the remeasurement.

| Function | Purpose |
|---|---|
| `oz.LeaseLiabilityλ(Payments, Rate, [InAdvance])` | Present value of the lease payments not paid at the commencement date |
| `oz.LeaseScheduleλ(Payments, Rate, [InAdvance])` | Opening, payment, interest and closing rows for each period |
| `oz.ROUScheduleλ(Cost, Periods)` | Opening, depreciation and closing rows for the right-of-use asset |
| `oz.LeaseRemeasureλ(RevisedPayments, Rate, CarryingLiability, CarryingROU, [InAdvance])` | Revised liability, adjustment, revised asset and the remainder taken to profit or loss |

`Rate` is the rate **per period**. Convert an effective annual rate with
`oz.PeriodRateλ(AnnualRate, PeriodsPerYear)`. For example, a 5% effective annual
rate gives `oz.PeriodRateλ(0.05, 12)`, approximately 0.4074% per month. Divide a
nominal annual rate by `PeriodsPerYear` only when its compounding periods match
the payment periods: 5% nominal compounded monthly gives `0.05 / 12`,
approximately 0.4167% per month.

Set `InAdvance` to `TRUE` when the first supplied payment is made at the measurement date. That payment is excluded from the liability and the schedule's payment row; the remaining payments unwind after one period of interest. A schedule with payments in advance therefore has one fewer liability period. Add a payment made at or before commencement to the right-of-use asset cost instead.

The functions take these inputs rather than deciding them:

- **The payments.** Supply all payments in order, including the measurement-date payment first when `InAdvance` is `TRUE`. Include an amount expected to be payable under a residual value guarantee, or the exercise price of a purchase option, in the final period where [AASB 16](https://standards.aasb.gov.au/aasb-16-nov-2022) paragraph 27 brings it into the lease payments. Leave out variable payments that depend on sales or usage.
- **The rate.** Paragraph 26 discounts at the interest rate implicit in the lease where that rate can be readily determined, and at the lessee's incremental borrowing rate where it cannot. The implicit rate equates the present value of lease payments and the unguaranteed residual value with the asset's fair value plus any initial direct costs of the lessor. Include all these components and their dates when using `oz.IRRλ`: the initial outflow is fair value plus the lessor's initial direct costs, and the inflows are the lease payments and unguaranteed residual value. The helper wraps XIRR and returns an annual rate. Convert it to the rate per period before passing it, for example `(1 + annual) ^ (1 / 12) - 1` for monthly payments.
- **The asset's cost.** Paragraph 24 builds it from the initial liability, payments made at or before commencement less incentives received, initial direct costs, and an estimate of dismantling and restoration costs. Add those 4 up and pass the total as `Cost`.

Together the liability and the asset produce a front-loaded expense profile. Interest falls as the liability unwinds while straight-line depreciation does not, so a lease costs more in its first period than its last even though the rent never moves.

Remeasurement follows paragraphs 39 to 43. A change in future payments caused by a change in an index or a rate is remeasured under paragraph 42(b), and only when the cash flows actually change. Paragraph 43 holds the discount rate unchanged for that revision, and uses a revised rate only where the change arises from a floating interest rate. Paragraph 39 takes the adjustment to the right-of-use asset, and where that asset is already nil the remainder goes to profit or loss, which is the fourth row `oz.LeaseRemeasureλ` returns.

Paragraph references were read against the AASB 16 compilation on 24 August 2026. Recheck the current compilation at the time of use.

**These are schedules, not determinations.** They do not decide the lease term, decide what counts as a lease payment, decide whether an arrangement contains a lease, or apply the short-term and low-value recognition exemptions. There is no lessor accounting here. Use them to build and check a model, not to conclude on the standard.

### Lease journals

`oz.LeaseJournalλ(LeaseSchedule, ROUSchedule, [Accounts])` turns the two schedules into journal lines with the period, account, debit, credit and narration. The journal has a header and a debit and credit line for each entry. The paragraph references are to compilation 8, checked on 20 November 2026. It covers annual reporting periods that start from 1 January 2024 to 31 December 2026. [AASB 16 compilation 8](https://standards.aasb.gov.au/aasb-16-nov-2022)

- **Period 0, commencement.** Debit the right-of-use asset and credit the lease liability with the liability's opening balance: paragraph 22 recognises both, paragraph 23 measures the asset at cost and paragraph 26 measures the liability. The rest of the asset's cost, the payments at or before commencement less incentives received and the initial direct costs of paragraph 24(b) and (c), is credited to cash, or debited when an incentive received is larger than those costs. A restoration estimate in the cost, paragraph 24(d), is a provision under AASB 137 (paragraph 25), not cash: journal it from cash to the provision yourself.
- **Each later period.** Debit interest expense and credit the liability with that period's interest (paragraphs 36(a) and 37; paragraph 38(a) takes it to profit or loss), debit the liability and credit cash with the payment (paragraph 36(b)), and debit depreciation expense and credit accumulated depreciation with the right-of-use charge (paragraphs 31 and 32).

Pass `oz.LeaseScheduleλ` and `oz.ROUScheduleλ` for the same lease. For payments in advance the liability schedule is a period shorter than the asset's, so the last period has depreciation only. Amounts are unrounded, as the schedules give them; debits equal credits in every period and the liability closes at nil. A nil amount gives no line and a negative one, such as interest at a negative rate, swaps its debit and credit. `Accounts` takes up to 6 names or codes in the order right-of-use asset, lease liability, interest expense, cash, depreciation expense and accumulated depreciation; a blank keeps its default. Paragraph 49 presents the interest apart from the depreciation, which the separate accounts allow. It does not journal a remeasurement or variable payments left out of the liability: post those yourself.

When an entity applies AASB 18, amended AASB 16 paragraph 49 requires it to classify interest on the lease liability in the financing category of profit or loss. [AASB 18 Appendix D, AASB 16 amendments](https://standards.aasb.gov.au/sites/default/files/2024-06/AASB18_06-24_AppendixD_0.pdf)

## Hire purchase and chattel mortgages

`oz.HPScheduleλ(Financed, AnnualRate, Periods, [PeriodsPerYear], [InAdvance], [Balloon])` returns 5 rows, one column a payment: opening balance, payment, interest, principal and closing balance, in cents. `Financed` and `Balloon` must be whole cents, and `InAdvance` TRUE, FALSE, 1 or 0.

- The rate per period is `oz.PeriodRateλ(AnnualRate, PeriodsPerYear)`, an effective annual rate converted to the payment frequency (52 weekly, 26 fortnightly, 12 monthly by default, 4 quarterly). For a nominal rate that compounds with each payment, pass `EFFECT(Nominal, PeriodsPerYear)`. For the rate a contract's instalment implies, pass `oz.AnnualRateλ(RATE(Periods, -Instalment, Financed, -Balloon, InAdvance), PeriodsPerYear)`.
- The level payment is `oz.LevelPaymentλ` at that rate, the same as `-PMT(rate, Periods, Financed, -Balloon, InAdvance)`, rounded to the cent.
- Interest is rounded to the cent each period: on the opening balance for payments in arrears, and on the balance left after the payment for payments in advance.
- The final payment takes the rounding difference, so the last closing balance is the `Balloon` exactly. The balloon is paid at the end of the last period. In advance, the last period's interest is the balloon's share `Rate / (1 + Rate)` in cents, which can be a cent off interest on the balance left after the last payment.

`oz.HPJournalλ(HPSchedule, [Accounts])` journals that schedule: at period 0 a debit to the asset and a credit to the liability for the amount financed, then each period's interest (debit interest expense, credit the liability) and instalment (debit the liability, credit cash), and the balloon in the last period. Debits equal credits in every period and the liability closes at nil. `Accounts` takes up to 4 names or codes: asset, liability, interest expense and cash. Rename the liability for a chattel mortgage; the arithmetic is the same.

**Bookkeeping only.** Neither function works out the tax treatment: not Division 240 of the *Income Tax Assessment Act 1997* and not the GST input tax credits on the purchase or the finance. A deposit, a trade-in, fees and the asset's depreciation are outside the schedule and the journal.

## Comparing a lease, hire purchase, loan or cash

`oz.FinanceCompareλ(CashFlows, DiscountRate, [PeriodsPerYear], [Labels])` takes one row of cash flows per option and returns a header row and, for each option, its label, the undiscounted total and the present value at `DiscountRate`, an effective annual rate converted with `oz.PeriodRateλ`. Column 1 is at the start and column k + 1 at the end of period k. Enter payments positive and receipts, such as the sale of the asset at the end, negative.

- **Cash purchase:** the price in column 1.
- **Lease:** its payments, in arrears from column 2 or in advance from column 1.
- **Hire purchase or loan:** 0, or the deposit, in column 1, then the payment row of `oz.HPScheduleλ` with the balloon added to the last payment; for payments in advance, the payment row from column 1 and the balloon in the column after it.

The comparison is **pre-tax only**, as its last column heading says. It leaves out income tax deductions, depreciation, GST input tax credits and Division 240, which an after-tax comparison needs, and it ranks and recommends nothing.
