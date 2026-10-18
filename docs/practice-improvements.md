# Practice calculator improvements

Ozzit's companion now combines 13 arithmetic workflows, explicit assumptions,
input warnings and a linked contents page. Three calculators were added after
comparing similar tools: loan offers, break-even modelling and working-capital
cash release. The [32-product catalogue report](between-spreadsheets.md) records
the original Between The Spreadsheets comparison and its unresolved gaps.

## Similar tools inspected

The public pages below were checked on 18 October 2026. This compares their
documented features, not the correctness of their private workbooks. No account,
trial, purchase or form submission was used.

| Source | Publicly documented pattern | Decision for Ozzit |
| --- | --- | --- |
| [Accountants Desktop](https://accountantsdesktop.com.au/) | Excel templates, JobPapers, checklists, technical references and current/prior-year rates, grouped for practice work. | Give users a task-based entry point, traceable assumptions and visible missing inputs. Keep taxpayer data local. Do not reproduce its private JobPapers or rate library. |
| [CCH iKnowConnect Practical Tools](https://www.wolterskluwer.com/en-au/solutions/cch-iknowconnect/practical-tools) | Calculators and workpapers alongside procedures, examples and checklists, organised by year end, special transactions, client management and practice management. | Document how each calculation is used and what evidence it cannot replace. Keep professional judgement distinct from arithmetic. |
| [Australianbiz](https://www.australianbiz.com.au/default.aspx) | SME financial templates and calculators, including cash-flow budgets, hire-purchase amortisation, KPIs and loan repayments. | Reuse Ozzit's existing cash forecast, amortisation and ratios. Add a working-capital cash estimate that turns day assumptions into amounts. |
| [Vertex42 financial calculators](https://www.vertex42.com/Calculators/financial-calculators.html) | Loan, balloon, debt, budget and business calculators with explanations and assumptions. Its [break-even page](https://www.vertex42.com/ExcelTemplates/breakeven-analysis.html) explains contribution and target-profit arithmetic. | Add a simple fixed-rate offer comparison and an independently written cost-volume-profit sheet. Do not copy downloadable files or their layouts. |
| [Moneysmart mortgage calculator](https://moneysmart.gov.au/home-loans/mortgage-calculator) | Repayment and fee inputs, rate-change exploration and explicit modelling assumptions. | Show both payments and total outlay, identify fees separately and expose a user-supplied rate shock. Do not import its defaults or claim a lender's assessment. |

## Improvement decisions

| Idea | Disposition and reason |
| --- | --- |
| Put usable tasks before the full LAMBDA catalogue. | Implemented: a companion entry in the README task table and a contents page linking all 13 calculators. Every calculator links back. |
| Compare finance costs rather than interest rates alone. | Implemented: Loan offers separates loan payments, interest, fees, balloon and total outlay. |
| Show whether target profit fits operational capacity. | Implemented: Break even calculates whole required units and flags a target above supplied capacity. |
| Translate working-capital days into cash. | Implemented: Working capital shows receivables, inventory and payables on their matching flow bases and preserves negative cash release. |
| Make missing and invalid inputs visible. | Implemented: required numeric inputs distinguish blanks from zero; duplicate names are compared literally, including `*` and `?`. Invalid dates, zero weights, unsupported terms and inconsistent shares remain visible. Native tests change these inputs. |
| Keep rental profit and cash separate. | Implemented: loan principal, agent-held funds and signed GST cash settlements affect cash; incomplete cash-only inputs do not suppress a calculable operating result. |
| Use recognisable, consistent colours and navigation. | Implemented: the current Mermaid palette, platinum editable cells, named warning states and native internal hyperlinks. Colour is accompanied by text. |
| Add another general amortisation, lease, depreciation or ratio library. | Existing coverage: reuse the 137-function library and its worked guides. Duplicating it would add maintenance without a new supported task. |
| Add another general cash forecast or retention workpaper. | Existing coverage: keep the separate 13-week cash-flow and retention templates. |
| Automatically classify FBT, PSI, Division 7A, individual tax or AML/CTF. | Not included: those tools need a complete rule specification, current authority and the scope change described in the catalogue report. |
| Add client-system sync, accounts, subscriptions or private data uploads. | Not included: these are unnecessary for the requested local workbook improvements and introduce separate access and operating requirements. |

## New calculator contracts

### Loan offers

Enter up to eight offers with a principal, nominal annual rate, integer term in
months, final balloon, upfront cash fee and monthly cash fee. Payments occur at
month end and interest compounds monthly. The monthly loan payment comes from
Excel's `PMT`; at zero interest it is `(principal - balloon) / months`. The balloon
is paid in addition to the regular payments at maturity.

Total interest is regular payments plus balloon less principal. Total fees are
upfront fees plus the monthly fee multiplied by months. Total outlay includes
all payments, balloon and fees. The stressed monthly payment uses the supplied
rate change and includes the monthly fee. The ordinary loan-payment column
excludes fees. Compare the same principal and term when assessing offers.

Negative interest, non-integer or nonpositive months, a balloon above principal,
missing inputs and duplicate offer names fail validation. A negative rate shock
is permitted if the resulting rate remains nonnegative. There are no extra
repayments, variable-rate resets, offsets, break costs or borrowing-capacity
decisions.

### Break even

Supply fixed costs, price, variable cost, expected sales, target profit and
capacity for one common period. Contribution per unit is price less variable
cost. Divide fixed costs by contribution for break-even units; add target profit
to fixed costs for target units. Both required unit counts round upwards.

Expected profit is expected units multiplied by contribution less fixed costs.
The sheet flags target units above capacity and calculates the price needed to
reach the target at that capacity. Prices and unit costs are constant modelling
assumptions. A nonpositive contribution produces a visible input warning. Zero
capacity leaves the required price unavailable. Financing and income tax are
excluded, and the user supplies a consistent GST basis.

### Working capital

Supply credit sales, cost of sales, credit purchases and the matching period's
day count. Current/target receivable days use credit sales; inventory days use
cost of sales; payable days use credit purchases. Each estimated balance is
the matching period flow divided by days, multiplied by its balance days.

Cash released is the reduction in receivables and inventory plus the increase
in payables. A negative result means more cash is invested. This is a steady-state
estimate rather than a forecast of collection/payment dates. Use consistent
valuation and GST bases; confirm that operational targets and supplier terms
are feasible.

## Mermaid colours

The palette was verified against the live
[website tokens](https://duguid.com.au/assets/tokens.css?v=20261013) and Ozzit's
existing `tools/postbuild/workbook_palette.py`. It uses black `#000000`, ivory
`#FFFFF0`, Tyrian purple `#66023C` and crimson `#990024`. Platinum `#E5E4E2`
identifies inputs. Existing financial-model conventions retain blue input text;
the companion uses `#2455A4`. Purple headers carry ivory text. Warnings use
crimson text on a pale crimson fill, with the failed condition written out.
Dates/diary tabs are black, the contents tab platinum and financial tabs purple.

The workbook uses a light working surface for readable tables and printing.
It does not rely on dark purple against black for essential text. The main
library and earlier templates already use the established palette. The new
companion uses the same colour roles.

## Native preparation and checks

The authoring runtime cannot produce a usable cached result for `HYPERLINK`.
Navigation therefore uses native Excel cell hyperlinks, added by
`tools/prepare_practice_template.ps1`. The script checks the expected workbook,
adds 26 internal links, recalculates and saves. It refuses to run while a user
Excel session is open. Rebuild in this order:

```powershell
node tools/build_practice_calculators.mjs
powershell.exe -NoProfile -File tools/prepare_practice_template.ps1
python tools/sanitise_workbook.py templates/practice-calculators.xlsx
powershell.exe -NoProfile -File tools/excel_practice_selftest.ps1 -Out docs/practice-native-evidence.json
```

The final native test verifies unchanged workbook bytes and its examples after
input changes. The retained receipt binds the workbook, builder, native preparer
and test runner. Portable repository tests check that binding and reject macros
or external relationships. CodeQL includes JavaScript so the new authoring
builder joins its existing Python and Actions coverage.
