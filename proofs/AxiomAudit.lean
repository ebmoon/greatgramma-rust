import Examples.Feasibility
import Refinement.Validate

/-!
The U2 feasibility and U3 validation roots must remain free of generated
external or semantic-language axioms. These commands are intentionally checked
by the proof gate rather than hidden behind derived propositions.
-/

#print axioms Greatgramma.Refinement.totalLoops
#print axioms Greatgramma.Refinement.validationBoundary
