//! Yacc/LALR table construction, parser execution, and lookahead classification.
pub(crate) mod compile;
pub(crate) mod execute;
pub(crate) mod lookahead;
pub(crate) mod preprocess;
pub(crate) mod yacc;

pub use execute::{ParserError, ParserExecution, execute_terminals};
pub use lookahead::execute_sequence_head;
pub use preprocess::{
    PreparedParser, SequenceClassification, classify_sequence_head, prepare_parser,
};
