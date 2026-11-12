# Code organization

GreatGramma is one Rust library. Its internal modules group code by domain;
`crates/greatgramma/src/lib.rs` re-exports the public types and functions so
callers use `greatgramma::{compile, Matcher, TokenizerManifest, ...}`.

## Compilation

[`grammar/compile.rs`](../crates/greatgramma/src/grammar/compile.rs) implements
`normalize` and `compile`. It checks source limits, normalizes token bytes,
parses Yacc through `parser/yacc.rs`, constructs LALR tables through
`parser/compile.rs`, and builds a byte DFA through `lexer/regex.rs` and
`lexer/dfa.rs`. The resulting normalized grammar is structurally validated
before preparation.

Source grammar types, normalized tables, IDs, and validation belong to
`grammar/`. Lexer and parser construction live beside their corresponding
execution code. Tokenizer manifests and the strict tokenizer JSON profile
belong to `tokenizer/`.

## Preparation and matching

[`matcher/engine.rs`](../crates/greatgramma/src/matcher/engine.rs) owns
`PreparedGrammar`, `Matcher`, and the `prepare` entry point. Preparation builds
token-execution caches, singleton/sequence tables, and inverse token buckets in
`preparation/`, then computes parser lookahead classifications in
`parser/preprocess.rs`.

`preparation/budget.rs` owns preparation limits, work accounting, and shared
fallible vector allocation. `tokenizer/execute.rs` handles individual token
execution; it does not build caches or own preparation budgets.

During generation, the matcher combines prepared buckets with stack-dependent
parser checks. `matcher/mask.rs` handles packed bit operations. Matcher updates
stage all affected rows before committing, preserving atomic batch behavior.

## Errors and module boundaries

Error types and their `Display`/`Error` implementations live with the subsystem
that produces them. Compilation exposes tokenizer, validation, and preparation
failures through `CompileError::source()`.

The six subsystem modules are private to the crate. Their public types are
re-exported at the crate root; shared implementation helpers have crate-scoped
visibility only where another subsystem needs them. There are no separate
compiler/runtime packages or compatibility aliases for the previous layout.

## Tests and Python integration

`crates/greatgramma/tests/` exercises the public Rust API: grammar compilation,
tokenizers, validation, lexer/parser behavior, preprocessing, masks, and errors.
Unit tests beside implementation code check internal allocation and buffer-reuse
invariants. `tests/conformance/oracle.rs` is an independent interpreter used by
the Cargo `conformance` integration target.

`crates/greatgramma-python/src/lib.rs` only registers the extension module.
The binding implementation is split into:

| File | Responsibility |
| --- | --- |
| `boundary.rs` | Validate borrowed Python inputs before owned allocation |
| `compiler.rs` | Expose grammar compilation and tokenizer extraction to Python |
| `matcher.rs` | Own Python grammar/batch objects and transactional updates |
| `error.rs` | Convert errors, contain panics, and allocate Python byte results |

The typed facade, score masking, and Transformers integration live in
`python/greatgramma/`; Python integration tests live in `tests/python/`.
