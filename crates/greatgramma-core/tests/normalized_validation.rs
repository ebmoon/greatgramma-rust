use greatgramma_core::{
    Action, ArithmeticKind, DfaStateId, IdKind, LalrDimensions, LalrTable, LexerDfa, LimitKind,
    NonterminalId, ParserStateId, Production, ProductionId, TerminalId, TokenEntry, TokenId,
    UnvalidatedGrammar, ValidationError, ValidationLimits, ValidationTable,
};

fn byte_classes(class: u32) -> Vec<u32> {
    vec![class; 256]
}

fn valid_tokens() -> Vec<TokenEntry> {
    vec![
        TokenEntry::Bytes(b"a".to_vec()),
        TokenEntry::Bytes(b"a".to_vec()),
        TokenEntry::Eos,
        TokenEntry::Eos,
    ]
}

fn valid_lexer() -> LexerDfa {
    LexerDfa::new(
        2,
        1,
        byte_classes(0),
        vec![Some(DfaStateId::new(1)), None],
        DfaStateId::new(0),
        vec![None, Some(TerminalId::new(0))],
    )
}

fn valid_lalr() -> LalrTable {
    LalrTable::new(
        LalrDimensions::new(2, 2, 1),
        ParserStateId::new(0),
        TerminalId::new(1),
        vec![
            Action::Shift(ParserStateId::new(1)),
            Action::Error,
            Action::Error,
            Action::Accept,
        ],
        vec![None, None],
        vec![Production::new(NonterminalId::new(0), 1)],
    )
}

fn grammar_with(tokens: Vec<TokenEntry>, lexer: LexerDfa, lalr: LalrTable) -> UnvalidatedGrammar {
    UnvalidatedGrammar::new(tokens, lexer, lalr)
}

fn valid_grammar() -> UnvalidatedGrammar {
    grammar_with(valid_tokens(), valid_lexer(), valid_lalr())
}

#[test]
fn validates_owned_tables_and_exposes_only_checked_read_access() {
    let grammar = valid_grammar()
        .validate(ValidationLimits::default())
        .expect("minimal grammar should validate");

    assert_eq!(grammar.token_count(), 4);
    assert_eq!(
        grammar.token(TokenId::new(0)),
        Some(&TokenEntry::Bytes(b"a".to_vec()))
    );
    assert_eq!(
        grammar.token(TokenId::new(1)),
        Some(&TokenEntry::Bytes(b"a".to_vec()))
    );
    assert_eq!(grammar.token(TokenId::new(2)), Some(&TokenEntry::Eos));
    assert_eq!(grammar.token(TokenId::new(3)), Some(&TokenEntry::Eos));
    assert_eq!(grammar.token(TokenId::new(4)), None);

    let lexer = grammar.lexer();
    assert_eq!(lexer.state_count(), 2);
    assert_eq!(lexer.class_count(), 1);
    assert_eq!(lexer.start_state(), DfaStateId::new(0));
    assert_eq!(lexer.byte_class(b'a'), 0);
    assert_eq!(
        lexer.transition(DfaStateId::new(0), b'a'),
        Some(Some(DfaStateId::new(1)))
    );
    assert_eq!(lexer.transition(DfaStateId::new(1), b'a'), Some(None));
    assert_eq!(lexer.transition(DfaStateId::new(2), b'a'), None);
    assert_eq!(
        lexer.terminal(DfaStateId::new(1)),
        Some(Some(TerminalId::new(0)))
    );

    let lalr = grammar.lalr();
    assert_eq!(lalr.state_count(), 2);
    assert_eq!(lalr.terminal_count(), 2);
    assert_eq!(lalr.nonterminal_count(), 1);
    assert_eq!(lalr.production_count(), 1);
    assert_eq!(lalr.start_state(), ParserStateId::new(0));
    assert_eq!(lalr.eof_terminal(), TerminalId::new(1));
    assert_eq!(
        lalr.action(ParserStateId::new(0), TerminalId::new(0)),
        Some(Action::Shift(ParserStateId::new(1)))
    );
    assert_eq!(
        lalr.action(ParserStateId::new(1), TerminalId::new(1)),
        Some(Action::Accept)
    );
    assert_eq!(
        lalr.goto(ParserStateId::new(0), NonterminalId::new(0)),
        Some(None)
    );
    assert_eq!(
        lalr.production(ProductionId::new(0)),
        Some(Production::new(NonterminalId::new(0), 1))
    );
}

#[test]
fn rejects_missing_or_empty_token_kinds() {
    assert_eq!(
        grammar_with(Vec::new(), valid_lexer(), valid_lalr()).validate(ValidationLimits::default()),
        Err(ValidationError::EmptyTokenTable)
    );
    assert_eq!(
        grammar_with(vec![TokenEntry::Eos], valid_lexer(), valid_lalr())
            .validate(ValidationLimits::default()),
        Err(ValidationError::MissingOrdinaryToken)
    );
    assert_eq!(
        grammar_with(
            vec![TokenEntry::Bytes(b"a".to_vec())],
            valid_lexer(),
            valid_lalr(),
        )
        .validate(ValidationLimits::default()),
        Err(ValidationError::MissingEosToken)
    );
    assert_eq!(
        grammar_with(
            vec![TokenEntry::Bytes(Vec::new()), TokenEntry::Eos],
            valid_lexer(),
            valid_lalr(),
        )
        .validate(ValidationLimits::default()),
        Err(ValidationError::EmptyTokenBytes {
            token: TokenId::new(0)
        })
    );
}

#[test]
fn rejects_wrong_lexer_lengths_before_indexing() {
    let lexer = LexerDfa::new(
        2,
        1,
        byte_classes(0),
        vec![Some(DfaStateId::new(1))],
        DfaStateId::new(0),
        vec![None, Some(TerminalId::new(0))],
    );

    assert_eq!(
        grammar_with(valid_tokens(), lexer, valid_lalr()).validate(ValidationLimits::default()),
        Err(ValidationError::WrongTableLength {
            table: ValidationTable::LexerTransitions,
            expected: 2,
            actual: 1,
        })
    );
}

#[test]
fn rejects_out_of_range_lexer_classes_and_ids() {
    let bad_class = LexerDfa::new(
        2,
        1,
        byte_classes(1),
        vec![Some(DfaStateId::new(1)), None],
        DfaStateId::new(0),
        vec![None, Some(TerminalId::new(0))],
    );
    assert_eq!(
        grammar_with(valid_tokens(), bad_class, valid_lalr()).validate(ValidationLimits::default()),
        Err(ValidationError::ByteClassOutOfRange {
            byte: 0,
            class: 1,
            class_count: 1,
        })
    );

    let bad_transition = LexerDfa::new(
        2,
        1,
        byte_classes(0),
        vec![Some(DfaStateId::new(2)), None],
        DfaStateId::new(0),
        vec![None, Some(TerminalId::new(0))],
    );
    assert_eq!(
        grammar_with(valid_tokens(), bad_transition, valid_lalr())
            .validate(ValidationLimits::default()),
        Err(ValidationError::IdOutOfRange {
            table: ValidationTable::LexerTransitions,
            index: Some(0),
            kind: IdKind::DfaState,
            id: 2,
            count: 2,
        })
    );

    let bad_label = LexerDfa::new(
        2,
        1,
        byte_classes(0),
        vec![Some(DfaStateId::new(1)), None],
        DfaStateId::new(0),
        vec![None, Some(TerminalId::new(2))],
    );
    assert_eq!(
        grammar_with(valid_tokens(), bad_label, valid_lalr()).validate(ValidationLimits::default()),
        Err(ValidationError::IdOutOfRange {
            table: ValidationTable::LexerTerminals,
            index: Some(1),
            kind: IdKind::Terminal,
            id: 2,
            count: 2,
        })
    );
}

#[test]
fn rejects_wrong_parser_lengths_and_referenced_ids() {
    let short_actions = LalrTable::new(
        LalrDimensions::new(2, 2, 1),
        ParserStateId::new(0),
        TerminalId::new(1),
        vec![Action::Error],
        vec![None, None],
        vec![Production::new(NonterminalId::new(0), 1)],
    );
    assert_eq!(
        grammar_with(valid_tokens(), valid_lexer(), short_actions)
            .validate(ValidationLimits::default()),
        Err(ValidationError::WrongTableLength {
            table: ValidationTable::ParserActions,
            expected: 4,
            actual: 1,
        })
    );

    let bad_shift = LalrTable::new(
        LalrDimensions::new(2, 2, 1),
        ParserStateId::new(0),
        TerminalId::new(1),
        vec![
            Action::Shift(ParserStateId::new(2)),
            Action::Error,
            Action::Error,
            Action::Accept,
        ],
        vec![None, None],
        vec![Production::new(NonterminalId::new(0), 1)],
    );
    assert_eq!(
        grammar_with(valid_tokens(), valid_lexer(), bad_shift)
            .validate(ValidationLimits::default()),
        Err(ValidationError::IdOutOfRange {
            table: ValidationTable::ParserActions,
            index: Some(0),
            kind: IdKind::ParserState,
            id: 2,
            count: 2,
        })
    );

    let bad_reduce = LalrTable::new(
        LalrDimensions::new(2, 2, 1),
        ParserStateId::new(0),
        TerminalId::new(1),
        vec![
            Action::Reduce(ProductionId::new(1)),
            Action::Error,
            Action::Error,
            Action::Accept,
        ],
        vec![None, None],
        vec![Production::new(NonterminalId::new(0), 1)],
    );
    assert_eq!(
        grammar_with(valid_tokens(), valid_lexer(), bad_reduce)
            .validate(ValidationLimits::default()),
        Err(ValidationError::IdOutOfRange {
            table: ValidationTable::ParserActions,
            index: Some(0),
            kind: IdKind::Production,
            id: 1,
            count: 1,
        })
    );
}

#[test]
fn rejects_parser_gotos_productions_and_accept_outside_eof() {
    let bad_goto = LalrTable::new(
        LalrDimensions::new(2, 2, 1),
        ParserStateId::new(0),
        TerminalId::new(1),
        vec![
            Action::Shift(ParserStateId::new(1)),
            Action::Error,
            Action::Error,
            Action::Accept,
        ],
        vec![Some(ParserStateId::new(2)), None],
        vec![Production::new(NonterminalId::new(0), 1)],
    );
    assert_eq!(
        grammar_with(valid_tokens(), valid_lexer(), bad_goto).validate(ValidationLimits::default()),
        Err(ValidationError::IdOutOfRange {
            table: ValidationTable::ParserGotos,
            index: Some(0),
            kind: IdKind::ParserState,
            id: 2,
            count: 2,
        })
    );

    let bad_lhs = LalrTable::new(
        LalrDimensions::new(2, 2, 1),
        ParserStateId::new(0),
        TerminalId::new(1),
        vec![
            Action::Shift(ParserStateId::new(1)),
            Action::Error,
            Action::Error,
            Action::Accept,
        ],
        vec![None, None],
        vec![Production::new(NonterminalId::new(1), 1)],
    );
    assert_eq!(
        grammar_with(valid_tokens(), valid_lexer(), bad_lhs).validate(ValidationLimits::default()),
        Err(ValidationError::IdOutOfRange {
            table: ValidationTable::Productions,
            index: Some(0),
            kind: IdKind::Nonterminal,
            id: 1,
            count: 1,
        })
    );

    let early_accept = LalrTable::new(
        LalrDimensions::new(2, 2, 1),
        ParserStateId::new(0),
        TerminalId::new(1),
        vec![Action::Accept, Action::Error, Action::Error, Action::Accept],
        vec![None, None],
        vec![Production::new(NonterminalId::new(0), 1)],
    );
    assert_eq!(
        grammar_with(valid_tokens(), valid_lexer(), early_accept)
            .validate(ValidationLimits::default()),
        Err(ValidationError::AcceptOnNonEof {
            state: ParserStateId::new(0),
            terminal: TerminalId::new(0),
        })
    );
}

#[test]
fn rejects_limits_and_checked_cross_product_overflow() {
    let limits = ValidationLimits {
        max_dfa_states: 1,
        ..ValidationLimits::default()
    };
    assert_eq!(
        valid_grammar().validate(limits),
        Err(ValidationError::LimitExceeded {
            limit: LimitKind::DfaStates,
            actual: 2,
            maximum: 1,
        })
    );

    let overflowing_lexer = LexerDfa::new(
        65_536,
        65_536,
        byte_classes(0),
        Vec::new(),
        DfaStateId::new(0),
        Vec::new(),
    );
    let permissive_limits = ValidationLimits {
        max_dfa_states: u64::MAX,
        max_dfa_classes: u64::MAX,
        max_dfa_cells: u64::MAX,
        max_logical_bytes: u64::MAX,
        max_work: u64::MAX,
        ..ValidationLimits::default()
    };
    assert_eq!(
        grammar_with(valid_tokens(), overflowing_lexer, valid_lalr()).validate(permissive_limits),
        Err(ValidationError::ArithmeticOverflow {
            calculation: ArithmeticKind::LexerCells,
        })
    );
}

#[test]
fn token_limits_stop_before_later_invalid_entries() {
    // Fixed table validation costs 275 work units. The first token's entry and
    // byte use the final two; charging the next entry must fail before reading
    // its empty byte payload.
    let work_limits = ValidationLimits {
        max_work: 277,
        ..ValidationLimits::default()
    };
    assert_eq!(
        grammar_with(
            vec![
                TokenEntry::Bytes(b"a".to_vec()),
                TokenEntry::Bytes(Vec::new()),
                TokenEntry::Eos,
            ],
            valid_lexer(),
            valid_lalr(),
        )
        .validate(work_limits),
        Err(ValidationError::LimitExceeded {
            limit: LimitKind::Work,
            actual: 278,
            maximum: 277,
        })
    );

    // The first byte exhausts its own budget. The second valid byte must trip
    // that limit before validation reaches the later empty entry.
    let byte_limits = ValidationLimits {
        max_token_bytes: 1,
        ..ValidationLimits::default()
    };
    assert_eq!(
        grammar_with(
            vec![
                TokenEntry::Bytes(b"a".to_vec()),
                TokenEntry::Bytes(b"b".to_vec()),
                TokenEntry::Bytes(Vec::new()),
                TokenEntry::Eos,
            ],
            valid_lexer(),
            valid_lalr(),
        )
        .validate(byte_limits),
        Err(ValidationError::LimitExceeded {
            limit: LimitKind::TokenBytes,
            actual: 2,
            maximum: 1,
        })
    );
}
