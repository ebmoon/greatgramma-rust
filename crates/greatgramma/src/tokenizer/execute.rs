use std::{error::Error, fmt};

use crate::preparation::budget::{reserve_one, reserved_vec};
use crate::{
    DfaStateId, LexerError, LexerInput, LexerState, LexerStep, TerminalId, TokenEntry, TokenId,
    ValidatedGrammar, lexer_step,
};

/// The result of composing one complete model token with lexer execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenExecution {
    /// An ordinary token consumed all of its bytes and left a residual lexer state.
    Continue {
        state: LexerState,
        emitted: Vec<TerminalId>,
    },
    /// An EOS token finished lexer execution.
    Finished {
        emitted: Vec<TerminalId>,
        eof: TerminalId,
    },
}

/// A fail-closed error for invalid public token-composition inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenStepError {
    InvalidToken { token: TokenId },
    InvalidState { state: DfaStateId },
    AllocationFailure { requested: usize },
}

/// Composes exactly one normalized model token with the corrected lexer.
///
/// `Ok(None)` is ordinary lexer rejection. Invalid IDs remain distinct from
/// rejection, and the function never exposes a partial successor.
pub fn execute_token(
    grammar: &ValidatedGrammar,
    source: LexerState,
    token: TokenId,
) -> Result<Option<TokenExecution>, TokenStepError> {
    check_source(grammar, source)?;
    let entry = grammar
        .token(token)
        .ok_or(TokenStepError::InvalidToken { token })?;

    execute_entry(grammar, source, entry)
}

pub(crate) fn execute_entry(
    grammar: &ValidatedGrammar,
    source: LexerState,
    entry: &TokenEntry,
) -> Result<Option<TokenExecution>, TokenStepError> {
    match entry {
        TokenEntry::Bytes(bytes) => execute_bytes(grammar, source, bytes),
        TokenEntry::Eos => execute_eos(grammar, source),
    }
}

fn check_source(grammar: &ValidatedGrammar, source: LexerState) -> Result<(), TokenStepError> {
    match source {
        LexerState::Start => Ok(()),
        LexerState::Dfa(state) if grammar.lexer().terminal(state).is_some() => Ok(()),
        LexerState::Dfa(state) => Err(TokenStepError::InvalidState { state }),
    }
}

fn execute_bytes(
    grammar: &ValidatedGrammar,
    source: LexerState,
    bytes: &[u8],
) -> Result<Option<TokenExecution>, TokenStepError> {
    let mut state = source;
    let mut emitted = Vec::new();
    for &byte in bytes {
        let Some(destination) = execute_byte(grammar, state, byte, &mut emitted)? else {
            return Ok(None);
        };
        state = destination;
    }
    Ok(Some(TokenExecution::Continue { state, emitted }))
}

fn execute_byte(
    grammar: &ValidatedGrammar,
    state: LexerState,
    byte: u8,
    emitted: &mut Vec<TerminalId>,
) -> Result<Option<LexerState>, TokenStepError> {
    match lexer_step(grammar, state, LexerInput::Byte(byte)) {
        Ok(LexerStep::Continue {
            state: destination,
            emitted: terminal,
        }) => {
            if let Some(terminal) = terminal {
                token_push(emitted, terminal)?;
            }
            Ok(Some(destination))
        }
        Ok(LexerStep::Finished { .. }) => Ok(None),
        Err(error) => map_lexer_rejection(error),
    }
}

fn execute_eos(
    grammar: &ValidatedGrammar,
    source: LexerState,
) -> Result<Option<TokenExecution>, TokenStepError> {
    match lexer_step(grammar, source, LexerInput::Eos) {
        Ok(LexerStep::Finished { terminal, eof }) => {
            let mut emitted = token_output(1)?;
            if let Some(terminal) = terminal {
                emitted.push(terminal);
            }
            Ok(Some(TokenExecution::Finished { emitted, eof }))
        }
        Ok(LexerStep::Continue { .. }) => Ok(None),
        Err(error) => map_lexer_rejection(error),
    }
}

fn map_lexer_rejection<T>(error: LexerError) -> Result<Option<T>, TokenStepError> {
    match error {
        LexerError::InvalidState { state } => Err(TokenStepError::InvalidState { state }),
        LexerError::CannotBeginLexeme { .. }
        | LexerError::ByteAfterUnfinishedResidual { .. }
        | LexerError::EosAfterUnfinishedResidual { .. } => Ok(None),
    }
}

fn token_output(capacity: usize) -> Result<Vec<TerminalId>, TokenStepError> {
    reserved_vec(capacity).map_err(|requested| TokenStepError::AllocationFailure { requested })
}

fn token_push(values: &mut Vec<TerminalId>, value: TerminalId) -> Result<(), TokenStepError> {
    reserve_one(values).map_err(|requested| TokenStepError::AllocationFailure { requested })?;
    values.push(value);
    Ok(())
}

impl fmt::Display for TokenStepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidToken { token } => {
                write!(f, "invalid token {} for token execution", token.get())
            }
            Self::InvalidState { state } => {
                write!(f, "invalid lexer state {} for token execution", state.get())
            }
            Self::AllocationFailure { requested } => {
                write!(
                    f,
                    "token execution allocation failed for {requested} terminals"
                )
            }
        }
    }
}

impl Error for TokenStepError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PreparationError, preparation::budget::prepared_vec};

    #[test]
    fn allocation_capacity_overflow_is_structured() {
        assert_eq!(
            prepared_vec::<u8>(usize::MAX).err(),
            Some(PreparationError::AllocationFailure {
                requested: usize::MAX,
            })
        );
        assert_eq!(
            token_output(usize::MAX).err(),
            Some(TokenStepError::AllocationFailure {
                requested: usize::MAX,
            })
        );
    }
}
