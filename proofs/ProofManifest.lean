import Examples.Feasibility
import Refinement.Validate

open Aeneas Aeneas.Std

/-!
The authoritative handwritten theorem inventory grows with each Phase 2 unit.
Keeping explicit typed references here makes declaration drift an elaboration
failure; coverage JSON and Blueprint status remain secondary projections.
-/

namespace Greatgramma.ProofManifest

/-- U2: total weakest-precondition evidence for the representative validation,
fact-propagation, ranked-parser, and transactional-batch loops. -/
theorem u2Feasibility : Greatgramma.Refinement.TotalLoopEvidence :=
  Greatgramma.Refinement.totalLoops

/-- U3: successful execution of the public generated validation boundary
establishes both the generated representation invariants and the stable pure
structural specification for the caller's exact normalized tables. -/
theorem u3ValidationBoundary
    (grammar : GreatgrammaCore.normalized.UnvalidatedGrammar)
    (limits : GreatgrammaCore.limits.ValidationLimits) :
    WP.spec
      (GreatgrammaCore.normalized.UnvalidatedGrammar.validate grammar limits)
      (fun result => match result with
        | .Ok validated =>
          Greatgramma.Invariants.ValidatedWF validated ∧
            Greatgramma.Invariants.ValidatedRepresents grammar validated ∧
            Greatgramma.Spec.StructuralWF
              (Greatgramma.Invariants.viewUnvalidated grammar)
        | .Err _ => True) :=
  Greatgramma.Refinement.validationBoundary grammar limits

end Greatgramma.ProofManifest
