use std::{error::Error, fmt};

use crate::{
    DfaStateId, LexerState, PreparationError, PreparationLimits, SequenceId, TerminalId,
    TokenExecution, TokenId, ValidatedGrammar,
};

use super::budget::{WorkBudget, check_items, prepared_push, prepared_vec};
use super::sequence::SequenceTable;
use super::tokens::{
    PreparedTokenTable, prepare_token_table, source_index, source_index_for_count,
};

/// A checked error for read-only prepared-spanner queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpannerQueryError {
    InvalidState { state: DfaStateId },
    InvalidSequence { sequence: SequenceId },
}

pub(crate) struct InverseBucket {
    pub(crate) sequence: SequenceId,
    pub(crate) tokens: Vec<TokenId>,
}

/// Opaque, read-only exact sequence heads and inverse token buckets.
pub struct PreparedSpanner {
    token_table: PreparedTokenTable,
    sequences: SequenceTable,
    inverse: Vec<Vec<InverseBucket>>,
}

impl PreparedSpanner {
    #[must_use]
    pub fn sequence_count(&self) -> usize {
        self.sequences.sequence_count()
    }

    #[must_use]
    pub fn sequence(&self, sequence_id: SequenceId) -> Option<&[TerminalId]> {
        self.sequences.sequence(sequence_id)
    }

    pub fn singleton_terminals(
        &self,
        source: LexerState,
    ) -> Result<&[TerminalId], SpannerQueryError> {
        let source_index = self.query_source(source)?;
        self.sequences
            .singleton(source_index)
            .ok_or_else(|| invalid_state(source))
    }

    pub fn inverse_tokens(
        &self,
        source: LexerState,
        sequence_id: SequenceId,
    ) -> Result<&[TokenId], SpannerQueryError> {
        if self.sequences.sequence(sequence_id).is_none() {
            return Err(SpannerQueryError::InvalidSequence {
                sequence: sequence_id,
            });
        }
        let row = self.inverse_buckets(source)?;
        Ok(row
            .iter()
            .find(|entry| entry.sequence == sequence_id)
            .map_or(&[], |entry| entry.tokens.as_slice()))
    }

    pub(crate) fn token_execution(
        &self,
        source: LexerState,
        token: TokenId,
    ) -> Result<&Option<TokenExecution>, SpannerQueryError> {
        let source_index = self.query_source(source)?;
        self.token_table
            .cell(source_index, token)
            .ok_or(SpannerQueryError::InvalidSequence {
                sequence: SequenceId::new(token.get()),
            })
    }

    pub(crate) fn inverse_buckets(
        &self,
        source: LexerState,
    ) -> Result<&[InverseBucket], SpannerQueryError> {
        let source_index = self.query_source(source)?;
        self.inverse
            .get(source_index)
            .map(Vec::as_slice)
            .ok_or_else(|| invalid_state(source))
    }

    fn query_source(&self, source: LexerState) -> Result<usize, SpannerQueryError> {
        source_index_for_count(self.token_table.source_count(), source)
            .ok_or_else(|| invalid_state(source))
    }
}

/// Prepares exact singleton heads and deterministic inverse token buckets.
pub fn prepare_spanner(
    grammar: &ValidatedGrammar,
    limits: PreparationLimits,
) -> Result<PreparedSpanner, PreparationError> {
    let mut work = WorkBudget::new(limits);
    prepare_spanner_with_budget(grammar, limits, &mut work)
}

pub(crate) fn prepare_spanner_with_budget(
    grammar: &ValidatedGrammar,
    limits: PreparationLimits,
    work: &mut WorkBudget,
) -> Result<PreparedSpanner, PreparationError> {
    let token_table = prepare_token_table(grammar, limits, work)?;
    let mut sequences = SequenceTable::build_singletons(grammar, limits, work)?;
    let source_count = token_table.source_count();
    let mut inverse_members = 0_usize;
    let inverse = build_inverse_rows(
        grammar,
        &token_table,
        &mut sequences,
        source_count,
        limits,
        work,
        &mut inverse_members,
    )?;

    Ok(PreparedSpanner {
        token_table,
        sequences,
        inverse,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_inverse_rows(
    grammar: &ValidatedGrammar,
    token_table: &PreparedTokenTable,
    sequences: &mut SequenceTable,
    source_count: usize,
    limits: PreparationLimits,
    work: &mut WorkBudget,
    inverse_members: &mut usize,
) -> Result<Vec<Vec<InverseBucket>>, PreparationError> {
    let mut inverse = prepared_vec(source_count)?;
    for source in 0..source_count {
        inverse.push(build_inverse_row(
            grammar,
            token_table,
            sequences,
            source,
            limits,
            work,
            inverse_members,
        )?);
    }
    Ok(inverse)
}

fn build_inverse_row(
    grammar: &ValidatedGrammar,
    token_table: &PreparedTokenTable,
    sequences: &mut SequenceTable,
    source: usize,
    limits: PreparationLimits,
    work: &mut WorkBudget,
    inverse_members: &mut usize,
) -> Result<Vec<InverseBucket>, PreparationError> {
    let mut row = Vec::new();
    for token_index in 0..grammar.token_count() {
        append_token_continuations(
            grammar,
            token_table,
            sequences,
            source,
            TokenId::new(token_index),
            &mut row,
            limits,
            work,
            inverse_members,
        )?;
    }
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
fn append_token_continuations(
    grammar: &ValidatedGrammar,
    token_table: &PreparedTokenTable,
    sequences: &mut SequenceTable,
    source: usize,
    token: TokenId,
    row: &mut Vec<InverseBucket>,
    limits: PreparationLimits,
    work: &mut WorkBudget,
    inverse_members: &mut usize,
) -> Result<(), PreparationError> {
    work.charge(1)?;
    let execution = token_table
        .cell(source, token)
        .ok_or(PreparationError::InvariantViolation)?;
    if let Some(TokenExecution::Continue { state, emitted }) = execution {
        let destination =
            source_index(grammar, *state).ok_or(PreparationError::InvariantViolation)?;
        let singleton_count = sequences
            .singleton_count(destination)
            .ok_or(PreparationError::InvariantViolation)?;
        *inverse_members = check_items(inverse_members.checked_add(singleton_count), limits)?;
        append_continuations(sequences, destination, emitted, token, row, limits, work)?;
    }
    Ok(())
}

fn append_continuations(
    sequences: &mut SequenceTable,
    destination: usize,
    emitted: &[TerminalId],
    token: TokenId,
    row: &mut Vec<InverseBucket>,
    limits: PreparationLimits,
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    let singleton_count = sequences
        .singleton_count(destination)
        .ok_or(PreparationError::InvariantViolation)?;
    for singleton in 0..singleton_count {
        work.charge(1)?;
        let continuation = sequences
            .singleton_terminal(destination, singleton)
            .ok_or(PreparationError::InvariantViolation)?;
        let sequence = sequences.intern_head(emitted, continuation, limits, work)?;
        append_member(row, sequence, token, limits, work)?;
    }
    Ok(())
}

fn append_member(
    row: &mut Vec<InverseBucket>,
    sequence_id: SequenceId,
    token: TokenId,
    limits: PreparationLimits,
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    work.charge(row.len())?;
    if let Some(entry) = row.iter_mut().find(|entry| entry.sequence == sequence_id) {
        check_items(entry.tokens.len().checked_add(1), limits)?;
        return prepared_push(&mut entry.tokens, token);
    }

    check_items(row.len().checked_add(1), limits)?;
    let mut tokens = prepared_vec(1)?;
    tokens.push(token);
    prepared_push(row, InverseBucket {
        sequence: sequence_id,
        tokens,
    })
}

fn invalid_state(source: LexerState) -> SpannerQueryError {
    SpannerQueryError::InvalidState {
        state: match source {
            LexerState::Start => DfaStateId::new(u32::MAX),
            LexerState::Dfa(state) => state,
        },
    }
}

impl fmt::Display for SpannerQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState { state } => {
                write!(f, "invalid lexer state {} for spanner query", state.get())
            }
            Self::InvalidSequence { sequence } => {
                write!(f, "invalid sequence {} for spanner query", sequence.get())
            }
        }
    }
}

impl Error for SpannerQueryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarse_limits_bound_inverse_bucket_growth_and_lookup_work() {
        let limits = PreparationLimits {
            max_items: 1,
            ..PreparationLimits::default()
        };
        let mut row = Vec::new();
        let mut work = WorkBudget::new(limits);
        append_member(
            &mut row,
            SequenceId::new(0),
            TokenId::new(0),
            limits,
            &mut work,
        )
        .expect("first member fits");
        assert_eq!(
            append_member(
                &mut row,
                SequenceId::new(0),
                TokenId::new(1),
                limits,
                &mut work,
            )
            .err(),
            Some(PreparationError::TooLarge {
                required: 2,
                maximum: 1,
            })
        );
        assert_eq!(row[0].tokens, [TokenId::new(0)]);

        let lookup_limits = PreparationLimits {
            max_work: 0,
            ..PreparationLimits::default()
        };
        let mut lookup_work = WorkBudget::new(lookup_limits);
        assert_eq!(
            append_member(
                &mut row,
                SequenceId::new(1),
                TokenId::new(1),
                lookup_limits,
                &mut lookup_work,
            )
            .err(),
            Some(PreparationError::TooLarge {
                required: 1,
                maximum: 0,
            })
        );
        assert_eq!(row.len(), 1);
    }
}
