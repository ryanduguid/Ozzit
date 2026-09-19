# Ozzit: Excel LAMBDA functions for Australian financial modelling

One Excel workbook holding 133 native LAMBDA functions and 5 named help tables under
the `oz.` prefix: amortisation, depreciation, ratios, AASB 16 lessee schedules, and
Australian GST and financial-year helpers. There is no add-in, macro, VBA or external
dependency, so a copied function travels inside your own file and a reviewer can read
the arithmetic. It suits an accountant or analyst modelling in Excel; it is not a
bookkeeping tool and never touches a ledger.

**Needs Microsoft 365 or Excel 2024 or later**, which support `LAMBDA` and dynamic
arrays. LibreOffice, Google Sheets and earlier Excel versions cannot evaluate the
functions. MIT licensed.

**Current published release: [v3.4.1](https://github.com/ryanduguid/Ozzit/releases/tag/v3.4.1)**,
13 September 2026. [Download ozzit.xlsx](https://github.com/ryanduguid/Ozzit/releases/download/v3.4.1/ozzit.xlsx)
(438,897 bytes, SHA-256 `71db4f7650b8f40a27c06524bb031166dfe96b9f25c4b8e8ba2928679d2391e7`).
The v3.4.2 cut below is prepared on this branch and is not published yet, so its tag,
release page and download do not exist.

Synthetic example. Review aid, not professional advice; the reviewer decides the GST treatment.

**Input:** $1,100, assumed wholly taxable and GST-inclusive at 10%.

```excel
=oz.GSTExtractλ(1100)
```

**Output:** $100.00 GST, reproduced in Microsoft 365. The hand calculation is $1,100 / 11.

**Human decision:** Does the evidence establish that the whole supply is taxable?

![Synthetic GST example in Excel: $1,100 input and oz.GSTExtractλ(1100) returning $100.00](assets/ozzit-gst-extract-synthetic.png)
Captured in Microsoft 365 using Ozzit v3.2.0, Australian tax!A19:B19; the reviewer still decides whether the supply is taxable.

<details>
<summary>Setup, function catalogue, workbook examples and reference</summary>

A library of 133 native Excel LAMBDA functions plus 5 named help tables for dynamic-array financial models, with inline help and editable demonstration worksheets under the `oz.` prefix. LibreOffice and older Excel versions cannot evaluate the functions.

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
- [Comparing a modelling schedule with an accounting carrying amount](docs/depreciation-comparison.md)
- [Function index](functions.csv) and [source views](src/)

## Verification and attribution

The prepared v3.4.2 cut (citation metadata in [CITATION.cff](CITATION.cff), dated 16 September 2026) awaits the signed tag, native gates and publication [RELEASING.md](RELEASING.md) reserves for a maintainer, so its tag, release page and download do not exist yet. The published release remains [v3.4.1](https://github.com/ryanduguid/Ozzit/releases/tag/v3.4.1).

v3.4.2 `ozzit.xlsx` SHA-256: `cc52d7faa43ba672c56cd2f6ff7588f8565d825908cfcca3fc2ead5ce9b6b350` (443,674 bytes), the same file as the tracked `ozzit.xlsx` on `main`, whose digest is pinned in [release/workbook-base.json](release/workbook-base.json) and checked by the tool tests. Excel 16.0 build 20430 on 16 September 2026: 1,129 formulas recalculated with 0 in error, 905 self-test assertions with 0 failures, 19,444 cached values equal to what their formulas produce.

- [Repository checks](AGENTS.md) and [release and native verification](RELEASING.md)
- [Changes](CHANGELOG.md) and [citation](CITATION.cff)

MIT licensed. See [LICENCE](LICENCE) and [ATTRIBUTION.md](ATTRIBUTION.md) for provenance and licence.

</details>
