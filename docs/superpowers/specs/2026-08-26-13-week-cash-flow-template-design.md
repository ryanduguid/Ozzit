# Ozzit 13-Week Cash-Flow Template Design

## Purpose

Create a standalone, Ozzit-branded 13-week cash-flow workbook for Australian FP&A teams and small-business finance operators. It replaces the supplied two-sheet US template with a weekly liquidity model that is suitable for management review, simple to update and easy to audit.

The workbook will ship with illustrative Australian data. Every sample input will be clearly marked as illustrative and will use the same editable-cell treatment as the cells a user replaces.

## Design principles

- Use native Excel formulas only. No VBA, macros, add-ins, external workbook links, volatile formulas or embedded Ozzit named functions are required.
- Target Microsoft 365 and Excel 2024 or later, matching the current Ozzit workbook support statement.
- Keep assumptions, editable cash inputs, calculations, outputs and checks visibly distinct.
- Make every total and KPI traceable to labelled cells.
- Treat liquidity warnings as business alerts, not model-integrity failures.
- Keep the workbook compact enough to operate directly in Excel without a transaction-import subsystem.
- Use Australian English, AUD and current Australian cash-timing terminology.

## Workbook structure

The workbook contains six worksheets in this order.

### 1. Start Here

The opening sheet explains the purpose, intended user, update cycle and sign convention. It includes:

- Ozzit wordmark treatment and workbook title
- `ILLUSTRATIVE DATA` notice
- version, prepared date, currency, units and selected scenario
- a five-step weekly update process
- a colour legend for inputs, calculations, cross-sheet links, warnings and passed checks
- a concise disclaimer that the workbook supports cash planning and is not tax, payroll or legal advice

### 2. Dashboard

The management view shows:

- Week 13 closing cash
- lowest weekly closing cash
- minimum cash buffer
- minimum headroom or funding need
- number of weeks below the buffer
- 13-week net cash change
- a line chart of closing cash against the minimum buffer
- a column chart comparing weekly receipts and payments
- a 13-row liquidity outlook with week ending, closing cash, headroom and status

The dashboard contains no editable cash-flow inputs. All metrics, tables and chart helper ranges are formula-linked to the forecast and assumptions.

### 3. Assumptions

The assumptions sheet contains the only model-wide controls:

- business name
- forecast start date
- as-at date
- currency and units
- opening cash
- minimum cash buffer
- selected scenario, validated to `Base`, `Upside` or `Downside`
- receipt and variable-payment factors for each scenario

Default illustrative controls are:

| Scenario | Receipt factor | Variable-payment factor |
|---|---:|---:|
| Base | 100% | 100% |
| Upside | 108% | 98% |
| Downside | 85% | 105% |

The forecast start date must be a Monday. A visible check identifies any other weekday.

### 4. 13-Week Forecast

Columns B:N represent thirteen consecutive weeks. Column O contains the 13-week total for flow rows and the Week 13 value for balance rows.

The header derives each week start and week end from the forecast start date. It also labels each column `Actual` or `Forecast` from the as-at date without using `TODAY()`.

The model contains these cash-receipt rows:

- customer receipts, current
- customer receipts, overdue
- cash and EFTPOS sales
- GST refunds and ATO credits
- other operating receipts
- asset sale proceeds
- equity or owner funding
- loan proceeds
- scenario receipt adjustment
- total cash receipts

The model contains these cash-payment rows:

- suppliers and trade creditors
- net wages
- PAYG withholding
- superannuation, paid each pay cycle
- payroll tax and workers compensation
- rent and occupancy
- utilities and communications
- insurance
- software and subscriptions
- marketing and sales
- professional fees
- freight, vehicles and travel
- GST and BAS payments
- PAYG income-tax instalments
- FBT and other tax
- interest and bank fees
- loan principal
- capital expenditure
- dividends and distributions
- other operating payments
- scenario variable-payment adjustment
- total cash payments

The calculation block contains:

- opening cash
- net cash movement
- closing cash
- minimum cash buffer
- headroom or funding gap
- weekly liquidity status

Formula rules are:

- Week 1 opening cash links to the opening-cash assumption. Each later week links to the preceding closing cash.
- The scenario receipt adjustment applies only to customer receipts and cash/EFTPOS sales.
- The scenario variable-payment adjustment applies only to suppliers, marketing, freight/vehicles/travel and other operating payments.
- Total receipts and total payments sum every visible component, including the relevant scenario adjustment.
- Net cash movement equals total receipts less total payments.
- Closing cash equals opening cash plus net cash movement.
- Headroom equals closing cash less the minimum cash buffer.
- Status is `BELOW BUFFER` when headroom is negative, `WATCH` when closing cash is within 25% above the buffer, and `OK` otherwise.

Illustrative inputs will include ordinary weekly activity and recognisable timing events such as payroll, BAS, loan, capex and customer-receipt spikes. The Base scenario will produce at least one buffer breach without producing negative cash, so the warning controls and management view are visible on opening.

### 5. Weekly Review

This sheet supports forecast-accuracy review without storing transaction-level data. It has one row per week with:

- week start and week end
- forecast receipts and actual receipts
- receipt variance in dollars and percentage
- forecast payments and actual payments
- payment variance in dollars and percentage
- forecast closing cash and actual closing cash
- closing-cash variance
- owner commentary

Actuals and commentary are editable. Variance formulas remain blank until the corresponding actual is entered. Favourable and unfavourable formats are applied consistently and explicitly.

### 6. Checks & Sources

The top of the sheet shows two separate outcomes:

- `MODEL STATUS: PASS/FAIL` for structural and arithmetic integrity
- `LIQUIDITY STATUS: OK/WATCH/ACTION REQUIRED` for business risk

The integrity checks cover:

- exactly thirteen forecast weeks
- Monday forecast start
- valid scenario selection
- Week 1 opening cash linked to assumptions
- all weekly opening-to-closing roll-forwards
- receipt totals against visible components
- payment totals against visible components
- Week 13 closing cash against opening cash plus cumulative net movement
- formula-error count across the model ranges

Each check has `Check`, `Actual`, `Expected`, `Difference`, `Tolerance`, `Status`, `Where to fix` and `Notes` columns. Model status aggregates only these integrity checks.

The source log records official URLs, scope, as-at date and notes for:

- GST and BAS timing
- PAYG withholding
- the 12% superannuation guarantee rate applying from 1 July 2025, and Payday Super timing applying from 1 July 2026

Statutory dates remain user-entered cash-flow assumptions because reporting cycles, agent concessions and entity circumstances differ.

## Styling

The workbook follows the current Ozzit workbook palette and typography:

- brand purple `#5C2D91`
- near-black `#04001F`
- dark neutral `#2B2733`
- grey-violet `#4F485E`
- warm grey `#B1AFAD`
- pale lavender `#DED9E8`
- light lavender `#F3F1F6`
- secondary purple `#7A5AB5`
- warning red `#C00000`
- Aptos Display for titles and Aptos for body text

Gridlines are hidden on every sheet. Title bands and section rows use purple or near-black with restrained whitespace. Calculation areas do not use merged cells.

Editable values use blue text on a warm-grey or light-lavender fill. Formulas use black text. Cross-sheet links use green text. Warning states use red text and pale-red fill. Passing checks use dark green text and pale-green fill.

Financial values use whole AUD with zero displayed as a dash and negatives in red parentheses. Percentages use one decimal place. Dates use `dd mmm yyyy`. Important labels and chart axes include units.

Working sheets freeze the header rows and first label column. Widths and row heights are set explicitly and verified from renders rather than left to default sizing.

## Source workbook treatment

The supplied workbook is a reference for the 13-week concept only. Its formulas are reimplemented in the new structure. No Tradepoint CFOs branding, US contact details, US tax labels, `1099` terminology or source marketing copy will remain.

## GitHub integration

The completed workbook will be added as:

`templates/13-week-cash-flow-forecast.xlsx`

The repository will also receive:

- `templates/README.md` with purpose, Excel compatibility, update instructions and the illustrative-data warning
- one `templates/` row in the main repository-layout table

No build dependency is added to the repository. The workbook remains a normal `.xlsx` file and the existing Ozzit library artefacts are unchanged.

Changes will be committed on `feat/13-week-cash-flow-template`, pushed to `ryanduguid/Ozzit`, and submitted as a pull request to `main`.

## Verification

Completion requires all of the following:

1. The workbook imports and exports through the approved spreadsheet runtime.
2. Key ranges show the intended formulas and calculated values.
3. Formula-error scans find no `#REF!`, `#DIV/0!`, `#VALUE!`, `#NAME?` or `#N/A` in the completed workbook.
4. Independent arithmetic checks reproduce weekly totals, roll-forwards, KPI values, scenario adjustments and model status.
5. Every worksheet is rendered and visually inspected at normal scale.
6. Charts have correct categories, series, units and placement.
7. The workbook contains no macros, external workbook links or superseded source branding.
8. The repository's existing test suite passes after the new files are added.
9. The workbook copied to the repository and the user-facing output have identical SHA-256 hashes.
10. The GitHub pull request is open and its available checks are green before completion is reported.
