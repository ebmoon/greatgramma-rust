use greatgramma_core::{
    Action, DfaStateId, LalrDimensions, LalrTable, LexerDfa, NonterminalId, ParserError,
    ParserStateId, PreparationError, PreparationLimits, Production, ProductionId,
    SequenceClassification, SequenceId, TerminalId, TokenEntry, UnvalidatedGrammar,
    ValidationLimits, classify_sequence_head, execute_sequence_head, prepare_parser,
    prepare_spanner,
};

const ITEM: TerminalId = TerminalId::new(0);
const CLOSE: TerminalId = TerminalId::new(1);
const EOF: TerminalId = TerminalId::new(2);
const INNER: NonterminalId = NonterminalId::new(0);
const WRAPPED: NonterminalId = NonterminalId::new(1);

fn state(value: u32) -> ParserStateId {
    ParserStateId::new(value)
}

fn index(value: u32) -> usize {
    usize::try_from(value).expect("fixture index fits")
}

fn reduce(production: u32, rank: u32) -> Action {
    Action::Reduce {
        production: ProductionId::new(production),
        rank,
    }
}

fn lexer() -> LexerDfa {
    let mut byte_classes = vec![0; 256];
    byte_classes[usize::from(b'c')] = 1;
    LexerDfa::new(
        3,
        2,
        byte_classes,
        vec![
            Some(DfaStateId::new(1)),
            Some(DfaStateId::new(2)),
            None,
            None,
            None,
            None,
        ],
        DfaStateId::new(0),
        vec![None, Some(ITEM), Some(CLOSE)],
    )
}

fn grammar() -> greatgramma_core::ValidatedGrammar {
    let dimensions = LalrDimensions::new(5, 3, 2);
    let mut actions = vec![Action::Error; 15];
    actions[index(ITEM.get())] = Action::Shift(state(1));
    actions[3 + index(CLOSE.get())] = reduce(0, 2);
    actions[6 + index(CLOSE.get())] = reduce(1, 1);
    actions[9 + index(CLOSE.get())] = Action::Shift(state(4));
    actions[12 + index(EOF.get())] = Action::Accept;

    let mut gotos = vec![None; 10];
    gotos[index(INNER.get())] = Some(state(2));
    gotos[index(WRAPPED.get())] = Some(state(3));
    gotos[8 + index(INNER.get())] = Some(state(2));
    gotos[8 + index(WRAPPED.get())] = Some(state(0));

    let lalr = LalrTable::new(
        dimensions,
        state(0),
        EOF,
        actions,
        gotos,
        vec![Production::new(INNER, 1), Production::new(WRAPPED, 1)],
    );
    UnvalidatedGrammar::new(
        vec![
            TokenEntry::Bytes(b"i".to_vec()),
            TokenEntry::Bytes(b"c".to_vec()),
            TokenEntry::Bytes(b"ic".to_vec()),
            TokenEntry::Eos,
        ],
        lexer(),
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("fixture validates")
}

fn pop_two_grammar() -> greatgramma_core::ValidatedGrammar {
    let list = NonterminalId::new(0);
    let dimensions = LalrDimensions::new(4, 3, 1);
    let mut actions = vec![Action::Error; 12];
    actions[3 + index(CLOSE.get())] = Action::Shift(state(3));
    actions[6 + index(CLOSE.get())] = reduce(0, 1);
    actions[9 + index(EOF.get())] = Action::Accept;

    let mut gotos = vec![None; 4];
    gotos[0] = Some(state(1));
    gotos[3] = Some(state(0));
    let lalr = LalrTable::new(
        dimensions,
        state(0),
        EOF,
        actions,
        gotos,
        vec![Production::new(list, 2)],
    );
    UnvalidatedGrammar::new(
        vec![
            TokenEntry::Bytes(b"i".to_vec()),
            TokenEntry::Bytes(b"c".to_vec()),
            TokenEntry::Bytes(b"ic".to_vec()),
            TokenEntry::Eos,
        ],
        lexer(),
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("pop-two fixture validates")
}

fn one_sequence_grammar() -> greatgramma_core::ValidatedGrammar {
    let eof = TerminalId::new(1);
    let lexer = LexerDfa::new(
        2,
        1,
        vec![0; 256],
        vec![Some(DfaStateId::new(1)), None],
        DfaStateId::new(0),
        vec![None, Some(ITEM)],
    );
    let lalr = LalrTable::new(
        LalrDimensions::new(2, 2, 0),
        state(0),
        eof,
        vec![
            Action::Shift(state(1)),
            Action::Error,
            Action::Error,
            Action::Accept,
        ],
        Vec::new(),
        Vec::new(),
    );
    UnvalidatedGrammar::new(
        vec![TokenEntry::Bytes(b"i".to_vec()), TokenEntry::Eos],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("one-sequence fixture validates")
}

fn empty_sequence_grammar() -> greatgramma_core::ValidatedGrammar {
    let lexer = LexerDfa::new(
        1,
        1,
        vec![0; 256],
        vec![None],
        DfaStateId::new(0),
        vec![None],
    );
    let lalr = LalrTable::new(
        LalrDimensions::new(3, 1, 0),
        state(0),
        TerminalId::new(0),
        vec![Action::Error; 3],
        Vec::new(),
        Vec::new(),
    );
    UnvalidatedGrammar::new(
        vec![TokenEntry::Bytes(b"x".to_vec()), TokenEntry::Eos],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("empty-sequence fixture validates")
}

fn find_sequence(
    spanner: &greatgramma_core::PreparedSpanner,
    expected: &[TerminalId],
) -> SequenceId {
    let mut index = 0_usize;
    while index < spanner.sequence_count() {
        let sequence = SequenceId::new(u32::try_from(index).expect("fixture IDs fit"));
        if spanner.sequence(sequence) == Some(expected) {
            return sequence;
        }
        index += 1;
    }
    panic!("missing fixture sequence {expected:?}")
}

fn prepared_allows(
    parser: &greatgramma_core::PreparedParser,
    grammar: &greatgramma_core::ValidatedGrammar,
    spanner: &greatgramma_core::PreparedSpanner,
    stack: &[ParserStateId],
    sequence: SequenceId,
) -> Result<bool, ParserError> {
    let top = stack.last().copied().ok_or(ParserError::EmptyStack)?;
    match parser.classification(top, sequence)? {
        SequenceClassification::AlwaysReadable => Ok(true),
        SequenceClassification::Rejected => Ok(false),
        SequenceClassification::Dependent => {
            let head = spanner
                .sequence(sequence)
                .ok_or(ParserError::InvalidSequence { sequence })?;
            execute_sequence_head(grammar, stack, head).map(|result| result.is_some())
        }
    }
}

#[test]
fn prepares_exact_three_way_classifications() {
    let grammar = grammar();
    let spanner = prepare_spanner(&grammar, PreparationLimits::default()).expect("spanner");
    let parser =
        prepare_parser(&grammar, &spanner, PreparationLimits::default()).expect("parser table");
    let item = find_sequence(&spanner, &[ITEM]);
    let close = find_sequence(&spanner, &[CLOSE]);
    let item_close = find_sequence(&spanner, &[ITEM, CLOSE]);

    assert_eq!(
        parser.classification(state(0), item_close),
        Ok(SequenceClassification::AlwaysReadable)
    );
    assert_eq!(
        parser.classification(state(4), item),
        Ok(SequenceClassification::Rejected)
    );
    assert_eq!(
        parser.classification(state(1), close),
        Ok(SequenceClassification::Dependent)
    );

    let mut parser_state = 0_u32;
    while parser_state < grammar.lalr().state_count() {
        let mut sequence_index = 0_usize;
        while sequence_index < spanner.sequence_count() {
            let sequence = SequenceId::new(u32::try_from(sequence_index).expect("fixture IDs fit"));
            let head = spanner.sequence(sequence).expect("sequence exists");
            assert_eq!(
                parser.classification(state(parser_state), sequence),
                classify_sequence_head(&grammar, state(parser_state), head),
            );
            sequence_index += 1;
        }
        parser_state += 1;
    }
}

#[test]
fn dependent_cells_replay_the_exact_full_stack_probe() {
    let grammar = grammar();
    let spanner = prepare_spanner(&grammar, PreparationLimits::default()).expect("spanner");
    let parser =
        prepare_parser(&grammar, &spanner, PreparationLimits::default()).expect("parser table");
    let close = find_sequence(&spanner, &[CLOSE]);

    assert_eq!(
        prepared_allows(&parser, &grammar, &spanner, &[state(0), state(1)], close,),
        Ok(true)
    );
    assert_eq!(
        prepared_allows(&parser, &grammar, &spanner, &[state(4), state(1)], close,),
        Ok(false)
    );
}

#[test]
fn pop_two_dependency_uses_states_below_the_top_two() {
    let grammar = pop_two_grammar();
    let spanner = prepare_spanner(&grammar, PreparationLimits::default()).expect("spanner");
    let parser =
        prepare_parser(&grammar, &spanner, PreparationLimits::default()).expect("parser table");
    let close = find_sequence(&spanner, &[CLOSE]);

    assert_eq!(
        parser.classification(state(2), close),
        Ok(SequenceClassification::Dependent)
    );
    assert_eq!(
        prepared_allows(
            &parser,
            &grammar,
            &spanner,
            &[state(0), state(1), state(2)],
            close,
        ),
        Ok(true)
    );
    assert_eq!(
        prepared_allows(
            &parser,
            &grammar,
            &spanner,
            &[state(3), state(1), state(2)],
            close,
        ),
        Ok(false)
    );

    let mut lower = 0_u32;
    while lower < grammar.lalr().state_count() {
        let mut middle = 0_u32;
        while middle < grammar.lalr().state_count() {
            let stack = [state(lower), state(middle), state(2)];
            let head = spanner.sequence(close).expect("close head exists");
            assert_eq!(
                prepared_allows(&parser, &grammar, &spanner, &stack, close),
                execute_sequence_head(&grammar, &stack, head).map(|result| result.is_some()),
                "stack={stack:?}",
            );
            middle += 1;
        }
        lower += 1;
    }
}

#[test]
fn prepared_allowance_matches_the_full_interpreter_on_bounded_stacks() {
    let grammar = grammar();
    let spanner = prepare_spanner(&grammar, PreparationLimits::default()).expect("spanner");
    let parser =
        prepare_parser(&grammar, &spanner, PreparationLimits::default()).expect("parser table");

    let mut lower = 0_u32;
    while lower < grammar.lalr().state_count() {
        let mut top = 0_u32;
        while top < grammar.lalr().state_count() {
            let stack = [state(lower), state(top)];
            let mut sequence_index = 0_usize;
            while sequence_index < spanner.sequence_count() {
                let sequence =
                    SequenceId::new(u32::try_from(sequence_index).expect("fixture IDs fit"));
                let head = spanner.sequence(sequence).expect("sequence exists");
                assert_eq!(
                    prepared_allows(&parser, &grammar, &spanner, &stack, sequence),
                    execute_sequence_head(&grammar, &stack, head).map(|result| result.is_some()),
                    "stack={stack:?}, head={head:?}",
                );
                sequence_index += 1;
            }
            top += 1;
        }
        lower += 1;
    }
}

#[test]
fn prepared_classification_queries_check_ids() {
    let grammar = grammar();
    let spanner = prepare_spanner(&grammar, PreparationLimits::default()).expect("spanner");
    let parser =
        prepare_parser(&grammar, &spanner, PreparationLimits::default()).expect("parser table");
    let item_close = find_sequence(&spanner, &[ITEM, CLOSE]);

    assert_eq!(
        parser.classification(state(99), item_close),
        Err(ParserError::InvalidState { state: state(99) })
    );
    assert_eq!(
        parser.classification(state(0), SequenceId::new(u32::MAX)),
        Err(ParserError::InvalidSequence {
            sequence: SequenceId::new(u32::MAX),
        })
    );
}

#[test]
fn preparation_limits_cover_rows_cells_and_symbolic_action_work() {
    let grammar = grammar();
    let spanner = prepare_spanner(&grammar, PreparationLimits::default()).expect("spanner");
    let states = usize::try_from(grammar.lalr().state_count()).expect("fixture state count fits");
    let items = states
        .checked_mul(spanner.sequence_count())
        .and_then(|cells| cells.checked_add(states))
        .expect("fixture table items fit");

    assert_eq!(
        prepare_parser(
            &grammar,
            &spanner,
            PreparationLimits {
                max_items: items - 1,
                ..PreparationLimits::default()
            },
        )
        .err(),
        Some(PreparationError::TooLarge {
            required: items,
            maximum: items - 1,
        })
    );
    assert_eq!(
        prepare_parser(
            &grammar,
            &spanner,
            PreparationLimits {
                max_work: 0,
                ..PreparationLimits::default()
            },
        )
        .err(),
        Some(PreparationError::TooLarge {
            required: states,
            maximum: 0,
        })
    );
}

#[test]
fn empty_sequence_tables_still_charge_parser_rows() {
    let grammar = empty_sequence_grammar();
    let spanner = prepare_spanner(&grammar, PreparationLimits::default()).expect("spanner");
    assert_eq!(spanner.sequence_count(), 0);

    assert_eq!(
        prepare_parser(
            &grammar,
            &spanner,
            PreparationLimits {
                max_items: 2,
                ..PreparationLimits::default()
            },
        )
        .err(),
        Some(PreparationError::TooLarge {
            required: 3,
            maximum: 2,
        })
    );
    assert_eq!(
        prepare_parser(
            &grammar,
            &spanner,
            PreparationLimits {
                max_items: 3,
                max_work: 0,
            },
        )
        .err(),
        Some(PreparationError::TooLarge {
            required: 3,
            maximum: 0,
        })
    );

    let parser = prepare_parser(
        &grammar,
        &spanner,
        PreparationLimits {
            max_items: 3,
            max_work: 3,
        },
    )
    .expect("three empty rows fit exactly");
    assert_eq!(
        parser.classification(state(2), SequenceId::new(0)),
        Err(ParserError::InvalidSequence {
            sequence: SequenceId::new(0),
        })
    );
}

#[test]
fn preparation_work_is_cumulative_and_stops_at_the_exact_boundary() {
    let grammar = one_sequence_grammar();
    let spanner = prepare_spanner(&grammar, PreparationLimits::default()).expect("spanner");
    assert_eq!(spanner.sequence_count(), 2);

    assert_eq!(
        prepare_parser(
            &grammar,
            &spanner,
            PreparationLimits {
                max_items: 5,
                ..PreparationLimits::default()
            },
        )
        .err(),
        Some(PreparationError::TooLarge {
            required: 6,
            maximum: 5,
        })
    );
    assert_eq!(
        prepare_parser(
            &grammar,
            &spanner,
            PreparationLimits {
                max_items: 6,
                max_work: 4,
            },
        )
        .err(),
        Some(PreparationError::TooLarge {
            required: 5,
            maximum: 4,
        })
    );
    assert!(
        prepare_parser(
            &grammar,
            &spanner,
            PreparationLimits {
                max_items: 6,
                max_work: 18,
            },
        )
        .is_ok()
    );
}

#[test]
fn invalid_reduction_progress_fails_preparation_closed() {
    let dimensions = LalrDimensions::new(1, 3, 1);
    let lalr = LalrTable::new(
        dimensions,
        state(0),
        EOF,
        vec![reduce(0, 0), Action::Error, Action::Accept],
        vec![Some(state(0))],
        vec![Production::new(INNER, 0)],
    );
    let grammar = UnvalidatedGrammar::new(
        vec![
            TokenEntry::Bytes(b"i".to_vec()),
            TokenEntry::Bytes(b"c".to_vec()),
            TokenEntry::Eos,
        ],
        lexer(),
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("structural validation succeeds");
    let spanner = prepare_spanner(&grammar, PreparationLimits::default()).expect("spanner");

    assert_eq!(
        prepare_parser(&grammar, &spanner, PreparationLimits::default()).err(),
        Some(PreparationError::InvariantViolation)
    );
    assert_eq!(
        prepare_parser(
            &grammar,
            &spanner,
            PreparationLimits {
                max_work: 0,
                ..PreparationLimits::default()
            },
        )
        .err(),
        Some(PreparationError::TooLarge {
            required: 1,
            maximum: 0,
        }),
        "the work limit must stop before the invalid reduction cycle is inspected",
    );
    assert_eq!(
        prepare_parser(
            &grammar,
            &spanner,
            PreparationLimits {
                max_work: 6,
                ..PreparationLimits::default()
            },
        )
        .err(),
        Some(PreparationError::TooLarge {
            required: 7,
            maximum: 6,
        }),
        "the work limit must stop after one reduction and before the cycle repeats",
    );
}
