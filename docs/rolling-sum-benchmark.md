# Rolling-sum benchmark, 8 September 2026

Keep the current `oz.RollingSumλ` calculation. Subtracting cumulative totals was
faster on long rows, but lost small values after a large value and changed how
text inputs behave.

## Method

Measured in Excel 16.0 build 20326 on Windows, using the workbook from commit
`b881b5f8628856b1d5e997550ae5e6e5369b2b57`. The workbook was opened read-only
and closed without saving; its SHA-256 remained
`5fc14b1496fcba9a164e7f0bee16f727b991b0583336a4ba6a1798f4feaff302`.

To reproduce the measurements, close Excel and run this command from the
repository root on Windows with Microsoft 365 Excel installed:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\benchmark_rolling_sum.ps1 -Path .\ozzit.xlsx
```

The script prints the workbook hash, Excel version, measurement time and Markdown
tables. It refuses to start while Excel is running and closes its own workbook
without saving. The tables below are its output from 8 September 2026 at 23:31
AEST. Timings will vary between runs.

Each input was a horizontal row of numbers, with zero-based column `i` holding
`((i * 17) % 997) / 100`. The current function and the candidate below occupied
separate spill anchors on a temporary sheet. Calculation was manual. After three
warm-up calculations per anchor, `Range.Calculate()` was timed nine times per
anchor, alternating which anchor ran first. The table reports medians, including
COM call overhead, and the largest absolute difference across the two output
rows. These timings describe this machine and these inputs.

```excel
=LAMBDA(values,size,
    LET(
        totals,SCAN(0,values,LAMBDA(acc,value,acc+value)),
        cols,SEQUENCE(,COLUMNS(values)),
        totals-IF(cols>size,
            CHOOSECOLS(totals,IF(cols>size,cols-size,1)),0)
    )
)
```

## Results

| Columns | Window | Current, ms | Cumulative candidate, ms | Maximum absolute difference |
|---:|---:|---:|---:|---:|
| 120 | 12 | 1.469 | 1.263 | 9.95E-014 |
| 1200 | 120 | 8.350 | 1.519 | 2.27E-012 |
| 10000 | 120 | 60.793 | 3.735 | 1.51E-011 |
| 10000 | 1000 | 762.672 | 5.534 | 1.73E-011 |

The following cases were also evaluated in native Excel. Numeric results were
read through `Value2`, so cell formatting cannot hide a zero or a lost value.

| Input | Window | Current result | Candidate result |
|---|---:|---|---|
| {1E16,1,1} | 1 | [1E+16, 1, 1] | [1E+16, 0, 0] |
| HSTACK(1,NA(),3,4,5) | 2 | [#N/A, , , , ] | [#N/A, , , , ] |
| {1,"x",3,4} | 2 | [1, 1, 3, 7] | [1, #VALUE!, #VALUE!, #VALUE!] |

For the `NA()` case, both formulas return a scalar error at the anchor. The empty
entries are the remaining cells inspected in the output row.

## Decision

The cumulative candidate does not preserve the function's results. Adding text
handling would not fix the precision loss. The current implementation remains
unchanged; the candidate was not added to the library. Revisit optimisation only
with representative large models and an approach that passes these cases as well
as the existing native assertions.
