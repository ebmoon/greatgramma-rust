import GreatgrammaCore.Types

open Aeneas Aeneas.Std Result ControlFlow Error
open GreatgrammaCore

/-! # Formatting boundary

Formatting is not part of GreatGramma's decoding semantics.  The one generated
reference to `Option`'s `Debug` implementation is kept in this isolated module:
`none` succeeds without changing the formatter, while `some` delegates to the
element's supplied `Debug` model.  No grammar theorem depends on formatter
contents.
-/

@[rust_fun "core::option::{core::fmt::Debug<core::option::Option<@T>>}::fmt"]
def core.option.Option.Insts.CoreFmtDebug.fmt
  {T : Type} (fmtDebugInst : core.fmt.Debug T) :
  Option T → core.fmt.Formatter →
    Result ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter)
  | none, formatter => ok (.Ok (), formatter)
  | some value, formatter => fmtDebugInst.fmt value formatter

namespace Greatgramma.External

@[step] theorem option_debug_none_spec {T : Type} (fmtDebugInst : core.fmt.Debug T)
    (formatter : core.fmt.Formatter) :
    core.option.Option.Insts.CoreFmtDebug.fmt fmtDebugInst none formatter
      ⦃ outcome formatter' =>
        outcome = .Ok () ∧ formatter' = formatter ⦄ := by
  simp [core.option.Option.Insts.CoreFmtDebug.fmt, WP.spec_ok]

theorem option_debug_some_eq {T : Type} (fmtDebugInst : core.fmt.Debug T)
    (value : T) (formatter : core.fmt.Formatter) :
    core.option.Option.Insts.CoreFmtDebug.fmt fmtDebugInst (some value) formatter =
      fmtDebugInst.fmt value formatter := rfl

end Greatgramma.External
