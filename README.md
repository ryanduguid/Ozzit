# Ozzit: Excel LAMBDA functions for Australian financial modelling

One Excel workbook holding 133 native LAMBDA functions and 5 named help tables under
the `oz.` prefix: amortisation, depreciation, ratios, AASB 16 lessee schedules, and
Australian GST and financial-year helpers. The functions need no add-in, macro, VBA or
external dependency, so a copied function travels inside your own file and a reviewer
can read the arithmetic. The file carries a hidden reference to the Advanced Formula
Environment task pane used to author the functions; the functions do not depend on
it. It suits an accountant or analyst modelling in Excel; it is not a
bookkeeping tool and never touches a ledger.

**Needs Microsoft 365 or Excel 2024 or later**, which support `LAMBDA` and dynamic
arrays. LibreOffice, Google Sheets and earlier Excel versions cannot evaluate the
functions. MIT licensed.

**Current published release: [v3.4.2](https://github.com/ryanduguid/Ozzit/releases/tag/v3.4.2)**,
published 20 September 2026 from the cut dated 16 September 2026.
[Download ozzit.xlsx](https://github.com/ryanduguid/Ozzit/releases/download/v3.4.2/ozzit.xlsx)
(439,097 bytes, SHA-256 `0306793a7e473ce70e78149fea1e107fc0f714d61ab50960c16f6fd528878f6f`).

Synthetic example. Review aid, not professional advice; the reviewer decides the GST treatment.

**Input:** $1,100, assumed wholly taxable and GST-inclusive at 10%.

```excel
=oz.GSTExtractλ(1100)
```

**Output:** the cell displays $100.00 GST under currency formatting, reproduced in
Microsoft 365. The function returns the unrounded binary-float result,
99.999999999999986, so a caller who needs cent-exact totals wraps it in
`ROUND(...,2)`. The hand calculation is $1,100 / 11.

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

The current release is v3.4.2, dated 16 September 2026; citation metadata is in [CITATION.cff](CITATION.cff). It was published on 20 September 2026 from the signed tag `v3.4.2` on commit `621af0e265306f00a83e17c4d27dca09031c5ccd`, after the native gates [RELEASING.md](RELEASING.md) requires ran on the exact file it ships. The [release page](https://github.com/ryanduguid/Ozzit/releases/tag/v3.4.2) carries `ozzit.xlsx`, `provenance.json` and `SHA256SUMS`, and GitHub reports the release immutable.

v3.4.2 `ozzit.xlsx` SHA-256: `0306793a7e473ce70e78149fea1e107fc0f714d61ab50960c16f6fd528878f6f` (439,097 bytes), the same bytes as the release asset. `main` is ahead of that tag, so the file you clone is not the file the release ships: the tracked `ozzit.xlsx` is SHA-256 `72949c61c31bb760f6533e49744f9f30944d93caf7afb1190e8b25f536695a4f` (443,243 bytes), pinned in [release/workbook-base.json](release/workbook-base.json) and checked by the tool tests. On the tracked file, Excel 16.0 build 20430 on 23 September 2026: 1,129 formulas recalculated with 0 in error, 925 self-test assertions with 0 failures, 19,446 cached values equal to what their formulas produce, and the workbook byte-identical before and after both native gates.

- [Repository checks](AGENTS.md) and [release and native verification](RELEASING.md)
- [Changes](CHANGELOG.md) and [citation](CITATION.cff)

MIT licensed. See [LICENCE](LICENCE) and [ATTRIBUTION.md](ATTRIBUTION.md) for provenance and licence. [DISCLAIMER.md](DISCLAIMER.md) states that this is not advice and names the bodies the project is not affiliated with.

</details>
