use greatgramma_core::{
    Action, AdvanceResult, DfaStateId, EngineError, LalrDimensions, LalrTable, LexerDfa, Matcher,
    ParserStateId, PreparationLimits, TerminalId, TokenEntry, TokenId, UnvalidatedGrammar,
    ValidationLimits, prepare,
};

const ITEM: TerminalId = TerminalId::new(0);
const EOF: TerminalId = TerminalId::new(1);

fn grammar() -> greatgramma_core::ValidatedGrammar {
    let mut classes = vec![0; 256];
    classes[usize::from(b'a')] = 1;
    let lexer = LexerDfa::new(
        2,
        2,
        classes,
        vec![None, Some(DfaStateId::new(1)), None, None],
        DfaStateId::new(0),
        vec![None, Some(ITEM)],
    );
    let lalr = LalrTable::new(
        LalrDimensions::new(2, 2, 0),
        ParserStateId::new(0),
        EOF,
        vec![
            Action::Shift(ParserStateId::new(1)),
            Action::Error,
            Action::Shift(ParserStateId::new(1)),
            Action::Accept,
        ],
        Vec::new(),
        Vec::new(),
    );
    UnvalidatedGrammar::new(
        vec![
            TokenEntry::Bytes(b"a".to_vec()),
            TokenEntry::Bytes(b"aa".to_vec()),
            TokenEntry::Bytes(b"x".to_vec()),
            TokenEntry::Eos,
            TokenEntry::Bytes(b"a".to_vec()),
        ],
        lexer,
        lalr,
    )
    .validate(ValidationLimits::default())
    .expect("fixture validates")
}

fn matcher_after(history: &[TokenId]) -> Matcher {
    let mut matcher = prepare(grammar(), PreparationLimits::default())
        .expect("preparation succeeds")
        .into_matcher(1)
        .expect("matcher allocation succeeds");
    let mut index = 0_usize;
    while index < history.len() {
        assert_eq!(
            matcher.advance(0, history[index]),
            Ok(AdvanceResult::Continue),
            "recursive history contains only continuing tokens",
        );
        index += 1;
    }
    matcher
}

fn assert_equivalence(history: &mut Vec<TokenId>, depth: usize) {
    let mut matcher = matcher_after(history);
    let mut packed = vec![0xff; matcher.mask_bytes()];
    matcher.mask(0, &mut packed).expect("fixture is live");

    let mut token_index = 0_u32;
    while token_index < matcher.token_count() {
        let token = TokenId::new(token_index);
        let byte = usize::try_from(token_index / 8).expect("fixture byte fits");
        let bit = u8::try_from(token_index % 8).expect("fixture bit fits");
        let masked = packed[byte] & (1_u8 << bit) != 0;
        let allowed = matcher.allows(0, token).expect("valid token ID");
        let mut candidate = matcher_after(history);
        let advanced = candidate.advance(0, token);
        assert_eq!(masked, allowed, "mask/allows token {token_index}");
        assert_eq!(masked, advanced.is_ok(), "mask/advance token {token_index}");

        if depth > 0 {
            match advanced {
                Ok(AdvanceResult::Continue) => {
                    history.push(token);
                    assert_equivalence(history, depth - 1);
                    history.pop();
                }
                Ok(AdvanceResult::Accepted) => {
                    assert_eq!(candidate.is_completed(0), Ok(true));
                    assert_eq!(
                        candidate.allows(0, TokenId::new(0)),
                        Err(EngineError::Completed)
                    );
                }
                Err(EngineError::ConstraintViolation { token: rejected }) => {
                    assert_eq!(rejected, token);
                }
                Err(error) => panic!("unexpected fixture error: {error:?}"),
            }
        }
        token_index += 1;
    }

    let tail_bits = matcher.token_count() % 8;
    if tail_bits != 0 {
        let tail = *packed.last().expect("nonempty fixture mask");
        let used = (1_u16 << tail_bits) - 1;
        assert_eq!(u16::from(tail) & !used, 0, "unused tail bits must be zero");
    }
}

#[test]
fn mask_allows_and_advance_agree_on_every_bounded_history() {
    assert_equivalence(&mut Vec::new(), 3);
}
