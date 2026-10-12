The Atheris target feeds fabricated text to two parsers in `tools/`: the
module statement splitter in `verify_sources.py`, which every gate uses to read
`src/`, and the help-result number parser in `generate_selftest_examples.py`,
which turns printed examples into self-test checks. The splitter must return
strings, and every line the number parser accepts must give a finite value
with a positive tolerance. Neither may raise. The seed corpus holds the first
4 KiB of each `src/` module, a few printed help results and an empty file.

On Linux with Python 3.14, from the repository root, run:

```sh
python -m pip install --require-hashes --only-binary :all: -r fuzz/requirements-fuzz.txt
python -m fuzz.ozzit_parsers fuzz/corpus -max_total_time=60 -max_len=4096 -timeout=2 -rss_limit_mb=1024
```

The tools use only the standard library, so the harness imports them from
`tools/`. `.github/workflows/fuzzing.yml` runs the target after main pushes,
weekly and on demand, with read-only repository access and a five-minute
timeout; it is not a merge gate. `tools/tests/test_fuzz_harness.py` replays
the corpus in the unit tests. Atheris does not support Windows, so
coverage-guided fuzzing runs on Linux. A successful run is bounded evidence,
not proof that every input is handled.

After changing the pinned version, regenerate the hashes with:

```sh
uv pip compile fuzz/requirements-fuzz.in --python-version 3.14 --python-platform x86_64-unknown-linux-gnu --generate-hashes --output-file fuzz/requirements-fuzz.txt
```
