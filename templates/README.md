# Ozzit 13-week cash-flow forecast

`13-week-cash-flow-forecast.xlsx` is a standalone weekly liquidity model for Australian FP&A practitioners, finance operators and small-business owners. It uses native Excel formulas and is intended for Microsoft 365 or Excel 2024 and later.

The workbook opens with illustrative data from an Australian business so that the model, warnings and review views are visible immediately. Replace the sample values before using it for a business decision. Editable cells are marked as inputs in the workbook. Do not overwrite formula or cross-sheet-link cells.

## Weekly workflow

1. **Replace the sample data.** On `13-Week Forecast`, replace the blue input cells with the business's expected receipts and payments. Use whole AUD unless the assumptions say otherwise.
2. **Set the assumptions, governance and scenario.** On `Assumptions`, enter the business name, forecast start date, as-at date, opening cash and minimum cash buffer. Enter the liquidity action lead time, prepared by, reviewed by, forecast owner and next review date, then select `Base`, `Upside` or `Downside` in the scenario control.
3. **Preserve the weekly forecast, then update.** Before refreshing the live forecast, paste the original period dates, forecast receipts, forecast payments and forecast closing cash into the blue snapshot cells on `Weekly Review` as values. Snapshot cells must not link to the live forecast. Then refresh the forecast inputs and enter available actual receipts, actual payments and actual closing cash. Record owner commentary for each reported week. The row-level review status remains incomplete until all three actuals and commentary are present.
4. **Review the outputs.** Use `Dashboard` for the 13-week cash and buffer view, first buffer breach, funding requirement, action deadline and three-scenario comparison. Its forecast-accuracy block reports weeks actualised, cumulative receipt and payment variance percentages, average closing-cash bias and review completeness. Accuracy measures remain blank until actuals are entered. Then use `Weekly Review` to compare receipt, payment and closing-cash variances and keep the source or rationale for material changes with the working papers.
5. **Resolve checks, then archive or roll forward.** Before sharing the forecast, review `Checks & Sources`, fix any failed model check and record decisions. Save the reviewed file as the period's archive, then copy it for the next cycle and update the dates, actuals and forecast inputs.

## Scenario behaviour

Scenario factors apply only to weeks labelled `Forecast`. Weeks labelled `Actual` remain unchanged. The selected scenario is set in `Assumptions`. `Base` uses 100% receipts and 100% selected variable payments. `Upside` uses 108% receipts and 98% selected variable payments. `Downside` uses 85% receipts and 105% selected variable payments.

The scenario receipt adjustment is calculated from customer receipts, overdue receipts and cash / EFTPOS sales. It does not change GST refunds or credits, other operating receipts, asset sale proceeds, equity or owner funding, or loan proceeds. The scenario variable-payment adjustment is calculated from suppliers and inventory, marketing, freight / vehicles / travel and other operating payments. It does not change wages, PAYG withholding, superannuation, payroll tax, rent, utilities, insurance, software, professional services, GST / BAS payments, income-tax instalments, FBT, interest, loan principal, capital expenditure or dividends.

## Checks and limitations

`Checks & Sources` contains separate `MODEL STATUS` and `LIQUIDITY STATUS` results. Its nine model checks confirm exactly 13 weeks, a Monday start, a valid scenario, the opening-cash tie, weekly roll-forwards, weekly receipt and payment totals, the Week 13 closing-cash equation and the absence of formula errors. A liquidity warning is a business alert and is not a model-integrity failure. The same sheet records the official sources and the scope notes used when the workbook was prepared.

This is an illustrative FP&A cash-planning model. It is not tax advice, and it is not BAS, payroll, superannuation, financial or legal advice. Statutory amounts, payment timing and dates are planning assumptions confirmed by the user or adviser. The workbook does not determine tax classifications, entity-specific lodgement dates or payroll treatment, and it does not import transaction-level actuals. Confirm the completed forecast with the responsible finance professional before relying on it.
