## Ratio definitions

Each of the 38 ratio functions in `src/Ratios.txt` divides one supplied figure by
another. This page records the arithmetic each one performs, the balance or period
each argument should carry, and where a definition differs from the one the name
usually means. It was written by reading the formulas in the source at commit
`56ef006`; no workbook value was recalculated for it.
`tools/tests/test_ratio_definitions.py` fails when a formula on this page no longer
matches the source.

### Conventions that apply to every ratio

- **Arguments.** Every argument is required except `OpeningEquity` in `oz.ROEλ`;
  omitting a required argument returns the function's help text. `oz.BVPSλ` needs
  `PreferredStock` and `oz.EquityRatioλ` needs `IntangibleAssets`: pass 0 when there
  are none.
- **Zero denominators.** No ratio tests its denominator, so a zero denominator
  returns Excel's `#DIV/0!` error. A negative denominator, such as negative equity
  or negative working capital, returns a ratio whose sign is flipped: read the sign
  before the size.
- **Units.** Margins, returns, the payout and retention ratios and dividend yield
  come back as fractions (0.25 for 25%), so format them as percentages; `oz.DSIλ`
  returns days; `oz.EPSλ` and `oz.BVPSλ` return an amount per share; the rest return
  a plain ratio such as 1.075.
- **Periods.** No ratio annualises. Where a flow is divided by a balance (the
  turnover ratios, the returns and `oz.DSIλ`), supply a full year's flow, or scale a
  shorter period's flow to a year first. `oz.DSIλ` multiplies by a fixed 365.
- **Balances.** The argument name usually says which balance a function expects.
  Arguments named `Average…` want an average of opening and closing balances, and
  `oz.ROEλ` averages opening and closing equity itself. `WorkingCapital` in
  `oz.WorkingCapitalTurnoverRatioλ` is an average too, as its help says, though its
  name does not. The others divide by the balance as supplied; common definitions of
  return on assets and asset turnover use average total assets, and many analysts
  divide return on invested capital by average invested capital.

### The 38 functions

| Function | Arithmetic in the source | Notes |
| --- | --- | --- |
| `oz.CurrentRatioλ` | `Assets/Liabilities` | Current assets and current liabilities. |
| `oz.QuickRatioλ` | `QuickAssets/Liabilities` | Quick assets and current liabilities. |
| `oz.CashRatioλ` | `Cash/Liabilities` | Cash and cash equivalents, and current liabilities. |
| `oz.OperatingCashFlowRatioλ` | `OperatingCashFlow/Liabilities` | Supply a year's operating cash flow; the denominator is current liabilities. |
| `oz.ReceivablesTurnoverRatioλ` | `NetCreditSales/AverageAccountsReceivable` | Supply a year's credit sales. |
| `oz.InventoryTurnoverRatioλ` | `CostOfGoodsSold/AverageInventory` | Supply a year's cost of goods sold. |
| `oz.WorkingCapitalTurnoverRatioλ` | `NetAnnualSales/WorkingCapital` | Supply average working capital, as the help asks. Negative working capital gives a negative ratio. |
| `oz.DebtRatioλ` | `TotalDebt/TotalAssets` | Same arithmetic as `oz.DebtToAssetRatioλ`; both names are in common use. |
| `oz.DSCRλ` | `NetOperatingIncome/TotalDebtService` | |
| `oz.DebtToCapitalRatioλ` | `Debt/(Debt+ShareholdersEquity)` | |
| `oz.EquityMultiplierλ` | `TotalAssets/TotalShareholdersEquity` | |
| `oz.DebtToEquityRatioλ` | `TotalLiabilities/ShareholdersEquity` | The numerator is total liabilities, the broader of the two usual definitions; the narrower divides interest-bearing debt by equity. |
| `oz.DebtToAssetRatioλ` | `TotalDebt/TotalAssets` | Same arithmetic as `oz.DebtRatioλ`. |
| `oz.InterestCoverageRatioλ` | `OperatingIncome/InterestExpenses` | Gross interest expense; some definitions deduct interest income first. |
| `oz.EquityRatioλ` | `ShareholdersEquity/(TotalAssets - IntangibleAssets)` | Deducts intangibles from assets. Pass 0 for equity over total assets, the more common definition. |
| `oz.AssetTurnoverRatioλ` | `NetSales/TotalAssets` | Supply a year's sales. |
| `oz.DSIλ` | `AverageInventory/CostOfGoodsSold * DpY` | `DpY` is 365, so supply a year's cost of goods sold. |
| `oz.OperatingRatioλ` | `(OperatingExpenses+CostOfGoodsSold)/NetSales` | |
| `oz.GrossMarginλ` | `GrossProfit/NetSales` | |
| `oz.EBITDAMarginλ` | `EBITDA/TotalRevenue` | |
| `oz.OperatingMarginλ` | `OperatingEarnings/Revenue` | |
| `oz.PretaxMarginλ` | `PretaxEarnings/Revenue` | |
| `oz.NetProfitMarginλ` | `NetIncome/Revenue` | |
| `oz.CashFlowMarginλ` | `CashFlowFromOperatingActivities/Revenue` | |
| `oz.ROAλ` | `NetIncome/TotalAssets` | Supply a year's net income; common definitions use average total assets. |
| `oz.ROEλ` | `NetIncome/((ShareholdersEquity + Opening)/2)` | `Opening` is `OpeningEquity`; when it is omitted or not a number, closing equity stands in and the ratio uses closing equity alone. |
| `oz.ROIλ` | `NetReturnonInvestment/CostofInvestment` | |
| `oz.ROICλ` | `NetOperatingProfitAfterTax/InvestedCapital` | Invested capital is debt plus equity as supplied; a common convention divides by average invested capital. |
| `oz.PriceEarningsRatioλ` | `SharePrice/EarningsPerShare` | |
| `oz.PriceToBookRatioλ` | `MarketPricePerShare/BookValuePerShare` | The help asks for tangible book value per share, which makes this price to tangible book. For the usual price-to-book ratio, supply book value per share including intangibles, as `oz.BVPSλ` returns. |
| `oz.PriceToSalesRatioλ` | `MarketPricePerShare/SalesPerShare` | |
| `oz.PriceToCashRatioλ` | `MarketPricePerShare/OperatingCashFlowPerShare` | |
| `oz.BVPSλ` | `(ShareholdersEquity - PreferredStock)/AverageSharesOutstanding` | The argument asks for average shares; book value at a date is usually divided by the shares on issue at that date. |
| `oz.CAPERatioλ` | `SharePrice/TenYearAverageEarningsInflationAdjusted` | The denominator is ten-year average earnings per share adjusted for inflation. |
| `oz.DPRλ` | `DividendsPaid/NetIncome` | |
| `oz.DividendYieldRatioλ` | `AnnualDividendsPerShare/SharePrice` | |
| `oz.EPSλ` | `(NetIncome-PreferredDividends)/EndOfPeriodCommonSharesOutstanding` | Divides by shares at the end of the period. Basic earnings per share under AASB 133 paragraph 10 uses the weighted average number of ordinary shares outstanding during the period, so pass that average for an AASB 133 figure. |
| `oz.RetentionRatioλ` | `RetainedEarnings/NetIncome` | The numerator is the period's earnings retained (net income less dividends), not the retained earnings balance the argument name suggests. |

### Gaps

Receivable days, payable days, WIP days and the cash conversion cycle are not
present as functions in the audited revision. `oz.ReceivablesTurnoverRatioλ` and
`oz.InventoryTurnoverRatioλ` are related but return a turnover, not days, and
`oz.DSIλ` covers inventory only. Receivable days over a year is 365 divided by the
receivables turnover when both use the same year's credit sales. Common definitions
differ on the day basis: some divide by the period's own flow and days, others by a
rolling three months, so a function would have to state its basis in its help.
Annualising a short period, and a variance with a stated rule for zero, negative and
sign-changing comparisons, are also absent. Adding any of them needs a workbook
release with the native gates in `RELEASING.md`, so this page records the gaps
rather than filling them.

### Help text found while reading

The help for `oz.CurrentRatioλ` lists "Marketable securities" and "Assets expected to
liquidated in the current year" twice, and the second should read "expected to be
liquidated". Changing help text changes the workbook's cached help, so the fix waits
for the next workbook release.
