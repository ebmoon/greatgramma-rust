use crate::{ParserStateId, TerminalId, TokenId};

/// Identifies a normalized table or scalar field in a validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationTable {
    Tokens,
    LexerByteClasses,
    LexerTransitions,
    LexerStart,
    LexerTerminals,
    ParserActions,
    ParserGotos,
    ParserStart,
    ParserEof,
    Productions,
}

/// Identifies the domain of an out-of-range checked ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdKind {
    DfaState,
    Terminal,
    ParserState,
    Nonterminal,
    Production,
}

/// Identifies a checked size calculation that overflowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithmeticKind {
    TokenCount,
    TokenBytes,
    ProductionCount,
    LexerCells,
    ParserActionCells,
    ParserGotoCells,
    ParserCells,
    LogicalBytes,
    Work,
}

/// Identifies the finite budget exceeded by normalized input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitKind {
    Tokens,
    TokenBytes,
    DfaStates,
    DfaClasses,
    DfaCells,
    ParserStates,
    Terminals,
    Nonterminals,
    Productions,
    ParserCells,
    LogicalBytes,
    Work,
}

/// A fail-closed structural validation error for caller-owned normalized data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    EmptyTokenTable,
    MissingOrdinaryToken,
    MissingEosToken,
    EmptyTokenBytes {
        token: TokenId,
    },
    WrongTableLength {
        table: ValidationTable,
        expected: usize,
        actual: usize,
    },
    IdOutOfRange {
        table: ValidationTable,
        index: Option<usize>,
        kind: IdKind,
        id: u32,
        count: u32,
    },
    ByteClassOutOfRange {
        byte: u8,
        class: u32,
        class_count: u32,
    },
    AcceptOnNonEof {
        state: ParserStateId,
        terminal: TerminalId,
    },
    ArithmeticOverflow {
        calculation: ArithmeticKind,
    },
    LimitExceeded {
        limit: LimitKind,
        actual: u64,
        maximum: u64,
    },
    AllocationFailure {
        table: ValidationTable,
        requested: usize,
    },
}
