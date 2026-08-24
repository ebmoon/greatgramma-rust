use crate::{
    LexerError, LexerInput, LexerStep, PreparationError, PreparationLimits, SequenceId, TerminalId,
    ValidatedGrammar, lexer_step,
};

use crate::token_step::{
    WorkBudget, check_items, prepared_push, prepared_vec, source_at, source_count, source_index,
};

#[derive(Clone, Copy)]
struct Fact {
    source: usize,
    terminal: TerminalId,
}

/// Exact singleton terminals and plain, deterministically interned heads.
pub(crate) struct SequenceTable {
    singletons: Vec<Vec<TerminalId>>,
    sequences: Vec<Vec<TerminalId>>,
    sequence_items: usize,
}

impl SequenceTable {
    pub(crate) fn build_singletons(
        grammar: &ValidatedGrammar,
        limits: PreparationLimits,
        work: &mut WorkBudget,
    ) -> Result<Self, PreparationError> {
        let sources = source_count(grammar)?;
        let terminals = usize::try_from(grammar.lalr().terminal_count())
            .map_err(|_| PreparationError::InvariantViolation)?;
        let fact_cells = check_items(sources.checked_mul(terminals), limits)?;
        let byte_steps = check_items(sources.checked_mul(256), limits)?;
        work.charge(byte_steps)?;

        let mut seen = prepared_vec(fact_cells)?;
        seen.resize(fact_cells, false);

        let mut predecessors = prepared_vec(sources)?;
        let mut predecessor_row = 0_usize;
        while predecessor_row < sources {
            predecessors.push(Vec::new());
            predecessor_row += 1;
        }

        let mut facts = Vec::new();
        let mut source = 0_usize;
        while source < sources {
            scan_source(
                grammar,
                source,
                terminals,
                &mut seen,
                &mut facts,
                &mut predecessors,
                work,
            )?;
            source += 1;
        }

        let mut cursor = 0_usize;
        while cursor < facts.len() {
            let fact = *facts
                .get(cursor)
                .ok_or(PreparationError::InvariantViolation)?;
            propagate_fact(&predecessors, &mut seen, &mut facts, fact, terminals, work)?;
            cursor += 1;
        }

        let mut singletons = prepared_vec(sources)?;
        source = 0;
        while source < sources {
            work.charge(terminals)?;
            singletons.push(collect_singletons(&seen, source, terminals)?);
            source += 1;
        }

        Ok(Self {
            singletons,
            sequences: Vec::new(),
            sequence_items: 0,
        })
    }

    pub(crate) fn singleton(&self, source: usize) -> Option<&[TerminalId]> {
        self.singletons.get(source).map(Vec::as_slice)
    }

    pub(crate) fn sequence_count(&self) -> usize {
        self.sequences.len()
    }

    pub(crate) fn sequence(&self, sequence: SequenceId) -> Option<&[TerminalId]> {
        let index = usize::try_from(sequence.get()).ok()?;
        self.sequences.get(index).map(Vec::as_slice)
    }

    pub(crate) fn intern_head(
        &mut self,
        direct: &[TerminalId],
        continuation: TerminalId,
        limits: PreparationLimits,
        work: &mut WorkBudget,
    ) -> Result<SequenceId, PreparationError> {
        let head_len = check_items(direct.len().checked_add(1), limits)?;
        work.charge(direct.len())?;
        let mut head = prepared_vec(head_len)?;
        let mut direct_index = 0_usize;
        while direct_index < direct.len() {
            head.push(
                *direct
                    .get(direct_index)
                    .ok_or(PreparationError::InvariantViolation)?,
            );
            direct_index += 1;
        }
        head.push(continuation);

        let mut index = 0_usize;
        while index < self.sequences.len() {
            work.charge(head_len)?;
            if self.sequences.get(index) == Some(&head) {
                return sequence_id(index);
            }
            index += 1;
        }

        check_items(self.sequences.len().checked_add(1), limits)?;
        let sequence = sequence_id(self.sequences.len())?;
        let sequence_items = check_items(self.sequence_items.checked_add(head_len), limits)?;
        prepared_push(&mut self.sequences, head)?;
        self.sequence_items = sequence_items;
        Ok(sequence)
    }
}

fn scan_source(
    grammar: &ValidatedGrammar,
    source: usize,
    terminal_count: usize,
    seen: &mut [bool],
    facts: &mut Vec<Fact>,
    predecessors: &mut [Vec<usize>],
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    let lexer_source = source_at(source).ok_or(PreparationError::InvariantViolation)?;
    let mut destinations = prepared_vec(256)?;
    let mut byte = 0_u16;
    while byte < 256 {
        let input = u8::try_from(byte).map_err(|_| PreparationError::InvariantViolation)?;
        match lexer_step(grammar, lexer_source, LexerInput::Byte(input)) {
            Ok(LexerStep::Continue {
                emitted: Some(terminal),
                ..
            }) => add_fact(seen, facts, source, terminal, terminal_count)?,
            Ok(LexerStep::Continue {
                state: destination,
                emitted: None,
            }) => {
                let destination = source_index(grammar, destination)
                    .ok_or(PreparationError::InvariantViolation)?;
                work.charge(destinations.len())?;
                if !destinations.contains(&destination) {
                    let row = predecessors
                        .get_mut(destination)
                        .ok_or(PreparationError::InvariantViolation)?;
                    prepared_push(row, source)?;
                    destinations.push(destination);
                }
            }
            Ok(LexerStep::Finished { .. }) => {}
            Err(LexerError::InvalidState { .. }) => {
                return Err(PreparationError::InvariantViolation);
            }
            Err(
                LexerError::CannotBeginLexeme { .. }
                | LexerError::ByteAfterUnfinishedResidual { .. }
                | LexerError::EosAfterUnfinishedResidual { .. },
            ) => {}
        }
        byte += 1;
    }
    Ok(())
}

fn propagate_fact(
    predecessors: &[Vec<usize>],
    seen: &mut [bool],
    facts: &mut Vec<Fact>,
    fact: Fact,
    terminal_count: usize,
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    let row = predecessors
        .get(fact.source)
        .ok_or(PreparationError::InvariantViolation)?;
    work.charge(row.len())?;
    let mut predecessor_index = 0_usize;
    while predecessor_index < row.len() {
        let predecessor = *row
            .get(predecessor_index)
            .ok_or(PreparationError::InvariantViolation)?;
        add_fact(seen, facts, predecessor, fact.terminal, terminal_count)?;
        predecessor_index += 1;
    }
    Ok(())
}

fn collect_singletons(
    seen: &[bool],
    source: usize,
    terminal_count: usize,
) -> Result<Vec<TerminalId>, PreparationError> {
    let mut row = Vec::new();
    let mut terminal = 0_usize;
    while terminal < terminal_count {
        let index = fact_index(source, terminal, terminal_count)?;
        if *seen
            .get(index)
            .ok_or(PreparationError::InvariantViolation)?
        {
            prepared_push(
                &mut row,
                TerminalId::new(
                    u32::try_from(terminal).map_err(|_| PreparationError::InvariantViolation)?,
                ),
            )?;
        }
        terminal += 1;
    }
    Ok(row)
}

fn add_fact(
    seen: &mut [bool],
    facts: &mut Vec<Fact>,
    source: usize,
    terminal: TerminalId,
    terminal_count: usize,
) -> Result<(), PreparationError> {
    let terminal_index =
        usize::try_from(terminal.get()).map_err(|_| PreparationError::InvariantViolation)?;
    if terminal_index >= terminal_count {
        return Err(PreparationError::InvariantViolation);
    }
    let index = fact_index(source, terminal_index, terminal_count)?;
    let present = seen
        .get_mut(index)
        .ok_or(PreparationError::InvariantViolation)?;
    if !*present {
        *present = true;
        prepared_push(facts, Fact { source, terminal })?;
    }
    Ok(())
}

fn fact_index(
    source: usize,
    terminal: usize,
    terminal_count: usize,
) -> Result<usize, PreparationError> {
    source
        .checked_mul(terminal_count)
        .and_then(|start| start.checked_add(terminal))
        .ok_or(PreparationError::InvariantViolation)
}

fn sequence_id(index: usize) -> Result<SequenceId, PreparationError> {
    Ok(SequenceId::new(
        u32::try_from(index).map_err(|_| PreparationError::InvariantViolation)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_table() -> SequenceTable {
        SequenceTable {
            singletons: Vec::new(),
            sequences: Vec::new(),
            sequence_items: 0,
        }
    }

    #[test]
    fn coarse_limits_bound_sequence_storage_and_lookup_work() {
        let limits = PreparationLimits {
            max_items: 2,
            ..PreparationLimits::default()
        };
        let mut table = empty_table();
        let mut work = WorkBudget::new(limits);
        assert_eq!(
            table.intern_head(&[TerminalId::new(0)], TerminalId::new(1), limits, &mut work,),
            Ok(SequenceId::new(0))
        );
        assert_eq!(
            table
                .intern_head(&[TerminalId::new(1)], TerminalId::new(0), limits, &mut work,)
                .err(),
            Some(PreparationError::TooLarge {
                required: 4,
                maximum: 2,
            })
        );

        let lookup_limits = PreparationLimits {
            max_work: 1,
            ..PreparationLimits::default()
        };
        let mut lookup_work = WorkBudget::new(lookup_limits);
        assert_eq!(
            table
                .intern_head(
                    &[TerminalId::new(0)],
                    TerminalId::new(1),
                    lookup_limits,
                    &mut lookup_work,
                )
                .err(),
            Some(PreparationError::TooLarge {
                required: 3,
                maximum: 1,
            })
        );
    }
}
