## Modules

Every function shares one prefix, `oz.`, so a call is three characters of namespace
rather than eight. The groupings below describe what the library covers; they are not
part of the name.

| Group | Functions | Covers |
|---|---|---|
| Dates | 14 | Dates and timelines: periods, schedules, overlaps, occurrence tests, financial-year labels |
| Essentials | 17 | Array essentials: row and column totals, averages, counts, range conversion |
| Financial | 46 | Financial building blocks: amortisation, depreciation, corkscrews, IRR, rolling sums, GST, AASB 16 leases |
| Ratios | 39 | Financial ratios: liquidity, leverage, margins, returns, market multiples |
| Utilities | 17 | Standalone copies of the Essentials functions, each carrying a `U` suffix |
| Debt | 5 | Debt sculpting: amortisation schedule, fixed and variable DSCR sculpting, sculpting interest |

Where two groups shipped a function of the same name, the fuller implementation keeps
the plain name and the other takes a one-letter tag: `B` for debt, `E` for essentials,
`U` for utilities. So `oz.Amortiseλ` is the financial amortisation schedule and
`oz.AmortiseBλ` the debt one; `oz.SumRowsλ` and `oz.SumRowsUλ` are the essentials and
utilities copies. The five About tables take words instead: `oz.AboutFinancialλ`,
`oz.AboutRatiosλ`, and so on.

## Worksheet catalogue

| Worksheet | What it demonstrates |
|---|---|
| `oz.CountDOWλ` | Count instances of a specific day of the week between two dates |
| `oz.IsBetweenλ` | Determine if a value is between a lower and upper limit |
| `oz.IsOccurrenceDateλ` | Determine if a date passed is when a potentially repeating event happens |
| `oz.OverLapDaysλ` | Return how many days overlap two period ranges. |
| `oz.Periodsλ` | Determine the number of periods from Starts to Ends inclusive |
| `oz.PeriodLabelλ` | Creates a label for a date based on period interval |
| `oz.ScheduleRatesλ` | Schedule rates that persist until replaced in a timeline. |
| `oz.ScheduleRatesByItemsλ` | Schedule rates that persist until replaced in a timeline for each item in a list. |
| `oz.ScheduleValuesλ` | Schedules values in a timeline. |
| `oz.ScheduleValuesByItemsλ` | Schedules values in a timeline for each item in a list. |
| `oz.Timelineλ` | Creates a horizontal list of start or end dates for a timeline |
| `oz.Amortiseλ` | Creates a corkscrew amortisation schedule. |
| `oz.LabelAmortiseλ` | Create row labels for Amortiseλ result |
| `oz.SumAmortiseλ` | Create totals for payments, interest, and principal portion in Amortiseλ results |
| `oz.Corkscrewλ` | Creates a simple corkscrew where the closing balance is the sum of independent flows plus opening balance |
| `oz.Cumulativeλ` | Creates a row or column of cumulative totals from a total row or column |
| `oz.Depreciateλ` | Create a block of CAPEX, Opening Balance, Depreciation Values, and Book Value for each asset |
| `oz.LabelDepreciateλ` | Create row labels for Depreciateλ result |
| `oz.SumDepreciateλ` | Create row totals for CAPEX, Depreciation, Salvage Value, and Disposal costs in Depreciateλ results |
| `oz.SumContainsλ` | Creates a row of totals for each row in an array where its labels contain a unique letter, word, or phrase. |
| `oz.IntOnIntλ` | Calculate Interest on Interest. Use to determine amount needed to cover debt plus interest on debt |
| `oz.IRRλ` | Calculates IRR, correcting for when the first investment is not in the first period |
| `oz.Reversalλ` | Create a row that reverses input values in the next period. |
| `oz.Movementλ` | Create a row of differences from column to column |
| `oz.RollingSumλ` | Creates totals for preceding values of a set size moving from beginning to end over a row of values. |
| `oz.SumPeriodsλ` | Groups and totals all columns in a Values array by period resulting in one column for each period. |
| `oz.TimelineOffsetλ` | Determines how many columns a date is offset from a timeline's first date |
| `oz.TimelinePositionλ` | Places an array or value appropriately within a model's timeline. |
| `oz.AboutEssentialsλ` | About the Ozzit Array Essentials library |
| `oz.CountCλ` | Count the number of times one or more characters appear in a string |
| `oz.SumRowsλ` | Creates totals for each row in array. |
| `oz.SumColsλ` | Creates totals for each column in array. |
| `oz.AvgRowsλ` | Gets the average of each row in an array |
| `oz.AvgColsλ` | Gets the average of each column in an array |
| `oz.MinRowsλ` | Gets the minimum of each row in an array |
| `oz.MinColsλ` | Gets the minimum of each column in an array |
| `oz.MaxRowsλ` | Gets the maximum of each row in an array |
| `oz.MaxColsλ` | Gets the maximum of each column in an array |
| `oz.CountRowsλ` | Count the number of numbers in each row of an array |
| `oz.CountColsλ` | Count the number of numbers in each column of an array |
| `oz.CountARowsλ` | Count all non-empty cells in each row of a range. NOTE! Dynamic Arrays always fill each cell. |
| `oz.CountAColsλ` | Count all non-empty cells in each column of a range. NOTE! Dynamic Arrays always fill each cell. |
| `oz.IsBetweenEλ` | Determine if a value is between a lower and upper limit |
| `oz.RangeToDAEλ` | Convert a static range into a dynamic array |
| `oz.FinancialRatios` | Three dozen financial Ratios |

The other 94 functions, including all of Ratios, Utilities, Debt and the AASB 16 lease helpers and every module About table except `oz.AboutEssentialsλ`, have no dedicated worksheet; call any of them with no arguments for inline help, `oz.FinancialRatios` demonstrates the ratio suite on one worksheet, and [functions.csv](../functions.csv) lists every function with its signature.

## Dynamic-Array Formula Walkthrough

Ozzit functions use native Excel dynamic arrays to spill full calculation schedules from a single formula cell:

### 1. Loan Amortisation (`=oz.Amortiseλ(Principals, APRs, Terms, StartDates, [Timeline])`)
Spills a six-row corkscrew per loan across the model's timeline, one column per period: debt issued, opening balance, interest, payment, closing balance and the principal repaid. Payments are monthly; on a quarterly or annual timeline the months are grouped, and on a weekly or daily one each month lands in the period holding its start. `oz.LabelAmortiseλ` labels the rows and `oz.SumAmortiseλ` totals them. For a single 100,000 loan at 5% over 60 months from 1 July 2026, with the timeline omitted, the block runs:

| Row | Jul 2026 | Aug 2026 | Sep 2026 | ... |
| :--- | ---: | ---: | ---: | :---: |
| Debt issued | 100,000.00 | 0.00 | 0.00 | ... |
| Opening balance | 100,000.00 | 98,529.54 | 97,052.96 | ... |
| Interest | 416.67 | 410.54 | 404.39 | ... |
| Payment | -1,887.12 | -1,887.12 | -1,887.12 | ... |
| Closing balance | 98,529.54 | 97,052.96 | 95,570.22 | ... |
| Principal repaid | 1,470.46 | 1,476.58 | 1,482.73 | ... |

### 2. Diminishing Value Depreciation (`=oz.DiminishingValueλ(Cost, Life)`)
Calculates diminishing balance at 200% straight-line rate with exact terminal residual write-off:

| Year 1 | Year 2 | Year 3 | Year 4 | Year 5 (Residual Write-off) | Total Written Off |
| :---: | :---: | :---: | :---: | :---: | :---: |
| \$400.00 | \$240.00 | \$144.00 | \$86.40 | \$129.60 | \$1,000.00 |

### 3. GST Arithmetic (`=oz.GSTExtractλ(Amounts)` & `=oz.GSTAddλ(Amounts)`)
Extracts or appends GST across full dynamic arrays according to *ANTS(GST)A 1999* ss 9-70/75:
- `=oz.GSTExtractλ(1100)` -> returns `$100.00`
- `=oz.GSTAddλ(1000)` -> returns `$1,100.00`
