//! Prepared grammars, packed masks, and atomic single-row and batch advancement.
mod engine;
mod mask;

pub use engine::{AdvanceResult, EngineError, Matcher, PreparedGrammar, prepare};
