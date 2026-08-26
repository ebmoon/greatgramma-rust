import Invariants.Prepared

open Aeneas Aeneas.Std

namespace Greatgramma.Invariants

def LexerStateWF
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar) :
    GreatgrammaCore.lexer.LexerState → Prop
  | .Start => True
  | .Dfa state => state.val < grammar.lexer.state_count.val

def ParserStackIdsInRange
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar)
    (stack : alloc.vec.Vec GreatgrammaCore.ids.ParserStateId) : Prop :=
  ∀ state ∈ stack.val, state.val < grammar.lalr.dimensions.state_count.val

def ParserStackWF
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar)
    (stack : alloc.vec.Vec GreatgrammaCore.ids.ParserStateId) : Prop :=
  stack.val ≠ [] ∧ ParserStackIdsInRange grammar stack

def MatcherStatusWF
    (grammar : GreatgrammaCore.normalized.ValidatedGrammar)
    (status : GreatgrammaCore.engine.MatcherStatus) : Prop :=
  match status.phase with
  | .Running lexer => LexerStateWF grammar lexer ∧ ParserStackWF grammar status.parser_stack
  | .Accepted => status.parser_stack.val = []

/-- Workspace validity is separated because transactional proofs preserve it
even when an operation returns a domain error and leaves committed rows alone.
Uncommitted phases may be stale and stacks may be empty after a failed reserve,
so the workspace records only its persistent row shape and the range safety of
parser-state IDs that remain in reusable staging and scratch storage. -/
def WorkspaceWF (matcher : GreatgrammaCore.engine.Matcher) : Prop :=
  matcher.staging.val.length = matcher.states.val.length ∧
    (∀ status ∈ matcher.staging.val,
      ParserStackIdsInRange matcher.prepared.grammar status.parser_stack) ∧
    ParserStackIdsInRange matcher.prepared.grammar matcher.scratch

structure MatcherWF (matcher : GreatgrammaCore.engine.Matcher) : Prop where
  prepared : PreparedWF matcher.prepared
  rowsNonempty : matcher.states.val ≠ []
  committedRows :
    ∀ status ∈ matcher.states.val, MatcherStatusWF matcher.prepared.grammar status
  workspace : WorkspaceWF matcher

end Greatgramma.Invariants
