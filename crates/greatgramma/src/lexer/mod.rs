//! Byte-regex compilation, DFA construction, and byte/EOS execution.
pub(crate) mod dfa;
mod execute;
pub(crate) mod regex;

pub use execute::{LexerError, LexerInput, LexerState, LexerStep, lexer_step};
