use crate::{
    ParserError, ParserStateId, PreparationError, PreparationLimits, PreparedSpanner, SequenceId,
    TerminalId, ValidatedGrammar,
};

use crate::lalr::{
    RunOutcome, RunStatus, execute_terminals_symbolic,
    execute_terminals_symbolic_checked_with_limit,
};
use crate::token_step::{WorkBudget, check_items, prepared_vec};

/// Whether a sequence head is readable from a parser state without consulting
/// states below the current stack top.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceClassification {
    AlwaysReadable,
    Rejected,
    Dependent,
}

/// Direct parser-state by sequence-head classification table.
pub struct PreparedParser {
    rows: Vec<Vec<SequenceClassification>>,
}

impl PreparedParser {
    /// Returns the precomputed classification for one checked table cell.
    pub fn classification(
        &self,
        state: ParserStateId,
        sequence: SequenceId,
    ) -> Result<SequenceClassification, ParserError> {
        let state_index =
            usize::try_from(state.get()).map_err(|_| ParserError::InvalidState { state })?;
        let row = self
            .rows
            .get(state_index)
            .ok_or(ParserError::InvalidState { state })?;
        let sequence_index = usize::try_from(sequence.get())
            .map_err(|_| ParserError::InvalidSequence { sequence })?;
        row.get(sequence_index)
            .copied()
            .ok_or(ParserError::InvalidSequence { sequence })
    }
}

/// Classifies one sequence head by executing it from a symbolic one-state
/// stack. The final terminal is probed but none of its effects are committed.
pub fn classify_sequence_head(
    grammar: &ValidatedGrammar,
    state: ParserStateId,
    sequence_head: &[TerminalId],
) -> Result<SequenceClassification, ParserError> {
    if sequence_head.is_empty() {
        return Err(ParserError::EmptySequenceHead);
    }
    let initial_stack = [state];
    let outcome = execute_terminals_symbolic(grammar.lalr(), &initial_stack, sequence_head)?;
    Ok(classify_outcome(outcome))
}

/// Builds the direct parser-state by interned-sequence classification table.
pub fn prepare_parser(
    grammar: &ValidatedGrammar,
    spanner: &PreparedSpanner,
    limits: PreparationLimits,
) -> Result<PreparedParser, PreparationError> {
    let state_count = usize::try_from(grammar.lalr().state_count())
        .map_err(|_| PreparationError::InvariantViolation)?;
    let sequence_count = spanner.sequence_count();
    check_items(state_count.checked_mul(sequence_count), limits)?;

    let mut work = WorkBudget::new(limits);
    let mut rows = prepared_vec(state_count)?;
    let mut state_index = 0_usize;
    while state_index < state_count {
        let state = ParserStateId::new(
            u32::try_from(state_index).map_err(|_| PreparationError::InvariantViolation)?,
        );
        let mut row = prepared_vec(sequence_count)?;
        let mut sequence_index = 0_usize;
        while sequence_index < sequence_count {
            let sequence = SequenceId::new(
                u32::try_from(sequence_index).map_err(|_| PreparationError::InvariantViolation)?,
            );
            let head = spanner
                .sequence(sequence)
                .ok_or(PreparationError::InvariantViolation)?;
            let classification =
                classify_sequence_head_with_budget(grammar, state, head, &mut work)?;
            row.push(classification);
            sequence_index += 1;
        }
        rows.push(row);
        state_index += 1;
    }

    Ok(PreparedParser { rows })
}

fn classify_sequence_head_with_budget(
    grammar: &ValidatedGrammar,
    state: ParserStateId,
    sequence_head: &[TerminalId],
    work: &mut WorkBudget,
) -> Result<SequenceClassification, PreparationError> {
    if sequence_head.is_empty() {
        return Err(PreparationError::InvariantViolation);
    }

    let initial_stack = [state];
    let status = execute_terminals_symbolic_checked_with_limit(
        grammar.lalr(),
        &initial_stack,
        sequence_head,
        work.remaining(),
    )
    .map_err(map_parser_preparation_error)?;
    match status {
        RunStatus::Complete {
            outcome,
            work: inspected,
        } => {
            work.charge(inspected)?;
            Ok(classify_outcome(outcome))
        }
        RunStatus::WorkLimitExceeded { required } => match work.charge(required) {
            Err(error) => Err(error),
            Ok(()) => Err(PreparationError::InvariantViolation),
        },
    }
}

fn classify_outcome(outcome: RunOutcome) -> SequenceClassification {
    match outcome {
        RunOutcome::Rejected => SequenceClassification::Rejected,
        RunOutcome::Dependent => SequenceClassification::Dependent,
        RunOutcome::Continue { .. } | RunOutcome::Accepted => {
            SequenceClassification::AlwaysReadable
        }
    }
}

fn map_parser_preparation_error(error: ParserError) -> PreparationError {
    match error {
        ParserError::AllocationFailure { requested } => {
            PreparationError::AllocationFailure { requested }
        }
        ParserError::EmptyStack
        | ParserError::EmptySequenceHead
        | ParserError::InvalidState { .. }
        | ParserError::InvalidTerminal { .. }
        | ParserError::InvalidSequence { .. }
        | ParserError::StackUnderflow { .. }
        | ParserError::MissingGoto { .. }
        | ParserError::AcceptBeforeEnd { .. }
        | ParserError::InvalidReductionProgress { .. }
        | ParserError::InvariantViolation => PreparationError::InvariantViolation,
    }
}
