use super::budget::{WorkBudget, check_items, prepared_vec};
use crate::tokenizer::execute::execute_entry;
use crate::{
    DfaStateId, LexerState, PreparationError, PreparationLimits, TokenEntry, TokenExecution,
    TokenId, TokenStepError, ValidatedGrammar,
};

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

    let mut emitted_items = 0_usize;
    let rows = build_token_rows(grammar, sources, tokens, limits, work, &mut emitted_items)?;

    Ok(PreparedTokenTable { rows })
}

fn build_token_rows(
    grammar: &ValidatedGrammar,
    source_count: usize,
    token_count: usize,
    limits: PreparationLimits,
    work: &mut WorkBudget,
    emitted_items: &mut usize,
) -> Result<Vec<Vec<Option<TokenExecution>>>, PreparationError> {
    let mut rows = prepared_vec(source_count)?;
    for source_index in 0..source_count {
        let source = source_at(source_index).ok_or(PreparationError::InvariantViolation)?;
        rows.push(build_token_row(
            grammar,
            source,
            token_count,
            limits,
            work,
            emitted_items,
        )?);
    }
    Ok(rows)
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
    for token_index in 0..token_count {
        append_token_cell(
            grammar,
            source,
            token_index,
            limits,
            work,
            emitted_items,
            &mut row,
        )?;
    }
    Ok(row)
}

fn append_token_cell(
    grammar: &ValidatedGrammar,
    source: LexerState,
    token_index: usize,
    limits: PreparationLimits,
    work: &mut WorkBudget,
    emitted_items: &mut usize,
    row: &mut Vec<Option<TokenExecution>>,
) -> Result<(), PreparationError> {
    let token =
        TokenId::new(u32::try_from(token_index).map_err(|_| PreparationError::InvariantViolation)?);
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
        Some(
            TokenExecution::Continue { emitted, .. } | TokenExecution::Finished { emitted, .. },
        ) => emitted.len(),
        None => 0,
    };
    *emitted_items = check_items((*emitted_items).checked_add(emitted), limits)?;
    row.push(execution);
    Ok(())
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
