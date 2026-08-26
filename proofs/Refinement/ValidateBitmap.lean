import GreatgrammaCore.Funs
import Invariants.Validated

open Aeneas Aeneas.Std Result

namespace Greatgramma.Refinement

attribute [local instance] Classical.propDecidable

/-!
Semantic specification for validation's ignored-terminal bitmap builder.
The generated builder starts with a zero-filled vector and overwrites exactly
the caller-supplied terminal positions with `1`.
-/

structure IgnoredBitmapRep
    (source : Slice GreatgrammaCore.ids.TerminalId)
    (terminalCount : U32) (bitmap : alloc.vec.Vec U8) : Prop where
  length : bitmap.val.length = terminalCount.val
  bits : ∀ bit ∈ bitmap.val, bit.val = 0 ∨ bit.val = 1
  membership : ∀ index, index < terminalCount.val →
    ((∃ bit, bitmap.val[index]? = some bit ∧ bit ≠ 0#u8) ↔
      ∃ terminal ∈ source.val, terminal.val = index)

private def SourceSeen
    (source : Slice GreatgrammaCore.ids.TerminalId)
    (processed target : Nat) : Prop :=
  ∃ position terminal,
    position < processed ∧ source.val[position]? = some terminal ∧
      terminal.val = target

private structure IgnoredBitmapPrefix
    (source : Slice GreatgrammaCore.ids.TerminalId)
    (terminalCount : U32) (processed : Nat)
    (bitmap : alloc.vec.Vec U8) : Prop where
  length : bitmap.val.length = terminalCount.val
  lookup : ∀ target, target < terminalCount.val →
    bitmap.val[target]? =
      some (if SourceSeen source processed target then 1#u8 else 0#u8)

private theorem List.set_opt_some (values : List α) (index : Nat) (value : α) :
    values.set_opt index (some value) = values.set index value := by
  induction values generalizing index with
  | nil => simp [List.set_opt]
  | cons head tail ih =>
      cases index with
      | zero => simp [List.set_opt]
      | succ index => simp [List.set_opt, ih]

private theorem sourceSeen_succ
    (source : Slice GreatgrammaCore.ids.TerminalId)
    (position target : Nat) (within : position < source.val.length) :
    SourceSeen source (position + 1) target ↔
      SourceSeen source position target ∨ source.val[position].val = target := by
  constructor
  · rintro ⟨seenAt, terminal, before, lookup, value⟩
    by_cases earlier : seenAt < position
    · exact Or.inl ⟨seenAt, terminal, earlier, lookup, value⟩
    · have same : seenAt = position := by omega
      subst seenAt
      have terminalEq : source.val[position] = terminal := by
        simpa [List.getElem?_eq_getElem within] using lookup
      exact Or.inr ((congrArg (fun item => item.val) terminalEq).trans value)
  · rintro (earlier | current)
    · obtain ⟨seenAt, terminal, before, lookup, value⟩ := earlier
      exact ⟨seenAt, terminal, by omega, lookup, value⟩
    · exact ⟨position, source.val[position], by omega,
        by simp [List.getElem?_eq_getElem within], current⟩

private theorem sourceSeen_all_iff
    (source : Slice GreatgrammaCore.ids.TerminalId) (target : Nat) :
    SourceSeen source source.val.length target ↔
      ∃ terminal ∈ source.val, terminal.val = target := by
  constructor
  · rintro ⟨position, terminal, within, lookup, value⟩
    exact ⟨terminal, List.mem_of_getElem? lookup, value⟩
  · rintro ⟨terminal, membership, value⟩
    obtain ⟨position, within, entry⟩ := List.mem_iff_getElem.mp membership
    exact ⟨position, terminal, within,
      by simpa [List.getElem?_eq_getElem within] using entry, value⟩

private theorem ignoredBitmapPrefix_zero
    (source : Slice GreatgrammaCore.ids.TerminalId) (terminalCount : U32) :
    IgnoredBitmapPrefix source terminalCount 0
      ⟨List.replicate terminalCount.val 0#u8, by scalar_tac⟩ := by
  constructor
  · simp
  · intro target inRange
    simp [SourceSeen, inRange]

private theorem ignoredBitmapPrefix_mark
    (source : Slice GreatgrammaCore.ids.TerminalId) (terminalCount : U32)
    (position : Nat) (within : position < source.val.length)
    (bitmap : alloc.vec.Vec U8)
    (hprefix : IgnoredBitmapPrefix source terminalCount position bitmap) :
    IgnoredBitmapPrefix source terminalCount (position + 1)
      ⟨bitmap.val.set source.val[position].val 1#u8, by
        simpa [List.length_set] using bitmap.property⟩ := by
  constructor
  · simpa using hprefix.length
  · intro target targetRange
    have targetWithin : target < bitmap.val.length := by
      simpa [hprefix.length] using targetRange
    rw [List.getElem?_set_of_lt (1#u8) bitmap.val targetWithin]
    rw [hprefix.lookup target targetRange]
    rw [sourceSeen_succ source position target within]
    by_cases same : source.val[position].val = target <;>
      by_cases seen : SourceSeen source position target <;> simp_all

private theorem ignoredBitmapPrefix_complete
    (source : Slice GreatgrammaCore.ids.TerminalId) (terminalCount : U32)
    (bitmap : alloc.vec.Vec U8)
    (hprefix : IgnoredBitmapPrefix source terminalCount source.val.length
      bitmap) :
    IgnoredBitmapRep source terminalCount bitmap := by
  constructor
  · exact hprefix.length
  · intro bit membership
    obtain ⟨index, within, entry⟩ := List.mem_iff_getElem.mp membership
    have targetRange : index < terminalCount.val := by
      rw [← hprefix.length]
      exact within
    have lookup := hprefix.lookup index targetRange
    rw [List.getElem?_eq_getElem within, entry] at lookup
    split at lookup <;> simp_all
  · intro index inRange
    rw [hprefix.lookup index inRange]
    rw [sourceSeen_all_iff]
    by_cases seen : ∃ terminal ∈ source.val, terminal.val = index <;>
      simp [seen]

theorem IgnoredBitmapRep.eofZero
    {source : Slice GreatgrammaCore.ids.TerminalId}
    {terminalCount : U32} {bitmap : alloc.vec.Vec U8}
    (rep : IgnoredBitmapRep source terminalCount bitmap)
    (eof : GreatgrammaCore.ids.TerminalId)
    (eofRange : eof.val < terminalCount.val)
    (excluded : eof ∉ source.val) :
    bitmap.val[eof.val]? = some 0#u8 := by
  have bitmapRange : eof.val < bitmap.val.length := by
    simpa [rep.length] using eofRange
  let bit := bitmap.val[eof.val]
  have lookup : bitmap.val[eof.val]? = some bit := by
    simp [bit, List.getElem?_eq_getElem bitmapRange]
  have membership : bit ∈ bitmap.val :=
    List.mem_of_getElem? lookup
  rcases rep.bits bit membership with zero | one
  · have bitZero : bit = 0#u8 := by scalar_tac
    simpa [bitZero] using lookup
  · have bitNonzero : bit ≠ 0#u8 := by scalar_tac
    obtain ⟨terminal, terminalMem, terminalValue⟩ :=
      (rep.membership eof.val eofRange).mp ⟨bit, lookup, bitNonzero⟩
    have terminalEq : terminal = eof := by scalar_tac
    exact False.elim (excluded (terminalEq ▸ terminalMem))

@[step]
theorem countAsUsize_ok
    (count : U32) (calculation : GreatgrammaCore.error.ArithmeticKind) :
    GreatgrammaCore.validate.count_as_usize count calculation
      ⦃ result => ∃ index : Usize,
        result = .Ok index ∧ index.val = count.val ⦄ := by
  have hconvert : count.val ≤ UScalar.max .Usize := by scalar_tac
  simp [GreatgrammaCore.validate.count_as_usize,
    Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
    core.num.tryFromUScalar, hconvert, core.result.Result.map_err]

@[step]
theorem markIgnoredTerminal_spec
    (table : Slice U8) (terminal : GreatgrammaCore.ids.TerminalId)
    (inRange : terminal.val < table.val.length) :
    GreatgrammaCore.validate.mark_ignored_terminal table terminal
      ⦃ output => output.1 = .Ok () ∧
        output.2.val = table.val.set terminal.val 1#u8 ⦄ := by
  have hconvert : terminal.val ≤ UScalar.max .Usize := by scalar_tac
  simp [GreatgrammaCore.validate.mark_ignored_terminal,
    GreatgrammaCore.ids.TerminalId.get,
    Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
    core.num.tryFromUScalar, hconvert,
    core.slice.Slice.get_mut, core.slice.index.Usize.get_mut,
    List.getElem?_eq_getElem inRange, List.set_opt_some]

@[step]
theorem buildIgnoredTerminalTableLoop_spec
    (source : Slice GreatgrammaCore.ids.TerminalId)
    (terminalCount : U32) (table : alloc.vec.Vec U8) (index : Usize)
    (indexBound : index.val ≤ source.val.length)
    (sourceRange : ∀ terminal ∈ source.val,
      terminal.val < terminalCount.val)
    (hprefix : IgnoredBitmapPrefix source terminalCount index.val table) :
    GreatgrammaCore.validate.build_ignored_terminal_table_loop
      source table none index
      ⦃ output => output.2 = none ∧
        IgnoredBitmapPrefix source terminalCount source.val.length output.1 ⦄ := by
  unfold GreatgrammaCore.validate.build_ignored_terminal_table_loop
  simp
  split
  · rename_i within
    have indexMax : index.val < Usize.max :=
      lt_of_lt_of_le within source.property
    have currentMem : source.val[index.val] ∈ source.val :=
      List.getElem_mem (l := source.val) (h := within)
    have currentRange := sourceRange source.val[index.val] currentMem
    have tableRange : source.val[index.val].val < table.val.length := by
      simpa [hprefix.length] using currentRange
    simp [lift, alloc.vec.Vec.deref_mut]
    step
    step
    rw [r_post1]
    simp [core.result.Result.err]
    step as ⟨indexNext⟩
    have indexNextVal : indexNext.val = index.val + 1 := by scalar_tac
    apply buildIgnoredTerminalTableLoop_spec source terminalCount _ indexNext
      (by scalar_tac) sourceRange
    have marked := ignoredBitmapPrefix_mark source terminalCount index.val
      within table hprefix
    rw [ti_post] at r_post2
    constructor
    · rw [r_post2]
      exact marked.length
    · intro target targetRange
      rw [r_post2]
      simpa [indexNextVal] using marked.lookup target targetRange
  · have indexAtEnd : index.val = source.val.length := by omega
    simpa [indexAtEnd] using hprefix
termination_by source.val.length - index.val
decreasing_by simp_all; omega

@[step]
theorem buildIgnoredTerminalTable_spec
    (source : Slice GreatgrammaCore.ids.TerminalId) (terminalCount : U32)
    (sourceRange : ∀ terminal ∈ source.val,
      terminal.val < terminalCount.val) :
    GreatgrammaCore.validate.build_ignored_terminal_table source terminalCount
      ⦃ result => match result with
        | .Ok bitmap => IgnoredBitmapRep source terminalCount bitmap
        | .Err _ => True ⦄ := by
  unfold GreatgrammaCore.validate.build_ignored_terminal_table
  step*
  simp_all [core.result.Result.Insts.CoreOpsTry.branch]
  by_cases canReserve : r.val < Usize.max
  · have canFit : r.val ≤ Usize.max := Nat.le_of_lt canReserve
    simp [alloc.vec.Vec.try_reserve_exact, alloc.vec.logical_reserve,
      alloc.vec.Vec.new, canReserve, canFit, core.result.Result.map_err]
    step
    have table1Val : table1.val =
        List.replicate terminalCount.val 0#u8 := by
      simpa [r_post2, List.resize] using table1_post
    have initial : IgnoredBitmapPrefix source terminalCount 0 table1 := by
      have zero := ignoredBitmapPrefix_zero source terminalCount
      constructor
      · rw [table1Val]
        exact zero.length
      · intro target targetRange
        rw [table1Val]
        exact zero.lookup target targetRange
    step
    simp [table2_post1]
    exact ignoredBitmapPrefix_complete source terminalCount table2 table2_post2
  · simp [alloc.vec.Vec.try_reserve_exact, alloc.vec.logical_reserve,
      alloc.vec.Vec.new, canReserve, core.result.Result.map_err,
      GreatgrammaCore.validate.build_ignored_terminal_table.closure.Insts.CoreOpsFunctionFnOnceTupleTryReserveErrorValidationError.call_once,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

end Greatgramma.Refinement
