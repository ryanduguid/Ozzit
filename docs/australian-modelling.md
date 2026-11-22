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

## Capital gains

`oz.CGTParcelsλ(Parcels, Sales)` matches sales of one class of shares or units to the parcels bought, first in, first out, and `oz.NetCapitalGainλ(IncomeYear, Gains, [Discountable], [Losses], [PriorLosses], [Entity])` works out the year's net capital gain from the result and any other gains and losses. Both read the [Income Tax Assessment Act 1997](https://www.legislation.gov.au/C2004A05138/2026-05-21/2026-05-21/text/original/pdf/3) as compilation 264 (21 May 2026) gives it, read on 22 November 2026.

- Parcels and Sales are 3 columns each, with the rows of one times the rows of the other at most 1,000,000: the date, the units, in millionths and under 1,000,000,000 in all, and the parcel's cost base or the sale's proceeds less its selling costs, in whole cents under 10,000,000,000. The units are matched in whole millionths, so no sliver of a unit is lost to rounding. Sales take the earliest parcels first, the basis the ATO accepts in [TD 33](https://www.ato.gov.au/law/view/document?DocID=CGD/TD33/NAT/ATO/00001) for identical shares that cannot be told apart. A parcel or sale's share of an amount is rounded to the cent so the shares add up to the whole.
- The 12 months column is TRUE when the parcel was acquired at least 12 months before the sale (section 115-25(1)), counting neither the day of acquisition nor the day of the sale, as the ATO's [CGT discount](https://www.ato.gov.au/individuals-and-families/investments-and-assets/capital-gains-tax/cgt-discount) page sets out. Date a sale under contract by the contract.
- `oz.NetCapitalGainλ` follows section 102-5(1): the year's capital losses, then earlier years' net capital losses, reduce the gains, those not marked Discountable first, as step 5 of the ATO's [How to calculate your CGT](https://www.ato.gov.au/individuals-and-families/investments-and-assets/capital-gains-tax/calculating-your-cgt/how-to-calculate-your-cgt) advises; the Act lets you choose the order. The discount then takes half of each discount capital gain left for an individual or trust, a third for a complying superannuation entity and nothing for a company (sections 115-10 and 115-100). Each amount is in whole cents under 10,000,000,000, and the gains, and the losses, under 10,000,000,000,000 in all; the sums are taken in whole cents. It returns the net capital gain, the discount and the losses to carry forward.

Both stop at 30 June 2027: the [Treasury Laws Amendment (Tax Reform No. 1) Act 2026](https://www.legislation.gov.au/C2026A00049/asmade/text) changes the method from the 2027-28 income year and the discount for CGT events from 1 July 2027, so `oz.CGTParcelsλ` refuses a later sale and `oz.NetCapitalGainλ` takes only 2026 (2025-26) or 2027 (2026-27). `oz.CGTParcelsλ` also refuses a sale taking units from a parcel acquired on or before 21 September 1999, which indexation or the pre-CGT rules can reach, and a sale of more units than are held by its date. Neither function decides whether a gain is a discount capital gain for the entity making it, apportions the discount for foreign or temporary residents, adds the affordable housing discount, applies the small business concessions, keeps collectables' and personal use assets' losses apart, selects parcels, or adjusts a cost base for a return of capital, bonus shares or a rollover; a parcel's cost base leaves out the third element, costs of owning, because a loss uses the same amount. The native self-test checks both against the ATO's Kathleen and Boris examples and hand-worked cases.

## Income tax

`oz.IncomeTaxλ(TaxableIncome, IncomeYear, [Resident])` returns the tax on each taxable income at the general rates of the [Income Tax Rates Act 1986](https://www.legislation.gov.au/C2004A03348/latest/text), Schedule 7, for an individual who was a resident (Part I) or a foreign resident (Part II) for the whole income year. It is the ATO's 'tax on this income', unrounded.

- `IncomeYear` is the year the income year ends: 2025, 2026 or 2027. Residents pay nothing to $18,200 (section 3(1)), then 16% to $45,000 in 2024-25 and 2025-26 or 15% in 2026-27, 30% to $135,000, 37% to $190,000 and 45% above. Foreign residents pay 30% from the first dollar to $135,000, then the same higher rates. The tables match the ATO's [resident](https://www.ato.gov.au/tax-rates-and-codes/tax-rates-australian-residents) and [foreign resident](https://www.ato.gov.au/tax-rates-and-codes/tax-rates-foreign-residents) rates for those years.
- `Resident` defaults to TRUE; FALSE or 0 means a foreign resident.
- 2027-28 and later are refused as a support boundary, although Schedule 7 already sets 14% for the first resident band from 2027-28: the ATO lists no rates for them yet, and the [Treasury Laws Amendment (Tax Reform No. 1) Act 2026](https://www.legislation.gov.au/C2026A00049/asmade/text) adds extra income tax on capital gains (Division 119) for CGT events from 1 July 2027.
- It leaves out the Medicare levy and surcharge, tax offsets, the pro-rated tax-free threshold of a part-year resident, working holiday makers' rates, minors' unearned income and special income components such as averaging.

## Division 7A minimum yearly repayment

`oz.Div7AMinRepaymentλ(OpeningBalance, BenchmarkRate, RemainingTerm)` returns the minimum yearly repayment of each Division 7A amalgamated loan under section 109E(6) of the [Income Tax Assessment Act 1936](https://www.legislation.gov.au/C1936A00027/latest/text), unrounded. It is `PMT(BenchmarkRate, RemainingTerm rounded up, -OpeningBalance)`.

- `OpeningBalance` is the amount of the loan not repaid by the end of the previous income year: one balance, or a column or row of them. A blank stays blank.
- `BenchmarkRate` is the current year's benchmark interest rate, which section 109N(2) sets as the Reserve Bank's bank variable housing loan rate last published before the year starts. The ATO's [benchmark rate page](https://www.ato.gov.au/tax-rates-and-codes/division-7a-benchmark-interest-rate) gives 8.77% for the year ending 30 June 2027 and 8.37% for 2026; a company with a substituted accounting period works out its own.
- `RemainingTerm` is the longest constituent loan's term less the years between the end of the year the loan was made and the end of the previous year. A part year is rounded up, as section 109E(6) requires. One term serves every balance, or give one for each. More than 25 years is refused: section 109N(3) allows 25 years only for a loan wholly secured by a registered mortgage over real property worth at least 110% of it, and 7 years otherwise, and sections 109E(3A) and (3B) govern a 7-year loan later secured and extended. Supply the balance and term those rules give.
- The ATO's [worked examples](https://www.ato.gov.au/businesses-and-organisations/corporate-tax-measures-and-assurance/private-company-benefits-division-7a-dividends/in-detail/division-7a-loans) come out at $9,835 ($55,000 at 5.95% over 7 years) and $10,079 ($50,430 at 5.45% over 6 years) when rounded to the dollar.
- It is arithmetic only. It does not decide whether a loan meets section 109N, which payments count (section 109R), or whether a shortfall is a dividend, and there is no minimum yearly repayment in the year the loan is made.

## Margin scheme GST

`oz.MarginSchemeGSTλ(Consideration, MarginBase)` returns the GST on each taxable supply of real property under the margin scheme: 1/11 of the amount by which the consideration exceeds the margin base, under section 75-10 of the [A New Tax System (Goods and Services Tax) Act 1999](https://www.legislation.gov.au/C2004A00446/latest/text). When the consideration does not exceed the base there is no margin, and the GST is 0. The result is not rounded; section 9-90(1) rounds the GST on an invoice with one taxable supply to the nearest cent.

- `Consideration` is the GST-inclusive price: one, or a column or row of them. A blank stays blank.
- `MarginBase` is, for each price, the consideration for the supplier's acquisition of the interest (section 75-10(2)) or an approved valuation (section 75-10(3)), in the same shape. Improvements, development costs, legal fees and stamp duty stay out of it (section 75-14), and a subdivided lot takes its share of the base (section 75-15).
- The ATO's [worked examples](https://www.ato.gov.au/businesses-and-organisations/gst-excise-and-indirect-taxes/gst/in-detail/your-industry/property/gst-and-the-margin-scheme/calculating-the-gst-payable) come out at $36,363 ($900,000 less $500,000, in whole dollars), $20,000 and $50,000.
- It is arithmetic only. It does not decide whether the margin scheme applies (section 75-5: a written agreement, and an interest not acquired through an ineligible supply), which base or valuation is right, or the margin for an interest from a GST group member, joint venture operator or deceased estate (section 75-11) or a supply to an associate (section 75-13).

## FBT gross-up

`oz.FBTGrossUpλ(TaxableValues, Type)` returns the grossed-up taxable value of each fringe benefit amount under section 5B of the [Fringe Benefits Tax Assessment Act 1986](https://www.legislation.gov.au/C2004A03280/latest/text), unrounded.

- Type 1 (the employer is entitled to a GST credit for the benefit) multiplies by (FBT rate + GST rate) / ((1 - FBT rate) x (1 + GST rate) x FBT rate), section 5B(1B); type 2 multiplies by 1 / (1 - FBT rate), section 5B(1C). The type 2 factor also grosses up a reportable fringe benefits amount (section 135P).
- The FBT rate is 47% (Fringe Benefits Tax Act 1986 section 6) and the GST rate 10%, so the factors are 57,000/27,401 = 2.08021... and 100/53 = 1.88679.... The ATO's [rates and thresholds page](https://www.ato.gov.au/tax-rates-and-codes/fringe-benefits-tax-rates-and-thresholds) shows them to 4 places, 2.0802 and 1.8868; a figure worked with those can differ by up to 0.01%.
- It covers FBT years from 1 April 2017. The years starting 1 April 2015 and 2016 used 49% (section 6A) and are not covered.
- `Type` is 1 or 2: one for every value, or one for each in the same shape.
- It works out rates only. It does not value a benefit, decide whether it is type 1 or type 2, or apply exemptions, the $30,000 or $17,000 caps of section 5B(1E), or rebates.

## Depreciation pools

`oz.LowValuePoolλ` and `oz.SmallBusinessPoolλ` roll a pool forward under the [Income Tax Assessment Act 1997](https://www.legislation.gov.au/C2004A05138/latest/text), one income year in each column. Each returns 4 rows: the opening pool balance, the deduction, the closing pool balance and the amount included in assessable income. Every amount passed in is the taxable use percentage (low-value pool, section 40-435) or taxable purpose proportion (small business pool, section 328-205) of it. Results are not rounded.

- `oz.LowValuePoolλ(OpeningBalance, LowCostAssets, [LowValueAssets], [TerminationValues])`: the decline in value is 18.75% of the low-cost assets allocated for the year and the second element amounts for the year (pass both in `LowCostAssets`), plus 37.5% of the opening balance and the opening adjustable values of low-value assets allocated for the year (section 40-440(1)). The closing balance is the opening balance plus the year's amounts, less the decline (section 40-440(2)), then less termination values, not below zero; any excess is assessable income (section 40-445).
- `oz.SmallBusinessPoolλ(OpeningBalance, Additions, [CostAdditions], [TerminationValues])`: the deduction is 30% of the opening pool balance (section 328-190(1)) plus 15% of the assets first used or installed ready for use in the year and of cost addition amounts (section 328-190(2) to (4)). When the opening balance plus additions and cost addition amounts, less termination values, is more than zero and less than $20,000, it is all deducted and the pool closes at zero (section 328-210). Below zero, it is assessable income and nothing is deducted (section 328-215). A closing balance that would fall below zero is also assessable income. Sums of cents can land a hair off 0 or $20,000 in binary arithmetic (2,420,135.56 + 2,116,636.38 - 4,516,771.94 gives 19,999.99999999907), so the pool adds whole dollars exactly and takes the rest of each amount to its 15 significant digits, the most that Excel keeps, and at most 15 decimal places, as a whole count of 1E-15 dollars that also adds exactly. Each boundary compares that count with the whole dollars short of it, so a pool is 0 or $20,000 only when its amounts add exactly to it, and any amount either side, a cent on trillions of dollars or a trillionth of a dollar, lands on that side. A closing balance carried to the next year is taken to 15 significant digits too.
- The $20,000 low pool value applies to income years ending from 1 July 2023 to 30 June 2026 under section 328-180(6)(e) of the [Income Tax (Transitional Provisions) Act 1997](https://www.legislation.gov.au/C2004A05128/latest/text), and from 1 July 2026 under section 328-210(1) itself. Earlier years used other limits, or none for years ending from 6 October 2020 to 30 June 2023 (section 328-181(5)), and are not covered.
- `OpeningBalance` is one value. `LowCostAssets` and `Additions` are a row with a value for each year; each other row is one value for every year or one for each. A blank is 0.
- They do not decide which assets may go in a pool, the taxable use or purpose, assets written off under section 328-180, balancing adjustments outside the pool, or roll-over relief.

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
