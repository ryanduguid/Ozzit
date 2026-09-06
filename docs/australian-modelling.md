## Australian conventions

The library is Australian-only: the foreign tax regimes and references the earlier library carried have been removed rather than relabelled, and the conventions below are Australian throughout.

- Australian English throughout (amortise, modelling, and so on), including function names.
- Dates are day-first, in text examples and in cell formats. Worksheets print on A4.
- Sample data uses AUD.
- Depreciation, GST and financial-year helpers:

| Function | Purpose |
|---|---|
| `oz.DiminishingValueλ(Cost, Life)` | Diminishing balance at 200% of the straight-line rate, writing the residual off in the final period |
| `oz.PrimeCostλ(Cost, Life)` | Straight line over whole years |
| `oz.GSTAddλ(Amounts, [Rate])` | Adds GST to GST-exclusive amounts, 10% by default |
| `oz.GSTExtractλ(Amounts, [Rate])` | Returns the GST inside GST-inclusive amounts |
| `oz.FinancialYearλ(Dates, [StartMonth])` | Labels dates with their financial year, starting 1 July |

The 10% default and one-eleventh extraction reflect the basic rule for a taxable supply in [*A New Tax System (Goods and Services Tax) Act 1999*](https://www.legislation.gov.au/C2004A00446/latest/text) ss 9-70 and 9-75 (source checked 20 August 2026). Recheck the current Act and any applicable special rule at the time of use. These helpers apply arithmetic only: they do not decide whether a supply is taxable, GST-free, input taxed, outside the GST system or subject to a special rule.

The same scope note is embedded as NOTES! rows in the `oz.GSTAddλ` and `oz.GSTExtractλ` inline help. The About table and Name Manager comments keep the one-line descriptions. `tools/postbuild/gst_help_text.py` applies it to the committed `ozzit.xlsx` and `src/`; `tools/transform_from_earlier.py` still needs the uncommitted earlier workbook and still stops at v3.0.0.

`oz.Depreciateλ` accepts the method codes `SLN`, `SYD`, `DB`, `DDB`, `VDB`, `DV` (diminishing value) and `PC` (prime cost).

**Modelling parameters.** The depreciation helpers compute multi-period asset amortisation from cost and effective life inputs. `oz.DiminishingValueλ` amortises the remaining undeducted balance in the final period so the multi-year schedule reconciles exactly to initial cost (for example, a cost of 1,000 over five years produces 400, 240, 144, 86.40 and 129.60). Part-year apportionments and balancing adjustments are applied in the financial model's period timeline.

## AASB 16 leases

Four functions cover lessee accounting. They compose: the liability feeds the schedule, the schedule's opening balance feeds the right-of-use asset, and a later index review feeds the remeasurement.

| Function | Purpose |
|---|---|
| `oz.LeaseLiabilityλ(Payments, Rate, [InAdvance])` | Present value of the lease payments not paid at the commencement date |
| `oz.LeaseScheduleλ(Payments, Rate, [InAdvance])` | Opening, payment, interest and closing rows for each period |
| `oz.ROUScheduleλ(Cost, Periods)` | Opening, depreciation and closing rows for the right-of-use asset |
| `oz.LeaseRemeasureλ(RevisedPayments, Rate, CarryingLiability, CarryingROU, [InAdvance])` | Revised liability, adjustment, revised asset and the remainder taken to profit or loss |

`Rate` is the rate **per period**, not per year: divide an annual rate by the number of periods in a year before passing it. Set `InAdvance` to `TRUE` when the first supplied payment is made at the measurement date. That payment is excluded from the liability and the schedule's payment row; the remaining payments unwind after one period of interest. A schedule with payments in advance therefore has one fewer liability period. Add a payment made at or before commencement to the right-of-use asset cost instead.

Three things the functions take rather than decide:

- **The payments.** Supply all payments in order, including the measurement-date payment first when `InAdvance` is `TRUE`. Include an amount expected to be payable under a residual value guarantee, or the exercise price of a purchase option, in the final period where [AASB 16](https://standards.aasb.gov.au/aasb-16-nov-2022) paragraph 27 brings it into the lease payments. Leave out variable payments that depend on sales or usage.
- **The rate.** Paragraph 26 discounts at the interest rate implicit in the lease where that rate can be readily determined, and at the lessee's incremental borrowing rate where it cannot. Where you have the fair value and the residual, `oz.IRRλ` over the same cash flows gives the implicit rate as an annual rate, because it wraps XIRR; convert it to the rate per period before passing it, for example `(1 + annual) ^ (1 / 12) - 1` for monthly payments.
- **The asset's cost.** Paragraph 24 builds it from the initial liability, payments made at or before commencement less incentives received, initial direct costs, and an estimate of dismantling and restoration costs. Add those four up and pass the total as `Cost`.

Together the liability and the asset produce the expense profile AASB 16 is known for. Interest falls as the liability unwinds while straight-line depreciation does not, so a lease costs more in its first period than its last even though the rent never moves. That is the shape the schedule is for.

Remeasurement follows paragraphs 39 to 43. A change in future payments caused by a change in an index or a rate is remeasured under paragraph 42(b), and only when the cash flows actually change. Paragraph 43 holds the discount rate unchanged for that revision, and uses a revised rate only where the change arises from a floating interest rate. Paragraph 39 takes the adjustment to the right-of-use asset, and where that asset is already nil the remainder goes to profit or loss, which is the fourth row `oz.LeaseRemeasureλ` returns.

Paragraph references were read against the AASB 16 compilation on 24 August 2026. Recheck the current compilation at the time of use.

**These are schedules, not determinations.** They do not decide the lease term, decide what counts as a lease payment, decide whether an arrangement contains a lease, or apply the short-term and low-value recognition exemptions. There is no lessor accounting here. Use them to build and check a model, not to conclude on the standard.
