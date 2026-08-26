import Invariants.Validated

/-!
Pure refinement from the generated validated representation to the stable
structural specification.  This file does not depend on the executable
validation proof: it records exactly how the representation invariants imply
the generated-code-independent `StructuralWF` predicate.
-/

namespace Greatgramma.Refinement

open Greatgramma.Invariants Greatgramma.Spec

private theorem viewTerminalId_injective :
    Function.Injective viewTerminalId := by
  rintro ⟨left⟩ ⟨right⟩ equal
  have valueEq := congrArg Greatgramma.Spec.TerminalId.val equal
  change left.toNat = right.toNat at valueEq
  have bitsEq : left = right := BitVec.toNat_injective valueEq
  subst right
  rfl

theorem validatedView_tokenTableWF
    (raw : GreatgrammaCore.normalized.UnvalidatedGrammar)
    (validated : GreatgrammaCore.normalized.ValidatedGrammar)
    (wf : Greatgramma.Invariants.ValidatedWF validated)
    (rep : Greatgramma.Invariants.ValidatedRepresents raw validated) :
    TokenTableWF (viewUnvalidated raw).tokens := by
  constructor
  · cases rawTokens : raw.tokens.val with
    | nil =>
        exfalso
        apply wf.tokenTableNonempty
        rw [rep.tokens, rawTokens]
    | cons head tail => simp [viewUnvalidated, rawTokens]
  · obtain ⟨token, membership, ordinary⟩ := wf.hasOrdinary
    cases token with
    | Bytes bytes =>
        obtain ⟨index, lookup⟩ := List.mem_iff_getElem?.mp membership
        refine ⟨⟨index⟩, bytes.val.map viewByte, ?_⟩
        simp only [viewUnvalidated, token?, List.getElem?_map]
        rw [← rep.tokens, lookup]
        rfl
    | Eos => simp [IsOrdinary] at ordinary
  · obtain ⟨token, membership, eos⟩ := wf.hasEos
    cases token with
    | Bytes bytes => simp [IsEos] at eos
    | Eos =>
        obtain ⟨index, lookup⟩ := List.mem_iff_getElem?.mp membership
        refine ⟨⟨index⟩, ?_⟩
        simp only [viewUnvalidated, token?, List.getElem?_map]
        rw [← rep.tokens, lookup]
        rfl
  · intro token bytes entry
    rcases token with ⟨index⟩
    simp only [viewUnvalidated, token?, List.getElem?_map] at entry
    cases lookup : raw.tokens.val[index]? with
    | none => simp [lookup] at entry
    | some token =>
        have rawMembership : token ∈ raw.tokens.val :=
          List.mem_of_getElem? lookup
        have validatedMembership : token ∈ validated.tokens.val := by
          rwa [rep.tokens]
        have entryWf := wf.tokenEntries token validatedMembership
        cases token with
        | Bytes value =>
            simp [lookup, viewTokenEntry] at entry
            subst bytes
            cases valueBytes : value.val with
            | nil => exact False.elim (entryWf valueBytes)
            | cons head tail => simp
        | Eos => simp [lookup, viewTokenEntry] at entry

theorem validatedView_lexerWF
    (raw : GreatgrammaCore.normalized.UnvalidatedGrammar)
    (validated : GreatgrammaCore.normalized.ValidatedGrammar)
    (wf : Greatgramma.Invariants.ValidatedWF validated)
    (rep : Greatgramma.Invariants.ValidatedRepresents raw validated) :
    LexerWF (viewUnvalidated raw).lexer
      (viewUnvalidated raw).lalr.dimensions.terminalCount
      (viewUnvalidated raw).lalr.eofTerminal := by
  constructor
  · change (raw.lexer.byte_classes.val.map (fun value => value.val)).length = 256
    simp only [List.length_map]
    rw [← rep.byteClasses]
    exact wf.lexer.byteClassLength
  · change FlatShape
      (raw.lexer.transitions.val.map (Option.map viewDfaStateId))
      raw.lexer.state_count.val raw.lexer.class_count.val
    simp only [FlatShape, List.length_map]
    rw [← rep.transitions, ← rep.lexerStateCount, ← rep.lexerClassCount]
    exact wf.lexer.transitionLength
  · change (raw.lexer.terminals.val.map
      (Option.map viewTerminalId)).length = raw.lexer.state_count.val
    simp only [List.length_map]
    rw [← rep.lexerTerminals, ← rep.lexerStateCount]
    exact wf.lexer.terminalLength
  · change raw.lexer.start_state.val < raw.lexer.state_count.val
    rw [← rep.lexerStart, ← rep.lexerStateCount]
    exact wf.lexer.startInRange
  · intro classId membership
    change classId ∈ raw.lexer.byte_classes.val.map (fun value => value.val) at membership
    obtain ⟨rawClass, rawMembership, rfl⟩ := List.mem_map.mp membership
    have range := wf.lexer.byteClassesInRange rawClass (by
      simpa [rep.byteClasses] using rawMembership)
    simpa [viewUnvalidated, viewLexer, rep.lexerClassCount] using range
  · intro cell membership destination cellEq
    change cell ∈ raw.lexer.transitions.val.map
      (Option.map viewDfaStateId) at membership
    obtain ⟨rawCell, rawMembership, rfl⟩ := List.mem_map.mp membership
    cases rawCell with
    | none => simp at cellEq
    | some rawDestination =>
        simp only [Option.map_some, Option.some.injEq] at cellEq
        subst destination
        have range := wf.lexer.transitionsInRange (some rawDestination) (by
          simpa [rep.transitions] using rawMembership)
        simpa [viewUnvalidated, viewLexer, OptionalIdInRange, viewDfaStateId,
          rep.lexerStateCount] using range
  · intro cell membership terminal cellEq
    change cell ∈ raw.lexer.terminals.val.map
      (Option.map viewTerminalId) at membership
    obtain ⟨rawCell, rawMembership, rfl⟩ := List.mem_map.mp membership
    cases rawCell with
    | none => simp at cellEq
    | some rawTerminal =>
        simp only [Option.map_some, Option.some.injEq] at cellEq
        subst terminal
        have range := wf.lexer.terminalsInRange (some rawTerminal) (by
          simpa [rep.lexerTerminals] using rawMembership)
        simpa [viewUnvalidated, viewLalr, viewLalrDimensions, LexerTerminalWF,
          viewTerminalId, rep.dimensions] using range.1
  · have startRange : raw.lexer.start_state.val < raw.lexer.state_count.val := by
      simpa [rep.lexerStart, rep.lexerStateCount] using wf.lexer.startInRange
    have startLookup :
        raw.lexer.terminals.val[raw.lexer.start_state.val]? = some none := by
      simpa [rep.lexerTerminals, rep.lexerStart] using wf.lexer.startNonaccepting
    simp [viewUnvalidated, viewLexer, LexerDfa.terminal?, viewDfaStateId,
      startRange, startLookup]
  · change ∀ cell ∈ raw.lexer.terminals.val.map
      (Option.map viewTerminalId),
        cell ≠ some (viewTerminalId raw.lalr.eof_terminal)
    intro cell membership
    obtain ⟨rawCell, rawMembership, rfl⟩ := List.mem_map.mp membership
    cases rawCell with
    | none => simp
    | some rawTerminal =>
        have valid := wf.lexer.terminalsInRange (some rawTerminal) (by
          simpa [rep.lexerTerminals] using rawMembership)
        intro equalEof
        simp only [Option.map_some, Option.some.injEq] at equalEof
        apply valid.2
        rw [rep.parserEof]
        exact viewTerminalId_injective equalEof

theorem validatedView_lalrWF
    (raw : GreatgrammaCore.normalized.UnvalidatedGrammar)
    (validated : GreatgrammaCore.normalized.ValidatedGrammar)
    (wf : Greatgramma.Invariants.ValidatedWF validated)
    (rep : Greatgramma.Invariants.ValidatedRepresents raw validated) :
    LalrWF (viewUnvalidated raw).lalr := by
  constructor
  · change FlatShape (raw.lalr.actions.val.map viewAction)
      raw.lalr.dimensions.state_count.val
      raw.lalr.dimensions.terminal_count.val
    simp only [FlatShape, List.length_map]
    rw [← rep.actions, ← rep.dimensions]
    exact wf.lalr.actionLength
  · change FlatShape
      (raw.lalr.gotos.val.map (Option.map viewParserStateId))
      raw.lalr.dimensions.state_count.val
      raw.lalr.dimensions.nonterminal_count.val
    simp only [FlatShape, List.length_map]
    rw [← rep.gotos, ← rep.dimensions]
    exact wf.lalr.gotoLength
  · change raw.lalr.start_state.val < raw.lalr.dimensions.state_count.val
    rw [← rep.parserStart, ← rep.dimensions]
    exact wf.lalr.startInRange
  · change raw.lalr.eof_terminal.val < raw.lalr.dimensions.terminal_count.val
    rw [← rep.parserEof, ← rep.dimensions]
    exact wf.lalr.eofInRange
  · intro action membership destination actionEq
    change action ∈ raw.lalr.actions.val.map viewAction at membership
    obtain ⟨rawAction, rawMembership, rfl⟩ := List.mem_map.mp membership
    cases rawAction with
    | Error => simp [viewAction] at actionEq
    | Shift rawDestination =>
        simp only [viewAction, LalrAction.shift.injEq] at actionEq
        subst destination
        obtain ⟨index, lookup⟩ := List.mem_iff_getElem?.mp rawMembership
        have range := wf.lalr.actionsInRange index
          (.Shift rawDestination) (by simpa [rep.actions] using lookup)
        simpa [viewUnvalidated, viewLalr, viewLalrDimensions, viewParserStateId,
          ActionWF, rep.dimensions] using range
    | Reduce production rank => simp [viewAction] at actionEq
    | Accept => simp [viewAction] at actionEq
  · intro action membership production rank actionEq
    change action ∈ raw.lalr.actions.val.map viewAction at membership
    obtain ⟨rawAction, rawMembership, rfl⟩ := List.mem_map.mp membership
    cases rawAction with
    | Error => simp [viewAction] at actionEq
    | Shift destination => simp [viewAction] at actionEq
    | Reduce rawProduction rawRank =>
        simp only [viewAction, LalrAction.reduce.injEq] at actionEq
        obtain ⟨productionEq, rankEq⟩ := actionEq
        subst production
        subst rank
        obtain ⟨index, lookup⟩ := List.mem_iff_getElem?.mp rawMembership
        have range := wf.lalr.actionsInRange index
          (.Reduce rawProduction rawRank) (by simpa [rep.actions] using lookup)
        simpa [viewUnvalidated, viewLalr, viewProductionId, viewProduction,
          List.length_map, ActionWF, rep.productionCount] using range
    | Accept => simp [viewAction] at actionEq
  · rintro ⟨state⟩ ⟨terminal⟩ accepts
    change checkedFlatLookup (raw.lalr.actions.val.map viewAction)
      raw.lalr.dimensions.state_count.val
      raw.lalr.dimensions.terminal_count.val state terminal =
        some .accept at accepts
    unfold checkedFlatLookup at accepts
    split at accepts
    next bounds =>
      obtain ⟨stateRange, terminalRange⟩ := bounds
      unfold flatLookup flatIndex at accepts
      simp only [List.getElem?_map] at accepts
      cases lookup : raw.lalr.actions.val[
          state * raw.lalr.dimensions.terminal_count.val + terminal]? with
      | none => simp [lookup] at accepts
      | some rawAction =>
          cases rawAction with
          | Error => simp [lookup, viewAction] at accepts
          | Shift destination => simp [lookup, viewAction] at accepts
          | Reduce production rank => simp [lookup, viewAction] at accepts
          | Accept =>
              have validatedLookup : validated.lalr.actions.val[
                  state * raw.lalr.dimensions.terminal_count.val + terminal]? =
                    some .Accept := by
                simpa [rep.actions] using lookup
              have range := wf.lalr.actionsInRange
                (state * raw.lalr.dimensions.terminal_count.val + terminal)
                .Accept validatedLookup
              have rangeRaw :
                  (state * raw.lalr.dimensions.terminal_count.val + terminal) %
                      raw.lalr.dimensions.terminal_count.val =
                    raw.lalr.eof_terminal.val := by
                have rangePair :
                    0 < raw.lalr.dimensions.terminal_count.val ∧
                    (state * raw.lalr.dimensions.terminal_count.val + terminal) %
                        raw.lalr.dimensions.terminal_count.val =
                      raw.lalr.eof_terminal.val := by
                  simpa [ActionWF, rep.dimensions, rep.parserEof] using range
                exact rangePair.2
              have modulus :
                  (state * raw.lalr.dimensions.terminal_count.val + terminal) %
                      raw.lalr.dimensions.terminal_count.val = terminal := by
                simp [Nat.add_mod,
                  Nat.mod_eq_of_lt terminalRange]
              have terminalEq : terminal = raw.lalr.eof_terminal.val :=
                modulus.symm.trans rangeRaw
              cases hEof : raw.lalr.eof_terminal with
              | mk eofBits =>
                  simp [viewUnvalidated, viewLalr, viewTerminalId, hEof]
                    at terminalEq ⊢
                  exact terminalEq
    next outOfBounds => simp at accepts
  · intro cell membership destination cellEq
    change cell ∈ raw.lalr.gotos.val.map
      (Option.map viewParserStateId) at membership
    obtain ⟨rawCell, rawMembership, rfl⟩ := List.mem_map.mp membership
    cases rawCell with
    | none => simp at cellEq
    | some rawDestination =>
        simp only [Option.map_some, Option.some.injEq] at cellEq
        subst destination
        have range := wf.lalr.gotosInRange (some rawDestination) (by
          simpa [rep.gotos] using rawMembership)
        simpa [viewUnvalidated, viewLalr, viewLalrDimensions,
          OptionalIdInRange, viewParserStateId, rep.dimensions] using range
  · intro production membership
    change production ∈ raw.lalr.productions.val.map viewProduction at membership
    obtain ⟨rawProduction, rawMembership, rfl⟩ := List.mem_map.mp membership
    have range := wf.lalr.productionsInRange rawProduction (by
      simpa [rep.productions] using rawMembership)
    simpa [viewUnvalidated, viewLalr, viewLalrDimensions, viewProduction,
      viewNonterminalId, rep.dimensions] using range
  · intro terminal membership
    change terminal ∈ raw.lalr.ignored_terminals.val.map
      viewTerminalId at membership
    obtain ⟨rawTerminal, rawMembership, rfl⟩ := List.mem_map.mp membership
    have range := rep.ignoredSourceInRange rawTerminal rawMembership
    simpa [viewUnvalidated, viewLalr, viewLalrDimensions, viewTerminalId]
      using range
  · change viewTerminalId raw.lalr.eof_terminal ∉
      raw.lalr.ignored_terminals.val.map viewTerminalId
    intro membership
    obtain ⟨rawTerminal, rawMembership, equalEof⟩ :=
      List.mem_map.mp membership
    apply rep.ignoredSourceExcludesEof
    have : rawTerminal = raw.lalr.eof_terminal :=
      viewTerminalId_injective equalEof
    rwa [← this]

/-- The Rust-side validated representation implies the complete stable
structural specification for the exact caller-owned normalized tables. -/
theorem validatedView_structuralWF
    (raw : GreatgrammaCore.normalized.UnvalidatedGrammar)
    (validated : GreatgrammaCore.normalized.ValidatedGrammar)
    (wf : Greatgramma.Invariants.ValidatedWF validated)
    (rep : Greatgramma.Invariants.ValidatedRepresents raw validated) :
    StructuralWF (viewUnvalidated raw) :=
  ⟨validatedView_tokenTableWF raw validated wf rep,
    validatedView_lexerWF raw validated wf rep,
    validatedView_lalrWF raw validated wf rep⟩

end Greatgramma.Refinement
