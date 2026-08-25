import Lake
open Lake DSL

-- scripts/aeneas-smoke.sh creates this local link to the audited Aeneas checkout.
require aeneas from "./aeneas"

package «greatgramma-aeneas-smoke» {}

@[default_target]
lean_lib «GreatgrammaCore» {}
