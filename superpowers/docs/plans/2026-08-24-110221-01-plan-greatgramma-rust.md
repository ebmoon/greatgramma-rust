# GreatGramma Rust implementation plan

**Goal:** Build a fast, exact Rust reimplementation of GreatGramma with a Hugging Face `LogitsProcessor` interface, while keeping the semantic kernel small enough to translate with Aeneas and prove in Lean.

**Why planning is required:** The project couples lexer/token alignment, LALR reduction semantics, Python generation state, performance-sensitive preprocessing, and a later cross-language proof. Several plausible implementation choices would silently change the accepted language or make the Rust core untranslatable, so the proof boundary and failure behavior must be fixed before production code.

**Acceptance:** Phase 1 ships a fail-closed Python package whose masks exactly match a slow executable specification for the supported grammar/tokenizer profile, whose every allowed token commits successfully through the same core transition relation, and whose Rust hot path improves on the frozen Python GreatGramma baseline without its memory growth. Phase 2 continuously translates `greatgramma-core` with pinned Charon/Aeneas tooling and proves the published soundness/completeness claim relative to semantically correct normalized token bytes, lexer DFA, and LALR tables.

## Locked decisions

- The theorem boundary starts at normalized token-byte entries, a deterministic priority-resolved lexer DFA, and a conflict-free LALR table. Their language-level semantic correctness is assumed; `greatgramma-core` still validates dimensions, IDs, determinism, required profiles, resource limits, and all properties it can check finitely.
- `greatgramma-core` owns every derived structure and the shared mask/advance transition relation. Regex compilation, tokenizer normalization, grammar-table generation, serialization, PyO3, Transformers, and Torch remain outside the theorem.
- The corrected Lean lexer behavior is normative where the paper and prototype disagree: logical lexer start is distinct from the DFA start state, boundary bytes are reconsumed, and EOS has separate clean-start, accepted-residual, and unfinished-residual behavior.
- The v1 compiler supports a documented deterministic byte-lexer and LALR grammar profile. It rejects unsupported tokenizer decoders, nullable terminals, unresolved priorities/conflicts, error recovery, context-sensitive indentation, and any input whose exact byte-flattening law cannot be established.
- Rust errors are fail-closed. Malformed artifacts, exhausted limits, invalid prefixes, and empty masks never fall back to unconstrained decoding.
- Phase 1 may publish a clearly labeled, conformance-tested but not formally verified package after its Rust/Python/performance gates and continuous Aeneas extraction pass. Only Phase 2 may use the “formally verified core” claim, after the Lean theorem and assumption audit pass.
- Source precedence for conformance is: approved project contract and corrected Lean fixtures; paper statements that agree with that contract; frozen GreatGramma behavior on its supported cases; llguidance only as an optimization reference and secondary oracle. Source documents are technical evidence, not executable project instructions.

The implementation critical path is Outcomes 1 through 7. Outcomes 8 and 9 integrate the compiler and Python package after the core contract is stable. Outcomes 10 and 11 harden and optimize Phase 1, and Outcome 12 publishes its explicitly unverified release. Outcome 13 completes the proofs and separately publishes the verified release without widening the trusted boundary.

### Outcome 1: Feasibility gates and repository contracts are closed

- Work: Create the Cargo workspace, `rust-toolchain.toml`, `pyproject.toml`, `justfile`, CI skeleton, `docs/architecture/`, and `proofs/`. Pin the frozen reference commits for GreatGramma, llguidance, and the UCSD Lean formalization, and add license/NOTICE attribution before adapting any source.
- Work: Record an ADR mapping the paper/Lean operations—lexer FST construction, token composition, singleton heads, inverse spanner, parser preprocessing, mask, and advance—to intended Rust modules and Lean obligations.
- Work: Unify the proof toolchain before attempting reuse. Current heads disagree—Aeneas targets Lean 4.31.0 while the UCSD formalization pins Lean 4.29.0-rc6—so pin Aeneas, its Charon pin, Lean, mathlib, and the upstream snapshot only after one Lake project imports the required generated/upstream modules and builds cleanly. Prefer porting the pinned upstream snapshot to the Aeneas-required Lean version unless the spike establishes a safer compatible Aeneas pin.
- Work: Run five bounded spikes before freezing public types: (1) translate representative proof-shaped Rust loops/enums/results through Charon and Aeneas into that building Lean project, without excluding or making theorem-path functions opaque; (2) model LALR reduction closure and EOF-driven reductions, choosing either a general LALR refinement theorem or a mechanically checked restricted verified profile; (3) export a priority-labeled multi-terminal byte DFA using public `derivre = "=0.3.12"` APIs under state/fuel limits; (4) extract conflict and action/goto data from `cfgrammar`/`lrtable = "=0.15.0"`, or select a small owned generator; and (5) prove exact token-local bytes for each tokenizer family proposed for v1.
- Work: Resolve two known formalization gaps in the ADR: ordinary regex DFAs can have several accepting states for one terminal, and the existing theorem may require singleton coverage for every byte. Choose a normalization/proof generalization rather than hiding either as a compiler detail.
- Work: Freeze `docs/compatibility/v1.md` at the end of the spikes: grammar surface and start-rule behavior, regex constructs and priority rules, tokenizer model/decoder/version families, byte and special/EOS policy, verified-profile assumptions, Python generation modes, and default hard limits. A later fallback may not shrink this profile without an explicit plan/ADR amendment and user approval.
- Work: Before freezing the schema, run one throwaway compile→validate→prepare→mask/advance vertical slice with a real supported tokenizer/grammar and a representative 128K-token synthetic vocabulary. Pre-register maximum preparation time, cumulative work, peak RSS, derived bytes per input byte, and sequence/bucket counts; failure changes the design before Outcomes 2–7 rather than after APIs/proofs harden.
- Work: Freeze `docs/benchmarks/protocol-v1.md` with corpus hashes, upstream/environment pins, hardware controls, warm-up/repeat counts, statistical comparison, minimum required latency/RSS improvement, and retained-bytes/RSS-slope leak thresholds. Threshold changes require an explained benchmark ADR.
- Risks/open questions: General LALR reduction closure does not directly fit the existing real-time PDA step, and current Aeneas/Lean versions may not match the formalization's toolchain. A failed spike must select and document a conservative fallback; it must not be waived.
- Verify: `just feasibility`
- Verify: `just aeneas-smoke`
- Verify: `cargo metadata --no-deps`

### Outcome 2: The normalized core schema is safe, finite, and explicit

- Work: Add `crates/greatgramma-core` with `#![forbid(unsafe_code)]` and proof-oriented modules `ids.rs`, `normalized.rs`, `limits.rs`, `validate.rs`, and `error.rs`. Use concrete `u32` newtypes, enums, `Vec`/slices, deterministic ordering, checked arithmetic/indexing, and no third-party runtime dependency.
- Work: Define `UnvalidatedGrammar` from a token table, lexer table, parser table, explicit parser EOF, skip/separator policy, and preparation limits. Ordinary token entries carry nonempty raw bytes; one or more token IDs may alias the single EOS event. Duplicate ordinary byte spellings remain distinct token IDs.
- Work: Represent the lexer as a byte-class map, flat transition rows, a DFA start, and at most one already-priority-resolved terminal label per state. Represent LALR actions as error/shift/reduce/accept, gotos separately, and productions as left-hand side plus pop count. Keep serialization DTOs and third-party IDs outside core.
- Work: Validate table shapes and ranges, start/EOF/EOS data, checked offset sizes, reachable/co-reachable lexer states, nonnullable lexemes, terminal and byte-singleton coverage required by the verified profile, parser reachability/gotos, accept-only-on-EOF, no recovery actions, declared conflict-free provenance, any untrusted reduction-progress witness selected in Outcome 1, and all configured budgets. Validation establishes structural well-formedness only and says so in the public API and theorem statement.
- Work: Make validation an unskippable ownership transition: `UnvalidatedGrammar -> ValidatedGrammar -> PreparedGrammar`. Validated/prepared fields and constructors are private, and every preparation/runtime API accepts only the validated/prepared type; no public unchecked constructor or deserializer exists.
- Work: Make caller-data failures structured and path-specific. Preflight every cross-product with checked arithmetic and cumulative byte/work budgets, use fallible reservation before growth, and distinguish limit exhaustion from allocation failure. No validation or runtime path may panic, assert, unwrap caller data, partially mutate state, or grow beyond explicit cumulative and per-dimension budgets. Test every cap boundary plus injected reservation failure in the non-proof allocation wrapper.
- Verify: `cargo test -p greatgramma-core --test normalized_validation`
- Verify: `just aeneas-core`

### Outcome 3: Corrected lexer execution has an executable specification

- Work: Add `lexer.rs` and a deliberately slow reference implementation used by tests and later proofs. Model logical `Start` separately from `Dfa(dfa_start)` so empty input and a lexeme that returns to the DFA start cannot be confused.
- Work: Define the one-byte transition once: continue the current DFA path when possible; otherwise, if the current state accepts a terminal, emit it and reconsume the same boundary byte from a new lexeme; reject if the residual is nonaccepting or the reconsumed byte cannot begin a lexeme. Empty lexemes are impossible.
- Work: Define EOS outside ordinary byte execution. EOS at logical start emits parser EOF; EOS at an accepting residual emits its terminal followed by EOF; EOS at an unfinished residual rejects. Multiple EOS token IDs use this same semantic event.
- Work: Add red-first fixtures for clean EOS, accepted and unfinished residuals, `(ab)*a` after `ab`, boundary reconsumption, invalid boundary starts, keyword/identifier priority, overlapping accepting states, maximal munch within the supported one-byte-boundary profile, skip terminals, and no empty emission loop.
- Risks/open questions: Longer-context lexer decisions and indentation-sensitive lexing are deliberately out of v1; the compiler must diagnose them rather than approximating them.
- Verify: `cargo test -p greatgramma-core --test lexer_semantics`
- Verify: `just aeneas-core`

### Outcome 4: Model tokens compose exactly with the lexer

- Work: Add `token_step.rs`. For each relevant lexer state and token ID, call the direct token relation and store its result in an ordinary nested `Vec`; ordinary steps contain only the emitted terminal sequence and destination lexer state, while EOS steps remain separate.
- Work: Preserve duplicate token IDs, enforce two coarse representation-independent preparation limits, and centralize fallible vector growth behind one allocation seam. Keep this direct table as the proof-shaped implementation; trie prefix sharing, output pools, and row interning are Outcome 11 optimizations only after measurement.
- Work: Expose one pure `execute_token` relation to both speculative masking and definitive commit. A failed token step returns no successor and leaves the caller's state unchanged.
- Work: Test tokens that emit zero, one, or several terminals; merged tokens crossing lexeme boundaries; duplicate byte spellings; NUL/non-UTF-8 bytes; multiple EOS IDs; and the llguidance-style mask/commit boundary-spanning regression.
- Verify: `cargo test -p greatgramma-core --test token_composition`
- Verify: `just aeneas-core`

### Outcome 5: Sequence heads and the inverse token spanner are exact

- Work: Add `sequence.rs` and `spanner.rs`. Compute exact singleton-producible terminal heads with a terminating shared worklist, then form each realizable sequence as a token's direct emissions followed by exactly one hypothetical continuation head.
- Work: Make the continuation head explicitly speculative: parser preprocessing may inspect it to decide whether a partial lexeme can eventually finish, but `advance` commits only the direct emissions and lexer destination of the selected token.
- Work: Intern finite sequence heads with a direct deterministic scan and build sparse nested-`Vec` inverse buckets from `(source lexer state, sequence ID)` to token IDs. Close zero-output cycles with an append-only fact worklist, and reject excessive sequence/data growth under the same two coarse limits.
- Work: Differential-test the worklist, sequence pool, and inverse buckets against a small exhaustive enumerator, including reconverging DFA paths, cycles, tokens with no direct emission, and the regression where an appended head was accidentally committed.
- Verify: `cargo test -p greatgramma-core --test spanner_semantics`
- Verify: `just aeneas-core`

### Outcome 6: LALR semantics and parser preprocessing agree with a full interpreter

- Work: Add `lalr.rs`, `lalr_reference.rs`, and `preprocess.rs`. The full interpreter repeatedly performs reductions for a concrete lookahead, then shifts, accepts, or rejects. Nullable productions and EOF-triggered reduction chains are supported. Each reduce action carries a `u32` progress rank; after applying a reduction, another reduction must have a lower rank, or the same rank only when the current production pops at least two states. This makes `(rank, stack length)` decrease without a user-tunable completeness fuel, supports nullable and right-recursive chains in the v1 profile, and turns a bad witness into a structured invariant error.
- Work: Define the Lean-side LALR semantic model before optimizing the Rust partial evaluator. Do not equate one LALR action with the existing PDA theorem's one terminal-consuming step without a proved reduction-closure bridge.
- Work: Partially evaluate each interned sequence against parser states, classifying it as always readable, rejected, or dependent on the deeper runtime stack. Keep the first representation as a direct nested-vector table. Expose only the table query at this layer; Outcome 7's single opaque prepared owner resolves dependent cells with the exact slow full-stack head probe so independently prepared grammar/spanner/parser artifacts cannot be mixed. Tries, compiled stack programs, and stored suffix/remainder programs are deferred until Outcome 11 measurements justify them.
- Work: Preserve the ordinary-head boundary explicitly: consume the direct prefix, probe the final continuation terminal for readability, then discard both its shift and any reductions driven by that speculative lookahead. Add an explicit validated skip-terminal set backed by constant-time membership; skip terminals are parser no-ops and parser EOF cannot be skipped. Enforce preparation work limits inside reduction closure before each charged lookup or stack growth, not after a whole cell finishes.
- Work: Ensure EOF is accepted only after the lexer EOS event and all required parser reductions reach `Accept`. Cycles, invalid progress witnesses, missing gotos, exhausted safety budgets, or impossible stack pops are structured preparation/runtime errors rather than acceptance or ordinary rejection.
- Work: Exhaustively compare partial evaluation with the full interpreter over bounded tiny tables/stacks. Cover several reductions before shift, reductions that need deeper stack, nullable rules, right recursion, stack-dependent close delimiters, EOF reductions, accept/reject/dependent partitioning, and reduction cycles.
- Work: Test valid reduction chains exactly at the derived bound and invalid witnesses/chains beyond it. The completeness theorem and supported runtime profile contain no independent fixed-fuel assumption.
- Risks/open questions: If a precise finite partial-evaluation representation cannot cover the chosen LALR profile, Phase 1 must retain the exact slow per-token interpreter until a correct representation exists; performance cannot justify an under-approximate or over-approximate mask.
- Verify: `cargo test -p greatgramma-core --test parser_semantics`
- Verify: `just aeneas-core`

### Outcome 7: The stateful core matcher is exact and fail-closed

- Work: Add `mask.rs` and `engine.rs`. `prepare(ValidatedGrammar, PrepareLimits)` returns an immutable `PreparedGrammar`; its initial state contains logical lexer start and the parser start stack. The stable functional API provides caller-buffer mask computation, single-token allowance, and atomic advance returning continuing versus accepted state. A mutating convenience wrapper stays outside the translated kernel.
- Work: Build masks by combining preaccepted token buckets with dependent sequence buckets resolved against the actual LALR stack. `mask`, `allows`, and `advance` must call the same direct token and parser relations; mask computation is pure and rejected advance leaves the original state unchanged.
- Work: Treat completion, invalid token IDs, tokens after completion, dead ends, undersized buffers, corrupt prepared data, and exhausted limits as distinct results. An empty legal set is a `NoValidToken` condition, never an all-allowed mask.
- Work: Establish the central property on generated and adversarial fixtures: a token bit is set if and only if `advance` succeeds from that state; an EOS bit is set if and only if its advance reaches parser acceptance. Compare every optimized mask with the slow per-token oracle.
- Verify: `cargo test -p greatgramma-core --test mask_advance`
- Verify: `cargo test -p greatgramma-core --test bounded_exhaustive`
- Verify: `just aeneas-core`

### Outcome 8: The compiler produces only supported normalized grammars

- Work: Add `crates/greatgramma-compile` with `source.rs`, `tokenizer.rs`, `regex.rs`, `dfa.rs`, `grammar.rs`, `lr.rs`, `compile.rs`, and `error.rs`. Its stable API accepts an owned source grammar plus exact tokenizer JSON/byte manifest; normalization returns core-owned `UnvalidatedGrammar`, while the ordinary compile path validates and returns `PreparedGrammar`. Only core validation can create `ValidatedGrammar`, and no `derivre`, `cfgrammar`, `lrtable`, or tokenizer-library type crosses the boundary.
- Work: Define and document the first grammar syntax as a deliberate Lark-like subset translated to an owned CFG. If the Outcome 1 table spike cannot support that surface reliably, ship the normalized/Yacc input first and keep the Lark frontend behind an explicit experimental flag rather than reading Lark private tables.
- Work: Compile each terminal with public `derivre` APIs and construct a bounded priority-labeled product DFA. Reject nullable/search/unsupported lookaround or complement constructs, ambiguous priority, excess state growth, and cases needing more than the declared boundary context.
- Work: Generate/extract LALR tables, reject every shift/reduce or reduce/reduce conflict and recovery action, then immediately copy actions/gotos/rules into the owned schema. Initially reject precedence declarations if the selected library hides conflicts that it resolved.
- Work: Normalize exact token bytes without `decode()` heuristics. Whitelist tokenizer model/decoder pipelines whose token concatenation is byte-homomorphic; reject empty ordinary tokens, path-dependent decoding, missing verified singleton-byte coverage, unsupported added/special-token behavior, and mismatched EOS IDs. Emit a human-readable compatibility report alongside structured errors.
- Work: Treat prepared caches as derived and recompute them after load in v1. If a versioned artifact format is added later, validate all untrusted base data before constructing core types and never deserialize unchecked prepared indices.
- Verify: `cargo test -p greatgramma-compile`
- Verify: `cargo test -p greatgramma-compile --test tokenizer_bytes`
- Verify: `cargo test -p greatgramma-compile --test grammar_tables`

### Outcome 9: Python exposes a strict Transformers integration without duplicating semantics

- Work: Add `crates/greatgramma-python` for PyO3 and `python/greatgramma` for the typed facade, compiler convenience, packed-mask expansion, and `greatgramma.transformers.GreatGrammaLogitsProcessor`. Configure the native module as private `greatgramma._native`; Torch remains entirely in Python.
- Work: Make the canonical public compiler accept grammar text/path, exact tokenizer JSON, start rule, and a nonempty EOS-ID set. A `from_transformers` convenience may serialize a supported fast tokenizer's backend JSON, but must never infer bytes by decoding individual IDs or the original `"a" + token` workaround.
- Work: Make an owned GreatGramma generation session/wrapper the supported Transformers entry point. It validates the resolved `GenerationConfig`, decoder-only model, initial prompt/batch shape, EOS/padding settings, processor ordering, `num_beams = 1`, `num_return_sequences = 1`, and absence of assisted/contrastive/continuous-batching modes before it constructs and installs the processor as the last logits processor.
- Work: Keep `GreatGrammaLogitsProcessor` usable as the requested low-level Python `LogitsProcessor`, but label direct construction as an advanced API with explicit preconditions because its callback sees only `input_ids` and scores. It fail-closes on observable row-count, prefix, length, reuse, and concurrency discontinuities but does not claim it can infer hidden generation mode or ancestry.
- Work: Give the processor an explicit decoder-only prompt snapshot. The prompt is a boundary and is not consumed by the grammar. Each fixed-position row owns an independent core state; subsequent calls must extend it by exactly one token. Commit all rows atomically before asking Rust for the next masks. Add beam support only in a later plan with an explicit ancestry/fork protocol and dedicated tests.
- Work: Require generation EOS settings to match the compiled EOS set. After a row accepts EOS, only the configured pad token is permitted for that row and is not committed to the grammar. Batches larger than one require padding. Raise before returning an all-negative-infinity row.
- Work: Define the private mask ABI as owning row-major bytes of shape `rows × ceil(vocab_size/8)`, least-significant bit first, one meaning allowed, and zeroed tail bits. Expand through a cached 256-by-8 Torch lookup table on the score device and apply an out-of-place `-inf` mask while preserving shape, floating dtype, and device. Cross FFI and transfer once per batch, not once per row.
- Work: Publish stable Python exceptions for compile, tokenizer compatibility, configuration, sequence discontinuity, constraint violation, no-valid-token, and internal errors, each with a machine-readable code and optional row/token/position. Copy Python inputs before releasing the GIL, release it around native work, and never expose partial batch mutation. Build the extension with unwind support and wrap every exported operation in an explicit panic-to-`InternalError` boundary rather than exposing PyO3's `PanicException`.
- Work: Use a mixed maturin package with typing/stubs and a private native ABI. Target CPython 3.10–3.14 with `abi3-py310`; keep Torch/Transformers optional and CI-pinned to declared ranges. Do not promise pickling or serialized compiled grammars in v1.
- Verify: `uv run maturin develop --release`
- Verify: `uv run pytest tests/python -q`
- Verify: `uv run pytest tests/python/test_panic_boundary.py -q`
- Verify: `uv run pyright python/greatgramma`

### Outcome 10: Conformance, fuzzing, and end-to-end generation block semantic regressions

- Work: Add `tests/fixtures`, `tests/oracles`, and `tests/integration` with provenance metadata. Freeze corrected examples from the paper/Lean development, supported common cases from GreatGramma, and selected llguidance regressions; record expected disagreements instead of silently updating snapshots.
- Work: Implement the executable oracle as an independent direct interpreter over raw normalized token bytes, DFA transitions, LALR actions/gotos, and EOS. It may share data types and fixtures, but must not call production lexer stepping, token composition, preprocessing, mask, or advance helpers. Mutation tests deliberately corrupt each production relation and must be caught by oracle disagreement.
- Work: Add property and bounded-exhaustive generators for normalized DFAs/LALR tables, token vocabularies, parser stacks, and token histories. Check preparation determinism, no panics on malformed input, resource-limit termination, mask/reference equality, mask/advance equivalence, atomic rollback, and replay equivalence of incremental matcher state.
- Work: Differential-test exact tokenizer bytes against authoritative whole-sequence decoding for every supported family, including whitespace, byte fallback, added tokens, NUL, non-UTF-8 bytes, and special/EOS behavior.
- Work: Integration-test greedy and sampling generation, fixed divergent batches, multiple EOS IDs, early-finished rows and padding, every packed-mask byte boundary, CPU floating dtypes, supported CUDA dtypes when available, wrong vocabulary width, processor ordering, and every explicitly unsupported generation mode failing closed.
- Work: Run long-lived compile/session churn and at least 100,000 matcher steps under the frozen retained-bytes/RSS-slope rule to catch the upstream-style leak. Fuzz untrusted normalized/artifact inputs and the actual Python ABI—including panic/failure injection—without permitting an escaped panic, partial mutation, or unconstrained result.
- Verify: `cargo test --workspace --all-targets --all-features`
- Verify: `cargo fuzz run normalized_input -- -max_total_time=60`
- Verify: `just conformance`
- Verify: `just leak-check`

### Outcome 11: Measurement-guided representations meet the Phase 1 performance gate

- Work: Add reproducible small, medium, and 32K/50K/128K-vocabulary benchmark corpora. Measure preparation wall time, peak RSS, derived table size, cold/hot mask p50/p95/p99, advance latency and allocations, FFI/buffer cost, CPU-to-device transfer, Torch bit expansion/masking, and end-to-end generation throughput.
- Work: Freeze a supported Python GreatGramma benchmark and keep llguidance results informational because it implements a different Earley strategy. Apply the pre-registered Outcome 1 thresholds to the first correct Rust candidate; after it passes, record that run as the regression baseline for subsequent changes.
- Work: Optimize one representation at a time—flat trie subtree skipping, interned rows/sequence pools, offset-based adjacency, deterministic packed bitsets, and bounded state-result caches—while retaining the proof-shaped reference path and requiring differential equality after every change. Do not replace the planned inverse-spanner/LALR algorithm with llguidance's live Earley masking.
- Work: Phase 1 passes performance only when `advance` performs no post-construction allocation, caller-supplied mask computation performs no mask allocation, and the pre-registered latency, preparation-RSS, retained-bytes/RSS-slope, repeatability, and statistical-improvement thresholds from Outcome 1 pass on the frozen corpus/environment. After that baseline, a greater than 10% time or memory regression requires an approved benchmark update.
- Risks/open questions: CUDA transfer and Python mask expansion may dominate after native optimization; profile the layers independently before adding a Torch-native extension outside core.
- Verify: `cargo bench --workspace`
- Verify: `just benchmark-python`
- Verify: `just benchmark-compare`

### Outcome 12: Phase 1 ships with an auditable, explicitly unverified trust boundary

- Work: Document the frozen grammar syntax, regex/tokenizer profiles, EOS/prompt/batch rules, owned-generation versus low-level-processor contract, limits, error codes, benchmark corpus, compiler trust boundary, and known differences from GreatGramma and llguidance. Include a minimal Rust normalized-table example and a Python Transformers example.
- Work: Publish Rust API docs, Python typing, changelog/version policy, third-party attribution, and a compatibility matrix. Version the Rust crates and Python distribution together; the private native ABI ships in lockstep with the facade.
- Work: Build and smoke-test an sdist plus abi3 wheels for the declared Linux, macOS, and Windows targets in clean environments, including base import without Torch and the optional Transformers extra.
- Work: Label this release “Aeneas-translatable; Lean proof in progress,” not “formally verified.” Its gate is green Rust/Python conformance, fuzz, leak, benchmark, packaging, generated-Lean elaboration, and extraction-drift checks; incomplete handwritten semantic proofs do not block Phase 1 and cannot be implied by its documentation.
- Verify: `cargo fmt --all -- --check`
- Verify: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Verify: `just ci-phase1`
- Verify: `just package-check`

### Outcome 13: Aeneas translation and Lean proofs establish and release the conditional theorem

- Work: Keep `greatgramma-core` translatable from Outcome 2 onward and gate every core change with pinned Charon/Aeneas extraction. Use concrete enums/structs, indexed loops, explicit measures, sorted vectors/bitsets, and `Result`/`Option`; avoid unsafe code, concurrency/interior mutability, trait objects, hash maps/random iteration, implicit recursion, panic paths, and unsupported nested control flow.
- Work: Separate Aeneas-owned output and external templates under `proofs/Generated/` from handwritten external models, specifications, LALR bridges, invariants, and theorems under `proofs/GreatGramma/`. Maintain a generated-file manifest; regenerate into a temporary directory and replace only listed generated files. Pin Charon, Aeneas, Lean, mathlib, and the upstream formalization commit; audit every external model and permitted transitive axiom.
- Work: Define a pure normalized-table semantics and prove structural validation sufficient for safe execution, not semantic compiler correctness. State DFA labeling/token bytes/LALR language correctness as hypotheses exactly where used.
- Work: Prove, in dependency order: successful validation establishes safe dimensions/indices/progress invariants; public calls on validated artifacts and reachable states cannot panic or fail internally; corrected lexer/EOS execution; direct token composition; singleton-head and inverse-spanner membership; LALR reduction closure and partial-evaluator refinement; shared mask/advance membership; incremental matcher state equals replay; and packed/dense representation refinement to the proof-shaped implementation.
- Work: Prove the public result as two linked theorems: the Aeneas translation returns exactly the reference mask/successor and permits EOS exactly when EOF reduction closure accepts; then, under correct token-byte flattening, maximal-munch DFA, LALR-language, reduction-progress, nonempty/singleton-token, prunedness, and separator/no-op hypotheses, a mask bit is set exactly when the token prefix has an EOS-accepted continuation in the normalized language. Do not reuse the existing real-time PDA theorem for arbitrary LALR without the Outcome 1 bridge.
- Work: CI rejects handwritten `sorry`/`admit` or project-local unaudited axioms, stale generated output, translation drift, opaque/excluded theorem-path functions, or a theorem statement that silently adds assumptions. Keep an assumption inventory in generated documentation and label any restricted verified profile separately from broader runtime support.
- Work: Publish a separate verified release only after all Phase 1 gates remain green, the headliner theorems build, the assumption inventory matches the frozen claim, and release notes state exactly which compiler/Python/toolchain components remain outside verification.
- Risks/open questions: Aeneas limitations may require behavior-preserving refactors; each such refactor returns through Rust oracle tests before proof repair. If the general LALR bridge fails, the verified release must mechanically reject tables outside the restricted theorem profile rather than claiming full coverage.
- Verify: `just aeneas`
- Verify: `just lean`
- Verify: `just proof-assumptions`
- Verify: `just ci-verified`
