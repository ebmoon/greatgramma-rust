import GreatgrammaCore.Types

open Aeneas Aeneas.Std Result ControlFlow Error
open GreatgrammaCore

/-! # Integer-conversion models

These definitions delegate to Aeneas's bounded unsigned-scalar conversion.
The accompanying total specifications expose the exact success/failure result,
so later proofs do not need to unfold this external boundary.
-/

@[rust_fun
  "core::convert::num::ptr_try_from_impls::{core::convert::TryFrom<usize, u32, core::num::error::TryFromIntError>}::try_from"]
def Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from
  (value : Std.U32) :
  Result (core.result.Result Std.Usize core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .Usize value

@[rust_fun
  "core::convert::num::ptr_try_from_impls::{core::convert::TryFrom<u64, usize, core::num::error::TryFromIntError>}::try_from"]
def U64.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from
  (value : Std.Usize) :
  Result (core.result.Result Std.U64 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U64 value

@[rust_fun
  "core::convert::num::{core::convert::TryFrom<u8, u16, core::num::error::TryFromIntError>}::try_from"]
def U8.Insts.CoreConvertTryFromU16TryFromIntError.try_from
  (value : Std.U16) :
  Result (core.result.Result Std.U8 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U8 value

@[rust_fun
  "core::convert::num::ptr_try_from_impls::{core::convert::TryFrom<u8, usize, core::num::error::TryFromIntError>}::try_from"]
def U8.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from
  (value : Std.Usize) :
  Result (core.result.Result Std.U8 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U8 value

namespace Greatgramma.External

@[step] theorem usize_tryFrom_u32_spec (value : Std.U32) :
    Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from value
      ⦃ result => result =
        if value.val ≤ UScalar.max .Usize then
          .Ok (UScalar.cast .Usize value)
        else .Err () ⦄ := by
  unfold Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from
    core.num.tryFromUScalar
  split <;> simp_all only [WP.spec_ok]

@[step] theorem u64_tryFrom_usize_spec (value : Std.Usize) :
    U64.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from value
      ⦃ result => result =
        if value.val ≤ UScalar.max .U64 then
          .Ok (UScalar.cast .U64 value)
        else .Err () ⦄ := by
  unfold U64.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from
    core.num.tryFromUScalar
  split <;> simp_all only [WP.spec_ok]

@[step] theorem u8_tryFrom_u16_spec (value : Std.U16) :
    U8.Insts.CoreConvertTryFromU16TryFromIntError.try_from value
      ⦃ result => result =
        if value.val ≤ UScalar.max .U8 then
          .Ok (UScalar.cast .U8 value)
        else .Err () ⦄ := by
  unfold U8.Insts.CoreConvertTryFromU16TryFromIntError.try_from
    core.num.tryFromUScalar
  split <;> simp_all only [WP.spec_ok]

@[step] theorem u8_tryFrom_usize_spec (value : Std.Usize) :
    U8.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from value
      ⦃ result => result =
        if value.val ≤ UScalar.max .U8 then
          .Ok (UScalar.cast .U8 value)
        else .Err () ⦄ := by
  unfold U8.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from
    core.num.tryFromUScalar
  split <;> simp_all only [WP.spec_ok]

end Greatgramma.External
