# GreatGramma Rust

GreatGramma Rust is an exact grammar-constrained decoding engine with a small
Rust semantic kernel, a restricted Yacc/byte-regex compiler, and a Python
`LogitsProcessor` integration.

> Target Phase 1 label: **Aeneas-translatable; Lean proof in progress.**

This is not a formal-verification claim. The pinned source-built Charon/Aeneas
gate now translates the complete core, elaborates the generated Lean module,
and checks byte-for-byte drift against the reviewed baseline. No Lean soundness
or completeness theorem has been proved. See the
[trusted-boundary ADR](docs/architecture/0001-trusted-boundary.md).

## What is implemented

- `greatgramma-core`: dependency-free, `unsafe`-free normalized-table
  validation, lexer/token composition, LALR execution, preprocessing, packed
  masks, and atomic fixed-row advancement.
- `greatgramma-compile`: action-free original-Yacc parsing, shared compressed
  byte-regex vectors through `llguidance`/`derivre`, Pager LALR table generation
  through `lrtable`, and exact token-manifest normalization.
- `greatgramma`: a typed Python facade and private PyO3 module, plus a strict
  fixed-row Transformers-compatible processor. Torch is imported only when a
  mask is applied.

The supported surface is intentionally narrow. In particular, tokenizer
introspection outside the strict fast-tokenizer ByteLevel JSON profile, beam
ancestry, encoder-decoder models, assisted generation, serialized prepared
grammars, and longer-context lexer rollback are not supported. The complete
contract is in the [v1 compatibility profile](docs/compatibility/v1.md).

## Setup

The Python package is not published. Build it from a local checkout with
Python 3.10 or newer, [Rustup](https://rustup.rs/), and
[`uv`](https://docs.astral.sh/uv/):

```bash
git clone https://github.com/ebmoon/greatgramma-rust.git
cd greatgramma-rust
rustup toolchain install 1.98.0
uv sync --locked --group dev --extra transformers --no-install-project
uv run --frozen --no-sync maturin develop --release
```

The `transformers` extra installs the pinned Torch and Transformers versions
used by the Python generation integration. Run Python commands in the prepared
environment with `uv run --frozen --no-sync python ...`.

## Python generation example

The tokenizer manifest contains exact token-local bytes. Do not build it by
decoding token IDs independently; the caller is responsible for establishing
that concatenating these entries equals authoritative whole-sequence decoding.

```python
from transformers import AutoModelForCausalLM, AutoTokenizer

from greatgramma import (
    GreatGrammaLogitsProcessor,
    Terminal,
    TokenizerManifest,
    compile,
)

model_id = "microsoft/phi-2"
tokenizer = AutoTokenizer.from_pretrained(model_id)
model = AutoModelForCausalLM.from_pretrained(model_id)
input_ids = tokenizer(
    "Answer yes or no: Is two plus two four?\nAnswer:",
    return_tensors="pt",
).input_ids

compiled = compile(
    "%token ANSWER\n%%\nS: ANSWER;\n",
    start_rule="S",
    terminals=[Terminal("ANSWER", " (yes|no)")],
    tokenizer=TokenizerManifest.from_transformers(
        tokenizer,
        eos_token_ids={tokenizer.eos_token_id},
    ),
)

# The prompt is a boundary and is not parsed by the grammar.
processor = GreatGrammaLogitsProcessor(compiled, input_ids)

output = model.generate(
    input_ids,
    max_new_tokens=5,
    logits_processor=[processor],
)
print(
    tokenizer.decode(
        output[0],
        skip_special_tokens=True,
        clean_up_tokenization_spaces=False,
    )
)
```

The first run downloads about 5.6 GB of Phi-2 weights. Phi-2 exposes 51,200
model logits for a 50,295-entry tokenizer; GreatGramma masks every model-only
suffix logit to `-inf`. Keep GreatGramma last when supplying other custom logits
processors so no later processor can re-enable an invalid token.

As an alternative to a caller-audited manifest,
`TokenizerManifest.from_transformers(tokenizer, eos_token_ids=...)` reads only
`backend_tokenizer.to_str()` and accepts the restricted dense ByteLevel JSON
profile. It never derives bytes with per-token `decode()`.

`compiled.generate(...)` is the checked wrapper for callers that already own a
resolved `GenerationConfig`. Direct
`GreatGrammaLogitsProcessor(...)` or `compiled.logits_processor(...)`
construction is an advanced API: it requires a fixed row order, the exact
initial prompt on the first callback, and exactly one new token per row on each
later callback. Do not use it with beam search, multiple return sequences,
assisted generation, or another mode that can reorder or extend rows by more
than one token. One `CompiledGrammar` creates one generation session.

Multirow generation requires an in-vocabulary pad token distinct from every EOS
token. The wrapper rejects that ambiguous padding configuration for multirow
generation because it does not accept an explicit attention mask.

## Rust example

The compiler is the ordinary Rust entry point:

```rust
use greatgramma_compile::{
    CompileLimits, SourceGrammar, TerminalSpec, TokenSpec, TokenizerManifest,
    compile,
};
use greatgramma_core::TokenId;

let source = SourceGrammar::new(
    "%start S\n%token ITEM\n%%\nS: ITEM;\n",
    vec![TerminalSpec::new("ITEM", "i+", 0)],
);
let tokens = TokenizerManifest::new(vec![
    TokenSpec::Bytes(b"i".to_vec()),
    TokenSpec::Bytes(b"ii".to_vec()),
    TokenSpec::Eos,
]);
let prepared = compile(source, tokens, CompileLimits::default()).unwrap();
let mut matcher = prepared.into_matcher(1).unwrap();
let mut mask = vec![0; matcher.mask_bytes()];

matcher.mask(0, &mut mask).unwrap();
assert_eq!(mask[0] & 0b11, 0b11);
matcher.advance(0, TokenId::new(1)).unwrap();
matcher.advance(0, TokenId::new(2)).unwrap();
assert!(matcher.is_completed(0).unwrap());
```

For callers that already own semantically correct normalized tables, see the
[manual normalized-table example](docs/rust-api.md).

## Development checks

The workspace targets Rust 1.98.0.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
python3 scripts/check-publication-metadata.py
scripts/conformance-oracle.sh
scripts/fuzz-smoke.sh
scripts/leak-check.sh
uv run --frozen --no-sync pytest tests/python -q
uv run --frozen --no-sync pyright
scripts/package-check.sh
```

CI runs Rust checks, the independent conformance oracle, bounded normalized-input
fuzzing, Python tests and strict typing across Python 3.10 and 3.14 on Linux,
macOS, and Windows, plus host sdist/abi3-wheel smoke tests. Local benchmark and
100,000-step leak harnesses are also available. A retained frozen-upstream
comparison, a selected project license, and green final-commit CI are still
required before a Phase 1 tag. The
[benchmark protocol](docs/benchmarks/protocol-v1.md) separates implemented
measurements from performance claims that have not yet earned release status.

## Error and trust contracts

Every failure is fail-closed: malformed input, unsupported syntax, exhausted
limits, invalid history, and an empty legal-token set produce an error rather
than unconstrained logits. Python errors expose stable codes documented in the
[error reference](docs/errors.md).

The compiler and Python integration are outside the planned theorem boundary.
The core structurally validates normalized tables but assumes their
language-level token-byte, lexer-DFA, and LALR semantics are correct.

## Licensing and distribution

No license has been selected for this project. Do not publish or distribute
source or built artifacts until the repository owner selects a license and the
publication guards are intentionally removed. Rust crates use `publish = false`
and Python metadata uses `Private :: Do Not Upload`. Upstream licenses in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) license those dependencies and
references, not this project.
