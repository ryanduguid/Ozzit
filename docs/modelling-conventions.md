# Modelling conventions

This page reads Ozzit's design against two public modelling guides: the FAST
Standard, version 02c (July 2019), and the ICAEW Financial Modelling Code (2024).
For each rule that bears on a function library, it records whether Ozzit follows
it, departs from it and why, or finds that it does not apply. It is a reading of
the rules against the source at the commit that adds this page, not a claim that
Ozzit complies with either guide, and neither body has reviewed it.

Both guides were written for models: workbooks with inputs, a time axis,
calculations and outputs. Ozzit is a library of 133 LAMBDA functions and 5 help
tables that models call. FAST 02c mentions neither LAMBDA nor dynamic arrays, and
the ICAEW Code does not mention LAMBDA, so each rule is read for its purpose.
Rules about workbook and worksheet layout still apply to any model built with
Ozzit, including the 13-week cash-flow template, which this page does not assess.

## Named LAMBDA functions

The largest question is Ozzit's form. A named LAMBDA hides its arithmetic behind a
name, which FAST 3.03-08 warns against for ranges, and its definition is a
multi-line formula, which FAST 3.03-01 to 3.03-03 rule out in cells. Against that:

- The call is short and names what it returns, and its arguments stay in view:
  `=oz.CashRatioλ(Cash, Liabilities)`.
- The arithmetic is readable outside Name Manager. [`src/`](../src/) holds each
  definition as text, [`oz.txt`](../oz.txt) holds the library as one Advanced
  Formula Environment module, and every function except the three `λDV`
  argument validators returns its own help table (description, parameters and a
  worked example) when called without its required arguments.
- Each definition stages its work in named `LET` steps (defaults, checks,
  procedure, result), which is FAST's calculation-block idea (2.02) inside one
  formula.
- CI checks that the text views match the workbook (`tools/verify_sources.py`,
  `tools/verify_afe.py`), and the native Excel self-test in
  [RELEASING.md](../RELEASING.md) checks results.

So the definitions depart from FAST 3.03-01 to 3.03-03 and 3.03-08, and the call
sites follow those rules' purpose. The ICAEW Code's "Minimise calculation
complexity" section accepts that some computations are easier to follow when
such hard rules are broken; Ozzit relies on that. A model that calls Ozzit should
still keep its own formulas short and labelled.

## FAST Standard 02c

| Rule | Ozzit | Reason |
|---|---|---|
| Sections 1 and 2: workbook and worksheet design | Does not apply | They govern models. The library workbook holds help and example sheets, not a model. |
| 3.02-01 Consistent formulas along the series | Follows | A function returns a whole series from one formula, so a row has one formula rather than copies. |
| 3.03-01 to 3.03-03 Formula length and multi-line formulas | Departs in definitions | See [Named LAMBDA functions](#named-lambda-functions). |
| 3.03-05 and 3.03-07 Limit IF; never nest IFs | Departs | 59 functions call `IF`, and at least 18 nest one `IF` inside another, mostly to set a default: `IF(ISOMITTED(Rows), 1, IF(Rows = 0, 1, Rows))` in `oz.RangeToDAλ`. Each nest sits in one named `LET` step. |
| 3.03-08 Do not use Excel Names | Departs | Every function is a name; that is how Excel delivers a LAMBDA library. See [Named LAMBDA functions](#named-lambda-functions). |
| 3.03-09 Do not construct array formulas | Departs | The functions return dynamic arrays by design. FAST's exceptions cover a calculation that cannot be done without arrays (3.03-09.2) and one where avoiding arrays is harder to review than the array form (3.03-09.3); a schedule that returns one row of periods from one call rests on the second. |
| 3.04-01 No embedded constants | Departs in part | Rates are arguments: the 10% default in `oz.GSTAddλ` and `oz.GSTExtractλ` can be overridden, and its source is in the help. But `oz.DayCountRateλ`, `oz.Depreciateλ` and `oz.TimelineOffsetλ` embed 30.5 days a month to turn day counts into whole months. FAST allows universal constants such as 12 months a year; 30.5 is an approximation, not a universal constant. |
| 3.05 Labelling | Follows in part | Every help table names and describes each parameter and marks it required or optional; not every parameter carries a unit. The three `λDV` validators have no help table. |
| 3.06 Links | Does not apply | The library has no links between cells. |
| 4.01-01 INDEX over CHOOSE | Departs narrowly | Every function ends with `CHOOSE` over values already named in `LET`: the result, the help table or a message, or for the three `λDV` validators TRUE, their messages or an error. That is not a choice between scenario options, which is the rule's concern. Functions that pick from a series use `INDEX` (15 functions). |
| 4.01-02 Never use NPV | Follows | No function calls `NPV` or `XNPV`. |
| 4.01-03 No OFFSET or INDIRECT | Departs in 3 functions | `oz.RangeToDAλ` and its copies `oz.RangeToDAEλ` and `oz.RangeToDAUλ` use `OFFSET` to return a block of a given size from a starting cell. Their help points to Excel's newer TRIMRANGE and trim references for the same job. No function calls `INDIRECT`. |
| 4.01-04 ROUND | Follows | 7 functions round. Six turn day counts or effective lives into whole months or periods, which a schedule needs. `oz.Allocateλ` rounds each part to the cent and puts the difference in the last part, as its help states: the single, stated adjustment point the rule asks for instead of rounding throughout. |
| 4.02-02 Do not merge cells | Departs | 48 of the workbook's 49 sheets merge a few cells, mostly for titles and notes. Of the 51 formulas in merged ranges, 46 are a title showing the sheet's name, `=TEXTAFTER(CELL("filename",A1),"]")`; the other 5 are examples, such as the help-table call `=oz.Amortiseλ()`. |
| 4.03-01 and 4.03-02 Anchored, workbook-level Names | Follows | Every function name is workbook-level; the only sheet-level names are Excel's print areas. A LAMBDA name refers to no range, so anchoring does not arise. |
| 4.06 Macros | Follows | FAST 02c has no settled position; Ozzit ships an ordinary `.xlsx` with no macros. |

## ICAEW Financial Modelling Code (2024)

The Code's text is reserved, so this page refers to its sections by heading only.

| Section | Ozzit | Reason |
|---|---|---|
| Model definition and purpose; layout and structure | Does not apply | They govern a model's scope, goals, layout and navigation. |
| Include user guidance | Follows | Every function except the three `λDV` validators has a help table, and 5 About tables list each module's functions. |
| Avoid duplication | Departs | Utilities repeats the 17 Essentials functions with a `U` suffix so that it can be installed alone; see the [function guide](function-guide.md). |
| Don't hide things | Departs, with copies | The logic sits in Name Manager, not on a sheet; `src/`, `oz.txt` and the help tables are the visible copies, and CI checks them against the workbook. |
| Use consistent formulas | Follows | As FAST 3.02-01. |
| Use clear and meaningful labels; use clear range names | Follows | Function names say what they return, the `λ` suffix marks a function, and the `oz.` prefix marks the module. |
| Review and test your models | Follows | CI runs the verification gates in [CONTRIBUTING.md](../CONTRIBUTING.md), and releases need the native Excel self-test and cached-value evidence in [RELEASING.md](../RELEASING.md). |
| Minimise calculation complexity | Departs in definitions | As FAST 3.03-01 to 3.03-03; the Code allows it where breaking a hard rule is easier to follow. |
| Build traceable references (no array formulas) | Departs | As FAST 3.03-09. |
| Avoid hardcoding | Departs in part | As FAST 3.04-01: rates are arguments, but 30.5 days a month is embedded. |
| Avoid circular references | Follows | No function refers to its own result. |
| Avoid unnecessary rounding | Departs once | `oz.Allocateλ` balances rounded parts back to the original amount, which the Code advises against; its help says so. The other rounding produces whole periods, not presentation. |
| Use VBA and macros sparingly; document VBA | Follows | No VBA. |

## Sources and credit

- *The FAST Standard*, version 02c (July 2019), FAST Standard Organisation,
  [fast-standard.org](https://www.fast-standard.org), licensed under
  [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). This page cites its
  rule numbers, shortens and paraphrases their titles, and adds Ozzit's own
  verdicts; it does not reproduce the Standard.
- *Financial Modelling Code* (2024), ICAEW, [icaew.com](https://www.icaew.com).
  All rights reserved by ICAEW; the page refers to section headings and does not
  reproduce its text.

Both documents were read on 29 September 2026. The figures above come from
[`src/`](../src/) and `ozzit.xlsx` at the same commit;
`tools/tests/test_modelling_conventions.py` fails if the `OFFSET`, `INDIRECT`,
`NPV`, rounding, name-scope or macro facts change.
