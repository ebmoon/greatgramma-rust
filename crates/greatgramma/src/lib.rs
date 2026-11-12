//! Grammar-constrained decoding: compile source grammars, prepare token masks,
//! and advance fixed-row matchers through one Rust API.
//!
//! ```
//! use greatgramma::{
//!     compile, CompileLimits, SourceGrammar, TerminalSpec, TokenId, TokenSpec,
//!     TokenizerManifest,
//! };
//!
//! let source = SourceGrammar::new(
//!     "%start S\n%token ITEM\n%%\nS: ITEM;\n",
//!     vec![TerminalSpec::new("ITEM", "i+", 0)],
//! );
//! let tokenizer = TokenizerManifest::new(vec![
//!     TokenSpec::Bytes(b"i".to_vec()),
//!     TokenSpec::Eos,
//! ]);
//! let mut matcher = compile(source, tokenizer, CompileLimits::default())?
//!     .into_matcher(1)?;
//! assert!(matcher.allows(0, TokenId::new(0))?);
//! matcher.advance(0, TokenId::new(0))?;
//! matcher.advance(0, TokenId::new(1))?;
//! assert!(matcher.is_completed(0)?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]

mod grammar;
mod lexer;
mod matcher;
mod parser;
mod preparation;
mod tokenizer;

pub use grammar::{
    Action, ArithmeticKind, CompileError, CompileLimits, DfaStateId, IdKind, LalrDimensions,
    LalrTable, LexerDfa, LimitKind, NonterminalId, ParserStateId, Production, ProductionId,
    SequenceId, SourceGrammar, TerminalId, TerminalSpec, TokenEntry, TokenId, UnvalidatedGrammar,
    ValidatedGrammar, ValidatedLalr, ValidatedLexer, ValidationError, ValidationLimits,
    ValidationTable, compile, normalize,
};
pub use lexer::{LexerError, LexerInput, LexerState, LexerStep, lexer_step};
pub use matcher::{AdvanceResult, EngineError, Matcher, PreparedGrammar, prepare};
pub use parser::{
    ParserError, ParserExecution, PreparedParser, SequenceClassification, classify_sequence_head,
    execute_sequence_head, execute_terminals, prepare_parser,
};
pub use preparation::{
    PreparationError, PreparationLimits, PreparedSpanner, SpannerQueryError, prepare_spanner,
};
pub use tokenizer::{
    CompatibilityReport, TokenExecution, TokenSpec, TokenStepError, TokenizerError,
    TokenizerJsonLimits, TokenizerManifest, execute_token,
};
