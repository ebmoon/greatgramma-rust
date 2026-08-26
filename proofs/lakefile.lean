import Lake
open Lake DSL

require aeneas from git
  "https://github.com/AeneasVerif/aeneas.git" @
    "5d08da45a405913bbee6fd544e01debf8154ac9d" / "backends/lean"

package «greatgramma-proofs» where
  preferReleaseBuild := true

@[default_target]
lean_lib «GreatgrammaGenerated» where
  srcDir := "Generated"
  roots := #[`GreatgrammaCore]

@[default_target]
lean_lib «Greatgramma» where
  roots := #[`Greatgramma]

lean_exe «blueprintCheckDecls» where
  root := `BlueprintCheckDecls
  supportInterpreter := true
