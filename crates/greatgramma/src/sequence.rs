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
        seen.resize(fact_cells, 0);

        let mut predecessors = prepared_vec(sources)?;
        predecessors.resize_with(sources, Vec::new);

        let mut facts = Vec::new();
        scan_sources(
            grammar,
            sources,
            terminals,
            &mut seen,
            &mut facts,
            &mut predecessors,
            work,
        )?;
        propagate_facts(&predecessors, &mut seen, &mut facts, terminals, work)?;
        let singletons = collect_all_singletons(&seen, sources, terminals, work)?;

        Ok(Self {
            singletons,
            sequences: Vec::new(),
            sequence_items: 0,
        })
    }

    pub(crate) fn singleton(&self, source: usize) -> Option<&[TerminalId]> {
        self.singletons.get(source).map(Vec::as_slice)
    }

    pub(crate) fn singleton_terminal(&self, source: usize, index: usize) -> Option<TerminalId> {
        self.singletons.get(source)?.get(index).copied()
    }

    pub(crate) fn singleton_count(&self, source: usize) -> Option<usize> {
        self.singletons.get(source).map(Vec::len)
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
        head.extend_from_slice(direct);
        head.push(continuation);

        match find_sequence(&self.sequences, &head, work)? {
            Some(index) => sequence_id(index),
            None => {
                check_items(self.sequences.len().checked_add(1), limits)?;
                let sequence = sequence_id(self.sequences.len())?;
                let sequence_items =
                    check_items(self.sequence_items.checked_add(head_len), limits)?;
                prepared_push(&mut self.sequences, head)?;
                self.sequence_items = sequence_items;
                Ok(sequence)
            }
        }
    }
}

fn scan_sources(
    grammar: &ValidatedGrammar,
    source_count: usize,
    terminal_count: usize,
    seen: &mut [u8],
    facts: &mut Vec<Fact>,
    predecessors: &mut [Vec<usize>],
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    for source in 0..source_count {
        scan_source(
            grammar,
            source,
            terminal_count,
            seen,
            facts,
            predecessors,
            work,
        )?;
    }
    Ok(())
}

fn propagate_facts(
    predecessors: &[Vec<usize>],
    seen: &mut [u8],
    facts: &mut Vec<Fact>,
    terminal_count: usize,
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    // Newly discovered facts are appended to this FIFO worklist as it is traversed.
    let mut cursor = 0;
    while let Some(fact) = facts.get(cursor).copied() {
        propagate_fact(predecessors, seen, facts, fact, terminal_count, work)?;
        cursor += 1;
    }
    Ok(())
}

fn collect_all_singletons(
    seen: &[u8],
    source_count: usize,
    terminal_count: usize,
    work: &mut WorkBudget,
) -> Result<Vec<Vec<TerminalId>>, PreparationError> {
    let mut singletons = prepared_vec(source_count)?;
    for source in 0..source_count {
        work.charge(terminal_count)?;
        singletons.push(collect_singletons(seen, source, terminal_count)?);
    }
    Ok(singletons)
}

fn find_sequence(
    sequences: &[Vec<TerminalId>],
    head: &[TerminalId],
    work: &mut WorkBudget,
) -> Result<Option<usize>, PreparationError> {
    for (index, sequence) in sequences.iter().enumerate() {
        work.charge(head.len())?;
        if sequence == head {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn scan_source(
    grammar: &ValidatedGrammar,
    source: usize,
    terminal_count: usize,
    seen: &mut [u8],
    facts: &mut Vec<Fact>,
    predecessors: &mut [Vec<usize>],
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    let lexer_source = source_at(source).ok_or(PreparationError::InvariantViolation)?;
    if let crate::LexerState::Dfa(state) = lexer_source {
        if let Some(Some(terminal)) = grammar.lexer().terminal(state) {
            add_fact(seen, facts, source, terminal, terminal_count)?;
        }
    }
    let mut destinations = prepared_vec(256)?;
    for byte in u8::MIN..=u8::MAX {
        scan_byte(
            grammar,
            lexer_source,
            byte,
            source,
            terminal_count,
            seen,
            facts,
            predecessors,
            &mut destinations,
            work,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn scan_byte(
    grammar: &ValidatedGrammar,
    lexer_source: crate::LexerState,
    byte: u8,
    source: usize,
    terminal_count: usize,
    seen: &mut [u8],
    facts: &mut Vec<Fact>,
    predecessors: &mut [Vec<usize>],
    destinations: &mut Vec<usize>,
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    match lexer_step(grammar, lexer_source, LexerInput::Byte(byte)) {
        Ok(LexerStep::Continue {
            emitted: Some(terminal),
            ..
        }) => add_fact(seen, facts, source, terminal, terminal_count),
        Ok(LexerStep::Continue {
            state: destination,
            emitted: None,
        }) => {
            let destination =
                source_index(grammar, destination).ok_or(PreparationError::InvariantViolation)?;
            work.charge(destinations.len())?;
            if !destinations.contains(&destination) {
                append_predecessor(predecessors, destination, source)?;
                destinations.push(destination);
            }
            Ok(())
        }
        Ok(LexerStep::Finished { .. }) => Ok(()),
        Err(LexerError::InvalidState { .. }) => Err(PreparationError::InvariantViolation),
        Err(
            LexerError::CannotBeginLexeme { .. }
            | LexerError::ByteAfterUnfinishedResidual { .. }
            | LexerError::EosAfterUnfinishedResidual { .. },
        ) => Ok(()),
    }
}

fn append_predecessor(
    predecessors: &mut [Vec<usize>],
    destination: usize,
    source: usize,
) -> Result<(), PreparationError> {
    let row = predecessors
        .get_mut(destination)
        .ok_or(PreparationError::InvariantViolation)?;
    prepared_push(row, source)
}

fn propagate_fact(
    predecessors: &[Vec<usize>],
    seen: &mut [u8],
    facts: &mut Vec<Fact>,
    fact: Fact,
    terminal_count: usize,
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    let row = predecessors
        .get(fact.source)
        .ok_or(PreparationError::InvariantViolation)?;
    work.charge(row.len())?;
    for &predecessor in row {
        add_fact(seen, facts, predecessor, fact.terminal, terminal_count)?;
    }
    Ok(())
}

fn collect_singletons(
    seen: &[u8],
    source: usize,
    terminal_count: usize,
) -> Result<Vec<TerminalId>, PreparationError> {
    let mut row = Vec::new();
    for terminal in 0..terminal_count {
        let index = fact_index(source, terminal, terminal_count)?;
        if *seen
            .get(index)
            .ok_or(PreparationError::InvariantViolation)?
            != 0
        {
            let terminal =
                u32::try_from(terminal).map_err(|_| PreparationError::InvariantViolation)?;
            prepared_push(&mut row, TerminalId::new(terminal))?;
        }
    }
    Ok(row)
}

fn add_fact(
    seen: &mut [u8],
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
    if *present == 0 {
        *present = 1;
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
