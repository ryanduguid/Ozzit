# Day-count comparison with QuantLib

Ozzit's first-period year fractions agreed with QuantLib 1.43 for 72 comparisons
across 18 boundary pairs. Desktop Excel 16.0 build 20527 evaluated the shipped
`oz.DayCountRateλ` name on 5 October 2026. The largest absolute difference was
`4.163336342344337e-17`, below the comparison tolerance of `1e-12`.

The retained record is
`tools/tests/postbuild/fixtures/quantlib-day-counts.json`. It includes the
QuantLib values, exact wheel and extracted binary hashes, generator hash,
constructor expressions, counter names, Python version and platform, and the
Excel results, exact formula-input and helper hashes, and evaluation time. The
retained formula input is `quantlib-day-count-formulas.json` in the same fixture
directory. The workbook hash before and after evaluation was
`8c42e2278de61bdacfb090b940e684bb12a7636a256816aa20f81360b71bb760` at
Ozzit base `8a0d807b5a92051707dafe10c800cafa6b57437e`.

| Method | QuantLib construction | Boundary rule |
| --- | --- | --- |
| 1 | `Thirty360(European)` | Clamp either endpoint's 31st to the 30th |
| 2 | `Actual360(False)` | Elapsed days divided by 360; exclude the following boundary |
| 3 | `Actual365Fixed(Standard)` | Elapsed days divided by 365 |
| 4 | `ActualActual(ISDA)` | Split at calendar-year boundaries and use each year's length |

The counter receives `[start, following)`, corresponding to the LAMBDA's
`Starts` and `Next`, where `Next = Ends + 1`. Review the matching implementation
in [QuantLib's tagged source](https://github.com/lballabio/QuantLib/tree/v1.43/ql/time/daycounters).
QuantLib supplies an independent implementation; the reviewer still chooses
the financial convention.

The cases include February ends, 31st-day clamping, a full leap year, the 2000
and 2100 century rules, calendar-year crossings and long positive intervals.
Eight controls distinguish the selected rules from USA, Bond Basis, 30/360
ISDA, inclusive Actual/360, Actual/365 NoLeap, Actual/Actual ISMA and AFB, and
confusing Actual/365 Fixed with Actual/Actual ISDA over a leap year.
The schedule-free ISMA control only demonstrates a different enum's result.

## Reproduce the reference values

The optional generator requires Linux and this exact
[published wheel](https://files.pythonhosted.org/packages/2e/39/2df89a3f4fe6668535d7c56f29ee646f7642e46b7fc6a9db1707804c7520/quantlib-1.43-cp39-abi3-manylinux_2_24_x86_64.manylinux_2_28_x86_64.whl).
It checks the wheel hash, extracts it into a temporary directory, calls
QuantLib directly and writes a new reference file. It imports no Ozzit
arithmetic and adds no normal test dependency.

```sh
python tools/tests/postbuild/generate_quantlib_day_count_fixture.py /path/to/quantlib-1.43-cp39-abi3-manylinux_2_24_x86_64.manylinux_2_28_x86_64.whl /path/to/new-reference.json
```

To reproduce the native comparison on Windows, construct each case as
`=INDEX(oz.DayCountRateλ(HSTACK(DATE(y,m,d),DATE(y2,m2,d2)),1,method),1,1)`.
Pass the retained input file to `tools/excel_eval_formulas_verified.ps1` with a
new output path. This wrapper freezes the input bytes and invokes the existing
Excel helper, which refuses to run while user Excel is open, opens the workbook
read-only and closes without saving. It retains the final output only after
the helper completes successfully and the workbook hash is checked. It records
both helper hashes and reports the scratch location containing the owned Excel
PID for timeout recovery. The original helper is pinned to historical release
evidence and remains part of that record.

## What the record proves

The native run covers the first result from two supplied dates, with APR=1,
an explicit convention and default `EndDates`. The zero-length pair records
the observed result without asserting that duplicate dates form a useful
financial timeline. Later inferred periods, `EndDates=TRUE`, date coercion,
APR broadcasting, reverse intervals and Excel's 1900 date anomaly are outside
the QuantLib comparison. The additional native qualification below covers
positive schedules and records the remaining limitations.

Normal CI checks the retained native record against the current workbook
hash and exact evaluated formulas, compares the Python arithmetic with the
independent values, rejects the recorded wrong variants and retains the
formula-text pins. CI does not start
Excel. A workbook change requires new native evidence before this record can
qualify the changed workbook.

## Native schedule and input qualification

On 6 October 2026, Excel 16.0 build 20527 evaluated 113 additional formulas
against the same workbook hash. All 312 numeric values from 87 supported-input
cases on strictly increasing timelines agreed with declared rational calculations
within `1e-12`. Three invalid
numeric convention cases matched the native `#VALUE!` control. The remaining
23 cases retain observations of unusual or invalid inputs, including the error
controls; they do not qualify those inputs for financial use.

The inputs and expectations are in
`tools/tests/postbuild/fixtures/native-day-count-cases.json`, and the native
results are in `native-day-count-results.json` beside it. The existing verified
Excel helper evaluated that exact input file with LF line endings, as Git stores it.
The result records its exact input hash, both helper hashes, workbook hash and
evaluation time. Excel opened the workbook read-only and closed without saving;
its bytes were unchanged.

The numeric cases cover all four conventions across common and leap-year
monthly schedules, month ends, weekly schedules, annual periods and calendar-year
crossings. They include inferred final start-date periods and inferred first
end-date periods, the 27/28-day inference threshold, row and column dates,
scalar and row or column APR, zero and negative APR, omitted and non-numeric
conventions, and logical or numeric end-date flags. ISO text, English month-name
text and mixed numeric/text dates matched numeric dates in this Excel profile.
Text-date acceptance in other locales remains unverified.

Each base schedule declares rational terms separately from the workbook
implementation. For example, January through April 2024 has Actual/Actual
terms `31/366`, `29/366`, `31/366` and `30/366`; the end-date schedule uses
31 January, 29 February, 31 March and 30 April for the same periods. The tests
apply each APR to those declared terms and require one result row of the expected
length. These hand calculations extend the native input qualification; they
are separate from the compiled QuantLib record above.

Descending dates expose an Actual/Actual limitation. For the first interval
from 15 January 2024 back to 15 December 2023, Excel returned `-31/366`
(`-0.08469945355191257`). Signed ISDA is `-(17/365 + 14/366)`
(`-0.08482670858597201`). The help does not explicitly specify descending-date
support. This interval is not qualified for signed ISDA; use ascending timelines
pending a date-ordering contract decision. The regression check records the
discrepancy without treating the observed result as a correct financial value.

Mismatched APR lengths can return partial numeric rows with `#N/A`, and a
two-dimensional timeline can return a matrix containing `#N/A`. Those shapes
are outside the documented row-or-column, scalar-or-one-rate-per-period input
contract. Fractional date serials and Excel's fictitious 29 February 1900 remain
observations only. A single date and invalid text produced native errors.

To repeat the run, pass `native-day-count-cases.json` to
`tools/excel_eval_formulas_verified.ps1` with a new output path while Excel is
closed. Its extra expectation fields are ignored by the native helper. Review
the rational terms and limitations before replacing the retained result. CI
checks both records against the current workbook and helper hashes; it does
not launch Excel. A changed workbook requires refreshed native evidence.
