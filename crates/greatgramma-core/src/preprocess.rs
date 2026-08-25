use crate::{
    ParserError, ParserStateId, PreparationError, PreparationLimits, PreparedSpanner, SequenceId,
    TerminalId, ValidatedGrammar,
};

use crate::lalr::{
    RunDecision, RunStatus, execute_terminals_symbolic,
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
    let decision = execute_terminals_symbolic(grammar.lalr(), &initial_stack, sequence_head)?;
    Ok(classify_decision(decision))
}

/// Builds the direct parser-state by interned-sequence classification table.
/// Logical rows and classification cells both count toward preparation limits.
pub fn prepare_parser(
    grammar: &ValidatedGrammar,
    prepared_spanner: &PreparedSpanner,
    limits: PreparationLimits,
) -> Result<PreparedParser, PreparationError> {
    let mut work = WorkBudget::new(limits);
    prepare_parser_with_budget(grammar, prepared_spanner, limits, &mut work)
}

pub(crate) fn prepare_parser_with_budget(
    grammar: &ValidatedGrammar,
    prepared_spanner: &PreparedSpanner,
    limits: PreparationLimits,
    work: &mut WorkBudget,
) -> Result<PreparedParser, PreparationError> {
    let state_count = usize::try_from(grammar.lalr().state_count())
        .map_err(|_| PreparationError::InvariantViolation)?;
    let sequence_count = prepared_spanner.sequence_count();
    let table_items = state_count
        .checked_mul(sequence_count)
        .and_then(|cells| cells.checked_add(state_count));
    check_items(table_items, limits)?;

    work.charge(state_count)?;
    let mut rows = prepared_vec(state_count)?;
    let mut error = None;
    let mut state_index = 0_usize;
    while state_index < state_count && error.is_none() {
        error = match u32::try_from(state_index) {
            Ok(raw_state) => {
                let state = ParserStateId::new(raw_state);
                match prepare_parser_row(grammar, prepared_spanner, state, sequence_count, work) {
                    Ok(row) => {
                        rows.push(row);
                        None
                    }
                    Err(row_error) => Some(row_error),
                }
            }
            Err(_) => Some(PreparationError::InvariantViolation),
        };
        state_index += 1;
    }

    match error {
        Some(error) => Err(error),
        None => Ok(PreparedParser { rows }),
    }
}

fn prepare_parser_row(
    grammar: &ValidatedGrammar,
    prepared_spanner: &PreparedSpanner,
    state: ParserStateId,
    sequence_count: usize,
    work: &mut WorkBudget,
) -> Result<Vec<SequenceClassification>, PreparationError> {
    let mut row = prepared_vec(sequence_count)?;
    let mut error = None;
    let mut sequence_index = 0_usize;
    while sequence_index < sequence_count && error.is_none() {
        error = match u32::try_from(sequence_index) {
            Ok(raw_sequence) => {
                let sequence = SequenceId::new(raw_sequence);
                match prepared_spanner.sequence(sequence) {
                    Some(head) => {
                        match classify_sequence_head_with_budget(grammar, state, head, work) {
                            Ok(classification) => {
                                row.push(classification);
                                None
                            }
                            Err(classification_error) => Some(classification_error),
                        }
                    }
                    None => Some(PreparationError::InvariantViolation),
                }
            }
            Err(_) => Some(PreparationError::InvariantViolation),
        };
        sequence_index += 1;
    }
    match error {
        Some(error) => Err(error),
        None => Ok(row),
    }
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
            decision,
            work: inspected,
        } => {
            work.charge(inspected)?;
            Ok(classify_decision(decision))
        }
        RunStatus::WorkLimitExceeded { required } => match work.charge(required) {
            Err(error) => Err(error),
            Ok(()) => Err(PreparationError::InvariantViolation),
        },
    }
}

fn classify_decision(decision: RunDecision) -> SequenceClassification {
    match decision {
        RunDecision::Rejected => SequenceClassification::Rejected,
        RunDecision::Dependent => SequenceClassification::Dependent,
        RunDecision::Continue | RunDecision::Accepted => SequenceClassification::AlwaysReadable,
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
