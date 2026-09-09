## 13-week cash-flow forecast template

`templates/13-week-cash-flow-forecast.xlsx` is a standalone weekly liquidity model for Australian FP&A work. It is a separate workbook from `ozzit.xlsx`: it uses native Excel formulas, needs no LAMBDA import and carries none of the `oz.` functions. Open it in Microsoft 365 or Excel 2024 or later.

- `Assumptions` holds the business name, forecast start (a Monday), as-at date, opening cash, minimum cash buffer, the governance fields and the `Base`, `Upside` or `Downside` scenario control.
- `13-Week Forecast` takes expected receipts and payments in the blue input cells, in whole AUD.
- `Weekly Review` keeps a values-only snapshot of the prior forecast beside the actuals and reports receipt, payment and closing-cash variances with owner commentary.
- `Dashboard` shows the 13-week cash and buffer view, first buffer breach, funding requirement, action deadline, the three-scenario comparison and forecast accuracy once actuals exist.
- `Checks & Sources` runs nine model checks (exactly 13 weeks, a Monday start, a valid scenario, the opening-cash tie, weekly roll-forwards, weekly totals, the Week 13 closing-cash equation and no formula errors) and keeps `MODEL STATUS` separate from `LIQUIDITY STATUS`.

The workbook opens with illustrative sample data so the views are visible at once; replace it before any business decision. The [template guide](../templates/README.md) covers the weekly workflow, scenario behaviour, limits and separate native Excel regression gate. The library's release gates cover `ozzit.xlsx`. This template is a planning model, not tax, BAS, payroll, superannuation, financial or legal advice.
