use std::collections::HashMap;

use greatgramma_core::{DfaStateId, LexerDfa, LimitKind, TerminalId, ValidationError};
use llguidance::earley::regexvec::{RegexVec, StateID};

use crate::{
    CompileError,
    regex::{CompiledRegexes, TerminalMeta},
};

pub(crate) fn build_lexer_dfa(
    mut compiled: CompiledRegexes,
    maximum_states: usize,
    maximum_cells: u64,
    maximum_work: u64,
) -> Result<LexerDfa, CompileError> {
    let maximum_states = maximum_states.min(u32::MAX as usize);
    if maximum_states == 0 {
        return Err(CompileError::TooManyDfaStates {
            required: 1,
            maximum: 0,
        });
    }
    check_regex_output_limit(&compiled.regexes, compiled.max_output_bytes)?;

    let (byte_classes, representatives) = compressed_alphabet(&compiled.regexes)?;
    let class_count = representatives.len();
    check_cell_limit(1, class_count, maximum_cells)?;
    compiled
        .regexes
        .set_max_states(maximum_states.saturating_add(3));
    compiled.regexes.set_fuel(maximum_work.saturating_add(1));

    let mut work = 0_u64;
    let mut states = reserved_vec(1)?;
    fallible_push(&mut states, compiled.start)?;
    let mut state_ids = HashMap::new();
    reserve_index(&mut state_ids, 1)?;
    state_ids.insert(compiled.start, 0_usize);
    let mut transitions = Vec::new();
    let mut labels = Vec::new();
    let mut state_index = 0_usize;

    while let Some(&state) = states.get(state_index) {
        let label = resolve_label(
            &compiled.regexes,
            &compiled.aliases,
            state,
            &mut work,
            maximum_work,
        )?;
        fallible_push(&mut labels, label)?;
        reserve_additional(&mut transitions, class_count)?;

        for &byte in &representatives {
            charge(&mut work, 1, maximum_work)?;
            let next = compiled.regexes.transition(state, byte);
            if compiled.regexes.has_error() {
                return Err(regex_vec_error(
                    &compiled.regexes,
                    states.len().saturating_add(1),
                    maximum_states,
                    maximum_work,
                ));
            }
            check_regex_output_limit(&compiled.regexes, compiled.max_output_bytes)?;
            if next.is_dead() {
                transitions.push(None);
                continue;
            }

            let destination = match state_ids.get(&next).copied() {
                Some(index) => index,
                None => {
                    let required = states.len().saturating_add(1);
                    if required > maximum_states {
                        return Err(CompileError::TooManyDfaStates {
                            required,
                            maximum: maximum_states,
                        });
                    }
                    check_cell_limit(required, class_count, maximum_cells)?;
                    reserve_additional(&mut states, 1)?;
                    reserve_index(&mut state_ids, required)?;
                    states.push(next);
                    state_ids.insert(next, required - 1);
                    required - 1
                }
            };
            transitions.push(Some(DfaStateId::new(destination as u32)));
        }
        state_index += 1;
    }

    check_regex_output_limit(&compiled.regexes, compiled.max_output_bytes)?;
    charge(&mut work, transitions.len() as u64, maximum_work)?;
    reject_multi_byte_fallback(&labels, &transitions, &representatives)?;

    Ok(LexerDfa::new(
        states.len() as u32,
        class_count as u32,
        byte_classes,
        transitions,
        DfaStateId::new(0),
        labels,
    ))
}

fn check_regex_output_limit(regexes: &RegexVec, maximum: usize) -> Result<(), CompileError> {
    let required = regexes.num_bytes();
    if required > maximum {
        return Err(CompileError::RegexOutputLimit { required, maximum });
    }
    Ok(())
}

fn compressed_alphabet(regexes: &RegexVec) -> Result<(Vec<u32>, Vec<u8>), CompileError> {
    let class_count = regexes.alpha().len();
    let mut representatives_by_class = reserved_vec(class_count)?;
    representatives_by_class.resize(class_count, None);
    let mut byte_classes = reserved_vec(usize::from(u8::MAX) + 1)?;
    for byte in u8::MIN..=u8::MAX {
        let class = regexes.alpha().map(byte);
        representatives_by_class[class].get_or_insert(byte);
        byte_classes.push(class as u32);
    }

    let mut representatives = reserved_vec(class_count)?;
    for byte in representatives_by_class {
        representatives.push(byte.expect("derivre alphabet classes are nonempty"));
    }
    Ok((byte_classes, representatives))
}

fn resolve_label(
    regexes: &RegexVec,
    aliases: &[Option<TerminalMeta>],
    state: StateID,
    work: &mut u64,
    maximum_work: u64,
) -> Result<Option<TerminalId>, CompileError> {
    let mut winner: Option<&TerminalMeta> = None;
    for index in regexes.state_desc(state).greedy_accepting.as_slice() {
        let Some(terminal) = aliases[index.as_usize()].as_ref() else {
            continue;
        };
        charge(work, 1, maximum_work)?;
        match winner {
            None => winner = Some(terminal),
            Some(previous) if terminal.priority < previous.priority => winner = Some(terminal),
            Some(previous) if terminal.priority == previous.priority => {
                return Err(CompileError::AmbiguousPriority {
                    first: fallible_string_copy(&previous.name)?,
                    second: fallible_string_copy(&terminal.name)?,
                    priority: terminal.priority,
                });
            }
            Some(_) => {}
        }
    }
    Ok(winner.map(|terminal| terminal.terminal))
}

fn reject_multi_byte_fallback(
    labels: &[Option<TerminalId>],
    transitions: &[Option<DfaStateId>],
    representatives: &[u8],
) -> Result<(), CompileError> {
    for (source, _) in labels
        .iter()
        .enumerate()
        .filter(|(_, label)| label.is_some())
    {
        for (class, &byte) in representatives.iter().enumerate() {
            let destination = transitions[source * representatives.len() + class];
            if let Some(destination) = destination
                && labels[destination.get() as usize].is_none()
            {
                return Err(CompileError::UnsupportedLexerBoundary {
                    state: source as u32,
                    byte,
                });
            }
        }
    }
    Ok(())
}

fn regex_vec_error(
    regexes: &RegexVec,
    required_states: usize,
    maximum_states: usize,
    maximum_work: u64,
) -> CompileError {
    if regexes.get_fuel() == 0 {
        CompileError::CompilerWorkLimit {
            required: maximum_work.saturating_add(1),
            maximum: maximum_work,
        }
    } else {
        CompileError::TooManyDfaStates {
            required: required_states,
            maximum: maximum_states,
        }
    }
}

fn check_cell_limit(
    state_count: usize,
    class_count: usize,
    maximum: u64,
) -> Result<(), CompileError> {
    let required = (state_count as u64).saturating_mul(class_count as u64);
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

fn reserve_additional<T>(output: &mut Vec<T>, additional: usize) -> Result<(), CompileError> {
    output
        .try_reserve_exact(additional)
        .map_err(|_| CompileError::AllocationFailure {
            requested: output.len().saturating_add(additional),
        })
}

fn fallible_push<T>(output: &mut Vec<T>, value: T) -> Result<(), CompileError> {
    if output.len() == output.capacity() {
        reserve_additional(output, 1)?;
    }
    output.push(value);
    Ok(())
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

fn reserve_index(
    index: &mut HashMap<StateID, usize>,
    requested: usize,
) -> Result<(), CompileError> {
    index
        .try_reserve(1)
        .map_err(|_| CompileError::AllocationFailure { requested })
}
