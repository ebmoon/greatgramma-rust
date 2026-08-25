//! Owned Yacc, byte-regex, and tokenizer-manifest compiler for GreatGramma.

#![forbid(unsafe_code)]

mod compile;
mod dfa;
mod error;
mod grammar;
mod lr;
mod regex;
mod source;
mod tokenizer;
mod tokenizer_json;

pub use compile::{CompileLimits, compile, normalize};
pub use error::{CompileError, TokenizerError};
pub use source::{SourceGrammar, TerminalSpec};
pub use tokenizer::{TokenSpec, TokenizerManifest};
pub use tokenizer_json::{CompatibilityReport, TokenizerJsonLimits};
