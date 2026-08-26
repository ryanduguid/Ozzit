# Ozzit 13-Week Cash-Flow Forecast Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a polished, auditable 13-week cash-flow workbook for Australian FP&A teams, using Ozzit's visual language and native Excel formulas, then publish it through a feature-branch pull request.

**Architecture:** A single uncommitted `@oai/artifact-tool` generator produces the standalone workbook and its identical repository copy. One committed Python `unittest` contract reads the `.xlsx` package with the standard library so CI can verify structure, formulas, safety and Australian content without adding a build dependency. The workbook separates assumptions, editable cash inputs, calculated cash position, review commentary, management reporting and model controls across six worksheets.

**Tech Stack:** Microsoft Excel `.xlsx`; native Excel formulas compatible with Microsoft 365 and Excel 2024; `@oai/artifact-tool` for workbook generation, inspection and rendering; Python standard library for committed package-contract tests; Ozzit's existing Python `unittest` suite; GitHub CLI for publication.

**Spec:** `docs/superpowers/specs/2026-08-26-13-week-cash-flow-template-design.md`

## Global Constraints

- Use Australian English, AUD, day-first dates and the Ozzit palette defined in the approved specification.
- Keep the workbook standalone: no VBA, Office Scripts, add-ins, external workbook links, volatile functions or Ozzit named-function dependencies.
- Use `@oai/artifact-tool` for all workbook authoring. Do not use `openpyxl`, `xlsxwriter`, `pandas` or direct ZIP/XML mutation to create or edit the workbook.
- Keep one executable generator at `C:/Users/-/Documents/Codex/2026-08-26/i-wa/work/ozzit-cashflow-build/build_cash_flow_template.mjs`. Do not commit it or its dependencies.
- Run the spreadsheet operation marker exactly once immediately before the first authoring command.
- Keep every editable user input visibly distinct from formulas and cross-sheet links. Hide gridlines on every worksheet and avoid merged cells in calculation areas.
- Preserve unrelated repository content. Do not change `ozzit.xlsx`, `src/` or existing modelling functions.
- Use test-first cycles: extend the contract, observe its expected failure against the current artefact, then make the smallest generator change that passes it.
- Save the final deliverable to `C:/Users/-/Documents/Codex/2026-08-26/i-wa/outputs/01a03dff-a375-7fa3-afa2-16f32928d9d8/Ozzit-13-Week-Cash-Flow-Forecast.xlsx` and copy identical bytes to `templates/13-week-cash-flow-forecast.xlsx`.

## Workbook Contract

Use these worksheet names and order:

1. `Start Here`
2. `Dashboard`
3. `Assumptions`
4. `13-Week Forecast`
5. `Weekly Review`
6. `Checks & Sources`

The forecast uses columns `B:N` for the 13 weeks and column `O` for the total or terminal value. Use the following row map exactly so formulas, checks and documentation stay auditable:

```text
5  Week commencing        22 Suppliers and inventory
6  Week ending            23 Net wages
7  Actual / Forecast      24 PAYG withholding
9  CASH RECEIPTS          25 Superannuation
10 Customer receipts      26 Payroll tax / workers compensation
11 Overdue receipts       27 Rent
12 Cash / EFTPOS sales    28 Utilities
13 GST refunds / credits  29 Insurance
14 Other operating        30 Software and subscriptions
15 Asset sale proceeds    31 Marketing
16 Equity / owner funding 32 Professional services
17 Loan proceeds          33 Freight, vehicles and travel
18 Scenario receipt adj.  34 GST / BAS payments
19 Total cash receipts    35 PAYG income tax instalments
21 CASH PAYMENTS          36 FBT / other tax
37 Interest and bank fees 38 Loan principal
39 Capital expenditure    40 Dividends / distributions
41 Other operating        42 Scenario payment adjustment
43 Total cash payments    45 CASH POSITION
46 Opening cash balance   47 Net cash movement
48 Closing cash balance   49 Minimum cash buffer
50 Headroom / (gap)       51 Liquidity status
```

The core formulas in week 1 must follow this contract and fill right through week 13:

```excel
B18 = IF(B$7="Actual",0,SUM(B10:B12)*(INDEX(Assumptions!$C$15:$C$17,MATCH(Assumptions!$B$12,Assumptions!$B$15:$B$17,0))-1))
B19 = SUM(B10:B18)
B42 = IF(B$7="Actual",0,SUM(B22,B31,B33,B41)*(INDEX(Assumptions!$D$15:$D$17,MATCH(Assumptions!$B$12,Assumptions!$B$15:$B$17,0))-1))
B43 = SUM(B22:B42)
B46 = Assumptions!$B$10
C46 = B48
B47 = B19-B43
B48 = B46+B47
B49 = Assumptions!$B$11
B50 = B48-B49
B51 = IF(B48<B49,"BELOW BUFFER",IF(B48<=B49*1.25,"WATCH","OK"))
```

Set the illustrative assumptions to a Monday start of 31 August 2026, an as-at date of 6 September 2026, opening cash of AUD 400,000, a minimum buffer of AUD 100,000 and selected scenario `Base`. The scenario table is Base `100% / 100%`, Upside `108% / 98%`, Downside `85% / 105%` for receipts and selected variable payments respectively.

Use these AUD thousands input arrays for rows `10:17` and `22:41`, multiplying by 1,000 in the workbook. Unlisted rows are zero.

```text
10 current receipts: 118,125,120,115,110,112,105,108,110,116,120,125,130
11 overdue receipts: 30,20,15,10,8,8,5,5,7,10,10,12,12
12 cash/EFTPOS: 18,20,19,18,18,17,17,18,18,19,20,21,22
14 other operating: 4,4,4,4,4,4,4,4,4,4,4,4,4
15 asset sale: 0,0,0,0,0,0,0,0,0,100,0,0,0
16 owner funding: 0,0,0,0,0,0,150,0,0,0,0,0,0
17 loan proceeds: 0,0,0,0,0,0,0,0,125,0,0,0,0
22 suppliers: 82,88,90,95,98,100,105,108,110,105,100,95,90
23 net wages: 42,42,42,42,42,42,42,42,42,42,42,42,42
24 PAYG withholding: 12,12,12,12,12,12,12,12,12,12,12,12,12
25 superannuation: 6,6,6,6,6,6,6,6,6,6,6,6,6
26 payroll tax / workers compensation: 5,0,0,0,5,0,0,0,5,0,0,0,0
27 rent: 25,0,0,0,25,0,0,0,25,0,0,0,25
28 utilities: 0,0,0,0,4,0,0,0,4,0,0,0,4
29 insurance: 0,6,0,0,0,0,0,0,0,0,0,0,0
30 software: 3,3,3,3,3,3,3,3,3,3,3,3,3
31 marketing: 6,6,6,6,6,6,6,6,6,6,6,6,6
32 professional services: 0,0,8,0,0,0,8,0,0,0,8,0,0
33 freight / vehicles / travel: 10,10,10,10,10,10,10,10,10,10,10,10,10
34 GST / BAS: 0,0,0,0,0,0,0,0,45,0,0,0,0
35 PAYG income tax instalment: 0,0,0,0,0,0,0,0,18,0,0,0,0
37 interest and bank fees: 4,0,0,0,4,0,0,0,4,0,0,0,4
38 loan principal: 12,0,0,0,12,0,0,0,12,0,0,0,12
39 capital expenditure: 0,0,0,0,0,25,0,0,0,40,0,0,0
41 other operating: 5,5,5,5,5,5,5,5,5,5,5,5,5
```

At Base, the independently expected closing cash series is AUD `358k, 349k, 325k, 293k, 201k, 133k, 217k, 160k, 152k, 172k, 134k, 117k, 66k`. Week 13 must be below the AUD 100k buffer without cash becoming negative.

## Task 1: Contract-first workbook scaffold and controls

**Files:**

- Create: `tools/tests/test_cash_flow_template.py`
- Create: `templates/13-week-cash-flow-forecast.xlsx`
- Create, uncommitted: `C:/Users/-/Documents/Codex/2026-08-26/i-wa/work/ozzit-cashflow-build/build_cash_flow_template.mjs`
- Create, uncommitted: `C:/Users/-/Documents/Codex/2026-08-26/i-wa/work/ozzit-cashflow-build/verify_cash_flow_template.py`

- [ ] Write a standard-library package test that fails because the workbook does not yet exist. Name the production change that will make the test pass.
- [ ] Assert the six-sheet order, 13 weekly columns, required row labels, no VBA or external-link package parts, no inherited `Tradepoint`, `1099`, `federal tax`, `state tax` or source-template contact text, and the presence of the Ozzit palette anchors.
- [ ] Run `python -m unittest tools.tests.test_cash_flow_template -v` and record the expected missing-workbook failure.
- [ ] Run the required spreadsheet operation marker once, then create the generator and use `@oai/artifact-tool` to build the six-sheet scaffold, named controls, validation lists, Ozzit styling, print settings and fixed illustrative assumptions.
- [ ] Make `Start Here` usable without training: purpose, five-step workflow, legend, version/date, scope limits and links to each working sheet.
- [ ] Make `Assumptions` the single control panel. Include scenario validation, data-quality notes and clear editable/formula styles.
- [ ] Export the workbook to both required locations and confirm identical SHA-256 hashes.
- [ ] Re-run the focused test and commit the passing scaffold, test and approved specification/plan changes with message `feat: scaffold Australian cash flow template`.

## Task 2: Forecast engine and weekly review

**Files:**

- Modify: `tools/tests/test_cash_flow_template.py`
- Modify: `templates/13-week-cash-flow-forecast.xlsx`
- Modify, uncommitted: `C:/Users/-/Documents/Codex/2026-08-26/i-wa/work/ozzit-cashflow-build/build_cash_flow_template.mjs`
- Modify, uncommitted: `C:/Users/-/Documents/Codex/2026-08-26/i-wa/work/ozzit-cashflow-build/verify_cash_flow_template.py`

- [ ] Extend the test for the row map, scenario-factor formulas, cash roll-forward formulas, data-validation range and the exact illustrative Base closing-cash series. Observe the expected failure before editing the generator.
- [ ] Build the `13-Week Forecast` sheet from the contract above. Input rows remain editable; scenario rows, totals and cash-position rows remain formula-driven.
- [ ] Apply whole-dollar AUD formats with dashes for zero and red parentheses for negatives, day-first dates, one-decimal percentages and input/formula/cross-sheet colour rules.
- [ ] Add conditional formatting for `BELOW BUFFER`, `WATCH` and `OK`; freeze the label and time headers; add explanatory comments to scenario-adjustment and statutory-payment rows.
- [ ] Build `Weekly Review` as 13 rows linked to the forecast. Include week ending, forecast closing cash, actual closing cash input, variance, rolling forecast note, owner, action and status. Keep variance blank until actual cash is entered.
- [ ] Recalculate and inspect the forecast values, then commit with message `feat: add 13-week forecast engine`.

## Task 3: Dashboard, checks and sources

**Files:**

- Modify: `tools/tests/test_cash_flow_template.py`
- Modify: `templates/13-week-cash-flow-forecast.xlsx`
- Modify, uncommitted: `C:/Users/-/Documents/Codex/2026-08-26/i-wa/work/ozzit-cashflow-build/build_cash_flow_template.mjs`
- Modify, uncommitted: `C:/Users/-/Documents/Codex/2026-08-26/i-wa/work/ozzit-cashflow-build/verify_cash_flow_template.py`

- [ ] Extend the test for dashboard KPI formulas, two chart relationships, nine control checks, separate `MODEL STATUS` and `LIQUIDITY STATUS` outputs, and official-source URLs. Observe the expected failure.
- [ ] Build management KPI cards for selected scenario, opening cash, lowest closing cash, minimum headroom, weeks below buffer and week of lowest cash.
- [ ] Add a line chart for closing cash versus the minimum buffer and a clustered column chart for receipts versus payments. Use readable axes, concise titles and source ranges that cover all 13 weeks.
- [ ] Build nine integrity checks: Monday start, as-at date in range, recognised scenario, opening cash tie, opening-balance continuity, receipt totals, payment totals, closing-cash equation and Weekly Review linkage.
- [ ] Keep model integrity separate from the commercial liquidity warning. Use `PASS`/`FAIL` for model status and `ACTION REQUIRED`/`WATCH`/`HEALTHY` for liquidity.
- [ ] Add a source log for GST, BAS due dates, super guarantee and Payday Super using official ATO or Treasury URLs, checked 26 August 2026. State that entity-specific lodgement dates and tax classifications must be confirmed by the user or adviser.
- [ ] Re-run the focused test and commit with message `feat: add cash flow dashboard and controls`.

## Task 4: Repository documentation and user hand-off

**Files:**

- Create: `templates/README.md`
- Modify: `README.md`
- Modify: `tools/tests/test_cash_flow_template.py`

- [ ] Extend the test to require a repository-layout entry for `templates/` and a template README that names the workbook, Excel baseline, illustrative-data warning, workflow and limitation notes. Observe the expected failure.
- [ ] Add a concise `templates/README.md` explaining who the workbook is for, how to replace the sample data, how scenarios work, where checks live and that it supports planning rather than tax advice.
- [ ] Add one `templates/` row to the main repository layout table and a short link from Getting started without changing Ozzit's existing function-library instructions.
- [ ] Run the focused test and complete repository suite, then commit with message `docs: add cash flow template guide`.

## Task 5: Independent verification, visual review and publication

**Files:**

- Modify as defects require: `templates/13-week-cash-flow-forecast.xlsx`
- Modify as defects require: `tools/tests/test_cash_flow_template.py`
- Modify as defects require: `templates/README.md`
- Modify as defects require: `README.md`
- Produce: `C:/Users/-/Documents/Codex/2026-08-26/i-wa/outputs/01a03dff-a375-7fa3-afa2-16f32928d9d8/Ozzit-13-Week-Cash-Flow-Forecast.xlsx`

- [ ] Run all automated gates in `C:/Users/-/Documents/Codex/2026-08-26/i-wa/GATES.md` and capture fresh evidence.
- [ ] Import the final workbook with `@oai/artifact-tool`; inspect the key ranges and scan formulas for `#REF!`, `#DIV/0!`, `#VALUE!`, `#NAME?` and `#N/A`.
- [ ] Render every worksheet and both dashboard charts. Inspect at readable scale and correct any clipping, overlap, weak contrast, inconsistent borders, excessive whitespace or poor print pagination.
- [ ] Check the ZIP package independently for macros, external links, executable relationships, stale source-template names and invalid worksheet references.
- [ ] Confirm the Base sample series independently, switch to Upside and Downside in a temporary inspection copy, and verify scenario direction without changing the shipped Base setting.
- [ ] Confirm the repository and output copies have the same SHA-256 hash and that `git diff --check` is clean.
- [ ] Run `python -m unittest discover -s tools/tests -v` and every command from `.github/workflows/verify.yml`.
- [ ] Request broad final review. Resolve every blocking or important finding and rerun the affected checks.
- [ ] Push `feat/13-week-cash-flow-template`, open a pull request to `main`, wait for required checks, and record the URL and successful check result in the gate evidence.

## Plan Self-Review

- [x] Every approved worksheet, formula family, Australian payment category, scenario rule, style rule, source note and GitHub deliverable is assigned to a task.
- [x] The workbook formula contract and illustrative Base cash series are explicit and independently checkable.
- [x] The implementation keeps one uncommitted generator and adds no authoring dependency to the repository.
- [x] No open marker, placeholder path, unresolved product decision or speculative feature remains.
- [x] Every implementation task includes a failing-test observation, a passing check and a precise commit boundary.
