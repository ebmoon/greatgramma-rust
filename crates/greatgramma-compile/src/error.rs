use std::{error::Error, fmt};

use greatgramma_core::{PreparationError, ValidationError};

/// Exact tokenizer-manifest failures, distinct from grammar compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenizerError {
    EmptyManifest,
    EmptyOrdinaryToken { token: usize },
    MissingOrdinaryToken,
    MissingEos,
    JsonTooLarge { bytes: usize, maximum: usize },
    DecodedOutputTooLarge { bytes: usize, maximum: usize },
    WorkLimit { required: u64, maximum: u64 },
    InvalidJson { message: String },
    IncompatibleJson { reason: String },
    MissingSingletonByte { byte: u8 },
}

/// Fail-closed compiler diagnostics. Third-party error types never cross this boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileError {
    Tokenizer(TokenizerError),
    GrammarSyntax {
        messages: Vec<String>,
    },
    UnsupportedGrammar {
        feature: String,
    },
    GrammarTable {
        message: String,
    },
    GrammarConflict {
        shift_reduce: usize,
        reduce_reduce: usize,
    },
    SourceTooLarge {
        bytes: usize,
        maximum: usize,
    },
    TooManyTerminalSpecs {
        count: usize,
        maximum: usize,
    },
    TooManyIgnoredTerminals {
        count: usize,
        maximum: usize,
    },
    RegexSourceTooLarge {
        bytes: usize,
        maximum: usize,
    },
    RegexOutputLimit {
        required: usize,
        maximum: usize,
    },
    MissingTerminal {
        name: String,
    },
    UnknownTerminal {
        name: String,
    },
    DuplicateTerminal {
        name: String,
    },
    UnknownIgnoredTerminal {
        name: String,
    },
    DuplicateIgnoredTerminal {
        name: String,
    },
    IgnoredTerminalUsed {
        name: String,
    },
    UnsupportedRegex {
        name: String,
        feature: String,
    },
    InvalidRegex {
        name: String,
        message: String,
    },
    NullableTerminal {
        name: String,
    },
    UnmatchableTerminal {
        name: String,
    },
    AmbiguousPriority {
        first: String,
        second: String,
        priority: u32,
    },
    TooManyDfaStates {
        required: usize,
        maximum: usize,
    },
    UnsupportedLexerBoundary {
        state: u32,
        byte: u8,
    },
    TooManyParserCells {
        required: u64,
        maximum: u64,
    },
    TooManyParserStates {
        required: usize,
        maximum: u64,
    },
    GrammarTooLarge {
        terminals: u64,
        nonterminals: u64,
        productions: u64,
    },
    CompilerWorkLimit {
        required: u64,
        maximum: u64,
    },
    AllocationFailure {
        requested: usize,
    },
    NoReductionProgressWitness,
    Validation(ValidationError),
    Preparation(PreparationError),
}

impl fmt::Display for TokenizerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyManifest => write!(formatter, "the token manifest is empty"),
            Self::EmptyOrdinaryToken { token } => {
                write!(formatter, "ordinary token {token} has no bytes")
            }
            Self::MissingOrdinaryToken => write!(formatter, "the manifest has no ordinary token"),
            Self::MissingEos => write!(formatter, "the manifest has no EOS token"),
            Self::JsonTooLarge { bytes, maximum } => {
                write!(
                    formatter,
                    "tokenizer JSON has {bytes} bytes, maximum is {maximum}"
                )
            }
            Self::DecodedOutputTooLarge { bytes, maximum } => write!(
                formatter,
                "decoded tokenizer bytes need {bytes} bytes, maximum is {maximum}"
            ),
            Self::WorkLimit { required, maximum } => write!(
                formatter,
                "tokenizer extraction needs more than {maximum} work units (stopped at {required})"
            ),
            Self::InvalidJson { message } => write!(formatter, "invalid tokenizer JSON: {message}"),
            Self::IncompatibleJson { reason } => {
                write!(formatter, "unsupported tokenizer JSON: {reason}")
            }
            Self::MissingSingletonByte { byte } => write!(
                formatter,
                "tokenizer JSON has no ordinary singleton token for byte 0x{byte:02x}"
            ),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tokenizer(error) => write!(formatter, "tokenizer manifest: {error}"),
            Self::GrammarSyntax { messages } => {
                write!(formatter, "invalid Yacc grammar: {}", messages.join("; "))
            }
            Self::UnsupportedGrammar { feature } => {
                write!(formatter, "unsupported grammar feature: {feature}")
            }
            Self::GrammarTable { message } => write!(formatter, "LALR table: {message}"),
            Self::GrammarConflict {
                shift_reduce,
                reduce_reduce,
            } => write!(
                formatter,
                "grammar has {shift_reduce} shift/reduce and {reduce_reduce} reduce/reduce conflicts"
            ),
            Self::SourceTooLarge { bytes, maximum } => {
                write!(
                    formatter,
                    "grammar source has {bytes} bytes, maximum is {maximum}"
                )
            }
            Self::TooManyTerminalSpecs { count, maximum } => write!(
                formatter,
                "grammar has {count} terminal specifications, maximum is {maximum}"
            ),
            Self::TooManyIgnoredTerminals { count, maximum } => write!(
                formatter,
                "grammar has {count} ignored terminals, maximum is {maximum}"
            ),
            Self::RegexSourceTooLarge { bytes, maximum } => write!(
                formatter,
                "terminal regexes have {bytes} source bytes, maximum is {maximum}"
            ),
            Self::RegexOutputLimit { required, maximum } => write!(
                formatter,
                "compiled regexes need {required} bytes, maximum is {maximum}"
            ),
            Self::MissingTerminal { name } => write!(formatter, "missing terminal spec for {name}"),
            Self::UnknownTerminal { name } => write!(formatter, "unknown terminal spec {name}"),
            Self::DuplicateTerminal { name } => write!(formatter, "duplicate terminal spec {name}"),
            Self::UnknownIgnoredTerminal { name } => {
                write!(formatter, "unknown ignored terminal {name}")
            }
            Self::DuplicateIgnoredTerminal { name } => {
                write!(formatter, "duplicate ignored terminal {name}")
            }
            Self::IgnoredTerminalUsed { name } => {
                write!(formatter, "ignored terminal {name} is used by a production")
            }
            Self::UnsupportedRegex { name, feature } => {
                write!(
                    formatter,
                    "terminal {name} uses unsupported regex feature {feature}"
                )
            }
            Self::InvalidRegex { name, message } => {
                write!(formatter, "invalid regex for terminal {name}: {message}")
            }
            Self::NullableTerminal { name } => write!(formatter, "terminal {name} is nullable"),
            Self::UnmatchableTerminal { name } => {
                write!(formatter, "terminal {name} cannot match any bytes")
            }
            Self::AmbiguousPriority {
                first,
                second,
                priority,
            } => write!(
                formatter,
                "terminals {first} and {second} both accept at priority {priority}"
            ),
            Self::TooManyDfaStates { required, maximum } => {
                write!(
                    formatter,
                    "lexer DFA needs {required} states, maximum is {maximum}"
                )
            }
            Self::UnsupportedLexerBoundary { state, byte } => write!(
                formatter,
                "lexer state {state} needs last-accept rollback after byte 0x{byte:02x}"
            ),
            Self::TooManyParserCells { required, maximum } => {
                write!(
                    formatter,
                    "LALR table needs {required} cells, maximum is {maximum}"
                )
            }
            Self::TooManyParserStates { required, maximum } => write!(
                formatter,
                "LR parser needs {required} states, maximum is {maximum}"
            ),
            Self::GrammarTooLarge {
                terminals,
                nonterminals,
                productions,
            } => write!(
                formatter,
                "grammar dimensions exceed limits ({terminals} terminals, {nonterminals} nonterminals, {productions} productions)"
            ),
            Self::CompilerWorkLimit { required, maximum } => write!(
                formatter,
                "compiler needs more than {maximum} work units (stopped at {required})"
            ),
            Self::AllocationFailure { requested } => write!(
                formatter,
                "compiler allocation failed for {requested} table entries"
            ),
            Self::NoReductionProgressWitness => {
                write!(
                    formatter,
                    "LALR reduction closure is cyclic in the supported profile"
                )
            }
            Self::Validation(error) => {
                write!(formatter, "normalized grammar validation: {error:?}")
            }
            Self::Preparation(error) => write!(formatter, "grammar preparation: {error:?}"),
        }
    }
}

impl Error for TokenizerError {}
impl Error for CompileError {}
