//! Proof-oriented semantic kernel for GreatGramma.

#![forbid(unsafe_code)]

mod error;
mod ids;
mod lexer;
mod limits;
mod normalized;
mod validate;

pub use error::{ArithmeticKind, IdKind, LimitKind, ValidationError, ValidationTable};
pub use ids::{DfaStateId, NonterminalId, ParserStateId, ProductionId, TerminalId, TokenId};
pub use lexer::{LexerError, LexerInput, LexerState, LexerStep, lexer_step};
pub use limits::ValidationLimits;
pub use normalized::{
    Action, LalrDimensions, LalrTable, LexerDfa, Production, TokenEntry, UnvalidatedGrammar,
    ValidatedGrammar, ValidatedLalr, ValidatedLexer,
};
