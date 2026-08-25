use crate::{
    Action, NonterminalId, ParserStateId, ProductionId, SequenceId, TerminalId, ValidatedGrammar,
    ValidatedLalr,
};

use crate::token_step::reserve_one;

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

/// Allocation-free result for parser execution into caller-owned storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunDecision {
    Rejected,
    Continue,
    Accepted,
    Dependent,
}

/// Internal bounded-execution result used during preparation.
pub(crate) enum RunStatus {
    Complete { decision: RunDecision, work: usize },
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
    let mut destination = Vec::new();
    match execute_terminals_concrete_into(grammar.lalr(), stack, terminals, &mut destination)? {
        RunDecision::Rejected => Ok(None),
        RunDecision::Continue => Ok(Some(ParserExecution::Continue { stack: destination })),
        RunDecision::Accepted => Ok(Some(ParserExecution::Accepted)),
        RunDecision::Dependent => Err(ParserError::InvariantViolation),
    }
}

/// Runs from a symbolic stack base for exact sequence-head preprocessing.
///
/// Unlike concrete execution, popping the supplied base returns `Dependent`:
/// the action can only be decided with parser states below that base.
pub(crate) fn execute_terminals_symbolic(
    parser_table: &ValidatedLalr,
    stack: &[ParserStateId],
    terminals: &[TerminalId],
) -> Result<RunDecision, ParserError> {
    validate_inputs(parser_table, stack, terminals)?;
    let mut destination = Vec::new();
    let status = run_validated_into(
        parser_table,
        stack,
        terminals,
        UnderflowMode::Dependent,
        None,
        &mut destination,
    )?;
    unbounded_decision(status)
}

pub(crate) fn execute_terminals_symbolic_checked_with_limit(
    parser_table: &ValidatedLalr,
    stack: &[ParserStateId],
    terminals: &[TerminalId],
    maximum_work: usize,
) -> Result<RunStatus, ParserError> {
    let mut destination = Vec::new();
    let status = run_validated_into(
        parser_table,
        stack,
        terminals,
        UnderflowMode::Dependent,
        Some(maximum_work),
        &mut destination,
    )?;
    match status {
        DecisionStatus::Complete { decision, work } => Ok(RunStatus::Complete { decision, work }),
        DecisionStatus::WorkLimitExceeded { required } => {
            Ok(RunStatus::WorkLimitExceeded { required })
        }
    }
}

/// Executes concrete parser actions into reusable caller-owned stack storage.
/// The destination is overwritten even when parsing rejects or reports an
/// error, while the source stack is never modified.
pub(crate) fn execute_terminals_concrete_into(
    parser_table: &ValidatedLalr,
    stack: &[ParserStateId],
    terminals: &[TerminalId],
    destination: &mut Vec<ParserStateId>,
) -> Result<RunDecision, ParserError> {
    unbounded_decision(run_validated_into(
        parser_table,
        stack,
        terminals,
        UnderflowMode::Error,
        None,
        destination,
    )?)
}

/// Continues concrete parser execution directly in caller-owned stack storage.
pub(crate) fn execute_terminals_concrete_in_place(
    parser_table: &ValidatedLalr,
    stack: &mut Vec<ParserStateId>,
    terminals: &[TerminalId],
) -> Result<RunDecision, ParserError> {
    unbounded_decision(run_validated_in_place(
        parser_table,
        stack,
        terminals,
        UnderflowMode::Error,
        None,
    )?)
}

pub(crate) fn validate_inputs(
    parser_table: &ValidatedLalr,
    stack: &[ParserStateId],
    terminals: &[TerminalId],
) -> Result<(), ParserError> {
    if stack.is_empty() {
        return Err(ParserError::EmptyStack);
    }

    let mut error = None;
    let mut stack_index = 0_usize;
    while stack_index < stack.len() {
        if error.is_none() {
            error = match stack.get(stack_index).copied() {
                Some(state) if state.get() >= parser_table.state_count() => {
                    Some(ParserError::InvalidState { state })
                }
                Some(_) => None,
                None => Some(ParserError::InvariantViolation),
            };
        }
        stack_index += 1;
    }

    match error {
        Some(error) => Err(error),
        None => {
            let mut terminal_error = None;
            let mut terminal_index = 0_usize;
            while terminal_index < terminals.len() {
                if terminal_error.is_none() {
                    terminal_error = match terminals.get(terminal_index).copied() {
                        Some(terminal) if terminal.get() >= parser_table.terminal_count() => {
                            Some(ParserError::InvalidTerminal { terminal })
                        }
                        Some(_) => None,
                        None => Some(ParserError::InvariantViolation),
                    };
                }
                terminal_index += 1;
            }
            match terminal_error {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }
}

fn run_validated_into(
    parser_table: &ValidatedLalr,
    initial_stack: &[ParserStateId],
    terminals: &[TerminalId],
    underflow_mode: UnderflowMode,
    maximum_work: Option<usize>,
    stack: &mut Vec<ParserStateId>,
) -> Result<DecisionStatus, ParserError> {
    let mut work = RunWork::new(maximum_work);
    if !work.charge(initial_stack.len())? {
        return Ok(work.exceeded());
    }
    copy_stack_into(initial_stack, stack)?;
    run_initialized(parser_table, stack, terminals, underflow_mode, work)
}

fn run_validated_in_place(
    parser_table: &ValidatedLalr,
    stack: &mut Vec<ParserStateId>,
    terminals: &[TerminalId],
    underflow_mode: UnderflowMode,
    maximum_work: Option<usize>,
) -> Result<DecisionStatus, ParserError> {
    let mut work = RunWork::new(maximum_work);
    if !work.charge(stack.len())? {
        return Ok(work.exceeded());
    }
    run_initialized(parser_table, stack, terminals, underflow_mode, work)
}

fn run_initialized(
    parser_table: &ValidatedLalr,
    stack: &mut Vec<ParserStateId>,
    terminals: &[TerminalId],
    underflow_mode: UnderflowMode,
    mut work: RunWork,
) -> Result<DecisionStatus, ParserError> {
    let mut terminal_index = 0_usize;

    while terminal_index < terminals.len() {
        let terminal = *terminals
            .get(terminal_index)
            .ok_or(ParserError::InvariantViolation)?;
        if !work.charge(1)? {
            return Ok(work.exceeded());
        }
        let ignored = parser_table
            .is_ignored(terminal)
            .ok_or(ParserError::InvariantViolation)?;
        if !ignored {
            match run_terminal(
                parser_table,
                stack,
                terminal,
                terminal_index,
                terminals.len(),
                underflow_mode,
                &mut work,
            )? {
                TerminalRun::Shifted => {}
                TerminalRun::Complete(decision) => return Ok(work.complete(decision)),
                TerminalRun::WorkLimitExceeded => return Ok(work.exceeded()),
            }
        }
        terminal_index += 1;
    }

    Ok(work.complete(RunDecision::Continue))
}

enum TerminalRun {
    Shifted,
    Complete(RunDecision),
    WorkLimitExceeded,
}

fn run_terminal(
    parser_table: &ValidatedLalr,
    stack: &mut Vec<ParserStateId>,
    terminal: TerminalId,
    terminal_index: usize,
    terminal_count: usize,
    underflow_mode: UnderflowMode,
    work: &mut RunWork,
) -> Result<TerminalRun, ParserError> {
    let mut previous_reduction = None;
    loop {
        let state = stack_top(stack)?;
        if !work.charge(1)? {
            return Ok(TerminalRun::WorkLimitExceeded);
        }
        let action = parser_table
            .action(state, terminal)
            .ok_or(ParserError::InvariantViolation)?;
        match action {
            Action::Error => return Ok(TerminalRun::Complete(RunDecision::Rejected)),
            Action::Shift(destination) => {
                if !work.charge(1)? {
                    return Ok(TerminalRun::WorkLimitExceeded);
                }
                parser_push(stack, destination)?;
                return Ok(TerminalRun::Shifted);
            }
            Action::Reduce { production, rank } => {
                check_reduction_progress(previous_reduction, rank, state, terminal)?;
                let applied =
                    apply_reduction(parser_table, stack, production, underflow_mode, work)?;
                let pop_len = match applied {
                    ReductionResult::Applied { pop_len } => pop_len,
                    ReductionResult::Dependent => {
                        return Ok(TerminalRun::Complete(RunDecision::Dependent));
                    }
                    ReductionResult::WorkLimitExceeded => {
                        return Ok(TerminalRun::WorkLimitExceeded);
                    }
                };
                previous_reduction = Some(PreviousReduction { rank, pop_len });
            }
            Action::Accept => {
                if terminal_index.checked_add(1) != Some(terminal_count) {
                    return Err(ParserError::AcceptBeforeEnd { terminal_index });
                }
                return Ok(TerminalRun::Complete(RunDecision::Accepted));
            }
        }
    }
}

fn unbounded_decision(status: DecisionStatus) -> Result<RunDecision, ParserError> {
    match status {
        DecisionStatus::Complete { decision, .. } => Ok(decision),
        DecisionStatus::WorkLimitExceeded { .. } => Err(ParserError::InvariantViolation),
    }
}

enum DecisionStatus {
    Complete { decision: RunDecision, work: usize },
    WorkLimitExceeded { required: usize },
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

    fn complete(self, decision: RunDecision) -> DecisionStatus {
        DecisionStatus::Complete {
            decision,
            work: self.used,
        }
    }

    fn exceeded(self) -> DecisionStatus {
        DecisionStatus::WorkLimitExceeded {
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
    parser_table: &ValidatedLalr,
    stack: &mut Vec<ParserStateId>,
    production_id: ProductionId,
    underflow_mode: UnderflowMode,
    work: &mut RunWork,
) -> Result<ReductionResult, ParserError> {
    if !work.charge(1)? {
        return Ok(ReductionResult::WorkLimitExceeded);
    }
    let production = parser_table
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
    let destination = match parser_table.goto(source, production.lhs()) {
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

fn copy_stack_into(
    source: &[ParserStateId],
    destination: &mut Vec<ParserStateId>,
) -> Result<(), ParserError> {
    destination.clear();
    if destination.capacity() < source.len() {
        destination.try_reserve_exact(source.len()).map_err(|_| {
            ParserError::AllocationFailure {
                requested: source.len(),
            }
        })?;
    }
    destination.extend_from_slice(source);
    Ok(())
}

fn parser_push(stack: &mut Vec<ParserStateId>, state: ParserStateId) -> Result<(), ParserError> {
    reserve_one(stack).map_err(|requested| ParserError::AllocationFailure { requested })?;
    stack.push(state);
    Ok(())
}

fn stack_top(stack: &[ParserStateId]) -> Result<ParserStateId, ParserError> {
    stack.last().copied().ok_or(ParserError::InvariantViolation)
}
