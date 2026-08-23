# Provenance and licence

Ozzit began as a derivative of a third-party Excel LAMBDA workbook. The original author has granted written permission for this repository to be published as open source and waived the attribution that earlier releases carried. This repository is not affiliated with or endorsed by the upstream project.

## Licence status

[LICENCE](LICENCE) is MIT and covers the whole repository, `ozzit.xlsx`, `src/` and `functions.csv` included.

Releases up to v3.1.0 limited the MIT grant to `tools/`, `.github/`, the Markdown files and `assets/`, because no licence for the upstream material had been located and the upstream author therefore retained all rights in it. That limitation no longer applies.

## The build input

`tools/transform_from_upstream.py` rebuilds `ozzit.xlsx` from the upstream workbook, which is not in this repository and cannot be. The build was last run against a file of 1,478,643 bytes, sha256 `f38dbc83b4a18fc7d71d0f4bcf39680d74694b9aa129f5b3deb39b014e0bbb67`, holding 224 parts. A rebuild that starts from that file reproduces the v3.0.0 `src/` and `functions.csv`; anything else is a different input and the build's own assertions will say so. The transform does not take `ozzit.xlsx` as a substitute for that upstream file.

That claim describes the v3.0.0 baseline. Post-v3.0.0 passes start from the committed `ozzit.xlsx` and `src/` rather than from upstream. The committed input those later passes were written against is 439,209 bytes, sha256 `26a3e6246ff3d849bb2eb9295b39900682a3ec69dd8475005278de7ad22ef44e`, holding 211 parts. `tools/postbuild/` records the tracked successors of the v3.1.0 session (FY27 help text, workbook palette) and later committed-input inserts such as the GST help note. A rebuild from the upstream file still stops at the v3.0.0 artefacts. Byte-for-byte reproduction of the current workbook from upstream is not claimed: the FY27 date shift was Excel-state-dependent and an Excel save is not stable across Excel builds.

## What changed from the upstream workbook

- All upstream namespaces were replaced by a single `oz.` prefix, with a one-letter tag where two modules shared a function name. Upstream branding, branded artwork, the cover video thumbnails and their YouTube link, and Dropbox file links were removed. One maths-citation link (a Diarmuid Early video, credited in an `IntOnIntλ` source comment) was retained deliberately.
- Help-block links that pointed at upstream gists and the upstream site now point at this repository, relabelled from "Gist URL" to "Repository".
- Per-function revision histories were removed from the module sources, from the Advanced Formula Environment store and from the workbook's creator metadata. This repository's own history is in [CHANGELOG.md](CHANGELOG.md) and in git.
- American English converted to Australian English throughout, including function renames (`Amortizeλ` family to `Amortiseλ`).
- Calibri replaced with Aptos; US date formats replaced with day-first formats; sample data currency set to AUD.
- All sample and demonstration dates moved forward two years, calendar-aware (29 February maps to 28 February in non-leap targets); function version stamps set to 18 August 2026 by the v3.0.0 transform (the v3.1.0 help-text passes later re-dated them to 20 August 2026).
- Defects repaired: the undefined `Sheetλ` title formula on 46 worksheets, an undefined about-box function, locale-fragile `RANDBETWEEN` text-date arguments, a dead table-of-contents link, and assorted typos.
- The foreign depreciation regime was removed outright, and five Australian functions were added: diminishing-value and prime-cost depreciation schedules, two GST helpers and a financial-year label. The two depreciation schedules are modelling helpers, not tax calculations, and v2.1.0 removed the claim that they implement an ATO method.
- Removed an empty Power Query data mashup, orphaned rich-value image residue, and the embedded printer configuration.
