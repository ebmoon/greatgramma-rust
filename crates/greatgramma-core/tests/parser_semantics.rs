use greatgramma_core::{
    Action, DfaStateId, LalrDimensions, LalrTable, LexerDfa, NonterminalId, ParserError,
    ParserExecution, ParserStateId, Production, ProductionId, SequenceClassification, TerminalId,
    TokenEntry, UnvalidatedGrammar, ValidationLimits, classify_sequence_head,
    execute_sequence_head, execute_terminals,
};

const ITEM: TerminalId = TerminalId::new(0);
const CLOSE: TerminalId = TerminalId::new(1);
const EOF: TerminalId = TerminalId::new(2);
const LIST: NonterminalId = NonterminalId::new(0);

fn state(value: u32) -> ParserStateId {
    ParserStateId::new(value)
}

fn production(value: u32) -> ProductionId {
    ProductionId::new(value)
}

fn reduce(value: u32, rank: u32) -> Action {
    Action::Reduce {
        production: production(value),
        rank,
    }
}

fn sparse_lalr(
    dimensions: LalrDimensions,
    eof: TerminalId,
    action_cells: &[(u32, TerminalId, Action)],
    goto_cells: &[(u32, NonterminalId, u32)],
    productions: Vec<Production>,
    ignored: Vec<TerminalId>,
) -> LalrTable {
    let state_count = usize::try_from(dimensions.state_count()).expect("fixture states fit");
    let terminal_count =
        usize::try_from(dimensions.terminal_count()).expect("fixture terminals fit");
    let nonterminal_count =
        usize::try_from(dimensions.nonterminal_count()).expect("fixture nonterminals fit");
    let mut actions = vec![Action::Error; state_count * terminal_count];
    let mut gotos = vec![None; state_count * nonterminal_count];

    for &(source, terminal, action) in action_cells {
        let source = usize::try_from(source).expect("fixture state fits");
        let terminal = usize::try_from(terminal.get()).expect("fixture terminal fits");
        actions[source * terminal_count + terminal] = action;
    }
    for &(source, nonterminal, destination) in goto_cells {
        let source = usize::try_from(source).expect("fixture state fits");
        let nonterminal = usize::try_from(nonterminal.get()).expect("fixture nonterminal fits");
        gotos[source * nonterminal_count + nonterminal] = Some(state(destination));
    }

    LalrTable::new(dimensions, state(0), eof, actions, gotos, productions)
        .with_ignored_terminals(ignored)
}

fn validated(lalr: LalrTable) -> greatgramma_core::ValidatedGrammar {
    let lexer = LexerDfa::new(
        2,
        1,
        vec![0; 256],
        vec![Some(DfaStateId::new(1)), None],
        DfaStateId::new(0),
        vec![None, Some(ITEM)],
    );
    UnvalidatedGrammar::new(
        vec![TokenEntry::Bytes(b"a".to_vec()), TokenEntry::Eos],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("parser fixture validates")
}

fn close_delimiter_grammar() -> greatgramma_core::ValidatedGrammar {
    let inner = NonterminalId::new(0);
    let wrapped = NonterminalId::new(1);
    validated(sparse_lalr(
        LalrDimensions::new(5, 3, 2),
        EOF,
        &[
            (0, ITEM, Action::Shift(state(1))),
            (1, CLOSE, reduce(0, 2)),
            (2, CLOSE, reduce(1, 1)),
            (3, CLOSE, Action::Shift(state(4))),
            (4, EOF, Action::Accept),
        ],
        &[(0, inner, 2), (0, wrapped, 3)],
        vec![Production::new(inner, 1), Production::new(wrapped, 1)],
        Vec::new(),
    ))
}

#[test]
fn closes_multiple_reductions_before_shifting_the_delimiter() {
    let grammar = close_delimiter_grammar();

    assert_eq!(
        execute_terminals(&grammar, &[state(0)], &[ITEM, CLOSE]),
        Ok(Some(ParserExecution::Continue {
            stack: vec![state(0), state(3), state(4)],
        }))
    );
    assert_eq!(
        execute_terminals(&grammar, &[state(0)], &[ITEM, CLOSE, EOF]),
        Ok(Some(ParserExecution::Accepted))
    );
}

#[test]
fn probes_the_final_head_terminal_without_committing_its_reductions_or_shift() {
    let grammar = close_delimiter_grammar();

    assert_eq!(
        execute_sequence_head(&grammar, &[state(0)], &[ITEM, CLOSE]),
        Ok(Some(vec![state(0), state(1)]))
    );
    assert_eq!(
        execute_sequence_head(&grammar, &[state(0), state(1)], &[CLOSE]),
        Ok(Some(vec![state(0), state(1)]))
    );
}

#[test]
fn classifies_heads_by_symbolic_base_dependency() {
    let grammar = close_delimiter_grammar();

    assert_eq!(
        classify_sequence_head(&grammar, state(0), &[ITEM, CLOSE]),
        Ok(SequenceClassification::AlwaysReadable)
    );
    assert_eq!(
        classify_sequence_head(&grammar, state(4), &[ITEM]),
        Ok(SequenceClassification::Rejected)
    );
    assert_eq!(
        classify_sequence_head(&grammar, state(1), &[CLOSE]),
        Ok(SequenceClassification::Dependent)
    );
}

#[test]
fn ignored_terminals_are_parser_noops_in_direct_and_speculative_positions() {
    let ignored = TerminalId::new(1);
    let grammar = validated(sparse_lalr(
        LalrDimensions::new(2, 3, 0),
        EOF,
        &[(0, ITEM, Action::Shift(state(1))), (1, EOF, Action::Accept)],
        &[],
        Vec::new(),
        vec![ignored],
    ));

    assert_eq!(
        execute_terminals(&grammar, &[state(0)], &[ignored, ITEM, ignored]),
        Ok(Some(ParserExecution::Continue {
            stack: vec![state(0), state(1)],
        }))
    );
    assert_eq!(
        execute_sequence_head(&grammar, &[state(0)], &[ignored]),
        Ok(Some(vec![state(0)]))
    );
    assert_eq!(
        execute_sequence_head(&grammar, &[state(0)], &[ignored, ITEM]),
        Ok(Some(vec![state(0)]))
    );
    assert_eq!(
        classify_sequence_head(&grammar, state(0), &[ignored]),
        Ok(SequenceClassification::AlwaysReadable)
    );
}

#[test]
fn nullable_and_right_recursive_reductions_finish_only_on_explicit_eof() {
    let grammar = validated(sparse_lalr(
        LalrDimensions::new(4, 3, 1),
        EOF,
        &[
            (0, ITEM, Action::Shift(state(1))),
            (0, EOF, reduce(0, 2)),
            (1, ITEM, Action::Shift(state(1))),
            (1, EOF, reduce(0, 2)),
            (2, EOF, reduce(1, 1)),
            (3, EOF, Action::Accept),
        ],
        &[(0, LIST, 3), (1, LIST, 2)],
        vec![Production::new(LIST, 0), Production::new(LIST, 2)],
        Vec::new(),
    ));

    assert_eq!(
        execute_terminals(&grammar, &[state(0)], &[EOF]),
        Ok(Some(ParserExecution::Accepted))
    );
    assert_eq!(
        execute_terminals(&grammar, &[state(0)], &[ITEM, ITEM]),
        Ok(Some(ParserExecution::Continue {
            stack: vec![state(0), state(1), state(1)],
        }))
    );
    assert_eq!(
        execute_terminals(&grammar, &[state(0)], &[ITEM, ITEM, EOF]),
        Ok(Some(ParserExecution::Accepted))
    );
}

fn reduction_error_grammar(
    pop_len: u32,
    rank: u32,
    goto: Option<u32>,
) -> greatgramma_core::ValidatedGrammar {
    let goto_cells = goto.map_or_else(Vec::new, |destination| vec![(0, LIST, destination)]);
    validated(sparse_lalr(
        LalrDimensions::new(1, 3, 1),
        EOF,
        &[(0, ITEM, reduce(0, rank)), (0, EOF, Action::Accept)],
        &goto_cells,
        vec![Production::new(LIST, pop_len)],
        Vec::new(),
    ))
}

#[test]
fn reports_underflow_and_missing_goto_as_invariant_errors() {
    let underflow = reduction_error_grammar(1, 1, Some(0));
    assert_eq!(
        execute_terminals(&underflow, &[state(0)], &[ITEM]),
        Err(ParserError::StackUnderflow {
            production: production(0),
            pop_len: 1,
            stack_len: 1,
        })
    );

    let missing_goto = reduction_error_grammar(0, 1, None);
    assert_eq!(
        execute_terminals(&missing_goto, &[state(0)], &[ITEM]),
        Err(ParserError::MissingGoto {
            state: state(0),
            nonterminal: LIST,
            production: production(0),
        })
    );
}

#[test]
fn rejects_accept_before_end_and_non_decreasing_reduction_cycles() {
    let accept = validated(sparse_lalr(
        LalrDimensions::new(1, 3, 0),
        EOF,
        &[(0, EOF, Action::Accept)],
        &[],
        Vec::new(),
        Vec::new(),
    ));
    assert_eq!(
        execute_terminals(&accept, &[state(0)], &[EOF, EOF]),
        Err(ParserError::AcceptBeforeEnd { terminal_index: 0 })
    );

    let cycle = reduction_error_grammar(0, 0, Some(0));
    assert_eq!(
        execute_terminals(&cycle, &[state(0)], &[ITEM]),
        Err(ParserError::InvalidReductionProgress {
            state: state(0),
            terminal: ITEM,
        })
    );

    let equal_after_one_pop = reduction_error_grammar(1, 0, Some(0));
    assert_eq!(
        execute_terminals(&equal_after_one_pop, &[state(0), state(0)], &[ITEM]),
        Err(ParserError::InvalidReductionProgress {
            state: state(0),
            terminal: ITEM,
        })
    );

    let increasing_rank = validated(sparse_lalr(
        LalrDimensions::new(2, 3, 1),
        EOF,
        &[(0, ITEM, reduce(0, 0)), (1, ITEM, reduce(1, 1))],
        &[(0, LIST, 1)],
        vec![Production::new(LIST, 0), Production::new(LIST, 0)],
        Vec::new(),
    ));
    assert_eq!(
        execute_terminals(&increasing_rank, &[state(0)], &[ITEM]),
        Err(ParserError::InvalidReductionProgress {
            state: state(1),
            terminal: ITEM,
        })
    );
}

#[test]
fn validates_public_parser_inputs_before_table_access() {
    let grammar = close_delimiter_grammar();

    assert_eq!(
        execute_terminals(&grammar, &[], &[ITEM]),
        Err(ParserError::EmptyStack)
    );
    assert_eq!(
        execute_terminals(&grammar, &[state(99)], &[ITEM]),
        Err(ParserError::InvalidState { state: state(99) })
    );
    assert_eq!(
        execute_terminals(&grammar, &[state(0)], &[TerminalId::new(99)]),
        Err(ParserError::InvalidTerminal {
            terminal: TerminalId::new(99),
        })
    );
    assert_eq!(
        execute_sequence_head(&grammar, &[state(0)], &[]),
        Err(ParserError::EmptySequenceHead)
    );
}
