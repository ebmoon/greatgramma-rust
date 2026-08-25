use crate::{ParserStateId, TerminalId, ValidatedGrammar};

use crate::lalr::{
    ParserError, RunDecision, execute_terminals_concrete_in_place, execute_terminals_concrete_into,
    validate_inputs,
};

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
    let mut committed = Vec::new();
    match execute_terminals_concrete_into(grammar.lalr(), stack, direct, &mut committed)? {
        RunDecision::Rejected => return Ok(None),
        RunDecision::Continue => {}
        RunDecision::Accepted => {
            return Err(ParserError::AcceptBeforeEnd {
                terminal_index: direct_len
                    .checked_sub(1)
                    .ok_or(ParserError::InvariantViolation)?,
            });
        }
        RunDecision::Dependent => return Err(ParserError::InvariantViolation),
    }

    let probe = sequence_head
        .get(direct_len..)
        .ok_or(ParserError::InvariantViolation)?;
    let mut probe_stack = Vec::new();
    match execute_terminals_concrete_into(grammar.lalr(), &committed, probe, &mut probe_stack)? {
        RunDecision::Rejected => Ok(None),
        RunDecision::Continue | RunDecision::Accepted => Ok(Some(committed)),
        RunDecision::Dependent => Err(ParserError::InvariantViolation),
    }
}

/// Checks one realizable head with a single reusable stack. The speculative
/// final terminal may overwrite `scratch` because callers need only the
/// decision, not the direct-prefix stack returned by the public reference API.
pub(crate) fn sequence_head_allowed_with_scratch(
    grammar: &ValidatedGrammar,
    stack: &[ParserStateId],
    sequence_head: &[TerminalId],
    scratch: &mut Vec<ParserStateId>,
) -> Result<bool, ParserError> {
    let direct_len = sequence_head
        .len()
        .checked_sub(1)
        .ok_or(ParserError::EmptySequenceHead)?;
    let direct = sequence_head
        .get(..direct_len)
        .ok_or(ParserError::InvariantViolation)?;
    match execute_terminals_concrete_into(grammar.lalr(), stack, direct, scratch)? {
        RunDecision::Rejected => return Ok(false),
        RunDecision::Continue => {}
        RunDecision::Accepted => {
            return Err(ParserError::AcceptBeforeEnd {
                terminal_index: direct_len
                    .checked_sub(1)
                    .ok_or(ParserError::InvariantViolation)?,
            });
        }
        RunDecision::Dependent => return Err(ParserError::InvariantViolation),
    }

    let probe = sequence_head
        .get(direct_len..)
        .ok_or(ParserError::InvariantViolation)?;
    match execute_terminals_concrete_in_place(grammar.lalr(), scratch, probe)? {
        RunDecision::Rejected => Ok(false),
        RunDecision::Continue | RunDecision::Accepted => Ok(true),
        RunDecision::Dependent => Err(ParserError::InvariantViolation),
    }
}
