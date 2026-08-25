#![no_main]
#![forbid(unsafe_code)]

use greatgramma_core::{
    Action, DfaStateId, LalrDimensions, LalrTable, LexerDfa, NonterminalId, ParserStateId,
    PreparationLimits, Production, ProductionId, TerminalId, TokenEntry, TokenId,
    UnvalidatedGrammar, ValidationLimits, prepare,
};
use libfuzzer_sys::fuzz_target;

const MAX_TOKENS: usize = 8;
const MAX_TOKEN_BYTES: usize = 8;
const MAX_DFA_STATES: u32 = 4;
const MAX_DFA_CLASSES: u32 = 4;
const MAX_PARSER_STATES: u32 = 4;
const MAX_TERMINALS: u32 = 4;
const MAX_NONTERMINALS: u32 = 3;
const MAX_PRODUCTIONS: usize = 6;
const MAX_RUNTIME_STEPS: usize = 16;

fuzz_target!(|data: &[u8]| {
    let mut input = Input::new(data);
    let grammar = match input.byte() % 4 {
        0 => canonical_grammar(),
        1 => shaped_grammar(&mut input, true),
        2 => shaped_grammar(&mut input, false),
        _ => malformed_grammar(&mut input),
    };
    exercise(grammar, &mut input);
});

fn exercise(grammar: UnvalidatedGrammar, input: &mut Input<'_>) {
    let Ok(validated) = grammar.validate(fuzz_validation_limits()) else {
        return;
    };
    let Ok(prepared) = prepare(
        validated,
        PreparationLimits {
            max_items: 4_096,
            max_work: 20_000,
        },
    ) else {
        return;
    };

    let rows = 1 + usize::from(input.byte() % 3);
    let Ok(mut matcher) = prepared.into_matcher(rows) else {
        return;
    };
    let mut accepted = vec![false; rows];

    let mut step = 0_usize;
    while step < MAX_RUNTIME_STEPS {
        let mask_bytes = matcher.mask_bytes();
        let total_mask_bytes = rows
            .checked_mul(mask_bytes)
            .expect("fuzz dimensions are bounded");
        let mut current_masks = vec![0; total_mask_bytes];
        let mut tokens = vec![None; rows];
        let mut blocked = false;
        let mut active_rows = 0_usize;
        let mut row = 0_usize;
        while row < rows {
            if accepted[row] {
                assert_eq!(matcher.is_completed(row), Ok(true));
                row += 1;
                continue;
            }

            let start = row * mask_bytes;
            let end = start + mask_bytes;
            let mask = &mut current_masks[start..end];
            mask.fill(0xa5);
            match matcher.mask(row, mask) {
                Ok(()) => {
                    assert_mask_matches_allows(&matcher, row, mask);
                }
                Err(_) => {
                    assert!(mask.iter().all(|byte| *byte == 0));
                    blocked = true;
                    break;
                }
            }

            assert_failed_mask_is_zero(&mut matcher, row);

            if input.byte() & 7 == 0 {
                let invalid = TokenId::new(matcher.token_count().saturating_add(1));
                let before = mask.to_vec();
                assert!(matcher.allows(row, invalid).is_err());
                assert!(matcher.advance(row, invalid).is_err());
                let mut after = vec![0xff; matcher.mask_bytes()];
                matcher
                    .mask(row, &mut after)
                    .expect("an invalid token must not mutate the matcher");
                assert_eq!(after, before);
            }

            let allowed = allowed_tokens(mask, matcher.token_count());
            assert!(!allowed.is_empty());
            let choice = (usize::from(input.byte()) + step + row) % allowed.len();
            let token = allowed[choice];
            assert_eq!(matcher.allows(row, token), Ok(true));
            tokens[row] = Some(token);
            active_rows += 1;
            row += 1;
        }

        if blocked {
            let mut all_rows = vec![0x5a; total_mask_bytes];
            assert!(matcher.masks(&mut all_rows).is_err());
            assert!(all_rows.iter().all(|byte| *byte == 0));
            break;
        }

        if active_rows == 0 {
            break;
        }

        if accepted.iter().all(|is_accepted| !is_accepted) {
            let mut all_rows = vec![0xa5; total_mask_bytes];
            matcher
                .masks(&mut all_rows)
                .expect("individual live masks imply a live batch mask");
            assert_eq!(all_rows, current_masks);
        }

        let mut successor_masks = vec![0xa5; total_mask_bytes];
        match matcher.advance_active_and_masks_no_result(&tokens, &mut successor_masks) {
            Ok(()) => {
                row = 0;
                while row < rows {
                    let start = row * mask_bytes;
                    let end = start + mask_bytes;
                    let successor = &successor_masks[start..end];
                    if matcher.is_completed(row) == Ok(true) {
                        accepted[row] = true;
                        assert!(successor.iter().all(|byte| *byte == 0));
                    } else {
                        assert_mask_matches_allows(&matcher, row, successor);
                    }
                    row += 1;
                }
            }
            Err(_) => {
                assert!(successor_masks.iter().all(|byte| *byte == 0));
                row = 0;
                while row < rows {
                    if accepted[row] {
                        assert_eq!(matcher.is_completed(row), Ok(true));
                    } else {
                        let start = row * mask_bytes;
                        let end = start + mask_bytes;
                        let mut after = vec![0xff; mask_bytes];
                        matcher
                            .mask(row, &mut after)
                            .expect("failed atomic advancement must retain each live row");
                        assert_eq!(after, current_masks[start..end]);
                    }
                    row += 1;
                }
                break;
            }
        }
        step += 1;
    }

    let mut invalid_row_output = vec![0x5a; matcher.mask_bytes()];
    assert!(matcher.mask(rows, &mut invalid_row_output).is_err());
    assert!(invalid_row_output.iter().all(|byte| *byte == 0));
}

fn assert_mask_matches_allows(matcher: &greatgramma_core::Matcher, row: usize, mask: &[u8]) {
    let mut any = false;
    let mut token_index = 0_u32;
    while token_index < matcher.token_count() {
        let token = TokenId::new(token_index);
        let set = mask_bit(mask, token_index);
        assert_eq!(matcher.allows(row, token), Ok(set));
        any |= set;
        token_index += 1;
    }
    assert!(
        any,
        "a successful mask must constrain to a nonempty token set"
    );

    let tail_bits = matcher.token_count() % 8;
    if tail_bits != 0 {
        let used = (1_u16 << tail_bits) - 1;
        let tail = u16::from(*mask.last().expect("a nonempty vocabulary has a mask byte"));
        assert_eq!(tail & !used, 0, "out-of-vocabulary tail bits were exposed");
    }
}

fn assert_failed_mask_is_zero(matcher: &mut greatgramma_core::Matcher, row: usize) {
    if matcher.mask_bytes() == 0 {
        return;
    }
    let mut short = vec![0x5a; matcher.mask_bytes() - 1];
    assert!(matcher.mask(row, &mut short).is_err());
    assert!(short.iter().all(|byte| *byte == 0));
}

fn allowed_tokens(mask: &[u8], token_count: u32) -> Vec<TokenId> {
    let mut tokens = Vec::new();
    let mut token = 0_u32;
    while token < token_count {
        if mask_bit(mask, token) {
            tokens.push(TokenId::new(token));
        }
        token += 1;
    }
    tokens
}

fn mask_bit(mask: &[u8], token: u32) -> bool {
    let byte = usize::try_from(token / 8).expect("bounded token index fits usize");
    let bit = u8::try_from(token % 8).expect("bit index fits u8");
    mask.get(byte).copied().unwrap_or(0) & (1_u8 << bit) != 0
}

fn canonical_grammar() -> UnvalidatedGrammar {
    let mut byte_classes = vec![0; 256];
    byte_classes[usize::from(b'a')] = 1;
    let lexer = LexerDfa::new(
        2,
        2,
        byte_classes,
        vec![None, Some(DfaStateId::new(1)), None, None],
        DfaStateId::new(0),
        vec![None, Some(TerminalId::new(0))],
    );
    let lalr = LalrTable::new(
        LalrDimensions::new(2, 2, 0),
        ParserStateId::new(0),
        TerminalId::new(1),
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
        vec![TokenEntry::Bytes(vec![b'a']), TokenEntry::Eos],
        lexer,
        lalr,
    )
}

fn shaped_grammar(input: &mut Input<'_>, bounded_ids: bool) -> UnvalidatedGrammar {
    let dfa_states = 2 + u32::from(input.byte()) % (MAX_DFA_STATES - 1);
    let dfa_classes = 1 + u32::from(input.byte()) % MAX_DFA_CLASSES;
    let parser_states = 1 + u32::from(input.byte()) % MAX_PARSER_STATES;
    let terminal_count = 2 + u32::from(input.byte()) % (MAX_TERMINALS - 1);
    let nonterminal_count = u32::from(input.byte()) % (MAX_NONTERMINALS + 1);
    let eof = terminal_count - 1;

    let ordinary_count = 1 + usize::from(input.byte()) % (MAX_TOKENS - 1);
    let mut tokens = Vec::with_capacity(ordinary_count + 1);
    let mut index = 0_usize;
    while index < ordinary_count {
        let len = 1 + usize::from(input.byte()) % MAX_TOKEN_BYTES;
        tokens.push(TokenEntry::Bytes(input.bytes(len)));
        index += 1;
    }
    tokens.push(TokenEntry::Eos);

    let mut byte_classes = Vec::with_capacity(256);
    index = 0;
    while index < 256 {
        let value = input.u32();
        byte_classes.push(if bounded_ids {
            value % dfa_classes
        } else {
            value
        });
        index += 1;
    }

    let transition_count = usize::try_from(dfa_states * dfa_classes).expect("bounded dimensions");
    let mut transitions = Vec::with_capacity(transition_count);
    index = 0;
    while index < transition_count {
        transitions.push(
            input
                .optional_id(dfa_states, bounded_ids)
                .map(DfaStateId::new),
        );
        index += 1;
    }

    let mut terminals = vec![None; usize::try_from(dfa_states).expect("bounded dimensions")];
    terminals[1] = Some(TerminalId::new(0));
    index = 2;
    while index < terminals.len() {
        terminals[index] = input.optional_id(eof, bounded_ids).map(TerminalId::new);
        index += 1;
    }
    let lexer = LexerDfa::new(
        dfa_states,
        dfa_classes,
        byte_classes,
        transitions,
        DfaStateId::new(0),
        terminals,
    );

    let production_count = if nonterminal_count == 0 {
        0
    } else {
        usize::from(input.byte()) % (MAX_PRODUCTIONS + 1)
    };
    let mut productions = Vec::with_capacity(production_count);
    index = 0;
    while index < production_count {
        let lhs = input.id(nonterminal_count, bounded_ids);
        productions.push(Production::new(
            NonterminalId::new(lhs),
            u32::from(input.byte() % 6),
        ));
        index += 1;
    }

    let action_count = usize::try_from(parser_states * terminal_count).expect("bounded dimensions");
    let mut actions = Vec::with_capacity(action_count);
    index = 0;
    while index < action_count {
        let column = u32::try_from(index).expect("bounded index") % terminal_count;
        actions.push(input.action(parser_states, production_count, column == eof, bounded_ids));
        index += 1;
    }

    let goto_count =
        usize::try_from(parser_states * nonterminal_count).expect("bounded dimensions");
    let mut gotos = Vec::with_capacity(goto_count);
    index = 0;
    while index < goto_count {
        gotos.push(
            input
                .optional_id(parser_states, bounded_ids)
                .map(ParserStateId::new),
        );
        index += 1;
    }

    let ignored_count = usize::from(input.byte() % 4);
    let mut ignored = Vec::with_capacity(ignored_count);
    index = 0;
    while index < ignored_count {
        ignored.push(TerminalId::new(input.id(eof, bounded_ids)));
        index += 1;
    }
    let lalr = LalrTable::new(
        LalrDimensions::new(parser_states, terminal_count, nonterminal_count),
        ParserStateId::new(input.id(parser_states, bounded_ids)),
        TerminalId::new(eof),
        actions,
        gotos,
        productions,
    )
    .with_ignored_terminals(ignored);

    UnvalidatedGrammar::new(tokens, lexer, lalr)
}

fn malformed_grammar(input: &mut Input<'_>) -> UnvalidatedGrammar {
    let dfa_states = input.declared_count(MAX_DFA_STATES);
    let dfa_classes = input.declared_count(MAX_DFA_CLASSES);
    let parser_states = input.declared_count(MAX_PARSER_STATES);
    let terminal_count = input.declared_count(MAX_TERMINALS);
    let nonterminal_count = input.declared_count(MAX_NONTERMINALS);

    let token_count = usize::from(input.byte()) % (MAX_TOKENS + 1);
    let mut tokens = Vec::with_capacity(token_count);
    let mut index = 0_usize;
    while index < token_count {
        if input.byte() & 3 == 0 {
            tokens.push(TokenEntry::Eos);
        } else {
            let len = usize::from(input.byte()) % (MAX_TOKEN_BYTES + 1);
            tokens.push(TokenEntry::Bytes(input.bytes(len)));
        }
        index += 1;
    }

    let byte_class_count = if input.byte() & 1 == 0 {
        256
    } else {
        usize::from(input.byte()) % 17
    };
    let mut byte_classes = Vec::with_capacity(byte_class_count);
    index = 0;
    while index < byte_class_count {
        byte_classes.push(input.u32());
        index += 1;
    }

    let transitions = input.optional_dfa_ids(20);
    let terminal_states = input.optional_terminal_ids(8);
    let lexer = LexerDfa::new(
        dfa_states,
        dfa_classes,
        byte_classes,
        transitions,
        DfaStateId::new(input.u32()),
        terminal_states,
    );

    let action_count = usize::from(input.byte()) % 25;
    let mut actions = Vec::with_capacity(action_count);
    index = 0;
    while index < action_count {
        actions.push(input.unbounded_action());
        index += 1;
    }
    let gotos = input.optional_parser_ids(20);
    let production_count = usize::from(input.byte()) % (MAX_PRODUCTIONS + 1);
    let mut productions = Vec::with_capacity(production_count);
    index = 0;
    while index < production_count {
        productions.push(Production::new(
            NonterminalId::new(input.u32()),
            input.u32(),
        ));
        index += 1;
    }
    let ignored_count = usize::from(input.byte() % 7);
    let mut ignored = Vec::with_capacity(ignored_count);
    index = 0;
    while index < ignored_count {
        ignored.push(TerminalId::new(input.u32()));
        index += 1;
    }
    let lalr = LalrTable::new(
        LalrDimensions::new(parser_states, terminal_count, nonterminal_count),
        ParserStateId::new(input.u32()),
        TerminalId::new(input.u32()),
        actions,
        gotos,
        productions,
    )
    .with_ignored_terminals(ignored);
    UnvalidatedGrammar::new(tokens, lexer, lalr)
}

fn fuzz_validation_limits() -> ValidationLimits {
    ValidationLimits {
        max_tokens: MAX_TOKENS as u64,
        max_token_bytes: (MAX_TOKENS * MAX_TOKEN_BYTES) as u64,
        max_dfa_states: u64::from(MAX_DFA_STATES),
        max_dfa_classes: u64::from(MAX_DFA_CLASSES),
        max_dfa_cells: u64::from(MAX_DFA_STATES * MAX_DFA_CLASSES),
        max_parser_states: u64::from(MAX_PARSER_STATES),
        max_terminals: u64::from(MAX_TERMINALS),
        max_nonterminals: u64::from(MAX_NONTERMINALS),
        max_productions: MAX_PRODUCTIONS as u64,
        max_parser_cells: 32,
        max_logical_bytes: 4_096,
        max_work: 4_096,
    }
}

struct Input<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Input<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn byte(&mut self) -> u8 {
        let value = self.data.get(self.offset).copied().unwrap_or(0);
        self.offset = self.offset.saturating_add(1);
        value
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(len);
        let mut index = 0_usize;
        while index < len {
            bytes.push(self.byte());
            index += 1;
        }
        bytes
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }

    fn declared_count(&mut self, maximum: u32) -> u32 {
        let value = self.u32();
        if value & 7 == 7 {
            u32::MAX
        } else {
            value % (maximum + 1)
        }
    }

    fn id(&mut self, count: u32, bounded: bool) -> u32 {
        let value = self.u32();
        if bounded && count != 0 {
            value % count
        } else {
            value
        }
    }

    fn optional_id(&mut self, count: u32, bounded: bool) -> Option<u32> {
        if self.byte() & 1 == 0 {
            None
        } else {
            Some(self.id(count, bounded))
        }
    }

    fn action(
        &mut self,
        state_count: u32,
        production_count: usize,
        is_eof: bool,
        bounded: bool,
    ) -> Action {
        match self.byte() % 4 {
            0 => Action::Error,
            1 => Action::Shift(ParserStateId::new(self.id(state_count, bounded))),
            2 if production_count != 0 => Action::Reduce {
                production: ProductionId::new(self.id(
                    u32::try_from(production_count).expect("bounded production count"),
                    bounded,
                )),
                rank: self.u32(),
            },
            3 if is_eof => Action::Accept,
            _ => Action::Error,
        }
    }

    fn unbounded_action(&mut self) -> Action {
        match self.byte() % 4 {
            0 => Action::Error,
            1 => Action::Shift(ParserStateId::new(self.u32())),
            2 => Action::Reduce {
                production: ProductionId::new(self.u32()),
                rank: self.u32(),
            },
            _ => Action::Accept,
        }
    }

    fn optional_dfa_ids(&mut self, maximum: usize) -> Vec<Option<DfaStateId>> {
        let count = usize::from(self.byte()) % (maximum + 1);
        let mut values = Vec::with_capacity(count);
        let mut index = 0_usize;
        while index < count {
            values.push(self.optional_id(0, false).map(DfaStateId::new));
            index += 1;
        }
        values
    }

    fn optional_terminal_ids(&mut self, maximum: usize) -> Vec<Option<TerminalId>> {
        let count = usize::from(self.byte()) % (maximum + 1);
        let mut values = Vec::with_capacity(count);
        let mut index = 0_usize;
        while index < count {
            values.push(self.optional_id(0, false).map(TerminalId::new));
            index += 1;
        }
        values
    }

    fn optional_parser_ids(&mut self, maximum: usize) -> Vec<Option<ParserStateId>> {
        let count = usize::from(self.byte()) % (maximum + 1);
        let mut values = Vec::with_capacity(count);
        let mut index = 0_usize;
        while index < count {
            values.push(self.optional_id(0, false).map(ParserStateId::new));
            index += 1;
        }
        values
    }
}
