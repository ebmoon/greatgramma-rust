# GreatGramma Rust

GreatGramma Rust is a Phase 1 bootstrap for exact grammar-constrained decoding.
The production workspace targets Rust 1.98.0 and keeps the semantic kernel
dependency-free and free of unsafe Rust.

This repository is not formally verified. The intended claim for the eventual
release is “Aeneas-translatable; Lean proof in progress” until the separately
audited Lean theorem and assumptions inventory exist.

## Bootstrap checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```

`just aeneas-smoke` is deliberately fail-closed. It reports the missing
pinned Aeneas/Charon toolchain rather than claiming translation succeeded.
See the trusted-boundary ADR for the exact pins and current blocker.
