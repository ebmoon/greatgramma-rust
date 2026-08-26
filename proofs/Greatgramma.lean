import GreatgrammaCore
import Spec.Assumptions
import Invariants.Matcher
import Refinement.Normalized
import Refinement.ValidateBitmap
import Refinement.Validate
import Refinement.ValidatedView

/-!
The stable root of the handwritten GreatGramma proof library.

Phase 2 proofs should be exposed through this module instead of importing the
Aeneas-generated modules directly. The `Spec` modules are independent of
generated declaration names, `Invariants` restore Rust's representation
boundaries, and `Refinement` is the only layer that connects those interfaces
to translated code.
-/
