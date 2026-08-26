import Spec.Token
import Spec.Lexer
import Spec.Lalr

namespace Greatgramma.Spec

structure NormalizedGrammar where
  tokens : TokenTable
  lexer : LexerDfa
  lalr : LalrTable
deriving DecidableEq, Repr

/-!
The complete structural boundary.  It mentions no intended tokenizer, lexer
language, grammar language, or continuation assumption.
-/
structure StructuralWF (grammar : NormalizedGrammar) : Prop where
  tokens : TokenTableWF grammar.tokens
  lexer : LexerWF grammar.lexer grammar.lalr.dimensions.terminalCount
    grammar.lalr.eofTerminal
  lalr : LalrWF grammar.lalr

structure RunningState where
  lexer : LexerPosition
  parserStack : ParserStack
deriving DecidableEq, Repr

inductive MatcherState where
  | running (state : RunningState)
  | accepted
deriving DecidableEq, Repr

def NormalizedGrammar.initialState (grammar : NormalizedGrammar) : MatcherState :=
  .running ⟨.logicalStart, grammar.lalr.initialStack⟩

def NormalizedGrammar.eosTerminals? (grammar : NormalizedGrammar)
    (position : LexerPosition) : Option (List TerminalId) :=
  match position with
  | .logicalStart => some [grammar.lalr.eofTerminal]
  | .dfa state => do
      let terminal ← (grammar.lexer.terminal? state).join
      pure [terminal, grammar.lalr.eofTerminal]

/-!
An ordinary token is admitted by the finite-head relation when its direct
emissions can be committed and a *realizable first lexer emission* is readable
as the final speculative parser head.  The future bytes witnessing that first
emission are not assumed to be model-tokenizable here; that logically distinct
premise is `ContinuationRealizable` in `Spec.Assumptions`.
-/
def OrdinaryFiniteHeadAllows (grammar : NormalizedGrammar) (state : RunningState)
    (token : TokenId) : Prop :=
  ∃ bytes residual emitted continuation,
    token? grammar.tokens token = some (.bytes bytes) ∧
    grammar.lexer.scan state.lexer bytes = some (residual, emitted) ∧
    grammar.lexer.FirstEmission residual continuation ∧
    grammar.lalr.HeadReadable state.parserStack (emitted ++ [continuation])

def EosFiniteHeadAllows (grammar : NormalizedGrammar) (state : RunningState)
    (token : TokenId) : Prop :=
  ∃ terminals,
    token? grammar.tokens token = some .eos ∧
    grammar.eosTerminals? state.lexer = some terminals ∧
    RunTerminals grammar.lalr state.parserStack terminals .accepted

def FiniteHeadAllows (grammar : NormalizedGrammar) (status : MatcherState)
    (token : TokenId) : Prop :=
  match status with
  | .accepted => False
  | .running state =>
      OrdinaryFiniteHeadAllows grammar state token ∨
      EosFiniteHeadAllows grammar state token

/-!
The transition relation commits only the token's direct emissions.  The final
head used by ordinary allowance remains speculative.  EOS instead flushes a
residual lexeme and must accept parser EOF.
-/
inductive FiniteHeadStep (grammar : NormalizedGrammar) :
    MatcherState → TokenId → MatcherState → Prop where
  | ordinary {state token bytes residual emitted nextStack}
      (entry : token? grammar.tokens token = some (.bytes bytes))
      (scan : grammar.lexer.scan state.lexer bytes = some (residual, emitted))
      (allowed : OrdinaryFiniteHeadAllows grammar state token)
      (commit : RunTerminals grammar.lalr state.parserStack emitted (.running nextStack)) :
      FiniteHeadStep grammar (.running state) token
        (.running ⟨residual, nextStack⟩)
  | eos {state token terminals}
      (entry : token? grammar.tokens token = some .eos)
      (flush : grammar.eosTerminals? state.lexer = some terminals)
      (accepts : RunTerminals grammar.lalr state.parserStack terminals .accepted) :
      FiniteHeadStep grammar (.running state) token .accepted

inductive FiniteHeadSteps (grammar : NormalizedGrammar) :
    MatcherState → TokenHistory → MatcherState → Prop where
  | nil {state} : FiniteHeadSteps grammar state [] state
  | cons {source token next rest destination}
      (step : FiniteHeadStep grammar source token next)
      (tail : FiniteHeadSteps grammar next rest destination) :
      FiniteHeadSteps grammar source (token :: rest) destination

def ReachableMatcherState (grammar : NormalizedGrammar) (state : MatcherState) : Prop :=
  ∃ history, FiniteHeadSteps grammar grammar.initialState history state

def AcceptedTokenHistory (grammar : NormalizedGrammar) (history : TokenHistory) : Prop :=
  FiniteHeadSteps grammar grammar.initialState history .accepted

def HasAcceptingContinuation (grammar : NormalizedGrammar)
    (state : MatcherState) : Prop :=
  ∃ continuation, FiniteHeadSteps grammar state continuation .accepted

/-!
An implementation-independent interface for an executable finite-head
checker.  Later units construct this interface from prepared Rust tables; the
two fields below make its Boolean decision and state update exact, rather than
merely sound.
-/
structure FiniteHeadChecker (grammar : NormalizedGrammar) where
  allows : MatcherState → TokenId → Bool
  advance? : MatcherState → TokenId → Option MatcherState
  allows_iff : ∀ state token,
    allows state token = true ↔ FiniteHeadAllows grammar state token
  advance_iff : ∀ source token destination,
    advance? source token = some destination ↔
      FiniteHeadStep grammar source token destination

theorem FiniteHeadChecker.disallowsAccepted
    (checker : FiniteHeadChecker grammar) (token : TokenId) :
    checker.allows .accepted token = false := by
  cases h : checker.allows .accepted token with
  | false => rfl
  | true =>
      have : FiniteHeadAllows grammar .accepted token :=
        (checker.allows_iff .accepted token).mp h
      exact False.elim this

end Greatgramma.Spec
