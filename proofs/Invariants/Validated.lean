import GreatgrammaCore.Types
import Spec.FiniteHeadChecker

open Aeneas Aeneas.Std

namespace Greatgramma.Invariants

/-!
Structural facts established by Rust's normalized validation boundary.  These
predicates intentionally omit lexer-language correctness, parser reachability,
stack safety, and reduction-rank semantics: validation does not check them.
-/

def viewByte (byte : U8) : Greatgramma.Spec.Byte :=
  ⟨byte.val, by scalar_tac⟩

def viewDfaStateId (id : GreatgrammaCore.ids.DfaStateId) :
    Greatgramma.Spec.DfaStateId := ⟨id.val⟩

def viewTokenId (id : GreatgrammaCore.ids.TokenId) :
    Greatgramma.Spec.TokenId := ⟨id.val⟩

def viewTerminalId (id : GreatgrammaCore.ids.TerminalId) :
    Greatgramma.Spec.TerminalId := ⟨id.val⟩

def viewParserStateId (id : GreatgrammaCore.ids.ParserStateId) :
    Greatgramma.Spec.ParserStateId := ⟨id.val⟩

def viewNonterminalId (id : GreatgrammaCore.ids.NonterminalId) :
    Greatgramma.Spec.NonterminalId := ⟨id.val⟩

def viewProductionId (id : GreatgrammaCore.ids.ProductionId) :
    Greatgramma.Spec.ProductionId := ⟨id.val⟩

def viewSequenceId (id : GreatgrammaCore.ids.SequenceId) :
    Greatgramma.Spec.SequenceId := ⟨id.val⟩

def viewValidationTable :
    GreatgrammaCore.error.ValidationTable → Greatgramma.Spec.ValidationTable
  | .Tokens => .tokens
  | .LexerByteClasses => .lexerByteClasses
  | .LexerTransitions => .lexerTransitions
  | .LexerStart => .lexerStart
  | .LexerTerminals => .lexerTerminals
  | .ParserActions => .parserActions
  | .ParserGotos => .parserGotos
  | .ParserStart => .parserStart
  | .ParserEof => .parserEof
  | .ParserIgnoredTerminals => .parserIgnoredTerminals
  | .Productions => .productions

def viewIdKind : GreatgrammaCore.error.IdKind → Greatgramma.Spec.IdKind
  | .DfaState => .dfaState
  | .Terminal => .terminal
  | .ParserState => .parserState
  | .Nonterminal => .nonterminal
  | .Production => .production

def viewArithmeticKind :
    GreatgrammaCore.error.ArithmeticKind → Greatgramma.Spec.ArithmeticKind
  | .TokenCount => .tokenCount
  | .TokenBytes => .tokenBytes
  | .ProductionCount => .productionCount
  | .IgnoredTerminalCount => .ignoredTerminalCount
  | .LexerCells => .lexerCells
  | .ParserActionCells => .parserActionCells
  | .ParserGotoCells => .parserGotoCells
  | .ParserCells => .parserCells
  | .LogicalBytes => .logicalBytes
  | .Work => .work

def viewLimitKind : GreatgrammaCore.error.LimitKind → Greatgramma.Spec.LimitKind
  | .Tokens => .tokens
  | .TokenBytes => .tokenBytes
  | .DfaStates => .dfaStates
  | .DfaClasses => .dfaClasses
  | .DfaCells => .dfaCells
  | .ParserStates => .parserStates
  | .Terminals => .terminals
  | .Nonterminals => .nonterminals
  | .Productions => .productions
  | .ParserCells => .parserCells
  | .LogicalBytes => .logicalBytes
  | .Work => .work

def viewValidationLimits (limits : GreatgrammaCore.limits.ValidationLimits) :
    Greatgramma.Spec.ValidationLimits where
  maxTokens := limits.max_tokens.val
  maxTokenBytes := limits.max_token_bytes.val
  maxDfaStates := limits.max_dfa_states.val
  maxDfaClasses := limits.max_dfa_classes.val
  maxDfaCells := limits.max_dfa_cells.val
  maxParserStates := limits.max_parser_states.val
  maxTerminals := limits.max_terminals.val
  maxNonterminals := limits.max_nonterminals.val
  maxProductions := limits.max_productions.val
  maxParserCells := limits.max_parser_cells.val
  maxLogicalBytes := limits.max_logical_bytes.val
  maxWork := limits.max_work.val

def viewValidationError :
    GreatgrammaCore.error.ValidationError → Greatgramma.Spec.ValidationFailure
  | .EmptyTokenTable => .emptyTokenTable
  | .MissingOrdinaryToken => .missingOrdinaryToken
  | .MissingEosToken => .missingEosToken
  | .EmptyTokenBytes token => .emptyTokenBytes (viewTokenId token)
  | .WrongTableLength table expected actual =>
      .wrongTableLength (viewValidationTable table) expected.val actual.val
  | .IdOutOfRange table index kind id count =>
      .idOutOfRange (viewValidationTable table) (index.map (·.val))
        (viewIdKind kind) id.val count.val
  | .ByteClassOutOfRange byte classId classCount =>
      .byteClassOutOfRange (viewByte byte) classId.val classCount.val
  | .AcceptingLexerStart state terminal =>
      .acceptingLexerStart (viewDfaStateId state) (viewTerminalId terminal)
  | .LexerTerminalIsParserEof state terminal =>
      .lexerTerminalIsParserEof (viewDfaStateId state) (viewTerminalId terminal)
  | .IgnoredTerminalIsParserEof index terminal =>
      .ignoredTerminalIsParserEof index.val (viewTerminalId terminal)
  | .AcceptOnNonEof state terminal =>
      .acceptOnNonEof (viewParserStateId state) (viewTerminalId terminal)
  | .ArithmeticOverflow calculation =>
      .arithmeticOverflow (viewArithmeticKind calculation)
  | .LimitExceeded limit actual maximum =>
      .limitExceeded (viewLimitKind limit) actual.val maximum.val
  | .AllocationFailure table requested =>
      .allocationFailure (viewValidationTable table) requested.val

def viewTokenEntry :
    GreatgrammaCore.normalized.TokenEntry → Greatgramma.Spec.TokenEntry
  | .Bytes bytes => .bytes (bytes.val.map viewByte)
  | .Eos => .eos

def viewAction : GreatgrammaCore.normalized.Action → Greatgramma.Spec.LalrAction
  | .Error => .error
  | .Shift destination => .shift (viewParserStateId destination)
  | .Reduce production rank => .reduce (viewProductionId production) rank.val
  | .Accept => .accept

def viewProduction (production : GreatgrammaCore.normalized.Production) :
    Greatgramma.Spec.Production :=
  ⟨viewNonterminalId production.lhs, production.pop_len.val⟩

def viewLalrDimensions
    (dimensions : GreatgrammaCore.normalized.LalrDimensions) :
    Greatgramma.Spec.LalrDimensions :=
  ⟨dimensions.state_count.val, dimensions.terminal_count.val,
    dimensions.nonterminal_count.val⟩

def viewLexer (lexer : GreatgrammaCore.normalized.LexerDfa) :
    Greatgramma.Spec.LexerDfa where
  stateCount := lexer.state_count.val
  classCount := lexer.class_count.val
  byteClasses := lexer.byte_classes.val.map (·.val)
  transitions := lexer.transitions.val.map (Option.map viewDfaStateId)
  startState := viewDfaStateId lexer.start_state
  terminals := lexer.terminals.val.map (Option.map viewTerminalId)

def viewLalr (lalr : GreatgrammaCore.normalized.LalrTable) :
    Greatgramma.Spec.LalrTable where
  dimensions := viewLalrDimensions lalr.dimensions
  startState := viewParserStateId lalr.start_state
  eofTerminal := viewTerminalId lalr.eof_terminal
  actions := lalr.actions.val.map viewAction
  gotos := lalr.gotos.val.map (Option.map viewParserStateId)
  productions := lalr.productions.val.map viewProduction
  ignoredTerminals := lalr.ignored_terminals.val.map viewTerminalId

def viewUnvalidated
    (grammar : GreatgrammaCore.normalized.UnvalidatedGrammar) :
    Greatgramma.Spec.NormalizedGrammar where
  tokens := grammar.tokens.val.map viewTokenEntry
  lexer := viewLexer grammar.lexer
  lalr := viewLalr grammar.lalr

def TokenEntryWF : GreatgrammaCore.normalized.TokenEntry → Prop
  | .Bytes bytes => bytes.val ≠ []
  | .Eos => True

def IsOrdinary : GreatgrammaCore.normalized.TokenEntry → Prop
  | .Bytes _ => True
  | .Eos => False

def IsEos : GreatgrammaCore.normalized.TokenEntry → Prop
  | .Bytes _ => False
  | .Eos => True

def OptionalIdInRange (count : U32) : Option U32 → Prop
  | none => True
  | some id => id.val < count.val

def LexerTerminalWF (terminalCount : U32) (eof : GreatgrammaCore.ids.TerminalId) :
    Option GreatgrammaCore.ids.TerminalId → Prop
  | none => True
  | some terminal => terminal.val < terminalCount.val ∧ terminal ≠ eof

def ActionWF
    (stateCount productionCount terminalCount : U32)
    (eof : GreatgrammaCore.ids.TerminalId) (index : Nat) :
    GreatgrammaCore.normalized.Action → Prop
  | .Error => True
  | .Shift destination => destination.val < stateCount.val
  | .Reduce production _ => production.val < productionCount.val
  | .Accept => terminalCount.val > 0 ∧ index % terminalCount.val = eof.val

structure ValidatedLexerWF
    (lexer : GreatgrammaCore.normalized.ValidatedLexer)
    (terminalCount : U32) (eof : GreatgrammaCore.ids.TerminalId) : Prop where
  byteClassLength : lexer.byte_classes.val.length = 256
  transitionLength :
    lexer.transitions.val.length = lexer.state_count.val * lexer.class_count.val
  terminalLength : lexer.terminals.val.length = lexer.state_count.val
  startInRange : lexer.start_state.val < lexer.state_count.val
  byteClassesInRange :
    ∀ byteClass ∈ lexer.byte_classes.val, byteClass.val < lexer.class_count.val
  transitionsInRange :
    ∀ destination ∈ lexer.transitions.val,
      OptionalIdInRange lexer.state_count destination
  terminalsInRange :
    ∀ terminal ∈ lexer.terminals.val, LexerTerminalWF terminalCount eof terminal
  startNonaccepting : lexer.terminals.val[lexer.start_state.val]? = some none

structure ValidatedLalrWF
    (lalr : GreatgrammaCore.normalized.ValidatedLalr) : Prop where
  actionLength :
    lalr.actions.val.length =
      lalr.dimensions.state_count.val * lalr.dimensions.terminal_count.val
  gotoLength :
    lalr.gotos.val.length =
      lalr.dimensions.state_count.val * lalr.dimensions.nonterminal_count.val
  productionLength : lalr.productions.val.length = lalr.production_count.val
  ignoredLength :
    lalr.ignored_terminals.val.length = lalr.dimensions.terminal_count.val
  startInRange : lalr.start_state.val < lalr.dimensions.state_count.val
  eofInRange : lalr.eof_terminal.val < lalr.dimensions.terminal_count.val
  actionsInRange :
    ∀ index action,
      lalr.actions.val[index]? = some action →
      ActionWF lalr.dimensions.state_count lalr.production_count
        lalr.dimensions.terminal_count lalr.eof_terminal index action
  gotosInRange :
    ∀ destination ∈ lalr.gotos.val,
      OptionalIdInRange lalr.dimensions.state_count destination
  productionsInRange :
    ∀ production ∈ lalr.productions.val,
      production.lhs.val < lalr.dimensions.nonterminal_count.val
  ignoredBits :
    ∀ bit ∈ lalr.ignored_terminals.val, bit.val = 0 ∨ bit.val = 1
  eofNotIgnored : lalr.ignored_terminals.val[lalr.eof_terminal.val]? = some 0#u8

structure ValidatedWF
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar) : Prop where
  tokenCount : grammar.token_count.val = grammar.tokens.val.length
  tokenTableNonempty : grammar.tokens.val ≠ []
  tokenEntries : ∀ token ∈ grammar.tokens.val, TokenEntryWF token
  hasOrdinary : ∃ token ∈ grammar.tokens.val, IsOrdinary token
  hasEos : ∃ token ∈ grammar.tokens.val, IsEos token
  lexer : ValidatedLexerWF grammar.lexer grammar.lalr.dimensions.terminal_count
    grammar.lalr.eof_terminal
  lalr : ValidatedLalrWF grammar.lalr

/-- The successful value owns exactly the caller's tables.  The sole
representation change is expansion of ignored terminal IDs into a membership
bitmap. -/
structure ValidatedRepresents
    (raw : GreatgrammaCore.normalized.UnvalidatedGrammar)
    (validated : GreatgrammaCore.normalized.ValidatedGrammar) : Prop where
  tokenCount : validated.token_count.val = raw.tokens.val.length
  tokens : validated.tokens.val = raw.tokens.val
  lexerStateCount : validated.lexer.state_count = raw.lexer.state_count
  lexerClassCount : validated.lexer.class_count = raw.lexer.class_count
  byteClasses : validated.lexer.byte_classes.val = raw.lexer.byte_classes.val
  transitions : validated.lexer.transitions.val = raw.lexer.transitions.val
  lexerStart : validated.lexer.start_state = raw.lexer.start_state
  lexerTerminals : validated.lexer.terminals.val = raw.lexer.terminals.val
  dimensions : validated.lalr.dimensions = raw.lalr.dimensions
  parserStart : validated.lalr.start_state = raw.lalr.start_state
  parserEof : validated.lalr.eof_terminal = raw.lalr.eof_terminal
  actions : validated.lalr.actions.val = raw.lalr.actions.val
  gotos : validated.lalr.gotos.val = raw.lalr.gotos.val
  productionCount :
    validated.lalr.production_count.val = raw.lalr.productions.val.length
  productions : validated.lalr.productions.val = raw.lalr.productions.val
  ignoredLength :
    validated.lalr.ignored_terminals.val.length =
      raw.lalr.dimensions.terminal_count.val
  ignoredSourceInRange :
    ∀ terminal ∈ raw.lalr.ignored_terminals.val,
      terminal.val < raw.lalr.dimensions.terminal_count.val
  ignoredSourceExcludesEof :
    raw.lalr.eof_terminal ∉ raw.lalr.ignored_terminals.val
  ignoredMembership :
    ∀ index, index < raw.lalr.dimensions.terminal_count.val →
      ((∃ bit, validated.lalr.ignored_terminals.val[index]? = some bit ∧
          bit ≠ 0#u8) ↔
        ∃ terminal ∈ raw.lalr.ignored_terminals.val, terminal.val = index)

end Greatgramma.Invariants
