import Spec.Types

namespace Greatgramma.Spec

structure LexerDfa where
  stateCount : Nat
  classCount : Nat
  byteClasses : List Nat
  transitions : List (Option DfaStateId)
  startState : DfaStateId
  terminals : List (Option TerminalId)
deriving DecidableEq, Repr

inductive LexerPosition where
  | logicalStart
  | dfa (state : DfaStateId)
deriving DecidableEq, Repr

structure LexerByteEvent where
  next : LexerPosition
  emitted : Option TerminalId
deriving DecidableEq, Repr

def LexerDfa.byteClass? (lexer : LexerDfa) (byte : Byte) : Option Nat :=
  lexer.byteClasses[byte.val]?

/-!
The outer option reports whether the source/table lookup is valid.  The inner
option is the normalized DFA's deliberately absent transition.
-/
def LexerDfa.transition? (lexer : LexerDfa) (state : DfaStateId)
    (byte : Byte) : Option (Option DfaStateId) := do
  let classId ← lexer.byteClass? byte
  checkedFlatLookup lexer.transitions lexer.stateCount lexer.classCount state.val classId

def LexerDfa.terminal? (lexer : LexerDfa)
    (state : DfaStateId) : Option (Option TerminalId) :=
  if state.val < lexer.stateCount then lexer.terminals[state.val]? else none

/-!
One corrected maximal-munch byte step.  When a transition from an active DFA
state is missing, the priority-resolved terminal is emitted and the same
boundary byte is consumed from the DFA start state.  `logicalStart` is kept
distinct from `dfa startState`, so EOS cannot invent an empty lexeme.
-/
def LexerDfa.byteStep? (lexer : LexerDfa) (source : LexerPosition)
    (byte : Byte) : Option LexerByteEvent :=
  match source with
  | .logicalStart =>
      match lexer.transition? lexer.startState byte with
      | some (some destination) => some ⟨.dfa destination, none⟩
      | _ => none
  | .dfa state =>
      match lexer.transition? state byte with
      | some (some destination) => some ⟨.dfa destination, none⟩
      | some none =>
          match lexer.terminal? state, lexer.transition? lexer.startState byte with
          | some (some terminal), some (some destination) =>
              some ⟨.dfa destination, some terminal⟩
          | _, _ => none
      | none => none

def LexerDfa.scan (lexer : LexerDfa) :
    LexerPosition → List Byte → Option (LexerPosition × List TerminalId)
  | source, [] => some (source, [])
  | source, byte :: rest => do
      let event ← lexer.byteStep? source byte
      let (destination, emitted) ← lexer.scan event.next rest
      pure (destination, event.emitted.toList ++ emitted)

def LexerDfa.residualTerminal? (lexer : LexerDfa) :
    LexerPosition → Option TerminalId
  | .logicalStart => none
  | .dfa state => (lexer.terminal? state).join

/-!
A terminal is a realizable next head either after a zero-output byte path to a
residual that would flush it at EOS, or when a nonempty byte suffix emits it
first.  The first case exactly matches the accepting seeds and predecessor
propagation used by Rust's reverse-fact construction.
-/
def LexerDfa.FirstEmission (lexer : LexerDfa) (source : LexerPosition)
    (terminal : TerminalId) : Prop :=
  (∃ silent destination,
      lexer.scan source silent = some (destination, []) ∧
      lexer.residualTerminal? destination = some terminal) ∨
    ∃ suffix destination emitted,
      suffix ≠ [] ∧
      lexer.scan source suffix = some (destination, terminal :: emitted)

def LexerDfa.StateInRange (lexer : LexerDfa) (state : DfaStateId) : Prop :=
  state.val < lexer.stateCount

/-!
Pure structural validation facts for a lexer.  Parser-dependent label checks
are parameters here, not semantic language assumptions.
-/
structure LexerWF (lexer : LexerDfa) (terminalCount : Nat)
    (parserEof : TerminalId) : Prop where
  byteClassLength : lexer.byteClasses.length = 256
  transitionShape : FlatShape lexer.transitions lexer.stateCount lexer.classCount
  terminalLength : lexer.terminals.length = lexer.stateCount
  startInRange : lexer.startState.val < lexer.stateCount
  byteClassesInRange : ∀ classId ∈ lexer.byteClasses, classId < lexer.classCount
  destinationsInRange :
    ∀ cell ∈ lexer.transitions, ∀ destination,
      cell = some destination → destination.val < lexer.stateCount
  labelsInRange :
    ∀ cell ∈ lexer.terminals, ∀ terminal,
      cell = some terminal → terminal.val < terminalCount
  startNonaccepting : lexer.terminal? lexer.startState = some none
  labelsExcludeParserEof :
    ∀ cell ∈ lexer.terminals, cell ≠ some parserEof

inductive LexerReachable (lexer : LexerDfa) : LexerPosition → Prop where
  | initial : LexerReachable lexer .logicalStart
  | byte {source byte event}
      (reachable : LexerReachable lexer source)
      (step : lexer.byteStep? source byte = some event) :
      LexerReachable lexer event.next

@[simp] theorem LexerDfa.scan_nil (lexer : LexerDfa) (source : LexerPosition) :
    lexer.scan source [] = some (source, []) := rfl

@[simp] theorem LexerDfa.residualTerminal_logicalStart (lexer : LexerDfa) :
    lexer.residualTerminal? .logicalStart = none := rfl

end Greatgramma.Spec
