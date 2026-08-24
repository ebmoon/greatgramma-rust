# Phase 1 bootstrap execution

**Source plan:** `superpowers/docs/plans/2026-08-24-110221-01-plan-greatgramma-rust.md`

**Session goal:** Begin Phase 1 by establishing a reproducible workspace, the unskippable normalized-table validation boundary, and the corrected lexer semantics. This is the first executable slice of Outcomes 1–3; it does not claim to complete Phase 1.

## Global constraints

- `greatgramma-core` is dependency-free beyond `std`, uses `#![forbid(unsafe_code)]`, deterministic concrete representations, checked arithmetic/indexing, and structured errors.
- Token bytes, deterministic priority-resolved lexer DFA semantics, and conflict-free LALR-table semantics are trusted only after structural validation; compiler/PyO3/Torch remain outside core.
- Logical lexer start is distinct from `Dfa(dfa_start)`. Boundary bytes are reconsumed. EOS at logical start emits EOF; EOS at an accepting residual emits terminal then EOF; EOS at an unfinished residual rejects.
- No caller-controlled invalid input may panic, silently fall back, or partially construct a validated value.
- Production behavior is added test-first at public boundaries. All changes remain Aeneas-oriented even when the local translator is unavailable.

## Task 1: Scaffold the Phase 1 workspace and freeze bootstrap contracts

- Add the approved durable implementation plan to this branch at the same relative path.
- Create a Rust 2024 workspace pinned to Rust 1.98.0 with `greatgramma-core`, `greatgramma-compile`, and `greatgramma-python` members; the latter two may be minimal compiling placeholders, while core forbids unsafe code.
- Add a `justfile` with working formatting, lint, test, and Aeneas-smoke entry points. The smoke entry point must report a clear unavailable-tool error when Charon/Aeneas/Lake are absent; it must not pretend success.
- Add `pyproject.toml` metadata for a future mixed maturin package without adding Torch/Transformers yet.
- Add CI for format, clippy with warnings denied, and workspace tests on Linux. Do not add release publishing.
- Add `docs/architecture/0001-trusted-boundary.md`, `docs/compatibility/v1.md`, `docs/benchmarks/protocol-v1.md`, `THIRD_PARTY_NOTICES.md`, and a top-level README. Mark unresolved compiler/toolchain spikes explicitly and freeze reference commits/versions already selected by the source plan.
- Add a minimal `proofs/` Lake/Aeneas ownership skeleton only when supported by the toolchain feasibility report; otherwise document the exact blocker and leave `just aeneas-smoke` fail-closed.
- Verify with `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace --all-targets`.

## Task 2: Implement the unskippable normalized-table validation boundary

- In `greatgramma-core`, add concrete ID newtypes, token/lexer/LALR input tables, preparation limits, structured validation errors, and private validated types.
- The public ownership flow is `UnvalidatedGrammar::validate(ValidationLimits) -> Result<ValidatedGrammar, ValidationError>`; no public unchecked constructor creates `ValidatedGrammar`.
- Ordinary tokens are nonempty bytes; multiple token IDs may denote the one EOS semantic event; duplicate ordinary byte spellings are preserved.
- Validate table dimensions and IDs, lexer start/transitions/labels, parser actions/gotos/productions/start/EOF, accept-only-on-EOF, checked cross-products, and cumulative work/byte limits. This task establishes structural—not language-semantic—correctness.
- Write focused integration tests first for valid construction and representative failures, confirm the intended red state, then implement the minimum passing code.
- Verify with `cargo test -p greatgramma-core --test normalized_validation` and the workspace format/lint/test commands.

## Task 3: Implement corrected lexer execution and EOS behavior

- Add proof-shaped lexer state/output/error types and a pure public lexer-step API over `ValidatedGrammar` or a validated lexer view.
- Keep `Start` distinct from `Dfa(dfa_start)`, reconsume a boundary byte after emitting an accepting terminal, reject a nonaccepting residual or a boundary byte that cannot start a new lexeme, and make empty emissions impossible.
- Handle EOS separately: logical start emits parser EOF, accepting residual emits terminal then EOF, unfinished residual rejects; all EOS token IDs share the same event.
- Write red-first public-boundary tests for clean EOS, accepted/unfinished residuals, `(ab)*a` after `ab`, boundary reconsumption, invalid new-lexeme start, and overlapping accepting states with already-resolved priority labels.
- Verify with `cargo test -p greatgramma-core --test lexer_semantics` and the workspace format/lint/test commands.
