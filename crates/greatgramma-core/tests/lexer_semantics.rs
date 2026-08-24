use greatgramma_core::{
    Action, DfaStateId, LalrDimensions, LalrTable, LexerDfa, LexerError, LexerInput, LexerState,
    LexerStep, ParserStateId, TerminalId, TokenEntry, UnvalidatedGrammar, ValidatedGrammar,
    ValidationLimits, lexer_step,
};

const LEXEME_A: TerminalId = TerminalId::new(0);
const LEXEME_C: TerminalId = TerminalId::new(1);
const EOF: TerminalId = TerminalId::new(2);

fn byte_classes() -> Vec<u32> {
    let mut classes = vec![0; 256];
    classes[usize::from(b'a')] = 1;
    classes[usize::from(b'b')] = 2;
    classes[usize::from(b'c')] = 3;
    classes[usize::from(b'd')] = 4;
    classes[usize::from(b'e')] = 5;
    classes
}

fn set_transition(
    transitions: &mut [Option<DfaStateId>],
    class_count: usize,
    source: u32,
    class: usize,
    destination: u32,
) {
    let source = usize::try_from(source).expect("test state fits usize");
    transitions[source * class_count + class] = Some(DfaStateId::new(destination));
}

fn validated_grammar() -> ValidatedGrammar {
    let class_count = 6;
    let mut transitions = vec![None; 6 * class_count];

    // `(ab)*a`: after `ab`, execution is `Dfa(start)`, not logical `Start`.
    set_transition(&mut transitions, class_count, 0, 1, 1);
    set_transition(&mut transitions, class_count, 1, 2, 0);

    // `c` and `cc` end in successive accepting states with different,
    // already-priority-resolved terminal labels.
    set_transition(&mut transitions, class_count, 0, 3, 2);
    set_transition(&mut transitions, class_count, 2, 3, 3);

    // `de` provides a distinct unfinished residual after `d`.
    set_transition(&mut transitions, class_count, 0, 4, 4);
    set_transition(&mut transitions, class_count, 4, 5, 5);

    let lexer = LexerDfa::new(
        6,
        u32::try_from(class_count).expect("test class count fits u32"),
        byte_classes(),
        transitions,
        DfaStateId::new(0),
        vec![
            None,
            Some(LEXEME_A),
            Some(LEXEME_C),
            Some(LEXEME_A),
            None,
            Some(LEXEME_C),
        ],
    );
    let lalr = LalrTable::new(
        LalrDimensions::new(1, 3, 0),
        ParserStateId::new(0),
        EOF,
        vec![Action::Error, Action::Error, Action::Accept],
        Vec::new(),
        Vec::new(),
    );

    UnvalidatedGrammar::new(
        vec![
            TokenEntry::Bytes(b"a".to_vec()),
            TokenEntry::Bytes(b"c".to_vec()),
            TokenEntry::Eos,
            TokenEntry::Eos,
        ],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("lexer fixture should validate")
}

#[test]
fn eos_distinguishes_clean_accepted_and_unfinished_states() {
    let grammar = validated_grammar();

    assert_eq!(
        lexer_step(&grammar, LexerState::Start, LexerInput::Eos),
        Ok(LexerStep::Finished {
            terminal: None,
            eof: EOF,
        })
    );
    assert_eq!(
        lexer_step(
            &grammar,
            LexerState::Dfa(DfaStateId::new(1)),
            LexerInput::Eos,
        ),
        Ok(LexerStep::Finished {
            terminal: Some(LEXEME_A),
            eof: EOF,
        })
    );
    assert_eq!(
        lexer_step(
            &grammar,
            LexerState::Dfa(DfaStateId::new(4)),
            LexerInput::Eos,
        ),
        Err(LexerError::EosAfterUnfinishedResidual {
            state: DfaStateId::new(4),
        })
    );
}

#[test]
fn dfa_start_after_ab_is_not_logical_start() {
    let grammar = validated_grammar();

    let after_a = lexer_step(&grammar, LexerState::Start, LexerInput::Byte(b'a'));
    assert_eq!(
        after_a,
        Ok(LexerStep::Continue {
            state: LexerState::Dfa(DfaStateId::new(1)),
            emitted: None,
        })
    );
    let after_ab = lexer_step(
        &grammar,
        LexerState::Dfa(DfaStateId::new(1)),
        LexerInput::Byte(b'b'),
    );
    assert_eq!(
        after_ab,
        Ok(LexerStep::Continue {
            state: LexerState::Dfa(DfaStateId::new(0)),
            emitted: None,
        })
    );
    assert_eq!(
        lexer_step(
            &grammar,
            LexerState::Dfa(DfaStateId::new(0)),
            LexerInput::Eos,
        ),
        Err(LexerError::EosAfterUnfinishedResidual {
            state: DfaStateId::new(0),
        })
    );
}

#[test]
fn accepting_boundary_emits_then_reconsumes_the_same_byte() {
    let grammar = validated_grammar();

    assert_eq!(
        lexer_step(
            &grammar,
            LexerState::Dfa(DfaStateId::new(1)),
            LexerInput::Byte(b'c'),
        ),
        Ok(LexerStep::Continue {
            state: LexerState::Dfa(DfaStateId::new(2)),
            emitted: Some(LEXEME_A),
        })
    );
}

#[test]
fn accepting_boundary_rolls_back_when_reconsumption_fails() {
    let grammar = validated_grammar();

    assert_eq!(
        lexer_step(
            &grammar,
            LexerState::Dfa(DfaStateId::new(1)),
            LexerInput::Byte(b'x'),
        ),
        Err(LexerError::CannotBeginLexeme { byte: b'x' })
    );
    assert_eq!(
        lexer_step(&grammar, LexerState::Start, LexerInput::Byte(b'x')),
        Err(LexerError::CannotBeginLexeme { byte: b'x' })
    );
}

#[test]
fn nonaccepting_boundary_rejects_as_unfinished() {
    let grammar = validated_grammar();

    assert_eq!(
        lexer_step(
            &grammar,
            LexerState::Dfa(DfaStateId::new(4)),
            LexerInput::Byte(b'x'),
        ),
        Err(LexerError::ByteAfterUnfinishedResidual {
            state: DfaStateId::new(4),
            byte: b'x',
        })
    );
}

#[test]
fn transitions_through_successive_accepting_states_before_emitting() {
    let grammar = validated_grammar();

    assert_eq!(
        lexer_step(&grammar, LexerState::Start, LexerInput::Byte(b'c')),
        Ok(LexerStep::Continue {
            state: LexerState::Dfa(DfaStateId::new(2)),
            emitted: None,
        })
    );
    assert_eq!(
        lexer_step(
            &grammar,
            LexerState::Dfa(DfaStateId::new(2)),
            LexerInput::Eos,
        ),
        Ok(LexerStep::Finished {
            terminal: Some(LEXEME_C),
            eof: EOF,
        })
    );
    assert_eq!(
        lexer_step(
            &grammar,
            LexerState::Dfa(DfaStateId::new(2)),
            LexerInput::Byte(b'c'),
        ),
        Ok(LexerStep::Continue {
            state: LexerState::Dfa(DfaStateId::new(3)),
            emitted: None,
        })
    );
    assert_eq!(
        lexer_step(
            &grammar,
            LexerState::Dfa(DfaStateId::new(3)),
            LexerInput::Eos,
        ),
        Ok(LexerStep::Finished {
            terminal: Some(LEXEME_A),
            eof: EOF,
        })
    );
}

#[test]
fn invalid_public_dfa_state_is_a_structured_error() {
    let grammar = validated_grammar();
    let invalid = DfaStateId::new(99);
    let byte_result = lexer_step(&grammar, LexerState::Dfa(invalid), LexerInput::Byte(b'a'));
    let eos_result = lexer_step(&grammar, LexerState::Dfa(invalid), LexerInput::Eos);

    assert_eq!(
        byte_result,
        Err(LexerError::InvalidState { state: invalid })
    );
    assert_eq!(eos_result, Err(LexerError::InvalidState { state: invalid }));
}
