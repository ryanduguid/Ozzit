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
this comparison.

Normal CI checks the retained native record against the current workbook
hash and exact evaluated formulas, compares the Python arithmetic with the
independent values, rejects the recorded wrong variants and retains the
formula-text pins. CI does not start
Excel. A workbook change requires new native evidence before this record can
qualify the changed workbook.
