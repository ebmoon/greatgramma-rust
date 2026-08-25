#![forbid(unsafe_code)]

use std::collections::{BTreeSet, VecDeque};

const CASES: &str = include_str!("../fixtures/conformance/boundary_eos_v1.tsv");
const MUTATION_WITNESSES: &str = include_str!("../fixtures/conformance/mutation_witnesses_v1.tsv");

#[derive(Clone, Debug, Eq, PartialEq)]
enum RawToken {
    Bytes(Vec<u8>),
    Eos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RawAction {
    Error,
    Shift(usize),
    Reduce { production: usize, rank: u32 },
    Accept,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawProduction {
    lhs: usize,
    pop_len: usize,
}

#[derive(Clone, Debug)]
struct RawLexer {
    start: usize,
    transitions: Vec<Option<usize>>,
    terminals: Vec<Option<usize>>,
}

impl RawLexer {
    fn state_count(&self) -> usize {
        self.terminals.len()
    }

    fn transition(&self, state: usize, byte: u8) -> Option<usize> {
        self.transitions[state * 256 + usize::from(byte)]
    }
}

#[derive(Clone, Debug)]
struct RawParser {
    start: usize,
    eof: usize,
    terminal_count: usize,
    nonterminal_count: usize,
    actions: Vec<RawAction>,
    gotos: Vec<Option<usize>>,
    productions: Vec<RawProduction>,
}

impl RawParser {
    fn state_count(&self) -> usize {
        self.actions.len() / self.terminal_count
    }

    fn action(&self, state: usize, terminal: usize) -> RawAction {
        self.actions[state * self.terminal_count + terminal]
    }

    fn goto(&self, state: usize, nonterminal: usize) -> Option<usize> {
        self.gotos[state * self.nonterminal_count + nonterminal]
    }
}

#[derive(Clone, Debug)]
struct Fixture {
    tokens: Vec<RawToken>,
    lexer: RawLexer,
    parser: RawParser,
    first_eos: usize,
}

fn fixture() -> Fixture {
    const ITEM: usize = 0;
    const CLOSE: usize = 1;
    const EOF: usize = 2;

    let mut transitions = vec![None; 6 * 256];
    let mut edge = |source: usize, byte: u8, destination: usize| {
        transitions[source * 256 + usize::from(byte)] = Some(destination);
    };
    edge(0, b'i', 1);
    edge(0, b'c', 2);
    edge(0, b'u', 3);
    edge(0, 0, 4);
    edge(0, 0xff, 5);
    edge(3, b'i', 1);

    let mut actions = vec![RawAction::Error; 4 * 3];
    actions[ITEM] = RawAction::Shift(1);
    actions[3 + CLOSE] = RawAction::Shift(2);
    actions[6 + EOF] = RawAction::Reduce {
        production: 0,
        rank: 1,
    };
    actions[9 + EOF] = RawAction::Accept;

    let mut gotos = vec![None; 4];
    gotos[0] = Some(3);

    Fixture {
        tokens: vec![
            RawToken::Bytes(vec![b'i']),
            RawToken::Bytes(vec![b'c']),
            RawToken::Bytes(vec![b'i', b'c']),
            RawToken::Eos,
            RawToken::Eos,
            RawToken::Bytes(vec![0]),
            RawToken::Bytes(vec![0xff]),
            RawToken::Bytes(vec![b'u']),
            RawToken::Bytes(vec![b'u', b'i']),
            RawToken::Bytes(vec![b'x']),
        ],
        lexer: RawLexer {
            start: 0,
            transitions,
            terminals: vec![None, Some(ITEM), Some(CLOSE), None, Some(ITEM), Some(ITEM)],
        },
        parser: RawParser {
            start: 0,
            eof: EOF,
            terminal_count: 3,
            nonterminal_count: 1,
            actions,
            gotos,
            productions: vec![RawProduction { lhs: 0, pop_len: 2 }],
        },
        first_eos: 3,
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum LexPosition {
    LogicalStart,
    Dfa(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OracleStatus {
    Running {
        lexer: LexPosition,
        parser_stack: Vec<usize>,
    },
    Accepted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Continue,
    Accepted,
    Rejected,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Value(bool),
    Completed,
}

impl Step {
    fn availability(self) -> Availability {
        match self {
            Self::Continue | Self::Accepted => Availability::Value(true),
            Self::Rejected => Availability::Value(false),
            Self::Completed => Availability::Completed,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    DropBoundaryReconsume,
    DropEosFlush,
    OnlyFirstEos,
    AcceptCleanEos,
    NulTerminator,
    Utf8Lossy,
}

impl Mutation {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "drop-boundary-reconsume" => Some(Self::DropBoundaryReconsume),
            "drop-eos-flush" => Some(Self::DropEosFlush),
            "only-first-eos" => Some(Self::OnlyFirstEos),
            "accept-clean-eos" => Some(Self::AcceptCleanEos),
            "nul-terminator" => Some(Self::NulTerminator),
            "utf8-lossy" => Some(Self::Utf8Lossy),
            _ => None,
        }
    }
}

// This module is the oracle. It deliberately has no greatgramma_core imports.
// It operates only on the raw fixture representation declared above.
mod oracle {
    use super::*;

    #[derive(Clone, Debug)]
    pub(super) struct Oracle<'a> {
        fixture: &'a Fixture,
        status: OracleStatus,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ParseEvent {
        Shifted,
        Accepted,
        Rejected,
    }

    #[derive(Clone, Copy, Debug)]
    struct LexEvent {
        state: LexPosition,
        emitted: Option<usize>,
    }

    impl<'a> Oracle<'a> {
        pub(super) fn new(fixture: &'a Fixture) -> Self {
            Self {
                fixture,
                status: OracleStatus::Running {
                    lexer: LexPosition::LogicalStart,
                    parser_stack: vec![fixture.parser.start],
                },
            }
        }

        pub(super) fn advance(&mut self, token: usize, mutation: Option<Mutation>) -> Step {
            let OracleStatus::Running {
                lexer,
                parser_stack,
            } = self.status.clone()
            else {
                return Step::Completed;
            };
            let Some(entry) = self.fixture.tokens.get(token) else {
                return Step::Rejected;
            };

            match entry {
                RawToken::Eos => self.advance_eos(token, lexer, parser_stack, mutation),
                RawToken::Bytes(bytes) => self.advance_bytes(lexer, parser_stack, bytes, mutation),
            }
        }

        fn advance_bytes(
            &mut self,
            lexer: LexPosition,
            parser_stack: Vec<usize>,
            raw_bytes: &[u8],
            mutation: Option<Mutation>,
        ) -> Step {
            let bytes = mutated_bytes(raw_bytes, mutation);
            let Some((next_lexer, emitted)) =
                compose_bytes(&self.fixture.lexer, lexer, &bytes, mutation)
            else {
                return Step::Rejected;
            };

            if !ordinary_allowed(self.fixture, &parser_stack, &emitted, next_lexer, mutation) {
                return Step::Rejected;
            }

            let mut next_stack = parser_stack;
            if !commit_terminals(&self.fixture.parser, &mut next_stack, &emitted) {
                return Step::Rejected;
            }
            self.status = OracleStatus::Running {
                lexer: next_lexer,
                parser_stack: next_stack,
            };
            Step::Continue
        }

        fn advance_eos(
            &mut self,
            token: usize,
            lexer: LexPosition,
            mut parser_stack: Vec<usize>,
            mutation: Option<Mutation>,
        ) -> Step {
            if mutation == Some(Mutation::OnlyFirstEos) && token != self.fixture.first_eos {
                return Step::Rejected;
            }
            if mutation == Some(Mutation::AcceptCleanEos) && lexer == LexPosition::LogicalStart {
                self.status = OracleStatus::Accepted;
                return Step::Accepted;
            }

            let residual = match lexer {
                LexPosition::LogicalStart => None,
                LexPosition::Dfa(state) => match self.fixture.lexer.terminals[state] {
                    Some(terminal) => Some(terminal),
                    None => return Step::Rejected,
                },
            };
            if mutation != Some(Mutation::DropEosFlush) {
                if let Some(terminal) = residual {
                    if feed_terminal(&self.fixture.parser, &mut parser_stack, terminal)
                        != ParseEvent::Shifted
                    {
                        return Step::Rejected;
                    }
                }
            }
            if feed_terminal(
                &self.fixture.parser,
                &mut parser_stack,
                self.fixture.parser.eof,
            ) != ParseEvent::Accepted
            {
                return Step::Rejected;
            }
            self.status = OracleStatus::Accepted;
            Step::Accepted
        }
    }

    fn mutated_bytes(bytes: &[u8], mutation: Option<Mutation>) -> Vec<u8> {
        match mutation {
            Some(Mutation::NulTerminator) => bytes
                .iter()
                .copied()
                .take_while(|byte| *byte != 0)
                .collect(),
            Some(Mutation::Utf8Lossy) => String::from_utf8_lossy(bytes).as_bytes().to_vec(),
            _ => bytes.to_vec(),
        }
    }

    fn compose_bytes(
        lexer: &RawLexer,
        source: LexPosition,
        bytes: &[u8],
        mutation: Option<Mutation>,
    ) -> Option<(LexPosition, Vec<usize>)> {
        let mut state = source;
        let mut emitted = Vec::new();
        for &byte in bytes {
            let event = byte_step(lexer, state, byte, mutation)?;
            if let Some(terminal) = event.emitted {
                emitted.push(terminal);
            }
            state = event.state;
        }
        Some((state, emitted))
    }

    fn byte_step(
        lexer: &RawLexer,
        source: LexPosition,
        byte: u8,
        mutation: Option<Mutation>,
    ) -> Option<LexEvent> {
        match source {
            LexPosition::LogicalStart => {
                lexer
                    .transition(lexer.start, byte)
                    .map(|destination| LexEvent {
                        state: LexPosition::Dfa(destination),
                        emitted: None,
                    })
            }
            LexPosition::Dfa(state) => {
                if let Some(destination) = lexer.transition(state, byte) {
                    return Some(LexEvent {
                        state: LexPosition::Dfa(destination),
                        emitted: None,
                    });
                }
                let terminal = lexer.terminals[state]?;
                if mutation == Some(Mutation::DropBoundaryReconsume) {
                    return Some(LexEvent {
                        state: LexPosition::LogicalStart,
                        emitted: Some(terminal),
                    });
                }
                lexer
                    .transition(lexer.start, byte)
                    .map(|destination| LexEvent {
                        state: LexPosition::Dfa(destination),
                        emitted: Some(terminal),
                    })
            }
        }
    }

    fn ordinary_allowed(
        fixture: &Fixture,
        stack: &[usize],
        direct: &[usize],
        residual: LexPosition,
        mutation: Option<Mutation>,
    ) -> bool {
        next_terminals(&fixture.lexer, residual, mutation)
            .into_iter()
            .any(|continuation| probe_head(&fixture.parser, stack, direct, continuation))
    }

    fn next_terminals(
        lexer: &RawLexer,
        source: LexPosition,
        mutation: Option<Mutation>,
    ) -> BTreeSet<usize> {
        let mut terminals = BTreeSet::new();
        let mut queue = VecDeque::from([source]);
        let mut seen = BTreeSet::from([source]);

        while let Some(state) = queue.pop_front() {
            for byte in 0_u16..=255 {
                let byte = u8::try_from(byte).expect("bounded byte");
                let Some(event) = byte_step(lexer, state, byte, mutation) else {
                    continue;
                };
                if let Some(terminal) = event.emitted {
                    terminals.insert(terminal);
                } else if seen.insert(event.state) {
                    queue.push_back(event.state);
                }
            }
        }
        terminals
    }

    fn probe_head(
        parser: &RawParser,
        source: &[usize],
        direct: &[usize],
        continuation: usize,
    ) -> bool {
        let mut stack = source.to_vec();
        if !commit_terminals(parser, &mut stack, direct) {
            return false;
        }
        feed_terminal(parser, &mut stack, continuation) == ParseEvent::Shifted
    }

    fn commit_terminals(parser: &RawParser, stack: &mut Vec<usize>, terminals: &[usize]) -> bool {
        terminals
            .iter()
            .all(|&terminal| feed_terminal(parser, stack, terminal) == ParseEvent::Shifted)
    }

    fn feed_terminal(parser: &RawParser, stack: &mut Vec<usize>, terminal: usize) -> ParseEvent {
        loop {
            let Some(&state) = stack.last() else {
                return ParseEvent::Rejected;
            };
            match parser.action(state, terminal) {
                RawAction::Error => return ParseEvent::Rejected,
                RawAction::Shift(destination) => {
                    stack.push(destination);
                    return ParseEvent::Shifted;
                }
                RawAction::Accept => return ParseEvent::Accepted,
                RawAction::Reduce { production, .. } => {
                    let Some(rule) = parser.productions.get(production) else {
                        return ParseEvent::Rejected;
                    };
                    if rule.pop_len >= stack.len() {
                        return ParseEvent::Rejected;
                    }
                    stack.truncate(stack.len() - rule.pop_len);
                    let Some(&source) = stack.last() else {
                        return ParseEvent::Rejected;
                    };
                    let Some(destination) = parser.goto(source, rule.lhs) else {
                        return ParseEvent::Rejected;
                    };
                    stack.push(destination);
                }
            }
        }
    }
}

// This adapter is intentionally separate from `oracle`: it is the only module
// that observes the production engine.
mod production {
    use super::*;
    use greatgramma_core::{
        Action, AdvanceResult, DfaStateId, EngineError, LalrDimensions, LalrTable, LexerDfa,
        Matcher, NonterminalId, ParserStateId, PreparationLimits, Production, ProductionId,
        TerminalId, TokenEntry, TokenId, UnvalidatedGrammar, ValidationLimits, prepare,
    };

    pub(super) struct Driver {
        matcher: Matcher,
    }

    impl Driver {
        pub(super) fn new(raw: &Fixture) -> Self {
            let lexer = LexerDfa::new(
                u32::try_from(raw.lexer.state_count()).expect("fixture state count"),
                256,
                (0_u32..256).collect(),
                raw.lexer
                    .transitions
                    .iter()
                    .map(|destination| {
                        destination.map(|state| {
                            DfaStateId::new(u32::try_from(state).expect("fixture DFA state"))
                        })
                    })
                    .collect(),
                DfaStateId::new(u32::try_from(raw.lexer.start).expect("fixture DFA start")),
                raw.lexer
                    .terminals
                    .iter()
                    .map(|terminal| {
                        terminal.map(|value| {
                            TerminalId::new(u32::try_from(value).expect("fixture terminal"))
                        })
                    })
                    .collect(),
            );
            let parser = LalrTable::new(
                LalrDimensions::new(
                    u32::try_from(raw.parser.state_count()).expect("fixture parser states"),
                    u32::try_from(raw.parser.terminal_count).expect("fixture terminals"),
                    u32::try_from(raw.parser.nonterminal_count).expect("fixture nonterminals"),
                ),
                ParserStateId::new(u32::try_from(raw.parser.start).expect("fixture parser start")),
                TerminalId::new(u32::try_from(raw.parser.eof).expect("fixture EOF")),
                raw.parser
                    .actions
                    .iter()
                    .map(|action| match *action {
                        RawAction::Error => Action::Error,
                        RawAction::Shift(state) => Action::Shift(ParserStateId::new(
                            u32::try_from(state).expect("fixture shift state"),
                        )),
                        RawAction::Reduce { production, rank } => Action::Reduce {
                            production: ProductionId::new(
                                u32::try_from(production).expect("fixture production"),
                            ),
                            rank,
                        },
                        RawAction::Accept => Action::Accept,
                    })
                    .collect(),
                raw.parser
                    .gotos
                    .iter()
                    .map(|destination| {
                        destination.map(|state| {
                            ParserStateId::new(u32::try_from(state).expect("fixture goto state"))
                        })
                    })
                    .collect(),
                raw.parser
                    .productions
                    .iter()
                    .map(|production| {
                        Production::new(
                            NonterminalId::new(u32::try_from(production.lhs).expect("fixture lhs")),
                            u32::try_from(production.pop_len).expect("fixture pop length"),
                        )
                    })
                    .collect(),
            );
            let tokens = raw
                .tokens
                .iter()
                .map(|token| match token {
                    RawToken::Bytes(bytes) => TokenEntry::Bytes(bytes.clone()),
                    RawToken::Eos => TokenEntry::Eos,
                })
                .collect();
            let grammar = UnvalidatedGrammar::new(tokens, lexer, parser)
                .validate(ValidationLimits::default())
                .expect("conformance fixture validates");
            let matcher = prepare(grammar, PreparationLimits::default())
                .expect("conformance fixture prepares")
                .into_matcher(1)
                .expect("one-row matcher");
            Self { matcher }
        }

        pub(super) fn allows(&self, token: usize) -> Availability {
            let token = TokenId::new(u32::try_from(token).expect("fixture token"));
            match self.matcher.allows(0, token) {
                Ok(value) => Availability::Value(value),
                Err(EngineError::Completed) => Availability::Completed,
                Err(error) => panic!("unexpected allows error: {error:?}"),
            }
        }

        pub(super) fn mask_allows(&mut self, token: usize) -> Availability {
            let mut mask = vec![0xff; self.matcher.mask_bytes()];
            match self.matcher.mask(0, &mut mask) {
                Ok(()) => {
                    let byte = token / 8;
                    let bit = token % 8;
                    Availability::Value(mask[byte] & (1_u8 << bit) != 0)
                }
                Err(EngineError::NoValidToken) => Availability::Value(false),
                Err(EngineError::Completed) => Availability::Completed,
                Err(error) => panic!("unexpected mask error: {error:?}"),
            }
        }

        pub(super) fn advance(&mut self, token: usize) -> Step {
            let token = TokenId::new(u32::try_from(token).expect("fixture token"));
            match self.matcher.advance(0, token) {
                Ok(AdvanceResult::Continue) => Step::Continue,
                Ok(AdvanceResult::Accepted) => Step::Accepted,
                Err(EngineError::ConstraintViolation { .. }) => Step::Rejected,
                Err(EngineError::Completed) => Step::Completed,
                Err(error) => panic!("unexpected advance error: {error:?}"),
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Case {
    name: String,
    tokens: Vec<usize>,
    expected: String,
}

fn cases() -> Vec<Case> {
    CASES
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("name\t"))
        .map(|line| {
            let mut columns = line.split('\t');
            let name = columns.next().expect("case name").to_owned();
            let tokens = columns
                .next()
                .expect("case token list")
                .split(',')
                .map(|value| value.parse().expect("numeric token ID"))
                .collect();
            let expected = columns.next().expect("case result").to_owned();
            assert!(columns.next().is_none(), "extra case column in {name}");
            Case {
                name,
                tokens,
                expected,
            }
        })
        .collect()
}

fn trace(raw: &Fixture, tokens: &[usize], mutation: Option<Mutation>) -> Vec<Step> {
    let mut machine = oracle::Oracle::new(raw);
    let mut trace = Vec::new();
    for &token in tokens {
        let step = machine.advance(token, mutation);
        trace.push(step);
        if matches!(step, Step::Rejected | Step::Completed) {
            break;
        }
    }
    trace
}

fn outcome(trace: &[Step]) -> String {
    for (index, step) in trace.iter().enumerate() {
        match step {
            Step::Rejected => return format!("rejected@{index}"),
            Step::Completed => return format!("completed@{index}"),
            Step::Continue | Step::Accepted => {}
        }
    }
    match trace.last() {
        Some(Step::Accepted) => "accepted".to_owned(),
        Some(Step::Continue) => "continue".to_owned(),
        Some(Step::Rejected | Step::Completed) => unreachable!("returned above"),
        None => "empty".to_owned(),
    }
}

fn compare_production(raw: &Fixture, tokens: &[usize], label: &str) -> Step {
    let mut expected = oracle::Oracle::new(raw);
    let mut actual = production::Driver::new(raw);
    let mut last = Step::Continue;

    for (index, &token) in tokens.iter().enumerate() {
        let oracle_step = expected.advance(token, None);
        let availability = oracle_step.availability();
        assert_eq!(
            actual.allows(token),
            availability,
            "allows mismatch for {label} at token position {index} (ID {token})"
        );
        assert_eq!(
            actual.mask_allows(token),
            availability,
            "mask mismatch for {label} at token position {index} (ID {token})"
        );
        let production_step = actual.advance(token);
        assert_eq!(
            production_step, oracle_step,
            "advance mismatch for {label} at token position {index} (ID {token})"
        );
        last = oracle_step;
        if matches!(last, Step::Rejected | Step::Completed) {
            break;
        }
    }
    last
}

#[test]
fn curated_traces_match_provenance_and_production() {
    let raw = fixture();
    for case in cases() {
        let expected_trace = trace(&raw, &case.tokens, None);
        assert_eq!(
            outcome(&expected_trace),
            case.expected,
            "fixture expectation drifted for {}",
            case.name
        );
        compare_production(&raw, &case.tokens, &case.name);
    }
}

#[test]
fn bounded_live_histories_match_production() {
    fn walk(raw: &Fixture, prefix: &mut Vec<usize>, remaining: usize, prior: Step) {
        if remaining == 0 || matches!(prior, Step::Rejected | Step::Completed) {
            return;
        }
        for token in 0..raw.tokens.len() {
            prefix.push(token);
            let label = format!("bounded history {prefix:?}");
            let step = compare_production(raw, prefix, &label);
            walk(raw, prefix, remaining - 1, step);
            prefix.pop();
        }
    }

    let raw = fixture();
    walk(&raw, &mut Vec::new(), 3, Step::Continue);
}

#[test]
fn every_deliberate_mutation_has_a_named_witness() {
    let raw = fixture();
    let cases = cases();
    for line in MUTATION_WITNESSES.lines().filter(|line| {
        !line.is_empty() && !line.starts_with('#') && !line.starts_with("mutation\t")
    }) {
        let mut columns = line.split('\t');
        let mutation_name = columns.next().expect("mutation name");
        let witness_name = columns.next().expect("mutation witness");
        assert!(columns.next().is_none(), "extra mutation witness column");
        let mutation = Mutation::parse(mutation_name)
            .unwrap_or_else(|| panic!("unknown mutation {mutation_name}"));
        let witness = cases
            .iter()
            .find(|case| case.name == witness_name)
            .unwrap_or_else(|| panic!("unknown witness case {witness_name}"));
        let correct = trace(&raw, &witness.tokens, None);
        let mutated = trace(&raw, &witness.tokens, Some(mutation));
        assert_ne!(
            mutated, correct,
            "mutation {mutation_name} survived witness {witness_name}"
        );
    }
}
