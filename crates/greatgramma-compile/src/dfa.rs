use std::collections::HashMap;

use derivre::StateID;
use greatgramma_core::{DfaStateId, LexerDfa, LimitKind, TerminalId, ValidationError};

use crate::{CompileError, regex::CompiledTerminal};

pub(crate) fn build_product_dfa(
    mut terminals: Vec<CompiledTerminal>,
    maximum_states: usize,
    maximum_cells: u64,
    maximum_work: u64,
) -> Result<LexerDfa, CompileError> {
    if maximum_states == 0 {
        return Err(CompileError::TooManyDfaStates {
            required: 1,
            maximum: 0,
        });
    }
    check_cell_limit(1, maximum_cells)?;

    let mut work = 0_u64;
    charge(&mut work, as_u64(terminals.len()), maximum_work)?;
    let mut start = reserved_vec(terminals.len())?;
    for terminal in &mut terminals {
        fallible_push(&mut start, terminal.regex.initial_state())?;
    }

    let mut states = reserved_vec(1)?;
    let start_key = fallible_copy(&start)?;
    fallible_push(&mut states, start)?;
    let mut state_ids = HashMap::new();
    reserve_index(&mut state_ids, 1)?;
    state_ids.insert(start_key, 0_usize);
    let mut transitions = Vec::new();
    let mut labels = Vec::new();
    let mut state_index = 0_usize;

    while state_index < states.len() {
        charge(&mut work, as_u64(terminals.len()), maximum_work)?;
        let state = fallible_copy(&states[state_index])?;
        charge(&mut work, as_u64(terminals.len()), maximum_work)?;
        fallible_push(&mut labels, resolve_label(&mut terminals, &state)?)?;
        transitions
            .try_reserve_exact(256)
            .map_err(|_| CompileError::AllocationFailure {
                requested: transitions.len().saturating_add(256),
            })?;
        for byte in u8::MIN..=u8::MAX {
            let terminal_work = as_u64(terminals.len());
            let byte_work = terminal_work.saturating_mul(4).saturating_add(1);
            charge(&mut work, byte_work, maximum_work)?;
            let next = transition(&mut terminals, &state, byte)?;
            if next.iter().all(StateID::is_dead) {
                transitions.push(None);
                continue;
            }
            let destination = match state_ids.get(&next).copied() {
                Some(index) => index,
                None => {
                    let required = states.len().saturating_add(1);
                    if required > maximum_states || required > u32::MAX as usize {
                        return Err(CompileError::TooManyDfaStates {
                            required,
                            maximum: maximum_states.min(u32::MAX as usize),
                        });
                    }
                    check_cell_limit(required, maximum_cells)?;
                    let stored_state = fallible_copy(&next)?;
                    states
                        .try_reserve(1)
                        .map_err(|_| CompileError::AllocationFailure {
                            requested: required,
                        })?;
                    reserve_index(&mut state_ids, required)?;
                    state_ids.insert(next, required - 1);
                    states.push(stored_state);
                    required - 1
                }
            };
            transitions.push(Some(DfaStateId::new(u32::try_from(destination).map_err(
                |_| CompileError::TooManyDfaStates {
                    required: destination.saturating_add(1),
                    maximum: u32::MAX as usize,
                },
            )?)));
        }
        state_index += 1;
    }

    let state_count = u32::try_from(states.len()).map_err(|_| CompileError::TooManyDfaStates {
        required: states.len(),
        maximum: u32::MAX as usize,
    })?;
    charge(&mut work, as_u64(transitions.len()), maximum_work)?;
    reject_multi_byte_fallback(&labels, &transitions)?;
    let mut byte_classes = reserved_vec(256)?;
    for byte in 0_u32..=255 {
        byte_classes.push(byte);
    }
    Ok(LexerDfa::new(
        state_count,
        256,
        byte_classes,
        transitions,
        DfaStateId::new(0),
        labels,
    ))
}

fn reject_multi_byte_fallback(
    labels: &[Option<TerminalId>],
    transitions: &[Option<DfaStateId>],
) -> Result<(), CompileError> {
    let mut source = 0_usize;
    while source < labels.len() {
        if labels[source].is_some() {
            let mut byte = 0_usize;
            while byte < 256 {
                let cell = source
                    .checked_mul(256)
                    .and_then(|offset| offset.checked_add(byte))
                    .ok_or(CompileError::TooManyDfaStates {
                        required: labels.len(),
                        maximum: u32::MAX as usize,
                    })?;
                if let Some(destination) = transitions.get(cell).copied().flatten() {
                    let destination_index = usize::try_from(destination.get()).map_err(|_| {
                        CompileError::TooManyDfaStates {
                            required: labels.len(),
                            maximum: u32::MAX as usize,
                        }
                    })?;
                    if labels.get(destination_index).copied().flatten().is_none() {
                        return Err(CompileError::UnsupportedLexerBoundary {
                            state: u32::try_from(source).map_err(|_| {
                                CompileError::TooManyDfaStates {
                                    required: labels.len(),
                                    maximum: u32::MAX as usize,
                                }
                            })?,
                            byte: u8::try_from(byte).map_err(|_| {
                                CompileError::TooManyDfaStates {
                                    required: labels.len(),
                                    maximum: u32::MAX as usize,
                                }
                            })?,
                        });
                    }
                }
                byte += 1;
            }
        }
        source += 1;
    }
    Ok(())
}

fn transition(
    terminals: &mut [CompiledTerminal],
    source: &[StateID],
    byte: u8,
) -> Result<Vec<StateID>, CompileError> {
    let mut destination = reserved_vec(terminals.len())?;
    for (terminal, state) in terminals.iter_mut().zip(source) {
        destination.push(terminal.regex.transition(*state, byte));
    }
    Ok(destination)
}

fn resolve_label(
    terminals: &mut [CompiledTerminal],
    state: &[StateID],
) -> Result<Option<TerminalId>, CompileError> {
    let mut winner: Option<usize> = None;
    for index in 0..terminals.len() {
        if !terminals[index].regex.is_accepting(state[index]) {
            continue;
        }
        match winner {
            None => winner = Some(index),
            Some(previous) if terminals[index].priority < terminals[previous].priority => {
                winner = Some(index);
            }
            Some(previous) if terminals[index].priority == terminals[previous].priority => {
                return Err(CompileError::AmbiguousPriority {
                    first: fallible_string_copy(&terminals[previous].name)?,
                    second: fallible_string_copy(&terminals[index].name)?,
                    priority: terminals[index].priority,
                });
            }
            Some(_) => {}
        }
    }
    Ok(winner.map(|index| terminals[index].terminal))
}

fn check_cell_limit(state_count: usize, maximum: u64) -> Result<(), CompileError> {
    let required = as_u64(state_count).saturating_mul(256);
    if required > maximum {
        return Err(CompileError::Validation(ValidationError::LimitExceeded {
            limit: LimitKind::DfaCells,
            actual: required,
            maximum,
        }));
    }
    Ok(())
}

fn charge(work: &mut u64, amount: u64, maximum: u64) -> Result<(), CompileError> {
    *work = work.saturating_add(amount);
    if *work > maximum {
        return Err(CompileError::CompilerWorkLimit {
            required: *work,
            maximum,
        });
    }
    Ok(())
}

fn reserved_vec<T>(capacity: usize) -> Result<Vec<T>, CompileError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| CompileError::AllocationFailure {
            requested: capacity,
        })?;
    Ok(output)
}

fn fallible_copy<T: Copy>(source: &[T]) -> Result<Vec<T>, CompileError> {
    let mut output = reserved_vec(source.len())?;
    output.extend_from_slice(source);
    Ok(output)
}

fn fallible_string_copy(source: &str) -> Result<String, CompileError> {
    let mut output = String::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| CompileError::AllocationFailure {
            requested: source.len(),
        })?;
    output.push_str(source);
    Ok(output)
}

fn fallible_push<T>(output: &mut Vec<T>, value: T) -> Result<(), CompileError> {
    if output.len() == output.capacity() {
        output
            .try_reserve(1)
            .map_err(|_| CompileError::AllocationFailure {
                requested: output.len().saturating_add(1),
            })?;
    }
    output.push(value);
    Ok(())
}

fn reserve_index(
    index: &mut HashMap<Vec<StateID>, usize>,
    requested: usize,
) -> Result<(), CompileError> {
    index
        .try_reserve(1)
        .map_err(|_| CompileError::AllocationFailure { requested })
}

fn as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
