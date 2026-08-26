import External.Contracts

open Aeneas Aeneas.Std Result ControlFlow Error
open GreatgrammaCore

/-! # `Vec` external models

Only list contents are observable in Aeneas's `Vec`.  Reservation therefore
uses a deterministic logical allocator, while its public theorem is phrased
against `AllocatorContract`, which also admits real allocation failure.
-/

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::capacity"]
def alloc.vec.Vec.capacity
  {T : Type} (_allocator : Type) (vector : alloc.vec.Vec T) :
  Result Std.Usize :=
  ok (alloc.vec.Vec.len vector)

/-- Deterministic witness used to interpret fallible reservation in Lean.
It rejects the explicit `usize::MAX` failure witness and any request whose
required length overflows `usize`; both branches preserve the vector exactly. -/
def alloc.vec.logical_reserve
  {T : Type} (vector : alloc.vec.Vec T) (additional : Std.Usize) :
  Result ((core.result.Result Unit alloc.collections.TryReserveError) ×
    alloc.vec.Vec T) :=
  if additional.val < Std.Usize.max ∧
      vector.val.length + additional.val ≤ Std.Usize.max then
    ok (.Ok (), vector)
  else
    ok (.Err .modeled, vector)

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::try_reserve"]
def alloc.vec.Vec.try_reserve
  {T : Type} (_allocator : Type) :
  alloc.vec.Vec T → Std.Usize →
    Result ((core.result.Result Unit alloc.collections.TryReserveError) ×
      alloc.vec.Vec T) :=
  alloc.vec.logical_reserve

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::try_reserve_exact"]
def alloc.vec.Vec.try_reserve_exact
  {T : Type} (_allocator : Type) :
  alloc.vec.Vec T → Std.Usize →
    Result ((core.result.Result Unit alloc.collections.TryReserveError) ×
      alloc.vec.Vec T) :=
  alloc.vec.logical_reserve

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::truncate"]
def alloc.vec.Vec.truncate
  {T : Type} (_allocator : Type) :
  alloc.vec.Vec T → Std.Usize → Result (alloc.vec.Vec T)
  | vector, length =>
    ok ⟨vector.val.take length.val, by
      have bound := vector.property
      simpa [List.length_take] using
        Nat.le_trans (Nat.min_le_right length.val vector.val.length) bound⟩

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::clear"]
def alloc.vec.Vec.clear
  {T : Type} (_allocator : Type) :
  alloc.vec.Vec T → Result (alloc.vec.Vec T)
  | _ => ok (alloc.vec.Vec.new T)

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::is_empty"]
def alloc.vec.Vec.is_empty
  {T : Type} (_allocator : Type) : alloc.vec.Vec T → Result Bool
  | vector => ok vector.val.isEmpty

@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<alloc::vec::Vec<@T>, [@U]>}::ne"]
def alloc.vec.Vec.Insts.CoreCmpPartialEqSlice.ne
  {T U : Type} (_allocator : Type) (partialEqInst : core.cmp.PartialEq T U) :
  alloc.vec.Vec T → Slice U → Result Bool
  | vector, slice =>
    core.slice.cmp.PartialEqSlice.ne partialEqInst
      (alloc.vec.Vec.deref vector) slice

@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<alloc::vec::Vec<@T>, [@U]>}::eq"]
def alloc.vec.Vec.Insts.CoreCmpPartialEqSlice.eq
  {T U : Type} (_allocator : Type) (partialEqInst : core.cmp.PartialEq T U) :
  alloc.vec.Vec T → Slice U → Result Bool
  | vector, slice =>
    core.slice.cmp.PartialEqSlice.eq partialEqInst
      (alloc.vec.Vec.deref vector) slice

namespace Greatgramma.External

@[step] theorem vec_capacity_spec {T : Type} (allocator : Type)
    (vector : alloc.vec.Vec T) :
    alloc.vec.Vec.capacity allocator vector
      ⦃ observed => observed.val = vector.val.length ⦄ := by
  simp [alloc.vec.Vec.capacity, WP.spec_ok]

theorem vec_capacity_contract {T : Type} (allocator : Type) :
    CapacityContract (@alloc.vec.Vec.capacity T allocator) := by
  intro vector
  exact vec_capacity_spec allocator vector

@[step] theorem logicalReserve_spec {T : Type} (vector : alloc.vec.Vec T)
    (additional : Std.Usize) :
    alloc.vec.logical_reserve vector additional
      ⦃ outcome vector' =>
        vector' = vector ∧
        outcome = if additional.val < Std.Usize.max ∧
            vector.val.length + additional.val ≤ Std.Usize.max then
          .Ok () else .Err .modeled ⦄ := by
  unfold alloc.vec.logical_reserve
  split <;> simp_all only [WP.spec_ok, WP.uncurry'_pair, and_self]

theorem logicalReserve_allocatorContract {T : Type} :
    AllocatorContract (@alloc.vec.logical_reserve T) := by
  intro vector additional
  unfold alloc.vec.logical_reserve
  split <;> simp_all only [WP.spec_ok, WP.uncurry'_pair, true_or,
    or_true, and_self]

theorem vec_tryReserve_allocatorContract {T : Type} (allocator : Type) :
    AllocatorContract (@alloc.vec.Vec.try_reserve T allocator) := by
  exact logicalReserve_allocatorContract

theorem vec_tryReserveExact_allocatorContract {T : Type} (allocator : Type) :
    AllocatorContract (@alloc.vec.Vec.try_reserve_exact T allocator) := by
  exact logicalReserve_allocatorContract

@[step] theorem vec_truncate_spec {T : Type} (allocator : Type)
    (vector : alloc.vec.Vec T) (length : Std.Usize) :
    alloc.vec.Vec.truncate allocator vector length
      ⦃ result => result.val = vector.val.take length.val ⦄ := by
  simp [alloc.vec.Vec.truncate, WP.spec_ok]

@[step] theorem vec_clear_spec {T : Type} (allocator : Type)
    (vector : alloc.vec.Vec T) :
    alloc.vec.Vec.clear allocator vector
      ⦃ result => result.val = [] ⦄ := by
  simp [alloc.vec.Vec.clear, alloc.vec.Vec.new, WP.spec_ok]

@[step] theorem vec_isEmpty_spec {T : Type} (allocator : Type)
    (vector : alloc.vec.Vec T) :
    alloc.vec.Vec.is_empty allocator vector
      ⦃ result => result = vector.val.isEmpty ⦄ := by
  simp [alloc.vec.Vec.is_empty, WP.spec_ok]

end Greatgramma.External
