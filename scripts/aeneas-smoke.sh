#!/usr/bin/env bash

set -euo pipefail

if [ -z "${AENEAS_ROOT:-}" ]; then
    echo "Aeneas smoke unavailable: set AENEAS_ROOT to the pinned Aeneas checkout (5d08da45a405913bbee6fd544e01debf8154ac9d)." >&2
    exit 1
fi

missing=()
for tool in "${AENEAS_ROOT}/bin/aeneas" "${AENEAS_ROOT}/charon/bin/charon" lake; do
    if ! command -v "${tool}" >/dev/null 2>&1 && [ ! -x "${tool}" ]; then
        missing+=("${tool}")
    fi
done

if [ "${#missing[@]}" -ne 0 ]; then
    echo "Aeneas smoke unavailable: missing ${missing[*]}. Required pins: Aeneas 5d08da45a405913bbee6fd544e01debf8154ac9d, Charon f5208b1c4ce287898a1fc015d41a20da3baf0974, Rust nightly-2026-06-01, Lean 4.31.0." >&2
    exit 1
fi

echo "Aeneas smoke fixture has not been scaffolded; see docs/architecture/0001-trusted-boundary.md." >&2
exit 1
