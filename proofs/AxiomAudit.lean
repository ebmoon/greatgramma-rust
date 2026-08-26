import Lean.Util.CollectAxioms
import Examples.Feasibility
import Refinement.Validate

/-!
The U2 feasibility and U3 validation roots must remain free of generated
external or semantic-language axioms. The elaboration-time check below fails
the proof gate if either root depends on anything outside Lean's accepted
logical foundation; the print commands preserve the dependency list in build
output for inspection.
-/

private def allowedAxioms : Array Lean.Name :=
  #[``propext, ``Classical.choice, ``Quot.sound]

private def assertOnlyAllowedAxioms (declName : Lean.Name) :
    Lean.Elab.Command.CommandElabM Unit := do
  let axioms ← Lean.collectAxioms declName
  let forbidden := axioms.filter fun axiomName => !allowedAxioms.contains axiomName
  unless forbidden.isEmpty do
    let forbidden := forbidden.qsort Lean.Name.lt |>.map Lean.MessageData.ofConstName
    throwError m!"'{declName}' depends on forbidden axioms: {forbidden.toList}"

run_cmd do
  assertOnlyAllowedAxioms ``Greatgramma.Refinement.totalLoops
  assertOnlyAllowedAxioms ``Greatgramma.Refinement.validationBoundary

#print axioms Greatgramma.Refinement.totalLoops
#print axioms Greatgramma.Refinement.validationBoundary
