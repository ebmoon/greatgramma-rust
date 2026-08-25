# Rust API guide

Most callers should use `greatgramma-compile`. It owns the supported source
contract and returns an opaque `PreparedGrammar` only after normalization,
validation, and bounded preparation succeed:

```rust
use greatgramma_compile::{
    CompileLimits, SourceGrammar, TerminalSpec, TokenSpec, TokenizerManifest, compile,
};

let source = SourceGrammar::new(
    "%start S\n%token ITEM\n%%\nS: ITEM;\n",
    vec![TerminalSpec::new("ITEM", "i+", 0)],
);
let tokenizer = TokenizerManifest::new(vec![
    TokenSpec::Bytes(b"i".to_vec()),
    TokenSpec::Bytes(b"ii".to_vec()),
    TokenSpec::Eos,
]);

let prepared = compile(source, tokenizer, CompileLimits::default())
    .expect("supported source should compile");
let mut matcher = prepared.into_matcher(1).expect("one row is valid");

let mut mask = vec![0_u8; matcher.mask_bytes()];
matcher.mask(0, &mut mask).expect("initial mask exists");
assert_ne!(mask[0] & 0b0000_0001, 0); // token 0, "i", is legal
```

The complete supported grammar, regex, manifest, priority, and resource-limit
profile is in [the compatibility contract](compatibility/v1.md). In particular,
the compiler accepts action-free original Yacc and a deliberately restricted
byte-regex profile; it is not a general Yacc/regex frontend.

`CompileLimits` bounds source bytes and terminal counts before Yacc parsing,
then applies aggregate regex source, construction-work, and compiled-output
budgets. Strict tokenizer JSON extraction uses `TokenizerJsonLimits`; callers
that need lower service-specific ceilings can call
`TokenizerManifest::from_tokenizer_json_with_limits`.

## Manual normalized tables

`greatgramma-core` also exposes the normalized schema for callers that already
have a lexer DFA and LALR table. This is an advanced trust boundary. Validation
checks finite structure, IDs, dimensions, EOF placement, and limits, but cannot
prove that caller-supplied tables implement a source grammar or tokenizer.

The following complete table recognizes one `ITEM` terminal spelled `i`, then
EOS. Parser terminal 0 is `ITEM`, terminal 1 is EOF, and nonterminal 0 is the
start symbol.

```rust
use greatgramma_core::{
    Action, DfaStateId, LalrDimensions, LalrTable, LexerDfa, NonterminalId,
    ParserStateId, PreparationLimits, Production, ProductionId, TerminalId,
    TokenEntry, TokenId, UnvalidatedGrammar, ValidationLimits, prepare,
};

let mut byte_classes = vec![1_u32; 256];
byte_classes[usize::from(b'i')] = 0;

let lexer = LexerDfa::new(
    2, // states
    2, // byte classes: `i`, everything else
    byte_classes,
    vec![
        Some(DfaStateId::new(1)), None, // state 0
        None, None,                     // accepting state 1
    ],
    DfaStateId::new(0),
    vec![None, Some(TerminalId::new(0))],
);

let parser = LalrTable::new(
    LalrDimensions::new(3, 2, 1),
    ParserStateId::new(0),
    TerminalId::new(1), // EOF
    vec![
        Action::Shift(ParserStateId::new(1)), Action::Error,
        Action::Error, Action::Reduce {
            production: ProductionId::new(0),
            rank: 0,
        },
        Action::Error, Action::Accept,
    ],
    vec![Some(ParserStateId::new(2)), None, None],
    vec![Production::new(NonterminalId::new(0), 1)],
);

let normalized = UnvalidatedGrammar::new(
    vec![TokenEntry::Bytes(b"i".to_vec()), TokenEntry::Eos],
    lexer,
    parser,
);
let validated = normalized
    .validate(ValidationLimits::default())
    .expect("table is structurally valid");
let prepared = prepare(validated, PreparationLimits::default())
    .expect("table fits preparation limits");
let mut matcher = prepared.into_matcher(1).expect("one row is valid");

let mut mask = vec![0_u8; matcher.mask_bytes()];
matcher.mask(0, &mut mask).expect("initial mask exists");
assert_eq!(mask[0] & 0b0000_0011, 0b0000_0001);

matcher
    .advance(0, TokenId::new(0))
    .expect("token 0 is legal");
matcher.mask(0, &mut mask).expect("successor mask exists");
assert_eq!(mask[0] & 0b0000_0011, 0b0000_0010); // only EOS remains
```

The `rank` in every reduce action is a compiler-supplied progress witness. A
manual table must satisfy the core parser's reduction-progress rules at runtime.
`greatgramma-compile` derives these ranks and rejects a table construction when
it cannot produce a witness.

## Matchers and masks

`PreparedGrammar::into_matcher(rows)` consumes the prepared grammar and creates
one fixed-size batch. This ownership prevents parser/lexer state from being
mixed across grammars.

- `allows(row, token)` is a non-mutating legality query.
- `advance(row, token)` commits one legal token or returns an error without a
  partial successor.
- `advance_batch(tokens)` atomically advances every row.
- `advance_active(tokens)` atomically advances running rows while preserving
  accepted rows represented by `None`.
- `advance_active_and_masks(tokens, output)` computes successor masks and
  commits only if every running successor also has a valid next-token mask.
- `advance_active_and_masks_no_result(tokens, output)` is the steady-state
  variant used by the Python adapter. It reuses the matcher's retained parser
  stacks and omits the per-row result vector. Calls allocate only when a parser
  or probe stack exceeds its previous high-water capacity; an unbounded LR
  stack cannot promise a fixed capacity for every future input.
- `mask(row, output)` and `masks(output)` clear output on failure.

The no-result batch path reports a rejected token as
`EngineError::BatchConstraintViolation { row, token }`. Like the other batch
operations, it leaves every committed row unchanged and clears the entire mask
buffer when any row fails.

Masks are packed LSB-first: token ID `t` uses bit `t % 8` in byte `t / 8`.
`masks` concatenates fixed-width rows in row-major order. Required bytes per row
are `(token_count + 7) / 8`; unused high bits in the final byte stay zero.
Caller buffers may be larger than required, and all supplied bytes are cleared
before a mask is written.

An EOS entry is a semantic end marker, not a byte string. It finishes the
current accepting lexer token, feeds the parser EOF terminal, and is allowed
only when that sequence accepts. A matcher row becomes permanently accepted
after a successful EOS advance. Core APIs leave padding policy to the caller;
`allows`, `advance`, and per-row mask queries then return `EngineError::Completed`.
Use `is_completed` and the `None` entries accepted by `advance_active` for a
mixed running/completed batch. The Python fixed-row adapter supplies the
configured pad token for accepted rows.

## Validation and semantic responsibility

The safe state transition is:

```text
UnvalidatedGrammar --validate--> ValidatedGrammar --prepare--> PreparedGrammar
                   --into_matcher--> Matcher
```

No constructor above performs network access or consults a tokenizer. The
caller remains responsible for the semantic accuracy of normalized token bytes,
lexer language/priority behavior, and parser language. Prefer the compiler when
possible because it narrows that responsibility to source specifications and
the exact manifest. See [the trusted-boundary ADR](architecture/0001-trusted-boundary.md)
and [the error contract](errors.md).

The core is labeled **Aeneas-translatable; Lean proof in progress**. That label
describes an implementation constraint and unfinished verification program. It
is not a claim that this API, its compiler inputs, or its runtime behavior has
been formally verified.
