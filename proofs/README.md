# Phase 2 Lean workspace

This directory is the pinned Lean 4.31 workspace for the Phase 2 verification.
Unit 1 establishes reproducible extraction and proof inventories. Units 2 and
3 add the first total generated-code proofs and the stable structural model;
the end-to-end constrained-decoding theorem remains later Phase 2 work.

## What Unit 1 freezes

- `aeneas-manifest.env` pins Aeneas, Charon, Rust, and Lean.
- `lakefile.lean` pins the Aeneas Lean backend by Git commit and subdirectory.
  `lake-manifest.json` locks its complete Mathlib dependency graph.
- `Generated.manifest` lists only Aeneas-owned output after deterministic
  namespace packaging.
- `Generated/GreatgrammaCore/translation.json` is the machine-readable source
  for `proof-coverage.json` and `public-api-coverage.json`.
- `Greatgramma.lean` is the stable handwritten import root. Later proof units
  add specifications and theorems beneath this root rather than linking
  documentation directly to generated names.

The split translator emits `TypesExternal_Template.lean` and
`FunsExternal_Template.lean`, but its generated modules import the names
without `_Template`. Unit 2 keeps those adjacent files as declaration-free
import bridges and supplies the reviewed definitions and contracts under
`External/`. Both remain deliberately excluded from `Generated.manifest`: the
smoke check reconciles the union of handwritten Rust hooks against every
generated template hook, rejects `axiom`, `opaque`, `constant`, `sorry`, and
`admit` throughout this boundary, and elaborates the generated root against
the exact modeled types.

Allocator capacity is erased by Aeneas's list-based `Vec` representation.
The reserve models therefore preserve contents and expose a deterministic
logical failure at requests that cannot fit the erased representation;
`Greatgramma.External.AllocatorContract` states only the observable totality,
typed-outcome, and content-preservation guarantee. It deliberately makes no
claim about fidelity to a concrete system allocator. Formatting is modeled
only well enough to keep generated `Debug` code outside theorem roots.

The extraction uses `-split-files -gen-lib-entry -loops-to-rec -emit-json`.
At the pinned Aeneas revision, all 46 translated Rust loops still elaborate
through `partial_fixpoint`; `-loops-to-rec` alone does not create termination
proofs. `Greatgramma.Refinement.totalLoops` supplies total `WP.spec` evidence
for representative validation, fact-propagation, ranked-parser, and
transactional-batch loops. The local axiom audit reports only `propext`,
`Classical.choice`, and `Quot.sound` for that root.

## Structural specification layer

`Spec/` defines generated-name-independent token, corrected lexer, ranked LALR,
and finite-head semantics. `Invariants/` restores the representation boundaries
that Rust privacy enforces but generated Lean structures do not: `ValidatedWF`,
`PreparedWF`, `MatcherWF`, and `WorkspaceWF`. The AS1--AS7 language premises
are explicit `Prop`-valued structures, not global axioms, and successful
validation is not allowed to manufacture them. `WorkspaceWF` records both the
persistent staging-row shape and parser-state range safety in reusable staging
and scratch buffers, while deliberately permitting empty or phase-stale
uncommitted buffers after allocation failure.

`Refinement/Normalized.lean` gives exact total specifications for identifier,
constructor, and normalized lookup APIs. `Refinement/ValidateBitmap.lean`
proves that ignored-terminal IDs are expanded into the exact membership bitmap,
and `Refinement/Validate.lean` proves every generated validation subpipeline.
Its public `Greatgramma.Refinement.validationBoundary` theorem says that an
`Ok` result from the translated public `UnvalidatedGrammar::validate` owns the
caller's exact tables, satisfies `ValidatedWF`, and induces the stable pure
`StructuralWF` predicate; an `Err` result exposes no validated artifact. This
is structural validation only and does not manufacture AS1--AS7 semantic
correctness assumptions.

`Examples/Validation.lean` covers the minimal valid grammar, empty ordinary
bytes, missing ordinary/EOS entries, malformed dimensions and IDs, non-EOF
acceptance, ignored EOF, duplicate EOS entries, exact normalized lookups, and
the corrected lexer boundary-reconsumption and EOS-flush behavior. The U3 root
axiom audit, like U2's, reports only Lean's accepted standard foundations:
`propext`, `Classical.choice`, and `Quot.sound`.

## Build and reconcile inventories

Prerequisites are Lean/Elan, Python 3, and Git. From the repository root:

```sh
just proof-fast
```

The equivalent commands when `just` is unavailable are:

```sh
cd proofs && lake build Greatgramma blueprintCheckDecls && cd ..
python3 scripts/check-proof-coverage.py --self-test
python3 scripts/check-public-api-coverage.py --self-test
```

The proof inventory classifies every local translated function and loop. The
public-API inventory separately reconciles Rust re-exports and public methods
with generated Lean declarations or specific exclusions. Entries remain
`unproved` until a stable handwritten view or theorem exists; later units do
not infer semantic coverage merely from the generated declaration. Typed root
references in `ProofManifest.lean` and reports in `AxiomAudit.lean` are the
authoritative theorem and trust-boundary gates as those roots are added.

The public-API parser supports this crate's root `pub use` declarations,
ordinary inherent methods, and the repository's `define_id!` method generator.
Other direct root exports, inherent-implementation forms, or item-generating
macros fail closed until the checker is extended for their syntax.

## Reproduce extraction

Build the pinned Aeneas checkout and its pinned `charon/` checkout, then run:

```sh
AENEAS_ROOT=/absolute/path/to/aeneas just aeneas-smoke
# Equivalent when `just` is unavailable:
AENEAS_ROOT=/absolute/path/to/aeneas scripts/aeneas-smoke.sh
```

The script verifies both source commits and clean worktrees, rebuilds Charon in
a fresh target directory, extracts the complete `greatgramma-core` crate,
translates it with the exact U1 flags, packages the flat split output under the
`GreatgrammaCore` module namespace, reconciles the recursive manifest, compares
every generated byte, and elaborates the pinned checked-in Lean workspace.

Set `CHARON_ROOT` only when Charon is not at `$AENEAS_ROOT/charon`. Set
`AENEAS_KEEP_TMP=1` to retain the scratch directory for drift inspection. The
script never updates checked-in generated files.

## Blueprint

The proof roadmap and its local HTML/PDF instructions are in
[`../blueprint/README.md`](../blueprint/README.md). Blueprint state and graph
checks are documentation gates only; they do not replace Lean elaboration,
coverage reconciliation, or later theorem/axiom gates.
