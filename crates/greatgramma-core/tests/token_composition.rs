use greatgramma_core::{
    Action, DfaStateId, LalrDimensions, LalrTable, LexerDfa, LexerState, ParserStateId, TerminalId,
    TokenEntry, TokenExecution, TokenId, TokenStepError, UnvalidatedGrammar, ValidationLimits,
    execute_token,
};

const A: TerminalId = TerminalId::new(0);
const C: TerminalId = TerminalId::new(1);
const NUL: TerminalId = TerminalId::new(2);
const HIGH: TerminalId = TerminalId::new(3);
const EOF: TerminalId = TerminalId::new(4);

fn byte_classes() -> Vec<u32> {
    let mut classes = vec![0; 256];
    classes[usize::from(b'a')] = 1;
    classes[usize::from(b'b')] = 2;
    classes[usize::from(b'c')] = 3;
    classes[usize::from(b'd')] = 4;
    classes[0] = 5;
    classes[usize::from(0xff_u8)] = 6;
    classes
}

fn transition(
    table: &mut [Option<DfaStateId>],
    class_count: usize,
    source: usize,
    class: usize,
    destination: u32,
) {
    table[source * class_count + class] = Some(DfaStateId::new(destination));
}

fn grammar() -> greatgramma_core::ValidatedGrammar {
    let class_count = 7;
    let mut transitions = vec![None; 6 * class_count];
    transition(&mut transitions, class_count, 0, 1, 1); // a
    transition(&mut transitions, class_count, 1, 2, 0); // ab returns to DFA start
    transition(&mut transitions, class_count, 0, 3, 2); // c
    transition(&mut transitions, class_count, 0, 4, 5); // d leaves an unfinished residual
    transition(&mut transitions, class_count, 0, 5, 3); // NUL
    transition(&mut transitions, class_count, 0, 6, 4); // 0xff

    let lexer = LexerDfa::new(
        6,
        u32::try_from(class_count).expect("fixture class count fits"),
        byte_classes(),
        transitions,
        DfaStateId::new(0),
        vec![None, Some(A), Some(C), Some(NUL), Some(HIGH), None],
    );
    let lalr = LalrTable::new(
        LalrDimensions::new(1, 5, 0),
        ParserStateId::new(0),
        EOF,
        vec![
            Action::Error,
            Action::Error,
            Action::Error,
            Action::Error,
            Action::Accept,
        ],
        Vec::new(),
        Vec::new(),
    );
    UnvalidatedGrammar::new(
        vec![
            TokenEntry::Bytes(b"a".to_vec()),
            TokenEntry::Bytes(b"ac".to_vec()),
            TokenEntry::Bytes(b"acac".to_vec()),
            TokenEntry::Bytes(b"ac".to_vec()),
            TokenEntry::Bytes(vec![0, 0xff]),
            TokenEntry::Bytes(b"d".to_vec()),
            TokenEntry::Bytes(b"x".to_vec()),
            TokenEntry::Eos,
            TokenEntry::Eos,
        ],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("fixture validates")
}

#[test]
fn composes_normalized_token_bytes_without_committing_a_continuation() {
    let grammar = grammar();

    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(0)),
        Ok(Some(TokenExecution::Continue {
            state: LexerState::Dfa(DfaStateId::new(1)),
            emitted: Vec::new(),
        }))
    );
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(1)),
        Ok(Some(TokenExecution::Continue {
            state: LexerState::Dfa(DfaStateId::new(2)),
            emitted: vec![A],
        }))
    );
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(2)),
        Ok(Some(TokenExecution::Continue {
            state: LexerState::Dfa(DfaStateId::new(2)),
            emitted: vec![A, C, A],
        }))
    );
}

#[test]
fn preserves_duplicate_bytes_binary_tokens_and_ordinary_rejection() {
    let grammar = grammar();
    let expected = Ok(Some(TokenExecution::Continue {
        state: LexerState::Dfa(DfaStateId::new(2)),
        emitted: vec![A],
    }));
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(1)),
        expected
    );
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(3)),
        expected
    );
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(4)),
        Ok(Some(TokenExecution::Continue {
            state: LexerState::Dfa(DfaStateId::new(4)),
            emitted: vec![NUL],
        }))
    );
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(6)),
        Ok(None)
    );
}

#[test]
fn keeps_logical_start_distinct_and_finishes_every_eos_alias() {
    let grammar = grammar();
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(7)),
        Ok(Some(TokenExecution::Finished {
            emitted: Vec::new(),
            eof: EOF,
        }))
    );
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(8)),
        Ok(Some(TokenExecution::Finished {
            emitted: Vec::new(),
            eof: EOF,
        }))
    );
    assert_eq!(
        execute_token(
            &grammar,
            LexerState::Dfa(DfaStateId::new(1)),
            TokenId::new(7),
        ),
        Ok(Some(TokenExecution::Finished {
            emitted: vec![A],
            eof: EOF,
        }))
    );
    assert_eq!(
        execute_token(
            &grammar,
            LexerState::Dfa(DfaStateId::new(0)),
            TokenId::new(7),
        ),
        Ok(None)
    );
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(5)),
        Ok(Some(TokenExecution::Continue {
            state: LexerState::Dfa(DfaStateId::new(5)),
            emitted: Vec::new(),
        }))
    );
    assert_eq!(
        execute_token(
            &grammar,
            LexerState::Dfa(DfaStateId::new(5)),
            TokenId::new(7),
        ),
        Ok(None)
    );
}

#[test]
fn reports_invalid_token_and_source_without_treating_them_as_rejection() {
    let grammar = grammar();
    assert_eq!(
        execute_token(&grammar, LexerState::Start, TokenId::new(99)),
        Err(TokenStepError::InvalidToken {
            token: TokenId::new(99),
        })
    );
    assert_eq!(
        execute_token(
            &grammar,
            LexerState::Dfa(DfaStateId::new(99)),
            TokenId::new(0),
        ),
        Err(TokenStepError::InvalidState {
            state: DfaStateId::new(99),
        })
    );
}
