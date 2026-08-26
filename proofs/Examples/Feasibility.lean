import GreatgrammaCore

open Aeneas Aeneas.Std Result ControlFlow Error
open GreatgrammaCore

/-!
Executable feasibility proofs for the Aeneas translation boundary.

These theorems deliberately use `WP.spec`, rather than the divergence-tolerant
`WP.dspec`: each statement therefore establishes that the translated loop
returns normally (it neither reaches Aeneas `fail` nor `div`).  Later units can
strengthen the postconditions with functional-correctness properties without
having to revisit termination.
-/

namespace Greatgramma.Refinement

@[step]
theorem resultErrTotal {T E : Type} :
    (r : core.result.Result T E) → core.result.Result.err r ⦃ _ => True ⦄
  | .Ok _ => by simp [core.result.Result.err]
  | .Err _ => by simp [core.result.Result.err]

@[step]
theorem usizeToU8Total (index : Std.Usize) :
    U8.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from index
      ⦃ _ => True ⦄ := by
  unfold U8.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from
    core.num.tryFromUScalar
  split <;> simp

@[step]
theorem validateByteClassTotal
    (index : Std.Usize) (byteClass classCount : Std.U32) :
    validate.validate_byte_class index byteClass classCount ⦃ _ => True ⦄ := by
  by_cases hclass : classCount.val ≤ byteClass.val
  · by_cases hindex : index.val ≤ Std.U8.max
    · simp [validate.validate_byte_class, hclass, hindex,
        U8.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from,
        core.num.tryFromUScalar, core.result.Result.map_err,
        validate.validate_byte_class.closure.Insts.CoreOpsFunctionFnOnceTupleTryFromIntErrorValidationError.call_once,
        core.result.Result.Insts.CoreOpsTry.branch]
    · simp [validate.validate_byte_class, hclass, hindex,
        U8.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from,
        core.num.tryFromUScalar, core.result.Result.map_err,
        validate.validate_byte_class.closure.Insts.CoreOpsFunctionFnOnceTupleTryFromIntErrorValidationError.call_once,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  · simp [validate.validate_byte_class, hclass]

/-- Byte-class validation terminates and does not fail in Aeneas' outer
    computation monad.  Validation failures remain ordinary Rust `Result`
    values inside the successful return. -/
@[step]
theorem validateByteClassesLoopTotal
    (classes : Slice Std.U32) (classCount : Std.U32)
    (validationError : Option error.ValidationError) (index : Std.Usize) :
    validate.validate_byte_classes_loop classes classCount validationError index
      ⦃ _ => True ⦄ := by
  unfold validate.validate_byte_classes_loop
  simp
  split
  · step*
  · simp
termination_by classes.length - index.val
decreasing_by scalar_decr_tac

/-! ## Reverse-fact propagation -/

/-- Local, state-independent totality contract for the non-recursive body of
    the predecessor loop.  U5 strengthens and discharges this contract from
    the `seen`/fact-table invariants; U2 only needs it to isolate the recursive
    Aeneas equation from that later semantic proof. -/
def PropagatePredecessorContract
    (row : alloc.vec.Vec Std.Usize) (terminal : ids.TerminalId)
    (terminalCount : Std.Usize) : Prop :=
  ∀ (seen : Slice Std.U8) (facts : alloc.vec.Vec sequence.Fact)
    (index : Std.Usize), index.val < row.length →
    ∃ output,
      sequence.propagate_predecessor (alloc.vec.Vec.deref row) index seen facts
        terminal terminalCount = ok output

/-- Given the local body contract, the generated singleton predecessor loop is
    a total `WP.spec` computation.  The proof crosses a genuine recursive edge
    and uses the remaining row length as its well-founded measure. -/
@[step]
theorem propagateFactLoopTotal
    (seen : Slice Std.U8) (facts : alloc.vec.Vec sequence.Fact)
    (terminal : ids.TerminalId) (terminalCount : Std.Usize)
    (row : alloc.vec.Vec Std.Usize)
    (error : Option token_step.PreparationError) (index : Std.Usize)
    (bodyTotal : PropagatePredecessorContract row terminal terminalCount) :
    sequence.propagate_fact_loop seen facts terminal terminalCount row error index
      ⦃ _ => True ⦄ := by
  unfold sequence.propagate_fact_loop
  simp
  split
  · rename_i hrow
    have hindexMax : index.val < Std.Usize.max :=
      lt_of_lt_of_le hrow row.property
    split
    · obtain ⟨output, houtput⟩ := bodyTotal seen facts index (by scalar_tac)
      rcases output with ⟨r, seen', facts'⟩
      simp [houtput, core.result.Result.err]
      cases r with
      | Ok value =>
          simp
          step as ⟨index'⟩
          apply propagateFactLoopTotal seen' facts' terminal terminalCount row
            none index' bodyTotal
      | Err nextError =>
          simp
          step as ⟨index'⟩
          apply propagateFactLoopTotal seen' facts' terminal terminalCount row
            (some nextError) index' bodyTotal
    · simp
  · simp
termination_by Std.Usize.max - index.val
decreasing_by
  all_goals norm_num at *
  all_goals omega

/-! ## Ranked parser loop -/

/-- Local contracts needed to isolate the generated ranked-reduction loop.
    They describe only termination, ordinary Rust results, and work progress;
    they do not assume parser-language correctness. -/
structure RankedParserContracts
    (parserTable : normalized.ValidatedLalr) (terminal : ids.TerminalId)
    (underflowMode : lalr.UnderflowMode) (maximum : Std.Usize) : Prop where
  stackTop : ∀ stack : alloc.vec.Vec ids.ParserStateId,
    ∃ result, lalr.stack_top (alloc.vec.Vec.deref stack) = ok result
  charge : ∀ work : lalr.RunWork,
    ∃ result work',
      lalr.RunWork.charge work 1#usize = ok (result, work') ∧
      (result = core.result.Result.Ok true →
        work.maximum = some maximum → work.used.val ≤ maximum.val →
        work'.maximum = some maximum ∧
        work.used.val < work'.used.val ∧ work'.used.val ≤ maximum.val)
  action : ∀ state : ids.ParserStateId,
    ∃ result, normalized.ValidatedLalr.action parserTable state terminal =
      ok result
  push : ∀ (stack : alloc.vec.Vec ids.ParserStateId)
    (state : ids.ParserStateId),
    ∃ result stack', lalr.parser_push stack state = ok (result, stack')
  progress : ∀ (previous : Option lalr.PreviousReduction)
    (rank : Std.U32) (state : ids.ParserStateId),
    ∃ result,
      lalr.check_reduction_progress previous rank state terminal = ok result
  reduction : ∀ (stack : alloc.vec.Vec ids.ParserStateId)
    (production : ids.ProductionId) (work : lalr.RunWork),
    ∃ result stack' work',
      lalr.apply_reduction parserTable stack production underflowMode work =
        ok (result, stack', work') ∧
      (∀ popLength,
        result = core.result.Result.Ok
          (lalr.ReductionResult.Applied popLength) →
        work.maximum = some maximum → work.used.val ≤ maximum.val →
        work'.maximum = some maximum ∧
        work.used.val < work'.used.val ∧ work'.used.val ≤ maximum.val)

/-- The generated reduction loop is total under its local helper contracts and
    a finite work bound.  A successful applied reduction consumes work, so the
    remaining-work measure strictly decreases at the actual recursive call. -/
@[step]
theorem runTerminalLoopRankedTotal
    (parserTable : normalized.ValidatedLalr)
    (stack : alloc.vec.Vec ids.ParserStateId) (terminal : ids.TerminalId)
    (terminalIndex terminalCount : Std.Usize)
    (underflowMode : lalr.UnderflowMode) (work : lalr.RunWork)
    (previousReduction : Option lalr.PreviousReduction)
    (maximum : Std.Usize)
    (contracts : RankedParserContracts parserTable terminal underflowMode
      maximum)
    (hmaximum : work.maximum = some maximum)
    (hused : work.used.val ≤ maximum.val) :
    lalr.run_terminal_loop parserTable stack terminal terminalIndex
      terminalCount underflowMode work previousReduction ⦃ _ => True ⦄ := by
  unfold lalr.run_terminal_loop
  simp only
  obtain ⟨topResult, htop⟩ := contracts.stackTop stack
  rw [htop]
  cases topResult with
  | Err topError =>
      simp [core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  | Ok state =>
      obtain ⟨chargeResult, work', hcharge, hchargeProgress⟩ :=
        contracts.charge work
      rw [hcharge]
      cases chargeResult with
      | Err chargeError =>
          simp [core.result.Result.Insts.CoreOpsTry.branch,
            core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
      | Ok allowed =>
          cases allowed with
          | false =>
              simp [core.result.Result.Insts.CoreOpsTry.branch]
          | true =>
              have hwork' := hchargeProgress rfl hmaximum hused
              obtain ⟨hmaximum', husedLt, hused'⟩ := hwork'
              simp only [core.result.Result.Insts.CoreOpsTry.branch, bind_tc_ok]
              obtain ⟨actionResult, haction⟩ := contracts.action state
              rw [haction]
              cases actionResult with
              | none =>
                  simp [core.option.Option.ok_or,
                    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
              | some action =>
                  simp [core.option.Option.ok_or]
                  cases action with
                  | Error => simp
                  | Shift destination =>
                      simp only
                      obtain ⟨shiftCharge, shiftWork, hshiftCharge,
                        hshiftProgress⟩ := contracts.charge work'
                      rw [hshiftCharge]
                      cases shiftCharge with
                      | Err shiftChargeError =>
                          simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
                      | Ok shiftAllowed =>
                          cases shiftAllowed with
                          | false =>
                              simp
                          | true =>
                              simp only [bind_tc_ok]
                              obtain ⟨pushResult, stack', hpush⟩ :=
                                contracts.push stack destination
                              rw [hpush]
                              cases pushResult <;>
                                simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
                  | Reduce production rank =>
                      simp only
                      obtain ⟨progressResult, hprogress⟩ :=
                        contracts.progress previousReduction rank state
                      rw [hprogress]
                      cases progressResult with
                      | Err progressError =>
                          simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
                      | Ok progressValue =>
                          simp only [bind_tc_ok]
                          obtain ⟨reductionResult, stack', reductionWork,
                            hreduction, hreductionProgress⟩ :=
                            contracts.reduction stack production work'
                          rw [hreduction]
                          cases reductionResult with
                          | Err reductionError =>
                              simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
                          | Ok reduction =>
                              simp
                              cases reduction with
                              | Applied popLength =>
                                  have hreductionWork := hreductionProgress
                                    popLength rfl hmaximum' hused'
                                  obtain ⟨hmaximum'', husedLt', hused''⟩ :=
                                    hreductionWork
                                  apply runTerminalLoopRankedTotal parserTable
                                    stack' terminal terminalIndex terminalCount
                                    underflowMode reductionWork
                                    (some { rank, pop_len := popLength }) maximum
                                    contracts hmaximum'' hused''
                              | Dependent => simp
                              | WorkLimitExceeded => simp
                  | Accept =>
                      simp only
                      cases hnext : terminalIndex.checked_add 1#usize with
                      | none =>
                        simp [lift,
                          core.cmp.PartialEq.ne.trait_default,
                          core.cmp.PartialEq.ne.default,
                          core.option.Option.Insts.CoreCmpPartialEqOption.eq,
                          core.cmp.impls.PartialEqUsize.eq]
                      | some nextIndex =>
                        simp [lift,
                          core.cmp.PartialEq.ne.trait_default,
                          core.cmp.PartialEq.ne.default,
                          core.option.Option.Insts.CoreCmpPartialEqOption.eq,
                          core.cmp.impls.PartialEqUsize.eq]
                        split <;> simp
termination_by maximum.val - work.used.val
decreasing_by omega

/-! ## Transactional batch staging -/

/-- Totality contract for one non-committing batch row.  It intentionally says
    nothing about the eventual commit: the generated staging loop only builds
    scratch state and a result vector. -/
def StageBatchRowContract
    (prepared : engine.PreparedGrammar) (states : Slice engine.MatcherStatus)
    (tokens : Slice ids.TokenId) : Prop :=
  ∀ (staging : Slice engine.MatcherStatus)
    (results : alloc.vec.Vec engine.AdvanceResult) (row : Std.Usize),
    row.val < states.length →
    ∃ output,
      engine.stage_batch_row prepared states staging tokens results row =
        ok output

/-- The generated batch staging loop terminates without outer failure or
    divergence whenever its single-row body is total.  The proof follows the
    real row-by-row recursive edge; an ordinary `EngineError` stops the loop. -/
@[step]
theorem stageBatchLoopTotal
    (prepared : engine.PreparedGrammar) (states : Slice engine.MatcherStatus)
    (staging : Slice engine.MatcherStatus) (tokens : Slice ids.TokenId)
    (results : alloc.vec.Vec engine.AdvanceResult)
    (error : Option engine.EngineError) (row : Std.Usize)
    (bodyTotal : StageBatchRowContract prepared states tokens) :
    engine.stage_batch_loop prepared states staging tokens results error row
      ⦃ _ => True ⦄ := by
  unfold engine.stage_batch_loop
  simp
  split
  · rename_i hrow
    have hrowMax : row.val < Std.Usize.max :=
      lt_of_lt_of_le hrow states.property
    split
    · obtain ⟨output, houtput⟩ := bodyTotal staging results row hrow
      rcases output with ⟨r, staging', results'⟩
      simp [houtput, core.result.Result.err]
      cases r with
      | Ok value =>
          simp
          step as ⟨row'⟩
          apply stageBatchLoopTotal prepared states staging' tokens results'
            none row' bodyTotal
      | Err nextError =>
          simp
          step as ⟨row'⟩
          apply stageBatchLoopTotal prepared states staging' tokens results'
            (some nextError) row' bodyTotal
    · simp
  · simp
termination_by Std.Usize.max - row.val
decreasing_by
  all_goals norm_num at *
  all_goals omega

/-! ## Feasibility root -/

/-- Stable evidence bundle for the four representative U2 loop families.
    Helper-loop theorems above remain public so later refinement units can
    reuse them without unpacking this root. -/
structure TotalLoopEvidence : Prop where
  validation : ∀ (classes : Slice Std.U32) (classCount : Std.U32)
    (validationError : Option error.ValidationError) (index : Std.Usize),
    WP.spec
      (validate.validate_byte_classes_loop classes classCount validationError
        index) (fun _ => True)
  factPropagation : ∀ (seen : Slice Std.U8)
    (facts : alloc.vec.Vec sequence.Fact) (terminal : ids.TerminalId)
    (terminalCount : Std.Usize) (row : alloc.vec.Vec Std.Usize)
    (error : Option token_step.PreparationError) (index : Std.Usize),
    PropagatePredecessorContract row terminal terminalCount →
    WP.spec
      (sequence.propagate_fact_loop seen facts terminal terminalCount row error
        index) (fun _ => True)
  rankedParser : ∀ (parserTable : normalized.ValidatedLalr)
    (stack : alloc.vec.Vec ids.ParserStateId) (terminal : ids.TerminalId)
    (terminalIndex terminalCount : Std.Usize)
    (underflowMode : lalr.UnderflowMode) (work : lalr.RunWork)
    (previousReduction : Option lalr.PreviousReduction) (maximum : Std.Usize),
    RankedParserContracts parserTable terminal underflowMode maximum →
    work.maximum = some maximum →
    work.used.val ≤ maximum.val →
    WP.spec
      (lalr.run_terminal_loop parserTable stack terminal terminalIndex
        terminalCount underflowMode work previousReduction) (fun _ => True)
  transactionalBatch : ∀ (prepared : engine.PreparedGrammar)
    (states staging : Slice engine.MatcherStatus) (tokens : Slice ids.TokenId)
    (results : alloc.vec.Vec engine.AdvanceResult)
    (error : Option engine.EngineError) (row : Std.Usize),
    StageBatchRowContract prepared states tokens →
    WP.spec
      (engine.stage_batch_loop prepared states staging tokens results error row)
      (fun _ => True)

/-- U2's representative feasibility root.  Because every field is a
    `WP.spec`, the bundle records normal return rather than merely allowing
    divergence through `WP.dspec`. -/
theorem totalLoops : TotalLoopEvidence where
  validation := validateByteClassesLoopTotal
  factPropagation := propagateFactLoopTotal
  rankedParser := runTerminalLoopRankedTotal
  transactionalBatch := stageBatchLoopTotal

end Greatgramma.Refinement
