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

mod compile;
mod compile_error;
mod dfa;
mod diagnostics;
mod engine;
mod error;
mod grammar;
mod ids;
mod lalr;
mod lalr_reference;
mod lexer;
mod limits;
mod lr;
mod mask;
mod normalized;
mod preprocess;
mod regex;
mod sequence;
mod source;
mod spanner;
mod token_step;
mod tokenizer;
mod tokenizer_json;
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

pub use compile::{CompileLimits, compile, normalize};
pub use compile_error::{CompileError, TokenizerError};
pub use source::{SourceGrammar, TerminalSpec};
pub use tokenizer::{TokenSpec, TokenizerManifest};
pub use tokenizer_json::{CompatibilityReport, TokenizerJsonLimits};
