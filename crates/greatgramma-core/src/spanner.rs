use crate::{
    DfaStateId, LexerState, PreparationError, PreparationLimits, SequenceId, TerminalId,
    TokenExecution, TokenId, ValidatedGrammar,
};

use crate::sequence::SequenceTable;
use crate::token_step::{
    PreparedTokenTable, WorkBudget, check_items, prepare_token_table, prepared_push, prepared_vec,
    source_index, source_index_for_count,
};

/// A checked error for read-only prepared-spanner queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpannerQueryError {
    InvalidState { state: DfaStateId },
    InvalidSequence { sequence: SequenceId },
}

struct InverseBucket {
    sequence: SequenceId,
    tokens: Vec<TokenId>,
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
    pub fn sequence(&self, sequence: SequenceId) -> Option<&[TerminalId]> {
        self.sequences.sequence(sequence)
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
        sequence: SequenceId,
    ) -> Result<&[TokenId], SpannerQueryError> {
        if self.sequences.sequence(sequence).is_none() {
            return Err(SpannerQueryError::InvalidSequence { sequence });
        }
        let source_index = self.query_source(source)?;
        let row = self
            .inverse
            .get(source_index)
            .ok_or_else(|| invalid_state(source))?;
        let mut index = 0_usize;
        while index < row.len() {
            let entry = row
                .get(index)
                .ok_or(SpannerQueryError::InvalidSequence { sequence })?;
            if entry.sequence == sequence {
                return Ok(&entry.tokens);
            }
            index += 1;
        }
        Ok(&[])
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
    let token_table = prepare_token_table(grammar, limits, &mut work)?;
    let mut sequences = SequenceTable::build_singletons(grammar, limits, &mut work)?;
    let source_count = token_table.source_count();
    let mut inverse = prepared_vec(source_count)?;
    let mut inverse_members = 0_usize;

    let mut source = 0_usize;
    while source < source_count {
        inverse.push(build_inverse_row(
            grammar,
            &token_table,
            &mut sequences,
            source,
            limits,
            &mut work,
            &mut inverse_members,
        )?);
        source += 1;
    }

    Ok(PreparedSpanner {
        token_table,
        sequences,
        inverse,
    })
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
    let mut token_index = 0_u32;
    while token_index < grammar.token_count() {
        work.charge(1)?;
        let token = TokenId::new(token_index);
        let execution = token_table
            .cell(source, token)
            .ok_or(PreparationError::InvariantViolation)?;
        if let Some(TokenExecution::Continue { state, emitted }) = execution {
            let destination =
                source_index(grammar, *state).ok_or(PreparationError::InvariantViolation)?;
            let singleton_count = sequences
                .singleton(destination)
                .ok_or(PreparationError::InvariantViolation)?
                .len();
            *inverse_members = check_items(inverse_members.checked_add(singleton_count), limits)?;
            append_continuations(
                sequences,
                destination,
                emitted,
                token,
                &mut row,
                limits,
                work,
            )?;
        }
        token_index += 1;
    }
    Ok(row)
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
        .singleton(destination)
        .ok_or(PreparationError::InvariantViolation)?
        .len();
    let mut singleton = 0_usize;
    while singleton < singleton_count {
        work.charge(1)?;
        let continuation = *sequences
            .singleton(destination)
            .and_then(|values| values.get(singleton))
            .ok_or(PreparationError::InvariantViolation)?;
        let sequence = sequences.intern_head(emitted, continuation, limits, work)?;
        append_member(row, sequence, token, limits, work)?;
        singleton += 1;
    }
    Ok(())
}

fn append_member(
    row: &mut Vec<InverseBucket>,
    sequence: SequenceId,
    token: TokenId,
    limits: PreparationLimits,
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    work.charge(row.len())?;
    let mut index = 0_usize;
    while index < row.len() {
        let entry = row
            .get_mut(index)
            .ok_or(PreparationError::InvariantViolation)?;
        if entry.sequence == sequence {
            check_items(entry.tokens.len().checked_add(1), limits)?;
            prepared_push(&mut entry.tokens, token)?;
            return Ok(());
        }
        index += 1;
    }

    check_items(row.len().checked_add(1), limits)?;
    let mut tokens = prepared_vec(1)?;
    tokens.push(token);
    prepared_push(row, InverseBucket { sequence, tokens })?;
    Ok(())
}

fn invalid_state(source: LexerState) -> SpannerQueryError {
    SpannerQueryError::InvalidState {
        state: match source {
            LexerState::Start => DfaStateId::new(u32::MAX),
            LexerState::Dfa(state) => state,
        },
    }
}

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
