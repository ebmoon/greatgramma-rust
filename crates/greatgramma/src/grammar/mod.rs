//! Source grammar compilation, normalized tables, and structural validation.
mod compile;
mod error;
mod ids;
mod limits;
mod source;
mod tables;
mod validate;
mod validation_error;

pub use compile::{CompileLimits, compile, normalize};
pub use error::CompileError;
pub use ids::{
    DfaStateId, NonterminalId, ParserStateId, ProductionId, SequenceId, TerminalId, TokenId,
};
pub use limits::ValidationLimits;
pub use source::{SourceGrammar, TerminalSpec};
pub use tables::{
    Action, LalrDimensions, LalrTable, LexerDfa, Production, TokenEntry, UnvalidatedGrammar,
    ValidatedGrammar, ValidatedLalr, ValidatedLexer,
};
pub use validation_error::{ArithmeticKind, IdKind, LimitKind, ValidationError, ValidationTable};
