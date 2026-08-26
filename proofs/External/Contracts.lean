import GreatgrammaCore.Types

open Aeneas Aeneas.Std Result ControlFlow Error
open GreatgrammaCore

/-! # Contracts for erased allocator observations

Aeneas represents `Vec T` by its bounded list of elements; Rust capacity,
layout, and allocator identity are intentionally erased.  Consequently the
semantic obligation at this boundary is not to predict a real allocator.  It
is to guarantee termination, an explicit success/failure outcome, and content
preservation for either outcome.
-/

namespace Greatgramma.External

abbrev ReserveFn (T : Type) :=
  alloc.vec.Vec T → Std.Usize →
    Result ((core.result.Result Unit alloc.collections.TryReserveError) ×
      alloc.vec.Vec T)

/-- The observable contract shared by logical and real allocation models.
It permits either allocator outcome, but rules out panic/divergence and makes
failure atomic at the content level. -/
def AllocatorContract {T : Type} (reserve : ReserveFn T) : Prop :=
  ∀ vector additional,
    reserve vector additional ⦃ outcome vector' =>
      vector'.val = vector.val ∧
      (outcome = .Ok () ∨ outcome = .Err .modeled) ⦄

abbrev CapacityFn (T : Type) :=
  alloc.vec.Vec T → Result Std.Usize

/-- Because capacity is erased, the canonical logical observation is the
smallest valid capacity: the current element count. -/
def CapacityContract {T : Type} (capacity : CapacityFn T) : Prop :=
  ∀ vector,
    capacity vector ⦃ observed => observed.val = vector.val.length ⦄

end Greatgramma.External
