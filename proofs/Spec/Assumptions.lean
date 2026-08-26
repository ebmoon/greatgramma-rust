import Spec.FiniteHeadChecker

/-!
Named semantic premises for the language-correctness tier.

None of these declarations is an axiom: each is a `Prop`-valued structure that
must be supplied explicitly to a theorem.  In particular, successful Rust
validation establishes `StructuralWF`; it does not manufacture any value from
this file.
-/

namespace Greatgramma.Spec

abbrev ModelVocabulary := TokenId → Option TokenEntry
abbrev TerminalLanguage := List TerminalId → Prop
abbrev ByteLanguage := List Byte → Prop

/-! AS1: normalized token bytes and all EOS entries agree with the model. -/
structure TokenBytesCorrect (grammar : NormalizedGrammar)
    (modelVocabulary : ModelVocabulary) : Prop where
  agrees : ∀ token, modelVocabulary token = token? grammar.tokens token

/-!
An external statement of priority-resolved, one-lookahead maximal-munch lexer
semantics.  Keeping it relational permits an intended grammar/compiler model
without importing that compiler into the implementation proof.
-/
structure LexerSemantics where
  scans : LexerPosition → List Byte → LexerPosition → List TerminalId → Prop
  firstEmission : LexerPosition → TerminalId → Prop

/-! AS2: the supplied DFA implements the intended maximal-munch relation. -/
structure LexerDfaCorrect (lexer : LexerDfa) (semantics : LexerSemantics) : Prop where
  scanIff : ∀ source bytes destination emitted,
    lexer.scan source bytes = some (destination, emitted) ↔
      semantics.scans source bytes destination emitted
  firstEmissionIff : ∀ source terminal,
    lexer.FirstEmission source terminal ↔
      semantics.firstEmission source terminal
  statesReachable : ∀ state,
    state.val < lexer.stateCount → LexerReachable lexer (.dfa state)

/-!
AS3: table execution is safe on reachable stacks.  The structural ID and
flattened-table obligations remain in `LalrWF`; these fields state the stronger
operational facts not checked by normalized validation.
-/
structure LalrOperationalWF (table : LalrTable) : Prop where
  reachableStacksWellFormed : ∀ stack,
    table.ReachableStack stack → ParserStackWF table stack
  actionDefined : ∀ stack terminal,
    table.ReachableStack stack →
    terminal.val < table.dimensions.terminalCount →
    ¬ table.IsIgnored terminal →
    ∃ action, table.currentAction? stack terminal = some action
  reachableReductionDefined : ∀ stack terminal production rank,
    table.ReachableStack stack →
    table.currentAction? stack terminal = some (.reduce production rank) →
    ∃ next popLen, table.applyReduction? stack production = some (next, popLen)

/-!
AS4: ranked reduction execution is total and deterministic on reachable input.
Existence rules out invalid rank progress and infinite reduction chains;
uniqueness makes the relation a deterministic parser operation.
-/
structure ReductionRankCorrect (table : LalrTable) : Prop where
  total : ∀ stack terminal,
    table.ReachableStack stack →
    terminal.val < table.dimensions.terminalCount →
    ¬ table.IsIgnored terminal →
    ∃ outcome, FeedTerminal table terminal none stack outcome
  deterministic : ∀ stack terminal left right,
    table.ReachableStack stack →
    FeedTerminal table terminal none stack left →
    FeedTerminal table terminal none stack right →
    left = right

/-! AS5: ranked LALR acceptance denotes the intended terminal language. -/
structure LalrLanguageCorrect (table : LalrTable)
    (language : TerminalLanguage) : Prop where
  acceptsIff : ∀ terminals, table.Accepts terminals ↔ language terminals

/-!
AS6: the missing realizability premise in the original paper.  Every reachable
ordinary finite head admitted by the checker has an actual model-token
continuation to acceptance.  The witness starts with the admitted token's
committed successor, so arbitrary byte suffixes cannot silently stand in for
model tokens.
-/
structure ContinuationRealizable (grammar : NormalizedGrammar) : Prop where
  realize : ∀ state token,
    ReachableMatcherState grammar (.running state) →
    OrdinaryFiniteHeadAllows grammar state token →
    ∃ successor continuation,
      FiniteHeadStep grammar (.running state) token successor ∧
      FiniteHeadSteps grammar successor continuation .accepted

/-! AS7: every reachable parser configuration has an accepting continuation. -/
structure ParserProductive (table : LalrTable) : Prop where
  productive : ∀ stack,
    table.ReachableStack stack →
    ∃ terminals, RunTerminals table stack terminals .accepted

/-!
The bundle used by the pure language theorem.  `StructuralWF` is intentionally
not a field: callers pass structural evidence separately, and the generated
implementation-refinement theorem never needs this semantic bundle.
-/
structure SemanticAssumptions (grammar : NormalizedGrammar)
    (modelVocabulary : ModelVocabulary) (lexerSemantics : LexerSemantics)
    (terminalLanguage : TerminalLanguage) : Prop where
  tokenBytes : TokenBytesCorrect grammar modelVocabulary
  lexerDfa : LexerDfaCorrect grammar.lexer lexerSemantics
  lalrOperational : LalrOperationalWF grammar.lalr
  reductionRanks : ReductionRankCorrect grammar.lalr
  lalrLanguage : LalrLanguageCorrect grammar.lalr terminalLanguage
  continuation : ContinuationRealizable grammar
  parserProductive : ParserProductive grammar.lalr

end Greatgramma.Spec
