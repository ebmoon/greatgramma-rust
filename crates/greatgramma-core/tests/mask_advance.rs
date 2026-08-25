use greatgramma_core::{
    Action, AdvanceResult, DfaStateId, EngineError, LalrDimensions, LalrTable, LexerDfa,
    NonterminalId, ParserStateId, PreparationLimits, Production, ProductionId, TerminalId,
    TokenEntry, TokenId, UnvalidatedGrammar, ValidationLimits, prepare, prepare_parser,
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

fn reduce(production: u32, rank: u32) -> Action {
    Action::Reduce {
        production: ProductionId::new(production),
        rank,
    }
}

fn grammar() -> greatgramma_core::ValidatedGrammar {
    let mut byte_classes = vec![0; 256];
    byte_classes[usize::from(b'c')] = 1;
    byte_classes[usize::from(b'z')] = 2;
    let lexer = LexerDfa::new(
        3,
        3,
        byte_classes,
        vec![
            Some(DfaStateId::new(1)),
            Some(DfaStateId::new(2)),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(DfaStateId::new(2)),
        ],
        DfaStateId::new(0),
        vec![None, Some(ITEM), Some(CLOSE)],
    );

    let mut actions = vec![Action::Error; 15];
    actions[usize::try_from(ITEM.get()).expect("fixture ID fits")] = Action::Shift(state(1));
    actions[3 + usize::try_from(CLOSE.get()).expect("fixture ID fits")] = reduce(0, 2);
    actions[6 + usize::try_from(CLOSE.get()).expect("fixture ID fits")] = reduce(1, 1);
    actions[9 + usize::try_from(CLOSE.get()).expect("fixture ID fits")] = Action::Shift(state(4));
    actions[12 + usize::try_from(EOF.get()).expect("fixture ID fits")] = Action::Accept;

    let mut gotos = vec![None; 10];
    gotos[usize::try_from(INNER.get()).expect("fixture ID fits")] = Some(state(2));
    gotos[usize::try_from(WRAPPED.get()).expect("fixture ID fits")] = Some(state(3));
    let lalr = LalrTable::new(
        LalrDimensions::new(5, 3, 2),
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
            TokenEntry::Bytes(b"z".to_vec()),
            TokenEntry::Bytes(b"ii".to_vec()),
            TokenEntry::Bytes(b"cc".to_vec()),
            TokenEntry::Bytes(b"ccc".to_vec()),
            TokenEntry::Bytes(b"cccc".to_vec()),
            TokenEntry::Eos,
        ],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("fixture validates")
}

fn dead_end_grammar() -> greatgramma_core::ValidatedGrammar {
    let lexer = LexerDfa::new(
        1,
        1,
        vec![0; 256],
        vec![None],
        DfaStateId::new(0),
        vec![None],
    );
    let lalr = LalrTable::new(
        LalrDimensions::new(1, 1, 0),
        state(0),
        TerminalId::new(0),
        vec![Action::Error],
        Vec::new(),
        Vec::new(),
    );
    UnvalidatedGrammar::new(
        vec![TokenEntry::Bytes(b"x".to_vec()), TokenEntry::Eos],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("dead-end fixture validates")
}

fn nonzero_start_grammar() -> greatgramma_core::ValidatedGrammar {
    let lexer = LexerDfa::new(
        1,
        1,
        vec![0; 256],
        vec![None],
        DfaStateId::new(0),
        vec![None],
    );
    let lalr = LalrTable::new(
        LalrDimensions::new(2, 1, 0),
        state(1),
        TerminalId::new(0),
        vec![Action::Error, Action::Accept],
        Vec::new(),
        Vec::new(),
    );
    UnvalidatedGrammar::new(
        vec![TokenEntry::Bytes(b"x".to_vec()), TokenEntry::Eos],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("nonzero-start fixture validates")
}

fn stack_dependent_grammar() -> greatgramma_core::ValidatedGrammar {
    let a = TerminalId::new(0);
    let b = TerminalId::new(1);
    let x = TerminalId::new(2);
    let close = TerminalId::new(3);
    let skip = TerminalId::new(4);
    let eof = TerminalId::new(5);
    let reduced = NonterminalId::new(0);

    let mut byte_classes = vec![5; 256];
    byte_classes[usize::from(b'a')] = 0;
    byte_classes[usize::from(b'b')] = 1;
    byte_classes[usize::from(b'x')] = 2;
    byte_classes[usize::from(b's')] = 3;
    byte_classes[usize::from(b'z')] = 4;
    let mut transitions = vec![None; 6 * 6];
    transitions[0] = Some(DfaStateId::new(1));
    transitions[1] = Some(DfaStateId::new(2));
    transitions[2] = Some(DfaStateId::new(3));
    transitions[3] = Some(DfaStateId::new(4));
    transitions[4 * 6 + 4] = Some(DfaStateId::new(5));
    let lexer = LexerDfa::new(
        6,
        6,
        byte_classes,
        transitions,
        DfaStateId::new(0),
        vec![None, Some(a), Some(b), Some(x), Some(skip), Some(close)],
    );

    let mut actions = vec![Action::Error; 6 * 6];
    actions[usize::try_from(a.get()).expect("fixture ID fits")] = Action::Shift(state(1));
    actions[usize::try_from(b.get()).expect("fixture ID fits")] = Action::Shift(state(4));
    actions[4 * 6 + usize::try_from(x.get()).expect("fixture ID fits")] = Action::Shift(state(1));
    actions[6 + usize::try_from(close.get()).expect("fixture ID fits")] = reduce(0, 1);
    actions[2 * 6 + usize::try_from(close.get()).expect("fixture ID fits")] =
        Action::Shift(state(3));
    let mut gotos = vec![None; 6];
    gotos[usize::try_from(reduced.get()).expect("fixture ID fits")] = Some(state(2));
    gotos[4] = Some(state(5));
    let lalr = LalrTable::new(
        LalrDimensions::new(6, 6, 1),
        state(0),
        eof,
        actions,
        gotos,
        vec![Production::new(reduced, 1)],
    )
    .with_ignored_terminals(vec![skip]);

    UnvalidatedGrammar::new(
        vec![
            TokenEntry::Bytes(b"as".to_vec()),
            TokenEntry::Bytes(b"bxs".to_vec()),
            TokenEntry::Bytes(b"z".to_vec()),
            TokenEntry::Eos,
        ],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("stack-dependent fixture validates")
}

#[test]
fn masks_and_advances_with_one_exact_transition_relation() {
    let mut matcher = prepare(grammar(), PreparationLimits::default())
        .expect("preparation succeeds")
        .into_matcher(1)
        .expect("matcher allocation succeeds");

    assert_eq!(matcher.token_count(), 10);
    assert_eq!(matcher.mask_bytes(), 2);
    assert_eq!(matcher.row_count(), 1);
    assert_eq!(matcher.is_completed(0), Ok(false));

    let mut mask = [0xff; 2];
    assert_eq!(matcher.mask(0, &mut mask), Ok(()));
    assert_eq!(mask, [0b0000_0101, 0]);
    for token in 0_u32..10 {
        assert_eq!(
            matcher.allows(0, TokenId::new(token)),
            Ok(
                mask[usize::try_from(token / 8).expect("byte index fits")] & (1 << (token % 8))
                    != 0
            ),
        );
    }

    assert_eq!(
        matcher.advance(0, TokenId::new(1)),
        Err(EngineError::ConstraintViolation {
            token: TokenId::new(1),
        })
    );
    mask.fill(0xff);
    matcher
        .mask(0, &mut mask)
        .expect("rejected advance preserves its source");
    assert_eq!(mask, [0b0000_0101, 0]);

    assert_eq!(
        matcher.advance(0, TokenId::new(5)),
        Err(EngineError::ConstraintViolation {
            token: TokenId::new(5),
        }),
        "a token that emits ITEM before probing another ITEM must roll back",
    );

    assert_eq!(
        matcher.advance(0, TokenId::new(0)),
        Ok(AdvanceResult::Continue)
    );
    mask.fill(0xff);
    assert_eq!(matcher.mask(0, &mut mask), Ok(()));
    assert_eq!(mask, [0b0000_0010, 0]);
    for eos in [TokenId::new(3), TokenId::new(9)] {
        assert_eq!(matcher.allows(0, eos), Ok(false));
        assert_eq!(
            matcher.advance(0, eos),
            Err(EngineError::ConstraintViolation { token: eos })
        );
    }

    assert_eq!(
        matcher.advance(0, TokenId::new(1)),
        Ok(AdvanceResult::Continue)
    );
    mask.fill(0xff);
    assert_eq!(matcher.mask(0, &mut mask), Ok(()));
    assert_eq!(mask, [0b0001_1000, 0b0000_0010]);

    let mut combined = prepare(grammar(), PreparationLimits::default())
        .expect("combined preparation")
        .into_matcher(1)
        .expect("combined matcher");
    assert_eq!(
        combined.advance(0, TokenId::new(2)),
        Ok(AdvanceResult::Continue)
    );
    let mut combined_mask = [0xff; 2];
    combined
        .mask(0, &mut combined_mask)
        .expect("only the direct ITEM prefix was committed");
    assert_eq!(combined_mask, mask);

    let mut alias = prepare(grammar(), PreparationLimits::default())
        .expect("alias preparation")
        .into_matcher(1)
        .expect("alias matcher");
    alias.advance(0, TokenId::new(0)).expect("item");
    alias.advance(0, TokenId::new(1)).expect("close");
    assert_eq!(
        alias.advance(0, TokenId::new(9)),
        Ok(AdvanceResult::Accepted)
    );

    assert_eq!(
        matcher.advance(0, TokenId::new(4)),
        Ok(AdvanceResult::Continue)
    );
    let mut probe_mask = [0xff; 2];
    matcher
        .mask(0, &mut probe_mask)
        .expect("the dependent CLOSE probe was not committed");
    assert_eq!(probe_mask, mask);

    assert_eq!(
        matcher.advance(0, TokenId::new(3)),
        Ok(AdvanceResult::Accepted)
    );
    assert_eq!(matcher.is_completed(0), Ok(true));

    let mut completed_mask = [0xff; 2];
    assert_eq!(
        matcher.mask(0, &mut completed_mask),
        Err(EngineError::Completed)
    );
    assert_eq!(completed_mask, [0; 2]);
    assert_eq!(
        matcher.allows(0, TokenId::new(0)),
        Err(EngineError::Completed)
    );
    assert_eq!(
        matcher.advance(0, TokenId::new(0)),
        Err(EngineError::Completed)
    );
}

#[test]
fn dependent_sequences_use_the_full_reachable_parser_stack() {
    let mut allowed = prepare(stack_dependent_grammar(), PreparationLimits::default())
        .expect("allowed preparation")
        .into_matcher(1)
        .expect("allowed matcher");
    let mut rejected = prepare(stack_dependent_grammar(), PreparationLimits::default())
        .expect("rejected preparation")
        .into_matcher(1)
        .expect("rejected matcher");
    assert_eq!(
        allowed.advance(0, TokenId::new(0)),
        Ok(AdvanceResult::Continue)
    );
    assert_eq!(
        rejected.advance(0, TokenId::new(1)),
        Ok(AdvanceResult::Continue)
    );

    assert_eq!(allowed.allows(0, TokenId::new(2)), Ok(true));
    assert_eq!(rejected.allows(0, TokenId::new(2)), Ok(false));

    let mut allowed_mask = [0xff];
    allowed
        .mask(0, &mut allowed_mask)
        .expect("the allowed context has at least one token");
    assert_ne!(allowed_mask[0] & (1 << 2), 0);

    let mut rejected_mask = [0xff];
    let rejected_mask_result = rejected.mask(0, &mut rejected_mask);
    assert_eq!(rejected_mask[0] & (1 << 2), 0);
    assert_eq!(
        rejected.advance(0, TokenId::new(2)),
        Err(EngineError::ConstraintViolation {
            token: TokenId::new(2),
        })
    );
    let mut repeated_mask = [0xff];
    assert_eq!(
        rejected.mask(0, &mut repeated_mask),
        rejected_mask_result,
        "rejected advance must preserve the source state",
    );
    assert_eq!(repeated_mask, rejected_mask);
}

#[test]
fn invalid_inputs_and_dead_ends_fail_closed() {
    assert_eq!(
        prepare(grammar(), PreparationLimits::default())
            .expect("zero-row preparation")
            .into_matcher(0)
            .err(),
        Some(EngineError::EmptyBatch)
    );
    let mut matcher = prepare(grammar(), PreparationLimits::default())
        .expect("preparation succeeds")
        .into_matcher(1)
        .expect("matcher");
    assert_eq!(
        matcher.allows(0, TokenId::new(99)),
        Err(EngineError::InvalidToken {
            token: TokenId::new(99),
        })
    );
    assert_eq!(
        matcher.advance(0, TokenId::new(99)),
        Err(EngineError::InvalidToken {
            token: TokenId::new(99),
        })
    );

    let mut short = [0xff];
    assert_eq!(
        matcher.mask(0, &mut short),
        Err(EngineError::BufferTooSmall {
            required: 2,
            actual: 1,
        })
    );
    assert_eq!(short, [0]);

    let mut invalid_row_mask = [0xff; 2];
    assert_eq!(
        matcher.mask(1, &mut invalid_row_mask),
        Err(EngineError::InvalidRow { row: 1, rows: 1 })
    );
    assert_eq!(invalid_row_mask, [0; 2]);
    assert_eq!(
        matcher.advance_batch(&[]),
        Err(EngineError::BatchSizeMismatch {
            expected: 1,
            actual: 0,
        })
    );

    let mut dead = prepare(dead_end_grammar(), PreparationLimits::default())
        .expect("dead-end preparation succeeds")
        .into_matcher(1)
        .expect("dead-end matcher");
    assert_eq!(dead.allows(0, TokenId::new(0)), Ok(false));
    assert_eq!(dead.allows(0, TokenId::new(1)), Ok(false));
    assert_eq!(
        dead.advance(0, TokenId::new(0)),
        Err(EngineError::ConstraintViolation {
            token: TokenId::new(0),
        })
    );

    let mut mask = [0xff];
    assert_eq!(dead.mask(0, &mut mask), Err(EngineError::NoValidToken));
    assert_eq!(mask, [0]);
}

#[test]
fn batch_advances_are_transactional_and_masks_are_row_major() {
    let mut matcher = prepare(grammar(), PreparationLimits::default())
        .expect("preparation")
        .into_matcher(2)
        .expect("two-row matcher");
    assert_eq!(
        matcher.advance_batch(&[TokenId::new(0), TokenId::new(1)]),
        Err(EngineError::ConstraintViolation {
            token: TokenId::new(1),
        })
    );
    let mut masks = [0xff; 4];
    matcher
        .masks(&mut masks)
        .expect("failed batch leaves both rows unchanged");
    assert_eq!(masks, [0b0000_0101, 0, 0b0000_0101, 0]);

    assert_eq!(
        matcher.advance_batch(&[TokenId::new(0), TokenId::new(2)]),
        Ok(vec![AdvanceResult::Continue, AdvanceResult::Continue])
    );
    matcher.masks(&mut masks).expect("row-major masks");
    assert_eq!(masks, [0b0000_0010, 0, 0b0001_1000, 0b0000_0010]);
}

#[test]
fn no_result_active_advance_reports_the_rejected_row_and_rolls_back() {
    let mut matcher = prepare(grammar(), PreparationLimits::default())
        .expect("preparation")
        .into_matcher(2)
        .expect("two-row matcher");
    let mut output = [0xff; 4];

    assert_eq!(
        matcher.advance_active_and_masks_no_result(
            &[Some(TokenId::new(0)), Some(TokenId::new(1))],
            &mut output,
        ),
        Err(EngineError::BatchConstraintViolation {
            row: 1,
            token: TokenId::new(1),
        })
    );
    assert_eq!(output, [0; 4]);

    matcher
        .masks(&mut output)
        .expect("failed transaction leaves both rows unchanged");
    assert_eq!(output, [0b0000_0101, 0, 0b0000_0101, 0]);

    matcher
        .advance_active_and_masks_no_result(
            &[Some(TokenId::new(0)), Some(TokenId::new(2))],
            &mut output,
        )
        .expect("valid transaction commits");
    assert_eq!(output, [0b0000_0010, 0, 0b0001_1000, 0b0000_0010]);
}

#[test]
fn accepted_batch_rows_can_be_retained_while_running_rows_advance() {
    let mut matcher = prepare(grammar(), PreparationLimits::default())
        .expect("preparation")
        .into_matcher(2)
        .expect("two-row matcher");
    assert_eq!(
        matcher.advance_batch(&[TokenId::new(2), TokenId::new(0)]),
        Ok(vec![AdvanceResult::Continue, AdvanceResult::Continue])
    );
    assert_eq!(
        matcher.advance_active(&[Some(TokenId::new(3)), Some(TokenId::new(1))]),
        Ok(vec![
            Some(AdvanceResult::Accepted),
            Some(AdvanceResult::Continue),
        ])
    );
    assert_eq!(matcher.is_completed(0), Ok(true));
    assert_eq!(matcher.is_completed(1), Ok(false));
    assert_eq!(
        matcher.advance_active(&[None, Some(TokenId::new(9))]),
        Ok(vec![None, Some(AdvanceResult::Accepted)])
    );
    assert_eq!(matcher.is_completed(1), Ok(true));
}

#[test]
fn top_level_preparation_uses_one_cumulative_work_budget() {
    let limits = PreparationLimits {
        max_work: 526,
        ..PreparationLimits::default()
    };
    let grammar = dead_end_grammar();
    let spanner = prepare_spanner(&grammar, limits).expect("spanner exactly fits its budget");
    prepare_parser(&grammar, &spanner, limits).expect("parser alone fits the same budget");

    assert_eq!(
        prepare(dead_end_grammar(), limits).err(),
        Some(greatgramma_core::PreparationError::TooLarge {
            required: 527,
            maximum: 526,
        })
    );
    prepare(
        dead_end_grammar(),
        PreparationLimits {
            max_work: 527,
            ..PreparationLimits::default()
        },
    )
    .expect("the reported exact cumulative requirement succeeds");
}

#[test]
fn initial_state_uses_logical_lexer_start_and_configured_parser_start() {
    let mut matcher = prepare(nonzero_start_grammar(), PreparationLimits::default())
        .expect("preparation succeeds")
        .into_matcher(1)
        .expect("matcher");
    let mut mask = [0xff];
    matcher.mask(0, &mut mask).expect("EOS is legal");
    assert_eq!(mask, [0b0000_0010]);
    assert_eq!(
        matcher.advance(0, TokenId::new(1)),
        Ok(AdvanceResult::Accepted)
    );
}
