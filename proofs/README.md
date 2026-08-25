# Aeneas extraction smoke

Phase 1 requires a real Charon → Aeneas → Lean smoke check for
`greatgramma-core`. The check is an extraction/elaboration gate, not a proof of
the generated program or of GreatGramma's language claim.

The audited inputs live in `aeneas-manifest.env`. The script verifies the Git
commits, tracked-worktree cleanliness, embedded binary versions, Rust nightly,
and Lean version before translating anything. Because Charon's binary reports
only a semantic version, the script rebuilds Charon from the verified pinned
source tree in a fresh temporary target directory instead of trusting ignored
pre-existing build artifacts. It then:

1. runs Charon on the whole `greatgramma-core` crate with `--preset=aeneas`;
2. translates the resulting LLBC into one Lean module with Aeneas;
3. elaborates that module against the pinned local Aeneas Lean library; and
4. compares the generated inventory and bytes with `Generated.manifest` and
   `Generated/`.

Build the pinned Aeneas checkout and its pinned `charon/` checkout, then run:

```sh
AENEAS_ROOT=/absolute/path/to/aeneas just aeneas-smoke
# Equivalent when `just` is unavailable:
AENEAS_ROOT=/absolute/path/to/aeneas scripts/aeneas-smoke.sh
```

Set `CHARON_ROOT` only when Charon is not at `$AENEAS_ROOT/charon`. The script
does not download tools or modify checked-in generated files. Set
`AENEAS_KEEP_TMP=1` to retain its scratch directory after a failure.

`Generated/GreatgrammaCore.lean` is the reviewed byte-exact output of the pinned
toolchain. The smoke check regenerates and elaborates it, then fails on any file
inventory or byte drift. The generated module contains Aeneas runtime axioms
and is only an extraction baseline; it is not a handwritten proof or a
soundness/completeness theorem.
