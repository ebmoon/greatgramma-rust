import Lake
open Lake DSL

require aeneas from git
  "https://github.com/AeneasVerif/aeneas.git" @
    "5d08da45a405913bbee6fd544e01debf8154ac9d" / "backends/lean"

package «greatgramma-proofs» where
  preferReleaseBuild := true

@[default_target]
lean_lib «GreatgrammaExternal» where
  roots := #[
    `External.Types,
    `External.Conversions,
    `External.Option,
    `External.Result,
    `External.Slice,
    `External.Contracts,
    `External.Vec,
    `External.Formatting
  ]

@[default_target]
lean_lib «GreatgrammaGenerated» where
  srcDir := "Generated"
  roots := #[`GreatgrammaCore]

@[default_target]
lean_lib «Greatgramma» where
  roots := #[
    `Greatgramma,
    `Spec.Types,
    `Spec.Token,
    `Spec.Lexer,
    `Spec.Lalr,
    `Spec.FiniteHeadChecker,
    `Spec.Assumptions,
    `Invariants.Validated,
    `Invariants.Prepared,
    `Invariants.Matcher,
    `Refinement.Normalized,
    `Refinement.ValidateBitmap,
    `Refinement.Validate,
    `Refinement.ValidatedView,
    `Examples.Feasibility,
    `Examples.Validation,
    `ProofManifest,
    `AxiomAudit
  ]

lean_exe «blueprintCheckDecls» where
  root := `BlueprintCheckDecls
  supportInterpreter := true
