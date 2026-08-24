use greatgramma_core::{
    Action, DfaStateId, LalrDimensions, LalrTable, LexerDfa, LexerState, ParserStateId,
    PreparationLimits, TerminalId, TokenEntry, UnvalidatedGrammar, ValidationLimits,
    prepare_spanner,
};

const VALUE: TerminalId = TerminalId::new(0);
const EOF: TerminalId = TerminalId::new(1);

fn chain_grammar(state_count: u32) -> greatgramma_core::ValidatedGrammar {
    let state_count_usize = usize::try_from(state_count).expect("fixture state count fits");
    let mut transitions = vec![None; state_count_usize];
    let mut state = 0_u32;
    while state + 1 < state_count {
        transitions[usize::try_from(state).expect("fixture state fits")] =
            Some(DfaStateId::new(state + 1));
        state += 1;
    }

    let mut terminals = vec![None; state_count_usize];
    terminals[state_count_usize - 1] = Some(VALUE);
    let lexer = LexerDfa::new(
        state_count,
        1,
        vec![0; 256],
        transitions,
        DfaStateId::new(0),
        terminals,
    );
    let lalr = LalrTable::new(
        LalrDimensions::new(1, 2, 0),
        ParserStateId::new(0),
        EOF,
        vec![Action::Error, Action::Accept],
        Vec::new(),
        Vec::new(),
    );
    UnvalidatedGrammar::new(
        vec![TokenEntry::Bytes(vec![0]), TokenEntry::Eos],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("scale fixture validates")
}

#[test]
fn prepares_a_two_thousand_state_zero_output_chain() {
    let grammar = chain_grammar(2_000);
    let spanner = prepare_spanner(&grammar, PreparationLimits::default())
        .expect("simple worklist fits the default limits");

    assert_eq!(
        spanner.singleton_terminals(LexerState::Start),
        Ok(&[VALUE][..])
    );
    assert_eq!(
        spanner.singleton_terminals(LexerState::Dfa(DfaStateId::new(1_999))),
        Ok(&[VALUE][..]),
    );
}
