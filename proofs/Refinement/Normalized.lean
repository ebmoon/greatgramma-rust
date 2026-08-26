import GreatgrammaCore.Funs

open Aeneas Aeneas.Std Result

namespace Greatgramma.Refinement

/-!
Stable specifications for the normalized table constructors and the transparent
identifier wrappers.  These lemmas deliberately expose mathematical values,
not generated implementation names, in their postconditions.
-/

/-- Row-major indexing with the same checked `usize` arithmetic as Rust. -/
def checkedFlatIndex (row width column : U32) : Option Usize := do
  let base ← Usize.checked_mul (UScalar.cast .Usize row)
    (UScalar.cast .Usize width)
  base.checked_add (UScalar.cast .Usize column)

/-- Safe row-major lookup: arithmetic overflow and table bounds both yield `none`. -/
def checkedFlatLookup {T : Type} (table : List T)
    (row width column : U32) : Option T := do
  let index ← checkedFlatIndex row width column
  table[index.val]?

/-- Pure safe-table semantics of one DFA transition lookup. -/
def lexerTransitionLookup
    (lexer : GreatgrammaCore.normalized.ValidatedLexer)
    (state : GreatgrammaCore.ids.DfaStateId) (byte : U8) :
    Option (Option GreatgrammaCore.ids.DfaStateId) :=
  if state.val < lexer.state_count.val then do
    let byteClass ← lexer.byte_classes.val[byte.val]?
    checkedFlatLookup lexer.transitions.val state lexer.class_count byteClass
  else none

theorem dfaStateIdNew_spec (value : U32) :
    GreatgrammaCore.ids.DfaStateId.new value ⦃ id => id = value ⦄ := by
  simp [GreatgrammaCore.ids.DfaStateId.new]

theorem dfaStateIdGet_spec (id : GreatgrammaCore.ids.DfaStateId) :
    GreatgrammaCore.ids.DfaStateId.get id ⦃ value => value = id ⦄ := by
  simp [GreatgrammaCore.ids.DfaStateId.get]

theorem tokenIdNew_spec (value : U32) :
    GreatgrammaCore.ids.TokenId.new value ⦃ id => id = value ⦄ := by
  simp [GreatgrammaCore.ids.TokenId.new]

theorem tokenIdGet_spec (id : GreatgrammaCore.ids.TokenId) :
    GreatgrammaCore.ids.TokenId.get id ⦃ value => value = id ⦄ := by
  simp [GreatgrammaCore.ids.TokenId.get]

theorem terminalIdNew_spec (value : U32) :
    GreatgrammaCore.ids.TerminalId.new value ⦃ id => id = value ⦄ := by
  simp [GreatgrammaCore.ids.TerminalId.new]

theorem terminalIdGet_spec (id : GreatgrammaCore.ids.TerminalId) :
    GreatgrammaCore.ids.TerminalId.get id ⦃ value => value = id ⦄ := by
  simp [GreatgrammaCore.ids.TerminalId.get]

theorem parserStateIdNew_spec (value : U32) :
    GreatgrammaCore.ids.ParserStateId.new value ⦃ id => id = value ⦄ := by
  simp [GreatgrammaCore.ids.ParserStateId.new]

theorem parserStateIdGet_spec (id : GreatgrammaCore.ids.ParserStateId) :
    GreatgrammaCore.ids.ParserStateId.get id ⦃ value => value = id ⦄ := by
  simp [GreatgrammaCore.ids.ParserStateId.get]

theorem nonterminalIdNew_spec (value : U32) :
    GreatgrammaCore.ids.NonterminalId.new value ⦃ id => id = value ⦄ := by
  simp [GreatgrammaCore.ids.NonterminalId.new]

theorem nonterminalIdGet_spec (id : GreatgrammaCore.ids.NonterminalId) :
    GreatgrammaCore.ids.NonterminalId.get id ⦃ value => value = id ⦄ := by
  simp [GreatgrammaCore.ids.NonterminalId.get]

theorem productionIdNew_spec (value : U32) :
    GreatgrammaCore.ids.ProductionId.new value ⦃ id => id = value ⦄ := by
  simp [GreatgrammaCore.ids.ProductionId.new]

theorem productionIdGet_spec (id : GreatgrammaCore.ids.ProductionId) :
    GreatgrammaCore.ids.ProductionId.get id ⦃ value => value = id ⦄ := by
  simp [GreatgrammaCore.ids.ProductionId.get]

theorem sequenceIdNew_spec (value : U32) :
    GreatgrammaCore.ids.SequenceId.new value ⦃ id => id = value ⦄ := by
  simp [GreatgrammaCore.ids.SequenceId.new]

theorem sequenceIdGet_spec (id : GreatgrammaCore.ids.SequenceId) :
    GreatgrammaCore.ids.SequenceId.get id ⦃ value => value = id ⦄ := by
  simp [GreatgrammaCore.ids.SequenceId.get]

theorem lalrDimensionsNew_spec (states terminals nonterminals : U32) :
    GreatgrammaCore.normalized.LalrDimensions.new states terminals nonterminals
      ⦃ dimensions =>
        dimensions.state_count = states ∧
        dimensions.terminal_count = terminals ∧
        dimensions.nonterminal_count = nonterminals ⦄ := by
  simp [GreatgrammaCore.normalized.LalrDimensions.new]

theorem lalrDimensionsStateCount_spec
    (dimensions : GreatgrammaCore.normalized.LalrDimensions) :
    GreatgrammaCore.normalized.LalrDimensions.impl.state_count dimensions
      ⦃ count => count = dimensions.state_count ⦄ := by
  simp [GreatgrammaCore.normalized.LalrDimensions.impl.state_count]

theorem lalrDimensionsTerminalCount_spec
    (dimensions : GreatgrammaCore.normalized.LalrDimensions) :
    GreatgrammaCore.normalized.LalrDimensions.impl.terminal_count dimensions
      ⦃ count => count = dimensions.terminal_count ⦄ := by
  simp [GreatgrammaCore.normalized.LalrDimensions.impl.terminal_count]

theorem lalrDimensionsNonterminalCount_spec
    (dimensions : GreatgrammaCore.normalized.LalrDimensions) :
    GreatgrammaCore.normalized.LalrDimensions.impl.nonterminal_count dimensions
      ⦃ count => count = dimensions.nonterminal_count ⦄ := by
  simp [GreatgrammaCore.normalized.LalrDimensions.impl.nonterminal_count]

theorem productionNew_spec
    (lhs : GreatgrammaCore.ids.NonterminalId) (popLength : U32) :
    GreatgrammaCore.normalized.Production.new lhs popLength
      ⦃ production => production.lhs = lhs ∧ production.pop_len = popLength ⦄ := by
  simp [GreatgrammaCore.normalized.Production.new]

theorem productionLhs_spec (production : GreatgrammaCore.normalized.Production) :
    GreatgrammaCore.normalized.Production.impl.lhs production
      ⦃ lhs => lhs = production.lhs ⦄ := by
  simp [GreatgrammaCore.normalized.Production.impl.lhs]

theorem productionPopLength_spec (production : GreatgrammaCore.normalized.Production) :
    GreatgrammaCore.normalized.Production.impl.pop_len production
      ⦃ popLength => popLength = production.pop_len ⦄ := by
  simp [GreatgrammaCore.normalized.Production.impl.pop_len]

theorem lexerDfaNew_spec
    (stateCount classCount : U32)
    (byteClasses : alloc.vec.Vec U32)
    (transitions : alloc.vec.Vec (Option GreatgrammaCore.ids.DfaStateId))
    (startState : GreatgrammaCore.ids.DfaStateId)
    (terminals : alloc.vec.Vec (Option GreatgrammaCore.ids.TerminalId)) :
    GreatgrammaCore.normalized.LexerDfa.new stateCount classCount byteClasses
      transitions startState terminals
      ⦃ lexer =>
        lexer.state_count = stateCount ∧
        lexer.class_count = classCount ∧
        lexer.byte_classes = byteClasses ∧
        lexer.transitions = transitions ∧
        lexer.start_state = startState ∧
        lexer.terminals = terminals ⦄ := by
  simp [GreatgrammaCore.normalized.LexerDfa.new]

theorem lalrTableNew_spec
    (dimensions : GreatgrammaCore.normalized.LalrDimensions)
    (startState : GreatgrammaCore.ids.ParserStateId)
    (eofTerminal : GreatgrammaCore.ids.TerminalId)
    (actions : alloc.vec.Vec GreatgrammaCore.normalized.Action)
    (gotos : alloc.vec.Vec (Option GreatgrammaCore.ids.ParserStateId))
    (productions : alloc.vec.Vec GreatgrammaCore.normalized.Production) :
    GreatgrammaCore.normalized.LalrTable.new dimensions startState eofTerminal
      actions gotos productions
      ⦃ table =>
        table.dimensions = dimensions ∧
        table.start_state = startState ∧
        table.eof_terminal = eofTerminal ∧
        table.actions = actions ∧
        table.gotos = gotos ∧
        table.productions = productions ∧
        table.ignored_terminals.val = [] ⦄ := by
  simp [GreatgrammaCore.normalized.LalrTable.new, alloc.vec.Vec.new]

theorem lalrTableWithIgnoredTerminals_spec
    (table : GreatgrammaCore.normalized.LalrTable)
    (ignored : alloc.vec.Vec GreatgrammaCore.ids.TerminalId) :
    GreatgrammaCore.normalized.LalrTable.with_ignored_terminals table ignored
      ⦃ result =>
        result.dimensions = table.dimensions ∧
        result.start_state = table.start_state ∧
        result.eof_terminal = table.eof_terminal ∧
        result.actions = table.actions ∧
        result.gotos = table.gotos ∧
        result.productions = table.productions ∧
        result.ignored_terminals = ignored ⦄ := by
  simp [GreatgrammaCore.normalized.LalrTable.with_ignored_terminals]

theorem unvalidatedGrammarNew_spec
    (tokens : alloc.vec.Vec GreatgrammaCore.normalized.TokenEntry)
    (lexer : GreatgrammaCore.normalized.LexerDfa)
    (lalr : GreatgrammaCore.normalized.LalrTable) :
    GreatgrammaCore.normalized.UnvalidatedGrammar.new tokens lexer lalr
      ⦃ grammar =>
        grammar.tokens = tokens ∧ grammar.lexer = lexer ∧ grammar.lalr = lalr ⦄ := by
  simp [GreatgrammaCore.normalized.UnvalidatedGrammar.new]

theorem validatedGrammarTokenCount_spec
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar) :
    GreatgrammaCore.normalized.ValidatedGrammar.impl.token_count grammar
      ⦃ count => count = grammar.token_count ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedGrammar.impl.token_count]

theorem validatedGrammarTokens_spec
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar) :
    GreatgrammaCore.normalized.ValidatedGrammar.impl.tokens grammar
      ⦃ tokens => tokens.val = grammar.tokens.val ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedGrammar.impl.tokens, alloc.vec.Vec.deref]

theorem validatedGrammarLexer_spec
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar) :
    GreatgrammaCore.normalized.ValidatedGrammar.impl.lexer grammar
      ⦃ lexer => lexer = grammar.lexer ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedGrammar.impl.lexer]

theorem validatedGrammarLalr_spec
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar) :
    GreatgrammaCore.normalized.ValidatedGrammar.impl.lalr grammar
      ⦃ lalr => lalr = grammar.lalr ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedGrammar.impl.lalr]

theorem validatedLexerStateCount_spec
    (lexer : GreatgrammaCore.normalized.ValidatedLexer) :
    GreatgrammaCore.normalized.ValidatedLexer.impl.state_count lexer
      ⦃ count => count = lexer.state_count ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedLexer.impl.state_count]

theorem validatedLexerClassCount_spec
    (lexer : GreatgrammaCore.normalized.ValidatedLexer) :
    GreatgrammaCore.normalized.ValidatedLexer.impl.class_count lexer
      ⦃ count => count = lexer.class_count ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedLexer.impl.class_count]

theorem validatedLexerStartState_spec
    (lexer : GreatgrammaCore.normalized.ValidatedLexer) :
    GreatgrammaCore.normalized.ValidatedLexer.impl.start_state lexer
      ⦃ state => state = lexer.start_state ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedLexer.impl.start_state]

theorem validatedLalrStateCount_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr) :
    GreatgrammaCore.normalized.ValidatedLalr.state_count lalr
      ⦃ count => count = lalr.dimensions.state_count ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedLalr.state_count]

theorem validatedLalrTerminalCount_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr) :
    GreatgrammaCore.normalized.ValidatedLalr.terminal_count lalr
      ⦃ count => count = lalr.dimensions.terminal_count ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedLalr.terminal_count]

theorem validatedLalrNonterminalCount_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr) :
    GreatgrammaCore.normalized.ValidatedLalr.nonterminal_count lalr
      ⦃ count => count = lalr.dimensions.nonterminal_count ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedLalr.nonterminal_count]

theorem validatedLalrProductionCount_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr) :
    GreatgrammaCore.normalized.ValidatedLalr.impl.production_count lalr
      ⦃ count => count = lalr.production_count ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedLalr.impl.production_count]

theorem validatedLalrStartState_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr) :
    GreatgrammaCore.normalized.ValidatedLalr.impl.start_state lalr
      ⦃ state => state = lalr.start_state ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedLalr.impl.start_state]

theorem validatedLalrEofTerminal_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr) :
    GreatgrammaCore.normalized.ValidatedLalr.impl.eof_terminal lalr
      ⦃ terminal => terminal = lalr.eof_terminal ⦄ := by
  simp [GreatgrammaCore.normalized.ValidatedLalr.impl.eof_terminal]

theorem validatedGrammarToken_spec
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar)
    (token : GreatgrammaCore.ids.TokenId) :
    GreatgrammaCore.normalized.ValidatedGrammar.token grammar token
      ⦃ entry => entry = grammar.tokens.val[token.val]? ⦄ := by
  unfold GreatgrammaCore.normalized.ValidatedGrammar.token
    GreatgrammaCore.ids.TokenId.get
  have hconvert : token.val ≤ UScalar.max .Usize := by scalar_tac
  simp [Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
    core.num.tryFromUScalar, hconvert,
    core.result.Result.ok,
    core.option.Option.Insts.CoreOpsTry_traitTry.branch,
    core.slice.Slice.get, alloc.vec.Vec.deref]

@[step] theorem validatedLexerByteClass_spec
    (lexer : GreatgrammaCore.normalized.ValidatedLexer) (byte : U8)
    (hbyte : byte.val < lexer.byte_classes.val.length) :
    GreatgrammaCore.normalized.ValidatedLexer.byte_class lexer byte
      ⦃ byteClass => lexer.byte_classes.val[byte.val]? = some byteClass ⦄ := by
  unfold GreatgrammaCore.normalized.ValidatedLexer.byte_class
    alloc.vec.Vec.index
  simp only [lift, bind_tc_ok]
  change Slice.index_usize lexer.byte_classes
    (core.convert.num.FromUsizeU8.from byte)
      ⦃ byteClass => lexer.byte_classes.val[byte.val]? = some byteClass ⦄
  have hbound : (core.convert.num.FromUsizeU8.from byte).val <
      lexer.byte_classes.length := by
    simpa [core.convert.num.FromUsizeU8.from_val_eq] using hbyte
  apply WP.spec_mono (Slice.index_usize_spec _ _ hbound)
  intro byteClass hclass
  simp [hclass, core.convert.num.FromUsizeU8.from_val_eq]

theorem validatedLexerTerminal_spec
    (lexer : GreatgrammaCore.normalized.ValidatedLexer)
    (state : GreatgrammaCore.ids.DfaStateId) :
    GreatgrammaCore.normalized.ValidatedLexer.terminal lexer state
      ⦃ terminal => terminal =
        if state.val < lexer.state_count.val then
          lexer.terminals.val[state.val]?
        else none ⦄ := by
  unfold GreatgrammaCore.normalized.ValidatedLexer.terminal
    GreatgrammaCore.normalized.ValidatedLexer.state_index
    GreatgrammaCore.ids.DfaStateId.get
  by_cases hstate : state.val < lexer.state_count.val
  · have hconvert : state.val ≤ UScalar.max .Usize := by scalar_tac
    simp [Nat.not_le.mpr hstate,
      Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
      core.num.tryFromUScalar, hconvert,
      core.result.Result.ok,
      core.option.Option.Insts.CoreOpsTry_traitTry.branch,
      core.slice.Slice.get, alloc.vec.Vec.deref,
      core.option.OptionShared0T.copied]
  · simp [hstate, Nat.le_of_not_gt hstate,
      core.option.Option.Insts.CoreOpsTry_traitTry.branch,
      core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual,
      alloc.vec.Vec.deref]

theorem validatedLexerTransition_spec
    (lexer : GreatgrammaCore.normalized.ValidatedLexer)
    (state : GreatgrammaCore.ids.DfaStateId) (byte : U8)
    (hbyte : byte.val < lexer.byte_classes.val.length) :
    GreatgrammaCore.normalized.ValidatedLexer.transition lexer state byte
      ⦃ result => result = lexerTransitionLookup lexer state byte ⦄ := by
  unfold GreatgrammaCore.normalized.ValidatedLexer.transition
    GreatgrammaCore.normalized.ValidatedLexer.state_index
    GreatgrammaCore.ids.DfaStateId.get
  by_cases hstate : state.val < lexer.state_count.val
  · have hstateConvert : state.val ≤ UScalar.max .Usize := by scalar_tac
    simp [Nat.not_le.mpr hstate,
      Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
      core.num.tryFromUScalar, hstateConvert,
      core.result.Result.ok,
      core.option.Option.Insts.CoreOpsTry_traitTry.branch]
    apply WP.spec_bind (validatedLexerByteClass_spec lexer byte hbyte)
    intro byteClass hclass
    have hclassConvert : byteClass.val ≤ UScalar.max .Usize := by scalar_tac
    have hcountConvert : lexer.class_count.val ≤ UScalar.max .Usize := by
      scalar_tac
    simp [hclassConvert, hcountConvert]
    unfold lexerTransitionLookup checkedFlatLookup checkedFlatIndex
    simp [hstate, hclass]
    cases hproduct : Usize.checked_mul (UScalar.cast .Usize state)
        (UScalar.cast .Usize lexer.class_count) with
    | none => simp [lift,
        core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]
    | some product =>
      cases hindex : product.checked_add (UScalar.cast .Usize byteClass) with
      | none => simp [lift, hindex,
          core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]
      | some index => simp [lift, hindex, core.slice.Slice.get,
          alloc.vec.Vec.deref, core.option.OptionShared0T.copied]
  · simp [lexerTransitionLookup, hstate, Nat.le_of_not_gt hstate,
      core.option.Option.Insts.CoreOpsTry_traitTry.branch,
      core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]

theorem validatedLalrProduction_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr)
    (production : GreatgrammaCore.ids.ProductionId) :
    GreatgrammaCore.normalized.ValidatedLalr.production lalr production
      ⦃ result => result = lalr.productions.val[production.val]? ⦄ := by
  unfold GreatgrammaCore.normalized.ValidatedLalr.production
    GreatgrammaCore.ids.ProductionId.get
  have hconvert : production.val ≤ UScalar.max .Usize := by scalar_tac
  simp [Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
    core.num.tryFromUScalar, hconvert,
    core.result.Result.ok,
    core.option.Option.Insts.CoreOpsTry_traitTry.branch,
    core.slice.Slice.get, alloc.vec.Vec.deref,
    core.option.OptionShared0T.copied]

theorem validatedLalrIsIgnored_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr)
    (terminal : GreatgrammaCore.ids.TerminalId) :
    GreatgrammaCore.normalized.ValidatedLalr.is_ignored lalr terminal
      ⦃ result => result =
        if terminal.val < lalr.dimensions.terminal_count.val then
          lalr.ignored_terminals.val[terminal.val]?.map (fun value => value != 0#u8)
        else none ⦄ := by
  unfold GreatgrammaCore.normalized.ValidatedLalr.is_ignored
    GreatgrammaCore.normalized.ValidatedLalr.terminal_index
    GreatgrammaCore.normalized.ValidatedLalr.terminal_count
    GreatgrammaCore.normalized.checked_index
    GreatgrammaCore.ids.TerminalId.get
  by_cases hterminal : terminal.val < lalr.dimensions.terminal_count.val
  · have hconvert : terminal.val ≤ UScalar.max .Usize := by scalar_tac
    simp [hterminal, Nat.not_le.mpr hterminal,
      Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
      core.num.tryFromUScalar, hconvert,
      core.result.Result.ok,
      core.option.Option.Insts.CoreOpsTry_traitTry.branch,
      core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual,
      core.slice.Slice.get, alloc.vec.Vec.deref]
    split <;> simp_all
  · simp [hterminal, Nat.le_of_not_gt hterminal,
      core.option.Option.Insts.CoreOpsTry_traitTry.branch,
      core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual,
      alloc.vec.Vec.deref]

theorem validatedLalrAction_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr)
    (state : GreatgrammaCore.ids.ParserStateId)
    (terminal : GreatgrammaCore.ids.TerminalId) :
    GreatgrammaCore.normalized.ValidatedLalr.action lalr state terminal
      ⦃ result => result =
        if state.val < lalr.dimensions.state_count.val ∧
            terminal.val < lalr.dimensions.terminal_count.val then
          checkedFlatLookup lalr.actions.val state
            lalr.dimensions.terminal_count terminal
        else none ⦄ := by
  unfold GreatgrammaCore.normalized.ValidatedLalr.action
    GreatgrammaCore.normalized.ValidatedLalr.state_index
    GreatgrammaCore.normalized.ValidatedLalr.terminal_index
    GreatgrammaCore.normalized.ValidatedLalr.state_count
    GreatgrammaCore.normalized.ValidatedLalr.terminal_count
    GreatgrammaCore.normalized.checked_index
    GreatgrammaCore.ids.ParserStateId.get
    GreatgrammaCore.ids.TerminalId.get
  by_cases hstate : state.val < lalr.dimensions.state_count.val
  · by_cases hterminal : terminal.val < lalr.dimensions.terminal_count.val
    · have hstateConvert : state.val ≤ UScalar.max .Usize := by scalar_tac
      have hterminalConvert : terminal.val ≤ UScalar.max .Usize := by scalar_tac
      have hcountConvert : lalr.dimensions.terminal_count.val ≤
          UScalar.max .Usize := by scalar_tac
      simp [hstate, hterminal, Nat.not_le.mpr hstate,
        Nat.not_le.mpr hterminal,
        Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
        core.num.tryFromUScalar, hstateConvert, hterminalConvert, hcountConvert,
        core.result.Result.ok,
        core.option.Option.Insts.CoreOpsTry_traitTry.branch,
        core.slice.Slice.get, alloc.vec.Vec.deref,
        core.option.OptionShared0T.copied]
      unfold checkedFlatLookup checkedFlatIndex
      cases hproduct : Usize.checked_mul (UScalar.cast .Usize state)
          (UScalar.cast .Usize lalr.dimensions.terminal_count) with
      | none => simp [lift,
          core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]
      | some product =>
        cases hindex : product.checked_add (UScalar.cast .Usize terminal) with
        | none => simp [lift, hindex,
            core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]
        | some index => simp [lift, hindex]
    · have hstateConvert : state.val ≤ UScalar.max .Usize := by scalar_tac
      simp [hstate, hterminal, Nat.not_le.mpr hstate,
        Nat.le_of_not_gt hterminal,
        Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
        core.num.tryFromUScalar, hstateConvert,
        core.result.Result.ok,
        core.option.Option.Insts.CoreOpsTry_traitTry.branch,
        core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]
  · simp [hstate, Nat.le_of_not_gt hstate,
      core.option.Option.Insts.CoreOpsTry_traitTry.branch,
      core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]

theorem validatedLalrGoto_spec
    (lalr : GreatgrammaCore.normalized.ValidatedLalr)
    (state : GreatgrammaCore.ids.ParserStateId)
    (nonterminal : GreatgrammaCore.ids.NonterminalId) :
    GreatgrammaCore.normalized.ValidatedLalr.goto lalr state nonterminal
      ⦃ result => result =
        if state.val < lalr.dimensions.state_count.val ∧
            nonterminal.val < lalr.dimensions.nonterminal_count.val then
          checkedFlatLookup lalr.gotos.val state
            lalr.dimensions.nonterminal_count nonterminal
        else none ⦄ := by
  unfold GreatgrammaCore.normalized.ValidatedLalr.goto
    GreatgrammaCore.normalized.ValidatedLalr.state_index
    GreatgrammaCore.normalized.ValidatedLalr.nonterminal_index
    GreatgrammaCore.normalized.ValidatedLalr.state_count
    GreatgrammaCore.normalized.ValidatedLalr.nonterminal_count
    GreatgrammaCore.normalized.checked_index
    GreatgrammaCore.ids.ParserStateId.get
    GreatgrammaCore.ids.NonterminalId.get
  by_cases hstate : state.val < lalr.dimensions.state_count.val
  · by_cases hnonterminal :
        nonterminal.val < lalr.dimensions.nonterminal_count.val
    · have hstateConvert : state.val ≤ UScalar.max .Usize := by scalar_tac
      have hnonterminalConvert : nonterminal.val ≤ UScalar.max .Usize := by
        scalar_tac
      have hcountConvert : lalr.dimensions.nonterminal_count.val ≤
          UScalar.max .Usize := by scalar_tac
      simp [hstate, hnonterminal, Nat.not_le.mpr hstate,
        Nat.not_le.mpr hnonterminal,
        Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
        core.num.tryFromUScalar, hstateConvert, hnonterminalConvert,
        hcountConvert, core.result.Result.ok,
        core.option.Option.Insts.CoreOpsTry_traitTry.branch,
        core.slice.Slice.get, alloc.vec.Vec.deref,
        core.option.OptionShared0T.copied]
      unfold checkedFlatLookup checkedFlatIndex
      cases hproduct : Usize.checked_mul (UScalar.cast .Usize state)
          (UScalar.cast .Usize lalr.dimensions.nonterminal_count) with
      | none => simp [lift,
          core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]
      | some product =>
        cases hindex : product.checked_add (UScalar.cast .Usize nonterminal) with
        | none => simp [lift, hindex,
            core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]
        | some index => simp [lift, hindex]
    · have hstateConvert : state.val ≤ UScalar.max .Usize := by scalar_tac
      simp [hstate, hnonterminal, Nat.not_le.mpr hstate,
        Nat.le_of_not_gt hnonterminal,
        Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
        core.num.tryFromUScalar, hstateConvert,
        core.result.Result.ok,
        core.option.Option.Insts.CoreOpsTry_traitTry.branch,
        core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]
  · simp [hstate, Nat.le_of_not_gt hstate,
      core.option.Option.Insts.CoreOpsTry_traitTry.branch,
      core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual]

end Greatgramma.Refinement
