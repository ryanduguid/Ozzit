# Valuation and sensitivity

`oz.FCFFBridgeλ`, `oz.WACCλ` and `oz.SensitivityGridλ` support a discounted cash flow model.
None of them chooses an input, makes a tax or accounting classification, or rounds its result.

## FCFFBridgeλ

`oz.FCFFBridgeλ(EBIT, TaxRate, Depreciation, CapitalExpenditure, WorkingCapitalChange)`
returns free cash flow to the firm for each period as 7 rows: EBIT, tax on EBIT, NOPAT,
depreciation, capital expenditure, the increase in working capital and FCFF.

- FCFF is EBIT x (1 - TaxRate), plus depreciation and amortisation, less capital expenditure,
  less the increase in non-cash working capital. Aswath Damodaran's
  [FCFF valuation notes](https://pages.stern.nyu.edu/~adamodar/New_Home_Page/lectures/fcff.html)
  set out the same bridge: EBIT (1 - t), less capital expenditure net of depreciation, less the
  change in working capital (read 24 November 2026).
- Working capital here is the valuation measure: non-cash current assets less non-debt current
  liabilities ([Damodaran, 'Estimating cash flows', slide 146](https://pages.stern.nyu.edu/~adamodar/pdfiles/eqnotes/dcfcf.pdf), read 24 November 2026).
- `EBIT` is one row with an amount for each period. `TaxRate` is one rate, or one for each
  period, from 0 to under 1. The other 3 are rows as wide as `EBIT`.
- Enter capital expenditure and an increase in working capital as positive amounts, and a
  decrease in working capital as negative. Tax on a negative EBIT is negative, a benefit; set
  the tax rate to 0 for a period whose losses give no benefit.
- `=oz.FCFFBridgeλ({1600,1680}, 0.4, {90,94.5}, {200,210}, {150,17.5})` returns NOPAT of 960
  and 1,008 and FCFF of 700 and 875. Those are the figures in Damodaran's two-year
  illustration: net capital expenditure of 110 and 115.50 and working capital changes of 150
  and 17.50. The EBIT, tax rate and depreciation split are assumed, as the illustration starts
  from EBIT (1 - t).

## WACCλ

`oz.WACCλ(EquityValue, DebtValue, RiskFreeRate, Beta, EquityRiskPremium, CostOfDebt, TaxRate)`
returns the weighted average cost of capital.

- The cost of equity is the capital asset pricing model's `RiskFreeRate + Beta x
  EquityRiskPremium`. The result is the cost of equity times `EquityValue / (EquityValue +
  DebtValue)`, plus `CostOfDebt x (1 - TaxRate)` times `DebtValue / (EquityValue +
  DebtValue)`. Damodaran's
  [discount rate slides](https://pages.stern.nyu.edu/~adamodar/pdfiles/eqnotes/dcfrates.pdf)
  give both formulas (slides 28 and 112, read 24 November 2026) and use market value weights
  and the marginal tax rate.
- Each input is one value. Equity and debt values are 0 or more and under 10,000,000,000,000,
  and cannot both be 0; rates are above -1 and below 2; `Beta` is from -10 to 10; `TaxRate`
  is from 0 to under 1.
- Rates are annual and used as entered, so keep them all nominal or all real.
- It does not adjust for dividend imputation credits. An Australian valuation that values
  franking credits needs that adjustment made separately.
- `=oz.WACCλ(600, 400, 0.04, 1.2, 0.06, 0.07, 0.3)` returns 0.0868: a cost of equity of 11.2%
  on 60% equity, plus 7% x 0.7 on 40% debt. The self-test also reproduces the 9.97% in
  Damodaran's slide 108: its 10.70% cost of equity, entered as a 4.29% risk-free rate plus a
  beta of 1 times 6.41%, its 9.29% cost of debt, 34% tax and 0.84 and 0.16 weights.

## SensitivityGridλ

`oz.SensitivityGridλ(Calculation, RowValues, ColumnValues)` returns a two-way sensitivity grid:
`Calculation`'s result for each value in `RowValues` (down) paired with each value in
`ColumnValues` (across).

- `Calculation` is a `LAMBDA` of 2 parameters, the row value then the column value, returning
  one value. A value that is not a `LAMBDA` is refused. Excel gives a `LAMBDA` with the wrong
  number of parameters `#VALUE!`, which then shows in every cell.
- `RowValues` is one column and `ColumnValues` one row, each of at most 1,000 values. Place
  them to the left of and above the grid.
- The grid recalculates with the workbook like any formula. A What-If Data Table also
  recalculates, but its cells are `TABLE()` entries on the same sheet as its input cells.
- An error in one result shows in its cell; the rest still calculate.
- `=oz.SensitivityGridλ(LAMBDA(Rate, Growth, 100 / (Rate - Growth)), {0.08;0.1}, {0.02,0.03})`
  returns 1,666.67 and 2,000 in its first row and 1,250 and 1,428.57 in its second: a
  growing perpetuity of 100 at 2 discount rates and 2 growth rates.
- With the other 2 functions, `LAMBDA(Rate, Growth, ...)` can value a model's FCFF at each
  WACC and terminal growth rate.
