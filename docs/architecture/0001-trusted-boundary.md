# ADR 0001: Trusted boundary and proof-toolchain bootstrap

## Status

Accepted for Phase 1. Updated to describe the implemented compiler, Python
boundary, and passing extraction/elaboration gate.

## Decision

The Rust theorem boundary begins at owned normalized token bytes, a
deterministic priority-resolved byte DFA, and a conflict-free ranked LALR table.
Their language-level correctness is assumed. `greatgramma-core` validates
finite structure and executes every derived/runtime relation after the
ownership transition:

```text
UnvalidatedGrammar -> ValidatedGrammar -> PreparedGrammar -> Matcher
```

Validated and prepared internals cannot be constructed or mixed by callers.
Masks, allowance queries, and atomic advancement share the same token and
parser relations. Empty legal sets and invariant failures are errors, never a
request to decode without constraints.

The compiler is implemented but remains outside the theorem. It uses
llguidance's derivre-backed `RegexVec` to build a restricted priority-labeled
byte DFA, parses source with cfgrammar, and uses lrtable's Pager LALR generator.
It lowers those library outputs into the core's normalized ranked schema. The
compiler must fail closed on unsupported source features; core validation does
not make compiler semantic correctness part of the theorem.

The PyO3 module, Python facade, tokenizer-manifest provenance, Torch mask
expansion, Transformers mode checks, packaging, and serialization are also
outside the theorem.

## Normative execution choices

The corrected lexer contract distinguishes logical start from the DFA start,
reconsumes a boundary byte, and treats EOS separately: clean start supplies
EOF, an accepting residual emits its terminal then EOF, and an unfinished
residual rejects. The compiler rejects lexers requiring buffered rollback to an
earlier accepting prefix.

The parser performs all reductions for one lookahead before shift, reject, or
accept. A generated rank witnesses termination: the next reduction rank must be
lower, except equal rank is allowed after a reduction that pops at least two
states. EOF acceptance occurs only through an EOS event.

| Rust module | Planned Lean obligation |
| --- | --- |
| `validate` / `normalized` | Safe dimensions, IDs, ownership transition, and structural invariants |
| `lexer` | Corrected byte stepping and EOS behavior |
| `token_step` | Direct model-token/lexer composition |
| `sequence` / `spanner` | Singleton heads, realizable sequences, and inverse membership |
| `lalr` / `lalr_reference` / `preprocess` | Reduction closure and partial-evaluator refinement |
| `mask` / `engine` | Shared membership, packed representation, replay, and atomic advance |

## Assumptions inventory

The future theorem must state, rather than hide, these external assumptions:

- token IDs carry the exact byte-homomorphic tokenizer entries claimed by the
  caller;
- lexer labels implement the intended priority and maximal-munch language for
  the supported boundary profile;
- LALR actions/gotos/productions recognize the intended grammar;
- the reduction-progress witness is correct for reachable executions; and
- any singleton-byte, prunedness, separator/no-op, and generation conditions
  required by the final theorem hold.

`greatgramma-core` checks the finite instances it can check structurally. It
does not prove the first three semantic assumptions from Yacc, regex, or
tokenizer source.

## Frozen proof-toolchain pins

- Aeneas: `5d08da45a405913bbee6fd544e01debf8154ac9d`
- Charon: `f5208b1c4ce287898a1fc015d41a20da3baf0974`
- Rust for Charon: `nightly-2026-06-01`
- Lean required by the Aeneas backend: `leanprover/lean4:v4.31.0`
- UCSD formalization:
  `ucsd-formal/constrained-decoding-formalization@112af02fa29b3e50c1048c36b4b8d18975dee102`,
  which pins `leanprover/lean4:v4.29.0-rc6`

The Lean pins are incompatible. The preferred resolution remains a documented
port of the frozen UCSD snapshot to Lean 4.31.0; selecting another Aeneas pin
requires an ADR amendment.

## Current proof status

`scripts/aeneas-smoke.sh` is deliberately fail-closed. It rebuilds Charon from
the pinned source in a fresh target directory, extracts the complete core,
translates it with pinned Aeneas, elaborates the generated Lean module, and
checks its inventory and bytes against the reviewed baseline. That gate passes
for this checkout.

Accordingly, the only permitted Phase 1 label is
**“Aeneas-translatable; Lean proof in progress.”** This records successful
extraction/elaboration but is not a claim of formal verification. A verified
release requires separately proved theorems and an assumption audit in
addition to the existing extraction-drift gate.
