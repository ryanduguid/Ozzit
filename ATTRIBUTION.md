# Attribution

Ozzit was built from scratch by Ryan Duguid. The 6 July 2024 workbook it was rebuilt from is his own earlier iteration.

## Licence status

The workbook material is Ryan Duguid's own work.

The repository's own work is separately licensed. [LICENCE](LICENCE) is MIT and covers `tools/`, `.github/`, the Markdown files and `assets/`, which were written for this repository. It does not extend to `ozzit.xlsx`, `src/` or `functions.csv`.

## The build input

`tools/transform_from_earlier.py` rebuilds `ozzit.xlsx` from Ryan Duguid's earlier workbook, which is not committed to this repository. The build was last run against a file of 1,478,643 bytes, sha256 `f38dbc83b4a18fc7d71d0f4bcf39680d74694b9aa129f5b3deb39b014e0bbb67`, holding 224 parts. A rebuild that starts from that file reproduces the v3.0.0 `src/` and `functions.csv`; anything else is a different input and the build's own assertions will say so. The transform does not take `ozzit.xlsx` as a substitute for that earlier file.

That claim describes the v3.0.0 baseline. Post-v3.0.0 passes start from the committed `ozzit.xlsx` and `src/` rather than from the earlier workbook. The committed input those later passes were written against is 439,209 bytes, sha256 `26a3e6246ff3d849bb2eb9295b39900682a3ec69dd8475005278de7ad22ef44e`, holding 211 parts. `tools/postbuild/` records the tracked successors of the v3.1.0 session (FY27 help text, Luma palette) and later committed-input inserts such as the GST help note. A rebuild from the earlier file still stops at the v3.0.0 artefacts. Byte-for-byte reproduction of the current workbook from the earlier workbook is not claimed: the FY27 date shift was Excel-state-dependent and an Excel save is not stable across Excel builds.

## What changed from Ryan Duguid's earlier workbook

- All namespaces renamed: the earlier namespaces were replaced by a single `oz.` prefix, with a one-letter tag where two modules shared a function name; earlier branding, branded artwork, the cover video thumbnails and their YouTube link, and Dropbox file links removed. One maths-citation link (a Diarmuid Early video, credited in an IntOnIntλ source comment) was retained deliberately.
- Help-block links that pointed at Ryan Duguid's gists and site now point at this repository, relabelled from "Gist URL" to "Repository".
- The author's name is preserved in the revision histories inside the module sources (one earlier misspelling, "Ryan Duguid", corrected), and the workbook's creator metadata credits him. Branding was removed; authorship records were not.
- American English converted to Australian English throughout, including function renames (`Amortizeλ` family to `Amortiseλ`).
- Calibri replaced with Aptos; US date formats replaced with day-first formats; sample data currency set to AUD.
- All sample and demonstration dates moved forward two years, calendar-aware (29 February maps to 28 February in non-leap targets); function version stamps set to 18 August 2026.
- Defects repaired: the undefined `Sheetλ` title formula on 46 worksheets, the undefined `the earlier namespace Aboutλ` function, locale-fragile `RANDBETWEEN` text-date arguments, a dead table-of-contents link, and assorted typos.
- The foreign depreciation regime was removed outright, and five Australian functions were added: diminishing-value and prime-cost depreciation schedules, two GST helpers and a financial-year label. The two depreciation schedules are modelling helpers, not tax calculations, and v2.1.0 removed the claim that they implement an ATO method.
- Removed an empty Power Query data mashup, orphaned rich-value image residue, and the embedded printer configuration.
