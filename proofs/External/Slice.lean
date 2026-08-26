import GreatgrammaCore.Types

open Aeneas Aeneas.Std Result ControlFlow Error
open GreatgrammaCore

/-! # Slice external models -/

@[rust_fun "core::slice::{[@T]}::last"]
def core.slice.Slice.last {T : Type} (slice : Slice T) : Result (Option T) :=
  ok slice.val.getLast?

namespace Greatgramma.External

@[step] theorem slice_last_spec {T : Type} (slice : Slice T) :
    core.slice.Slice.last slice
      ⦃ result => result = slice.val.getLast? ⦄ := by
  simp [core.slice.Slice.last, WP.spec_ok]

theorem slice_last_empty_spec {T : Type} (slice : Slice T)
    (hempty : slice.val = []) :
    core.slice.Slice.last slice ⦃ result => result = none ⦄ := by
  simp [core.slice.Slice.last, hempty, WP.spec_ok]

end Greatgramma.External
