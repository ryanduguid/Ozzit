# Between The Spreadsheets: coverage and companion calculators

Ozzit now includes twenty-one standalone arithmetic workflows in
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
The first tab links to its twenty-one calculators. It uses ordinary formulas and requires
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
| Mixed loan | Six opening purposes and 36 dated events. Allocate opening-balance simple interest by elapsed days and a supplied day basis. Add draws and signed purpose adjustments, apportion repayments to the resulting balances, allocate fees and show unapplied cash. | Draws and adjustments precede repayments on a date. Interest and fees are not capitalised. It stops at the final event; enter a zero-cash cut-off event when needed. No deductible/private classification or lender-statement reconciliation. |
| Rental | Gross rent, supplied GST, 12 expense rows with capital carve-outs and operating GST credits, three loans and five owners. Separate operating result from cash before income tax using principal, GST cash settlements and the change in agent-held funds. | Cash-based example; no depreciation, accrual adjustments, tax loss quarantining or tax-return labels. Capital portions include their GST; operating GST credits relate only to the operating portion. Enter GST payments positively and refunds negatively. Missing cash-only inputs leave profit available and cash unavailable. |
| Packaging | Five benefits. Compare employee cash pay and employer cost before/after packaging using supplied tax, on-costs, pre-tax salary reductions and post-tax employee contributions. Track supplied start/cease dates. | No PAYG, FBT, super or novated-lease eligibility engine. Benefit costs use a consistent GST basis. Benefits' employer cost is not assumed to equal their value to the employee. |
| Amortisation | Eleven cost cohorts across 11 annual periods. Allocate cost less residual over a supplied life and first-period fraction, with an optional supplied year accelerating the remaining depreciable amount. Show horizon allocation and closing book value. | The first-period fraction is supplied, not selected from tax rules. No tax classification, statutory useful lives or automatic borrowing-cost deduction. Starts must fall inside the displayed horizon. A residual remains on the books. |
| Timeline | Twelve date intervals, a reporting window and 24 monthly columns. Show inclusive days, window overlap and coloured occupied months. | Overlapping intervals are not combined. No PPR election, CGT, estate-return requirement or ASIC deadline is calculated. |
| Deed register | Twenty trusts with deed, signature, stamping, trustee, appointor, election, NSW amendment and evidence fields. Compare supplied vesting dates with the editable review date plus 60 calendar months. | A diary reminder only. Missing or past dates stay visible. It does not interpret a deed or decide amendment obligations. |
| Carry forward | Current capacity, use to date, a proposed addition, reviewer-supplied eligibility and five dated buckets. Apply current headroom first, then usable buckets oldest first; show capacity exceeded. | No statutory caps, balance threshold or deductibility decision. Enter remaining buckets after earlier consumption. Origin periods must increase and expiry periods must not decrease. Expired buckets have zero usable capacity. |
| Construction WIP | Twelve projects with contract value, actual/remaining costs, approved progress, prior modelled revenue, billings and collections. Show forecast profit, current revenue, unbilled revenue, advance billings and debtors. | Approved progress is an input, not a cost-to-cost or tax decision. Costs include supplied overheads. Revised progress may produce negative current revenue. |
| KPI awards | Twenty individual target bonuses with achievement, threshold, paid-score cap, withholding and supplied on-cost rates. | The independent rule pays target × capped score at/above the threshold. It does not redistribute a fixed pool or infer payroll withholding. |
| Asset disposal | Twelve supplied book-value, depreciation, sale, disposal-cost, replacement and GST records. Reconcile book profit and cash before/after supplied GST settlement. | No tax balancing adjustment, private-use decision or GST classification. The after-GST cash assumes all entered GST is settled in the modelling period. |
| Exit pay | Ten approved leave/loading, other leave, notice, redundancy/other gross amounts, withholding and super inputs. | Arithmetic only. No statutory eligibility, cap interaction, tax label or super-applicability decision. |
| Tax cash | Twelve assessed liability accounts with opening balances, signed expenses/adjustments, payments, refunds and noncash credits. | Payable and refund balances are shown separately. No cross-entity legal offset, tax-rate engine or distribution optimiser. |
| Investment | Ten annual value, distribution, capital-flow, debt, interest and assessed-tax periods. Show cash plus ending equity return. | Opening-balance interest and year-end capital/debt cash flows. Supplied tax may be a refund. Returns are undiscounted, not annualised. |
| Action register | Twenty reviewer-selected actions, owners, dates, completion evidence and reminder states. | All legal questions and deadlines are supplied. Recorded complete needs an owner, completion date and evidence, and is an administrative status only. |
| Benefit totals | Twenty final type 1/type 2 taxable-value records, supplied gross-up multipliers and an FBT rate. | Basic business/government gross-up arithmetic only. No employer eligibility, taxable-value classification, exemption, rebate or reportability decisions. No whole-dollar return rounding. |

The row counts above are fixed. To extend them, copy the relevant formulas,
extend totals and validation ranges, and add native checks. The mixed-loan table
is wide because each event shows all six purposes' opening, interest, closing,
fee and repayment allocations plus signed adjustments. Frozen dates identify the event while scrolling.

## All 32 product listings

“Partial” means the companion provides the identified arithmetic, with the
remaining product behaviour explicitly absent. No row is a claim of full product
parity. “Gap” means no replacement for that product's main workflow was added.

| # | Public product and documented features | Ozzit coverage and remaining gap |
| --- | --- | --- |
| 1 | [Overheads, WIP and income in advance](https://betweenthespreadsheets.com.au/products/allocation-of-overheads-work-in-progress-and-income-in-advance): project overhead allocation and construction-contract analysis using actual and projected costs/revenue. | Partial: Allocation handles overhead weights. Construction WIP reconciles reviewer-approved progress, modelled revenue, forecast costs, billings and collections. No TR 2018/3 treatment or revenue-recognition decision is made. |
| 2 | [AML/CTF designated-services checklist](https://betweenthespreadsheets.com.au/products/anti-money-laundering-and-counter-terrorism-financing-designated-services-checklist): 12 steps, service classifications, reliance checks, completion controls and a client email. | Gap: no designated-service or legal-branch replacement. Action register can record reviewer-selected steps, owners, evidence and supplied deadlines without deciding AML obligations. |
| 3 | [Private-use asset balancing adjustment](https://betweenthespreadsheets.com.au/products/balancing-adjustment-for-sale-of-assets-with-some-private-use-calculator): ten-year ownership, three logbooks and historical depreciation methods. | Gap: no private-use tax balancing adjustment, notional depreciation or historical-method engine. Asset disposal provides the separate supplied-book-value and cash arithmetic. |
| 4 | [Commercial motor vehicle](https://betweenthespreadsheets.com.au/products/commercial-motor-vehicle): seven-question vehicle classification, weights, payload, GST, depreciation, FBT, LCT and record requirements. | Gap: no vehicle classification or tax-limit decisions. Existing GST/depreciation primitives and Asset disposal accept reviewer-decided amounts. |
| 5 | [Contractor versus employee](https://betweenthespreadsheets.com.au/products/contractor-v-employee): equivalent contractor billings allowing for labour costs. | Partial: Contractor provides annual, weekly and hourly arithmetic. Exact vendor inclusions and rounding are unknown. |
| 6 | [Division 296](https://betweenthespreadsheets.com.au/products/division-296-calculator-updated): ten-year projections and comparisons of withdrawal and inside/outside-super scenarios. | Gap: no Division 296 law, tax or advice engine. Investment supplies a generic ten-period value/debt/cash projection using assessed tax inputs, without a super comparison or legal rules. |
| 7 | [Division 7A](https://betweenthespreadsheets.com.au/products/division-7a-analysis-tool): up to seven years, changing benchmarks, extra payments, dividend top-ups, deductibility and lender/recipient tax. | Gap: existing loan functions and Vendor finance provide general debt arithmetic, without minimum yearly repayments, Division 7A, dividend tax or deductibility decisions. |
| 8 | [Employee bonuses](https://betweenthespreadsheets.com.au/products/employee-bonus): equal, salary and eight-factor allocations for 20 employees; on-costs, withholding and a separate KPI bonus. | Partial: Allocation provides the three pool methods and supplied-rate costs. KPI awards adds an independent individual target/threshold/cap rule. PAYG and the private vendor award rule remain unsupported. |
| 9 | [Employee buy-in matrix](https://betweenthespreadsheets.com.au/products/employee-buy-in-matrix): vendor funding, interest, dividends, own funds and a tax cash reserve. | Partial: Vendor finance covers this disclosed cash roll-forward under its stated annual timing assumption. No comparison to the vendor workbook. |
| 10 | [Entertainment review](https://betweenthespreadsheets.com.au/products/entertainment-review): GST, deductibility, FBT, reimbursement, reportability, minor-benefit decisions and register checks. | Gap: no entertainment flowchart, exemptions or register-validity decisions. Benefit totals can gross up final reviewer-classified taxable values. |
| 11 | [Car FBT 2025](https://betweenthespreadsheets.com.au/products/fbt-for-cars-2025): ten employees, vehicle/acquisition variants, EVs, logbooks, reimbursements, gross-ups, comparisons and declarations. | Partial arithmetic only: Benefit totals grosses up final type 1/type 2 taxable values. Car valuation methods, EV exemptions, logbooks, reimbursements, reportability and declarations remain gaps. |
| 12 | [Car FBT 2026](https://betweenthespreadsheets.com.au/products/fbt-for-cars-31-march-2026-calculator): the car suite plus revised rates and an operating-cost private-use option. | Partial arithmetic only: Benefit totals accepts final taxable values and supplied period rates. It does not select car methods, calculate private use or determine the 2026 rules. |
| 13 | [Income definitions, free](https://betweenthespreadsheets.com.au/products/income-calculator-tax-offsets-concessions): seven income definitions for concessions, offsets, surcharges and repayments. | Gap: the public page names the definitions without exposing their complete formulas; no direct workbook link was found. |
| 14 | [Intangibles](https://betweenthespreadsheets.com.au/products/intangibles): 11-year borrowing, start-up, black-hole and website-cost schedules, examples and a classification flowchart. | Partial: Amortisation supports cohorts, supplied first-period fractions, residuals and early write-offs. It carries partial recovery into a final period. Tax classification, statutory day-count rules, borrowing-cost payout treatment and website pools remain gaps. |
| 15 | [Unit trusts guide](https://betweenthespreadsheets.com.au/products/investing-in-unit-trusts-a-guide-and-outline-of-the-rules): educational structure/legal material and income-tax/cash-flow calculations with interest. | Partial arithmetic only: Investment projects value, distributions, contributions, withdrawals, debt, interest and after-tax cash using supplied assessed tax. Unit-trust legal structure, entity tax and advice remain gaps. |
| 16 | [Lump sums in arrears 2025](https://betweenthespreadsheets.com.au/products/lump-sum-payments-in-arrears-tax-offset-2025): tax-offset calculation. | Gap: public inputs and full algorithm are unspecified. |
| 17 | [Lump sums in arrears 2026](https://betweenthespreadsheets.com.au/products/lump-sum-payments-in-arrears-tax-offset-2026): next-year tax-offset edition. | Gap: same evidence gap as the 2025 listing, with period-specific rules also required. |
| 18 | [Mixed-use loan](https://betweenthespreadsheets.com.au/products/mixed-use): six uses, three monthly redraws, charges between redraws, proportional repayments and adjustments. | Partial: Mixed loan now includes six signed principal adjustments at every event, after interest and before proportional repayments. It does not capitalise interest/fees, reconcile a lender statement or classify tax uses. |
| 19 | [PSI flowchart and expense allocation](https://betweenthespreadsheets.com.au/products/personal-services-income-flowchart-and-allocation-of-income-calculator): PSI/PSB decisions, three income types and seven allocation approaches. | Partial arithmetic only: Allocation accepts decided weights. The seven vendor methods, PSI/PSB branches and accounting-to-tax bridge remain gaps. |
| 20 | [Professional profits allocation](https://betweenthespreadsheets.com.au/products/professional-profits-allocation-calculator-2024-and-beyond): PCG gateways, risk scoring, income tests, six adults/three minors and case studies. | Gap: no PCG assessment, tax-rate engine or professional profit-allocation decision. |
| 21 | [Rental property](https://betweenthespreadsheets.com.au/products/rental-property-summary): five owners, GST, agent-held funds, three loans, capital carve-outs and advertised future loss-quarantining rules. | Partial: Rental supplies the cash/operating bridge, capital carve-outs, loans and five-owner allocation. New-property tests, tax classifications and loss quarantining remain gaps. |
| 22 | [Salary packaging and novated leases](https://betweenthespreadsheets.com.au/products/salary-packaging-salary-sacrifice-2026): five benefits, employee net pay, employer on-costs and commencement/cessation reminders. | Partial: Packaging compares supplied costs and provides start/cease reminders with date validation. Annual costs are not automatically prorated. Tax, super, payroll, FBT and lease-eligibility decisions remain manual. |
| 23 | [Share issue, transfer and reduction](https://betweenthespreadsheets.com.au/products/share-issue-transfer-reduction): corporate actions, agreement scenarios and prefilled ASIC lodgement dates. | Gap: no legal-step, agreement-test or automatic ASIC-deadline replacement. Action register can track reviewer-selected corporate actions and supplied dates/evidence. |
| 24 | [SMSF/LRBA setup](https://betweenthespreadsheets.com.au/products/smsf-how-lrbas-work-setup-checklist-for-an-asset-acquisition-borrowings): educational cash flows and a multi-adviser setup/document checklist. | Gap: no SMSF/LRBA eligibility, advice or document specification. Action register can track the chosen multi-adviser actions and evidence; Investment supports generic borrowing cash flows. |
| 25 | [Concessional contribution catch-up](https://betweenthespreadsheets.com.au/products/superannuation-concessional-contribution-catchup-calculator): five-year capacity ageing, balance eligibility, proposed contributions and expiry. | Partial: Carry forward provides current-first/FIFO bucket arithmetic with supplied eligibility, capacities and expiries. It does not determine actual caps, eligibility or a tax deduction. |
| 26 | [Tax position 2026](https://betweenthespreadsheets.com.au/products/tax-position-calculator-2026): group planning, distribution/dividend choices, tax estimates, obligations, losses and instalments; some offsets are explicitly manual. | Partial arithmetic only: Tax cash reconciles assessed liabilities, payments (including instalments), refunds and noncash credits across twelve accounts. It does not calculate individual tax or optimise distributions/dividends. Action register can hold supplied due dates. |
| 27 | [Tax position 2025](https://betweenthespreadsheets.com.au/products/tax-position-june-2025): the earlier tax-position edition. | Partial arithmetic only: Tax cash accepts historical assessed amounts and opening balances. It does not reproduce a historical tax-position rule engine. |
| 28 | [Termination pay](https://betweenthespreadsheets.com.au/products/termination-pay-entitlements-calculator): leave, loading, redundancy, invalidity, other ETPs, caps, super and reporting labels. | Partial: Exit pay calculates approved leave/loading, other leave, notice, redundancy/other gross amounts, net pay and employer cost from supplied inputs. Statutory entitlements, ETP/whole-income caps, super applicability and reporting labels remain gaps. |
| 29 | [Full FBT 2026](https://betweenthespreadsheets.com.au/products/the-full-fbt-calculator-for-31-march-2026): cars/entertainment plus other benefit types, reportable amounts, payroll tax, contributions, declarations and completion controls. | Partial arithmetic only: Benefit totals performs basic business/government type 1/type 2 gross-up and rate multiplication using reviewer inputs. Exemptions, rebates, reportability, benefit classification and consolidated return completion remain gaps. |
| 30 | [Property, active-asset and estate timelines](https://betweenthespreadsheets.com.au/products/timeline-of-principal-places-of-residence-ppr-rentals-active-assets-farms-gst-cgt-and-estates-calculator): editable visual timelines. | Partial: Timeline supplies dated intervals, monthly bands and overlap counts. Tax choices and estate obligations remain outside it. |
| 31 | [Trade-in and replacement asset](https://betweenthespreadsheets.com.au/products/trade-in-sale-of-asset-and-replacement-asset): disposal/replacement GST and tax with caps, write-offs and depreciation choices. | Partial arithmetic only: Asset disposal calculates book profit/loss, replacement cash and the supplied-GST cash bridge. Tax bases, private use, pooling, statutory limits and tax payable remain gaps. |
| 32 | [Trust deed summary](https://betweenthespreadsheets.com.au/products/trusts-summary): deed evidence, parties, elections, a five-year vesting flag and NSW foreign-person amendment records. | Partial: Deed register supplies the records and calendar reminder. No deed interpretation or legal amendment test. |

## Follow-up gap review on 19 October 2026

Eight new workflows and three existing-sheet extensions were feasible from the
public feature descriptions and explicit supplied-input contracts. Eight relevant
vendor pages were rechecked successfully. The benefit gross-up arithmetic was also
checked against the [ATO calculation steps](https://www.ato.gov.au/businesses-and-organisations/hiring-and-paying-your-workers/fringe-benefits-tax/calculating-your-fbt).
The workbook shows that source beside its benefit inputs. Rates remain editable
and must be confirmed for the relevant period and employer. The arithmetic uses
final reviewer-classified taxable values and does not make a tax classification.

The remaining gaps are the legal/tax decisions identified in the table, hidden
vendor rules and edition-specific validation, declarations and reporting.
A generic register or supplied-amount model does not establish product parity.
Further feature work needs the missing rules or a reviewed, independent
specification. The matrix links the existing modelling primitives where useful.

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
