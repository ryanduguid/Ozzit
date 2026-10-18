# Between The Spreadsheets: coverage and companion calculators

Ozzit now includes thirteen standalone arithmetic workflows in
[`practice-calculators.xlsx`](../templates/practice-calculators.xlsx). The public
Between The Spreadsheets catalogue was inspected on 18 October 2026: 32 product
listings, including year variants and a bundle. The companion implements the
documented arithmetic described below. It does not establish parity with the
purchased products.

## What the public website reveals

The [home page](https://betweenthespreadsheets.com.au/) is a Shopify storefront.
It links three calculator collections, a two-page
[catalogue](https://betweenthespreadsheets.com.au/collections/all), product pages,
business resources, an About page, a blog, contact, customer login and a cart.
The product pages describe Excel workbooks delivered by email after payment.
They expose descriptions and preview media, rather than live calculation forms.

All 32 product descriptions were read successfully. The product JSON endpoint
returned HTTP 429 and was not used. The zero-priced income-definitions listing
also uses the cart; its public page exposed no direct workbook download. The
[business resources page](https://betweenthespreadsheets.com.au/pages/business-resources)
contains placeholder text and a link to another business's resources. That
external site was not treated as part of this catalogue.

Public descriptions identify features, capacities and some assumptions. They do
not reveal complete input schemas, workbook formulas, validation branches,
rounding, declarations, worked examples or protected calculation tabs. No
purchased workbook was available for comparison. The implementation uses
independently written formulas and fabricated inputs; vendor formulas, prose,
images and branding have not been incorporated into the companion.

## Using the companion

Open the companion in desktop Excel and replace the blue inputs on platinum fills.
The first tab links to its thirteen calculators. It uses ordinary formulas and requires
no LAMBDA import, macro, add-in or external workbook. The existing `ozzit.xlsx`
library remains the authority for its 137 LAMBDAs and five help tables.

The three additions from the wider comparison, Loan offers, Break even and Working
capital, are documented in [the improvement report](practice-improvements.md).

The companion follows Ozzit's existing scope: a reviewer supplies tax
treatment, assessed tax, applicable rates, eligibility and legal conclusions.
Example rates and capacities are labelled modelling assumptions. The public
product copy is evidence of vendor features, not authority for current law.
In particular, its dated FBT editions, Division 296 claims and rental budget
changes have not been adopted as statutory rules.

| Tab | Inputs and calculated behaviour | Limits |
| --- | --- | --- |
| Allocation | One employer budget; equal, salary or eight-factor weights across 20 people/jobs. Cash pool = budget / ((1 + super cost rate) × (1 + payroll cost rate)). Allocate by relative weight; show cash, super, payroll cost, total and net after supplied withholding. | Payroll cost is assumed to apply to cash plus super. Rates and withholding are supplied. Set both cost rates to zero for overhead allocation. Values remain unrounded; payroll rounding and any cent remainder need reconciliation. Unused methods do not require scores. |
| Contractor | Show employee salary plus employer costs separately from contractor target cash, retirement saving, overheads and insurance. Divide required annual revenue by billable weeks and hours. | Excludes GST and personal income tax. Does not determine employee/contractor status. |
| Vendor finance | Ten annual periods. Interest on opening debt; dividends less a supplied cash reserve plus own funds fund repayments. Cap payment at debt plus interest and show excess cash separately. | Year-end payments; annual simple interest; no tax assessment or dividend/franking calculations. |
| Mixed loan | Six opening purposes and 36 dated events. Allocate opening-balance simple interest by elapsed days and a supplied day basis. Add draws, apportion repayments to the resulting balances, allocate fees and show unapplied cash. | Draws precede repayments on a date. Interest and fees are not capitalised. It stops at the final event; enter a zero-cash cut-off event when needed. No deductible/private classification or lender-statement reconciliation. |
| Rental | Gross rent, supplied GST, 12 expense rows with capital carve-outs and operating GST credits, three loans and five owners. Separate operating result from cash before income tax using principal, GST cash settlements and the change in agent-held funds. | Cash-based example; no depreciation, accrual adjustments, tax loss quarantining or tax-return labels. Capital portions include their GST; operating GST credits relate only to the operating portion. Enter GST payments positively and refunds negatively. Missing cash-only inputs leave profit available and cash unavailable. |
| Packaging | Five benefits. Compare employee cash pay and employer cost before/after packaging using supplied tax, on-costs, pre-tax salary reductions and post-tax employee contributions. | No PAYG, FBT, super or novated-lease eligibility engine. Benefit costs use a consistent GST basis. Benefits' employer cost is not assumed to equal their value to the employee. |
| Amortisation | Eleven cost cohorts across 11 annual periods. Allocate cost less residual over a supplied whole-year life, with an optional supplied year accelerating the remaining depreciable amount. Show horizon allocation and closing book value. | No part-year allocation, tax classification, statutory useful lives or automatic borrowing-cost deduction. Starts must fall inside the displayed horizon. A residual remains on the books. |
| Timeline | Twelve date intervals, a reporting window and 24 monthly columns. Show inclusive days, window overlap and coloured occupied months. | Overlapping intervals are not combined. No PPR election, CGT, estate-return requirement or ASIC deadline is calculated. |
| Deed register | Twenty trusts with deed, signature, stamping, trustee, appointor, election, NSW amendment and evidence fields. Compare supplied vesting dates with the editable review date plus 60 calendar months. | A diary reminder only. Missing or past dates stay visible. It does not interpret a deed or decide amendment obligations. |
| Carry forward | Current capacity, use to date, a proposed addition, reviewer-supplied eligibility and five dated buckets. Apply current headroom first, then usable buckets oldest first; show capacity exceeded. | No statutory caps, balance threshold or deductibility decision. Enter remaining buckets after earlier consumption. Origin periods must increase and expiry periods must not decrease. Expired buckets have zero usable capacity. |

The row counts above are fixed. To extend them, copy the relevant formulas,
extend totals and validation ranges, and add native checks. The mixed-loan table
is wide because each event shows all six purposes' opening, interest, closing,
fee and repayment allocations. Frozen dates identify the event while scrolling.

## All 32 product listings

“Partial” means the companion provides the identified arithmetic, with the
remaining product behaviour explicitly absent. No row is a claim of full product
parity. “Gap” means no replacement for that product's main workflow was added.

| # | Public product and documented features | Ozzit coverage and remaining gap |
| --- | --- | --- |
| 1 | [Overheads, WIP and income in advance](https://betweenthespreadsheets.com.au/products/allocation-of-overheads-work-in-progress-and-income-in-advance): project overhead allocation and construction-contract analysis using actual and projected costs/revenue. | Partial: Allocation handles supplied overhead weights. Existing `oz.Corkscrewλ` supports roll-forwards. Construction WIP, revenue recognition and income-in-advance decisions remain a gap. |
| 2 | [AML/CTF designated-services checklist](https://betweenthespreadsheets.com.au/products/anti-money-laundering-and-counter-terrorism-financing-designated-services-checklist): 12 steps, service classifications, reliance checks, completion controls and a client email. | Gap: requires an independently sourced legal workflow and the actual question/branch specification. No AML classification or client email was copied. |
| 3 | [Private-use asset balancing adjustment](https://betweenthespreadsheets.com.au/products/balancing-adjustment-for-sale-of-assets-with-some-private-use-calculator): ten-year ownership, three logbooks and historical depreciation methods. | Gap: existing depreciation schedules are modelling primitives only. No tax balancing-adjustment, notional-depreciation or historical-method engine. |
| 4 | [Commercial motor vehicle](https://betweenthespreadsheets.com.au/products/commercial-motor-vehicle): seven-question vehicle classification, weights, payload, GST, depreciation, FBT, LCT and record requirements. | Gap: existing `oz.GSTAddλ`, `oz.GSTExtractλ` and depreciation functions can use decided inputs. No vehicle classification or tax-limit decisions. |
| 5 | [Contractor versus employee](https://betweenthespreadsheets.com.au/products/contractor-v-employee): equivalent contractor billings allowing for labour costs. | Partial: Contractor provides annual, weekly and hourly arithmetic. Exact vendor inclusions and rounding are unknown. |
| 6 | [Division 296](https://betweenthespreadsheets.com.au/products/division-296-calculator-updated): ten-year projections and comparisons of withdrawal and inside/outside-super scenarios. | Gap: no Division 296 law, tax or advice engine. Vendor claims about enactment were not independently adopted. |
| 7 | [Division 7A](https://betweenthespreadsheets.com.au/products/division-7a-analysis-tool): up to seven years, changing benchmarks, extra payments, dividend top-ups, deductibility and lender/recipient tax. | Gap: existing loan functions and Vendor finance provide general debt arithmetic, without minimum yearly repayments, Division 7A, dividend tax or deductibility decisions. |
| 8 | [Employee bonuses](https://betweenthespreadsheets.com.au/products/employee-bonus): equal, salary and eight-factor allocations for 20 employees; on-costs, withholding and a separate KPI bonus. | Partial: Allocation provides all three allocation methods and supplied-rate costs. Withholding is manual. The separate KPI award rule is unknown and remains a gap. |
| 9 | [Employee buy-in matrix](https://betweenthespreadsheets.com.au/products/employee-buy-in-matrix): vendor funding, interest, dividends, own funds and a tax cash reserve. | Partial: Vendor finance covers this disclosed cash roll-forward under its stated annual timing assumption. No comparison to the vendor workbook. |
| 10 | [Entertainment review](https://betweenthespreadsheets.com.au/products/entertainment-review): GST, deductibility, FBT, reimbursement, reportability, minor-benefit decisions and register checks. | Gap: no entertainment flowchart, exemptions, register-validity or tax decisions. |
| 11 | [Car FBT 2025](https://betweenthespreadsheets.com.au/products/fbt-for-cars-2025): ten employees, vehicle/acquisition variants, EVs, logbooks, reimbursements, gross-ups, comparisons and declarations. | Gap: no car-FBT or declaration replacement. Existing depreciation/GST primitives do not establish this coverage. |
| 12 | [Car FBT 2026](https://betweenthespreadsheets.com.au/products/fbt-for-cars-31-march-2026-calculator): the car suite plus revised rates and an operating-cost private-use option. | Gap: requires period-specific authoritative rules and a tested calculation specification. |
| 13 | [Income definitions, free](https://betweenthespreadsheets.com.au/products/income-calculator-tax-offsets-concessions): seven income definitions for concessions, offsets, surcharges and repayments. | Gap: the public page names the definitions without exposing their complete formulas; no direct workbook link was found. |
| 14 | [Intangibles](https://betweenthespreadsheets.com.au/products/intangibles): 11-year borrowing, start-up, black-hole and website-cost schedules, examples and a classification flowchart. | Partial: Amortisation supplies cost cohorts, accumulation and a supplied early write-off year. Tax classification, part-year rules, borrowing-cost payout treatment and website pools remain gaps. |
| 15 | [Unit trusts guide](https://betweenthespreadsheets.com.au/products/investing-in-unit-trusts-a-guide-and-outline-of-the-rules): educational structure/legal material and income-tax/cash-flow calculations with interest. | Gap: general debt and cash-flow primitives exist, but no unit-trust legal, investment or tax model was added. |
| 16 | [Lump sums in arrears 2025](https://betweenthespreadsheets.com.au/products/lump-sum-payments-in-arrears-tax-offset-2025): tax-offset calculation. | Gap: public inputs and full algorithm are unspecified. |
| 17 | [Lump sums in arrears 2026](https://betweenthespreadsheets.com.au/products/lump-sum-payments-in-arrears-tax-offset-2026): next-year tax-offset edition. | Gap: same evidence gap as the 2025 listing, with period-specific rules also required. |
| 18 | [Mixed-use loan](https://betweenthespreadsheets.com.au/products/mixed-use): six uses, three monthly redraws, charges between redraws, proportional repayments and adjustments. | Partial: Mixed loan supplies six uses and 36 arbitrary events under explicit timing/day-basis assumptions. No lender-specific charge reconciliation, manual adjustment row or tax classification. |
| 19 | [PSI flowchart and expense allocation](https://betweenthespreadsheets.com.au/products/personal-services-income-flowchart-and-allocation-of-income-calculator): PSI/PSB decisions, three income types and seven allocation approaches. | Partial arithmetic only: Allocation accepts decided weights. The seven vendor methods, PSI/PSB branches and accounting-to-tax bridge remain gaps. |
| 20 | [Professional profits allocation](https://betweenthespreadsheets.com.au/products/professional-profits-allocation-calculator-2024-and-beyond): PCG gateways, risk scoring, income tests, six adults/three minors and case studies. | Gap: no PCG assessment, tax-rate engine or professional profit-allocation decision. |
| 21 | [Rental property](https://betweenthespreadsheets.com.au/products/rental-property-summary): five owners, GST, agent-held funds, three loans, capital carve-outs and advertised future loss-quarantining rules. | Partial: Rental supplies the cash/operating bridge, capital carve-outs, loans and five-owner allocation. New-property tests, tax classifications and loss quarantining remain gaps. |
| 22 | [Salary packaging and novated leases](https://betweenthespreadsheets.com.au/products/salary-packaging-salary-sacrifice-2026): five benefits, employee net pay, employer on-costs and commencement/cessation reminders. | Partial: Packaging compares supplied amounts. Tax, super, workers compensation, payroll tax, FBT, lease eligibility and timing decisions remain manual. |
| 23 | [Share issue, transfer and reduction](https://betweenthespreadsheets.com.au/products/share-issue-transfer-reduction): corporate actions, agreement scenarios and prefilled ASIC lodgement dates. | Gap: Timeline can display supplied dates but does not reproduce the legal steps, agreement tests or deadlines. |
| 24 | [SMSF/LRBA setup](https://betweenthespreadsheets.com.au/products/smsf-how-lrbas-work-setup-checklist-for-an-asset-acquisition-borrowings): educational cash flows and a multi-adviser setup/document checklist. | Gap: existing amortisation functions support decided loans, but there is no SMSF setup, LRBA eligibility, advice or legal-document workflow. |
| 25 | [Concessional contribution catch-up](https://betweenthespreadsheets.com.au/products/superannuation-concessional-contribution-catchup-calculator): five-year capacity ageing, balance eligibility, proposed contributions and expiry. | Partial: Carry forward provides current-first/FIFO bucket arithmetic with supplied eligibility, capacities and expiries. It does not determine actual caps, eligibility or a tax deduction. |
| 26 | [Tax position 2026](https://betweenthespreadsheets.com.au/products/tax-position-calculator-2026): group planning, distribution/dividend choices, tax estimates, obligations, losses and instalments; some offsets are explicitly manual. | Gap: no group tax planner, optimised distributions, individual tax or instalment model. |
| 27 | [Tax position 2025](https://betweenthespreadsheets.com.au/products/tax-position-june-2025): the earlier tax-position edition. | Gap: no historical tax-position replacement. |
| 28 | [Termination pay](https://betweenthespreadsheets.com.au/products/termination-pay-entitlements-calculator): leave, loading, redundancy, invalidity, other ETPs, caps, super and reporting labels. | Gap: statutory and employment inputs, classifications, cap interactions and reporting rules need a separate specification. |
| 29 | [Full FBT 2026](https://betweenthespreadsheets.com.au/products/the-full-fbt-calculator-for-31-march-2026): cars/entertainment plus other benefit types, reportable amounts, payroll tax, contributions, declarations and completion controls. | Gap: no consolidated FBT return or benefit-type decision engine. The listing bundles other products, so it is not an additional independent implementation of each. |
| 30 | [Property, active-asset and estate timelines](https://betweenthespreadsheets.com.au/products/timeline-of-principal-places-of-residence-ppr-rentals-active-assets-farms-gst-cgt-and-estates-calculator): editable visual timelines. | Partial: Timeline supplies dated intervals, monthly bands and overlap counts. Tax choices and estate obligations remain outside it. |
| 31 | [Trade-in and replacement asset](https://betweenthespreadsheets.com.au/products/trade-in-sale-of-asset-and-replacement-asset): disposal/replacement GST and tax with caps, write-offs and depreciation choices. | Gap: existing GST and depreciation functions accept decided inputs; no tax-basis, pooling, limit or replacement-asset decision engine. |
| 32 | [Trust deed summary](https://betweenthespreadsheets.com.au/products/trusts-summary): deed evidence, parties, elections, a five-year vesting flag and NSW foreign-person amendment records. | Partial: Deed register supplies the records and calendar reminder. No deed interpretation or legal amendment test. |

## Completing the remaining products

Full product parity needs licensed, authorised reference workbooks or a complete
independent functional specification. Obtain sanitised inputs/outputs, question
branches, rounding rules, date boundaries, warnings, declarations and expected
failure behaviour for each relevant edition. Public marketing descriptions
cannot supply that evidence.

Any future tax decision module also needs an explicit scope change to Ozzit's
current guidance, current primary legislative/ATO/ASIC/AUSTRAC sources,
edition-specific rules and professional review. Keep assumptions and proposals
separate from enacted rules. Test rule boundaries and their interactions in
native Excel before calling a workflow a replacement. No source workbook was
purchased, account accessed, customer contacted or product published here.

## Verification and rebuilding

Run the native test after closing your own Excel sessions:

```powershell
powershell.exe -NoProfile -File tools/excel_practice_selftest.ps1
```

The test opens the companion read-only, fully recalculates its examples, checks
for error cells, macros and external workbook links, changes inputs for each
workflow, closes without saving and verifies an unchanged SHA-256. The retained
task evidence records the Excel build, assertion count and exact workbook hash.
These tests verify the stated modelling arithmetic, not the vendor's hidden
formulas or tax outcomes.

The optional authoring builder is `tools/build_practice_calculators.mjs`. It
requires the bundled `@oai/artifact-tool` runtime made available through normal
Node module resolution. Run it from the repository root to regenerate the
companion. `OZZIT_PREVIEW_DIR` selects an optional preview directory. This runtime
is an authoring tool only; consumers need just the saved `.xlsx`. Follow the
[native preparation order](practice-improvements.md#native-preparation-and-checks)
to add links, recalculate, sanitise and verify the final bytes after rebuilding.
