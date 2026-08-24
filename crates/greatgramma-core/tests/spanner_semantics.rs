use greatgramma_core::{
    Action, DfaStateId, LalrDimensions, LalrTable, LexerDfa, LexerState, ParserStateId,
    PreparedSpanner, SequenceId, SpannerLimits, SpannerQueryError, TerminalId, TokenEntry,
    TokenExecution, TokenId, UnvalidatedGrammar, ValidationLimits, execute_token, prepare_spanner,
};

const A: TerminalId = TerminalId::new(0);
const B: TerminalId = TerminalId::new(1);
const C: TerminalId = TerminalId::new(2);
const UNREACHABLE: TerminalId = TerminalId::new(3);
const EOF: TerminalId = TerminalId::new(4);

fn transition(
    transitions: &mut [Option<DfaStateId>],
    class_count: usize,
    source: usize,
    class: usize,
    destination: u32,
) {
    transitions[source * class_count + class] = Some(DfaStateId::new(destination));
}

fn grammar() -> greatgramma_core::ValidatedGrammar {
    let mut classes = vec![0; 256];
    classes[usize::from(b'a')] = 1;
    classes[usize::from(b'b')] = 2;
    classes[usize::from(b'c')] = 3;
    classes[usize::from(b'x')] = 4;
    classes[usize::from(b'y')] = 5;
    classes[usize::from(b'z')] = 6;
    let class_count = 7;
    let mut transitions = vec![None; 7 * class_count];
    transition(&mut transitions, class_count, 0, 1, 1);
    transition(&mut transitions, class_count, 0, 2, 2);
    transition(&mut transitions, class_count, 0, 3, 5);
    transition(&mut transitions, class_count, 0, 4, 3);
    transition(&mut transitions, class_count, 3, 4, 3); // zero-output cycle
    transition(&mut transitions, class_count, 3, 5, 4);
    transition(&mut transitions, class_count, 3, 6, 4); // reconverges
    transition(&mut transitions, class_count, 4, 5, 4); // second zero cycle
    transition(&mut transitions, class_count, 4, 6, 5);

    let lexer = LexerDfa::new(
        7,
        u32::try_from(class_count).expect("fixture class count fits"),
        classes,
        transitions,
        DfaStateId::new(0),
        vec![
            None,
            Some(A),
            Some(B),
            None,
            None,
            Some(C),
            Some(UNREACHABLE),
        ],
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
            TokenEntry::Bytes(b"aa".to_vec()),
            TokenEntry::Bytes(b"aabb".to_vec()),
            TokenEntry::Bytes(b"aa".to_vec()),
            TokenEntry::Bytes(b"x".to_vec()),
            TokenEntry::Bytes(b"xyza".to_vec()),
            TokenEntry::Bytes(b"q".to_vec()),
            TokenEntry::Eos,
        ],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("fixture validates")
}

fn find_sequence(spanner: &PreparedSpanner, expected: &[TerminalId]) -> SequenceId {
    let mut index = 0;
    while index < spanner.sequence_count() {
        let id = SequenceId::new(u32::try_from(index).expect("fixture sequence count fits"));
        if spanner.sequence(id) == Some(expected) {
            return id;
        }
        index += 1;
    }
    panic!("missing sequence {expected:?}");
}

#[test]
fn computes_exact_singleton_heads_through_cycles_and_reconverging_raw_bytes() {
    let grammar = grammar();
    let spanner =
        prepare_spanner(&grammar, SpannerLimits::default()).expect("preparation succeeds");

    assert_eq!(
        spanner.singleton_terminals(LexerState::Start),
        Ok(&[A, B, C][..])
    );
    assert_eq!(
        spanner.singleton_terminals(LexerState::Dfa(DfaStateId::new(3))),
        Ok(&[C][..])
    );
    assert_eq!(
        spanner.singleton_terminals(LexerState::Dfa(DfaStateId::new(4))),
        Ok(&[C][..])
    );
    assert_eq!(
        spanner.singleton_terminals(LexerState::Dfa(DfaStateId::new(6))),
        Ok(&[UNREACHABLE][..])
    );
}

#[test]
fn inverse_buckets_preserve_direct_emissions_duplicate_ids_and_rejection() {
    let grammar = grammar();
    let spanner =
        prepare_spanner(&grammar, SpannerLimits::default()).expect("preparation succeeds");
    let a = find_sequence(&spanner, &[A]);
    let aa = find_sequence(&spanner, &[A, A]);
    let aabb = find_sequence(&spanner, &[A, A, B, B]);
    let ca = find_sequence(&spanner, &[C, A]);

    assert_eq!(
        spanner.inverse_tokens(LexerState::Start, a),
        Ok(&[TokenId::new(0)][..])
    );
    assert_eq!(
        spanner.inverse_tokens(LexerState::Start, aa),
        Ok(&[TokenId::new(1), TokenId::new(3)][..])
    );
    assert_eq!(
        spanner.inverse_tokens(LexerState::Start, ca),
        Ok(&[TokenId::new(5)][..])
    );
    assert_eq!(
        spanner.inverse_tokens(LexerState::Start, aabb),
        Ok(&[TokenId::new(2)][..])
    );
    assert_eq!(
        spanner.inverse_tokens(LexerState::Dfa(DfaStateId::new(0)), ca),
        Ok(&[TokenId::new(5)][..])
    );

    let result = execute_token(&grammar, LexerState::Start, TokenId::new(0))
        .expect("valid direct query")
        .expect("token is accepted");
    assert_eq!(
        result,
        TokenExecution::Continue {
            state: LexerState::Dfa(DfaStateId::new(1)),
            emitted: Vec::new(),
        }
    );
    assert_eq!(spanner.sequence(a), Some(&[A][..]));
}

#[test]
fn excludes_eos_empty_heads_and_fabricated_or_unchecked_membership() {
    let grammar = grammar();
    let spanner =
        prepare_spanner(&grammar, SpannerLimits::default()).expect("preparation succeeds");
    let ca = find_sequence(&spanner, &[C, A]);

    let mut sequence_index = 0;
    while sequence_index < spanner.sequence_count() {
        let sequence =
            SequenceId::new(u32::try_from(sequence_index).expect("fixture sequence count fits"));
        assert!(
            !spanner
                .sequence(sequence)
                .expect("sequence exists")
                .is_empty()
        );
        assert!(
            !spanner
                .inverse_tokens(LexerState::Start, sequence)
                .expect("query is checked")
                .contains(&TokenId::new(7))
        );
        sequence_index += 1;
    }
    assert_eq!(
        spanner.inverse_tokens(LexerState::Dfa(DfaStateId::new(6)), ca),
        Ok(&[][..])
    );
    assert_eq!(
        spanner.singleton_terminals(LexerState::Dfa(DfaStateId::new(99))),
        Err(SpannerQueryError::InvalidState {
            state: DfaStateId::new(99),
        })
    );
    assert_eq!(
        spanner.inverse_tokens(LexerState::Start, SequenceId::new(u32::MAX)),
        Err(SpannerQueryError::InvalidSequence {
            sequence: SequenceId::new(u32::MAX),
        })
    );
}
