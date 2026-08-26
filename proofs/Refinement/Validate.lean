import GreatgrammaCore.Funs
import Invariants.Validated
import Refinement.ValidateBitmap
import Refinement.ValidatedView

open Aeneas Aeneas.Std Result

namespace Greatgramma.Refinement

/-!
Semantic specifications for the normalized validation boundary.  Unlike the
U2 feasibility lemmas, these postconditions record the structural facts that
later refinement units consume.
-/

@[step]
theorem validateId_ok_iff
    (table : GreatgrammaCore.error.ValidationTable) (index : Option Usize)
    (kind : GreatgrammaCore.error.IdKind) (id count : U32) :
    GreatgrammaCore.validate.validate_id table index kind id count
      ⦃ result => result = .Ok () ↔ id.val < count.val ⦄ := by
  unfold GreatgrammaCore.validate.validate_id
  split <;> simp_all

theorem checkLength_ok_iff
    (table : GreatgrammaCore.error.ValidationTable) (expected actual : Usize) :
    GreatgrammaCore.validate.check_length table expected actual
      ⦃ result => result = .Ok () ↔ actual.val = expected.val ⦄ := by
  unfold GreatgrammaCore.validate.check_length
  split <;> simp_all

@[step]
theorem checkLength_spec
    (table : GreatgrammaCore.error.ValidationTable) (expected actual : Usize) :
    GreatgrammaCore.validate.check_length table expected actual
      ⦃ result => match result with
        | .Ok _ => actual.val = expected.val
        | .Err _ => True ⦄ := by
  apply WP.spec_mono (checkLength_ok_iff table expected actual)
  intro result hresult
  cases result <;> simp_all

@[step]
theorem validationResult_exact_spec
    (input : Option GreatgrammaCore.error.ValidationError) :
    GreatgrammaCore.validate.validation_result input
      ⦃ result => match result with
        | .Ok _ => input = none
        | .Err error => input = some error ⦄ := by
  match input with
  | none =>
    simp [GreatgrammaCore.validate.validation_result, WP.spec_ok]
  | some error =>
    simp [GreatgrammaCore.validate.validation_result, WP.spec_ok]

@[step]
theorem checkLimit_ok_iff
    (kind : GreatgrammaCore.error.LimitKind) (actual maximum : U64) :
    GreatgrammaCore.validate.check_limit kind actual maximum
      ⦃ result => result = .Ok () ↔ actual.val ≤ maximum.val ⦄ := by
  unfold GreatgrammaCore.validate.check_limit
  split <;> simp_all

@[step]
theorem checkedAdd_spec
    (left right : U64) (calculation : GreatgrammaCore.error.ArithmeticKind) :
    GreatgrammaCore.validate.checked_add left right calculation
      ⦃ result => match result with
        | .Ok value => value.val = left.val + right.val
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.checked_add
  step*
  cases ho : o with
  | none => simp [core.option.Option.ok_or]
  | some value =>
    have hpost := o_post
    simp [ho] at hpost
    simp [core.option.Option.ok_or]
    exact hpost.2.1

theorem checkedAdd_success
    (left right : U64) (calculation : GreatgrammaCore.error.ArithmeticKind)
    (hfits : left.val + right.val ≤ U64.max) :
    GreatgrammaCore.validate.checked_add left right calculation
      ⦃ result => ∃ value : U64,
        result = .Ok value ∧ value.val = left.val + right.val ⦄ := by
  unfold GreatgrammaCore.validate.checked_add
  step*
  cases ho : o with
  | none =>
    simp [ho] at o_post
    omega
  | some value =>
    have hpost := o_post
    simp [ho] at hpost
    simp [core.option.Option.ok_or]
    exact hpost.2.1

@[step]
theorem indexAsU32_ok
    (index : Usize) (calculation : GreatgrammaCore.error.ArithmeticKind)
    (hindex : index.val ≤ U32.max) :
    GreatgrammaCore.validate.index_as_u32 index calculation
      ⦃ result => ∃ value : U32, result = .Ok value ∧ value.val = index.val ⦄ := by
  unfold GreatgrammaCore.validate.index_as_u32
  step*
  obtain ⟨value, rfl, hvalue⟩ := r_post1 hindex
  simp [core.result.Result.map_err]
  exact hvalue

theorem indexAsU32_spec
    (index : Usize) (calculation : GreatgrammaCore.error.ArithmeticKind) :
    GreatgrammaCore.validate.index_as_u32 index calculation
      ⦃ result => match result with
        | .Ok value => index.val ≤ U32.max ∧ value.val = index.val
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.index_as_u32
  simp [
    core.convert.num.ptr_try_from_impls.TryFromU32Usize.try_from,
    core.num.tryFromUScalar,
    core.result.Result.map_err,
    GreatgrammaCore.validate.index_as_u32.closure.Insts.CoreOpsFunctionFnOnceTupleTryFromIntErrorValidationError.call_once]
  split
  · simp_all
    scalar_tac
  · simp_all

@[step]
theorem checkedMul_spec
    (left right : U64) (calculation : GreatgrammaCore.error.ArithmeticKind) :
    GreatgrammaCore.validate.checked_mul left right calculation
      ⦃ result => match result with
        | .Ok value => value.val = left.val * right.val
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.checked_mul
  step*
  cases ho : o with
  | none => simp [core.option.Option.ok_or]
  | some value =>
    have hpost := o_post
    simp [ho] at hpost
    simp [core.option.Option.ok_or]
    exact hpost.2.1

@[step]
theorem checkedProduct_spec
    (left right : U32) (calculation : GreatgrammaCore.error.ArithmeticKind) :
    GreatgrammaCore.validate.checked_product left right calculation
      ⦃ result => match result with
        | .Ok value => value.val = left.val * right.val
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.checked_product
  step*
  cases ho : o with
  | none => simp [core.option.Option.ok_or]
  | some value =>
    have hpost := o_post
    simp [ho] at hpost
    simp [core.option.Option.ok_or]
    exact hpost.2.1

@[step]
theorem checkedLen_spec
    (length : Usize) (calculation : GreatgrammaCore.error.ArithmeticKind) :
    GreatgrammaCore.validate.checked_len length calculation
      ⦃ result => match result with
        | .Ok value => length.val ≤ U32.max ∧ value.val = length.val
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.checked_len
  exact indexAsU32_spec length calculation

@[step]
theorem validateByteClass_ok_iff
    (index : Usize) (byteClass classCount : U32)
    (hindex : index.val < 256) :
    GreatgrammaCore.validate.validate_byte_class index byteClass classCount
      ⦃ result => result = .Ok () ↔ byteClass.val < classCount.val ⦄ := by
  unfold GreatgrammaCore.validate.validate_byte_class
  split
  · have hconvert : index.val ≤ U8.max := by scalar_tac
    simp [
      U8.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from,
      core.num.tryFromUScalar,
      hconvert,
      core.result.Result.map_err,
      GreatgrammaCore.validate.validate_byte_class.closure.Insts.CoreOpsFunctionFnOnceTupleTryFromIntErrorValidationError.call_once,
      core.result.Result.Insts.CoreOpsTry.branch]
    scalar_tac
  · simp_all

@[step]
theorem validateByteClassesLoop_spec
    (classes : Slice U32) (classCount : U32)
    (validationError : Option GreatgrammaCore.error.ValidationError)
    (index : Usize)
    (hindex : index.val ≤ classes.val.length)
    (hlength : classes.val.length ≤ 256) :
    GreatgrammaCore.validate.validate_byte_classes_loop
      classes classCount validationError index
      ⦃ result =>
        result = none →
          validationError = none ∧
          ∀ (position : Nat), index.val ≤ position →
            (hposition : position < classes.val.length) →
            classes.val[position].val < classCount.val ⦄ := by
  unfold GreatgrammaCore.validate.validate_byte_classes_loop
  simp
  split
  · step*
    simp_all
    cases r with
    | Ok value =>
      cases value
      simp at r_post
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      apply WP.spec_mono (validateByteClassesLoop_spec classes classCount none
        indexNext (by scalar_tac) hlength)
      intro result hresult hnone position hafter hposition
      by_cases hsame : position = index.val
      · subst position
        exact r_post
      · exact (hresult hnone).2 position (by omega) hposition
    | Err error =>
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      unfold GreatgrammaCore.validate.validate_byte_classes_loop
      simp [core.option.Option.is_none]
  · simp_all
    intro _ position hafter hposition
    omega
termination_by classes.val.length - index.val
decreasing_by simp_all; omega

/-- Completeness companion for byte-class validation.  Keeping this proof
recursive avoids unfolding all 256 generated iterations in concrete examples. -/
theorem validateByteClassesLoop_success
    (classes : Slice U32) (classCount : U32) (index : Usize)
    (hlength : classes.val.length ≤ 256)
    (hvalid : ∀ (position : Nat), index.val ≤ position →
      (hposition : position < classes.val.length) →
      classes.val[position].val < classCount.val) :
    GreatgrammaCore.validate.validate_byte_classes_loop
      classes classCount none index
      ⦃ result => result = none ⦄ := by
  unfold GreatgrammaCore.validate.validate_byte_classes_loop
  simp
  split
  · rename_i hwithin
    step*
    simp_all
    simp [core.result.Result.err]
    step as ⟨indexNext⟩
    apply validateByteClassesLoop_success classes classCount indexNext hlength
    intro position hafter hposition
    exact hvalid position (by omega) hposition
  · simp
termination_by classes.val.length - index.val
decreasing_by simp_all; omega

theorem validateByteClasses_success
    (classes : Slice U32) (classCount : U32)
    (hlength : classes.val.length ≤ 256)
    (hvalid : ∀ (position : Nat) (hposition : position < classes.val.length),
      classes.val[position].val < classCount.val) :
    GreatgrammaCore.validate.validate_byte_classes classes classCount
      ⦃ result => result = .Ok () ⦄ := by
  unfold GreatgrammaCore.validate.validate_byte_classes
  apply WP.spec_bind
    (validateByteClassesLoop_success classes classCount 0#usize hlength (by
      intro position _ hposition
      exact hvalid position hposition))
  intro validationError herror
  rw [herror]
  simp [GreatgrammaCore.validate.validation_result]

@[step]
theorem validateTransition_ok_iff
    (destination : Option GreatgrammaCore.ids.DfaStateId)
    (index : Usize) (stateCount : U32) :
    GreatgrammaCore.validate.validate_transition destination index stateCount
      ⦃ result => result = .Ok () ↔
        Greatgramma.Invariants.OptionalIdInRange stateCount destination ⦄ := by
  unfold GreatgrammaCore.validate.validate_transition
  cases hdestination : destination with
  | none => simp [Greatgramma.Invariants.OptionalIdInRange]
  | some destination =>
    simp [Greatgramma.Invariants.OptionalIdInRange]
    simp [GreatgrammaCore.ids.DfaStateId.get]
    exact validateId_ok_iff _ _ _ _ _

@[step]
theorem validateGoto_ok_iff
    (destination : Option GreatgrammaCore.ids.ParserStateId)
    (index : Usize) (stateCount : U32) :
    GreatgrammaCore.validate.validate_goto destination index stateCount
      ⦃ result => result = .Ok () ↔
        Greatgramma.Invariants.OptionalIdInRange stateCount destination ⦄ := by
  unfold GreatgrammaCore.validate.validate_goto
  cases hdestination : destination with
  | none => simp [Greatgramma.Invariants.OptionalIdInRange]
  | some destination =>
    simp [Greatgramma.Invariants.OptionalIdInRange]
    simp [GreatgrammaCore.ids.ParserStateId.get]
    exact validateId_ok_iff _ _ _ _ _

@[step]
theorem validateTransitionsLoop_spec
    (transitions : Slice (Option GreatgrammaCore.ids.DfaStateId))
    (stateCount : U32)
    (validationError : Option GreatgrammaCore.error.ValidationError)
    (index : Usize) (hindex : index.val ≤ transitions.val.length) :
    GreatgrammaCore.validate.validate_transitions_loop
      transitions stateCount validationError index
      ⦃ result =>
        result = none →
          validationError = none ∧
          ∀ (position : Nat), index.val ≤ position →
            (hposition : position < transitions.val.length) →
            Greatgramma.Invariants.OptionalIdInRange stateCount
              transitions.val[position] ⦄ := by
  unfold GreatgrammaCore.validate.validate_transitions_loop
  simp
  split
  · step*
    simp_all
    cases r with
    | Ok value =>
      cases value
      simp at r_post
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      apply WP.spec_mono (validateTransitionsLoop_spec transitions stateCount none
        indexNext (by scalar_tac))
      intro result hresult hnone position hafter hposition
      by_cases hsame : position = index.val
      · subst position
        exact r_post
      · exact (hresult hnone).2 position (by omega) hposition
    | Err error =>
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      unfold GreatgrammaCore.validate.validate_transitions_loop
      simp [core.option.Option.is_none]
  · simp_all
    intro _ position hafter hposition
    omega
termination_by transitions.val.length - index.val
decreasing_by simp_all; omega

theorem validateTransitionsLoop_success
    (transitions : Slice (Option GreatgrammaCore.ids.DfaStateId))
    (stateCount : U32) (index : Usize)
    (hvalid : ∀ (position : Nat), index.val ≤ position →
      (hposition : position < transitions.val.length) →
      Greatgramma.Invariants.OptionalIdInRange stateCount
        transitions.val[position]) :
    GreatgrammaCore.validate.validate_transitions_loop
      transitions stateCount none index
      ⦃ result => result = none ⦄ := by
  unfold GreatgrammaCore.validate.validate_transitions_loop
  simp
  split
  · rename_i hwithin
    step*
    simp_all
    simp [core.result.Result.err]
    step as ⟨indexNext⟩
    apply validateTransitionsLoop_success transitions stateCount indexNext
    intro position hafter hposition
    exact hvalid position (by omega) hposition
  · simp
termination_by transitions.val.length - index.val
decreasing_by simp_all; omega

theorem validateTransitions_success
    (transitions : Slice (Option GreatgrammaCore.ids.DfaStateId))
    (stateCount : U32)
    (hvalid : ∀ (position : Nat) (hposition : position < transitions.val.length),
      Greatgramma.Invariants.OptionalIdInRange stateCount
        transitions.val[position]) :
    GreatgrammaCore.validate.validate_transitions transitions stateCount
      ⦃ result => result = .Ok () ⦄ := by
  unfold GreatgrammaCore.validate.validate_transitions
  apply WP.spec_bind
    (validateTransitionsLoop_success transitions stateCount 0#usize (by
      intro position _ hposition
      exact hvalid position hposition))
  intro validationError herror
  rw [herror]
  simp [GreatgrammaCore.validate.validation_result]

@[step]
theorem validateGotosLoop_spec
    (gotos : Slice (Option GreatgrammaCore.ids.ParserStateId))
    (stateCount : U32)
    (validationError : Option GreatgrammaCore.error.ValidationError)
    (index : Usize) (hindex : index.val ≤ gotos.val.length) :
    GreatgrammaCore.validate.validate_gotos_loop
      gotos stateCount validationError index
      ⦃ result =>
        result = none →
          validationError = none ∧
          ∀ (position : Nat), index.val ≤ position →
            (hposition : position < gotos.val.length) →
            Greatgramma.Invariants.OptionalIdInRange stateCount gotos.val[position] ⦄ := by
  unfold GreatgrammaCore.validate.validate_gotos_loop
  simp
  split
  · step*
    simp_all
    cases r with
    | Ok value =>
      cases value
      simp at r_post
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      apply WP.spec_mono (validateGotosLoop_spec gotos stateCount none
        indexNext (by scalar_tac))
      intro result hresult hnone position hafter hposition
      by_cases hsame : position = index.val
      · subst position
        exact r_post
      · exact (hresult hnone).2 position (by omega) hposition
    | Err error =>
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      unfold GreatgrammaCore.validate.validate_gotos_loop
      simp [core.option.Option.is_none]
  · simp_all
    intro _ position hafter hposition
    omega
termination_by gotos.val.length - index.val
decreasing_by simp_all; omega

theorem validateGotosLoop_success
    (gotos : Slice (Option GreatgrammaCore.ids.ParserStateId))
    (stateCount : U32) (index : Usize)
    (hvalid : ∀ (position : Nat), index.val ≤ position →
      (hposition : position < gotos.val.length) →
      Greatgramma.Invariants.OptionalIdInRange stateCount gotos.val[position]) :
    GreatgrammaCore.validate.validate_gotos_loop gotos stateCount none index
      ⦃ result => result = none ⦄ := by
  unfold GreatgrammaCore.validate.validate_gotos_loop
  simp
  split
  · rename_i hwithin
    step*
    simp_all
    simp [core.result.Result.err]
    step as ⟨indexNext⟩
    apply validateGotosLoop_success gotos stateCount indexNext
    intro position hafter hposition
    exact hvalid position (by omega) hposition
  · simp
termination_by gotos.val.length - index.val
decreasing_by simp_all; omega

theorem validateGotos_success
    (gotos : Slice (Option GreatgrammaCore.ids.ParserStateId))
    (stateCount : U32)
    (hvalid : ∀ (position : Nat) (hposition : position < gotos.val.length),
      Greatgramma.Invariants.OptionalIdInRange stateCount gotos.val[position]) :
    GreatgrammaCore.validate.validate_gotos gotos stateCount
      ⦃ result => result = .Ok () ⦄ := by
  unfold GreatgrammaCore.validate.validate_gotos
  apply WP.spec_bind (validateGotosLoop_success gotos stateCount 0#usize (by
    intro position _ hposition
    exact hvalid position hposition))
  intro validationError herror
  rw [herror]
  simp [GreatgrammaCore.validate.validation_result]

@[step]
theorem validateProductionsLoop_spec
    (productions : Slice GreatgrammaCore.normalized.Production)
    (nonterminalCount : U32)
    (validationError : Option GreatgrammaCore.error.ValidationError)
    (index : Usize) (hindex : index.val ≤ productions.val.length) :
    GreatgrammaCore.validate.validate_productions_loop
      productions nonterminalCount validationError index
      ⦃ result =>
        result = none →
          validationError = none ∧
          ∀ (position : Nat), index.val ≤ position →
            (hposition : position < productions.val.length) →
            productions.val[position].lhs.val < nonterminalCount.val ⦄ := by
  unfold GreatgrammaCore.validate.validate_productions_loop
  simp
  split
  · step*
    simp_all [
      GreatgrammaCore.normalized.Production.impl.lhs,
      GreatgrammaCore.ids.NonterminalId.get]
    step with validateId_ok_iff as ⟨r, r_post⟩
    cases r with
    | Ok value =>
      cases value
      simp at r_post
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      apply WP.spec_mono (validateProductionsLoop_spec productions nonterminalCount
        none indexNext (by scalar_tac))
      intro result hresult hnone position hafter hposition
      by_cases hsame : position = index.val
      · subst position
        exact r_post
      · exact (hresult hnone).2 position (by omega) hposition
    | Err error =>
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      unfold GreatgrammaCore.validate.validate_productions_loop
      simp [core.option.Option.is_none]
  · simp_all
    intro _ position hafter hposition
    omega
termination_by productions.val.length - index.val
decreasing_by simp_all; omega

theorem validateProductionsLoop_success
    (productions : Slice GreatgrammaCore.normalized.Production)
    (nonterminalCount : U32) (index : Usize)
    (hvalid : ∀ (position : Nat), index.val ≤ position →
      (hposition : position < productions.val.length) →
      productions.val[position].lhs.val < nonterminalCount.val) :
    GreatgrammaCore.validate.validate_productions_loop
      productions nonterminalCount none index
      ⦃ result => result = none ⦄ := by
  unfold GreatgrammaCore.validate.validate_productions_loop
  simp
  split
  · rename_i hwithin
    step*
    simp_all [GreatgrammaCore.normalized.Production.impl.lhs,
      GreatgrammaCore.ids.NonterminalId.get]
    step with validateId_ok_iff as ⟨r, r_post⟩
    have hr : r = .Ok () := r_post.mpr (hvalid index (by omega) hwithin)
    rw [hr]
    simp [core.result.Result.err]
    step as ⟨indexNext⟩
    apply validateProductionsLoop_success productions nonterminalCount indexNext
    intro position hafter hposition
    exact hvalid position (by omega) hposition
  · simp
termination_by productions.val.length - index.val
decreasing_by simp_all; omega

theorem validateProductions_success
    (productions : Slice GreatgrammaCore.normalized.Production)
    (nonterminalCount : U32)
    (hvalid : ∀ (position : Nat) (hposition : position < productions.val.length),
      productions.val[position].lhs.val < nonterminalCount.val) :
    GreatgrammaCore.validate.validate_productions productions nonterminalCount
      ⦃ result => result = .Ok () ⦄ := by
  unfold GreatgrammaCore.validate.validate_productions
  apply WP.spec_bind
    (validateProductionsLoop_success productions nonterminalCount 0#usize (by
      intro position _ hposition
      exact hvalid position hposition))
  intro validationError herror
  rw [herror]
  simp [GreatgrammaCore.validate.validation_result]

@[step]
theorem validateLexerTerminal_ok
    (terminal : Option GreatgrammaCore.ids.TerminalId) (index : Usize)
    (start : GreatgrammaCore.ids.DfaStateId) (terminalCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId) (hindex : index.val ≤ U32.max) :
    GreatgrammaCore.validate.validate_lexer_terminal
      terminal index start terminalCount eof
      ⦃ result => result = .Ok () →
        Greatgramma.Invariants.LexerTerminalWF terminalCount eof terminal ∧
        (index.val = start.val → terminal = none) ⦄ := by
  cases hterminal : terminal with
  | none =>
    simp [GreatgrammaCore.validate.validate_lexer_terminal,
      Greatgramma.Invariants.LexerTerminalWF]
  | some terminal =>
    unfold GreatgrammaCore.validate.validate_lexer_terminal
    simp [GreatgrammaCore.ids.TerminalId.get]
    step with validateId_ok_iff as ⟨r, r_post⟩
    cases r with
    | Err error =>
      simp [
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
    | Ok value =>
      cases value
      simp at r_post
      simp [core.result.Result.Insts.CoreOpsTry.branch]
      step*
      simp_all [
        Greatgramma.Invariants.LexerTerminalWF,
        GreatgrammaCore.ids.DfaStateId.new,
        GreatgrammaCore.ids.DfaStateId.Insts.CoreCmpPartialEqDfaStateId.eq,
        GreatgrammaCore.ids.TerminalId.Insts.CoreCmpPartialEqTerminalId.eq]
      split <;> simp_all
      split <;> simp_all

theorem validateLexerTerminal_success
    (terminal : Option GreatgrammaCore.ids.TerminalId) (index : Usize)
    (start : GreatgrammaCore.ids.DfaStateId) (terminalCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId)
    (hindex : index.val ≤ U32.max)
    (hvalid : Greatgramma.Invariants.LexerTerminalWF terminalCount eof terminal)
    (hstart : index.val = start.val → terminal = none) :
    GreatgrammaCore.validate.validate_lexer_terminal
      terminal index start terminalCount eof
      ⦃ result => result = .Ok () ⦄ := by
  cases terminal with
  | none =>
    simp [GreatgrammaCore.validate.validate_lexer_terminal]
  | some terminal =>
    have hvalid' : terminal.val < terminalCount.val ∧ terminal ≠ eof := by
      simpa [Greatgramma.Invariants.LexerTerminalWF] using hvalid
    unfold GreatgrammaCore.validate.validate_lexer_terminal
    simp [GreatgrammaCore.ids.TerminalId.get]
    step with validateId_ok_iff as ⟨r, r_post⟩
    have hr : r = .Ok () := r_post.mpr hvalid'.1
    rw [hr]
    simp [core.result.Result.Insts.CoreOpsTry.branch]
    step*
    have hnotStart : index.val ≠ start.val := by
      intro heq
      have := hstart heq
      simp at this
    simp_all [GreatgrammaCore.ids.DfaStateId.new,
      GreatgrammaCore.ids.DfaStateId.Insts.CoreCmpPartialEqDfaStateId.eq,
      GreatgrammaCore.ids.TerminalId.Insts.CoreCmpPartialEqTerminalId.eq]

@[step]
theorem validateLexerTerminalsLoop_spec
    (terminals : Slice (Option GreatgrammaCore.ids.TerminalId))
    (start : GreatgrammaCore.ids.DfaStateId) (terminalCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId)
    (validationError : Option GreatgrammaCore.error.ValidationError)
    (index : Usize) (hindex : index.val ≤ terminals.val.length)
    (hlength : terminals.val.length ≤ U32.max) :
    GreatgrammaCore.validate.validate_lexer_terminals_loop
      terminals start terminalCount eof validationError index
      ⦃ result =>
        result = none →
          validationError = none ∧
          ∀ (position : Nat), index.val ≤ position →
            (hposition : position < terminals.val.length) →
            Greatgramma.Invariants.LexerTerminalWF terminalCount eof
              terminals.val[position] ∧
            (position = start.val → terminals.val[position] = none) ⦄ := by
  unfold GreatgrammaCore.validate.validate_lexer_terminals_loop
  simp
  split
  · step*
    simp_all
    cases r with
    | Ok value =>
      cases value
      simp at r_post
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      apply WP.spec_mono (validateLexerTerminalsLoop_spec terminals start
        terminalCount eof none indexNext (by scalar_tac) hlength)
      intro result hresult hnone position hafter hposition
      by_cases hsame : position = index.val
      · subst position
        exact r_post
      ·
        have hnext := (hresult hnone).2 position (by omega) hposition
        refine ⟨hnext.1, ?_⟩
        intro hstart
        simpa [hstart] using hnext.2 hstart
    | Err error =>
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      unfold GreatgrammaCore.validate.validate_lexer_terminals_loop
      simp [core.option.Option.is_none]
  · simp_all
    intro _ position hafter hposition
    omega
termination_by terminals.val.length - index.val
decreasing_by simp_all; omega

theorem validateLexerTerminalsLoop_success
    (terminals : Slice (Option GreatgrammaCore.ids.TerminalId))
    (start : GreatgrammaCore.ids.DfaStateId) (terminalCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId) (index : Usize)
    (hlength : terminals.val.length ≤ U32.max)
    (hvalid : ∀ (position : Nat), index.val ≤ position →
      (hposition : position < terminals.val.length) →
      Greatgramma.Invariants.LexerTerminalWF terminalCount eof
        terminals.val[position] ∧
      (position = start.val → terminals.val[position] = none)) :
    GreatgrammaCore.validate.validate_lexer_terminals_loop
      terminals start terminalCount eof none index
      ⦃ result => result = none ⦄ := by
  unfold GreatgrammaCore.validate.validate_lexer_terminals_loop
  simp
  split
  · rename_i hwithin
    step
    simp_all
    apply WP.spec_bind (validateLexerTerminal_success terminals.val[index.val]
      index start terminalCount eof (by omega)
      (hvalid index.val (by omega) hwithin).1
      (by
        intro heq
        simpa [heq] using (hvalid index.val (by omega) hwithin).2 heq))
    intro r hr
    rw [hr]
    simp [core.result.Result.err]
    step as ⟨indexNext⟩
    apply validateLexerTerminalsLoop_success terminals start terminalCount eof
      indexNext hlength
    intro position hafter hposition
    have hv := hvalid position (by omega) hposition
    refine ⟨hv.1, ?_⟩
    intro heq
    simpa [heq] using hv.2 heq
  · simp
termination_by terminals.val.length - index.val
decreasing_by simp_all; omega

theorem validateLexerTerminals_success
    (terminals : Slice (Option GreatgrammaCore.ids.TerminalId))
    (start : GreatgrammaCore.ids.DfaStateId) (terminalCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId)
    (hlength : terminals.val.length ≤ U32.max)
    (hvalid : ∀ (position : Nat) (hposition : position < terminals.val.length),
      Greatgramma.Invariants.LexerTerminalWF terminalCount eof
          terminals.val[position] ∧
        (position = start.val → terminals.val[position] = none)) :
    GreatgrammaCore.validate.validate_lexer_terminals
      terminals start terminalCount eof
      ⦃ result => result = .Ok () ⦄ := by
  unfold GreatgrammaCore.validate.validate_lexer_terminals
  apply WP.spec_bind
    (validateLexerTerminalsLoop_success terminals start terminalCount eof
      0#usize hlength (by
        intro position _ hposition
        exact hvalid position hposition))
  intro validationError herror
  rw [herror]
  simp [GreatgrammaCore.validate.validation_result]

@[step]
theorem validateIgnoredTerminal_ok_iff
    (terminal : GreatgrammaCore.ids.TerminalId) (index : Usize)
    (terminalCount : U32) (eof : GreatgrammaCore.ids.TerminalId) :
    GreatgrammaCore.validate.validate_ignored_terminal
      terminal index terminalCount eof
      ⦃ result => result = .Ok () ↔
        terminal.val < terminalCount.val ∧ terminal ≠ eof ⦄ := by
  unfold GreatgrammaCore.validate.validate_ignored_terminal
  simp [
    GreatgrammaCore.ids.TerminalId.get,
    GreatgrammaCore.validate.validate_id,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
    GreatgrammaCore.ids.TerminalId.Insts.CoreCmpPartialEqTerminalId.eq,
    GreatgrammaCore.ids.TerminalId]
  split <;> simp_all
  split <;> simp_all

@[step]
theorem validateIgnoredTerminalsLoop_spec
    (terminals : Slice GreatgrammaCore.ids.TerminalId)
    (terminalCount : U32) (eof : GreatgrammaCore.ids.TerminalId)
    (validationError : Option GreatgrammaCore.error.ValidationError)
    (index : Usize) (hindex : index.val ≤ terminals.val.length) :
    GreatgrammaCore.validate.validate_ignored_terminals_loop
      terminals terminalCount eof validationError index
      ⦃ result =>
        result = none →
          validationError = none ∧
          ∀ (position : Nat), index.val ≤ position →
            (hposition : position < terminals.val.length) →
            terminals.val[position].val < terminalCount.val ∧
              terminals.val[position].val ≠ eof.val ⦄ := by
  unfold GreatgrammaCore.validate.validate_ignored_terminals_loop
  simp
  split
  · step*
    simp_all
    cases r with
    | Ok value =>
      cases value
      simp at r_post
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      apply WP.spec_mono (validateIgnoredTerminalsLoop_spec terminals terminalCount
        eof none indexNext (by scalar_tac))
      intro result hresult hnone position hafter hposition
      by_cases hsame : position = index.val
      · subst position
        exact r_post
      · exact (hresult hnone).2 position (by omega) hposition
    | Err error =>
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      unfold GreatgrammaCore.validate.validate_ignored_terminals_loop
      simp [core.option.Option.is_none]
  · simp_all
    intro _ position hafter hposition
    omega
termination_by terminals.val.length - index.val
decreasing_by simp_all; omega

@[step]
theorem validateAction_ok
    (action : GreatgrammaCore.normalized.Action) (index terminalWidth : Usize)
    (stateCount productionCount terminalCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId)
    (hwidth : terminalWidth.val = terminalCount.val)
    (heof : eof.val < terminalCount.val) :
    GreatgrammaCore.validate.validate_action action index terminalWidth
      stateCount productionCount eof
      ⦃ result => result = .Ok () →
        Greatgramma.Invariants.ActionWF stateCount productionCount terminalCount
          eof index.val action ⦄ := by
  cases haction : action with
  | Error =>
    simp [GreatgrammaCore.validate.validate_action,
      Greatgramma.Invariants.ActionWF]
  | Shift destination =>
    unfold GreatgrammaCore.validate.validate_action
    simp [GreatgrammaCore.ids.ParserStateId.get]
    apply WP.spec_mono (validateId_ok_iff
      GreatgrammaCore.error.ValidationTable.ParserActions (some index)
      GreatgrammaCore.error.IdKind.ParserState destination stateCount)
    intro result hresult hok
    exact hresult.mp hok
  | Reduce production rank =>
    unfold GreatgrammaCore.validate.validate_action
    simp [GreatgrammaCore.ids.ProductionId.get]
    apply WP.spec_mono (validateId_ok_iff
      GreatgrammaCore.error.ValidationTable.ParserActions (some index)
      GreatgrammaCore.error.IdKind.Production production productionCount)
    intro result hresult hok
    exact hresult.mp hok
  | Accept =>
    unfold GreatgrammaCore.validate.validate_action
    simp
    step as ⟨column, hcolumnEq⟩
    have hcolumn : column.val ≤ U32.max := by
      have hpositive : 0 < terminalWidth.val := by omega
      have hmod : column.val < terminalWidth.val := by
        rw [hcolumnEq]
        exact Nat.mod_lt index.val hpositive
      have hcountMax : terminalCount.val ≤ U32.max := by scalar_tac
      omega
    step*
    simp_all [
      core.result.Result.Insts.CoreOpsTry.branch,
      GreatgrammaCore.ids.TerminalId.new,
      core.cmp.PartialEq.ne.trait_default,
      core.cmp.PartialEq.ne.default,
      GreatgrammaCore.ids.TerminalId.Insts.CoreCmpPartialEqTerminalId.eq]
    split
    · simp_all [Greatgramma.Invariants.ActionWF]
      omega
    · step as ⟨row⟩
      unfold GreatgrammaCore.validate.index_as_u32
      simp [
        core.convert.num.ptr_try_from_impls.TryFromU32Usize.try_from,
        core.num.tryFromUScalar,
        core.result.Result.map_err,
        GreatgrammaCore.validate.index_as_u32.closure.Insts.CoreOpsFunctionFnOnceTupleTryFromIntErrorValidationError.call_once,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        GreatgrammaCore.ids.ParserStateId.new]
      split <;> simp

@[step]
theorem validateActionsLoop_spec
    (actions : Slice GreatgrammaCore.normalized.Action)
    (stateCount productionCount terminalCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId) (terminalWidth : Usize)
    (validationError : Option GreatgrammaCore.error.ValidationError)
    (index : Usize) (hindex : index.val ≤ actions.val.length)
    (hwidth : terminalWidth.val = terminalCount.val)
    (heof : eof.val < terminalCount.val) :
    GreatgrammaCore.validate.validate_actions_loop actions stateCount
      productionCount eof terminalWidth validationError index
      ⦃ result =>
        result = none →
          validationError = none ∧
          ∀ (position : Nat), index.val ≤ position →
            (hposition : position < actions.val.length) →
            Greatgramma.Invariants.ActionWF stateCount productionCount
              terminalCount eof position actions.val[position] ⦄ := by
  unfold GreatgrammaCore.validate.validate_actions_loop
  simp
  split
  · step*
    simp_all
    cases r with
    | Ok value =>
      cases value
      simp at r_post
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      apply WP.spec_mono (validateActionsLoop_spec actions stateCount
        productionCount terminalCount eof terminalWidth none indexNext
        (by scalar_tac) hwidth heof)
      intro result hresult hnone position hafter hposition
      by_cases hsame : position = index.val
      · subst position
        exact r_post
      · exact (hresult hnone).2 position (by omega) hposition
    | Err error =>
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      unfold GreatgrammaCore.validate.validate_actions_loop
      simp [core.option.Option.is_none]
  · simp_all
    intro _ position hafter hposition
    omega
termination_by actions.val.length - index.val
decreasing_by simp_all; omega

@[step]
theorem validateTokenEntry_ok
    (token : GreatgrammaCore.normalized.TokenEntry) (index : Usize)
    (state : GreatgrammaCore.validate.TokenValidation)
    (limits : GreatgrammaCore.limits.ValidationLimits)
    (hindex : index.val ≤ U32.max) :
    GreatgrammaCore.validate.validate_token_entry token index state limits
      ⦃ output => output.1 = .Ok () →
        Greatgramma.Invariants.TokenEntryWF token ∧
        (output.2.has_ordinary = true ↔
          state.has_ordinary = true ∨
            Greatgramma.Invariants.IsOrdinary token) ∧
        (output.2.has_eos = true ↔
          state.has_eos = true ∨ Greatgramma.Invariants.IsEos token) ⦄ := by
  cases htoken : token with
  | Bytes bytes =>
    unfold GreatgrammaCore.validate.validate_token_entry
    have hlength : bytes.val.length ≤ U64.max := by
      have hvec := alloc.vec.Vec.len_ineq bytes
      cases System.Platform.numBits_eq with
      | inl _ =>
        simp_all [Usize.max, Usize.numBits, U64.max, U64.numBits]
        omega
      | inr _ =>
        simp_all [Usize.max, Usize.numBits, U64.max, U64.numBits]
    step* <;>
      simp_all [
        Greatgramma.Invariants.TokenEntryWF,
        Greatgramma.Invariants.IsOrdinary,
        Greatgramma.Invariants.IsEos,
        GreatgrammaCore.ids.TokenId.new,
        core.result.Result.map_err,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
    all_goals step*; try simp_all
    all_goals
      cases r4 with
      | Err error =>
        simp
      | Ok value =>
        simp at r4_post
        simp
        step*; simp_all
        all_goals
          cases r5 with
          | Err error =>
            simp
          | Ok value5 =>
            cases value5
            simp at r5_post
            simp
            step*; simp_all
            all_goals
              cases r6 with
              | Err error =>
                simp
              | Ok value6 =>
                simp at r6_post
                simp
                step*
  | Eos =>
    unfold GreatgrammaCore.validate.validate_token_entry
    step*;
      simp_all [
        Greatgramma.Invariants.TokenEntryWF,
        Greatgramma.Invariants.IsOrdinary,
        Greatgramma.Invariants.IsEos]

theorem validateEosTokenEntry_success
    (index : Usize) (state : GreatgrammaCore.validate.TokenValidation)
    (limits : GreatgrammaCore.limits.ValidationLimits)
    (hwork : state.work.val + 1 ≤ limits.max_work.val) :
    GreatgrammaCore.validate.validate_token_entry
      .Eos index state limits
      ⦃ output =>
        output.1 = .Ok () ∧
          output.2.has_ordinary = state.has_ordinary ∧
          output.2.has_eos = true ∧
          output.2.total_bytes = state.total_bytes ∧
          output.2.work.val = state.work.val + 1 ⦄ := by
  unfold GreatgrammaCore.validate.validate_token_entry
  apply WP.spec_bind (checkedAdd_success state.work 1#u64
    GreatgrammaCore.error.ArithmeticKind.Work (by
      have hmax := limits.max_work.hmax
      scalar_tac))
  intro r hr
  obtain ⟨work, hrResult, hworkValue⟩ := hr
  rw [hrResult]
  simp [core.result.Result.Insts.CoreOpsTry.branch]
  apply WP.spec_bind (checkLimit_ok_iff GreatgrammaCore.error.LimitKind.Work
    work limits.max_work)
  intro limitResult hlimitResult
  have hlimitOk : limitResult = .Ok () := hlimitResult.mpr (by
    rw [hworkValue]
    simpa using hwork)
  rw [hlimitOk]
  simp [WP.spec_ok, hworkValue]

theorem validateBytesTokenEntry_success
    (bytes : alloc.vec.Vec U8) (index : Usize)
    (state : GreatgrammaCore.validate.TokenValidation)
    (limits : GreatgrammaCore.limits.ValidationLimits)
    (hnonempty : bytes.val ≠ [])
    (hbytes : state.total_bytes.val + bytes.val.length ≤
      limits.max_token_bytes.val)
    (hwork : state.work.val + 1 + bytes.val.length ≤
      limits.max_work.val) :
    GreatgrammaCore.validate.validate_token_entry
      (.Bytes bytes) index state limits
      ⦃ output =>
        output.1 = .Ok () ∧
          output.2.has_ordinary = true ∧
          output.2.has_eos = state.has_eos ∧
          output.2.total_bytes.val =
            state.total_bytes.val + bytes.val.length ∧
          output.2.work.val = state.work.val + 1 + bytes.val.length ⦄ := by
  unfold GreatgrammaCore.validate.validate_token_entry
  apply WP.spec_bind (checkedAdd_success state.work 1#u64
    GreatgrammaCore.error.ArithmeticKind.Work (by
      have hmax := limits.max_work.hmax
      scalar_tac))
  intro firstWorkResult hfirstWorkResult
  obtain ⟨firstWork, hfirstWorkOk, hfirstWorkValue⟩ := hfirstWorkResult
  have hfirstWorkValue' : firstWork.val = state.work.val + 1 := by
    simpa using hfirstWorkValue
  rw [hfirstWorkOk]
  simp [core.result.Result.Insts.CoreOpsTry.branch]
  apply WP.spec_bind (checkLimit_ok_iff GreatgrammaCore.error.LimitKind.Work
    firstWork limits.max_work)
  intro firstLimitResult hfirstLimitResult
  have hfirstLimitOk : firstLimitResult = .Ok () := hfirstLimitResult.mpr (by
    rw [hfirstWorkValue']
    exact le_trans (Nat.le_add_right _ _) hwork)
  rw [hfirstLimitOk]
  simp
  apply WP.spec_bind (Greatgramma.External.vec_isEmpty_spec Global bytes)
  intro isEmpty hisEmpty
  have hisEmptyFalse : isEmpty = false := by
    rw [hisEmpty]
    simpa using hnonempty
  rw [hisEmptyFalse]
  simp
  let lengthUsize := alloc.vec.Vec.len bytes
  have hlengthU64 : lengthUsize.val ≤ UScalar.max .U64 := by
    dsimp [lengthUsize]
    have hlimitMax := limits.max_token_bytes.hmax
    scalar_tac
  apply WP.spec_bind (Greatgramma.External.u64_tryFrom_usize_spec lengthUsize)
  intro lengthResult hlengthResult
  rw [if_pos hlengthU64] at hlengthResult
  rw [hlengthResult]
  simp [core.result.Result.map_err]
  let lengthU64 := UScalar.cast .U64 lengthUsize
  have hlengthValue : lengthU64.val = bytes.val.length := by
    simp [lengthU64, lengthUsize, alloc.vec.Vec.len]
  apply WP.spec_bind (checkedAdd_success state.total_bytes lengthU64
    GreatgrammaCore.error.ArithmeticKind.TokenBytes (by
      rw [hlengthValue]
      have hmax := limits.max_token_bytes.hmax
      scalar_tac))
  intro totalResult htotalResult
  obtain ⟨totalBytes, htotalOk, htotalValue⟩ := htotalResult
  rw [htotalOk]
  simp
  apply WP.spec_bind (checkLimit_ok_iff GreatgrammaCore.error.LimitKind.TokenBytes
    totalBytes limits.max_token_bytes)
  intro byteLimitResult hbyteLimitResult
  have hbyteLimitOk : byteLimitResult = .Ok () := hbyteLimitResult.mpr (by
    rw [htotalValue, hlengthValue]
    exact hbytes)
  rw [hbyteLimitOk]
  simp
  apply WP.spec_bind (checkedAdd_success firstWork lengthU64
    GreatgrammaCore.error.ArithmeticKind.Work (by
      rw [hfirstWorkValue', hlengthValue]
      have hmax := limits.max_work.hmax
      scalar_tac))
  intro finalWorkResult hfinalWorkResult
  obtain ⟨finalWork, hfinalWorkOk, hfinalWorkValue⟩ := hfinalWorkResult
  rw [hfinalWorkOk]
  simp
  apply WP.spec_bind (checkLimit_ok_iff GreatgrammaCore.error.LimitKind.Work
    finalWork limits.max_work)
  intro finalLimitResult hfinalLimitResult
  have hfinalLimitOk : finalLimitResult = .Ok () := hfinalLimitResult.mpr (by
    rw [hfinalWorkValue, hfirstWorkValue', hlengthValue]
    exact hwork)
  rw [hfinalLimitOk]
  simp [WP.spec_ok, htotalValue, hfinalWorkValue, hfirstWorkValue', hlengthValue]

@[step]
theorem validateTokensLoop_spec
    (tokens : Slice GreatgrammaCore.normalized.TokenEntry)
    (limits : GreatgrammaCore.limits.ValidationLimits)
    (state : GreatgrammaCore.validate.TokenValidation)
    (validationError : Option GreatgrammaCore.error.ValidationError)
    (index : Usize)
    (hindex : index.val ≤ tokens.val.length)
    (hlength : tokens.val.length ≤ U32.max) :
    GreatgrammaCore.validate.validate_tokens_loop
      tokens limits state validationError index
      ⦃ output =>
        output.2 = none →
          validationError = none ∧
          (∀ (position : Nat), index.val ≤ position →
            (hposition : position < tokens.val.length) →
            Greatgramma.Invariants.TokenEntryWF tokens.val[position]) ∧
          (output.1.has_ordinary = true ↔
            state.has_ordinary = true ∨
              ∃ (position : Nat) (token : GreatgrammaCore.normalized.TokenEntry),
                index.val ≤ position ∧ tokens.val[position]? = some token ∧
                  Greatgramma.Invariants.IsOrdinary token) ∧
          (output.1.has_eos = true ↔
            state.has_eos = true ∨
              ∃ (position : Nat) (token : GreatgrammaCore.normalized.TokenEntry),
                index.val ≤ position ∧ tokens.val[position]? = some token ∧
                  Greatgramma.Invariants.IsEos token) ⦄ := by
  unfold GreatgrammaCore.validate.validate_tokens_loop
  simp
  split
  · rename_i hwithin
    step*
    simp_all
    cases r with
    | Ok value =>
      cases value
      simp at r_post
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      apply WP.spec_mono (validateTokensLoop_spec tokens limits state1 none
        indexNext (by scalar_tac) hlength)
      intro output houtput hnone
      have hsuffix := houtput hnone
      constructor
      · intro position hafter hposition
        by_cases hsame : position = index.val
        · subst position
          exact r_post.1
        · exact hsuffix.2.1 position (by omega) hposition
      · constructor
        · rw [hsuffix.2.2.1, r_post.2.1]
          constructor
          · rintro ((hstate | hordinary) |
                ⟨position, token, hafter, hlookup, hp⟩)
            · exact Or.inl hstate
            · exact Or.inr ⟨index.val, le_rfl, tokens.val[index.val],
                by simp [hwithin], hordinary⟩
            · exact Or.inr ⟨position, by omega, token, hlookup, hp⟩
          · rintro (hstate |
                ⟨position, hafter, token, hlookup, hordinary⟩)
            · exact Or.inl (Or.inl hstate)
            · by_cases hsame : position = index.val
              · subst position
                have htoken : tokens.val[index.val] = token := by
                  simpa [hwithin] using hlookup
                subst token
                exact Or.inl (Or.inr hordinary)
              · exact Or.inr
                  ⟨position, token, by omega, hlookup, hordinary⟩
        · rw [hsuffix.2.2.2, r_post.2.2]
          constructor
          · rintro ((hstate | heos) |
                ⟨position, token, hafter, hlookup, hp⟩)
            · exact Or.inl hstate
            · exact Or.inr ⟨index.val, le_rfl, tokens.val[index.val],
                by simp [hwithin], heos⟩
            · exact Or.inr ⟨position, by omega, token, hlookup, hp⟩
          · rintro (hstate | ⟨position, hafter, token, hlookup, heos⟩)
            · exact Or.inl (Or.inl hstate)
            · by_cases hsame : position = index.val
              · subst position
                have htoken : tokens.val[index.val] = token := by
                  simpa [hwithin] using hlookup
                subst token
                exact Or.inl (Or.inr heos)
              · exact Or.inr ⟨position, token, by omega, hlookup, heos⟩
    | Err error =>
      simp [core.result.Result.err]
      step as ⟨indexNext⟩
      unfold GreatgrammaCore.validate.validate_tokens_loop
      simp [core.option.Option.is_none]
  · simp_all [List.getElem?_eq_some_iff]
    intro hvalidation
    constructor
    · intro position hafter hposition
      omega
    constructor
    · intro position hafter hposition hordinary
      omega
    · intro position hafter hposition heos
      omega
termination_by tokens.val.length - index.val
decreasing_by simp_all; omega

@[step]
theorem validateTokens_ok
    (tokens : Slice GreatgrammaCore.normalized.TokenEntry)
    (initialWork : U64)
    (limits : GreatgrammaCore.limits.ValidationLimits)
    (hlength : tokens.val.length ≤ U32.max) :
    GreatgrammaCore.validate.validate_tokens tokens initialWork limits
      ⦃ result => match result with
        | .Ok _ =>
          (∀ (position : Nat) (hposition : position < tokens.val.length),
            Greatgramma.Invariants.TokenEntryWF tokens.val[position]) ∧
          (∃ (position : Nat) (token : GreatgrammaCore.normalized.TokenEntry),
            tokens.val[position]? = some token ∧
              Greatgramma.Invariants.IsOrdinary token) ∧
          (∃ (position : Nat) (token : GreatgrammaCore.normalized.TokenEntry),
            tokens.val[position]? = some token ∧
              Greatgramma.Invariants.IsEos token)
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate_tokens
  step
  cases herror : validation_error with
  | some error => simp
  | none =>
    have hloop := state_post herror
    simp
    split
    · rename_i hordinary
      split
      · rename_i heos
        refine ⟨?_, ?_, ?_⟩
        · intro position hposition
          exact hloop.1 position (by omega) hposition
        · have hflag : state.has_ordinary = true := by
            simpa using hordinary
          have hwitness := (hloop.2.1.mp hflag)
          rcases hwitness with hfalse | hwitness
          · simp at hfalse
          · rcases hwitness with ⟨position, token, _, hlookup, hp⟩
            exact ⟨position, token, hlookup, hp⟩
        · have hflag : state.has_eos = true := by
            simpa using heos
          have hwitness := (hloop.2.2.mp hflag)
          rcases hwitness with hfalse | hwitness
          · simp at hfalse
          · rcases hwitness with ⟨position, token, _, hlookup, hp⟩
            exact ⟨position, token, hlookup, hp⟩
      · simp
    · simp

@[step]
theorem fromU64U32_lift_spec (value : U32) :
    lift (core.convert.num.FromU64U32.from value)
      ⦃ result => result.val = value.val ⦄ := by
  simp [lift, WP.spec_ok, core.convert.num.FromU64U32.from_val_eq]

@[step]
theorem tryBranch_exact_spec {T E : Type} (input : core.result.Result T E) :
    core.result.Result.Insts.CoreOpsTry.branch input
      ⦃ output => match output with
        | .Continue value => input = .Ok value
        | .Break residual =>
          ∃ error, input = .Err error ∧ residual = .Err error ⦄ := by
  match input with
  | .Ok value =>
    simp [core.result.Result.Insts.CoreOpsTry.branch, WP.spec_ok]
  | .Err error =>
    simp [core.result.Result.Insts.CoreOpsTry.branch, WP.spec_ok]

@[step]
theorem optionOkOr_exact_spec {T E : Type} (input : Option T) (error : E) :
    core.option.Option.ok_or input error
      ⦃ output => match output with
        | .Ok value => input = some value
        | .Err observed => input = none ∧ observed = error ⦄ := by
  match input with
  | none => simp [core.option.Option.ok_or, WP.spec_ok]
  | some value => simp [core.option.Option.ok_or, WP.spec_ok]

def expectedValidationBaseWork
    (sizes : GreatgrammaCore.validate.DeclaredSizes) : Nat :=
  GreatgrammaCore.validate.LOGICAL_FIXED_SCALARS.val +
    GreatgrammaCore.validate.BYTE_CLASS_TABLE_LEN.val +
    sizes.lexer_states.val + sizes.lexer_cells.val +
    sizes.parser_action_cells.val *
      GreatgrammaCore.validate.LOGICAL_ACTION_WORK.val +
    sizes.parser_goto_cells.val + sizes.production_count.val +
    sizes.ignored_terminal_count.val

@[step]
theorem validationBaseWork_spec
    (sizes : GreatgrammaCore.validate.DeclaredSizes) :
    GreatgrammaCore.validate.validation_base_work sizes
      ⦃ result => match result with
        | .Ok value => value.val = expectedValidationBaseWork sizes
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validation_base_work
  step*
  all_goals simp_all [expectedValidationBaseWork]

def expectedLogicalBytes
    (sizes : GreatgrammaCore.validate.DeclaredSizes) : Nat :=
  sizes.token_count.val + sizes.token_bytes.val +
    GreatgrammaCore.validate.LOGICAL_FIXED_SCALARS.val *
      GreatgrammaCore.validate.LOGICAL_ID_BYTES.val +
    GreatgrammaCore.validate.BYTE_CLASS_TABLE_LEN.val *
      GreatgrammaCore.validate.LOGICAL_ID_BYTES.val +
    sizes.lexer_cells.val * GreatgrammaCore.validate.LOGICAL_ID_BYTES.val +
    sizes.lexer_states.val * GreatgrammaCore.validate.LOGICAL_ID_BYTES.val +
    sizes.parser_action_cells.val *
      GreatgrammaCore.validate.LOGICAL_ACTION_BYTES.val +
    sizes.parser_goto_cells.val *
      GreatgrammaCore.validate.LOGICAL_ID_BYTES.val +
    sizes.production_count.val *
      GreatgrammaCore.validate.LOGICAL_PRODUCTION_BYTES.val +
    sizes.ignored_terminal_count.val *
      GreatgrammaCore.validate.LOGICAL_ID_BYTES.val

@[step]
theorem logicalBytes_spec (sizes : GreatgrammaCore.validate.DeclaredSizes) :
    GreatgrammaCore.validate.logical_bytes sizes
      ⦃ result => match result with
        | .Ok value => value.val = expectedLogicalBytes sizes
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.logical_bytes
  step*
  all_goals simp_all [expectedLogicalBytes]

structure DeclaredSizesWF
    (tokens : Slice GreatgrammaCore.normalized.TokenEntry)
    (lexer : GreatgrammaCore.normalized.LexerDfa)
    (lalr : GreatgrammaCore.normalized.LalrTable)
    (sizes : GreatgrammaCore.validate.DeclaredSizes) : Prop where
  tokenCount : sizes.token_count.val = tokens.val.length
  tokensNonempty : tokens.val ≠ []
  tokenEntries :
    ∀ (position : Nat) (hposition : position < tokens.val.length),
      Greatgramma.Invariants.TokenEntryWF tokens.val[position]
  hasOrdinary :
    ∃ (position : Nat) (token : GreatgrammaCore.normalized.TokenEntry),
      tokens.val[position]? = some token ∧
        Greatgramma.Invariants.IsOrdinary token
  hasEos :
    ∃ (position : Nat) (token : GreatgrammaCore.normalized.TokenEntry),
      tokens.val[position]? = some token ∧ Greatgramma.Invariants.IsEos token
  lexerCells :
    sizes.lexer_cells.val = lexer.state_count.val * lexer.class_count.val
  parserActionCells :
    sizes.parser_action_cells.val =
      lalr.dimensions.state_count.val * lalr.dimensions.terminal_count.val
  parserGotoCells :
    sizes.parser_goto_cells.val =
      lalr.dimensions.state_count.val * lalr.dimensions.nonterminal_count.val
  productionCount : sizes.production_count.val = lalr.productions.val.length

@[step]
theorem validateDeclaredSizes_spec
    (tokens : Slice GreatgrammaCore.normalized.TokenEntry)
    (lexer : GreatgrammaCore.normalized.LexerDfa)
    (lalr : GreatgrammaCore.normalized.LalrTable)
    (limits : GreatgrammaCore.limits.ValidationLimits) :
    GreatgrammaCore.validate.validate_declared_sizes tokens lexer lalr limits
      ⦃ result => match result with
        | .Ok sizes => DeclaredSizesWF tokens lexer lalr sizes
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate_declared_sizes
  step*
  all_goals simp_all
  exact {
    tokenCount := r_post.2
    tokensNonempty := b_post
    tokenEntries := r18_post.1
    hasOrdinary := r18_post.2.1
    hasEos := r18_post.2.2
    lexerCells := r10_post
    parserActionCells := r12_post
    parserGotoCells := r13_post
    productionCount := r7_post.2
  }

@[step]
theorem validateIgnoredTerminals_spec
    (terminals : Slice GreatgrammaCore.ids.TerminalId)
    (terminalCount : U32) (eof : GreatgrammaCore.ids.TerminalId) :
    GreatgrammaCore.validate.validate_ignored_terminals
      terminals terminalCount eof
      ⦃ result => match result with
        | .Ok _ =>
          ∀ (position : Nat) (hposition : position < terminals.val.length),
            terminals.val[position].val < terminalCount.val ∧
              terminals.val[position] ≠ eof
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate_ignored_terminals
  step*
  cases result <;> simp_all

@[step]
theorem validateByteClasses_spec
    (classes : Slice U32) (classCount : U32)
    (hlength : classes.val.length ≤ 256) :
    GreatgrammaCore.validate.validate_byte_classes classes classCount
      ⦃ result => match result with
        | .Ok _ =>
          ∀ (position : Nat) (hposition : position < classes.val.length),
            classes.val[position].val < classCount.val
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate_byte_classes
  step*
  cases result <;> simp_all

@[step]
theorem validateTransitions_spec
    (transitions : Slice (Option GreatgrammaCore.ids.DfaStateId))
    (stateCount : U32) :
    GreatgrammaCore.validate.validate_transitions transitions stateCount
      ⦃ result => match result with
        | .Ok _ =>
          ∀ (position : Nat) (hposition : position < transitions.val.length),
            Greatgramma.Invariants.OptionalIdInRange stateCount
              transitions.val[position]
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate_transitions
  step*
  cases result <;> simp_all

@[step]
theorem validateGotos_spec
    (gotos : Slice (Option GreatgrammaCore.ids.ParserStateId))
    (stateCount : U32) :
    GreatgrammaCore.validate.validate_gotos gotos stateCount
      ⦃ result => match result with
        | .Ok _ =>
          ∀ (position : Nat) (hposition : position < gotos.val.length),
            Greatgramma.Invariants.OptionalIdInRange stateCount
              gotos.val[position]
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate_gotos
  step*
  cases result <;> simp_all

@[step]
theorem validateProductions_spec
    (productions : Slice GreatgrammaCore.normalized.Production)
    (nonterminalCount : U32) :
    GreatgrammaCore.validate.validate_productions productions nonterminalCount
      ⦃ result => match result with
        | .Ok _ =>
          ∀ (position : Nat) (hposition : position < productions.val.length),
            productions.val[position].lhs.val < nonterminalCount.val
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate_productions
  step*
  cases result <;> simp_all

@[step]
theorem validateLexerTerminals_spec
    (terminals : Slice (Option GreatgrammaCore.ids.TerminalId))
    (start : GreatgrammaCore.ids.DfaStateId) (terminalCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId)
    (hlength : terminals.val.length ≤ U32.max) :
    GreatgrammaCore.validate.validate_lexer_terminals
      terminals start terminalCount eof
      ⦃ result => match result with
        | .Ok _ =>
          ∀ (position : Nat) (hposition : position < terminals.val.length),
            Greatgramma.Invariants.LexerTerminalWF terminalCount eof
                terminals.val[position] ∧
              (position = start.val → terminals.val[position] = none)
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate_lexer_terminals
  step*
  cases result <;> simp_all
  intro position hposition hstart
  exact (validation_error_post position hposition).2 hstart

@[step]
theorem validateActions_spec
    (actions : Slice GreatgrammaCore.normalized.Action)
    (stateCount terminalCount productionCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId)
    (heof : eof.val < terminalCount.val) :
    GreatgrammaCore.validate.validate_actions actions stateCount terminalCount
      productionCount eof
      ⦃ result => match result with
        | .Ok _ =>
          ∀ (position : Nat) (hposition : position < actions.val.length),
            Greatgramma.Invariants.ActionWF stateCount productionCount
              terminalCount eof position actions.val[position]
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate_actions
  step*
  all_goals cases result <;> simp_all

@[step]
theorem validate_spec
    (grammar : GreatgrammaCore.normalized.UnvalidatedGrammar)
    (limits : GreatgrammaCore.limits.ValidationLimits) :
    GreatgrammaCore.validate.validate grammar limits
      ⦃ result => match result with
        | .Ok validated =>
          Greatgramma.Invariants.ValidatedWF validated ∧
            Greatgramma.Invariants.ValidatedRepresents grammar validated
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.validate
  simp [GreatgrammaCore.ids.DfaStateId.get,
    GreatgrammaCore.ids.ParserStateId.get,
    GreatgrammaCore.ids.TerminalId.get]
  step*
  all_goals try simp_all [alloc.vec.Vec.deref,
    GreatgrammaCore.validate.BYTE_CLASS_TABLE_LEN]
  all_goals try scalar_tac
  case sourceRange =>
    intro terminal hterminal
    obtain ⟨position, hposition, rfl⟩ :=
      List.mem_iff_getElem.mp hterminal
    exact (r13_post position hposition).1
  simp [GreatgrammaCore.normalized.ValidatedLexer.from_unvalidated,
    GreatgrammaCore.normalized.ValidatedLalr.from_unvalidated,
    GreatgrammaCore.normalized.ValidatedGrammar.from_parts, WP.spec_ok]
  have ignoredRange :
      ∀ terminal ∈ grammar.lalr.ignored_terminals.val,
        terminal.val < grammar.lalr.dimensions.terminal_count.val := by
    intro terminal hterminal
    obtain ⟨position, hposition, rfl⟩ :=
      List.mem_iff_getElem.mp hterminal
    exact (r13_post position hposition).1
  have ignoredExcludes :
      grammar.lalr.eof_terminal ∉ grammar.lalr.ignored_terminals.val := by
    intro heof
    obtain ⟨position, hposition, heq⟩ :=
      List.mem_iff_getElem.mp heof
    have hneq := (r13_post position hposition).2
    apply hneq
    simp [heq]
  constructor
  · exact {
      tokenCount := r_post.tokenCount
      tokenTableNonempty := r_post.tokensNonempty
      tokenEntries := by
        intro token htoken
        obtain ⟨position, hposition, rfl⟩ :=
          List.mem_iff_getElem.mp htoken
        exact r_post.tokenEntries position hposition
      hasOrdinary := by
        obtain ⟨position, token, hlookup, hordinary⟩ := r_post.hasOrdinary
        exact ⟨token, List.mem_of_getElem? hlookup, hordinary⟩
      hasEos := by
        obtain ⟨position, token, hlookup, heos⟩ := r_post.hasEos
        exact ⟨token, List.mem_of_getElem? hlookup, heos⟩
      lexer := {
        byteClassLength := r1_post
        transitionLength := r3_post.trans r_post.lexerCells
        terminalLength := r5_post
        startInRange := cf10_post
        byteClassesInRange := by
          intro byteClass hbyteClass
          obtain ⟨position, hposition, rfl⟩ :=
            List.mem_iff_getElem.mp hbyteClass
          apply r14_post position
          simpa [r1_post] using hposition
        transitionsInRange := by
          intro destination hdestination
          obtain ⟨position, hposition, rfl⟩ :=
            List.mem_iff_getElem.mp hdestination
          apply r15_post position
          simpa [r3_post] using hposition
        terminalsInRange := by
          intro terminal hterminal
          obtain ⟨position, hposition, rfl⟩ :=
            List.mem_iff_getElem.mp hterminal
          exact (r16_post position (by simpa [r5_post] using hposition)).1
        startNonaccepting := by
          have hstart : grammar.lexer.start_state.val <
              grammar.lexer.terminals.val.length := by omega
          have hnone :=
            (r16_post grammar.lexer.start_state.val cf10_post).2 rfl
          simp [List.getElem?_eq_getElem hstart, hnone]
      }
      lalr := {
        actionLength := r7_post.trans r_post.parserActionCells
        gotoLength := r9_post.trans r_post.parserGotoCells
        productionLength := r_post.productionCount.symm
        ignoredLength := r20_post.length
        startInRange := cf11_post
        eofInRange := cf12_post
        actionsInRange := by
          intro position action hlookup
          obtain ⟨hposition, heq⟩ :=
            List.getElem?_eq_some_iff.mp hlookup
          have hvalid := r17_post position
            (by simpa [r7_post] using hposition)
          simpa [heq] using hvalid
        gotosInRange := by
          intro destination hdestination
          obtain ⟨position, hposition, rfl⟩ :=
            List.mem_iff_getElem.mp hdestination
          apply r18_post position
          simpa [r9_post] using hposition
        productionsInRange := by
          intro production hproduction
          obtain ⟨position, hposition, rfl⟩ :=
            List.mem_iff_getElem.mp hproduction
          exact r19_post position hposition
        ignoredBits := r20_post.bits
        eofNotIgnored :=
          r20_post.eofZero grammar.lalr.eof_terminal cf12_post ignoredExcludes
      }
    }
  · exact {
      tokenCount := r_post.tokenCount
      tokens := rfl
      lexerStateCount := rfl
      lexerClassCount := rfl
      byteClasses := rfl
      transitions := rfl
      lexerStart := rfl
      lexerTerminals := rfl
      dimensions := rfl
      parserStart := rfl
      parserEof := rfl
      actions := rfl
      gotos := rfl
      productionCount := r_post.productionCount
      productions := rfl
      ignoredLength := r20_post.length
      ignoredSourceInRange := ignoredRange
      ignoredSourceExcludesEof := ignoredExcludes
      ignoredMembership := r20_post.membership
    }

/-- The public generated Rust validation boundary: every successful value is
structurally well formed, represents exactly the caller's input tables, and
therefore induces the pure normalized structural invariant. -/
theorem validationBoundary
    (grammar : GreatgrammaCore.normalized.UnvalidatedGrammar)
    (limits : GreatgrammaCore.limits.ValidationLimits) :
    GreatgrammaCore.normalized.UnvalidatedGrammar.validate grammar limits
      ⦃ result => match result with
        | .Ok validated =>
          Greatgramma.Invariants.ValidatedWF validated ∧
            Greatgramma.Invariants.ValidatedRepresents grammar validated ∧
            Greatgramma.Spec.StructuralWF
              (Greatgramma.Invariants.viewUnvalidated grammar)
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.normalized.UnvalidatedGrammar.validate
  apply WP.spec_mono (validate_spec grammar limits)
  intro result hresult
  cases result with
  | Err _ => trivial
  | Ok validated =>
    exact ⟨hresult.1, hresult.2,
      validatedView_structuralWF grammar validated hresult.1 hresult.2⟩

/-- A structurally invalid raw grammar cannot cross the generated validation
boundary.  The diagnostic is intentionally abstract: U3 proves rejection and
absence of a validated artifact, not the wording of validation errors. -/
theorem validationRejectsStructurallyInvalid
    (grammar : GreatgrammaCore.normalized.UnvalidatedGrammar)
    (limits : GreatgrammaCore.limits.ValidationLimits)
    (invalid : ¬ Greatgramma.Spec.StructuralWF
      (Greatgramma.Invariants.viewUnvalidated grammar)) :
    GreatgrammaCore.normalized.UnvalidatedGrammar.validate grammar limits
      ⦃ result => match result with
        | .Ok _ => False
        | .Err _ => True ⦄ := by
  apply WP.spec_mono (validationBoundary grammar limits)
  intro result hresult
  cases result with
  | Err _ => trivial
  | Ok _ => exact invalid hresult.2.2

end Greatgramma.Refinement
