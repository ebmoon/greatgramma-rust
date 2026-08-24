use crate::{
    DfaStateId, LexerError, LexerInput, LexerState, LexerStep, SequenceId, SpannerLimits,
    TerminalId, ValidatedGrammar, lexer_step,
};

use crate::spanner::{
    SpannerArithmeticKind, SpannerLimitKind, SpannerPreparationError, SpannerStorage,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ValueRange {
    pub(crate) start: usize,
    pub(crate) len: usize,
}

#[derive(Clone, Copy)]
struct ZeroEdge {
    source: usize,
    destination: usize,
}

#[derive(Clone, Copy)]
struct SingletonFact {
    source: usize,
    terminal: usize,
}

/// Exact singleton-terminal rows and deterministically interned sequence heads.
pub(crate) struct SequenceTable {
    singleton_rows: Vec<ValueRange>,
    singleton_pool: Vec<TerminalId>,
    sequences: Vec<ValueRange>,
    sequence_pool: Vec<TerminalId>,
}

pub(crate) struct SpannerBudget<'a> {
    limits: &'a SpannerLimits,
    singleton_edge_scans: u64,
    singleton_zero_edges: u64,
    singleton_visited: u64,
    singleton_facts: u64,
    singleton_propagation_work: u64,
    singleton_collection_work: u64,
    collected_singletons: u64,
    sequence_count: u64,
    sequence_pool_terminals: u64,
    sequence_interning_work: u64,
    sequence_append_work: u64,
    bucket_cross_product: u64,
    inverse_members: u64,
    bucket_membership_work: u64,
}

impl<'a> SpannerBudget<'a> {
    pub(crate) fn new(limits: &'a SpannerLimits) -> Self {
        Self {
            limits,
            singleton_edge_scans: 0,
            singleton_zero_edges: 0,
            singleton_visited: 0,
            singleton_facts: 0,
            singleton_propagation_work: 0,
            singleton_collection_work: 0,
            collected_singletons: 0,
            sequence_count: 0,
            sequence_pool_terminals: 0,
            sequence_interning_work: 0,
            sequence_append_work: 0,
            bucket_cross_product: 0,
            inverse_members: 0,
            bucket_membership_work: 0,
        }
    }

    pub(crate) fn charge(
        &mut self,
        kind: SpannerLimitKind,
        amount: u64,
    ) -> Result<(), SpannerPreparationError> {
        let (current, maximum) = self.counter_and_limit(kind);
        let actual =
            current
                .checked_add(amount)
                .ok_or(SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::Counter,
                })?;
        if actual > maximum {
            return Err(SpannerPreparationError::LimitExceeded {
                limit: kind,
                actual,
                maximum,
            });
        }
        self.set_counter(kind, actual);
        Ok(())
    }

    pub(crate) fn check_sequence_length(&self, actual: u64) -> Result<(), SpannerPreparationError> {
        if actual > self.limits.max_sequence_length {
            return Err(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::SequenceLength,
                actual,
                maximum: self.limits.max_sequence_length,
            });
        }
        Ok(())
    }

    fn counter_and_limit(&self, kind: SpannerLimitKind) -> (u64, u64) {
        match kind {
            SpannerLimitKind::SingletonEdgeScans => (
                self.singleton_edge_scans,
                self.limits.max_singleton_edge_scans,
            ),
            SpannerLimitKind::SingletonZeroEdges => (
                self.singleton_zero_edges,
                self.limits.max_singleton_zero_edges,
            ),
            SpannerLimitKind::SingletonVisited => {
                (self.singleton_visited, self.limits.max_singleton_visited)
            }
            SpannerLimitKind::SingletonFacts => {
                (self.singleton_facts, self.limits.max_singleton_facts)
            }
            SpannerLimitKind::SingletonPropagationWork => (
                self.singleton_propagation_work,
                self.limits.max_singleton_propagation_work,
            ),
            SpannerLimitKind::SingletonCollectionWork => (
                self.singleton_collection_work,
                self.limits.max_singleton_collection_work,
            ),
            SpannerLimitKind::CollectedSingletons => (
                self.collected_singletons,
                self.limits.max_collected_singletons,
            ),
            SpannerLimitKind::Sequences => (self.sequence_count, self.limits.max_sequence_count),
            SpannerLimitKind::SequencePoolTerminals => (
                self.sequence_pool_terminals,
                self.limits.max_sequence_pool_terminals,
            ),
            SpannerLimitKind::SequenceInterningWork => (
                self.sequence_interning_work,
                self.limits.max_sequence_interning_work,
            ),
            SpannerLimitKind::SequenceAppendWork => (
                self.sequence_append_work,
                self.limits.max_sequence_append_work,
            ),
            SpannerLimitKind::BucketCrossProduct => (
                self.bucket_cross_product,
                self.limits.max_bucket_cross_product,
            ),
            SpannerLimitKind::InverseMembers => {
                (self.inverse_members, self.limits.max_inverse_members)
            }
            SpannerLimitKind::BucketMembershipWork => (
                self.bucket_membership_work,
                self.limits.max_bucket_membership_work,
            ),
            SpannerLimitKind::SequenceLength => (0, self.limits.max_sequence_length),
        }
    }

    fn set_counter(&mut self, kind: SpannerLimitKind, value: u64) {
        match kind {
            SpannerLimitKind::SingletonEdgeScans => self.singleton_edge_scans = value,
            SpannerLimitKind::SingletonZeroEdges => self.singleton_zero_edges = value,
            SpannerLimitKind::SingletonVisited => self.singleton_visited = value,
            SpannerLimitKind::SingletonFacts => self.singleton_facts = value,
            SpannerLimitKind::SingletonPropagationWork => {
                self.singleton_propagation_work = value;
            }
            SpannerLimitKind::SingletonCollectionWork => {
                self.singleton_collection_work = value;
            }
            SpannerLimitKind::CollectedSingletons => self.collected_singletons = value,
            SpannerLimitKind::Sequences => self.sequence_count = value,
            SpannerLimitKind::SequencePoolTerminals => self.sequence_pool_terminals = value,
            SpannerLimitKind::SequenceInterningWork => self.sequence_interning_work = value,
            SpannerLimitKind::SequenceAppendWork => self.sequence_append_work = value,
            SpannerLimitKind::BucketCrossProduct => self.bucket_cross_product = value,
            SpannerLimitKind::InverseMembers => self.inverse_members = value,
            SpannerLimitKind::BucketMembershipWork => self.bucket_membership_work = value,
            SpannerLimitKind::SequenceLength => {}
        }
    }
}

impl SequenceTable {
    pub(crate) fn build_singletons(
        grammar: &ValidatedGrammar,
        budget: &mut SpannerBudget<'_>,
    ) -> Result<Self, SpannerPreparationError> {
        let source_count = source_count(grammar)?;
        let terminal_count = count_as_usize(u64::from(grammar.lalr().terminal_count()))?;
        let fact_cells = source_count.checked_mul(terminal_count).ok_or(
            SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::SingletonFactCells,
            },
        )?;
        budget.charge(
            SpannerLimitKind::SingletonVisited,
            count_as_u64(fact_cells)?,
        )?;

        let mut fact_seen = Vec::new();
        reserve_exact(&mut fact_seen, fact_cells, SpannerStorage::SingletonVisited)?;
        fact_seen.resize(fact_cells, false);
        let mut facts = Vec::new();
        let mut zero_edges = Vec::new();

        let mut source = 0;
        while source < source_count {
            let lexer_source = row_source(source)?;
            let mut byte = 0_u16;
            while byte < 256 {
                budget.charge(SpannerLimitKind::SingletonEdgeScans, 1)?;
                let input = u8::try_from(byte).map_err(|_| {
                    SpannerPreparationError::ArithmeticOverflow {
                        calculation: SpannerArithmeticKind::CountConversion,
                    }
                })?;
                match lexer_step(grammar, lexer_source, LexerInput::Byte(input)) {
                    Ok(LexerStep::Continue {
                        state: _,
                        emitted: Some(terminal),
                    }) => add_fact(
                        &mut fact_seen,
                        &mut facts,
                        source,
                        terminal,
                        terminal_count,
                        budget,
                    )?,
                    Ok(LexerStep::Continue {
                        state: destination,
                        emitted: None,
                    }) => {
                        let destination = source_row(grammar, destination).ok_or(
                            SpannerPreparationError::InvalidDerivedState { state: destination },
                        )?;
                        budget.charge(SpannerLimitKind::SingletonZeroEdges, 1)?;
                        reserve_exact(&mut zero_edges, 1, SpannerStorage::SingletonZeroEdges)?;
                        zero_edges.push(ZeroEdge {
                            source,
                            destination,
                        });
                    }
                    Ok(LexerStep::Finished { .. }) => {}
                    Err(LexerError::InvalidState { state }) => {
                        return Err(SpannerPreparationError::InvalidDerivedState {
                            state: LexerState::Dfa(state),
                        });
                    }
                    Err(
                        LexerError::CannotBeginLexeme { .. }
                        | LexerError::ByteAfterUnfinishedResidual { .. }
                        | LexerError::EosAfterUnfinishedResidual { .. },
                    ) => {}
                }
                byte = byte
                    .checked_add(1)
                    .ok_or(SpannerPreparationError::ArithmeticOverflow {
                        calculation: SpannerArithmeticKind::Counter,
                    })?;
            }
            source = next_index(source)?;
        }

        let mut cursor = 0;
        while cursor < facts.len() {
            let fact = *facts
                .get(cursor)
                .ok_or(SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::FactIndex,
                })?;
            let mut edge_index = 0;
            while edge_index < zero_edges.len() {
                budget.charge(SpannerLimitKind::SingletonPropagationWork, 1)?;
                let edge = *zero_edges.get(edge_index).ok_or(
                    SpannerPreparationError::ArithmeticOverflow {
                        calculation: SpannerArithmeticKind::FactIndex,
                    },
                )?;
                if edge.destination == fact.source {
                    let terminal = TerminalId::new(u32::try_from(fact.terminal).map_err(|_| {
                        SpannerPreparationError::ArithmeticOverflow {
                            calculation: SpannerArithmeticKind::CountConversion,
                        }
                    })?);
                    add_fact(
                        &mut fact_seen,
                        &mut facts,
                        edge.source,
                        terminal,
                        terminal_count,
                        budget,
                    )?;
                }
                edge_index = next_index(edge_index)?;
            }
            cursor = next_index(cursor)?;
        }

        let mut table = Self {
            singleton_rows: Vec::new(),
            singleton_pool: Vec::new(),
            sequences: Vec::new(),
            sequence_pool: Vec::new(),
        };
        reserve_exact(
            &mut table.singleton_rows,
            source_count,
            SpannerStorage::SingletonRows,
        )?;
        source = 0;
        while source < source_count {
            let start = table.singleton_pool.len();
            let mut terminal = 0;
            while terminal < terminal_count {
                budget.charge(SpannerLimitKind::SingletonCollectionWork, 1)?;
                let fact_index = fact_index(source, terminal, terminal_count)?;
                if *fact_seen.get(fact_index).ok_or(
                    SpannerPreparationError::ArithmeticOverflow {
                        calculation: SpannerArithmeticKind::FactIndex,
                    },
                )? {
                    budget.charge(SpannerLimitKind::CollectedSingletons, 1)?;
                    reserve_exact(&mut table.singleton_pool, 1, SpannerStorage::SingletonPool)?;
                    table
                        .singleton_pool
                        .push(TerminalId::new(u32::try_from(terminal).map_err(|_| {
                            SpannerPreparationError::ArithmeticOverflow {
                                calculation: SpannerArithmeticKind::CountConversion,
                            }
                        })?));
                }
                terminal = next_index(terminal)?;
            }
            table.singleton_rows.push(ValueRange {
                start,
                len: table.singleton_pool.len().checked_sub(start).ok_or(
                    SpannerPreparationError::ArithmeticOverflow {
                        calculation: SpannerArithmeticKind::ValueRange,
                    },
                )?,
            });
            source = next_index(source)?;
        }
        Ok(table)
    }

    pub(crate) fn singleton(&self, source: usize) -> Option<&[TerminalId]> {
        value(
            self.singleton_rows.get(source).copied()?,
            &self.singleton_pool,
        )
    }

    pub(crate) fn sequence_count(&self) -> usize {
        self.sequences.len()
    }

    pub(crate) fn sequence(&self, id: SequenceId) -> Option<&[TerminalId]> {
        let index = usize::try_from(id.get()).ok()?;
        value(self.sequences.get(index).copied()?, &self.sequence_pool)
    }

    pub(crate) fn intern_head(
        &mut self,
        direct: &[TerminalId],
        terminal: TerminalId,
        budget: &mut SpannerBudget<'_>,
    ) -> Result<SequenceId, SpannerPreparationError> {
        let len =
            direct
                .len()
                .checked_add(1)
                .ok_or(SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::SequenceLength,
                })?;
        let len_u64 = count_as_u64(len)?;
        budget.check_sequence_length(len_u64)?;
        let mut sequence_index = 0;
        while sequence_index < self.sequences.len() {
            budget.charge(SpannerLimitKind::SequenceInterningWork, 1)?;
            let range = *self.sequences.get(sequence_index).ok_or(
                SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::ValueRange,
                },
            )?;
            if range.len == len
                && head_equals(range, &self.sequence_pool, direct, terminal, budget)?
            {
                return sequence_id(sequence_index);
            }
            sequence_index = next_index(sequence_index)?;
        }

        budget.charge(SpannerLimitKind::Sequences, 1)?;
        budget.charge(SpannerLimitKind::SequencePoolTerminals, len_u64)?;
        budget.charge(SpannerLimitKind::SequenceAppendWork, len_u64)?;
        let id = sequence_id(self.sequences.len())?;
        reserve_exact(&mut self.sequences, 1, SpannerStorage::Sequences)?;
        reserve_exact(&mut self.sequence_pool, len, SpannerStorage::SequencePool)?;
        let start = self.sequence_pool.len();
        let mut direct_index = 0;
        while direct_index < direct.len() {
            self.sequence_pool.push(*direct.get(direct_index).ok_or(
                SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::SequenceLength,
                },
            )?);
            direct_index = next_index(direct_index)?;
        }
        self.sequence_pool.push(terminal);
        self.sequences.push(ValueRange { start, len });
        Ok(id)
    }
}

fn add_fact(
    seen: &mut [bool],
    facts: &mut Vec<SingletonFact>,
    source: usize,
    terminal: TerminalId,
    terminal_count: usize,
    budget: &mut SpannerBudget<'_>,
) -> Result<(), SpannerPreparationError> {
    let terminal_index = usize::try_from(terminal.get()).map_err(|_| {
        SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::CountConversion,
        }
    })?;
    if terminal_index >= terminal_count {
        return Err(SpannerPreparationError::InvalidDerivedTerminal { terminal });
    }
    let index = fact_index(source, terminal_index, terminal_count)?;
    let present = *seen
        .get(index)
        .ok_or(SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::FactIndex,
        })?;
    if present {
        return Ok(());
    }
    budget.charge(SpannerLimitKind::SingletonFacts, 1)?;
    reserve_exact(facts, 1, SpannerStorage::SingletonFacts)?;
    *seen
        .get_mut(index)
        .ok_or(SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::FactIndex,
        })? = true;
    facts.push(SingletonFact {
        source,
        terminal: terminal_index,
    });
    Ok(())
}

fn head_equals(
    range: ValueRange,
    pool: &[TerminalId],
    direct: &[TerminalId],
    terminal: TerminalId,
    budget: &mut SpannerBudget<'_>,
) -> Result<bool, SpannerPreparationError> {
    let mut index = 0;
    while index < direct.len() {
        budget.charge(SpannerLimitKind::SequenceInterningWork, 1)?;
        let pool_index =
            range
                .start
                .checked_add(index)
                .ok_or(SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::ValueRange,
                })?;
        if pool.get(pool_index) != direct.get(index) {
            return Ok(false);
        }
        index = next_index(index)?;
    }
    budget.charge(SpannerLimitKind::SequenceInterningWork, 1)?;
    let last = range.start.checked_add(direct.len()).ok_or(
        SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::ValueRange,
        },
    )?;
    Ok(pool.get(last) == Some(&terminal))
}

pub(crate) fn source_count(grammar: &ValidatedGrammar) -> Result<usize, SpannerPreparationError> {
    count_as_usize(
        u64::from(grammar.lexer().state_count())
            .checked_add(1)
            .ok_or(SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::SourceCount,
            })?,
    )
}

pub(crate) fn source_row(grammar: &ValidatedGrammar, source: LexerState) -> Option<usize> {
    match source {
        LexerState::Start => Some(0),
        LexerState::Dfa(state) if state.get() < grammar.lexer().state_count() => {
            usize::try_from(state.get()).ok()?.checked_add(1)
        }
        LexerState::Dfa(_) => None,
    }
}

fn row_source(row: usize) -> Result<LexerState, SpannerPreparationError> {
    if row == 0 {
        return Ok(LexerState::Start);
    }
    let state = row
        .checked_sub(1)
        .ok_or(SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::SourceCount,
        })?;
    Ok(LexerState::Dfa(DfaStateId::new(
        u32::try_from(state).map_err(|_| SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::CountConversion,
        })?,
    )))
}

fn fact_index(
    source: usize,
    terminal: usize,
    terminal_count: usize,
) -> Result<usize, SpannerPreparationError> {
    source
        .checked_mul(terminal_count)
        .and_then(|start| start.checked_add(terminal))
        .ok_or(SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::FactIndex,
        })
}

fn value(range: ValueRange, pool: &[TerminalId]) -> Option<&[TerminalId]> {
    pool.get(range.start..range.start.checked_add(range.len)?)
}

pub(crate) fn reserve_exact<T>(
    values: &mut Vec<T>,
    additional: usize,
    storage: SpannerStorage,
) -> Result<(), SpannerPreparationError> {
    let requested = values.len().checked_add(additional).ok_or(
        SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::AllocationSize,
        },
    )?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| SpannerPreparationError::AllocationFailure { storage, requested })
}

pub(crate) fn count_as_usize(value: u64) -> Result<usize, SpannerPreparationError> {
    usize::try_from(value).map_err(|_| SpannerPreparationError::ArithmeticOverflow {
        calculation: SpannerArithmeticKind::CountConversion,
    })
}

pub(crate) fn count_as_u64(value: usize) -> Result<u64, SpannerPreparationError> {
    u64::try_from(value).map_err(|_| SpannerPreparationError::ArithmeticOverflow {
        calculation: SpannerArithmeticKind::CountConversion,
    })
}

pub(crate) fn next_index(index: usize) -> Result<usize, SpannerPreparationError> {
    index
        .checked_add(1)
        .ok_or(SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::CountConversion,
        })
}

fn sequence_id(index: usize) -> Result<SequenceId, SpannerPreparationError> {
    Ok(SequenceId::new(u32::try_from(index).map_err(|_| {
        SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::SequenceId,
        }
    })?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Action, LalrDimensions, LalrTable, LexerDfa, ParserStateId, TokenEntry, UnvalidatedGrammar,
        ValidationLimits,
    };

    fn cyclic_fixture() -> ValidatedGrammar {
        let mut classes = vec![0; 256];
        classes[usize::from(b'a')] = 1;
        classes[usize::from(b'x')] = 2;
        classes[usize::from(b'y')] = 3;
        let width = 4;
        let mut transitions = vec![None; 4 * width];
        transitions[1] = Some(DfaStateId::new(1));
        transitions[2] = Some(DfaStateId::new(2));
        transitions[2 * width + 2] = Some(DfaStateId::new(2));
        transitions[2 * width + 3] = Some(DfaStateId::new(1));
        let lexer = LexerDfa::new(
            4,
            u32::try_from(width).expect("fixture width fits"),
            classes,
            transitions,
            DfaStateId::new(0),
            vec![
                None,
                Some(TerminalId::new(0)),
                None,
                Some(TerminalId::new(1)),
            ],
        );
        let lalr = LalrTable::new(
            LalrDimensions::new(1, 3, 0),
            ParserStateId::new(0),
            TerminalId::new(2),
            vec![Action::Error, Action::Error, Action::Accept],
            Vec::new(),
            Vec::new(),
        );
        UnvalidatedGrammar::new(
            vec![TokenEntry::Bytes(b"a".to_vec()), TokenEntry::Eos],
            lexer,
            lalr,
        )
        .validate(ValidationLimits::default())
        .expect("fixture validates")
    }

    fn insert_terminal(terminals: &mut Vec<TerminalId>, terminal: TerminalId) {
        if terminals.contains(&terminal) {
            return;
        }
        terminals.push(terminal);
        terminals.sort();
    }

    fn exhaustive_raw_bytes(grammar: &ValidatedGrammar, source: LexerState) -> Vec<TerminalId> {
        let mut terminals = Vec::new();
        let mut first = 0_u16;
        while first < 256 {
            let first_byte = u8::try_from(first).expect("raw byte fits");
            if let Ok(LexerStep::Continue { state, emitted }) =
                lexer_step(grammar, source, LexerInput::Byte(first_byte))
            {
                if let Some(terminal) = emitted {
                    insert_terminal(&mut terminals, terminal);
                } else {
                    let mut second = 0_u16;
                    while second < 256 {
                        let second_byte = u8::try_from(second).expect("raw byte fits");
                        if let Ok(LexerStep::Continue {
                            emitted: Some(terminal),
                            ..
                        }) = lexer_step(grammar, state, LexerInput::Byte(second_byte))
                        {
                            insert_terminal(&mut terminals, terminal);
                        }
                        second += 1;
                    }
                }
            }
            first += 1;
        }
        terminals
    }

    #[test]
    fn singleton_fixed_point_crosses_zero_cycles_and_omits_unreachable_labels() {
        let grammar = cyclic_fixture();
        let limits = SpannerLimits::default();
        let mut budget = SpannerBudget::new(&limits);
        let table =
            SequenceTable::build_singletons(&grammar, &mut budget).expect("fixed point terminates");
        assert_eq!(table.singleton(0), Some(&[TerminalId::new(0)][..]));
        assert_eq!(table.singleton(3), Some(&[TerminalId::new(0)][..]));
        assert_eq!(table.singleton(4), Some(&[TerminalId::new(1)][..]));
    }

    #[test]
    fn singleton_fixed_point_agrees_with_bounded_exhaustive_raw_byte_words() {
        let grammar = cyclic_fixture();
        let limits = SpannerLimits::default();
        let mut budget = SpannerBudget::new(&limits);
        let table =
            SequenceTable::build_singletons(&grammar, &mut budget).expect("fixed point terminates");
        let mut row = 0;
        while row < source_count(&grammar).expect("fixture count fits") {
            let source = row_source(row).expect("fixture row fits");
            assert_eq!(
                table.singleton(row),
                Some(exhaustive_raw_bytes(&grammar, source).as_slice()),
                "source row {row}",
            );
            row += 1;
        }
    }

    #[test]
    fn singleton_limits_fail_before_fact_matrix_or_worklist_growth() {
        let grammar = cyclic_fixture();
        let limits = SpannerLimits {
            max_singleton_visited: 1,
            ..SpannerLimits::default()
        };
        let mut budget = SpannerBudget::new(&limits);
        assert_eq!(
            SequenceTable::build_singletons(&grammar, &mut budget).err(),
            Some(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::SingletonVisited,
                actual: 15,
                maximum: 1,
            })
        );

        let limits = SpannerLimits {
            max_singleton_edge_scans: 0,
            ..SpannerLimits::default()
        };
        let mut budget = SpannerBudget::new(&limits);
        assert_eq!(
            SequenceTable::build_singletons(&grammar, &mut budget).err(),
            Some(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::SingletonEdgeScans,
                actual: 1,
                maximum: 0,
            })
        );

        let limits = SpannerLimits {
            max_singleton_propagation_work: 0,
            ..SpannerLimits::default()
        };
        let mut budget = SpannerBudget::new(&limits);
        assert_eq!(
            SequenceTable::build_singletons(&grammar, &mut budget).err(),
            Some(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::SingletonPropagationWork,
                actual: 1,
                maximum: 0,
            })
        );

        let limits = SpannerLimits {
            max_collected_singletons: 0,
            ..SpannerLimits::default()
        };
        let mut budget = SpannerBudget::new(&limits);
        assert_eq!(
            SequenceTable::build_singletons(&grammar, &mut budget).err(),
            Some(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::CollectedSingletons,
                actual: 1,
                maximum: 0,
            })
        );
    }
}
