use std::{error::Error, fmt};

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

impl Error for TokenizerError {}
