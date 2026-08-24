# Task 1 report: Phase 1 workspace bootstrap

## Status

DONE_WITH_CONCERNS

## Delivered files and decisions

- Added the Rust 2024 workspace, locked to Rust 1.98.0, with dependency-free
  `greatgramma-core` and compiling `greatgramma-compile`/
  `greatgramma-python` placeholders. The core and workspace forbid unsafe Rust.
- Added a minimal test-first core contract marker. The first focused test failed
  before `src/lib.rs` existed, then passed after the marker was implemented.
- Added `justfile` formatting, lint, test, and Aeneas entry points. The latter
  delegates to `scripts/aeneas-smoke.sh` and always exits nonzero until both a
  real fixture and the pinned tools exist.
- Added CI for Linux formatting, warning-denied clippy, and workspace tests;
  no release publishing workflow is present.
- Added the approved durable plan and the execution plan under
  `superpowers/docs/plans/`, package metadata, top-level documentation, trusted
  boundary ADR, compatibility profile, benchmark protocol, and notices.
- Deliberately did not create a `proofs/` Lake skeleton: the pinned Aeneas
  backend needs Lean 4.31.0 while
  `ucsd-formal/constrained-decoding-formalization@112af02` pins Lean
  4.29.0-rc6. This remains a required integration spike.

## Toolchain facts frozen in ADR 0001

- Aeneas `5d08da45a405913bbee6fd544e01debf8154ac9d`
- Charon `f5208b1c4ce287898a1fc015d41a20da3baf0974`
- Charon Rust `nightly-2026-06-01`
- Aeneas Lean `leanprover/lean4:v4.31.0`
- UCSD formalization `112af02`, Lean `leanprover/lean4:v4.29.0-rc6`

## Verification

Executed in `/Users/kanghee/greatgramma-rust-phase1` with the locally installed
Charon-compatible nightly because the production stable toolchain is not yet
installed as the host default:

```text
cargo +nightly-2026-06-01 fmt --all -- --check                 PASS
cargo +nightly-2026-06-01 clippy --workspace --all-targets --all-features -- -D warnings  PASS
cargo +nightly-2026-06-01 test --workspace --all-targets       PASS (1 integration test)
git diff --check                                                PASS
scripts/aeneas-smoke.sh                                        EXPECTED FAIL (AENEAS_ROOT unset)
```

`just aeneas-smoke` could not be executed because `just` is absent on this
host; the delegated script was run directly and produced the required clear
unavailable-tool error.

## Commit

Commit: `chore: bootstrap Phase 1 workspace` (this commit contains the report)

## Concerns

The Aeneas/Charon checkout, GNU Make newer than 3.81, and the required OCaml 5
environment are absent. The Lean version mismatch with the UCSD snapshot is
also unresolved. Consequently the smoke target is intentionally fail-closed,
not a proof or translation success signal.
