# Attribution

Ozzit is a derivative work. The underlying function library and documentation workbook were created by **the workbook author Ryan Duguid** and published as the **predecessor workbook** for his predecessor modelling ecosystem (the predecessor listing; the 6 July 2024 release, the latest located as at 18 August 2026). Copyright in the original work remains with the original author. This repository is not affiliated with or endorsed by him.

## Licence status

No open-source licence could be located for the predecessor work as at 18 August 2026:

- The author's public gists carry a bare "Copyright" notice with no licence grant.
- The the predecessor site site states no licence terms for the libraries.
- The the predecessor listing listing was not reachable for terms verification.

Absent a licence, the workbook author Ryan Duguid retains all rights in the original material. Treat the derived portions of this repository accordingly. If you are the workbook author Ryan Duguid and want changes to this repository, open an issue.

The repository's own work is separately licensed. [LICENCE](LICENCE) is MIT and covers `tools/`, `.github/`, the Markdown files and `assets/`, which were written for this repository. It does not extend to `ozzit.xlsx`, `src/` or `functions.csv`.

## The build input

`tools/transform_from_predecessor.py` rebuilds `ozzit.xlsx` from Ryan Duguid's predecessor workbook, which is not committed to this repository. The build was last run against a file of 1,478,643 bytes, sha256 `f38dbc83b4a18fc7d71d0f4bcf39680d74694b9aa129f5b3deb39b014e0bbb67`, holding 224 parts. A rebuild that starts from that file reproduces the v3.0.0 `src/` and `functions.csv`; anything else is a different input and the build's own assertions will say so.

That claim describes the v3.0.0 baseline. v3.1.0's FY27 examples, Luma palette and later maintenance passes changed the built workbook after that baseline was produced, and the tracked repository tools now preserve and verify those changes rather than regenerate the historical pass from the predecessor workbook. A rebuild from the predecessor file reproduces the v3.0.0 artefacts, then requires those later passes to reach the current release. Byte-for-byte reproduction of the current workbook is not claimed: the FY27 date shift was Excel-state-dependent and an Excel save is not stable across Excel builds.

## What changed from Ryan Duguid's predecessor workbook

- All namespaces renamed: the predecessor namespaces were replaced by a single `oz.` prefix, with a one-letter tag where two modules shared a function name; predecessor branding, branded artwork, the cover video thumbnails and their YouTube link, and Dropbox file links removed. One maths-citation link (a Diarmuid Early video, credited in an IntOnIntλ source comment) was retained deliberately.
- Help-block links that pointed at the workbook author Ryan Duguid's gists and site now point at this repository, relabelled from "Gist URL" to "Repository".
- The author's name is preserved in the revision histories inside the module sources (one predecessor misspelling, "the workbook author Ryan Duguid", corrected), and the workbook's creator metadata credits him. Branding was removed; authorship records were not.
- American English converted to Australian English throughout, including function renames (`Amortizeλ` family to `Amortiseλ`).
- Calibri replaced with Aptos; US date formats replaced with day-first formats; sample data currency set to AUD.
- All sample and demonstration dates moved forward two years, calendar-aware (29 February maps to 28 February in non-leap targets); function version stamps set to 18 August 2026.
- Defects repaired: the undefined `Sheetλ` title formula on 46 worksheets, the undefined `the predecessor namespace Aboutλ` function, locale-fragile `RANDBETWEEN` text-date arguments, a dead table-of-contents link, and assorted typos.
- The foreign depreciation regime was removed outright, and five Australian functions were added: diminishing-value and prime-cost depreciation schedules, two GST helpers and a financial-year label. The two depreciation schedules are modelling helpers, not tax calculations, and v2.1.0 removed the claim that they implement an ATO method.
- Removed an empty Power Query data mashup, orphaned rich-value image residue, and the embedded printer configuration.
