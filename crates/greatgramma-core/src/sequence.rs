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
        let mut predecessor_row = 0_usize;
        while predecessor_row < sources {
            predecessors.push(Vec::new());
            predecessor_row += 1;
        }

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
        match self.singletons.get(source) {
            Some(values) => Some(values),
            None => None,
        }
    }

    pub(crate) fn singleton_terminal(&self, source: usize, index: usize) -> Option<TerminalId> {
        match self.singletons.get(source) {
            Some(values) => values.get(index).copied(),
            None => None,
        }
    }

    pub(crate) fn singleton_count(&self, source: usize) -> Option<usize> {
        let values = self.singletons.get(source)?;
        Some(values.len())
    }

    pub(crate) fn sequence_count(&self) -> usize {
        self.sequences.len()
    }

    pub(crate) fn sequence(&self, sequence: SequenceId) -> Option<&[TerminalId]> {
        let index = usize::try_from(sequence.get()).ok()?;
        match self.sequences.get(index) {
            Some(values) => Some(values),
            None => None,
        }
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
        copy_direct(direct, &mut head)?;
        head.push(continuation);

        match find_sequence(&self.sequences, &head, head_len, work)? {
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
    let mut error = None;
    let mut source = 0_usize;
    while source < source_count && error.is_none() {
        error = scan_source(
            grammar,
            source,
            terminal_count,
            seen,
            facts,
            predecessors,
            work,
        )
        .err();
        source += 1;
    }
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn propagate_facts(
    predecessors: &[Vec<usize>],
    seen: &mut [u8],
    facts: &mut Vec<Fact>,
    terminal_count: usize,
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    let mut error = None;
    let mut cursor = 0_usize;
    while cursor < facts.len() && error.is_none() {
        error = match facts.get(cursor).copied() {
            Some(fact) => {
                propagate_fact(predecessors, seen, facts, fact, terminal_count, work).err()
            }
            None => Some(PreparationError::InvariantViolation),
        };
        cursor += 1;
    }
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn collect_all_singletons(
    seen: &[u8],
    source_count: usize,
    terminal_count: usize,
    work: &mut WorkBudget,
) -> Result<Vec<Vec<TerminalId>>, PreparationError> {
    let mut singletons = prepared_vec(source_count)?;
    let mut error = None;
    let mut source = 0_usize;
    while source < source_count && error.is_none() {
        error = match collect_singleton_row(seen, source, terminal_count, work) {
            Ok(row) => {
                singletons.push(row);
                None
            }
            Err(row_error) => Some(row_error),
        };
        source += 1;
    }
    match error {
        Some(error) => Err(error),
        None => Ok(singletons),
    }
}

fn collect_singleton_row(
    seen: &[u8],
    source: usize,
    terminal_count: usize,
    work: &mut WorkBudget,
) -> Result<Vec<TerminalId>, PreparationError> {
    work.charge(terminal_count)?;
    collect_singletons(seen, source, terminal_count)
}

fn copy_direct(
    direct: &[TerminalId],
    destination: &mut Vec<TerminalId>,
) -> Result<(), PreparationError> {
    let mut error = None;
    let mut index = 0_usize;
    while index < direct.len() && error.is_none() {
        error = match direct.get(index).copied() {
            Some(terminal) => {
                destination.push(terminal);
                None
            }
            None => Some(PreparationError::InvariantViolation),
        };
        index += 1;
    }
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn find_sequence(
    sequences: &[Vec<TerminalId>],
    head: &[TerminalId],
    head_len: usize,
    work: &mut WorkBudget,
) -> Result<Option<usize>, PreparationError> {
    let mut found = None;
    let mut error = None;
    let mut index = 0_usize;
    while index < sequences.len() && found.is_none() && error.is_none() {
        error = match work.charge(head_len) {
            Ok(()) => {
                if matches_sequence(sequences.get(index), head) {
                    found = Some(index);
                }
                None
            }
            Err(charge_error) => Some(charge_error),
        };
        index += 1;
    }
    match error {
        Some(error) => Err(error),
        None => Ok(found),
    }
}

fn matches_sequence(sequence: Option<&Vec<TerminalId>>, head: &[TerminalId]) -> bool {
    match sequence {
        Some(sequence) => sequence == head,
        None => false,
    }
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
    if let crate::LexerState::Dfa(state) = lexer_source
        && let Some(Some(terminal)) = grammar.lexer().terminal(state)
    {
        add_fact(seen, facts, source, terminal, terminal_count)?;
    }
    let mut destinations = prepared_vec(256)?;
    let mut error = None;
    let mut byte = 0_u16;
    while byte < 256 && error.is_none() {
        error = scan_byte(
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
        )
        .err();
        byte += 1;
    }
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_byte(
    grammar: &ValidatedGrammar,
    lexer_source: crate::LexerState,
    byte: u16,
    source: usize,
    terminal_count: usize,
    seen: &mut [u8],
    facts: &mut Vec<Fact>,
    predecessors: &mut [Vec<usize>],
    destinations: &mut Vec<usize>,
    work: &mut WorkBudget,
) -> Result<(), PreparationError> {
    let input = u8::try_from(byte).map_err(|_| PreparationError::InvariantViolation)?;
    match lexer_step(grammar, lexer_source, LexerInput::Byte(input)) {
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
                match append_predecessor(predecessors, destination, source) {
                    Ok(()) => destinations.push(destination),
                    Err(error) => return Err(error),
                }
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
    let row = match predecessors.get_mut(destination) {
        Some(row) => row,
        None => return Err(PreparationError::InvariantViolation),
    };
    match prepared_push(row, source) {
        Ok(()) => Ok(()),
        Err(error) => Err(error),
    }
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
    let mut error = None;
    let mut predecessor_index = 0_usize;
    while predecessor_index < row.len() && error.is_none() {
        error = propagate_predecessor(
            row,
            predecessor_index,
            seen,
            facts,
            fact.terminal,
            terminal_count,
        )
        .err();
        predecessor_index += 1;
    }
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn propagate_predecessor(
    row: &[usize],
    predecessor_index: usize,
    seen: &mut [u8],
    facts: &mut Vec<Fact>,
    terminal: TerminalId,
    terminal_count: usize,
) -> Result<(), PreparationError> {
    let predecessor = *row
        .get(predecessor_index)
        .ok_or(PreparationError::InvariantViolation)?;
    add_fact(seen, facts, predecessor, terminal, terminal_count)
}

fn collect_singletons(
    seen: &[u8],
    source: usize,
    terminal_count: usize,
) -> Result<Vec<TerminalId>, PreparationError> {
    let mut row = Vec::new();
    let mut error = None;
    let mut terminal = 0_usize;
    while terminal < terminal_count && error.is_none() {
        error = collect_singleton(seen, source, terminal, terminal_count, &mut row).err();
        terminal += 1;
    }
    match error {
        Some(error) => Err(error),
        None => Ok(row),
    }
}

fn collect_singleton(
    seen: &[u8],
    source: usize,
    terminal: usize,
    terminal_count: usize,
    row: &mut Vec<TerminalId>,
) -> Result<(), PreparationError> {
    let index = fact_index(source, terminal, terminal_count)?;
    if *seen
        .get(index)
        .ok_or(PreparationError::InvariantViolation)?
        != 0
    {
        prepared_push(
            row,
            TerminalId::new(
                u32::try_from(terminal).map_err(|_| PreparationError::InvariantViolation)?,
            ),
        )?;
    }
    Ok(())
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
    let present = match seen.get(index).copied() {
        Some(present) => present,
        None => return Err(PreparationError::InvariantViolation),
    };
    if present == 0 {
        match mark_fact_seen(seen, index) {
            Ok(()) => match prepared_push(facts, Fact { source, terminal }) {
                Ok(()) => {}
                Err(error) => return Err(error),
            },
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn mark_fact_seen(seen: &mut [u8], index: usize) -> Result<(), PreparationError> {
    match seen.get_mut(index) {
        Some(present) => {
            *present = 1;
            Ok(())
        }
        None => Err(PreparationError::InvariantViolation),
    }
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
