import Std

/-!
Stable, generated-code-independent mathematical types for GreatGramma.

The executable Rust representation uses fixed-width integers.  The specification
uses distinct natural-number identifiers instead: machine bounds belong to the
representation proof, while table membership belongs to the mathematical model.
-/

namespace Greatgramma.Spec

abbrev Byte := Fin 256

structure TokenId where
  val : Nat
deriving DecidableEq, Repr

structure DfaStateId where
  val : Nat
deriving DecidableEq, Repr

structure TerminalId where
  val : Nat
deriving DecidableEq, Repr

structure ParserStateId where
  val : Nat
deriving DecidableEq, Repr

structure NonterminalId where
  val : Nat
deriving DecidableEq, Repr

structure ProductionId where
  val : Nat
deriving DecidableEq, Repr

structure SequenceId where
  val : Nat
deriving DecidableEq, Repr

instance : Coe TokenId Nat := ⟨TokenId.val⟩
instance : Coe DfaStateId Nat := ⟨DfaStateId.val⟩
instance : Coe TerminalId Nat := ⟨TerminalId.val⟩
instance : Coe ParserStateId Nat := ⟨ParserStateId.val⟩
instance : Coe NonterminalId Nat := ⟨NonterminalId.val⟩
instance : Coe ProductionId Nat := ⟨ProductionId.val⟩
instance : Coe SequenceId Nat := ⟨SequenceId.val⟩

def flatIndex (width row column : Nat) : Nat := row * width + column

def flatLookup (cells : List α) (width row column : Nat) : Option α :=
  cells[flatIndex width row column]?

def checkedFlatLookup (cells : List α) (rows columns row column : Nat) : Option α :=
  if row < rows ∧ column < columns then
    flatLookup cells columns row column
  else
    none

def FlatShape (cells : List α) (rows columns : Nat) : Prop :=
  cells.length = rows * columns

inductive ValidationTable where
  | tokens
  | lexerByteClasses
  | lexerTransitions
  | lexerStart
  | lexerTerminals
  | parserActions
  | parserGotos
  | parserStart
  | parserEof
  | parserIgnoredTerminals
  | productions
deriving DecidableEq, Repr

inductive IdKind where
  | dfaState
  | terminal
  | parserState
  | nonterminal
  | production
deriving DecidableEq, Repr

inductive ArithmeticKind where
  | tokenCount
  | tokenBytes
  | productionCount
  | ignoredTerminalCount
  | lexerCells
  | parserActionCells
  | parserGotoCells
  | parserCells
  | logicalBytes
  | work
deriving DecidableEq, Repr

inductive LimitKind where
  | tokens
  | tokenBytes
  | dfaStates
  | dfaClasses
  | dfaCells
  | parserStates
  | terminals
  | nonterminals
  | productions
  | parserCells
  | logicalBytes
  | work
deriving DecidableEq, Repr

/-! A representation-independent view of `greatgramma_core::ValidationLimits`. -/
structure ValidationLimits where
  maxTokens : Nat
  maxTokenBytes : Nat
  maxDfaStates : Nat
  maxDfaClasses : Nat
  maxDfaCells : Nat
  maxParserStates : Nat
  maxTerminals : Nat
  maxNonterminals : Nat
  maxProductions : Nat
  maxParserCells : Nat
  maxLogicalBytes : Nat
  maxWork : Nat
deriving DecidableEq, Repr

def ValidationLimits.maximum (limits : ValidationLimits) : LimitKind → Nat
  | .tokens => limits.maxTokens
  | .tokenBytes => limits.maxTokenBytes
  | .dfaStates => limits.maxDfaStates
  | .dfaClasses => limits.maxDfaClasses
  | .dfaCells => limits.maxDfaCells
  | .parserStates => limits.maxParserStates
  | .terminals => limits.maxTerminals
  | .nonterminals => limits.maxNonterminals
  | .productions => limits.maxProductions
  | .parserCells => limits.maxParserCells
  | .logicalBytes => limits.maxLogicalBytes
  | .work => limits.maxWork

/-!
The proof layer preserves diagnostic categories but deliberately forgets Rust
formatting and platform-sized integer details.
-/
inductive ValidationFailure where
  | emptyTokenTable
  | missingOrdinaryToken
  | missingEosToken
  | emptyTokenBytes (token : TokenId)
  | wrongTableLength (table : ValidationTable) (expected actual : Nat)
  | idOutOfRange
      (table : ValidationTable) (index : Option Nat) (kind : IdKind)
      (id count : Nat)
  | byteClassOutOfRange (byte : Byte) (classId classCount : Nat)
  | acceptingLexerStart (state : DfaStateId) (terminal : TerminalId)
  | lexerTerminalIsParserEof (state : DfaStateId) (terminal : TerminalId)
  | ignoredTerminalIsParserEof (index : Nat) (terminal : TerminalId)
  | acceptOnNonEof (state : ParserStateId) (terminal : TerminalId)
  | arithmeticOverflow (calculation : ArithmeticKind)
  | limitExceeded (limit : LimitKind) (actual maximum : Nat)
  | allocationFailure (table : ValidationTable) (requested : Nat)
deriving DecidableEq, Repr

abbrev ValidationOutcome (α : Type u) := Except ValidationFailure α

def WithinLimit (limits : ValidationLimits) (kind : LimitKind) (actual : Nat) : Prop :=
  actual ≤ limits.maximum kind

end Greatgramma.Spec
