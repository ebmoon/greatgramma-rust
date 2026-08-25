//! Proof-oriented semantic kernel for GreatGramma.

#![forbid(unsafe_code)]

mod engine;
mod error;
mod ids;
mod lalr;
mod lalr_reference;
mod lexer;
mod limits;
mod mask;
mod normalized;
mod preprocess;
mod sequence;
mod spanner;
mod token_step;
mod validate;

pub use engine::{AdvanceResult, EngineError, Matcher, PreparedGrammar, prepare};
pub use error::{ArithmeticKind, IdKind, LimitKind, ValidationError, ValidationTable};
pub use ids::{
    DfaStateId, NonterminalId, ParserStateId, ProductionId, SequenceId, TerminalId, TokenId,
};
pub use lalr::{ParserError, ParserExecution, execute_terminals};
pub use lalr_reference::execute_sequence_head;
pub use lexer::{LexerError, LexerInput, LexerState, LexerStep, lexer_step};
pub use limits::{PreparationLimits, ValidationLimits};
pub use normalized::{
    Action, LalrDimensions, LalrTable, LexerDfa, Production, TokenEntry, UnvalidatedGrammar,
    ValidatedGrammar, ValidatedLalr, ValidatedLexer,
};
pub use preprocess::{
    PreparedParser, SequenceClassification, classify_sequence_head, prepare_parser,
};
pub use spanner::{PreparedSpanner, SpannerQueryError, prepare_spanner};
pub use token_step::{PreparationError, TokenExecution, TokenStepError, execute_token};
