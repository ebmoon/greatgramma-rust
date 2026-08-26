import GreatgrammaCore.Types

open Aeneas Aeneas.Std Result ControlFlow Error
open GreatgrammaCore

/-! # `Result` external models -/

@[rust_fun "core::result::{core::result::Result<@T, @E>}::is_err"]
def core.result.Result.is_err
  {T E : Type} : core.result.Result T E → Result Bool
  | .Ok _ => ok false
  | .Err _ => ok true

@[rust_fun "core::result::{core::result::Result<@T, @E>}::ok"]
def core.result.Result.ok
  {T E : Type} : core.result.Result T E → Result (Option T)
  | .Ok value => Aeneas.Std.Result.ok (some value)
  | .Err _ => Aeneas.Std.Result.ok none

@[rust_fun "core::result::{core::result::Result<@T, @E>}::err"]
def core.result.Result.err
  {T E : Type} : core.result.Result T E → Result (Option E)
  | .Ok _ => Aeneas.Std.Result.ok none
  | .Err error => Aeneas.Std.Result.ok (some error)

@[rust_fun "core::result::{core::result::Result<@T, @E>}::map_err"]
def core.result.Result.map_err
  {T E F O : Type} (fnOnceInst : core.ops.function.FnOnce O E F) :
  core.result.Result T E → O → Result (core.result.Result T F)
  | .Ok value, _ => Aeneas.Std.Result.ok (.Ok value)
  | .Err error, operation => do
    let mapped ← fnOnceInst.call_once operation error
    Aeneas.Std.Result.ok (.Err mapped)

namespace Greatgramma.External

@[step] theorem result_isErr_ok_spec {T E : Type} (value : T) :
    core.result.Result.is_err (E := E) (.Ok value)
      ⦃ result => result = false ⦄ := by
  simp [core.result.Result.is_err, WP.spec_ok]

@[step] theorem result_isErr_err_spec {T E : Type} (error : E) :
    core.result.Result.is_err (T := T) (.Err error)
      ⦃ result => result = true ⦄ := by
  simp [core.result.Result.is_err, WP.spec_ok]

@[step] theorem result_ok_ok_spec {T E : Type} (value : T) :
    core.result.Result.ok (E := E) (.Ok value)
      ⦃ result => result = some value ⦄ := by
  simp [core.result.Result.ok, WP.spec_ok]

@[step] theorem result_ok_err_spec {T E : Type} (error : E) :
    core.result.Result.ok (T := T) (.Err error)
      ⦃ result => result = none ⦄ := by
  simp [core.result.Result.ok, WP.spec_ok]

@[step] theorem result_err_ok_spec {T E : Type} (value : T) :
    core.result.Result.err (E := E) (.Ok value)
      ⦃ result => result = none ⦄ := by
  simp [core.result.Result.err, WP.spec_ok]

@[step] theorem result_err_err_spec {T E : Type} (error : E) :
    core.result.Result.err (T := T) (.Err error)
      ⦃ result => result = some error ⦄ := by
  simp [core.result.Result.err, WP.spec_ok]

@[step] theorem result_mapErr_ok_spec {T E F O : Type}
    (fnOnceInst : core.ops.function.FnOnce O E F) (value : T)
    (operation : O) :
    core.result.Result.map_err fnOnceInst (.Ok value) operation
      ⦃ result => result = .Ok value ⦄ := by
  simp [core.result.Result.map_err, WP.spec_ok]

@[step] theorem result_mapErr_err_spec {T E F O : Type}
    (fnOnceInst : core.ops.function.FnOnce O E F) (error : E)
    (operation : O) (post : F → Prop)
    (hcall : fnOnceInst.call_once operation error ⦃ mapped => post mapped ⦄) :
    core.result.Result.map_err (T := T) fnOnceInst (.Err error) operation
      ⦃ result => ∃ mapped, result = .Err mapped ∧ post mapped ⦄ := by
  unfold core.result.Result.map_err
  apply WP.spec_bind hcall
  intro mapped hmapped
  simp [WP.spec_ok, hmapped]

end Greatgramma.External
