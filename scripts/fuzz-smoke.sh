#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-fuzz >/dev/null 2>&1; then
  echo "cargo-fuzz 0.13.2 is required; install it with: cargo install cargo-fuzz --version 0.13.2 --locked" >&2
  exit 127
fi

actual_version="$(cargo fuzz --version | awk '{print $2}')"
if [[ "$actual_version" != "0.13.2" ]]; then
  echo "cargo-fuzz 0.13.2 is required; found ${actual_version}" >&2
  exit 2
fi

fuzz_toolchain="${FUZZ_TOOLCHAIN:-nightly-2026-06-01}"
if ! rustup run "$fuzz_toolchain" rustc --version >/dev/null 2>&1; then
  echo "Rust toolchain '${fuzz_toolchain}' is unavailable; cargo-fuzz requires nightly" >&2
  exit 127
fi

fuzz_corpus="$(mktemp -d "${TMPDIR:-/tmp}/greatgramma-fuzz-corpus.XXXXXX")"
cleanup() {
  rm -rf -- "$fuzz_corpus"
}
trap cleanup EXIT
printf '\000' >"$fuzz_corpus/canonical"
printf '\001' >"$fuzz_corpus/shaped-valid"
printf '\002' >"$fuzz_corpus/shaped-invalid"
printf '\003' >"$fuzz_corpus/malformed"

cargo +"$fuzz_toolchain" fuzz run normalized_input "$fuzz_corpus" -- \
  -runs="${FUZZ_RUNS:-1000}" \
  -max_len="${FUZZ_MAX_LEN:-2048}" \
  -seed="${FUZZ_SEED:-424242}" \
  -timeout="${FUZZ_TIMEOUT_SECONDS:-5}"
