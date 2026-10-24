# Ozzit: Excel LAMBDA functions for Australian financial modelling

<div align="center">

[![verify](https://img.shields.io/github/actions/workflow/status/ryanduguid/Ozzit/verify.yml?branch=main&label=verify&labelColor=000000&style=flat)](https://github.com/ryanduguid/Ozzit/actions/workflows/verify.yml) [![Codecov](https://img.shields.io/codecov/c/github/ryanduguid/Ozzit?label=coverage&labelColor=000000&style=flat)](https://codecov.io/gh/ryanduguid/Ozzit) [![CodeQL](https://img.shields.io/github/actions/workflow/status/ryanduguid/Ozzit/codeql.yml?branch=main&label=CodeQL&labelColor=000000&style=flat)](https://github.com/ryanduguid/Ozzit/actions/workflows/codeql.yml) [![SonarCloud](https://img.shields.io/sonar/quality_gate/ryanduguid_Ozzit?server=https%3A%2F%2Fsonarcloud.io&label=SonarCloud&labelColor=000000&style=flat)](https://sonarcloud.io/summary/new_code?id=ryanduguid_Ozzit) [![Codacy code quality](https://img.shields.io/codacy/grade/ffd3c7d7645541179caea8a353bd3f86?label=Codacy&labelColor=000000&style=flat)](https://app.codacy.com/gh/ryanduguid/Ozzit/dashboard)<br> [![OpenSSF Scorecard](https://img.shields.io/ossf-scorecard/github.com/ryanduguid/Ozzit?label=Scorecard&labelColor=000000&style=flat)](https://scorecard.dev/viewer/?uri=github.com/ryanduguid/Ozzit) [![OpenSSF Best Practices](https://img.shields.io/cii/level/15420?label=Best%20Practices&labelColor=000000&style=flat)](https://www.bestpractices.dev/projects/15420) [![VirusTotal](https://img.shields.io/badge/VirusTotal-scanned-990024?labelColor=000000&style=flat&logo=virustotal&logoColor=FFD32A&color=990024)](https://github.com/ryanduguid/Ozzit/releases/latest)<br> [![release](https://img.shields.io/github/v/release/ryanduguid/Ozzit?color=66023C&labelColor=000000&style=flat)](https://github.com/ryanduguid/Ozzit/releases/latest) ![Excel: 365 or 2024+](https://img.shields.io/badge/Excel-365%20or%202024%2B-66023C?labelColor=000000&style=flat&color=66023C) [![licence: Apache-2.0](https://img.shields.io/badge/licence-Apache--2.0-66023C?labelColor=000000&style=flat&color=66023C)](LICENSE)

</div>

One Excel workbook holding 143 native LAMBDA functions and 5 named help tables under
the `oz.` prefix: amortisation, depreciation, ratios including the ATO's small business
benchmarks, a checked dated rate lookup, AASB 16 lessee schedules, and Australian GST
and financial-year helpers. The functions need no add-in, macro, VBA or
external dependency, so a copied function travels inside your own file and a reviewer
can read the arithmetic. The file carries a hidden reference to the Advanced Formula
Environment task pane used to author the functions; the functions do not depend on
it. It suits an accountant or analyst modelling in Excel; it is not a
bookkeeping tool and never touches a ledger.

**Needs Microsoft 365 or Excel 2024 or later**, which support `LAMBDA` and dynamic
arrays. LibreOffice, Google Sheets and earlier Excel versions cannot evaluate the
functions. Apache-2.0 licensed.

**Current published release: [v3.5.0](https://github.com/ryanduguid/Ozzit/releases/tag/v3.5.0)**,
published 19 October 2026 from the cut dated 19 October 2026, with 143 functions.
[Download ozzit.xlsx](https://github.com/ryanduguid/Ozzit/releases/download/v3.5.0/ozzit.xlsx)
(462,490 bytes, SHA-256 `76d3722c785abd3813ffffffc5fbb2d6ab80628da10fb7f36cea7c67fd171330`).

**Fixed in v3.5.0, still present in v3.4.2.** One blank cell in a column of GST rates gives every row
10%, zero-rated rows included; one blank opening in `oz.Movementλ` sets every row's
opening to 0; and negative CFADS raises the balance in `oz.DebtSculptFixedλ` and
`oz.DebtSculptVariableλ`. One blank cell likewise gives every row the default in a
column of PeriodsPerYear for `oz.PeriodRateλ` and `oz.AnnualRateλ`, of Unit for
`oz.DateDifλ` and of Repeats for `oz.IsOccurrenceDateλ`, whose LastOccurrence column
reads no text date once another row holds a date or a blank. In v3.4.2, fill every such
cell and enter last occurrences as dates, or move to v3.5.0. Further changes and the DB/DDB compatibility limitations are recorded
under [v3.5.0 in the changelog](CHANGELOG.md).

Synthetic example. The reviewer decides the GST treatment.

**Input:** $1,100, assumed wholly taxable and GST-inclusive at 10%.

```excel
=oz.GSTExtractλ(1100)
```

**Output:** the cell displays $100.00 GST under currency formatting, reproduced in
Excel 16.0 build 20430 against the exact v3.4.2 release on 25 September 2026.
[Inputs, raw results and unchanged workbook hash](docs/reference-examples-v3.4.2.json).
The function returns the unrounded binary-float result,
99.999999999999986, so a caller who needs cent-exact totals wraps it in
`ROUND(...,2)`. The hand calculation is $1,100 / 11.

**Human decision:** Does the evidence establish that the whole supply is taxable?

[Browse ten worked formulas](https://duguid.com.au/tools/ozzit/#functions),
including arguments, expected results and limits for v3.4.2.

<details>
<summary>Historical v3.2.0 screenshot</summary>

![Synthetic GST example in Excel: $1,100 input and oz.GSTExtractλ(1100) returning $100.00](assets/ozzit-gst-extract-synthetic.png)
Captured in Microsoft 365 using Ozzit v3.2.0, Australian tax!A19:B19; the reviewer still decides whether the supply is taxable.

This earlier image is retained as historical evidence. The current numerical
check above used a different workbook file.

</details>

## Start with a task

You do not need all 143 functions. Start with the task you need to complete.
The loan and depreciation guides show the spilled result for a synthetic input
you can check by hand; the lease guide explains the four lessee functions.

| Task | Formula | Guide |
| --- | --- | --- |
| Loan schedule | `=oz.Amortiseλ(Principals, APRs, Terms, StartDates)` | [100,000 at 5% over 60 months](docs/function-guide.md#dynamic-array-formula-walkthrough) |
| Depreciation | `=oz.DiminishingValueλ(Cost, Life)` | [1,000 over 5 years](docs/function-guide.md#dynamic-array-formula-walkthrough) |
| AASB 16 lease | `=oz.LeaseScheduleλ(Payments, Rate, [InAdvance])` for the liability, `=oz.ROUScheduleλ(Cost, Periods)` for the right-of-use asset | [Lessee functions and rate conversion](docs/australian-modelling.md#aasb-16-leases) |
| Practice or business calculator | Open the [standalone companion](templates/practice-calculators.xlsx); select one of its 21 calculators | [Inputs, assumptions and coverage](docs/between-spreadsheets.md) |

[Open, copy functions and use modern Excel](docs/workbook-use.md) shows how to
copy a function into your own workbook.

For an AI assistant, [the Ozzit skill](skills/ozzit/SKILL.md) routes function
selection and composition through the existing catalogue and approved workbook.
Copy `skills/ozzit` into the host's skill directory and supply the approved bundle
location and revision. Installing the skill does not install the Excel names.

<details>
<summary>Setup, function catalogue, workbook examples and reference</summary>

A library of 143 native Excel LAMBDA functions plus 5 named help tables for dynamic-array financial models, with inline help and editable demonstration worksheets under the `oz.` prefix. LibreOffice and older Excel versions cannot evaluate the functions.

## Capture evidence

The 6 September 2026 capture used Excel 16.0 build 20326. The released workbook opened without a repair prompt and a full calculation rebuild returned numeric 100 for the displayed formula.

v3.2.0 release asset SHA-256: `13df5eb0e2e7a3d1b17a743a990c30adfd187d409be133996ec154543e78ff28`.

The [capture record and PowerShell reproduction](docs/native-gst-capture.md) preserve the Excel command, returned value, crop and image fingerprint.

The screenshot uses a disposable copy with a synthetic label, an explicit argument in B19 and currency formatting. It certifies this example in v3.2.0. The v3.4.2 workbook is a different file; its full native gate results are recorded in its release notes and in [CHANGELOG.md](CHANGELOG.md). The shipped workbook was not edited.

## Guides

- [Open, copy functions and use modern Excel](docs/workbook-use.md)
- [Modules, worksheets and formula walkthroughs](docs/function-guide.md)
- [Australian conventions and lease modelling](docs/australian-modelling.md)
- [Separate 13-week cash-flow template](docs/cash-flow-template.md)
- [Practice calculators and Between The Spreadsheets coverage](docs/between-spreadsheets.md): twenty-one standalone arithmetic workflows and the remaining gaps across its 32 public product listings.
- [Similar tools, improvement decisions and Mermaid colours](docs/practice-improvements.md)
- [Comparing a modelling schedule with an accounting carrying amount](docs/depreciation-comparison.md)
- [Ratio definitions: the arithmetic, periods and balances each ratio expects](docs/ratio-definitions.md)
- [Modelling conventions: where Ozzit follows or departs from the FAST Standard and the ICAEW Financial Modelling Code](docs/modelling-conventions.md)
- [Function index](functions.csv), [source views](src/) and the whole library as one Advanced Formula Environment module, [oz.txt](oz.txt)

## Verification and attribution

The current release is v3.5.0, dated 19 October 2026, under Apache-2.0. It was published on 19 October 2026 from the signed tag `v3.5.0` on commit `40b0408c3f1bd9a9cea7cf1580bd039bcba03fde`, after the native gates [RELEASING.md](RELEASING.md) requires ran on the exact file it ships. The [release page](https://github.com/ryanduguid/Ozzit/releases/tag/v3.5.0) carries `ozzit.xlsx`, `provenance.json` and `SHA256SUMS`, and GitHub reports the release immutable. [The v3.5.0 changes](CHANGELOG.md) describe what it adds. v3.4.2 (dated 16 September 2026, published 20 September 2026 from commit `621af0e265306f00a83e17c4d27dca09031c5ccd`) and earlier releases remain under MIT.

v3.5.0 `ozzit.xlsx` SHA-256: `76d3722c785abd3813ffffffc5fbb2d6ab80628da10fb7f36cea7c67fd171330` (462,490 bytes), the same bytes as the release asset. `main` is ahead of that tag, so the file you clone is not the file the release ships: the tracked `ozzit.xlsx` is SHA-256 `f0347a90e138e7c1d42c666f873d25b1dbbb7ac05095968f1d4383c4aa011270` (474,139 bytes), pinned in [release/workbook-base.json](release/workbook-base.json) and checked by the tool tests, and adds the 5 functions and the changes listed under Unreleased in the [changelog](CHANGELOG.md). On 24 October 2026, Excel 16.0 build 20527 recalculated 1,129 formulas in the tracked workbook with 0 in error, ran 1,205 self-test assertions with 0 failures and found 19,477 cached values equal to what their formulas produce. The workbook was byte-identical before and after both native gates.

- [Repository checks](AGENTS.md) and [release and native verification](RELEASING.md)
- [Changes](CHANGELOG.md) and [citation](CITATION.cff)

Ozzit's functions were written by Ryan Duguid. Ozzit is not derived from any third-party workbook or library.

Apache-2.0 licensed. See [LICENSE](LICENSE). If you redistribute this work or build on it, keep the attribution in [NOTICE](NOTICE). See [DISCLAIMER.md](DISCLAIMER.md) for scope and review requirements.

</details>
