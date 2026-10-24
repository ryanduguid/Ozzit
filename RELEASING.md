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

The v3.0.0 tracked builder starts from an earlier input that is not committed,
and stops at the v3.0.0 artefacts. Commit 96b9939 holds that builder and the
input's byte length and SHA-256. Post-v3.0.0 passes start
from the committed `ozzit.xlsx` and `src/`. They
do not regenerate the current workbook from the earlier workbook. The current result also
includes a one-off Excel-state-dependent date shift. The consistency checks do not prove
byte-for-byte regeneration of the current workbook. The v3.0.0
builder, the postbuild limits and any future one-off transformation remain
disclosed in `tools/postbuild/README.md` and `CHANGELOG.md`. Do not broaden a
reproducibility claim without a complete regenerator and a byte comparison.

A pull request that changes `ozzit.xlsx` realigns `release/workbook-base.json`
(the SHA-256, byte length, Git blob and last workbook-changing commit) and the
README digest in a separate commit after the workbook commit. Merge that pull
request with a merge commit, never a squash: the manifest names the workbook
commit, and `tools/tests/test_release_bundle.py` fails unless it is the last
commit on the branch to change the workbook. A squash replaces it. Pull requests that change only documentation may be squashed.

Apache-2.0 covers the whole repository, `ozzit.xlsx`, `src/` and `functions.csv` included.
Every source archive must still include `LICENSE` and `NOTICE`.

## Approval and tag

A human maintainer explicitly approves the exact candidate commit and release version after all 18 gates pass.
Create an annotated, cryptographically signed tag for that exact commit. Run
`git verify-tag` and record the tag object SHA, peeled commit SHA and
verification result.

Do not let `gh release create` silently create a lightweight tag from a branch.
Tag signing, remote tag publication and release publication are separate authorised actions; this document does not approve any of them.

## Future release bundle

The uploaded bundle contains exactly 3 files:

1. `ozzit.xlsx`: the consumer workbook copied byte-for-byte from the tagged tree.
2. `provenance.json`: canonical JSON binding the version and full candidate commit to the locked workbook hash, size, Git blob, last workbook change and deterministic workbook-gate results. It also states the copy-only build limit and the gates that remain outside the bundle.
3. `SHA256SUMS`: canonical SHA-256 lines for `ozzit.xlsx` and `provenance.json`. It does not include itself, which avoids a checksum cycle.

The release also carries one asset that is not part of the bundle: `ozzit-X.Y.Z.intoto.jsonl`, which `.github/workflows/attest-release.yml` adds to the draft. It is a Sigstore bundle holding build provenance for `ozzit.xlsx`, signed by `actions/attest` with that workflow's GitHub identity, and it names the workbook's SHA-256, the tag and the commit. `tools/prepare_release_bundle.py` neither creates nor accepts it.

The signed tag and GitHub's generated source archive remain the source distribution. Inspect that archive as the equivalent of `git archive` for the exact tag; do not upload a redundant custom source archive. The standalone workbook and tagged workbook must have the same SHA-256.

After all 18 gates pass in a clean candidate, stage and independently verify the bundle in 2 fresh directories outside the repository:

```powershell
tools/ozzit/target/release/ozzit prepare_release_bundle create --version X.Y.Z --source-commit <full-commit-sha> --output <first-new-directory>
tools/ozzit/target/release/ozzit prepare_release_bundle verify --bundle <first-new-directory> --version X.Y.Z --source-commit <full-commit-sha>
tools/ozzit/target/release/ozzit prepare_release_bundle create --version X.Y.Z --source-commit <full-commit-sha> --output <second-new-directory>
tools/ozzit/target/release/ozzit prepare_release_bundle verify --bundle <second-new-directory> --version X.Y.Z --source-commit <full-commit-sha>
```

Require all 3 files in both directories to be byte-identical. The tool refuses a dirty checkout, a mismatched workbook base, non-canonical version or commit values, an output inside the source repository and any existing output path. It runs commands as argument lists without a shell, stages under a fresh sibling directory and exposes the final name only after structural verification succeeds. It never tags, uploads or publishes anything.

The independent verifier must record and compare GitHub's API `digest` for the uploaded checksum file. The attestation bundle sits outside the checksum set so that the reviewed bundle keeps its fixed 3-file inventory. It signs only the workbook; `provenance.json` and `SHA256SUMS` rest on the independent bundle verification. Verify the attestation separately, as packaging step 6 sets out. Do not add a cosmetic SPDX or CycloneDX inventory to this workbook-only bundle.

## Verification gates

Install the pinned linter, type checker and citation parser, and the pinned Rust
toolchain, in the clean candidate, then build the native tools once:

```powershell
python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-ruff.txt
python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-mypy.txt
python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-citation.txt
rustup toolchain install
cargo build --locked --release --manifest-path tools/ozzit/Cargo.toml
```

Then run the following 16 static and repository gates, the same commands CI runs:

```powershell
python -m ruff check .
python -m mypy --config-file mypy.ini
python -m unittest discover -s tools/tests -v
cargo fmt --manifest-path tools/ozzit/Cargo.toml --check
cargo clippy --locked --manifest-path tools/ozzit/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/ozzit/Cargo.toml
tools/ozzit/target/release/ozzit verify_workbook ozzit.xlsx
tools/ozzit/target/release/ozzit verify_sources ozzit.xlsx src
tools/ozzit/target/release/ozzit verify_signatures src
tools/ozzit/target/release/ozzit verify_previous_names functions.csv
tools/ozzit/target/release/ozzit verify_index ozzit.xlsx src functions.csv
tools/ozzit/target/release/ozzit verify_cover ozzit.xlsx functions.csv CHANGELOG.md
tools/ozzit/target/release/ozzit verify_afe ozzit.xlsx src
tools/ozzit/target/release/ozzit verify_help_spills ozzit.xlsx
tools/ozzit/target/release/ozzit verify_native_evidence
python .github/ci/compare_gates.py
```

All 16 gate commands must exit zero. Record the lint result and substantive type-check, function,
signature, table, example, index, module and test counts rather than only their
exit status.

`ozzit verify_help_spills` is the one static gate that predicts an Excel result: it
lists, without Excel, the cached helps that a source change has left stale. It only
reads. A stale help means the cached-value gate will fail until `ozzit refresh_cache`
has run in Excel and `ozzit sanitise_workbook` after it; a clean report does not
replace the cached-value gate.

Before either native gate, prove that no user Excel process is running. Do not
close or attach to a user's Excel session. Then run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\excel_selftest.ps1 -Path .\ozzit.xlsx
tools/ozzit/target/release/ozzit verify_cache ozzit.xlsx
```

The current acceptance baseline is 1,129 formulas recalculated with zero error cells and 1,192 assertions run with zero failures: the 858 hand-written assertions, 23 of them randomised checks that draw fresh inputs on every run and name the inputs behind any miss, and the 334 that `ozzit generate_selftest_examples` derives from the help. The generated count is a static count of the fragment; record the number the script prints.
The cached-value gate must pass and report its actual comparison count. Record
the Excel version and build. Hash the workbook immediately before and after
both native gates and require the bytes to be byte-identical.

If a reviewed workbook change deliberately changes a formula, assertion or
cached-value count, update the documented expected count in the same candidate
and review that change. Do not silently weaken a floor.

## Packaging, independent verification and immutability

1. Build the exact 3-file bundle from the signed tag into a fresh staging directory outside the repository, then repeat it and compare every byte.
2. Compute `provenance.json` and `SHA256SUMS` only after the workbook has its final bytes.
3. Upload the assets to a draft release. A draft upload is not approval, publication or evidence of immutability.
4. Confirm that immutable releases are enabled (`gh api repos/ryanduguid/Ozzit/immutable-releases` reports `"enabled":true`), then run the attestation workflow on the signed tag: `gh workflow run attest-release.yml --ref vX.Y.Z`. Immutability is a prerequisite: the workflow checks that the release is still a draft just before uploading, and only immutable publication stops an upload that races a maintainer publishing it. Dispatching the workflow is a separate authorised action; it never publishes, and publication waits until the run has succeeded. It stops unless GitHub reports the tag as an annotated, verified signed tag whose signed name is the dispatched tag, on the run's commit; `verify-gates` passed in a trusted `push` or `workflow_dispatch` run of `main` for that commit; exactly one draft release for the tag holds exactly the 3 assets and their API digests match; `tools/prepare_release_bundle.py verify` passes on the downloaded draft; and the draft workbook is byte-identical to the tagged one. It then attests the tagged workbook, verifies the attestation through the API and through the bundle, refuses to continue if the tag, the draft or any of its 3 assets changed since verification, and uploads `ozzit-X.Y.Z.intoto.jsonl` to that same draft by its release ID.
5. An independent verifier downloads every asset from GitHub rather than reading the builder's local staging directory. Keep the 3 bundle files in their own directory for `tools/prepare_release_bundle.py verify`.
6. The independent verifier checks every checksum, API digest, filename and size; verifies the signed annotated tag; proves the standalone and tagged workbooks have the same SHA-256; inspects GitHub's source archive; and repeats the static and native gates on the downloaded candidate where the required Excel environment is available. It also verifies the attestation twice, through the API and with the downloaded bundle:

   ```powershell
   gh attestation verify <bundle-directory>/ozzit.xlsx --repo ryanduguid/Ozzit --source-digest <full-commit-sha> --source-ref refs/tags/vX.Y.Z --signer-workflow ryanduguid/Ozzit/.github/workflows/attest-release.yml
   gh attestation verify <bundle-directory>/ozzit.xlsx --bundle <download-directory>/ozzit-X.Y.Z.intoto.jsonl --repo ryanduguid/Ozzit --source-digest <full-commit-sha> --source-ref refs/tags/vX.Y.Z --signer-workflow ryanduguid/Ozzit/.github/workflows/attest-release.yml
   ```

7. Record the independent result in the release evidence. Only after it passes may an authorised maintainer publish the release.
8. Re-query the published release. Claim immutability only when GitHub reports `immutable=true`. If the feature is unavailable, record that limit, retain the no-overwrite and new-version rule, and do not run the attestation workflow. An immutable release accepts no new asset after publication, so a release published without the attestation bundle never gains one; v3.5.0 and earlier releases have none.
9. After verification, do not delete or replace assets, move the tag or edit evidence to make a failed candidate appear successful. Correct an error under a new version and tag.
