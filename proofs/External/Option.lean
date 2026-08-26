import GreatgrammaCore.Types

open Aeneas Aeneas.Std Result ControlFlow Error
open GreatgrammaCore

/-! # `Option` external models

The models below are content-level implementations of the Rust standard
library operations used by the translated core.  Operations that invoke a
trait method propagate that method's Aeneas result; the remaining operations
are manifestly total.
-/

@[rust_fun
  "core::option::{core::clone::Clone<core::option::Option<@T>>}::clone"]
def core.option.Option.Insts.CoreCloneClone.clone
  {T : Type} (cloneCloneInst : core.clone.Clone T) :
  Option T → Result (Option T)
  | none => ok none
  | some value => do
    let cloned ← cloneCloneInst.clone value
    ok (some cloned)

@[rust_fun "core::option::{core::option::Option<@T>}::ok_or"]
def core.option.Option.ok_or
  {T : Type} {E : Type} : Option T → E → Result (core.result.Result T E)
  | none, error => ok (.Err error)
  | some value, _ => ok (.Ok value)

@[rust_fun "core::option::{core::option::Option<@T>}::and_then"]
def core.option.Option.and_then
  {T : Type} {U : Type} {F : Type} (fnOnceInst :
    core.ops.function.FnOnce F T (Option U)) :
  Option T → F → Result (Option U)
  | none, _ => ok none
  | some value, function => fnOnceInst.call_once function value

@[rust_fun "core::option::{core::option::Option<@T>}::filter"]
def core.option.Option.filter
  {T : Type} {P : Type} (fnOnceInst :
    core.ops.function.FnOnce P T Bool) :
  Option T → P → Result (Option T)
  | none, _ => ok none
  | some value, predicate => do
    let keep ← fnOnceInst.call_once predicate value
    if keep then ok (some value) else ok none

@[rust_fun "core::option::{core::option::Option<&'0 @T>}::copied"]
def core.option.OptionShared0T.copied
  {T : Type} (_markerCopyInst : core.marker.Copy T) :
  Option T → Result (Option T) := ok

@[rust_fun
  "core::option::{core::cmp::PartialEq<core::option::Option<@T>, core::option::Option<@T>>}::eq"]
def core.option.Option.Insts.CoreCmpPartialEqOption.eq
  {T : Type} (partialEqInst : core.cmp.PartialEq T T) :
  Option T → Option T → Result Bool
  | none, none => ok true
  | some left, some right => partialEqInst.eq left right
  | _, _ => ok false

@[rust_fun
  "core::option::{core::ops::try_trait::Try<core::option::Option<@T>>}::branch"]
def core.option.Option.Insts.CoreOpsTry_traitTry.branch
  {T : Type} :
  Option T → Result (core.ops.control_flow.ControlFlow
    (Option core.convert.Infallible) T)
  | none => ok (.Break none)
  | some value => ok (.Continue value)

@[rust_fun
  "core::option::{core::ops::try_trait::FromResidual<core::option::Option<@T>, core::option::Option<core::convert::Infallible>>}::from_residual"]
def core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual
  (T : Type) : Option core.convert.Infallible → Result (Option T)
  | none => ok none
  | some impossible => impossible.casesOn

namespace Greatgramma.External

@[step] theorem option_okOr_none_spec {T E : Type} (error : E) :
    core.option.Option.ok_or (T := T) none error
      ⦃ result => result = .Err error ⦄ := by
  simp [core.option.Option.ok_or, WP.spec_ok]

@[step] theorem option_okOr_some_spec {T E : Type} (value : T) (error : E) :
    core.option.Option.ok_or (some value) error
      ⦃ result => result = .Ok value ⦄ := by
  simp [core.option.Option.ok_or, WP.spec_ok]

@[step] theorem option_copied_spec {T : Type} (copyInst : core.marker.Copy T)
    (value : Option T) :
    core.option.OptionShared0T.copied copyInst value
      ⦃ result => result = value ⦄ := by
  simp [core.option.OptionShared0T.copied, WP.spec_ok]

@[step] theorem option_branch_none_spec {T : Type} :
    core.option.Option.Insts.CoreOpsTry_traitTry.branch (T := T) none
      ⦃ result => result = .Break none ⦄ := by
  simp [core.option.Option.Insts.CoreOpsTry_traitTry.branch, WP.spec_ok]

@[step] theorem option_branch_some_spec {T : Type} (value : T) :
    core.option.Option.Insts.CoreOpsTry_traitTry.branch (some value)
      ⦃ result => result = .Continue value ⦄ := by
  simp [core.option.Option.Insts.CoreOpsTry_traitTry.branch, WP.spec_ok]

@[step] theorem option_fromResidual_none_spec (T : Type) :
    core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual
      T none ⦃ result => result = none ⦄ := by
  simp [
    core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual,
    WP.spec_ok]

end Greatgramma.External
