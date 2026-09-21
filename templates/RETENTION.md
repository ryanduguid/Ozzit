# Retention reconciliation

Open `retention-reconciliation.xlsx` in desktop Excel. This standalone sheet
reconciles 3 fabricated contracts: opening retention plus amounts withheld,
less releases, against an independently supplied ledger balance. It uses
ordinary formulas and contains no macros or external links.

Replace the blue input cells in rows 10 to 12 and retain source references and
review notes. Enter the period dates and tolerance above them. Each contract's
difference remains visible even if another contract's difference offsets it.
Blank inputs, duplicate contract IDs, missing evidence and invalid dates keep
the workpaper incomplete. An explicit zero is a supplied value.

The example closes at 58000.00 against ledger retention of 57750.00. Its
250.00 difference needs review. The sheet does not determine entitlement to
release, revenue recognition, tax treatment or permission to post a journal.

The calculation range is fixed at 3 contracts. Adding rows requires extending
the formulas, total range and regression checks; the template does not grow
automatically. Copy the workbook for each period and keep source evidence with it.

After confirming Excel is closed, run:

```powershell
powershell.exe -NoProfile -File tools/excel_retention_selftest.ps1
```

On 21 September 2026, 18 checks passed in Excel 16.0 build 20430. The native
test recalculates after each change, closes without saving and verifies that
the workbook hash is unchanged. This is formula verification, not a review of
any real contract or ledger.
