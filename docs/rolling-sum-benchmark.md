# Rolling-sum benchmark, 8 September 2026

Keep the current `oz.RollingSumλ` calculation. Subtracting cumulative totals was
faster on long rows, but lost small values after a large value and changed how
text inputs behave.

## Method

Measured in Excel 16.0 build 20326 on Windows, using the workbook from commit
`feb9dd1fa08b0acc3e6f3ea69795ef2b91ae80c8`. The workbook was opened read-only
and closed without saving; its SHA-256 remained
`8ea782fe7ee63605394acace586df76332fafedfea5d8bc43fb672e8424115a6`.

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
| 120 | 12 | 1.641 | 1.464 | 9.95e-14 |
| 1,200 | 120 | 8.703 | 1.940 | 2.27e-12 |
| 10,000 | 120 | 61.043 | 4.091 | 1.51e-11 |
| 10,000 | 1,000 | 758.964 | 5.532 | 1.73e-11 |

The following cases were also evaluated in native Excel. Numeric results were
read through `Value2`, so cell formatting cannot hide a zero or a lost value.

| Input | Window | Current result | Candidate result |
|---|---:|---|---|
| `{1E16,1,1}` | 1 | `{1E16,1,1}` | `{1E16,0,0}` |
| `{1,"x",3,4}` | 2 | `{1,1,3,7}` | `{1,#VALUE!,#VALUE!,#VALUE!}` |
| `HSTACK(1,NA(),3,4,5)` | 2 | `#N/A` at the anchor | `#N/A` at the anchor |

## Decision

The cumulative candidate does not preserve the function's results. Adding text
handling would not fix the precision loss. The current implementation remains
unchanged; the candidate was not added to the library. Revisit optimisation only
with representative large models and an approach that passes these cases as well
as the existing native assertions.
