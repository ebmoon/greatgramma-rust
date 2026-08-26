import Spec.Types

namespace Greatgramma.Spec

structure LalrDimensions where
  stateCount : Nat
  terminalCount : Nat
  nonterminalCount : Nat
deriving DecidableEq, Repr

inductive LalrAction where
  | error
  | shift (destination : ParserStateId)
  | reduce (production : ProductionId) (rank : Nat)
  | accept
deriving DecidableEq, Repr

structure Production where
  lhs : NonterminalId
  popLen : Nat
deriving DecidableEq, Repr

structure LalrTable where
  dimensions : LalrDimensions
  startState : ParserStateId
  eofTerminal : TerminalId
  actions : List LalrAction
  gotos : List (Option ParserStateId)
  productions : List Production
  ignoredTerminals : List TerminalId
deriving DecidableEq, Repr

def LalrTable.action? (table : LalrTable) (state : ParserStateId)
    (terminal : TerminalId) : Option LalrAction :=
  checkedFlatLookup table.actions table.dimensions.stateCount
    table.dimensions.terminalCount state.val terminal.val

/-! The outer option validates the cell; the inner option is an absent goto. -/
def LalrTable.goto? (table : LalrTable) (state : ParserStateId)
    (nonterminal : NonterminalId) : Option (Option ParserStateId) :=
  checkedFlatLookup table.gotos table.dimensions.stateCount
    table.dimensions.nonterminalCount state.val nonterminal.val

def LalrTable.production? (table : LalrTable)
    (production : ProductionId) : Option Production :=
  table.productions[production.val]?

def LalrTable.IsIgnored (table : LalrTable) (terminal : TerminalId) : Prop :=
  terminal ∈ table.ignoredTerminals

/-! Exactly the table facts checked at the normalized validation boundary. -/
structure LalrWF (table : LalrTable) : Prop where
  actionShape :
    FlatShape table.actions table.dimensions.stateCount table.dimensions.terminalCount
  gotoShape :
    FlatShape table.gotos table.dimensions.stateCount table.dimensions.nonterminalCount
  startInRange : table.startState.val < table.dimensions.stateCount
  eofInRange : table.eofTerminal.val < table.dimensions.terminalCount
  shiftsInRange :
    ∀ action ∈ table.actions, ∀ destination,
      action = .shift destination → destination.val < table.dimensions.stateCount
  reductionsInRange :
    ∀ action ∈ table.actions, ∀ production rank,
      action = .reduce production rank → production.val < table.productions.length
  acceptsOnlyEof :
    ∀ state terminal,
      table.action? state terminal = some .accept → terminal = table.eofTerminal
  gotosInRange :
    ∀ cell ∈ table.gotos, ∀ destination,
      cell = some destination → destination.val < table.dimensions.stateCount
  productionLhsInRange :
    ∀ production ∈ table.productions,
      production.lhs.val < table.dimensions.nonterminalCount
  ignoredInRange :
    ∀ terminal ∈ table.ignoredTerminals,
      terminal.val < table.dimensions.terminalCount
  ignoredExcludeEof : table.eofTerminal ∉ table.ignoredTerminals

/-! Parser stacks use the head of the list as their top. -/
abbrev ParserStack := List ParserStateId

def ParserStackWF (table : LalrTable) (stack : ParserStack) : Prop :=
  stack ≠ [] ∧ ∀ state ∈ stack, state.val < table.dimensions.stateCount

def LalrTable.currentAction? (table : LalrTable) (stack : ParserStack)
    (terminal : TerminalId) : Option LalrAction :=
  match stack with
  | [] => none
  | state :: _ => table.action? state terminal

/-!
Apply a production to a top-at-head stack.  Requiring a state below the popped
suffix is the pure counterpart of Rust's `pop_len < stack.len()` check.
-/
def LalrTable.applyReduction? (table : LalrTable) (stack : ParserStack)
    (productionId : ProductionId) : Option (ParserStack × Nat) :=
  match table.production? productionId with
  | none => none
  | some production =>
      let remaining := stack.drop production.popLen
      match remaining with
      | [] => none
      | source :: _ =>
          match table.goto? source production.lhs with
          | some (some destination) =>
              some (destination :: remaining, production.popLen)
          | _ => none

structure PreviousReduction where
  rank : Nat
  popLen : Nat
deriving DecidableEq, Repr

def ReductionProgress (previous : Option PreviousReduction) (nextRank : Nat) : Prop :=
  match previous with
  | none => True
  | some prior => nextRank < prior.rank ∨
      (nextRank = prior.rank ∧ 2 ≤ prior.popLen)

inductive TerminalOutcome where
  | rejected
  | shifted (stack : ParserStack)
  | accepted
deriving DecidableEq, Repr

/-!
Ranked execution of one nonignored terminal.  Reductions recur only through
the constructor carrying Rust's exact rank/pop-length progress condition.
-/
inductive FeedTerminal (table : LalrTable) (terminal : TerminalId) :
    Option PreviousReduction → ParserStack → TerminalOutcome → Prop where
  | reject {previous stack}
      (action : table.currentAction? stack terminal = some .error) :
      FeedTerminal table terminal previous stack .rejected
  | shift {previous stack destination}
      (action : table.currentAction? stack terminal = some (.shift destination)) :
      FeedTerminal table terminal previous stack (.shifted (destination :: stack))
  | accept {previous stack}
      (action : table.currentAction? stack terminal = some .accept) :
      FeedTerminal table terminal previous stack .accepted
  | reduce {previous stack production rank next popLen outcome}
      (action : table.currentAction? stack terminal = some (.reduce production rank))
      (progress : ReductionProgress previous rank)
      (applied : table.applyReduction? stack production = some (next, popLen))
      (rest : FeedTerminal table terminal (some ⟨rank, popLen⟩) next outcome) :
      FeedTerminal table terminal previous stack outcome

inductive ParserOutcome where
  | rejected
  | running (stack : ParserStack)
  | accepted
deriving DecidableEq, Repr

/-!
Ignored terminals are true parser no-ops.  Acceptance is only a complete run
when it occurs on the final supplied terminal.
-/
inductive RunTerminals (table : LalrTable) :
    ParserStack → List TerminalId → ParserOutcome → Prop where
  | nil {stack} : RunTerminals table stack [] (.running stack)
  | ignored {stack terminal rest outcome}
      (ignored : table.IsIgnored terminal)
      (tail : RunTerminals table stack rest outcome) :
      RunTerminals table stack (terminal :: rest) outcome
  | rejected {stack terminal rest}
      (notIgnored : ¬ table.IsIgnored terminal)
      (feed : FeedTerminal table terminal none stack .rejected) :
      RunTerminals table stack (terminal :: rest) .rejected
  | shifted {stack terminal rest next outcome}
      (notIgnored : ¬ table.IsIgnored terminal)
      (feed : FeedTerminal table terminal none stack (.shifted next))
      (tail : RunTerminals table next rest outcome) :
      RunTerminals table stack (terminal :: rest) outcome
  | accepted {stack terminal}
      (notIgnored : ¬ table.IsIgnored terminal)
      (feed : FeedTerminal table terminal none stack .accepted) :
      RunTerminals table stack [terminal] .accepted

def LalrTable.initialStack (table : LalrTable) : ParserStack := [table.startState]

def LalrTable.Accepts (table : LalrTable) (terminals : List TerminalId) : Prop :=
  RunTerminals table table.initialStack terminals .accepted

def LalrTable.ReachableStack (table : LalrTable) (stack : ParserStack) : Prop :=
  ∃ terminals, RunTerminals table table.initialStack terminals (.running stack)

/-!
A realizable sequence head commits every terminal except the final probe.  The
probe is readable when it shifts, accepts, or is an ignored no-op.
-/
def LalrTable.HeadReadable (table : LalrTable) (stack : ParserStack)
    (head : List TerminalId) : Prop :=
  ∃ direct probe committed,
    head = direct ++ [probe] ∧
    RunTerminals table stack direct (.running committed) ∧
    ((∃ next, RunTerminals table committed [probe] (.running next)) ∨
      RunTerminals table committed [probe] .accepted)

end Greatgramma.Spec
