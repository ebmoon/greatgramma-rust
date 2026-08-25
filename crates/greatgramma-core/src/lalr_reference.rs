use crate::{ParserStateId, TerminalId, ValidatedGrammar};

use crate::lalr::{ParserError, RunOutcome, execute_terminals_concrete_checked, validate_inputs};

/// Executes the direct prefix of a realizable sequence head and probes its
/// final terminal without committing the probe's reductions or shift.
///
/// `Ok(None)` is exactly parser rejection by `Action::Error` in either the
/// direct prefix or speculative probe.
pub fn execute_sequence_head(
    grammar: &ValidatedGrammar,
    stack: &[ParserStateId],
    sequence_head: &[TerminalId],
) -> Result<Option<Vec<ParserStateId>>, ParserError> {
    if sequence_head.is_empty() {
        return Err(ParserError::EmptySequenceHead);
    }
    validate_inputs(grammar.lalr(), stack, sequence_head)?;
    execute_sequence_head_checked(grammar, stack, sequence_head)
}

pub(crate) fn execute_sequence_head_checked(
    grammar: &ValidatedGrammar,
    stack: &[ParserStateId],
    sequence_head: &[TerminalId],
) -> Result<Option<Vec<ParserStateId>>, ParserError> {
    let direct_len = sequence_head
        .len()
        .checked_sub(1)
        .ok_or(ParserError::InvariantViolation)?;
    let direct = sequence_head
        .get(..direct_len)
        .ok_or(ParserError::InvariantViolation)?;
    let committed = match execute_terminals_concrete_checked(grammar.lalr(), stack, direct)? {
        RunOutcome::Rejected => return Ok(None),
        RunOutcome::Continue { stack } => stack,
        RunOutcome::Accepted => {
            return Err(ParserError::AcceptBeforeEnd {
                terminal_index: direct_len
                    .checked_sub(1)
                    .ok_or(ParserError::InvariantViolation)?,
            });
        }
        RunOutcome::Dependent => return Err(ParserError::InvariantViolation),
    };

    let probe = sequence_head
        .get(direct_len..)
        .ok_or(ParserError::InvariantViolation)?;
    match execute_terminals_concrete_checked(grammar.lalr(), &committed, probe)? {
        RunOutcome::Rejected => Ok(None),
        RunOutcome::Continue { .. } | RunOutcome::Accepted => Ok(Some(committed)),
        RunOutcome::Dependent => Err(ParserError::InvariantViolation),
    }
}
