# ADR 0001: Trusted boundary and proof-toolchain bootstrap

## Status

Accepted for Phase 1 bootstrap.

## Decision

The Rust core begins at owned normalized token bytes, a deterministic
priority-resolved byte DFA, and conflict-free LALR tables. Their language-level
correctness is assumed; structural validation and all runtime transitions are
inside the eventual proof boundary. Regex compilation, tokenizer extraction,
grammar-table generation, serialization, PyO3, Torch, and Transformers remain
outside it.

The corrected lexer contract distinguishes logical start from the DFA start,
reconsumes boundary bytes, and treats EOS separately: clean start emits EOF,
an accepting residual emits its terminal then EOF, and an unfinished residual
rejects.

| Intended Rust module | Lean obligation |
| --- | --- |
| `lexer` | Corrected byte stepping and EOS behavior |
| `token_step` | Direct token composition |
| `sequence` / `spanner` | Singleton heads and inverse spanner |
| `lalr` / `preprocess` | Reduction closure and refinement |
| `mask` / `engine` | Shared membership and atomic advance |

## Frozen Aeneas smoke pins

- Aeneas: `5d08da45a405913bbee6fd544e01debf8154ac9d`
- Charon: `f5208b1c4ce287898a1fc015d41a20da3baf0974`
- Rust for Charon: `nightly-2026-06-01`
- Lean required by Aeneas backend: `leanprover/lean4:v4.31.0`
- UCSD formalization snapshot:
  `ucsd-formal/constrained-decoding-formalization@112af02fa29b3e50c1048c36b4b8d18975dee102`,
  which pins
  `leanprover/lean4:v4.29.0-rc6`.

The two Lean pins are incompatible. No `proofs/` Lake skeleton is created
until a pinned integration project builds with generated Aeneas modules and
the upstream snapshot. The preferred resolution is a documented port of the
UCSD snapshot to Lean 4.31.0; selecting another Aeneas revision requires a new
spike and ADR amendment.

## Current blocker

This host has the needed Rust nightly and Lean 4.31.0, but lacks Aeneas,
Charon, and GNU Make newer than 3.81. The Aeneas source build also needs a
dedicated OCaml 5 switch and dependencies. `just aeneas-smoke` therefore
always fails with an explicit unavailable-tool message until those prerequisites
and a real fixture exist.

The compiler/table-extraction and exact-tokenizer-byte spikes remain unresolved;
no compiler or tokenizer profile is claimed supported by this bootstrap.
