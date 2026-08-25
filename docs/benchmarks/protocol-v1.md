# Benchmark protocol v1

## Current status

The repository contains runnable Rust timing/leak checks and a deterministic
Python comparison harness for this implementation and the frozen upstream
GreatGramma revision. The synthetic corpus definition, result schema, and Phase
1 thresholds are checked in under `tests/fixtures/benchmark/`.

Development runs have exercised all five default vocabulary sizes against the
real upstream Cython implementation. Those runs were made from a dirty
candidate checkout and are not release evidence. The comparator deliberately
rejects them. A performance claim requires a clean post-commit candidate run,
a same-host upstream run made by the identical runner, and retained raw NDJSON
plus comparison output.

## Frozen comparison sources

| Source | Pin | Use |
| --- | --- | --- |
| GreatGramma Python | `large-loris-models/greatgramma@4c21981386fc6d457efa381d1eb1863623d50fa1` | Required behavioral and performance baseline |
| llguidance | `guidance-ai/llguidance@c75b0d90e12c941b881566e0a77bb5fa24cd2731` | Informational only; its Earley algorithm is not a like-for-like baseline |
| GreatGramma Rust | exact clean commit plus `Cargo.lock` | Candidate |

## Synthetic corpus

`protocol-v1.json` defines one exact-byte language: one or more arbitrary bytes
followed by EOS. Candidate and upstream use equivalent restricted-Yacc and Lark
grammars. The upstream adapter maps bytes bijectively to same-code-point Latin-1
characters; it never performs tokenizer text decoding.

For every vocabulary, IDs 0–255 are singleton bytes. Later ordinary IDs are
`0xA5` followed by the ID as little-endian `u32`; the final ID is EOS. The
runner accepts only a clean tracked source state, materializes the exact Git
commit into a fresh temporary tracked-files-only tree, and builds and imports
there. Caller-checkout ignored artifacts therefore cannot enter Python's import
path. It records hashes of the grammar, token stream, protocol, runner, native
extension, and lock inputs, then rechecks source state and those executable
inputs before emitting a result. Default sizes are:

| Corpus ID suffix | Vocabulary |
| --- | ---: |
| `vocab-257` | 257 |
| `vocab-4096` | 4,096 |
| `vocab-32000` | 32,000 |
| `vocab-50000` | 50,000 |
| `vocab-128000` | 128,000 |

This scaling corpus complements, rather than replaces, the independent
conformance oracle and bounded exhaustive tests.

## Measurements

The Python runner records raw samples and nearest-rank p50/p95/p99 values for:

- source compilation plus core preparation;
- initial acceptance-mask access;
- one legal token advance plus successor acceptance mask; and
- the candidate's trusted fixed-append Python processor with a dependency-free
  masker that consumes the packed bytes.

Release evidence uses exactly 7 compile samples, 1,000 operation samples, and
10 operation warm-ups (compile uses one warm-up). Smaller diagnostic runs are
not accepted by the comparator.

Preparation peak RSS is measured in one clean child per implementation and
corpus with `resource.getrusage`. Both total and incremental values are
retained. The release metric is total clean-process peak RSS because required
runtime imports are part of deployment cost.

Initial-mask results are diagnostic only. Frozen upstream exposes an already
cached acceptance dictionary while the candidate materializes packed mask
bytes, so that operation is not used for a comparative release threshold.
Torch expansion/device transfer and model generation throughput are outside
this engine comparison.

The Rust harness separately measures normalized-table preparation, packed mask,
and advance latency at the same scaling points. Its 100,000-step repeated-
session leak gate permits at most 1,024 KiB first-to-last growth and an
ordinary-least-squares RSS slope of at most 8 bytes per step.

## Phase 1 comparison gate

`phase1-thresholds-v1.json` requires the candidate's 128k-vocabulary:

- compile/preparation p95;
- advance-and-successor-mask p95; and
- clean-process preparation peak RSS

to each be no more than `0.5` times the frozen upstream value. A failed or
different supported mask invalidates the comparison rather than becoming a
faster sample. The comparator also refuses dirty or unpinned sources,
protocol/runner/corpus drift, different host/Python metadata, incomplete raw
samples, mismatched sampling settings, build-affecting environment overrides,
or any upstream dependency version outside the exact protocol map. The
launchers clear the environment before Python starts, enable Python isolated
mode, and admit only `HOME`, `PATH`, `TMPDIR`, fixed C locale values, their
sanitization marker, and macOS's interpreter-added user-text-encoding value.
Every run binds the complete sanitized environment and its hash, and the
comparator requires exact candidate/upstream equality. Builds additionally use
a fresh empty `HOME`; the candidate exposes only its explicit Cargo/Rustup
caches, works offline, and refuses user or parent-directory Cargo
configuration. Candidate builds resolve Cargo and rustc through the pinned
Rustup 1.98.0 toolchain, retain their executable hashes and verbose versions,
and recheck that provenance after the build. The comparator accepts only
results produced by the runner in the current source tree.

Once the first clean result passes, retain it as the regression baseline. A
greater than 10% regression from that candidate baseline requires a reproduced
measurement and an explained benchmark-policy change.

## Commands

Create one dedicated Python 3.11 environment containing Maturin plus the exact
upstream dependencies below, and use that same interpreter for both producers
and the comparator. Fetch the locked Rust dependencies before entering the
offline benchmark build. The adapter materializes the clean candidate commit
and rebuilds the release extension in that fresh tree, so an ignored or stale
native artifact cannot satisfy the gate:

```sh
cargo fetch --locked
export PYTHON=/absolute/path/to/benchmark-py311/bin/python
scripts/benchmark-python.sh --output /private/tmp/greatgramma-candidate.ndjson
```

That environment must contain the exact Cython 3.0.11, Lark 1.2.2, interegular
0.3.3, Torch 2.5.1, Transformers 4.46.2, and llguidance 0.6.27 versions recorded
in the protocol. The adapter checks those versions, materializes the pinned
upstream commit, and force-rebuilds both upstream Cython extensions in that
fresh tree before importing them:

```sh
scripts/benchmark-python.sh \
  --upstream-checkout /absolute/path/to/greatgramma-at-4c219813 \
  --output /private/tmp/greatgramma-upstream.ndjson
```

Compare only same-host results:

```sh
scripts/benchmark-compare.sh \
  --candidate /private/tmp/greatgramma-candidate.ndjson \
  --baseline /private/tmp/greatgramma-upstream.ndjson \
  --thresholds tests/fixtures/benchmark/phase1-thresholds-v1.json \
  --output /private/tmp/greatgramma-comparison.ndjson
```

Both scripts support `--self-test`. Raw run and comparison files are CI/release
artifacts, not source-controlled benchmark claims.

llguidance numbers, if added later, must be labeled informational and must not
be presented as though its live Earley masking shared this implementation's
semantics.
