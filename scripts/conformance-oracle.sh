#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
scratch=$(mktemp -d "${TMPDIR:-/tmp}/greatgramma-conformance.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM

rustc \
    --edition=2024 \
    --crate-name greatgramma_core \
    --crate-type=lib \
    "$repo_root/crates/greatgramma-core/src/lib.rs" \
    -o "$scratch/libgreatgramma_core.rlib"

rustc \
    --edition=2024 \
    --test \
    "$repo_root/tests/conformance/oracle.rs" \
    --extern greatgramma_core="$scratch/libgreatgramma_core.rlib" \
    -o "$scratch/conformance-oracle"

"$scratch/conformance-oracle" "$@"
