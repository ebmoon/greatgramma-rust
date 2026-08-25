# Phase 1 fuzzing

`normalized_input` maps arbitrary bytes to small normalized token, lexer, and
parser tables. Some generated tables are structurally valid but semantically
hostile; others deliberately contain bad dimensions, lengths, classes, and
IDs. The target exercises validation, preparation, and reachable matcher
states.

For every successful mask, it checks exact agreement with `Matcher::allows`,
a nonempty in-vocabulary result, and zero unused tail bits. Every failed mask
query must clear its caller-owned output. These checks make a validation or
runtime error fail closed instead of accidentally exposing an unconstrained
mask.

Run the bounded, deterministic smoke gate from the repository root:

```console
scripts/fuzz-smoke.sh
```

The script requires `cargo-fuzz` 0.13.2 and the pinned
`nightly-2026-06-01` Rust toolchain. It exits with an error when either
prerequisite is unavailable. Each run starts from four fixed branch seeds in a
temporary corpus, so local corpus discoveries cannot change CI behavior or
dirty the checkout.
