set shell := ["bash", "-euo", "pipefail", "-c"]

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --workspace --all-targets

aeneas-smoke:
    @scripts/aeneas-smoke.sh

aeneas-core: aeneas-smoke
