The Rust target feeds fabricated text to the native module statement splitter
and help-result number parser. Neither may panic, and every accepted number
must have a positive exact tolerance. Rust's return types guarantee strings
and finite decimals. Invalid UTF-8 is decoded with replacement, as in the
temporary Python reference target. The seed corpus holds the first 4 KiB of
each `src/` module, a few printed help results and an empty file.

On Linux, from the repository root, run:

```sh
rustup toolchain install nightly-2026-10-15 --profile minimal
cargo install cargo-fuzz --locked --version 0.13.2
cargo fetch --locked --manifest-path tools/ozzit/fuzz/Cargo.toml
corpus=$(mktemp -d)
cp fuzz/corpus/* "$corpus/"
cd tools/ozzit
CARGO_NET_OFFLINE=true cargo +nightly-2026-10-15 fuzz run parsers "$corpus" -- -max_total_time=60 -max_len=4096 -timeout=2 -rss_limit_mb=1024
```

Use an owned corpus copy because libFuzzer adds generated seeds to it. Inspect
and retain useful failures before removing that copy. The target and its
separate Cargo lockfile live in `tools/ozzit/fuzz/`; ordinary stable Rust tests
in `tools/ozzit/tests/fuzz_corpus.rs` replay the same harness and all committed
seeds, plus invalid-byte, line-boundary and very small decimal cases.

`.github/workflows/fuzzing.yml` runs the target after main pushes, weekly and
on demand, with read-only repository access and a 20-minute job timeout. Its
random exploration lasts 60 seconds and is not a merge gate. A successful run
is bounded evidence, not proof that every input is handled.

`fuzz/ozzit_parsers.py`, its hash-pinned Atheris requirements and
`tools/tests/test_fuzz_harness.py` remain as temporary Python reference checks
until the wider Python retirement is qualified. The active fuzzing workflow
uses Rust.
