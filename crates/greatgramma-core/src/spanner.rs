use crate::{
    DfaStateId, LexerState, PreparationError, SequenceId, SpannerLimits, TerminalId, TokenId,
    ValidatedGrammar,
};

use crate::sequence::{
    SequenceTable, SpannerBudget, ValueRange, count_as_u64, count_as_usize, next_index,
    reserve_exact, source_count, source_row,
};
use crate::token_step::{PreparedTokenExecution, PreparedTokenTable, prepare_token_table};

/// Identifies a finite resource budget used while preparing sequence heads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpannerLimitKind {
    SingletonEdgeScans,
    SingletonZeroEdges,
    SingletonVisited,
    SingletonFacts,
    SingletonPropagationWork,
    SingletonCollectionWork,
    CollectedSingletons,
    Sequences,
    SequenceLength,
    SequencePoolTerminals,
    SequenceInterningWork,
    SequenceAppendWork,
    BucketCrossProduct,
    InverseMembers,
    BucketMembershipWork,
}

/// Identifies a checked spanner-preparation calculation that overflowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpannerArithmeticKind {
    CountConversion,
    Counter,
    SourceCount,
    SingletonFactCells,
    FactIndex,
    ValueRange,
    SequenceLength,
    SequenceId,
    BucketCrossProduct,
    BucketIndex,
    BucketOffset,
    DirectLength,
    AllocationSize,
}

/// Identifies derived spanner storage whose allocation could not be reserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpannerStorage {
    SingletonZeroEdges,
    SingletonVisited,
    SingletonFacts,
    SingletonRows,
    SingletonPool,
    Sequences,
    SequencePool,
    CandidateMembers,
    BucketCounts,
    BucketCursors,
    Buckets,
    InverseMembers,
    DirectLengths,
}

/// A fail-closed sequence-head and inverse-spanner preparation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpannerPreparationError {
    LimitExceeded {
        limit: SpannerLimitKind,
        actual: u64,
        maximum: u64,
    },
    ArithmeticOverflow {
        calculation: SpannerArithmeticKind,
    },
    AllocationFailure {
        storage: SpannerStorage,
        requested: usize,
    },
    TokenPreparation {
        error: PreparationError,
    },
    InvalidDerivedState {
        state: LexerState,
    },
    InvalidDerivedTerminal {
        terminal: TerminalId,
    },
}

/// A checked error for read-only prepared-spanner queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpannerQueryError {
    InvalidState { state: DfaStateId },
    InvalidSequence { sequence: SequenceId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CandidateMember {
    source: usize,
    sequence: SequenceId,
    token: TokenId,
    direct_len: u32,
}

struct BuiltBuckets {
    buckets: Vec<ValueRange>,
    members: Vec<TokenId>,
    direct_lengths: Vec<u32>,
}

/// Opaque, read-only exact sequence heads and inverse token buckets.
///
/// The direct lexer result remains privately owned alongside each bucket. The
/// last terminal of every exposed sequence is continuation evidence, not a
/// terminal committed by selecting an inverse member.
pub struct PreparedSpanner {
    dfa_state_count: u32,
    token_table: PreparedTokenTable,
    sequences: SequenceTable,
    buckets: Vec<ValueRange>,
    members: Vec<TokenId>,
    member_direct_lengths: Vec<u32>,
}

impl PreparedSpanner {
    /// Returns the number of deterministically interned nonempty sequence heads.
    #[must_use]
    pub fn sequence_count(&self) -> usize {
        self.sequences.sequence_count()
    }

    /// Returns one checked interned sequence head by ID.
    #[must_use]
    pub fn sequence(&self, sequence: SequenceId) -> Option<&[TerminalId]> {
        self.sequences.sequence(sequence)
    }

    /// Returns the exact sorted singleton-producible terminals for a source.
    pub fn singleton_terminals(
        &self,
        source: LexerState,
    ) -> Result<&[TerminalId], SpannerQueryError> {
        let row = self.query_source_row(source)?;
        self.sequences
            .singleton(row)
            .ok_or(SpannerQueryError::InvalidState {
                state: invalid_state_for_source(source),
            })
    }

    /// Returns sorted token IDs realizing `sequence` from `source`.
    ///
    /// EOS IDs never appear. An unrealizable checked pair returns an empty
    /// slice; invalid source or sequence IDs return a structured error.
    pub fn inverse_tokens(
        &self,
        source: LexerState,
        sequence: SequenceId,
    ) -> Result<&[TokenId], SpannerQueryError> {
        let row = self.query_source_row(source)?;
        let sequence_index = usize::try_from(sequence.get())
            .ok()
            .filter(|index| *index < self.sequences.sequence_count())
            .ok_or(SpannerQueryError::InvalidSequence { sequence })?;
        let bucket_index = row
            .checked_mul(self.sequences.sequence_count())
            .and_then(|start| start.checked_add(sequence_index))
            .ok_or(SpannerQueryError::InvalidSequence { sequence })?;
        let range = *self
            .buckets
            .get(bucket_index)
            .ok_or(SpannerQueryError::InvalidSequence { sequence })?;
        let end = range
            .start
            .checked_add(range.len)
            .ok_or(SpannerQueryError::InvalidSequence { sequence })?;
        self.member_direct_lengths
            .get(range.start..end)
            .ok_or(SpannerQueryError::InvalidSequence { sequence })?;
        self.members
            .get(range.start..end)
            .ok_or(SpannerQueryError::InvalidSequence { sequence })
    }

    fn query_source_row(&self, source: LexerState) -> Result<usize, SpannerQueryError> {
        match source {
            LexerState::Start if self.token_table.source_count() > 0 => Ok(0),
            LexerState::Start => Err(SpannerQueryError::InvalidState {
                state: DfaStateId::new(u32::MAX),
            }),
            LexerState::Dfa(state) if state.get() < self.dfa_state_count => {
                usize::try_from(state.get())
                    .ok()
                    .and_then(|index| index.checked_add(1))
                    .filter(|row| *row < self.token_table.source_count())
                    .ok_or(SpannerQueryError::InvalidState { state })
            }
            LexerState::Dfa(state) => Err(SpannerQueryError::InvalidState { state }),
        }
    }
}

/// Prepares exact singleton heads and deterministic inverse token buckets.
///
/// Construction accepts only validated base tables. Limits fail closed before
/// any derived relation is truncated or exposed.
pub fn prepare_spanner(
    grammar: &ValidatedGrammar,
    limits: SpannerLimits,
) -> Result<PreparedSpanner, SpannerPreparationError> {
    let token_table = prepare_token_table(grammar, limits.token)
        .map_err(|error| SpannerPreparationError::TokenPreparation { error })?;
    let mut budget = SpannerBudget::new(&limits);
    let mut sequences = SequenceTable::build_singletons(grammar, &mut budget)?;
    let source_count = source_count(grammar)?;
    let mut candidates = Vec::new();

    let mut source = 0;
    while source < source_count {
        let expected_source = token_table.source_for_row(source).ok_or(
            SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::SourceCount,
            },
        )?;
        if source_row(grammar, expected_source) != Some(source) {
            return Err(SpannerPreparationError::InvalidDerivedState {
                state: expected_source,
            });
        }
        let mut token_index = 0;
        while token_index < grammar.token_count() {
            budget.charge(SpannerLimitKind::BucketMembershipWork, 1)?;
            let token = TokenId::new(token_index);
            match token_table.cell(source, token).ok_or(
                SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::BucketIndex,
                },
            )? {
                PreparedTokenExecution::Continue { state, emitted } => {
                    let destination = source_row(grammar, state)
                        .ok_or(SpannerPreparationError::InvalidDerivedState { state })?;
                    let singleton_len = sequences
                        .singleton(destination)
                        .ok_or(SpannerPreparationError::ArithmeticOverflow {
                            calculation: SpannerArithmeticKind::ValueRange,
                        })?
                        .len();
                    let direct_len = u32::try_from(emitted.len()).map_err(|_| {
                        SpannerPreparationError::ArithmeticOverflow {
                            calculation: SpannerArithmeticKind::DirectLength,
                        }
                    })?;
                    let mut singleton_index = 0;
                    while singleton_index < singleton_len {
                        budget.charge(SpannerLimitKind::BucketMembershipWork, 1)?;
                        let terminal = *sequences
                            .singleton(destination)
                            .and_then(|singletons| singletons.get(singleton_index))
                            .ok_or(SpannerPreparationError::ArithmeticOverflow {
                                calculation: SpannerArithmeticKind::ValueRange,
                            })?;
                        let sequence = sequences.intern_head(emitted, terminal, &mut budget)?;
                        budget.charge(SpannerLimitKind::InverseMembers, 1)?;
                        reserve_exact(&mut candidates, 1, SpannerStorage::CandidateMembers)?;
                        candidates.push(CandidateMember {
                            source,
                            sequence,
                            token,
                            direct_len,
                        });
                        singleton_index = next_index(singleton_index)?;
                    }
                }
                PreparedTokenExecution::Rejected | PreparedTokenExecution::Finished { .. } => {}
            }
            token_index =
                token_index
                    .checked_add(1)
                    .ok_or(SpannerPreparationError::ArithmeticOverflow {
                        calculation: SpannerArithmeticKind::CountConversion,
                    })?;
        }
        source = next_index(source)?;
    }

    let sequence_count = sequences.sequence_count();
    let bucket_count = source_count.checked_mul(sequence_count).ok_or(
        SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::BucketCrossProduct,
        },
    )?;
    budget.charge(
        SpannerLimitKind::BucketCrossProduct,
        count_as_u64(bucket_count)?,
    )?;
    let built = build_buckets(bucket_count, sequence_count, &candidates, &mut budget)?;

    Ok(PreparedSpanner {
        dfa_state_count: grammar.lexer().state_count(),
        token_table,
        sequences,
        buckets: built.buckets,
        members: built.members,
        member_direct_lengths: built.direct_lengths,
    })
}

fn build_buckets(
    bucket_count: usize,
    sequence_count: usize,
    candidates: &[CandidateMember],
    budget: &mut SpannerBudget<'_>,
) -> Result<BuiltBuckets, SpannerPreparationError> {
    let mut counts = Vec::new();
    reserve_exact(&mut counts, bucket_count, SpannerStorage::BucketCounts)?;
    counts.resize(bucket_count, 0_usize);
    let mut candidate_index = 0;
    while candidate_index < candidates.len() {
        budget.charge(SpannerLimitKind::BucketMembershipWork, 1)?;
        let candidate = *candidates.get(candidate_index).ok_or(
            SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::BucketIndex,
            },
        )?;
        let bucket = bucket_index(candidate, sequence_count)?;
        let count = counts
            .get_mut(bucket)
            .ok_or(SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::BucketIndex,
            })?;
        *count = count
            .checked_add(1)
            .ok_or(SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::BucketOffset,
            })?;
        candidate_index = next_index(candidate_index)?;
    }

    let mut buckets = Vec::new();
    let mut cursors = Vec::new();
    reserve_exact(&mut buckets, bucket_count, SpannerStorage::Buckets)?;
    reserve_exact(&mut cursors, bucket_count, SpannerStorage::BucketCursors)?;
    let mut offset = 0;
    let mut bucket = 0;
    while bucket < bucket_count {
        let len = *counts
            .get(bucket)
            .ok_or(SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::BucketIndex,
            })?;
        buckets.push(ValueRange { start: offset, len });
        cursors.push(offset);
        offset = offset
            .checked_add(len)
            .ok_or(SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::BucketOffset,
            })?;
        bucket = next_index(bucket)?;
    }
    if offset != candidates.len() {
        return Err(SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::BucketOffset,
        });
    }

    let member_count = count_as_usize(count_as_u64(candidates.len())?)?;
    let mut members = Vec::new();
    let mut direct_lengths = Vec::new();
    reserve_exact(&mut members, member_count, SpannerStorage::InverseMembers)?;
    reserve_exact(
        &mut direct_lengths,
        member_count,
        SpannerStorage::DirectLengths,
    )?;
    members.resize(member_count, TokenId::new(0));
    direct_lengths.resize(member_count, 0);

    candidate_index = 0;
    while candidate_index < candidates.len() {
        budget.charge(SpannerLimitKind::BucketMembershipWork, 1)?;
        let candidate = *candidates.get(candidate_index).ok_or(
            SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::BucketIndex,
            },
        )?;
        let bucket = bucket_index(candidate, sequence_count)?;
        let cursor =
            cursors
                .get_mut(bucket)
                .ok_or(SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::BucketIndex,
                })?;
        let member =
            members
                .get_mut(*cursor)
                .ok_or(SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::BucketOffset,
                })?;
        let direct_len =
            direct_lengths
                .get_mut(*cursor)
                .ok_or(SpannerPreparationError::ArithmeticOverflow {
                    calculation: SpannerArithmeticKind::BucketOffset,
                })?;
        *member = candidate.token;
        *direct_len = candidate.direct_len;
        *cursor = cursor
            .checked_add(1)
            .ok_or(SpannerPreparationError::ArithmeticOverflow {
                calculation: SpannerArithmeticKind::BucketOffset,
            })?;
        candidate_index = next_index(candidate_index)?;
    }
    Ok(BuiltBuckets {
        buckets,
        members,
        direct_lengths,
    })
}

fn bucket_index(
    candidate: CandidateMember,
    sequence_count: usize,
) -> Result<usize, SpannerPreparationError> {
    let sequence = usize::try_from(candidate.sequence.get()).map_err(|_| {
        SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::CountConversion,
        }
    })?;
    candidate
        .source
        .checked_mul(sequence_count)
        .and_then(|start| start.checked_add(sequence))
        .ok_or(SpannerPreparationError::ArithmeticOverflow {
            calculation: SpannerArithmeticKind::BucketIndex,
        })
}

fn invalid_state_for_source(source: LexerState) -> DfaStateId {
    match source {
        LexerState::Start => DfaStateId::new(u32::MAX),
        LexerState::Dfa(state) => state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Action, LalrDimensions, LalrTable, LexerDfa, ParserStateId, TokenEntry, TokenExecution,
        UnvalidatedGrammar, ValidationLimits, execute_token,
    };

    fn fixture() -> ValidatedGrammar {
        let mut classes = vec![0; 256];
        classes[usize::from(b'a')] = 1;
        classes[usize::from(b'b')] = 2;
        let width = 3;
        let mut transitions = vec![None; 3 * width];
        transitions[1] = Some(DfaStateId::new(1));
        transitions[2] = Some(DfaStateId::new(2));
        let lexer = LexerDfa::new(
            3,
            u32::try_from(width).expect("fixture width fits"),
            classes,
            transitions,
            DfaStateId::new(0),
            vec![None, Some(TerminalId::new(0)), Some(TerminalId::new(1))],
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
            vec![
                TokenEntry::Bytes(b"a".to_vec()),
                TokenEntry::Bytes(b"aa".to_vec()),
                TokenEntry::Bytes(b"aa".to_vec()),
                TokenEntry::Eos,
            ],
            lexer,
            lalr,
        )
        .validate(ValidationLimits::default())
        .expect("fixture validates")
    }

    fn insert_terminal(terminals: &mut Vec<TerminalId>, terminal: TerminalId) {
        if !terminals.contains(&terminal) {
            terminals.push(terminal);
            terminals.sort();
        }
    }

    fn exhaustive_singletons(grammar: &ValidatedGrammar, source: LexerState) -> Vec<TerminalId> {
        let mut terminals = Vec::new();
        let mut first = 0_u16;
        while first < 256 {
            let first_byte = u8::try_from(first).expect("raw byte fits");
            if let Ok(crate::LexerStep::Continue { state, emitted }) =
                crate::lexer_step(grammar, source, crate::LexerInput::Byte(first_byte))
            {
                if let Some(terminal) = emitted {
                    insert_terminal(&mut terminals, terminal);
                } else {
                    let mut second = 0_u16;
                    while second < 256 {
                        let second_byte = u8::try_from(second).expect("raw byte fits");
                        if let Ok(crate::LexerStep::Continue {
                            emitted: Some(terminal),
                            ..
                        }) =
                            crate::lexer_step(grammar, state, crate::LexerInput::Byte(second_byte))
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

    fn insert_sequence(sequences: &mut Vec<Vec<TerminalId>>, sequence: Vec<TerminalId>) {
        if !sequences.contains(&sequence) {
            sequences.push(sequence);
            sequences.sort();
        }
    }

    #[test]
    fn buckets_are_flat_sorted_and_keep_speculative_terminal_separate() {
        let grammar = fixture();
        let spanner =
            prepare_spanner(&grammar, SpannerLimits::default()).expect("preparation succeeds");
        assert_eq!(spanner.members.len(), spanner.member_direct_lengths.len());
        let mut bucket_index = 0;
        while bucket_index < spanner.buckets.len() {
            let range = spanner.buckets[bucket_index];
            let values = &spanner.members[range.start..range.start + range.len];
            assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
            bucket_index += 1;
        }

        let head = (0..spanner.sequence_count())
            .map(|index| SequenceId::new(u32::try_from(index).expect("fixture count fits")))
            .find(|id| spanner.sequence(*id) == Some(&[TerminalId::new(0)][..]))
            .expect("singleton head is interned");
        assert_eq!(
            spanner.inverse_tokens(LexerState::Start, head),
            Ok(&[TokenId::new(0)][..])
        );
        assert_eq!(spanner.member_direct_lengths[0], 0);
        assert_eq!(
            execute_token(&grammar, LexerState::Start, TokenId::new(0)),
            Ok(Some(TokenExecution::Continue {
                state: LexerState::Dfa(DfaStateId::new(1)),
                emitted: Vec::new(),
            }))
        );
    }

    #[test]
    fn sequences_and_inverse_buckets_agree_with_direct_exhaustive_definition() {
        let grammar = fixture();
        let spanner =
            prepare_spanner(&grammar, SpannerLimits::default()).expect("preparation succeeds");
        let mut expected_sequences = Vec::new();
        let mut source_row = 0;
        while source_row < spanner.token_table.source_count() {
            let source = spanner
                .token_table
                .source_for_row(source_row)
                .expect("fixture row exists");
            let mut token_index = 0;
            while token_index < grammar.token_count() {
                let token = TokenId::new(token_index);
                if let Some(TokenExecution::Continue { state, emitted }) =
                    execute_token(&grammar, source, token).expect("derived input is valid")
                {
                    for terminal in exhaustive_singletons(&grammar, state) {
                        let mut head = emitted.clone();
                        head.push(terminal);
                        insert_sequence(&mut expected_sequences, head);
                    }
                }
                token_index += 1;
            }
            source_row += 1;
        }
        let mut actual_sequences = Vec::new();
        let mut sequence_index = 0;
        while sequence_index < spanner.sequence_count() {
            let sequence = SequenceId::new(
                u32::try_from(sequence_index).expect("fixture sequence count fits"),
            );
            actual_sequences.push(
                spanner
                    .sequence(sequence)
                    .expect("sequence ID exists")
                    .to_vec(),
            );
            sequence_index += 1;
        }
        actual_sequences.sort();
        assert_eq!(actual_sequences, expected_sequences);

        source_row = 0;
        while source_row < spanner.token_table.source_count() {
            let source = spanner
                .token_table
                .source_for_row(source_row)
                .expect("fixture row exists");
            sequence_index = 0;
            while sequence_index < spanner.sequence_count() {
                let sequence = SequenceId::new(
                    u32::try_from(sequence_index).expect("fixture sequence count fits"),
                );
                let head = spanner.sequence(sequence).expect("sequence exists");
                let (last, direct) = head.split_last().expect("heads are nonempty");
                let mut expected_tokens = Vec::new();
                let mut token_index = 0;
                while token_index < grammar.token_count() {
                    let token = TokenId::new(token_index);
                    match execute_token(&grammar, source, token).expect("derived input is valid") {
                        Some(TokenExecution::Continue { state, emitted })
                            if emitted == direct
                                && exhaustive_singletons(&grammar, state).contains(last) =>
                        {
                            expected_tokens.push(token);
                        }
                        _ => {}
                    }
                    token_index += 1;
                }
                assert_eq!(
                    spanner.inverse_tokens(source, sequence),
                    Ok(expected_tokens.as_slice()),
                    "source row {source_row}, sequence {sequence_index}",
                );
                sequence_index += 1;
            }
            source_row += 1;
        }
    }

    #[test]
    fn every_member_records_only_the_direct_prefix_as_committed() {
        let grammar = fixture();
        let spanner =
            prepare_spanner(&grammar, SpannerLimits::default()).expect("preparation succeeds");
        let sequence_count = spanner.sequence_count();
        let mut bucket_index = 0;
        while bucket_index < spanner.buckets.len() {
            let source_row = bucket_index / sequence_count;
            let sequence_index = bucket_index % sequence_count;
            let source = spanner
                .token_table
                .source_for_row(source_row)
                .expect("source row exists");
            let sequence = SequenceId::new(
                u32::try_from(sequence_index).expect("fixture sequence count fits"),
            );
            let head = spanner.sequence(sequence).expect("sequence exists");
            let (last, direct) = head.split_last().expect("head is nonempty");
            let range = spanner.buckets[bucket_index];
            let mut member = range.start;
            while member < range.start + range.len {
                let token = spanner.members[member];
                let execution = execute_token(&grammar, source, token)
                    .expect("derived input is valid")
                    .expect("inverse member is accepted");
                let TokenExecution::Continue { state, emitted } = execution else {
                    panic!("EOS cannot be an ordinary inverse member");
                };
                assert_eq!(emitted, direct);
                assert_eq!(
                    usize::try_from(spanner.member_direct_lengths[member])
                        .expect("direct length fits"),
                    emitted.len(),
                );
                assert!(exhaustive_singletons(&grammar, state).contains(last));
                assert_eq!(head.len(), emitted.len() + 1);
                member += 1;
            }
            bucket_index += 1;
        }
    }

    #[test]
    fn limits_cover_sequence_and_bucket_growth_without_truncation() {
        let grammar = fixture();
        let limits = SpannerLimits {
            max_sequence_count: 0,
            ..SpannerLimits::default()
        };
        assert_eq!(
            prepare_spanner(&grammar, limits).err(),
            Some(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::Sequences,
                actual: 1,
                maximum: 0,
            })
        );
        let limits = SpannerLimits {
            max_sequence_length: 0,
            ..SpannerLimits::default()
        };
        assert_eq!(
            prepare_spanner(&grammar, limits).err(),
            Some(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::SequenceLength,
                actual: 1,
                maximum: 0,
            })
        );
        let limits = SpannerLimits {
            max_sequence_pool_terminals: 0,
            ..SpannerLimits::default()
        };
        assert_eq!(
            prepare_spanner(&grammar, limits).err(),
            Some(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::SequencePoolTerminals,
                actual: 1,
                maximum: 0,
            })
        );
        let limits = SpannerLimits {
            max_inverse_members: 0,
            ..SpannerLimits::default()
        };
        assert_eq!(
            prepare_spanner(&grammar, limits).err(),
            Some(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::InverseMembers,
                actual: 1,
                maximum: 0,
            })
        );
        let limits = SpannerLimits {
            max_bucket_membership_work: 0,
            ..SpannerLimits::default()
        };
        assert_eq!(
            prepare_spanner(&grammar, limits).err(),
            Some(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::BucketMembershipWork,
                actual: 1,
                maximum: 0,
            })
        );
        let limits = SpannerLimits {
            max_bucket_cross_product: 0,
            ..SpannerLimits::default()
        };
        assert!(matches!(
            prepare_spanner(&grammar, limits),
            Err(SpannerPreparationError::LimitExceeded {
                limit: SpannerLimitKind::BucketCrossProduct,
                actual,
                maximum: 0,
            }) if actual > 0
        ));
    }
}
