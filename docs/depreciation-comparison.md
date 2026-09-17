## Comparing a modelling schedule with an accounting carrying amount

`oz.PrimeCostλ` and `oz.DiminishingValueλ` build a multi-period schedule from a
cost and an effective life. An accounting engine reporting under AASB 116
answers a different question: what is this asset's carrying amount on a given
date. The two agree where their assumptions agree and part company where they
do not, and the places they part company are worth seeing before anyone puts
the two figures in the same column.

Nothing here changes the workbook, adds a function or introduces an add-in,
a macro or a remote formula. This is a worked comparison on fabricated figures,
checked arithmetically by `tools/tests/test_depreciation_comparison.py`.

### The asset

Fabricated. A $120,000 item of plant, five-year life, nil residual value, acquired 1 July 2024.

### Prime cost, annual, and where it agrees

`=oz.PrimeCostλ(120000, 5)` spills five equal periods of 24,000.00. An
accounting engine on a prime-cost basis, asked for the year to 30 June 2025,
gives the same 24,000.00, because a full anniversary year of a straight-line
charge is the same number either way.

| Period | `oz.PrimeCostλ(120000, 5)` | Carrying amount at period end |
|---|---:|---:|
| 1 | 24,000.00 | 96,000.00 |
| 2 | 24,000.00 | 72,000.00 |
| 3 | 24,000.00 | 48,000.00 |
| 4 | 24,000.00 | 24,000.00 |
| 5 | 24,000.00 | 0.00 |

### Diminishing value, and where it does not

`=oz.DiminishingValueλ(120000, 5)` spills 48,000.00, 28,800.00, 17,280.00,
10,368.00 and 15,552.00. The first four are 200% of the straight-line rate
applied to the remaining balance. The fifth is not: it writes off the whole
remaining balance so the schedule reconciles exactly to cost, which is what a
model needs and what the function's own help says it does.

| Period | `oz.DiminishingValueλ(120000, 5)` | Remaining balance |
|---|---:|---:|
| 1 | 48,000.00 | 72,000.00 |
| 2 | 28,800.00 | 43,200.00 |
| 3 | 17,280.00 | 25,920.00 |
| 4 | 10,368.00 | 15,552.00 |
| 5 | 15,552.00 | 0.00 |

An engine that keeps applying the 40% factor in the final period charges
6,220.80 and leaves 9,331.20 on the books. That closing balance is valid only
if the engine's residual-value and useful-life assumptions support it. Under the
five-year, nil-residual assumptions stated here, the remaining depreciable
amount must be allocated by the end of that life; the engine's alternative is
therefore an assumption difference, not an equally valid result. A comparison
that does not align residual value, useful life and final-period convention will
read as a 9,331.20 error without explaining the difference.

### Four things to line up before comparing a figure

1. **Periods.** These functions produce whole periods from period one. An
   engine reporting at a date pro-rates, so a part year will not equal a whole
   one. Compare a full anniversary year against a full anniversary year, or
   compare nothing.
2. **Day count.** An engine may charge on actual days, on a 365-day year or in
   equal monthly instalments. Over a leap-containing year those differ. These
   functions have no day count at all: a period is a period.
3. **The final period.** `oz.DiminishingValueλ` writes the residual off. Most
   engines do not. The difference lands entirely in the last period.
4. **What the number is.** This is a modelling schedule. It is not a deduction
   under ITAA 1997 Division 40, and an AASB 116 carrying amount is not one
   either. Neither figure belongs in a tax return without being worked out on
   its own facts, and the two use different lives and conventions in the first
   place.

### Reconciling a movement

Whatever produced it, an asset movement closes or it does not:

    opening + additions - depreciation = closing

For the prime-cost year above: 120,000.00 + 0.00 - 24,000.00 = 96,000.00. If a
figure arrives that does not close, the missing piece is usually an addition
inside the window. Ask for it. Do not derive it from the gap: a number computed
to make a reconciliation pass is not evidence of anything.

### Native Excel evidence

**NOT_RUN.** The table above was checked by independent arithmetic in
`tools/tests/test_depreciation_comparison.py`, not by Excel. No cached value in
`ozzit.xlsx` was read or written, and no recalculation is claimed: XML tooling
has no formula engine.

To check it in Excel yourself, with the workbook open and calculation enabled:

```text
=oz.PrimeCostλ(120000, 5)
=oz.DiminishingValueλ(120000, 5)
=SUM(oz.DiminishingValueλ(120000, 5))
```

The third returns 120,000.00. If a released workbook's own gates are what you
need, run the sequence in [RELEASING.md](../RELEASING.md); this document is not
part of it and changes nothing it verifies.
