use crate::{DfaStateId, TerminalId, ValidatedGrammar, ValidatedLexer};

/// Runtime state of the normalized lexer.
///
/// Logical start is distinct from the DFA start state: returning to the DFA
/// start after consuming bytes still represents an unfinished residual.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexerState {
    Start,
    Dfa(DfaStateId),
}

/// One semantic input event for the normalized lexer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexerInput {
    Byte(u8),
    Eos,
}

/// One successful normalized lexer transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexerStep {
    Continue {
        state: LexerState,
        emitted: Option<TerminalId>,
    },
    Finished {
        terminal: Option<TerminalId>,
        eof: TerminalId,
    },
}

/// A fail-closed normalized lexer transition error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexerError {
    InvalidState { state: DfaStateId },
    CannotBeginLexeme { byte: u8 },
    ByteAfterUnfinishedResidual { state: DfaStateId, byte: u8 },
    EosAfterUnfinishedResidual { state: DfaStateId },
}

/// Executes one byte or EOS event against validated normalized tables.
///
/// The function is pure and allocation-free. A rejected boundary returns no
/// emitted terminal or successor state.
pub fn lexer_step(
    grammar: &ValidatedGrammar,
    state: LexerState,
    input: LexerInput,
) -> Result<LexerStep, LexerError> {
    match input {
        LexerInput::Byte(byte) => byte_step(grammar.lexer(), state, byte),
        LexerInput::Eos => eos_step(grammar, state),
    }
}

fn byte_step(lexer: &ValidatedLexer, state: LexerState, byte: u8) -> Result<LexerStep, LexerError> {
    match state {
        LexerState::Start => begin_lexeme(lexer, byte, None),
        LexerState::Dfa(source) => match lexer.transition(source, byte) {
            None => Err(LexerError::InvalidState { state: source }),
            Some(Some(destination)) => Ok(LexerStep::Continue {
                state: LexerState::Dfa(destination),
                emitted: None,
            }),
            Some(None) => finish_or_reject_residual(lexer, source, byte),
        },
    }
}

fn finish_or_reject_residual(
    lexer: &ValidatedLexer,
    state: DfaStateId,
    byte: u8,
) -> Result<LexerStep, LexerError> {
    match lexer.terminal(state) {
        None => Err(LexerError::InvalidState { state }),
        Some(None) => Err(LexerError::ByteAfterUnfinishedResidual { state, byte }),
        Some(Some(terminal)) => begin_lexeme(lexer, byte, Some(terminal)),
    }
}

fn begin_lexeme(
    lexer: &ValidatedLexer,
    byte: u8,
    emitted: Option<TerminalId>,
) -> Result<LexerStep, LexerError> {
    let start = lexer.start_state();
    match lexer.transition(start, byte) {
        None => Err(LexerError::InvalidState { state: start }),
        Some(None) => Err(LexerError::CannotBeginLexeme { byte }),
        Some(Some(destination)) => Ok(LexerStep::Continue {
            state: LexerState::Dfa(destination),
            emitted,
        }),
    }
}

fn eos_step(grammar: &ValidatedGrammar, state: LexerState) -> Result<LexerStep, LexerError> {
    let eof = grammar.lalr().eof_terminal();
    match state {
        LexerState::Start => Ok(LexerStep::Finished {
            terminal: None,
            eof,
        }),
        LexerState::Dfa(source) => match grammar.lexer().terminal(source) {
            None => Err(LexerError::InvalidState { state: source }),
            Some(None) => Err(LexerError::EosAfterUnfinishedResidual { state: source }),
            Some(Some(terminal)) => Ok(LexerStep::Finished {
                terminal: Some(terminal),
                eof,
            }),
        },
    }
}
