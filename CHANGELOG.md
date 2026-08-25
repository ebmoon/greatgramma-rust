# Changelog

All notable changes to the Rust workspace and Python package are recorded here.
Until a public release policy is adopted, the three Rust crates and the Python
package use the same version.

## Unreleased — 0.1.0

Target Phase 1 label: **Aeneas-translatable; Lean proof in progress**. The
pinned source-built Charon/Aeneas gate translates the complete core, elaborates
the generated Lean module, and checks it against a byte-exact baseline. This is
an implementation constraint and extraction result, not a formal-verification
claim.

### Added

- A dependency-free, `unsafe`-free semantic core with validated normalized
  token, lexer-DFA, and LALR tables; bounded preprocessing; packed next-token
  masks; atomic fixed-row batch advancement; and explicit EOS acceptance.
- A Rust compiler for action-free original Yacc, named restricted byte-regex
  terminals, exact token manifests and the supported strict `tokenizer.json`
  profile. It lowers llguidance's derivre-backed shared regex vector and
  lrtable's Pager LALR tables into the normalized ranked schema, then validates
  and prepares them through the core boundary.
- A typed Python package and private PyO3 extension with stable public error
  codes, an exact-manifest compiler entry point, a fixed-row Transformers
  `LogitsProcessor`, and an intentionally narrow decoder-only `generate`
  wrapper.
- Fail-closed handling for unsupported grammar/regex/tokenizer constructs,
  compilation and preparation limits, sequence discontinuity, invalid tokens,
  empty masks, completed rows, caught native panics, and score-mask failures.
- Compatibility, architecture, Rust API, error-contract, benchmark-protocol,
  and third-party provenance documentation.
- A pinned source-built Charon/Aeneas/Lean extraction gate and reviewed
  byte-exact generated Lean baseline for the complete semantic core.

### Compatibility limits

- The prompt is a snapshot only and is not consumed into grammar state. The
  first callback must equal it exactly; later callbacks must append exactly one
  token to each fixed row.
- Batched generation requires a pad token. Once a row accepts EOS, only that pad
  token may appear in later fixed-row callbacks and it is not committed to the
  grammar state.
- Beam search, multiple return sequences, beam groups, speculative/assisted
  decoding, contrastive search, prompt lookup, continuous batching,
  encoder-decoder models, row reordering, and dynamic row counts are not
  supported by the high-level wrapper.
- Token bytes and EOS IDs must be supplied exactly or extracted from the strict
  supported tokenizer JSON profile. General runtime tokenizer introspection is
  intentionally rejected.

### Required before a Phase 1 release

- Run and retain passing Rust formatting, lint, test, and publication-metadata
  checks on the final commit.
- Add CI coverage for Python 3.10+, native wheel builds, strict type checking,
  Python tests, repeated-session/leak checks, fuzz targets, and release-package
  smoke tests, then retain green results for the final commit. The workflows
  and local gates are implemented; their first PR run is not a retained release
  result.
- Run and retain the frozen same-host candidate/upstream comparison for the
  final commit. The differential corpus, hermetic benchmark harness, and
  numeric regression thresholds are implemented and frozen; no performance
  comparison is claimed without the retained final-commit artifacts.
- Select and add the project license, produce a complete transitive dependency
  notice report from the final Rust/Python lockfiles, and remove publication
  blocks only after an explicit release decision.

### Not included

- General tokenizer conversion, lexer modes, precedence resolution, semantic
  Yacc actions, arbitrary regex syntax, ambiguous equal-priority terminals, or
  all-CFG parsing.
- A proof of compiler correctness, tokenizer extraction correctness, Python or
  PyO3 correctness, or end-to-end constrained-decoding correctness.
- A soundness or completeness theorem for the Aeneas-generated Lean program.
- Published crates or wheels.
