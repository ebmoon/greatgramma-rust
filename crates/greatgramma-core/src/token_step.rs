use crate::{
    DfaStateId, LexerError, LexerInput, LexerState, LexerStep, PreparationLimits, TerminalId,
    TokenEntry, TokenId, ValidatedGrammar, lexer_step,
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

/// Preparation failed before exposing a partial derived table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    TooLarge { required: usize, maximum: usize },
    AllocationFailure { requested: usize },
    InvariantViolation,
}

pub(crate) struct WorkBudget {
    used: usize,
    maximum: usize,
}

impl WorkBudget {
    pub(crate) const fn new(limits: PreparationLimits) -> Self {
        Self {
            used: 0,
            maximum: limits.max_work,
        }
    }

    pub(crate) fn charge(&mut self, amount: usize) -> Result<(), PreparationError> {
        self.used = check_limit(self.used.checked_add(amount), self.maximum)?;
        Ok(())
    }

    pub(crate) fn remaining(&self) -> usize {
        self.maximum.saturating_sub(self.used)
    }
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

fn execute_entry(
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

    let mut index = 0_usize;
    while index < bytes.len() {
        let byte = bytes[index];
        match lexer_step(grammar, state, LexerInput::Byte(byte)) {
            Ok(LexerStep::Continue {
                state: destination,
                emitted: terminal,
            }) => {
                if let Some(terminal) = terminal {
                    token_push(&mut emitted, terminal)?;
                }
                state = destination;
            }
            Ok(LexerStep::Finished { .. }) => return Ok(None),
            Err(error) => return map_lexer_rejection(error),
        }
        index += 1;
    }

    Ok(Some(TokenExecution::Continue { state, emitted }))
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

fn map_lexer_rejection(error: LexerError) -> Result<Option<TokenExecution>, TokenStepError> {
    match error {
        LexerError::InvalidState { state } => Err(TokenStepError::InvalidState { state }),
        LexerError::CannotBeginLexeme { .. }
        | LexerError::ByteAfterUnfinishedResidual { .. }
        | LexerError::EosAfterUnfinishedResidual { .. } => Ok(None),
    }
}

pub(crate) struct PreparedTokenTable {
    rows: Vec<Vec<Option<TokenExecution>>>,
}

impl PreparedTokenTable {
    pub(crate) fn source_count(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn cell(&self, source: usize, token: TokenId) -> Option<&Option<TokenExecution>> {
        let token = usize::try_from(token.get()).ok()?;
        self.rows.get(source)?.get(token)
    }
}

pub(crate) fn prepare_token_table(
    grammar: &ValidatedGrammar,
    limits: PreparationLimits,
    work: &mut WorkBudget,
) -> Result<PreparedTokenTable, PreparationError> {
    let sources = source_count(grammar)?;
    let tokens =
        usize::try_from(grammar.token_count()).map_err(|_| PreparationError::InvariantViolation)?;
    let cells = check_items(sources.checked_mul(tokens), limits)?;
    work.charge(cells)?;

    let mut rows = prepared_vec(sources)?;
    let mut emitted_items = 0_usize;
    let mut source_index = 0_usize;
    while source_index < sources {
        let source = source_at(source_index).ok_or(PreparationError::InvariantViolation)?;
        rows.push(build_token_row(
            grammar,
            source,
            tokens,
            limits,
            work,
            &mut emitted_items,
        )?);
        source_index += 1;
    }

    Ok(PreparedTokenTable { rows })
}

fn build_token_row(
    grammar: &ValidatedGrammar,
    source: LexerState,
    token_count: usize,
    limits: PreparationLimits,
    work: &mut WorkBudget,
    emitted_items: &mut usize,
) -> Result<Vec<Option<TokenExecution>>, PreparationError> {
    let mut row = prepared_vec(token_count)?;
    let mut token_index = 0_usize;
    while token_index < token_count {
        let token = TokenId::new(
            u32::try_from(token_index).map_err(|_| PreparationError::InvariantViolation)?,
        );
        let entry = grammar
            .token(token)
            .ok_or(PreparationError::InvariantViolation)?;
        let token_work = match entry {
            TokenEntry::Bytes(bytes) => check_items(Some(bytes.len()), limits)?,
            TokenEntry::Eos => 1,
        };
        work.charge(token_work)?;
        let execution = execute_entry(grammar, source, entry).map_err(preparation_token_error)?;
        let emitted = match &execution {
            Some(TokenExecution::Continue { emitted, .. })
            | Some(TokenExecution::Finished { emitted, .. }) => emitted.len(),
            None => 0,
        };
        *emitted_items = check_items((*emitted_items).checked_add(emitted), limits)?;
        row.push(execution);
        token_index += 1;
    }
    Ok(row)
}

pub(crate) fn source_count(grammar: &ValidatedGrammar) -> Result<usize, PreparationError> {
    usize::try_from(grammar.lexer().state_count())
        .ok()
        .and_then(|count| count.checked_add(1))
        .ok_or(PreparationError::InvariantViolation)
}

pub(crate) fn source_at(index: usize) -> Option<LexerState> {
    if index == 0 {
        return Some(LexerState::Start);
    }
    let state = u32::try_from(index.checked_sub(1)?).ok()?;
    Some(LexerState::Dfa(DfaStateId::new(state)))
}

pub(crate) fn source_index(grammar: &ValidatedGrammar, source: LexerState) -> Option<usize> {
    source_count(grammar)
        .ok()
        .and_then(|count| source_index_for_count(count, source))
}

pub(crate) fn source_index_for_count(count: usize, source: LexerState) -> Option<usize> {
    match source {
        LexerState::Start if count > 0 => Some(0),
        LexerState::Start => None,
        LexerState::Dfa(state) => usize::try_from(state.get())
            .ok()?
            .checked_add(1)
            .filter(|index| *index < count),
    }
}

pub(crate) fn check_items(
    required: Option<usize>,
    limits: PreparationLimits,
) -> Result<usize, PreparationError> {
    check_limit(required, limits.max_items)
}

pub(crate) fn prepared_vec<T>(capacity: usize) -> Result<Vec<T>, PreparationError> {
    reserved_vec(capacity).map_err(|requested| PreparationError::AllocationFailure { requested })
}

pub(crate) fn prepared_push<T>(values: &mut Vec<T>, value: T) -> Result<(), PreparationError> {
    reserve_one(values).map_err(|requested| PreparationError::AllocationFailure { requested })?;
    values.push(value);
    Ok(())
}

fn token_output(capacity: usize) -> Result<Vec<TerminalId>, TokenStepError> {
    reserved_vec(capacity).map_err(|requested| TokenStepError::AllocationFailure { requested })
}

fn token_push(values: &mut Vec<TerminalId>, value: TerminalId) -> Result<(), TokenStepError> {
    reserve_one(values).map_err(|requested| TokenStepError::AllocationFailure { requested })?;
    values.push(value);
    Ok(())
}

pub(crate) fn reserved_vec<T>(capacity: usize) -> Result<Vec<T>, usize> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| capacity)?;
    Ok(values)
}

pub(crate) fn reserve_one<T>(values: &mut Vec<T>) -> Result<(), usize> {
    if values.len() == values.capacity() {
        let requested = match values.len().checked_add(1) {
            Some(requested) => requested,
            None => usize::MAX,
        };
        values.try_reserve(1).map_err(|_| requested)?;
    }
    Ok(())
}

fn preparation_token_error(error: TokenStepError) -> PreparationError {
    match error {
        TokenStepError::AllocationFailure { requested } => {
            PreparationError::AllocationFailure { requested }
        }
        TokenStepError::InvalidToken { .. } | TokenStepError::InvalidState { .. } => {
            PreparationError::InvariantViolation
        }
    }
}

fn check_limit(required: Option<usize>, maximum: usize) -> Result<usize, PreparationError> {
    let required = required.ok_or(PreparationError::TooLarge {
        required: usize::MAX,
        maximum,
    })?;
    if required > maximum {
        return Err(PreparationError::TooLarge { required, maximum });
    }
    Ok(required)
}

#[cfg(test)]
mod tests {
    use super::*;

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
