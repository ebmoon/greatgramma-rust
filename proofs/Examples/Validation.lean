import Spec.Assumptions
import Refinement.Normalized
import Refinement.Validate
import Refinement.ValidatedView

open Aeneas Aeneas.Std Result

/-!
Small regression examples for both the stable structural specification and
the Aeneas-generated validation/runtime boundary.  The positive fixtures are
intentionally only structurally valid: they do not claim lexer-language
correctness, parser productivity, or any other semantic assumption from
`Spec.Assumptions`.
-/

namespace Greatgramma.Examples.Validation

open Greatgramma.Spec
open Greatgramma.Invariants Greatgramma.Refinement GreatgrammaCore

private def byte0 : Greatgramma.Spec.Byte := ⟨0, by decide⟩

private def ordinaryToken : TokenEntry := .bytes [byte0]

private def minimalTokens : TokenTable := [ordinaryToken, .eos]

private def minimalLexer : LexerDfa where
  stateCount := 1
  classCount := 1
  byteClasses := List.replicate 256 0
  transitions := [none]
  startState := ⟨0⟩
  terminals := [none]

private def minimalLalr : LalrTable where
  dimensions := ⟨1, 1, 1⟩
  startState := ⟨0⟩
  eofTerminal := ⟨0⟩
  actions := [.error]
  gotos := [none]
  productions := []
  ignoredTerminals := []

/-- The smallest useful structural example: one ordinary token, one EOS token,
one lexer state/class, and one parser state/terminal/nonterminal. -/
def minimalGrammar : NormalizedGrammar where
  tokens := minimalTokens
  lexer := minimalLexer
  lalr := minimalLalr

private theorem minimalTokens_wf : TokenTableWF minimalTokens := by
  constructor
  · simp [minimalTokens]
  · exact ⟨⟨0⟩, [byte0], rfl⟩
  · exact ⟨⟨1⟩, rfl⟩
  · rintro ⟨token⟩ bytes entry
    cases token with
    | zero =>
        simp [token?, minimalTokens, ordinaryToken] at entry
        subst bytes
        simp
    | succ token =>
        cases token with
        | zero => simp [token?, minimalTokens, ordinaryToken] at entry
        | succ token => simp [token?, minimalTokens, ordinaryToken] at entry

private theorem minimalLexer_wf : LexerWF minimalLexer 1 ⟨0⟩ := by
  constructor
  · change (List.replicate 256 0).length = 256
    exact List.length_replicate
  · rfl
  · rfl
  · decide
  · intro classId membership
    change classId ∈ List.replicate 256 0 at membership
    have : classId = 0 := List.eq_of_mem_replicate membership
    subst classId
    decide
  · intro cell membership destination cellEq
    change cell ∈ [none] at membership
    simp only [List.mem_singleton] at membership
    rw [membership] at cellEq
    simp at cellEq
  · intro cell membership terminal cellEq
    change cell ∈ [none] at membership
    simp only [List.mem_singleton] at membership
    rw [membership] at cellEq
    simp at cellEq
  · rfl
  · intro cell membership
    change cell ∈ [none] at membership
    simp only [List.mem_singleton] at membership
    subst cell
    simp

private theorem minimalLalr_wf : LalrWF minimalLalr := by
  constructor
  · rfl
  · rfl
  · decide
  · decide
  · simp [minimalLalr]
  · simp [minimalLalr]
  · intro state terminal accepts
    simp [minimalLalr, LalrTable.action?, Greatgramma.Spec.checkedFlatLookup,
      Greatgramma.Spec.flatLookup, Greatgramma.Spec.flatIndex] at accepts
    obtain ⟨stateZero, terminalZero, impossible⟩ := accepts
    rw [stateZero, terminalZero] at impossible
    simp at impossible
  · simp [minimalLalr]
  · simp [minimalLalr]
  · simp [minimalLalr]
  · simp [minimalLalr]

theorem minimalGrammar_structuralWF : StructuralWF minimalGrammar := by
  exact ⟨minimalTokens_wf, minimalLexer_wf, minimalLalr_wf⟩

/-! ### Token-table rejection witnesses -/

def missingOrdinaryGrammar : NormalizedGrammar :=
  { minimalGrammar with tokens := [.eos] }

theorem missingOrdinaryGrammar_not_structuralWF :
    ¬ StructuralWF missingOrdinaryGrammar := by
  intro wf
  obtain ⟨⟨token⟩, bytes, entry⟩ := wf.tokens.hasOrdinary
  cases token with
  | zero => simp [missingOrdinaryGrammar, token?, minimalGrammar] at entry
  | succ token => simp [missingOrdinaryGrammar, token?, minimalGrammar] at entry

def missingEosGrammar : NormalizedGrammar :=
  { minimalGrammar with tokens := [ordinaryToken] }

theorem missingEosGrammar_not_structuralWF :
    ¬ StructuralWF missingEosGrammar := by
  intro wf
  obtain ⟨⟨token⟩, entry⟩ := wf.tokens.hasEos
  cases token with
  | zero => simp [missingEosGrammar, token?, minimalGrammar, ordinaryToken] at entry
  | succ token => simp [missingEosGrammar, token?, minimalGrammar, ordinaryToken] at entry

def emptyOrdinaryBytesGrammar : NormalizedGrammar :=
  { minimalGrammar with tokens := [.bytes [], .eos] }

theorem emptyOrdinaryBytesGrammar_not_structuralWF :
    ¬ StructuralWF emptyOrdinaryBytesGrammar := by
  intro wf
  have := wf.tokens.ordinaryNonempty ⟨0⟩ [] (by rfl)
  exact this rfl

/-! ### Flattened-dimension and identifier rejection witnesses -/

def wrongLexerShapeGrammar : NormalizedGrammar :=
  { minimalGrammar with
      lexer := { minimalLexer with transitions := [] } }

theorem wrongLexerShapeGrammar_not_structuralWF :
    ¬ StructuralWF wrongLexerShapeGrammar := by
  intro wf
  have shape := wf.lexer.transitionShape
  change FlatShape [] 1 1 at shape
  simp [FlatShape] at shape

def outOfRangeLexerStartGrammar : NormalizedGrammar :=
  { minimalGrammar with
      lexer := { minimalLexer with startState := ⟨1⟩ } }

theorem outOfRangeLexerStartGrammar_not_structuralWF :
    ¬ StructuralWF outOfRangeLexerStartGrammar := by
  intro wf
  have inRange := wf.lexer.startInRange
  change 1 < 1 at inRange
  omega

def outOfRangeParserShiftGrammar : NormalizedGrammar :=
  { minimalGrammar with
      lalr := { minimalLalr with actions := [.shift ⟨1⟩] } }

theorem outOfRangeParserShiftGrammar_not_structuralWF :
    ¬ StructuralWF outOfRangeParserShiftGrammar := by
  intro wf
  have shift : (1 : Nat) < 1 := by
    apply wf.lalr.shiftsInRange (.shift ⟨1⟩)
    · change (.shift ⟨1⟩ : LalrAction) ∈ [.shift ⟨1⟩]
      simp
    · rfl
  omega

/-! ### Parser policy rejection witnesses -/

private def acceptOnNonEofLalr : LalrTable where
  dimensions := ⟨1, 2, 1⟩
  startState := ⟨0⟩
  eofTerminal := ⟨1⟩
  actions := [.accept, .error]
  gotos := [none]
  productions := []
  ignoredTerminals := []

def acceptOnNonEofGrammar : NormalizedGrammar :=
  { minimalGrammar with lalr := acceptOnNonEofLalr }

theorem acceptOnNonEofGrammar_not_structuralWF :
    ¬ StructuralWF acceptOnNonEofGrammar := by
  intro wf
  have equalEof := wf.lalr.acceptsOnlyEof ⟨0⟩ ⟨0⟩ (by
    simp [acceptOnNonEofGrammar, acceptOnNonEofLalr, LalrTable.action?,
      Greatgramma.Spec.checkedFlatLookup, Greatgramma.Spec.flatLookup,
      Greatgramma.Spec.flatIndex])
  cases equalEof

def ignoredEofGrammar : NormalizedGrammar :=
  { minimalGrammar with
      lalr := { minimalLalr with ignoredTerminals := [⟨0⟩] } }

theorem ignoredEofGrammar_not_structuralWF :
    ¬ StructuralWF ignoredEofGrammar := by
  intro wf
  exact wf.lalr.ignoredExcludeEof (by simp [ignoredEofGrammar, minimalGrammar,
    minimalLalr])

/-!
Duplicate EOS entries are deliberately valid.  Token identity remains an
index, and validation only requires at least one EOS entry.
-/
def duplicateEosGrammar : NormalizedGrammar :=
  { minimalGrammar with tokens := [ordinaryToken, .eos, .eos] }

theorem duplicateEosGrammar_structuralWF : StructuralWF duplicateEosGrammar := by
  refine ⟨?_, ?_, ?_⟩
  · constructor
    · simp [duplicateEosGrammar, minimalGrammar]
    · exact ⟨⟨0⟩, [byte0], rfl⟩
    · exact ⟨⟨1⟩, rfl⟩
    · intro token bytes entry
      rcases token with ⟨token⟩
      cases token with
      | zero =>
          simp [duplicateEosGrammar, minimalGrammar, ordinaryToken, token?] at entry
          subst bytes
          simp
      | succ token =>
          cases token with
          | zero =>
              simp [duplicateEosGrammar, minimalGrammar, ordinaryToken, token?] at entry
          | succ token =>
              cases token with
              | zero =>
                  simp [duplicateEosGrammar, minimalGrammar, ordinaryToken, token?] at entry
              | succ token =>
                  simp [duplicateEosGrammar, minimalGrammar, ordinaryToken, token?] at entry
  · simpa [duplicateEosGrammar] using minimalGrammar_structuralWF.lexer
  · simpa [duplicateEosGrammar] using minimalGrammar_structuralWF.lalr

/-! ### Generated validation and representation fixture

The fixture below uses the actual Aeneas types.  Its two-state DFA recognizes
one-byte lexemes: state `1` is accepting, and its missing outgoing transition
forces the next byte to be reconsumed from state `0`.  The parser has a distinct
terminal `1` for EOF.
-/

private def generatedTokenBytes : alloc.vec.Vec U8 :=
  ⟨[0#u8], by scalar_tac⟩

private def generatedTokens : alloc.vec.Vec normalized.TokenEntry :=
  ⟨[.Bytes generatedTokenBytes, .Eos], by scalar_tac⟩

private def generatedByteClasses : alloc.vec.Vec U32 :=
  ⟨List.replicate 256 0#u32, by
    rw [List.length_replicate]
    scalar_tac⟩

private def generatedTransitions :
    alloc.vec.Vec (Option GreatgrammaCore.ids.DfaStateId) :=
  ⟨[some 1#u32, none], by scalar_tac⟩

private def generatedTerminals :
    alloc.vec.Vec (Option GreatgrammaCore.ids.TerminalId) :=
  ⟨[none, some 0#u32], by scalar_tac⟩

private def generatedActions : alloc.vec.Vec normalized.Action :=
  ⟨[.Error, .Error], by scalar_tac⟩

private def generatedGotos :
    alloc.vec.Vec (Option GreatgrammaCore.ids.ParserStateId) :=
  ⟨[none], by scalar_tac⟩

private def generatedProductions : alloc.vec.Vec normalized.Production :=
  ⟨[], by scalar_tac⟩

private def generatedIgnored :
    alloc.vec.Vec GreatgrammaCore.ids.TerminalId :=
  ⟨[], by scalar_tac⟩

private def generatedIgnoredBitmap : alloc.vec.Vec U8 :=
  ⟨[0#u8, 0#u8], by scalar_tac⟩

/-- Small valid input to the generated Rust validation boundary. -/
def generatedRaw : normalized.UnvalidatedGrammar where
  tokens := generatedTokens
  lexer := {
    state_count := 2#u32
    class_count := 1#u32
    byte_classes := generatedByteClasses
    transitions := generatedTransitions
    start_state := 0#u32
    terminals := generatedTerminals
  }
  lalr := {
    dimensions := ⟨1#u32, 2#u32, 1#u32⟩
    start_state := 0#u32
    eof_terminal := 1#u32
    actions := generatedActions
    gotos := generatedGotos
    productions := generatedProductions
    ignored_terminals := generatedIgnored
  }

/-- Limits large enough for every declared size and validation-work charge in
the small generated fixture. -/
def generatedLimits : GreatgrammaCore.limits.ValidationLimits where
  max_tokens := 1000#u64
  max_token_bytes := 1000#u64
  max_dfa_states := 1000#u64
  max_dfa_classes := 1000#u64
  max_dfa_cells := 1000#u64
  max_parser_states := 1000#u64
  max_terminals := 1000#u64
  max_nonterminals := 1000#u64
  max_productions := 1000#u64
  max_parser_cells := 1000#u64
  max_logical_bytes := 2000#u64
  max_work := 1000#u64

/-- The representation that successful validation constructs from
`generatedRaw`; ignored terminal IDs have become a two-bit bitmap. -/
def generatedValidated : normalized.ValidatedGrammar where
  token_count := 2#u32
  tokens := generatedTokens
  lexer := {
    state_count := 2#u32
    class_count := 1#u32
    byte_classes := generatedByteClasses
    transitions := generatedTransitions
    start_state := 0#u32
    terminals := generatedTerminals
  }
  lalr := {
    dimensions := ⟨1#u32, 2#u32, 1#u32⟩
    start_state := 0#u32
    eof_terminal := 1#u32
    actions := generatedActions
    gotos := generatedGotos
    production_count := 0#u32
    productions := generatedProductions
    ignored_terminals := generatedIgnoredBitmap
  }

private theorem spec_result_eq {T : Type} {computation : Result T}
    {expected : T} (spec : computation ⦃ result => result = expected ⦄) :
    computation = .ok expected := by
  unfold WP.spec WP.theta WP.wp_return at spec
  cases computation <;> simp_all

private theorem spec_eq_extract {T : Type} {computation : Result T}
    {expected : T} {post : T → Prop}
    (resultEq : computation = .ok expected)
    (spec : computation ⦃ post ⦄) : post expected := by
  unfold WP.spec WP.theta WP.wp_return at spec
  rw [resultEq] at spec
  simpa using spec

private def generatedTokenState0 : validate.TokenValidation where
  has_ordinary := false
  has_eos := false
  total_bytes := 0#u64
  work := 273#u64

private def generatedTokenState1 : validate.TokenValidation where
  has_ordinary := true
  has_eos := false
  total_bytes := 1#u64
  work := 275#u64

private def generatedTokenState2 : validate.TokenValidation where
  has_ordinary := true
  has_eos := true
  total_bytes := 1#u64
  work := 276#u64

private theorem generatedBytesToken_validate :
    validate.validate_token_entry (.Bytes generatedTokenBytes) 0#usize
      generatedTokenState0 generatedLimits
      ⦃ output => output = (.Ok (), generatedTokenState1) ⦄ := by
  apply WP.spec_mono (validateBytesTokenEntry_success generatedTokenBytes
    0#usize generatedTokenState0 generatedLimits (by decide)
    (by simp [generatedTokenState0, generatedTokenBytes, generatedLimits])
    (by simp [generatedTokenState0, generatedTokenBytes, generatedLimits]))
  rintro ⟨result, state⟩ houtput
  rcases houtput with ⟨hresult, hordinary, heos, hbytes, hwork⟩
  apply Prod.ext
  · exact hresult
  · cases state with
    | mk ordinary eos totalBytes work =>
      change ordinary = true at hordinary
      change eos = false at heos
      subst ordinary
      subst eos
      have totalBytesEq : totalBytes = 1#u64 := by
        apply UScalar.eq_of_val_eq
        simpa [generatedTokenState0, generatedTokenBytes] using hbytes
      have workEq : work = 275#u64 := by
        apply UScalar.eq_of_val_eq
        simpa [generatedTokenState0, generatedTokenBytes] using hwork
      subst totalBytes
      subst work
      rfl

private theorem generatedEosToken_validate :
    validate.validate_token_entry .Eos 1#usize generatedTokenState1
      generatedLimits
      ⦃ output => output = (.Ok (), generatedTokenState2) ⦄ := by
  apply WP.spec_mono (validateEosTokenEntry_success 1#usize
    generatedTokenState1 generatedLimits (by
      simp [generatedTokenState1, generatedLimits]))
  rintro ⟨result, state⟩ houtput
  rcases houtput with ⟨hresult, hordinary, heos, hbytes, hwork⟩
  apply Prod.ext
  · exact hresult
  · cases state with
    | mk ordinary eos totalBytes work =>
      change ordinary = true at hordinary
      subst ordinary
      change eos = true at heos
      subst eos
      have totalBytesEq : totalBytes = 1#u64 := by
        simpa [generatedTokenState1] using hbytes
      have workEq : work = 276#u64 := by
        apply UScalar.eq_of_val_eq
        simpa [generatedTokenState1] using hwork
      subst totalBytes
      subst work
      rfl

private theorem generatedTokensLoop2_validate :
    validate.validate_tokens_loop generatedTokens.deref generatedLimits
      generatedTokenState2 none 2#usize
      ⦃ output => output = (generatedTokenState2, none) ⦄ := by
  rw [validate.validate_tokens_loop.eq_1]
  change (if 2 < 2 then _ else Result.ok (generatedTokenState2, none))
    ⦃ output => output = (generatedTokenState2, none) ⦄
  simp

private theorem generatedTokensLoop1_validate :
    validate.validate_tokens_loop generatedTokens.deref generatedLimits
      generatedTokenState1 none 1#usize
      ⦃ output => output = (generatedTokenState2, none) ⦄ := by
  rw [validate.validate_tokens_loop.eq_1]
  change (if 1 < 2 then _ else Result.ok (generatedTokenState1, none))
    ⦃ output => output = (generatedTokenState2, none) ⦄
  rw [if_pos (by omega)]
  have tokenAt1 : generatedTokens.deref.index_usize 1#usize =
      Result.ok .Eos := by rfl
  rw [tokenAt1]
  simp only [core.option.Option.is_none, bind_tc_ok]
  rw [spec_result_eq generatedEosToken_validate]
  simp [core.result.Result.err]
  step as ⟨indexNext⟩
  have indexNextEq : indexNext = 2#usize := by
    apply UScalar.eq_of_val_eq
    scalar_tac
  subst indexNext
  rw [spec_result_eq generatedTokensLoop2_validate]
  simp

private theorem generatedTokensLoop0_validate :
    validate.validate_tokens_loop generatedTokens.deref generatedLimits
      generatedTokenState0 none 0#usize
      ⦃ output => output = (generatedTokenState2, none) ⦄ := by
  rw [validate.validate_tokens_loop.eq_1]
  change (if 0 < 2 then _ else Result.ok (generatedTokenState0, none))
    ⦃ output => output = (generatedTokenState2, none) ⦄
  rw [if_pos (by omega)]
  have tokenAt0 : generatedTokens.deref.index_usize 0#usize =
      Result.ok (.Bytes generatedTokenBytes) := by rfl
  rw [tokenAt0]
  simp only [core.option.Option.is_none, bind_tc_ok]
  rw [spec_result_eq generatedBytesToken_validate]
  simp [core.result.Result.err]
  step as ⟨indexNext⟩
  have indexNextEq : indexNext = 1#usize := by
    apply UScalar.eq_of_val_eq
    scalar_tac
  subst indexNext
  rw [spec_result_eq generatedTokensLoop1_validate]
  simp

private theorem generatedTokens_validate :
    validate.validate_tokens generatedTokens.deref 273#u64 generatedLimits
      ⦃ result => result = .Ok 1#u64 ⦄ := by
  unfold validate.validate_tokens
  have state0Eq : ({
      has_ordinary := false
      has_eos := false
      total_bytes := 0#u64
      work := 273#u64
    } : validate.TokenValidation) = generatedTokenState0 := rfl
  rw [state0Eq]
  rw [spec_result_eq generatedTokensLoop0_validate]
  simp [generatedTokenState2]

private def generatedSizes : validate.DeclaredSizes where
  token_count := 2#u32
  token_bytes := 1#u64
  lexer_states := 2#u32
  lexer_cells := 2#u32
  parser_action_cells := 2#u32
  parser_goto_cells := 1#u32
  production_count := 0#u32
  ignored_terminal_count := 0#u32

private theorem generatedCheckedLen2_exact :
    validate.checked_len 2#usize .TokenCount = .ok (.Ok 2#u32) := by
  unfold validate.checked_len validate.index_as_u32
    core.convert.num.ptr_try_from_impls.TryFromU32Usize.try_from
    core.num.tryFromUScalar
  rw [if_pos (by scalar_tac)]
  simp [core.result.Result.map_err]
  apply UScalar.eq_of_val_eq
  scalar_tac

private theorem generatedCheckedLen0_exact
    (kind : error.ArithmeticKind) :
    validate.checked_len 0#usize kind = .ok (.Ok 0#u32) := by
  unfold validate.checked_len validate.index_as_u32
    core.convert.num.ptr_try_from_impls.TryFromU32Usize.try_from
    core.num.tryFromUScalar
  rw [if_pos (by scalar_tac)]
  simp [core.result.Result.map_err]
  apply UScalar.eq_of_val_eq
  scalar_tac

private theorem generatedCheckedProduct21_exact :
    validate.checked_product 2#u32 1#u32 .LexerCells =
      .ok (.Ok 2#u32) := by
  unfold validate.checked_product
  simp [lift, U32.checked_mul, core.num.checked_mul_UScalar,
    UScalar.mul, UScalar.tryMk, UScalar.tryMkOpt,
    UScalar.check_bounds, Option.ofResult, core.option.Option.ok_or]

private theorem generatedCheckedProduct12_exact :
    validate.checked_product 1#u32 2#u32 .ParserActionCells =
      .ok (.Ok 2#u32) := by
  unfold validate.checked_product
  simp [lift, U32.checked_mul, core.num.checked_mul_UScalar,
    UScalar.mul, UScalar.tryMk, UScalar.tryMkOpt,
    UScalar.check_bounds, Option.ofResult, core.option.Option.ok_or]

private theorem generatedCheckedProduct11_exact :
    validate.checked_product 1#u32 1#u32 .ParserGotoCells =
      .ok (.Ok 1#u32) := by
  unfold validate.checked_product
  simp [lift, U32.checked_mul, core.num.checked_mul_UScalar,
    UScalar.mul, UScalar.tryMk, UScalar.tryMkOpt,
    UScalar.check_bounds, Option.ofResult, core.option.Option.ok_or]

private def generatedBaseSizes : validate.DeclaredSizes where
  token_count := 0#u32
  token_bytes := 0#u64
  lexer_states := 2#u32
  lexer_cells := 2#u32
  parser_action_cells := 2#u32
  parser_goto_cells := 1#u32
  production_count := 0#u32
  ignored_terminal_count := 0#u32

private theorem generatedCheckedAdd_exact
    (left right expected : U64) (kind : error.ArithmeticKind)
    (hbound : left.val + right.val < 2 ^ 64)
    (hvalue : expected.val = left.val + right.val) :
    validate.checked_add left right kind = .ok (.Ok expected) := by
  have hadd : (left + right : Result U64) = .ok expected := by
    change UScalar.add left right = .ok expected
    unfold UScalar.add UScalar.tryMk UScalar.tryMkOpt
    rw [dif_pos (by simpa [UScalar.check_bounds] using hbound)]
    simp
    apply UScalar.eq_of_val_eq
    simpa using hvalue.symm
  unfold validate.checked_add U64.checked_add
    core.num.checked_add_UScalar
  rw [hadd]
  simp [lift, Option.ofResult, core.option.Option.ok_or]

private theorem generatedCheckedAdd08_exact :
    validate.checked_add 0#u64 8#u64 .Work =
      .ok (.Ok 8#u64) := by
  apply generatedCheckedAdd_exact <;> scalar_tac

private theorem generatedCheckedMul_exact
    (left right expected : U64) (kind : error.ArithmeticKind)
    (hbound : left.val * right.val < 2 ^ 64)
    (hvalue : expected.val = left.val * right.val) :
    validate.checked_mul left right kind = .ok (.Ok expected) := by
  have hmul : (left * right : Result U64) = .ok expected := by
    change UScalar.mul left right = .ok expected
    unfold UScalar.mul UScalar.tryMk UScalar.tryMkOpt
    rw [dif_pos (by simpa [UScalar.check_bounds] using hbound)]
    simp
    apply UScalar.eq_of_val_eq
    simpa using hvalue.symm
  unfold validate.checked_mul U64.checked_mul
    core.num.checked_mul_UScalar
  rw [show UScalar.mul left right = .ok expected by exact hmul]
  simp [lift, Option.ofResult, core.option.Option.ok_or]

private theorem generatedCast256_exact :
    lift (UScalar.cast .U64 validate.BYTE_CLASS_TABLE_LEN) =
      (.ok 256#u64 : Result U64) := by
  cases System.Platform.numBits_eq <;>
    simp [lift, validate.BYTE_CLASS_TABLE_LEN, UScalar.cast] <;>
    apply UScalar.eq_of_val_eq <;>
    simp_all [UScalar.val]

private theorem generatedFromU32_2_exact :
    lift (core.convert.num.FromU64U32.from 2#u32) =
      (.ok 2#u64 : Result U64) := by
  simp [lift, core.convert.num.FromU64U32.from]
  apply UScalar.eq_of_val_eq
  simp [UScalar.val]

private theorem generatedFromU32_1_exact :
    lift (core.convert.num.FromU64U32.from 1#u32) =
      (.ok 1#u64 : Result U64) := by
  simp [lift, core.convert.num.FromU64U32.from]
  apply UScalar.eq_of_val_eq
  simp [UScalar.val]

private theorem generatedFromU32_0_exact :
    lift (core.convert.num.FromU64U32.from 0#u32) =
      (.ok 0#u64 : Result U64) := by
  simp [lift, core.convert.num.FromU64U32.from]
  apply UScalar.eq_of_val_eq
  simp [UScalar.val]

private theorem generatedFromU32_3_value_exact :
    core.convert.num.FromU64U32.from 3#u32 = 3#u64 := by
  apply UScalar.eq_of_val_eq
  simp

private theorem generatedCheckLimit_exact
    (kind : error.LimitKind) (actual maximum : U64)
    (hbound : actual.val ≤ maximum.val) :
    validate.check_limit kind actual maximum = .ok (.Ok ()) := by
  apply spec_result_eq
  apply WP.spec_mono (checkLimit_ok_iff kind actual maximum)
  intro result hresult
  exact hresult.mpr hbound

private theorem generatedBaseWork_exact :
    validate.validation_base_work generatedBaseSizes =
      .ok (.Ok 273#u64) := by
  unfold validate.validation_base_work
  rw [show validate.LOGICAL_FIXED_SCALARS = 8#u64 by
    simp [validate.LOGICAL_FIXED_SCALARS]]
  rw [generatedCheckedAdd08_exact]
  simp [core.result.Result.Insts.CoreOpsTry.branch]
  rw [generatedCast256_exact]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 8#u64 256#u64 264#u64 .Work
    (by scalar_tac) (by scalar_tac)]
  simp [generatedBaseSizes]
  rw [generatedFromU32_2_exact]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 264#u64 2#u64 266#u64 .Work
    (by scalar_tac) (by scalar_tac)]
  simp
  rw [generatedCheckedAdd_exact 266#u64 2#u64 268#u64 .Work
    (by scalar_tac) (by scalar_tac)]
  simp
  rw [show validate.LOGICAL_ACTION_WORK = 2#u64 by
    simp [validate.LOGICAL_ACTION_WORK]]
  rw [generatedCheckedMul_exact 2#u64 2#u64 4#u64 .Work
    (by scalar_tac) (by scalar_tac)]
  simp
  rw [generatedCheckedAdd_exact 268#u64 4#u64 272#u64 .Work
    (by scalar_tac) (by scalar_tac)]
  simp
  rw [generatedFromU32_1_exact]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 272#u64 1#u64 273#u64 .Work
    (by scalar_tac) (by scalar_tac)]
  simp
  rw [generatedFromU32_0_exact]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 273#u64 0#u64 273#u64 .Work
    (by scalar_tac) (by scalar_tac)]
  simp
  exact generatedCheckedAdd_exact 273#u64 0#u64 273#u64 .Work
    (by scalar_tac) (by scalar_tac)

private theorem generatedLogicalBytes_exact :
    validate.logical_bytes generatedSizes = .ok (.Ok 1103#u64) := by
  unfold validate.logical_bytes
  simp only [generatedSizes]
  rw [generatedFromU32_2_exact]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 2#u64 1#u64 3#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok, core.result.Result.Insts.CoreOpsTry.branch]
  rw [show validate.LOGICAL_FIXED_SCALARS = 8#u64 by
    simp [validate.LOGICAL_FIXED_SCALARS]]
  rw [show validate.LOGICAL_ID_BYTES = 4#u64 by
    simp [validate.LOGICAL_ID_BYTES]]
  rw [generatedCheckedMul_exact 8#u64 4#u64 32#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 3#u64 32#u64 35#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCast256_exact]
  simp only [bind_tc_ok]
  rw [generatedCheckedMul_exact 256#u64 4#u64 1024#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 35#u64 1024#u64 1059#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedMul_exact 2#u64 4#u64 8#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 1059#u64 8#u64 1067#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 1067#u64 8#u64 1075#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [show validate.LOGICAL_ACTION_BYTES = 12#u64 by
    simp [validate.LOGICAL_ACTION_BYTES]]
  rw [generatedCheckedMul_exact 2#u64 12#u64 24#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 1075#u64 24#u64 1099#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedFromU32_1_exact]
  simp only [bind_tc_ok]
  rw [generatedCheckedMul_exact 1#u64 4#u64 4#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 1099#u64 4#u64 1103#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedFromU32_0_exact]
  simp only [bind_tc_ok]
  rw [generatedCheckedMul_exact 0#u64 validate.LOGICAL_PRODUCTION_BYTES
    0#u64 .LogicalBytes
    (by simp [validate.LOGICAL_PRODUCTION_BYTES])
    (by simp [validate.LOGICAL_PRODUCTION_BYTES])]
  simp only [bind_tc_ok]
  rw [generatedCheckedAdd_exact 1103#u64 0#u64 1103#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedMul_exact 0#u64 4#u64 0#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)]
  simp only [bind_tc_ok]
  exact generatedCheckedAdd_exact 1103#u64 0#u64 1103#u64 .LogicalBytes
    (by scalar_tac) (by scalar_tac)

private theorem generatedU32CheckedAdd21_exact :
    U32.checked_add 2#u32 1#u32 = some 3#u32 := by
  have hspec := U32.checked_add_bv_spec 2#u32 1#u32
  cases hresult : U32.checked_add 2#u32 1#u32 with
  | none =>
    rw [hresult] at hspec
    simp at hspec
    exfalso
    scalar_tac
  | some value =>
    rw [hresult] at hspec
    simp at hspec
    have valueEq : value = 3#u32 := by
      apply UScalar.eq_of_val_eq
      calc
        value.val = (2#u32).val + (1#u32).val := hspec.2.1
        _ = (3#u32).val := by scalar_tac
    subst value
    exact hresult

private theorem generatedDeclaredSizes_validate :
    validate.validate_declared_sizes generatedTokens.deref generatedRaw.lexer
      generatedRaw.lalr generatedLimits
      ⦃ result => result = .Ok generatedSizes ⦄ := by
  have tokensEq := spec_result_eq generatedTokens_validate
  unfold validate.validate_declared_sizes
  simp only [generatedRaw]
  rw [show core.slice.Slice.is_empty generatedTokens.deref = .ok false by rfl]
  simp only [bind_tc_ok, Bool.false_eq_true, if_false]
  rw [show generatedTokens.deref.len = 2#usize by rfl]
  rw [generatedCheckedLen2_exact]
  simp only [bind_tc_ok, core.result.Result.Insts.CoreOpsTry.branch]
  rw [generatedFromU32_2_exact]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_tokens = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .Tokens 2#u64 1000#u64 (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_dfa_states = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .DfaStates 2#u64 1000#u64 (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedFromU32_1_exact]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_dfa_classes = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .DfaClasses 1#u64 1000#u64 (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_parser_states = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .ParserStates 1#u64 1000#u64 (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_terminals = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .Terminals 2#u64 1000#u64 (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_nonterminals = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .Nonterminals 1#u64 1000#u64 (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [show alloc.vec.Vec.len generatedProductions = 0#usize by rfl]
  rw [generatedCheckedLen0_exact .ProductionCount]
  simp only [bind_tc_ok]
  rw [show alloc.vec.Vec.len generatedIgnored = 0#usize by rfl]
  rw [generatedCheckedLen0_exact .IgnoredTerminalCount]
  simp only [bind_tc_ok]
  rw [generatedFromU32_0_exact]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_productions = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .Productions 0#u64 1000#u64 (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedProduct21_exact]
  simp only [bind_tc_ok]
  rw [generatedFromU32_2_exact]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_dfa_cells = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .DfaCells 2#u64 1000#u64 (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckedProduct12_exact]
  simp only [bind_tc_ok]
  rw [generatedCheckedProduct11_exact]
  simp only [bind_tc_ok]
  rw [generatedU32CheckedAdd21_exact]
  simp [lift, core.option.Option.ok_or]
  rw [generatedFromU32_3_value_exact]
  rw [show generatedLimits.max_parser_cells = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .ParserCells 3#u64 1000#u64
    (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [show ({
      token_count := 0#u32
      token_bytes := 0#u64
      lexer_states := 2#u32
      lexer_cells := 2#u32
      parser_action_cells := 2#u32
      parser_goto_cells := 1#u32
      production_count := 0#u32
      ignored_terminal_count := 0#u32
    } : validate.DeclaredSizes) = generatedBaseSizes by rfl]
  rw [generatedBaseWork_exact]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_work = 1000#u64 by rfl]
  rw [generatedCheckLimit_exact .Work 273#u64 1000#u64
    (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [tokensEq]
  simp only [bind_tc_ok]
  rw [show ({
      token_count := 2#u32
      token_bytes := 1#u64
      lexer_states := 2#u32
      lexer_cells := 2#u32
      parser_action_cells := 2#u32
      parser_goto_cells := 1#u32
      production_count := 0#u32
      ignored_terminal_count := 0#u32
    } : validate.DeclaredSizes) = generatedSizes by rfl]
  rw [generatedLogicalBytes_exact]
  simp only [bind_tc_ok]
  rw [show generatedLimits.max_logical_bytes = 2000#u64 by rfl]
  rw [generatedCheckLimit_exact .LogicalBytes 1103#u64 2000#u64
    (by scalar_tac)]
  simp [generatedSizes]

private theorem generatedByteClasses_validate :
    validate.validate_byte_classes generatedByteClasses.deref 1#u32
      ⦃ result => result = .Ok () ⦄ := by
  apply validateByteClasses_success
  · change (List.replicate 256 0#u32).length ≤ 256
    rw [List.length_replicate]
  · intro position hposition
    change (List.replicate 256 0#u32)[position].val < 1
    have membership : (List.replicate 256 0#u32)[position] ∈
        List.replicate 256 0#u32 := List.getElem_mem hposition
    have equalZero := List.eq_of_mem_replicate membership
    rw [equalZero]
    decide

private theorem generatedTransitions_validate :
    validate.validate_transitions generatedTransitions.deref 2#u32
      ⦃ result => result = .Ok () ⦄ := by
  apply validateTransitions_success
  intro position hposition
  change position < 2 at hposition
  change OptionalIdInRange 2#u32 ([some 1#u32, none][position])
  have positionEq : position = 0 ∨ position = 1 := by omega
  rcases positionEq with rfl | rfl <;>
    simp [OptionalIdInRange]

private theorem generatedLexerTerminals_validate :
    validate.validate_lexer_terminals generatedTerminals.deref
      0#u32 2#u32 1#u32
      ⦃ result => result = .Ok () ⦄ := by
  apply validateLexerTerminals_success
  · change 2 ≤ U32.max
    scalar_tac
  · intro position hposition
    change position < 2 at hposition
    change LexerTerminalWF 2#u32 1#u32 ([none, some 0#u32][position]) ∧
      (position = 0 → [none, some 0#u32][position] = none)
    have positionEq : position = 0 ∨ position = 1 := by omega
    rcases positionEq with rfl | rfl <;>
      simp [LexerTerminalWF]

private theorem generatedActionsLoop2_validate :
    validate.validate_actions_loop generatedActions.deref 1#u32 0#u32
      1#u32 2#usize none 2#usize
      ⦃ result => result = none ⦄ := by
  rw [validate.validate_actions_loop.eq_1]
  change (if 2 < 2 then _ else Result.ok none)
    ⦃ result => result = none ⦄
  simp

private theorem generatedActionsLoop1_validate :
    validate.validate_actions_loop generatedActions.deref 1#u32 0#u32
      1#u32 2#usize none 1#usize
      ⦃ result => result = none ⦄ := by
  rw [validate.validate_actions_loop.eq_1]
  change (if 1 < 2 then _ else Result.ok none)
    ⦃ result => result = none ⦄
  rw [if_pos (by omega)]
  have actionAt1 : generatedActions.deref.index_usize 1#usize =
      Result.ok .Error := by rfl
  rw [actionAt1]
  simp [validate.validate_action, core.result.Result.err]
  step as ⟨indexNext⟩
  have indexNextEq : indexNext = 2#usize := by
    apply UScalar.eq_of_val_eq
    scalar_tac
  subst indexNext
  rw [spec_result_eq generatedActionsLoop2_validate]
  simp

private theorem generatedActionsLoop0_validate :
    validate.validate_actions_loop generatedActions.deref 1#u32 0#u32
      1#u32 2#usize none 0#usize
      ⦃ result => result = none ⦄ := by
  rw [validate.validate_actions_loop.eq_1]
  change (if 0 < 2 then _ else Result.ok none)
    ⦃ result => result = none ⦄
  rw [if_pos (by omega)]
  have actionAt0 : generatedActions.deref.index_usize 0#usize =
      Result.ok .Error := by rfl
  rw [actionAt0]
  simp [validate.validate_action, core.result.Result.err]
  step as ⟨indexNext⟩
  have indexNextEq : indexNext = 1#usize := by
    apply UScalar.eq_of_val_eq
    scalar_tac
  subst indexNext
  rw [spec_result_eq generatedActionsLoop1_validate]
  simp

private theorem generatedActions_validate :
    validate.validate_actions generatedActions.deref 1#u32 2#u32 0#u32
      1#u32 ⦃ result => result = .Ok () ⦄ := by
  unfold validate.validate_actions
  apply WP.spec_bind (countAsUsize_ok 2#u32 .ParserActionCells)
  intro result hresult
  obtain ⟨terminalWidth, resultEq, widthEq⟩ := hresult
  rw [resultEq]
  simp [core.result.Result.Insts.CoreOpsTry.branch]
  have terminalWidthEq : terminalWidth = 2#usize := by
    apply UScalar.eq_of_val_eq
    exact widthEq
  subst terminalWidth
  rw [spec_result_eq generatedActionsLoop0_validate]
  simp [validate.validation_result]

private theorem generatedIgnored_validate :
    validate.validate_ignored_terminals generatedIgnored.deref 2#u32 1#u32
      ⦃ result => result = .Ok () ⦄ := by
  unfold validate.validate_ignored_terminals
  rw [validate.validate_ignored_terminals_loop.eq_1]
  rw [if_neg (by simp [generatedIgnored, alloc.vec.Vec.deref])]
  simp [validate.validation_result]

private theorem generatedCount2_exact :
    validate.count_as_usize 2#u32 .IgnoredTerminalCount =
      .ok (.Ok 2#usize) := by
  unfold validate.count_as_usize
    Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from
    core.num.tryFromUScalar
  rw [if_pos (by scalar_tac)]
  simp [core.result.Result.map_err]
  apply UScalar.eq_of_val_eq
  scalar_tac

private theorem generatedIgnoredBitmap_exact :
    validate.build_ignored_terminal_table generatedIgnored.deref 2#u32
      ⦃ result => result = .Ok generatedIgnoredBitmap ⦄ := by
  unfold validate.build_ignored_terminal_table
  rw [generatedCount2_exact]
  simp [core.result.Result.Insts.CoreOpsTry.branch]
  unfold alloc.vec.Vec.try_reserve_exact alloc.vec.logical_reserve
  rw [if_pos (by scalar_tac)]
  simp [core.result.Result.map_err]
  unfold alloc.vec.Vec.resize
  simp [generatedIgnored, generatedIgnoredBitmap, alloc.vec.Vec.deref]
  rw [validate.build_ignored_terminal_table_loop.eq_1]
  simp [WP.spec, WP.theta, WP.wp_return]
  apply Subtype.ext
  rfl

private theorem generatedGotos_validate :
    validate.validate_gotos generatedGotos.deref 1#u32
      ⦃ result => result = .Ok () ⦄ := by
  apply validateGotos_success
  intro position hposition
  change position < 1 at hposition
  change OptionalIdInRange 1#u32 ([none][position])
  have : position = 0 := by omega
  subst position
  simp [OptionalIdInRange]

private theorem generatedProductions_validate :
    validate.validate_productions generatedProductions.deref 1#u32
      ⦃ result => result = .Ok () ⦄ := by
  apply validateProductions_success
  intro position hposition
  change position < 0 at hposition
  omega

private theorem generatedCheckLength_exact
    (table : error.ValidationTable) (expected actual : Usize)
    (hvalue : actual.val = expected.val) :
    validate.check_length table expected actual = .ok (.Ok ()) := by
  apply spec_result_eq
  apply WP.spec_mono (checkLength_ok_iff table expected actual)
  intro result hresult
  exact hresult.mpr hvalue

private theorem generatedCountAsUsize_exact
    (count : U32) (calculation : error.ArithmeticKind)
    (expected : Usize) (hvalue : expected.val = count.val) :
    validate.count_as_usize count calculation = .ok (.Ok expected) := by
  apply spec_result_eq
  apply WP.spec_mono (countAsUsize_ok count calculation)
  intro result hresult
  obtain ⟨index, resultEq, indexValue⟩ := hresult
  have indexEq : index = expected := by
    apply UScalar.eq_of_val_eq
    exact indexValue.trans hvalue.symm
  simp [resultEq, indexEq]

private theorem generatedValidateId_exact
    (table : error.ValidationTable) (index : Option Usize)
    (kind : error.IdKind) (id count : U32)
    (hinRange : id.val < count.val) :
    validate.validate_id table index kind id count = .ok (.Ok ()) := by
  apply spec_result_eq
  apply WP.spec_mono (validateId_ok_iff table index kind id count)
  intro result hresult
  exact hresult.mpr hinRange

private theorem generatedByteClassesLength_exact :
    alloc.vec.Vec.len generatedByteClasses = validate.BYTE_CLASS_TABLE_LEN := by
  apply UScalar.eq_of_val_eq
  change (List.replicate 256 0#u32).length =
    validate.BYTE_CLASS_TABLE_LEN.val
  rw [List.length_replicate]
  cases System.Platform.numBits_eq <;>
    simp_all [validate.BYTE_CLASS_TABLE_LEN, UScalar.val]

/-- The translated public validator returns exactly the expected validated
representation on the concrete generated fixture. -/
theorem generatedValidation_exact :
    generatedRaw.validate generatedLimits = .ok (.Ok generatedValidated) := by
  have hDeclaredEq := spec_result_eq generatedDeclaredSizes_validate
  have hIgnoredEq := spec_result_eq generatedIgnored_validate
  have hByteEq := spec_result_eq generatedByteClasses_validate
  have hTransitionsEq := spec_result_eq generatedTransitions_validate
  have hLexerTermsEq := spec_result_eq generatedLexerTerminals_validate
  have hActionsEq := spec_result_eq generatedActions_validate
  have hGotosEq := spec_result_eq generatedGotos_validate
  have hProductionsEq := spec_result_eq generatedProductions_validate
  have hBitmapEq := spec_result_eq generatedIgnoredBitmap_exact
  unfold normalized.UnvalidatedGrammar.validate validate.validate
  simp only
  rw [show generatedRaw.tokens.deref = generatedTokens.deref by rfl]
  rw [hDeclaredEq]
  simp only [bind_tc_ok,
    core.result.Result.Insts.CoreOpsTry.branch]
  simp only [generatedRaw, generatedSizes]
  rw [generatedByteClassesLength_exact]
  rw [generatedCheckLength_exact .LexerByteClasses
    validate.BYTE_CLASS_TABLE_LEN validate.BYTE_CLASS_TABLE_LEN rfl]
  simp only [bind_tc_ok]
  rw [generatedCountAsUsize_exact 2#u32 .LexerCells 2#usize
    (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckLength_exact .LexerTransitions 2#usize
    (alloc.vec.Vec.len generatedTransitions) (by rfl)]
  simp only [bind_tc_ok]
  rw [generatedCheckLength_exact .LexerTerminals 2#usize
    (alloc.vec.Vec.len generatedTerminals) (by rfl)]
  simp only [bind_tc_ok]
  rw [generatedCountAsUsize_exact 2#u32 .ParserActionCells 2#usize
    (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckLength_exact .ParserActions 2#usize
    (alloc.vec.Vec.len generatedActions) (by rfl)]
  simp only [bind_tc_ok]
  rw [generatedCountAsUsize_exact 1#u32 .ParserGotoCells 1#usize
    (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedCheckLength_exact .ParserGotos 1#usize
    (alloc.vec.Vec.len generatedGotos) (by rfl)]
  simp only [bind_tc_ok]
  simp only [ids.DfaStateId.get, ids.ParserStateId.get,
    ids.TerminalId.get, bind_tc_ok]
  rw [generatedValidateId_exact .LexerStart none .DfaState 0#u32 2#u32
    (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedValidateId_exact .ParserStart none .ParserState 0#u32 1#u32
    (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [generatedValidateId_exact .ParserEof none .Terminal 1#u32 2#u32
    (by scalar_tac)]
  simp only [bind_tc_ok]
  rw [hIgnoredEq, hByteEq, hTransitionsEq, hLexerTermsEq,
    hActionsEq, hGotosEq, hProductionsEq, hBitmapEq]
  simp [normalized.ValidatedLexer.from_unvalidated,
    normalized.ValidatedLalr.from_unvalidated,
    normalized.ValidatedGrammar.from_parts, generatedValidated]

/-- The actual Aeneas-generated top-level validation method satisfies its U3
success-boundary contract on the executable fixture. -/
theorem generatedValidationBoundary :
    generatedRaw.validate generatedLimits
      ⦃ result => match result with
        | .Ok validated =>
          ValidatedWF validated ∧
            ValidatedRepresents generatedRaw validated ∧
            StructuralWF (viewUnvalidated generatedRaw)
        | .Err _ => True ⦄ :=
  validationBoundary generatedRaw generatedLimits

private theorem generatedValidation_invariants :
    ValidatedWF generatedValidated ∧
      ValidatedRepresents generatedRaw generatedValidated ∧
      StructuralWF (viewUnvalidated generatedRaw) :=
  spec_eq_extract (expected := .Ok generatedValidated)
    generatedValidation_exact generatedValidationBoundary

theorem generatedValidated_wf : ValidatedWF generatedValidated :=
  generatedValidation_invariants.1

theorem generatedValidated_represents :
    ValidatedRepresents generatedRaw generatedValidated :=
  generatedValidation_invariants.2.1

/-- The generated invariants bridge all the way to the stable, implementation-
independent structural predicate. -/
theorem generatedRaw_structuralWF :
    StructuralWF (viewUnvalidated generatedRaw) :=
  generatedValidation_invariants.2.2

/-- The exact result also satisfies the generated weakest-precondition
contract used by the surrounding refinement examples. -/
theorem generatedValidation_succeeds :
    generatedRaw.validate generatedLimits
      ⦃ result => result = .Ok generatedValidated ⦄ := by
  rw [generatedValidation_exact]
  simp [WP.spec, WP.theta, WP.wp_return]

/-! ### Concrete generated rejection fixtures

These use the translated public `UnvalidatedGrammar.validate` entry point.
Each proof rules out the `.Ok` branch from a concrete malformed raw table;
the exact diagnostic remains an implementation detail.
-/

private def generatedEmptyBytes : alloc.vec.Vec U8 :=
  ⟨[], by scalar_tac⟩

private def generatedEmptyOrdinaryTokens : alloc.vec.Vec normalized.TokenEntry :=
  ⟨[.Bytes generatedEmptyBytes, .Eos], by scalar_tac⟩

def generatedEmptyOrdinaryRaw : normalized.UnvalidatedGrammar :=
  { generatedRaw with tokens := generatedEmptyOrdinaryTokens }

theorem generatedEmptyOrdinaryRaw_not_structuralWF :
    ¬ StructuralWF (viewUnvalidated generatedEmptyOrdinaryRaw) := by
  intro wf
  have nonempty := wf.tokens.ordinaryNonempty ⟨0⟩ [] (by rfl)
  exact nonempty rfl

theorem generatedEmptyOrdinaryRaw_rejects :
    generatedEmptyOrdinaryRaw.validate generatedLimits
      ⦃ result => match result with
        | .Ok _ => False
        | .Err _ => True ⦄ :=
  validationRejectsStructurallyInvalid generatedEmptyOrdinaryRaw
    generatedLimits generatedEmptyOrdinaryRaw_not_structuralWF

private def generatedMissingEosTokens : alloc.vec.Vec normalized.TokenEntry :=
  ⟨[.Bytes generatedTokenBytes], by scalar_tac⟩

def generatedMissingEosRaw : normalized.UnvalidatedGrammar :=
  { generatedRaw with tokens := generatedMissingEosTokens }

theorem generatedMissingEosRaw_not_structuralWF :
    ¬ StructuralWF (viewUnvalidated generatedMissingEosRaw) := by
  intro wf
  obtain ⟨⟨index⟩, entry⟩ := wf.tokens.hasEos
  cases index with
  | zero => simp [generatedMissingEosRaw, generatedMissingEosTokens,
      generatedTokenBytes, viewUnvalidated, viewTokenEntry, token?] at entry
  | succ index => simp [generatedMissingEosRaw, generatedMissingEosTokens,
      generatedTokenBytes, viewUnvalidated, viewTokenEntry, token?] at entry

theorem generatedMissingEosRaw_rejects :
    generatedMissingEosRaw.validate generatedLimits
      ⦃ result => match result with
        | .Ok _ => False
        | .Err _ => True ⦄ :=
  validationRejectsStructurallyInvalid generatedMissingEosRaw generatedLimits
    generatedMissingEosRaw_not_structuralWF

private def generatedNoTransitions :
    alloc.vec.Vec (Option GreatgrammaCore.ids.DfaStateId) :=
  ⟨[], by scalar_tac⟩

def generatedWrongLexerShapeRaw : normalized.UnvalidatedGrammar :=
  { generatedRaw with
      lexer := { generatedRaw.lexer with transitions := generatedNoTransitions } }

theorem generatedWrongLexerShapeRaw_not_structuralWF :
    ¬ StructuralWF (viewUnvalidated generatedWrongLexerShapeRaw) := by
  intro wf
  have shape := wf.lexer.transitionShape
  change FlatShape [] 2 1 at shape
  simp [FlatShape] at shape

theorem generatedWrongLexerShapeRaw_rejects :
    generatedWrongLexerShapeRaw.validate generatedLimits
      ⦃ result => match result with
        | .Ok _ => False
        | .Err _ => True ⦄ :=
  validationRejectsStructurallyInvalid generatedWrongLexerShapeRaw
    generatedLimits generatedWrongLexerShapeRaw_not_structuralWF

def generatedOutOfRangeLexerStartRaw : normalized.UnvalidatedGrammar :=
  { generatedRaw with
      lexer := { generatedRaw.lexer with start_state := 2#u32 } }

theorem generatedOutOfRangeLexerStartRaw_not_structuralWF :
    ¬ StructuralWF (viewUnvalidated generatedOutOfRangeLexerStartRaw) := by
  intro wf
  have inRange := wf.lexer.startInRange
  change 2 < 2 at inRange
  omega

theorem generatedOutOfRangeLexerStartRaw_rejects :
    generatedOutOfRangeLexerStartRaw.validate generatedLimits
      ⦃ result => match result with
        | .Ok _ => False
        | .Err _ => True ⦄ :=
  validationRejectsStructurallyInvalid generatedOutOfRangeLexerStartRaw
    generatedLimits generatedOutOfRangeLexerStartRaw_not_structuralWF

private def generatedAcceptOnNonEofActions : alloc.vec.Vec normalized.Action :=
  ⟨[.Accept, .Error], by scalar_tac⟩

def generatedAcceptOnNonEofRaw : normalized.UnvalidatedGrammar :=
  { generatedRaw with
      lalr := { generatedRaw.lalr with actions := generatedAcceptOnNonEofActions } }

theorem generatedAcceptOnNonEofRaw_not_structuralWF :
    ¬ StructuralWF (viewUnvalidated generatedAcceptOnNonEofRaw) := by
  intro wf
  have equalEof := wf.lalr.acceptsOnlyEof ⟨0⟩ ⟨0⟩ (by
    simp [generatedAcceptOnNonEofRaw, generatedAcceptOnNonEofActions,
      generatedRaw, viewUnvalidated, viewLalr, viewLalrDimensions,
      viewAction, LalrTable.action?,
      Greatgramma.Spec.checkedFlatLookup, Greatgramma.Spec.flatLookup,
      Greatgramma.Spec.flatIndex])
  cases equalEof

theorem generatedAcceptOnNonEofRaw_rejects :
    generatedAcceptOnNonEofRaw.validate generatedLimits
      ⦃ result => match result with
        | .Ok _ => False
        | .Err _ => True ⦄ :=
  validationRejectsStructurallyInvalid generatedAcceptOnNonEofRaw
    generatedLimits generatedAcceptOnNonEofRaw_not_structuralWF

private def generatedIgnoredEof :
    alloc.vec.Vec GreatgrammaCore.ids.TerminalId :=
  ⟨[1#u32], by scalar_tac⟩

def generatedIgnoredEofRaw : normalized.UnvalidatedGrammar :=
  { generatedRaw with
      lalr := { generatedRaw.lalr with ignored_terminals := generatedIgnoredEof } }

theorem generatedIgnoredEofRaw_not_structuralWF :
    ¬ StructuralWF (viewUnvalidated generatedIgnoredEofRaw) := by
  intro wf
  exact wf.lalr.ignoredExcludeEof (by
    simp [generatedIgnoredEofRaw, generatedIgnoredEof, viewUnvalidated,
      generatedRaw, viewLalr, viewTerminalId])

theorem generatedIgnoredEofRaw_rejects :
    generatedIgnoredEofRaw.validate generatedLimits
      ⦃ result => match result with
        | .Ok _ => False
        | .Err _ => True ⦄ :=
  validationRejectsStructurallyInvalid generatedIgnoredEofRaw generatedLimits
    generatedIgnoredEofRaw_not_structuralWF

/-- The generated token accessor and the stable normalized lookup select the
same entry. -/
theorem generatedTokenLookup_agrees :
    normalized.ValidatedGrammar.token generatedValidated 0#u32
      ⦃ result =>
        result.map viewTokenEntry =
          token? (viewUnvalidated generatedRaw).tokens
            (viewTokenId 0#u32) ⦄ := by
  apply WP.spec_mono (validatedGrammarToken_spec generatedValidated 0#u32)
  intro result hresult
  rw [hresult]
  rfl

private theorem checkedFlatIndex_zero :
    Greatgramma.Refinement.checkedFlatIndex 0#u32 1#u32 0#u32 =
      some 0#usize := by
  unfold Greatgramma.Refinement.checkedFlatIndex
  cases hmul : Usize.checked_mul (UScalar.cast .Usize (0#u32))
      (UScalar.cast .Usize (1#u32)) with
  | none =>
      have hspec := Usize.checked_mul_bv_spec
        (UScalar.cast .Usize (0#u32)) (UScalar.cast .Usize (1#u32))
      rw [hmul] at hspec
      scalar_tac
  | some product =>
      have hspec := Usize.checked_mul_bv_spec
        (UScalar.cast .Usize (0#u32)) (UScalar.cast .Usize (1#u32))
      rw [hmul] at hspec
      have hproduct : product = 0#usize := by
        apply UScalar.eq_of_val_eq
        scalar_tac
      subst product
      cases hadd : Usize.checked_add (0#usize)
          (UScalar.cast .Usize (0#u32)) with
      | none =>
          have hspec := Usize.checked_add_bv_spec
            (0#usize) (UScalar.cast .Usize (0#u32))
          rw [hadd] at hspec
          scalar_tac
      | some index =>
          have hspec := Usize.checked_add_bv_spec
            (0#usize) (UScalar.cast .Usize (0#u32))
          rw [hadd] at hspec
          have hindex : index = 0#usize := by
            apply UScalar.eq_of_val_eq
            scalar_tac
          subst index
          simp [hadd]

private theorem checkedFlatIndex_one :
    Greatgramma.Refinement.checkedFlatIndex 1#u32 1#u32 0#u32 =
      some 1#usize := by
  unfold Greatgramma.Refinement.checkedFlatIndex
  cases hmul : Usize.checked_mul (UScalar.cast .Usize (1#u32))
      (UScalar.cast .Usize (1#u32)) with
  | none =>
      have hspec := Usize.checked_mul_bv_spec
        (UScalar.cast .Usize (1#u32)) (UScalar.cast .Usize (1#u32))
      rw [hmul] at hspec
      scalar_tac
  | some product =>
      have hspec := Usize.checked_mul_bv_spec
        (UScalar.cast .Usize (1#u32)) (UScalar.cast .Usize (1#u32))
      rw [hmul] at hspec
      have hproduct : product = 1#usize := by
        apply UScalar.eq_of_val_eq
        scalar_tac
      subst product
      cases hadd : Usize.checked_add (1#usize)
          (UScalar.cast .Usize (0#u32)) with
      | none =>
          have hspec := Usize.checked_add_bv_spec
            (1#usize) (UScalar.cast .Usize (0#u32))
          rw [hadd] at hspec
          scalar_tac
      | some index =>
          have hspec := Usize.checked_add_bv_spec
            (1#usize) (UScalar.cast .Usize (0#u32))
          rw [hadd] at hspec
          have hindex : index = 1#usize := by
            apply UScalar.eq_of_val_eq
            scalar_tac
          subst index
          simp [hadd]

private theorem generatedLookup_start0 :
    lexerTransitionLookup generatedValidated.lexer 0#u32 0#u8 =
      some (some 1#u32) := by
  unfold lexerTransitionLookup
  simp only [generatedValidated]
  rw [if_pos (by scalar_tac)]
  rw [show generatedByteClasses.val = List.replicate 256 0#u32 by rfl,
    List.getElem?_replicate]
  rw [if_pos (by scalar_tac)]
  change Greatgramma.Refinement.checkedFlatLookup generatedTransitions.val
    0#u32 1#u32 0#u32 = some (some 1#u32)
  unfold Greatgramma.Refinement.checkedFlatLookup
  rw [checkedFlatIndex_zero]
  rw [show generatedTransitions.val = [some 1#u32, none] by rfl]
  rfl

private theorem generatedLookup_start1 :
    lexerTransitionLookup generatedValidated.lexer 0#u32 1#u8 =
      some (some 1#u32) := by
  unfold lexerTransitionLookup
  simp only [generatedValidated]
  rw [if_pos (by scalar_tac)]
  rw [show generatedByteClasses.val = List.replicate 256 0#u32 by rfl,
    List.getElem?_replicate]
  rw [if_pos (by scalar_tac)]
  change Greatgramma.Refinement.checkedFlatLookup generatedTransitions.val
    0#u32 1#u32 0#u32 = some (some 1#u32)
  unfold Greatgramma.Refinement.checkedFlatLookup
  rw [checkedFlatIndex_zero]
  rw [show generatedTransitions.val = [some 1#u32, none] by rfl]
  rfl

private theorem generatedLookup_boundary1 :
    lexerTransitionLookup generatedValidated.lexer 1#u32 1#u8 = some none := by
  unfold lexerTransitionLookup
  simp only [generatedValidated]
  rw [if_pos (by scalar_tac)]
  rw [show generatedByteClasses.val = List.replicate 256 0#u32 by rfl,
    List.getElem?_replicate]
  rw [if_pos (by scalar_tac)]
  change Greatgramma.Refinement.checkedFlatLookup generatedTransitions.val
    1#u32 1#u32 0#u32 = some none
  unfold Greatgramma.Refinement.checkedFlatLookup
  rw [checkedFlatIndex_one]
  rw [show generatedTransitions.val = [some 1#u32, none] by rfl]
  rfl

/-- The generated transition accessor agrees with the stable DFA table lookup
after applying the representation view. -/
theorem generatedTransitionLookup_agrees :
    normalized.ValidatedLexer.transition generatedValidated.lexer 0#u32 0#u8
      ⦃ result =>
        result.map (Option.map viewDfaStateId) =
          (viewUnvalidated generatedRaw).lexer.transition?
            (viewDfaStateId 0#u32) (viewByte 0#u8) ⦄ := by
  apply WP.spec_mono (validatedLexerTransition_spec
    generatedValidated.lexer 0#u32 0#u8 (by
      change 0 < (List.replicate 256 0#u32).length
      rw [List.length_replicate]
      omega))
  intro result hresult
  rw [hresult]
  rw [generatedLookup_start0]
  rfl

private theorem generatedTransition_start0 :
    normalized.ValidatedLexer.transition generatedValidated.lexer 0#u32 0#u8
      ⦃ result => result = some (some 1#u32) ⦄ := by
  apply WP.spec_mono (validatedLexerTransition_spec
    generatedValidated.lexer 0#u32 0#u8 (by
      change 0 < (List.replicate 256 0#u32).length
      rw [List.length_replicate]
      omega))
  intro result hresult
  rw [hresult]
  exact generatedLookup_start0

private theorem generatedTransition_start1 :
    normalized.ValidatedLexer.transition generatedValidated.lexer 0#u32 1#u8
      ⦃ result => result = some (some 1#u32) ⦄ := by
  apply WP.spec_mono (validatedLexerTransition_spec
    generatedValidated.lexer 0#u32 1#u8 (by
      change 1 < (List.replicate 256 0#u32).length
      rw [List.length_replicate]
      omega))
  intro result hresult
  rw [hresult]
  exact generatedLookup_start1

private theorem generatedTransition_boundary1 :
    normalized.ValidatedLexer.transition generatedValidated.lexer 1#u32 1#u8
      ⦃ result => result = some none ⦄ := by
  apply WP.spec_mono (validatedLexerTransition_spec
    generatedValidated.lexer 1#u32 1#u8 (by
      change 1 < (List.replicate 256 0#u32).length
      rw [List.length_replicate]
      omega))
  intro result hresult
  rw [hresult]
  exact generatedLookup_boundary1

private theorem generatedTerminal_accepting :
    normalized.ValidatedLexer.terminal generatedValidated.lexer 1#u32
      ⦃ result => result = some (some 0#u32) ⦄ := by
  apply WP.spec_mono (validatedLexerTerminal_spec generatedValidated.lexer 1#u32)
  intro result hresult
  rw [hresult]
  decide

/-- Executing the translated Rust step consumes the first byte silently and
lands in accepting DFA state `1`. -/
theorem generatedSilentByte_reachesAcceptingResidual :
    lexer.lexer_step generatedValidated .Start (.Byte 0#u8)
      ⦃ result => result = .Ok (.Continue (.Dfa 1#u32) none) ⦄ := by
  simp [lexer.lexer_step, normalized.ValidatedGrammar.impl.lexer,
    lexer.byte_step, lexer.begin_lexeme,
    normalized.ValidatedLexer.impl.start_state]
  apply WP.spec_bind generatedTransition_start0
  intro result hresult
  rw [hresult]
  simp

/-- From accepting state `1`, a missing transition emits terminal `0` and the
same boundary byte is reconsumed from the start state. -/
theorem generatedBoundaryByte_is_reconsumed :
    lexer.lexer_step generatedValidated (.Dfa 1#u32) (.Byte 1#u8)
      ⦃ result => result = .Ok (.Continue (.Dfa 1#u32) (some 0#u32)) ⦄ := by
  simp [lexer.lexer_step, normalized.ValidatedGrammar.impl.lexer,
    lexer.byte_step]
  apply WP.spec_bind generatedTransition_boundary1
  intro transitionResult htransition
  rw [htransition]
  simp [lexer.finish_or_reject_residual]
  apply WP.spec_bind generatedTerminal_accepting
  intro terminalResult hterminal
  rw [hterminal]
  simp [lexer.begin_lexeme, normalized.ValidatedLexer.impl.start_state]
  apply WP.spec_bind generatedTransition_start1
  intro startResult hstart
  rw [hstart]
  simp

/-- EOS flushes the accepting residual and then exposes parser EOF `1`. -/
theorem generatedAcceptingResidual_eosFlush :
    lexer.lexer_step generatedValidated (.Dfa 1#u32) .Eos
      ⦃ result => result = .Ok (.Finished (some 0#u32) 1#u32) ⦄ := by
  simp [lexer.lexer_step, lexer.eos_step,
    normalized.ValidatedGrammar.impl.lalr,
    normalized.ValidatedLalr.impl.eof_terminal,
    normalized.ValidatedGrammar.impl.lexer]
  apply WP.spec_bind generatedTerminal_accepting
  intro result hresult
  rw [hresult]
  simp [generatedValidated]

/-- Validation's generated ID check exercises both an accepted identifier and
the exact fail-closed diagnostic for the first out-of-range identifier. -/
theorem generatedValidateId_accepts :
    validate.validate_id .LexerStart none .DfaState 0#u32 2#u32
      ⦃ result => result = .Ok () ⦄ := by
  simp [validate.validate_id]

theorem generatedValidateId_rejects :
    validate.validate_id .LexerStart none .DfaState 2#u32 2#u32
      ⦃ result => result = .Err (.IdOutOfRange .LexerStart none
        .DfaState 2#u32 2#u32) ⦄ := by
  simp [validate.validate_id]

/-! ### Lexer boundary and EOS regression examples -/

private def boundaryLexer : LexerDfa :=
  (viewUnvalidated generatedRaw).lexer

private def boundaryGrammar : NormalizedGrammar :=
  viewUnvalidated generatedRaw

private def byte1 : Greatgramma.Spec.Byte := ⟨1, by decide⟩

/-- The second byte cannot extend state `1`; it emits terminal `0` and is
reconsumed from the start transition, leaving a fresh state-`1` residual. -/
theorem boundaryByte_is_reconsumed :
    boundaryLexer.scan .logicalStart [byte0, byte1] =
      some (.dfa ⟨1⟩, [⟨0⟩]) := by
  rfl

/-- One silent byte reaches an accepting residual, so terminal `0` is a real
first emission even though no terminal has yet been emitted. -/
theorem silentReachability_firstEmission :
    boundaryLexer.FirstEmission .logicalStart ⟨0⟩ := by
  left
  exact ⟨[byte0], .dfa ⟨1⟩, rfl, rfl⟩

/-- EOS flushes that residual terminal before the distinct parser EOF
terminal, matching `lexer_step`/`execute_eos` ordering. -/
theorem acceptingResidual_eosFlush :
    boundaryGrammar.eosTerminals? (.dfa ⟨1⟩) =
      some [⟨0⟩, ⟨1⟩] := by
  rfl

end Greatgramma.Examples.Validation
