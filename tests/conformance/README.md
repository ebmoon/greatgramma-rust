# Phase 1 conformance oracle

Run from the repository root:

```sh
scripts/conformance-oracle.sh
```

The script uses `rustc` directly, so this test remains independent of the
workspace manifests. It compiles the current `greatgramma-core`, then compiles
and runs `oracle.rs` as a standalone Rust test binary in a temporary directory.

The `oracle` module does not import or call production lexer, token-composition,
parser-preprocessing, mask, or advance code. It interprets raw token bytes,
byte-DFA transitions, LALR actions/gotos, and EOS itself. A narrow production
adapter exists only to compare public `mask`, `allows`, and atomic `advance`
observations against that independent result.

The suite checks curated provenance cases, all live histories through depth
three, and mutation witnesses for boundary-byte loss, missing EOS flush,
single-EOS assumptions, clean-EOS acceptance, C-string-style NUL handling, and
lossy UTF-8 conversion.
