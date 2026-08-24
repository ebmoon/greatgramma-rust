# GreatGramma Rust

GreatGramma Rust is a Phase 1 bootstrap for exact grammar-constrained decoding.
The production workspace targets Rust 1.98.0 and keeps the semantic kernel
dependency-free and free of unsafe Rust.

This repository is not formally verified. The intended claim for the eventual
release is “Aeneas-translatable; Lean proof in progress” until the separately
audited Lean theorem and assumptions inventory exist.

## Licensing and distribution

No license has been selected for this project. Do not publish or distribute
source or built artifacts until the repository owner selects a license and the
publication guards are intentionally removed. The Rust crates set
`publish = false`; the Python metadata uses PyPI's
`Private :: Do Not Upload` classifier. Upstream licenses recorded in
`THIRD_PARTY_NOTICES.md` apply to those upstream projects, not to this project.

## Bootstrap checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
python3 scripts/check-publication-metadata.py
```

`just aeneas-smoke` is deliberately fail-closed. It reports the missing
pinned Aeneas/Charon toolchain rather than claiming translation succeeded.
See the trusted-boundary ADR for the exact pins and current blocker.
