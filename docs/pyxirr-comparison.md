## Checking the cash-flow functions against pyxirr

Ozzit's IRR, lease-liability and loan-amortisation functions were evaluated in desktop
Excel and compared with [pyxirr](https://github.com/Anexen/pyxirr), a separately written
Rust library of financial functions. Agreement between two implementations is evidence
that the arithmetic and conventions match; it does not make either one correct for a
particular accounting or tax purpose.

Every input is fabricated. Nothing here changes the workbook.

### How the comparison runs

`tools/pyxirr_comparison.py` holds the cases. For each one it:

1. enters the Ozzit formula in Excel through `tools/excel_eval_formulas.ps1`, which opens
   `ozzit.xlsx` read-only on a scratch sheet and confirms the file's SHA-256 is unchanged
   afterwards;
2. computes the same quantity with pyxirr from the same inputs;
3. records both, and the largest absolute difference, in `docs/pyxirr-comparison.json`
   with the Excel build, the workbook hash and the pyxirr version.

Tolerances are 0.0000001 for a rate and 0.000001 for an amount. Excel's XIRR stops
iterating at a small tolerance of its own, so rates are not expected to agree to the last
digit.

Rerun it on a Windows host with desktop Excel and no Excel window open:

```powershell
uv run --no-project --with pyxirr==0.10.8 python tools/pyxirr_comparison.py
```

CI has neither Excel nor pyxirr. `tools/tests/test_pyxirr_comparison.py` checks that the
recorded evidence covers exactly the cases in the script, with the same formulas, and that
the table below matches it.

### What each case tests

- **IRR, investment in period 3.** `oz.IRRλ` drops zero values before calling XIRR, so a
  timeline whose first investment is not in its first period still returns the rate from
  the first cash flow. pyxirr is given only the non-zero flows and their dates.
- **IRR, irregular dates.** Five flows over 14 months on uneven dates, testing the
  actual/365 day count both implementations use.
- **IRR, two valid rates.** Flows of -100, 230 and -132 change sign twice, so there are
  two rates at which the net present value is nil (about 10% and 20% a year). Excel's
  XIRR, and so `oz.IRRλ`, returns #NUM! here; pyxirr returns the lower rate. The record
  keeps both answers and the net present value at pyxirr's rate, which confirms it is a
  root. When two numbers are returned, the case compares the net present value at each
  rate, because either root is valid.
- **Lease liability.** The `oz.LeaseLiabilityλ` help example in arrears and in advance
  (where the measurement-date payment is excluded), and 60 monthly payments discounted at
  the monthly equivalent of a 6.5% effective annual rate, against pyxirr's present value.
- **Loan amortisation.** Every opening, interest, repayment, closing and principal figure
  of `oz.AmortiseBλ` in every model period, against pyxirr's `ipmt`, `ppmt` and `fv`:
  a monthly loan, and a quarterly loan whose repayments start in model period 3 of 12,
  where the periods before the start and after the term must be nil.

### Results

The run recorded in `docs/pyxirr-comparison.json`:

<!-- results:start -->
| Case | Compared | Values | Ozzit (Excel) | pyxirr | Largest difference | Agrees |
| --- | --- | --- | --- | --- | --- | --- |
| `irr-help-example` | rate | 1 | 0.2472589791 | 0.2472589845 | 5.44e-09 | yes |
| `irr-irregular-dates` | rate | 1 | 0.4097376287 | 0.4097376278 | 8.59e-10 | yes |
| `irr-two-roots` | rate | 1 | #NUM! | 0.09676477572 | n/a | no, see note |
| `lease-arrears` | amount | 1 | 272.3248029 | 272.3248029 | 1.71e-13 | yes |
| `lease-advance` | amount | 1 | 185.9410431 | 185.9410431 | 1.14e-13 | yes |
| `lease-60-months` | amount | 1 | 77005.37582 | 77005.37582 | 4.37e-11 | yes |
| `amortise-monthly` | amount | 300 | schedule | schedule | 2.09e-10 | yes |
| `amortise-quarterly-deferred` | amount | 60 | schedule | schedule | 1.96e-10 | yes |

- `irr-two-roots`: Excel's XIRR returns #NUM! for this cash flow with any guess tried (0.05, 0.1, 0.25), and IRRλ passes that on. pyxirr returns one of the two valid rates. Neither is wrong; a model with sign changes like these needs a stated rule for which rate it uses.
<!-- results:end -->

For schedules, the Values column counts every figure compared and the difference is the
largest across all of them.

### Limits

- The comparison covers these fabricated cases only, on the tracked `ozzit.xlsx` at the
  commit and SHA-256 in the JSON record. The released v3.4.2 file is a different artefact
  with its own hash, so this is not a check of the release download. A later workbook
  needs a fresh run.
- pyxirr is a second implementation, not an authority. Where the two differ the record
  says so; neither is adjusted to match the other.
- Whether a lease payment, rate or loan term is the right input is an accounting judgement
  these checks do not make.
