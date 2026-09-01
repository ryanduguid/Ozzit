# Releasing Ozzit

## Scope and history

This policy applies to future releases after it is merged into the default
branch. It does not alter or certify historical releases, their tags or their
assets.

Correct a published or verified candidate under a new version and tag. Never move or retarget an existing tag or overwrite or replace a verified asset.
A draft upload is a candidate only; it is not approval to publish a release.

## Candidate and source ownership

Start from a clean, isolated checkout of the exact candidate commit. Record the
full commit SHA and the pre-verification SHA-256 and byte length of
`ozzit.xlsx`.

`ozzit.xlsx` is the shipped authority. `src/*.txt`, the Advanced Formula
Environment (AFE) store and `functions.csv` are bound publication views that
must agree with the exact workbook in the same tagged commit. Do not approve
changes to `src/` or `functions.csv` in isolation. A formula change must update
the compiled defined names and every publication view together through a
documented source-owning change process.

The v3.0.0 tracked builder starts from its disclosed upstream input, which
is not committed, and stops at the v3.0.0 artefacts. Post-v3.0.0 passes start
from the committed `ozzit.xlsx` and `src/` recorded in `ATTRIBUTION.md`. They
do not regenerate the current workbook from upstream. The current result also
includes a one-off Excel-state-dependent date shift. The consistency checks do not prove
byte-for-byte regeneration of the current workbook. The v3.0.0
builder, the postbuild limits and any future one-off transformation remain
disclosed in `ATTRIBUTION.md` and `CHANGELOG.md`. Do not broaden a
reproducibility claim without a complete regenerator and a byte comparison.

MIT covers the whole repository, `ozzit.xlsx`, `src/` and `functions.csv` included. The upstream author granted written permission for the derived material to be released as open source and waived attribution, so the licence split that applied up to v3.1.0 is retired.
Every source archive must still include `ATTRIBUTION.md` and `LICENCE`. Keep the written permission on file outside this repository: it is the only record of why MIT reaches the derived material, and `ATTRIBUTION.md` no longer names the grantor.

## Approval and tag

A human maintainer explicitly approves the exact candidate commit and release version after all ten gates pass.
Create an annotated, cryptographically signed tag for that exact commit. Run
`git verify-tag` and record the tag object SHA, peeled commit SHA and
verification result.

Do not let `gh release create` silently create a lightweight tag from a branch.
Tag signing, remote tag publication and release publication are separate authorised actions; this document does not approve any of them.

## Future release bundle

The uploaded bundle contains exactly three files:

1. `ozzit.xlsx`: the consumer workbook copied byte-for-byte from the tagged tree.
2. `provenance.json`: canonical JSON binding the version and full candidate commit to the locked workbook hash, size, Git blob, last workbook change and deterministic workbook-gate results. It also states the copy-only build limit and the gates that remain outside the bundle.
3. `SHA256SUMS`: canonical SHA-256 lines for `ozzit.xlsx` and `provenance.json`. It does not include itself, which avoids a checksum cycle.

The signed tag and GitHub's generated source archive remain the source distribution. Inspect that archive as the equivalent of `git archive` for the exact tag; do not upload a redundant custom source archive. The standalone workbook and tagged workbook must have the same SHA-256.

After all ten gates pass in a clean candidate, stage and independently verify the bundle in two fresh directories outside the repository:

```powershell
python tools/prepare_release_bundle.py create --version X.Y.Z --source-commit <full-commit-sha> --output <first-new-directory>
python tools/prepare_release_bundle.py verify --bundle <first-new-directory> --version X.Y.Z --source-commit <full-commit-sha>
python tools/prepare_release_bundle.py create --version X.Y.Z --source-commit <full-commit-sha> --output <second-new-directory>
python tools/prepare_release_bundle.py verify --bundle <second-new-directory> --version X.Y.Z --source-commit <full-commit-sha>
```

Require all three files in both directories to be byte-identical. The tool refuses a dirty checkout, a mismatched workbook base, non-canonical version or commit values, an output inside the source repository and any existing output path. It runs commands as argument lists without a shell, stages under a fresh sibling directory and exposes the final name only after structural verification succeeds. It never tags, uploads or publishes anything.

The independent verifier must record and compare GitHub's API `digest` for the uploaded checksum file. If a detached signature is added later, define whether it sits outside the checksum set and verify it separately; do not create a checksum and signature cycle. Do not add a cosmetic SPDX or CycloneDX inventory to this workbook-only bundle.

## Verification gates

Install the pinned type checker in the clean candidate:

```powershell
python -m pip install "mypy==2.3.1"
```

Then run the following eight static and repository gates:

```powershell
python -m mypy --config-file mypy.ini
python tools/verify_workbook.py ozzit.xlsx
python tools/verify_sources.py ozzit.xlsx src
python tools/verify_signatures.py src
python tools/verify_previous_names.py functions.csv
python tools/verify_index.py ozzit.xlsx src functions.csv
python tools/verify_afe.py ozzit.xlsx src
python -m unittest discover -s tools/tests -v
```

All eight gate commands must exit zero. Record their substantive type-check, function,
signature, table, example, index, module and test counts rather than only their
exit status.

Before either native gate, prove that no user Excel process is running. Do not
close or attach to a user's Excel session. Then run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\excel_selftest.ps1 -Path .\ozzit.xlsx
python tools/verify_cache.py ozzit.xlsx
```

The current acceptance baseline is 1,129 formulas recalculated with zero error cells and 730 assertions run with zero failures: the 438 hand-written assertions and the 292 that `tools/generate_selftest_examples.py` derives from the help. The generated count is a static count of the fragment; record the number the script prints.
The cached-value gate must pass and report its actual comparison count. Record
the Excel version and build. Hash the workbook immediately before and after
both native gates and require the bytes to be byte-identical.

If a reviewed workbook change deliberately changes a formula, assertion or
cached-value count, update the documented expected count in the same candidate
and review that change. Do not silently weaken a floor.

## Packaging, independent verification and immutability

1. Build the exact three-file bundle from the signed tag into a fresh staging directory outside the repository, then repeat it and compare every byte.
2. Compute `provenance.json` and `SHA256SUMS` only after the workbook has its final bytes.
3. Upload the assets to a draft release. A draft upload is not approval, publication or evidence of immutability.
4. An independent verifier downloads every asset from GitHub rather than reading the builder's local staging directory.
5. The independent verifier checks every checksum, API digest, filename and size; verifies the signed annotated tag; proves the standalone and tagged workbooks have the same SHA-256; inspects GitHub's source archive; and repeats the static and native gates on the downloaded candidate where the required Excel environment is available.
6. Record the independent result in the release evidence. Only after it passes may an authorised maintainer publish the release.
7. Re-query the published release. Claim immutability only when GitHub reports `immutable=true`. If the feature is unavailable, record that limit and retain the no-overwrite and new-version rule.
8. After verification, do not delete or replace assets, move the tag or edit evidence to make a failed candidate appear successful. Correct an error under a new version and tag.
