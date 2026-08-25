# Python benchmark fixture v1

`protocol-v1.json` is the canonical definition for the Python/PyO3 benchmark.
Its canonical-JSON SHA-256 is embedded in every corpus and timing record, so a
grammar, generator, operation, or default-scale change makes old results
incompatible instead of silently comparing different work.

The exact-byte generator is deterministic and has no tokenizer dependency:

- ordinary token IDs 0 through 255 contain their corresponding singleton byte;
- later ordinary IDs contain `0xA5` plus the ID as little-endian `u32`; and
- the last vocabulary entry is the sole EOS token and has no ordinary bytes.

The default run covers vocabularies 257, 4,096, 32,000, 50,000, and 128,000.
Release sampling is fixed at 7 compile samples, 1,000 operation samples, and 10
operation warm-ups; compile uses one warm-up. The comparator rejects any other
counts.
The candidate and upstream grammars both recognize one or more arbitrary bytes
followed by EOS. The upstream adapter maps every byte bijectively to the
same-code-point Latin-1 character; it does not decode tokens as text.

Run a candidate benchmark. The adapter first rebuilds the release extension
with the same Python interpreter:

```console
export PYTHON=/path/to/dedicated-benchmark-py311/bin/python
cargo fetch --locked
scripts/benchmark-python.sh --output /private/tmp/greatgramma-candidate.ndjson
```

The result contains raw nanosecond samples, nearest-rank p50/p95/p99 values,
clean-child preparation peak RSS, corpus hashes, the candidate Git revision and
dirty state, environment metadata, and hashes for the native extension and lock
inputs.

To produce the baseline, use that same dedicated Python 3.11 environment and a
clean checkout of `large-loris-models/greatgramma`, detach exactly at
`4c21981386fc6d457efa381d1eb1863623d50fa1`, install its pinned Cython 3.0.11,
Lark 1.2.2, interegular 0.3.3, Torch 2.5.1, Transformers 4.46.2, and llguidance
0.6.27 dependencies. The runner force-rebuilds both upstream extensions before
import, so the baseline cannot reuse an ignored binary from another revision:

```console
scripts/benchmark-python.sh \
  --upstream-checkout /path/to/frozen-greatgramma \
  --output /private/tmp/greatgramma-upstream.ndjson
```

The upstream runner refuses a checkout with a different commit or tracked
modifications, verifies every pinned dependency version, and checks that both
imported Cython modules live inside it. The launchers clear the environment
before Python starts and the protocol permits only a small named set. Both
producers record the complete sanitized environment, use temporary empty build
homes, and recheck source, protocol, runner, environment, and native-artifact
drift before writing results. The candidate build is offline and refuses user
or parent-directory Cargo configuration. It also resolves the Rustup 1.98.0
Cargo/rustc executables explicitly, records their paths, hashes, and verbose
versions, and rechecks them. The comparator requires the current tracked runner
hash rather than accepting two matching stale adapters.
The comparator also refuses a dirty candidate. Candidate and upstream results
must be made by the identical runner hash, protocol hash, corpora, sample
counts, and warm-up counts. The shared `engine_*` operations
measure each implementation's actual acceptance-mask boundary. The upstream
adapter omits only `processor_advance_and_mask`, because that exact Python/PyO3
layer does not exist upstream.

Raw baseline results are retained as run artifacts rather than source files.
The Phase 1 policy is committed in `phase1-thresholds-v1.json`: on the 128k
corpus, candidate compile p95, advance-and-mask p95, and clean-process peak RSS
must each be at most half of the frozen upstream value. The threshold file uses
schema `greatgramma.python-benchmark-thresholds.v1`, selects the frozen
`baseline_source_pin` and exact `protocol_sha256`, and contains entries like:

```json
{
  "corpus_id": "synthetic-exact-byte-v1-vocab-128000",
  "operation": "engine_advance_and_mask",
  "metric": "p95_ns",
  "max_candidate_over_baseline": 0.5
}
```

Preparation memory is gated with operation `compile_prepare_peak_rss` and
metric `peak_rss_bytes`. Incremental RSS remains in the raw record for
diagnosis, but is not the release metric because each implementation's required
runtime imports are part of its clean-process deployment cost.

Then run:

```console
scripts/benchmark-compare.sh \
  --candidate /private/tmp/greatgramma-candidate.ndjson \
  --baseline /private/tmp/greatgramma-upstream.ndjson \
  --thresholds tests/fixtures/benchmark/phase1-thresholds-v1.json
```

The comparison exits 0 when every configured rule passes, 1 for a real
threshold miss, and 2 when any input is absent, unpinned, malformed, or
incompatible. `--self-test` on either script exercises its dependency-free
schema and fail-closed checks without making a performance claim.
