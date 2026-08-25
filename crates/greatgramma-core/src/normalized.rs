use crate::{
    DfaStateId, NonterminalId, ParserStateId, ProductionId, TerminalId, TokenId, ValidationError,
    ValidationLimits,
};

/// One model-token entry in the normalized vocabulary.
#[derive(Debug, Eq, PartialEq)]
pub enum TokenEntry {
    Bytes(Vec<u8>),
    Eos,
}

/// Caller-owned normalized lexer data. Its contents are untrusted until validation.
#[derive(Debug, Eq, PartialEq)]
pub struct LexerDfa {
    pub(crate) state_count: u32,
    pub(crate) class_count: u32,
    pub(crate) byte_classes: Vec<u32>,
    pub(crate) transitions: Vec<Option<DfaStateId>>,
    pub(crate) start_state: DfaStateId,
    pub(crate) terminals: Vec<Option<TerminalId>>,
}

impl LexerDfa {
    #[must_use]
    pub fn new(
        state_count: u32,
        class_count: u32,
        byte_classes: Vec<u32>,
        transitions: Vec<Option<DfaStateId>>,
        start_state: DfaStateId,
        terminals: Vec<Option<TerminalId>>,
    ) -> Self {
        Self {
            state_count,
            class_count,
            byte_classes,
            transitions,
            start_state,
            terminals,
        }
    }
}

/// Dimensions of a normalized LALR table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LalrDimensions {
    pub(crate) state_count: u32,
    pub(crate) terminal_count: u32,
    pub(crate) nonterminal_count: u32,
}

impl LalrDimensions {
    #[must_use]
    pub const fn new(state_count: u32, terminal_count: u32, nonterminal_count: u32) -> Self {
        Self {
            state_count,
            terminal_count,
            nonterminal_count,
        }
    }

    #[must_use]
    pub const fn state_count(self) -> u32 {
        self.state_count
    }

    #[must_use]
    pub const fn terminal_count(self) -> u32 {
        self.terminal_count
    }

    #[must_use]
    pub const fn nonterminal_count(self) -> u32 {
        self.nonterminal_count
    }
}

/// One normalized LALR action cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Error,
    Shift(ParserStateId),
    Reduce { production: ProductionId, rank: u32 },
    Accept,
}

/// One normalized LALR production.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Production {
    lhs: NonterminalId,
    pop_len: u32,
}

impl Production {
    #[must_use]
    pub const fn new(lhs: NonterminalId, pop_len: u32) -> Self {
        Self { lhs, pop_len }
    }

    #[must_use]
    pub const fn lhs(self) -> NonterminalId {
        self.lhs
    }

    #[must_use]
    pub const fn pop_len(self) -> u32 {
        self.pop_len
    }
}

/// Caller-owned normalized parser data. Its contents are untrusted until validation.
#[derive(Debug, Eq, PartialEq)]
pub struct LalrTable {
    pub(crate) dimensions: LalrDimensions,
    pub(crate) start_state: ParserStateId,
    pub(crate) eof_terminal: TerminalId,
    pub(crate) actions: Vec<Action>,
    pub(crate) gotos: Vec<Option<ParserStateId>>,
    pub(crate) productions: Vec<Production>,
    pub(crate) ignored_terminals: Vec<TerminalId>,
}

impl LalrTable {
    #[must_use]
    pub fn new(
        dimensions: LalrDimensions,
        start_state: ParserStateId,
        eof_terminal: TerminalId,
        actions: Vec<Action>,
        gotos: Vec<Option<ParserStateId>>,
        productions: Vec<Production>,
    ) -> Self {
        Self {
            dimensions,
            start_state,
            eof_terminal,
            actions,
            gotos,
            productions,
            ignored_terminals: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_ignored_terminals(mut self, ignored_terminals: Vec<TerminalId>) -> Self {
        self.ignored_terminals = ignored_terminals;
        self
    }
}

/// The public, caller-owned input to the normalized validation boundary.
#[derive(Debug, Eq, PartialEq)]
pub struct UnvalidatedGrammar {
    pub(crate) tokens: Vec<TokenEntry>,
    pub(crate) lexer: LexerDfa,
    pub(crate) lalr: LalrTable,
}

impl UnvalidatedGrammar {
    #[must_use]
    pub fn new(tokens: Vec<TokenEntry>, lexer: LexerDfa, lalr: LalrTable) -> Self {
        Self {
            tokens,
            lexer,
            lalr,
        }
    }

    /// Consumes untrusted input and returns an opaque value only after every
    /// finite structural check succeeds.
    pub fn validate(self, limits: ValidationLimits) -> Result<ValidatedGrammar, ValidationError> {
        crate::validate::validate(self, limits)
    }
}

/// Owned normalized grammar whose dimensions and references have been checked.
#[derive(Debug, Eq, PartialEq)]
pub struct ValidatedGrammar {
    token_count: u32,
    tokens: Vec<TokenEntry>,
    lexer: ValidatedLexer,
    lalr: ValidatedLalr,
}

impl ValidatedGrammar {
    pub(crate) fn from_parts(
        token_count: u32,
        tokens: Vec<TokenEntry>,
        lexer: ValidatedLexer,
        lalr: ValidatedLalr,
    ) -> Self {
        Self {
            token_count,
            tokens,
            lexer,
            lalr,
        }
    }

    #[must_use]
    pub const fn token_count(&self) -> u32 {
        self.token_count
    }

    #[must_use]
    pub fn tokens(&self) -> &[TokenEntry] {
        &self.tokens
    }

    #[must_use]
    pub fn token(&self, token: TokenId) -> Option<&TokenEntry> {
        let index = usize::try_from(token.get()).ok()?;
        self.tokens.get(index)
    }

    #[must_use]
    pub const fn lexer(&self) -> &ValidatedLexer {
        &self.lexer
    }

    #[must_use]
    pub const fn lalr(&self) -> &ValidatedLalr {
        &self.lalr
    }
}

/// Checked lexer data with read-only lookup operations.
#[derive(Debug, Eq, PartialEq)]
pub struct ValidatedLexer {
    state_count: u32,
    class_count: u32,
    byte_classes: Vec<u32>,
    transitions: Vec<Option<DfaStateId>>,
    start_state: DfaStateId,
    terminals: Vec<Option<TerminalId>>,
}

impl ValidatedLexer {
    pub(crate) fn from_unvalidated(lexer: LexerDfa) -> Self {
        Self {
            state_count: lexer.state_count,
            class_count: lexer.class_count,
            byte_classes: lexer.byte_classes,
            transitions: lexer.transitions,
            start_state: lexer.start_state,
            terminals: lexer.terminals,
        }
    }

    #[must_use]
    pub const fn state_count(&self) -> u32 {
        self.state_count
    }

    #[must_use]
    pub const fn class_count(&self) -> u32 {
        self.class_count
    }

    #[must_use]
    pub const fn start_state(&self) -> DfaStateId {
        self.start_state
    }

    #[must_use]
    pub fn byte_class(&self, byte: u8) -> u32 {
        self.byte_classes[usize::from(byte)]
    }

    /// Returns `None` for an invalid source state and `Some(None)` for a
    /// checked state whose byte transition is absent.
    #[must_use]
    pub fn transition(&self, state: DfaStateId, byte: u8) -> Option<Option<DfaStateId>> {
        let state_index = self.state_index(state)?;
        let class = usize::try_from(self.byte_class(byte)).ok()?;
        let class_count = usize::try_from(self.class_count).ok()?;
        let row_start = state_index.checked_mul(class_count)?;
        self.transitions.get(row_start.checked_add(class)?).copied()
    }

    /// Returns `None` for an invalid source state and the state's optional
    /// priority-resolved terminal otherwise.
    #[must_use]
    pub fn terminal(&self, state: DfaStateId) -> Option<Option<TerminalId>> {
        self.terminals.get(self.state_index(state)?).copied()
    }

    fn state_index(&self, state: DfaStateId) -> Option<usize> {
        if state.get() >= self.state_count {
            return None;
        }
        usize::try_from(state.get()).ok()
    }
}

/// Checked LALR data with read-only lookup operations.
#[derive(Debug, Eq, PartialEq)]
pub struct ValidatedLalr {
    dimensions: LalrDimensions,
    start_state: ParserStateId,
    eof_terminal: TerminalId,
    actions: Vec<Action>,
    gotos: Vec<Option<ParserStateId>>,
    production_count: u32,
    productions: Vec<Production>,
    ignored_terminals: Vec<u8>,
}

impl ValidatedLalr {
    pub(crate) fn from_unvalidated(
        lalr: LalrTable,
        production_count: u32,
        ignored_terminals: Vec<u8>,
    ) -> Self {
        Self {
            dimensions: lalr.dimensions,
            start_state: lalr.start_state,
            eof_terminal: lalr.eof_terminal,
            actions: lalr.actions,
            gotos: lalr.gotos,
            production_count,
            productions: lalr.productions,
            ignored_terminals,
        }
    }

    #[must_use]
    pub const fn state_count(&self) -> u32 {
        self.dimensions.state_count
    }

    #[must_use]
    pub const fn terminal_count(&self) -> u32 {
        self.dimensions.terminal_count
    }

    #[must_use]
    pub const fn nonterminal_count(&self) -> u32 {
        self.dimensions.nonterminal_count
    }

    #[must_use]
    pub const fn production_count(&self) -> u32 {
        self.production_count
    }

    #[must_use]
    pub const fn start_state(&self) -> ParserStateId {
        self.start_state
    }

    #[must_use]
    pub const fn eof_terminal(&self) -> TerminalId {
        self.eof_terminal
    }

    /// Returns `None` for an invalid terminal and whether a checked terminal
    /// is a parser no-op otherwise.
    #[must_use]
    pub fn is_ignored(&self, terminal: TerminalId) -> Option<bool> {
        let ignored = self.ignored_terminals.get(self.terminal_index(terminal)?)?;
        Some(*ignored != 0)
    }

    #[must_use]
    pub fn action(&self, state: ParserStateId, terminal: TerminalId) -> Option<Action> {
        let state_index = self.state_index(state)?;
        let terminal_index = self.terminal_index(terminal)?;
        let width = usize::try_from(self.terminal_count()).ok()?;
        let row_start = state_index.checked_mul(width)?;
        self.actions
            .get(row_start.checked_add(terminal_index)?)
            .copied()
    }

    /// Returns `None` for an invalid row/column and `Some(None)` for an absent
    /// goto in a checked cell.
    #[must_use]
    pub fn goto(
        &self,
        state: ParserStateId,
        nonterminal: NonterminalId,
    ) -> Option<Option<ParserStateId>> {
        let state_index = self.state_index(state)?;
        let nonterminal_index = self.nonterminal_index(nonterminal)?;
        let width = usize::try_from(self.nonterminal_count()).ok()?;
        let row_start = state_index.checked_mul(width)?;
        self.gotos
            .get(row_start.checked_add(nonterminal_index)?)
            .copied()
    }

    #[must_use]
    pub fn production(&self, production: ProductionId) -> Option<Production> {
        let index = usize::try_from(production.get()).ok()?;
        self.productions.get(index).copied()
    }

    fn state_index(&self, state: ParserStateId) -> Option<usize> {
        checked_index(state.get(), self.state_count())
    }

    fn terminal_index(&self, terminal: TerminalId) -> Option<usize> {
        checked_index(terminal.get(), self.terminal_count())
    }

    fn nonterminal_index(&self, nonterminal: NonterminalId) -> Option<usize> {
        checked_index(nonterminal.get(), self.nonterminal_count())
    }
}

fn checked_index(id: u32, count: u32) -> Option<usize> {
    if id >= count {
        return None;
    }
    usize::try_from(id).ok()
}
