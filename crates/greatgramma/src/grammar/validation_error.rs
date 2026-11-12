use std::{error::Error, fmt};

use crate::{DfaStateId, ParserStateId, TerminalId, TokenId};

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
    ParserIgnoredTerminals,
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
    IgnoredTerminalCount,
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
    AcceptingLexerStart {
        state: DfaStateId,
        terminal: TerminalId,
    },
    LexerTerminalIsParserEof {
        state: DfaStateId,
        terminal: TerminalId,
    },
    IgnoredTerminalIsParserEof {
        index: usize,
        terminal: TerminalId,
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

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTokenTable => f.write_str("the token table is empty"),
            Self::MissingOrdinaryToken => f.write_str("the token table has no ordinary token"),
            Self::MissingEosToken => f.write_str("the token table has no EOS token"),
            Self::EmptyTokenBytes { token } => {
                write!(f, "ordinary token {} has no bytes", token.get())
            }
            Self::WrongTableLength {
                table,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "{} has {actual} entries, expected {expected}",
                    table_name(*table)
                )
            }
            Self::IdOutOfRange {
                table,
                index,
                kind,
                id,
                count,
            } => {
                write!(f, "{}", table_name(*table))?;
                if let Some(index) = index {
                    write!(f, " entry {index}")?;
                }
                write!(
                    f,
                    " references {} {id}, but the count is {count}",
                    id_name(*kind)
                )
            }
            Self::ByteClassOutOfRange {
                byte,
                class,
                class_count,
            } => write!(
                f,
                "byte 0x{byte:02x} maps to class {class}, but the class count is {class_count}"
            ),
            Self::AcceptingLexerStart { state, terminal } => write!(
                f,
                "lexer start state {} accepts terminal {}, allowing an empty lexeme",
                state.get(),
                terminal.get()
            ),
            Self::LexerTerminalIsParserEof { state, terminal } => write!(
                f,
                "lexer state {} uses parser EOF terminal {} as a lexeme",
                state.get(),
                terminal.get()
            ),
            Self::IgnoredTerminalIsParserEof { index, terminal } => write!(
                f,
                "ignored terminal entry {index} refers to parser EOF terminal {}",
                terminal.get()
            ),
            Self::AcceptOnNonEof { state, terminal } => write!(
                f,
                "parser state {} accepts non-EOF terminal {}",
                state.get(),
                terminal.get()
            ),
            Self::ArithmeticOverflow { calculation } => {
                write!(
                    f,
                    "overflow while calculating {}",
                    calculation_name(*calculation)
                )
            }
            Self::LimitExceeded {
                limit,
                actual,
                maximum,
            } => {
                write!(
                    f,
                    "{} is {actual}, maximum is {maximum}",
                    limit_name(*limit)
                )
            }
            Self::AllocationFailure { table, requested } => {
                write!(
                    f,
                    "allocation failed for {requested} entries in {}",
                    table_name(*table)
                )
            }
        }
    }
}

impl Error for ValidationError {}

fn table_name(table: ValidationTable) -> &'static str {
    match table {
        ValidationTable::Tokens => "token table",
        ValidationTable::LexerByteClasses => "lexer byte-class table",
        ValidationTable::LexerTransitions => "lexer transition table",
        ValidationTable::LexerStart => "lexer start state",
        ValidationTable::LexerTerminals => "lexer terminal table",
        ValidationTable::ParserActions => "parser action table",
        ValidationTable::ParserGotos => "parser goto table",
        ValidationTable::ParserStart => "parser start state",
        ValidationTable::ParserEof => "parser EOF terminal",
        ValidationTable::ParserIgnoredTerminals => "ignored terminal table",
        ValidationTable::Productions => "production table",
    }
}

fn id_name(kind: IdKind) -> &'static str {
    match kind {
        IdKind::DfaState => "lexer state",
        IdKind::Terminal => "terminal",
        IdKind::ParserState => "parser state",
        IdKind::Nonterminal => "nonterminal",
        IdKind::Production => "production",
    }
}

fn calculation_name(kind: ArithmeticKind) -> &'static str {
    match kind {
        ArithmeticKind::TokenCount => "the token count",
        ArithmeticKind::TokenBytes => "token bytes",
        ArithmeticKind::ProductionCount => "the production count",
        ArithmeticKind::IgnoredTerminalCount => "the ignored terminal count",
        ArithmeticKind::LexerCells => "lexer table cells",
        ArithmeticKind::ParserActionCells => "parser action cells",
        ArithmeticKind::ParserGotoCells => "parser goto cells",
        ArithmeticKind::ParserCells => "parser table cells",
        ArithmeticKind::LogicalBytes => "logical storage bytes",
        ArithmeticKind::Work => "validation work",
    }
}

fn limit_name(kind: LimitKind) -> &'static str {
    match kind {
        LimitKind::Tokens => "token count",
        LimitKind::TokenBytes => "token byte count",
        LimitKind::DfaStates => "lexer state count",
        LimitKind::DfaClasses => "lexer class count",
        LimitKind::DfaCells => "lexer table cell count",
        LimitKind::ParserStates => "parser state count",
        LimitKind::Terminals => "terminal count",
        LimitKind::Nonterminals => "nonterminal count",
        LimitKind::Productions => "production count",
        LimitKind::ParserCells => "parser table cell count",
        LimitKind::LogicalBytes => "logical storage size in bytes",
        LimitKind::Work => "validation work",
    }
}
