set shell := ["bash", "-euo", "pipefail", "-c"]

fmt:
    cargo fmt --all
    cargo fmt --manifest-path fuzz/Cargo.toml

fmt-check:
    cargo fmt --all -- --check
    cargo fmt --manifest-path fuzz/Cargo.toml -- --check

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo clippy --manifest-path fuzz/Cargo.toml --all-targets --all-features -- -D warnings

test:
    cargo test --workspace --all-targets --all-features

python-check:
    uv sync --locked --group dev --no-install-project
    uv run --frozen --no-sync maturin develop --release --features panic-test-hook
    uv run --frozen --no-sync pytest tests/python -q
    uv run --frozen --no-sync pyright

metadata-check:
    python3 scripts/check-publication-metadata.py

conformance:
    @scripts/conformance-oracle.sh

fuzz-smoke:
    @scripts/fuzz-smoke.sh

leak-check:
    @scripts/leak-check.sh

benchmark-rust:
    @scripts/benchmark-phase1.sh

benchmark-python *args:
    @PYTHON=.venv/bin/python scripts/benchmark-python.sh {{args}}

benchmark-compare candidate baseline:
    @PYTHON=.venv/bin/python scripts/benchmark-compare.sh --candidate "{{candidate}}" --baseline "{{baseline}}" --thresholds tests/fixtures/benchmark/phase1-thresholds-v1.json

benchmark-self-test:
    @scripts/benchmark-python.sh --self-test
    @scripts/benchmark-compare.sh --self-test

package-check:
    @scripts/package-check.sh

ci-phase1: fmt-check lint test metadata-check conformance fuzz-smoke python-check leak-check benchmark-self-test package-check

aeneas-smoke:
    @scripts/aeneas-smoke.sh

aeneas-core: aeneas-smoke

proof-build:
    cd proofs && lake build Greatgramma blueprintCheckDecls

proof-coverage:
    python3 scripts/check-proof-coverage.py --self-test

public-api-coverage:
    python3 scripts/check-public-api-coverage.py --self-test

proof-fast: proof-build proof-coverage public-api-coverage

proof-docs-sync:
    uv sync --locked --group proof-docs --no-install-project

blueprint-clean:
    rm -rf blueprint/web blueprint/print blueprint/lean_decls blueprint/src/*.paux

blueprint-check: proof-fast proof-docs-sync blueprint-clean
    python3 scripts/check-blueprint-coverage.py --self-test
    uv run --frozen --no-sync --project . --directory blueprint/src plastex -c plastex.cfg web.tex
    cd proofs && lake exe blueprintCheckDecls ../blueprint/lean_decls

blueprint-build: blueprint-check
    cd blueprint/src && latexmk -xelatex -interaction=nonstopmode -halt-on-error -outdir=../print print.tex
