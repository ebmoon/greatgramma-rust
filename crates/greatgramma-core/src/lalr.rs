use crate::{
    Action, NonterminalId, ParserStateId, ProductionId, SequenceId, TerminalId, ValidatedGrammar,
    ValidatedLalr,
};

use crate::token_step::{reserve_one, reserved_vec};

/// The result of executing a complete terminal slice against the LALR table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParserExecution {
    /// Every terminal was consumed without accepting the grammar.
    Continue { stack: Vec<ParserStateId> },
    /// The final terminal selected the parser's accept action.
    Accepted,
}

/// A fail-closed error for parser execution and parser-table invariants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParserError {
    EmptyStack,
    EmptySequenceHead,
    InvalidState {
        state: ParserStateId,
    },
    InvalidTerminal {
        terminal: TerminalId,
    },
    InvalidSequence {
        sequence: SequenceId,
    },
    StackUnderflow {
        production: ProductionId,
        pop_len: u32,
        stack_len: usize,
    },
    MissingGoto {
        state: ParserStateId,
        nonterminal: NonterminalId,
        production: ProductionId,
    },
    AcceptBeforeEnd {
        terminal_index: usize,
    },
    InvalidReductionProgress {
        state: ParserStateId,
        terminal: TerminalId,
    },
    AllocationFailure {
        requested: usize,
    },
    InvariantViolation,
}

/// Internal result used by preprocessing from a symbolic one-state stack.
///
/// `Dependent` arises only when a reduction would pop the symbolic base. All
/// other variants have the same meaning as their concrete public counterpart.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RunOutcome {
    Rejected,
    Continue { stack: Vec<ParserStateId> },
    Accepted,
    Dependent,
}

/// Internal bounded-execution result used during preparation.
pub(crate) enum RunStatus {
    Complete { outcome: RunOutcome, work: usize },
    WorkLimitExceeded { required: usize },
}

#[derive(Clone, Copy)]
enum UnderflowMode {
    Error,
    Dependent,
}

#[derive(Clone, Copy)]
struct PreviousReduction {
    rank: u32,
    pop_len: u32,
}

/// Executes terminals using the normalized LALR table.
///
/// `Ok(None)` is exactly an `Action::Error` rejection. Invalid public IDs and
/// violated table invariants are reported separately, and no partial stack is
/// exposed on failure.
pub fn execute_terminals(
    grammar: &ValidatedGrammar,
    stack: &[ParserStateId],
    terminals: &[TerminalId],
) -> Result<Option<ParserExecution>, ParserError> {
    validate_inputs(grammar.lalr(), stack, terminals)?;
    match execute_terminals_concrete_checked(grammar.lalr(), stack, terminals)? {
        RunOutcome::Rejected => Ok(None),
        RunOutcome::Continue { stack } => Ok(Some(ParserExecution::Continue { stack })),
        RunOutcome::Accepted => Ok(Some(ParserExecution::Accepted)),
        RunOutcome::Dependent => Err(ParserError::InvariantViolation),
    }
}

/// Runs from a symbolic stack base for exact sequence-head preprocessing.
///
/// Unlike concrete execution, popping the supplied base returns `Dependent`:
/// the action can only be decided with parser states below that base.
pub(crate) fn execute_terminals_symbolic(
    lalr: &ValidatedLalr,
    stack: &[ParserStateId],
    terminals: &[TerminalId],
) -> Result<RunOutcome, ParserError> {
    validate_inputs(lalr, stack, terminals)?;
    unbounded_outcome(run_validated(
        lalr,
        stack,
        terminals,
        UnderflowMode::Dependent,
        None,
    )?)
}

pub(crate) fn execute_terminals_symbolic_checked_with_limit(
    lalr: &ValidatedLalr,
    stack: &[ParserStateId],
    terminals: &[TerminalId],
    maximum_work: usize,
) -> Result<RunStatus, ParserError> {
    run_validated(
        lalr,
        stack,
        terminals,
        UnderflowMode::Dependent,
        Some(maximum_work),
    )
}

pub(crate) fn execute_terminals_concrete_checked(
    lalr: &ValidatedLalr,
    stack: &[ParserStateId],
    terminals: &[TerminalId],
) -> Result<RunOutcome, ParserError> {
    unbounded_outcome(run_validated(
        lalr,
        stack,
        terminals,
        UnderflowMode::Error,
        None,
    )?)
}

pub(crate) fn validate_inputs(
    lalr: &ValidatedLalr,
    stack: &[ParserStateId],
    terminals: &[TerminalId],
) -> Result<(), ParserError> {
    if stack.is_empty() {
        return Err(ParserError::EmptyStack);
    }

    let mut stack_index = 0_usize;
    while stack_index < stack.len() {
        let state = *stack
            .get(stack_index)
            .ok_or(ParserError::InvariantViolation)?;
        if state.get() >= lalr.state_count() {
            return Err(ParserError::InvalidState { state });
        }
        stack_index += 1;
    }

    let mut terminal_index = 0_usize;
    while terminal_index < terminals.len() {
        let terminal = *terminals
            .get(terminal_index)
            .ok_or(ParserError::InvariantViolation)?;
        if terminal.get() >= lalr.terminal_count() {
            return Err(ParserError::InvalidTerminal { terminal });
        }
        terminal_index += 1;
    }

    Ok(())
}

fn run_validated(
    lalr: &ValidatedLalr,
    initial_stack: &[ParserStateId],
    terminals: &[TerminalId],
    underflow_mode: UnderflowMode,
    maximum_work: Option<usize>,
) -> Result<RunStatus, ParserError> {
    let mut work = RunWork::new(maximum_work);
    if !work.charge(initial_stack.len())? {
        return Ok(work.exceeded());
    }
    let mut stack = copy_stack(initial_stack)?;
    let mut terminal_index = 0_usize;

    while terminal_index < terminals.len() {
        let terminal = *terminals
            .get(terminal_index)
            .ok_or(ParserError::InvariantViolation)?;
        if !work.charge(1)? {
            return Ok(work.exceeded());
        }
        let ignored = lalr
            .is_ignored(terminal)
            .ok_or(ParserError::InvariantViolation)?;
        if ignored {
            terminal_index += 1;
            continue;
        }

        let mut previous_reduction = None;
        loop {
            let state = stack_top(&stack)?;
            if !work.charge(1)? {
                return Ok(work.exceeded());
            }
            let action = lalr
                .action(state, terminal)
                .ok_or(ParserError::InvariantViolation)?;
            match action {
                Action::Error => return Ok(work.complete(RunOutcome::Rejected)),
                Action::Shift(destination) => {
                    if !work.charge(1)? {
                        return Ok(work.exceeded());
                    }
                    parser_push(&mut stack, destination)?;
                    break;
                }
                Action::Reduce { production, rank } => {
                    check_reduction_progress(previous_reduction, rank, state, terminal)?;
                    let applied =
                        apply_reduction(lalr, &mut stack, production, underflow_mode, &mut work)?;
                    let pop_len = match applied {
                        ReductionResult::Applied { pop_len } => pop_len,
                        ReductionResult::Dependent => {
                            return Ok(work.complete(RunOutcome::Dependent));
                        }
                        ReductionResult::WorkLimitExceeded => return Ok(work.exceeded()),
                    };
                    previous_reduction = Some(PreviousReduction { rank, pop_len });
                }
                Action::Accept => {
                    if terminal_index.checked_add(1) != Some(terminals.len()) {
                        return Err(ParserError::AcceptBeforeEnd { terminal_index });
                    }
                    return Ok(work.complete(RunOutcome::Accepted));
                }
            }
        }
        terminal_index += 1;
    }

    Ok(work.complete(RunOutcome::Continue { stack }))
}

fn unbounded_outcome(status: RunStatus) -> Result<RunOutcome, ParserError> {
    match status {
        RunStatus::Complete { outcome, .. } => Ok(outcome),
        RunStatus::WorkLimitExceeded { .. } => Err(ParserError::InvariantViolation),
    }
}

struct RunWork {
    used: usize,
    maximum: Option<usize>,
}

impl RunWork {
    const fn new(maximum: Option<usize>) -> Self {
        Self { used: 0, maximum }
    }

    fn charge(&mut self, amount: usize) -> Result<bool, ParserError> {
        let Some(maximum) = self.maximum else {
            return Ok(true);
        };
        self.used = self
            .used
            .checked_add(amount)
            .ok_or(ParserError::InvariantViolation)?;
        Ok(self.used <= maximum)
    }

    fn complete(self, outcome: RunOutcome) -> RunStatus {
        RunStatus::Complete {
            outcome,
            work: self.used,
        }
    }

    fn exceeded(self) -> RunStatus {
        RunStatus::WorkLimitExceeded {
            required: self.used,
        }
    }
}

enum ReductionResult {
    Applied { pop_len: u32 },
    Dependent,
    WorkLimitExceeded,
}

fn apply_reduction(
    lalr: &ValidatedLalr,
    stack: &mut Vec<ParserStateId>,
    production_id: ProductionId,
    underflow_mode: UnderflowMode,
    work: &mut RunWork,
) -> Result<ReductionResult, ParserError> {
    if !work.charge(1)? {
        return Ok(ReductionResult::WorkLimitExceeded);
    }
    let production = lalr
        .production(production_id)
        .ok_or(ParserError::InvariantViolation)?;
    let pop_len =
        usize::try_from(production.pop_len()).map_err(|_| ParserError::InvariantViolation)?;

    if pop_len >= stack.len() {
        return match underflow_mode {
            UnderflowMode::Error => Err(ParserError::StackUnderflow {
                production: production_id,
                pop_len: production.pop_len(),
                stack_len: stack.len(),
            }),
            UnderflowMode::Dependent => Ok(ReductionResult::Dependent),
        };
    }

    let reduced_len = stack
        .len()
        .checked_sub(pop_len)
        .ok_or(ParserError::InvariantViolation)?;
    let source = *stack
        .get(
            reduced_len
                .checked_sub(1)
                .ok_or(ParserError::InvariantViolation)?,
        )
        .ok_or(ParserError::InvariantViolation)?;
    if !work.charge(1)? {
        return Ok(ReductionResult::WorkLimitExceeded);
    }
    let destination = match lalr.goto(source, production.lhs()) {
        None => return Err(ParserError::InvariantViolation),
        Some(None) => {
            return Err(ParserError::MissingGoto {
                state: source,
                nonterminal: production.lhs(),
                production: production_id,
            });
        }
        Some(Some(destination)) => destination,
    };

    if !work.charge(1)? {
        return Ok(ReductionResult::WorkLimitExceeded);
    }
    stack.truncate(reduced_len);
    parser_push(stack, destination)?;
    Ok(ReductionResult::Applied {
        pop_len: production.pop_len(),
    })
}

fn check_reduction_progress(
    previous: Option<PreviousReduction>,
    next_rank: u32,
    state: ParserStateId,
    terminal: TerminalId,
) -> Result<(), ParserError> {
    let Some(previous) = previous else {
        return Ok(());
    };
    if next_rank < previous.rank || (next_rank == previous.rank && previous.pop_len >= 2) {
        return Ok(());
    }
    Err(ParserError::InvalidReductionProgress { state, terminal })
}

fn copy_stack(stack: &[ParserStateId]) -> Result<Vec<ParserStateId>, ParserError> {
    let mut copied = reserved_vec(stack.len())
        .map_err(|requested| ParserError::AllocationFailure { requested })?;
    copied.extend_from_slice(stack);
    Ok(copied)
}

fn parser_push(stack: &mut Vec<ParserStateId>, state: ParserStateId) -> Result<(), ParserError> {
    reserve_one(stack).map_err(|requested| ParserError::AllocationFailure { requested })?;
    stack.push(state);
    Ok(())
}

fn stack_top(stack: &[ParserStateId]) -> Result<ParserStateId, ParserError> {
    stack.last().copied().ok_or(ParserError::InvariantViolation)
}
