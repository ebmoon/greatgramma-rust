use crate::{
    LexerState, ParserError, ParserStateId, PreparationError, PreparationLimits, PreparedParser,
    PreparedSpanner, SequenceClassification, SequenceId, SpannerQueryError, TokenExecution,
    TokenId, ValidatedGrammar,
};

use crate::lalr::{
    RunDecision, execute_terminals_concrete_in_place, execute_terminals_concrete_into,
};
use crate::lalr_reference::sequence_head_allowed_with_scratch;
use crate::mask;
use crate::preprocess::prepare_parser_with_budget;
use crate::spanner::{InverseBucket, prepare_spanner_with_budget};
use crate::token_step::{WorkBudget, reserved_vec};

/// One immutable, fully prepared normalized grammar and all of its derived tables.
pub struct PreparedGrammar {
    grammar: ValidatedGrammar,
    spanner: PreparedSpanner,
    parser: PreparedParser,
    mask_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MatcherPhase {
    Running { lexer: LexerState },
    Accepted,
}

#[derive(Debug, Eq, PartialEq)]
struct MatcherStatus {
    phase: MatcherPhase,
    parser_stack: Vec<ParserStateId>,
}

impl MatcherStatus {
    const fn empty() -> Self {
        Self {
            phase: MatcherPhase::Accepted,
            parser_stack: Vec::new(),
        }
    }

    fn set_accepted(&mut self) {
        self.phase = MatcherPhase::Accepted;
        self.parser_stack.clear();
    }
}

/// One owning matcher whose private row states cannot be mixed with another grammar.
pub struct Matcher {
    prepared: PreparedGrammar,
    states: Vec<MatcherStatus>,
    staging: Vec<MatcherStatus>,
    scratch: Vec<ParserStateId>,
}

/// Successful atomic token advancement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvanceResult {
    Continue,
    Accepted,
}

/// Fail-closed matcher preparation, query, and transition errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineError {
    InvalidToken { token: TokenId },
    InvalidRow { row: usize, rows: usize },
    EmptyBatch,
    BatchSizeMismatch { expected: usize, actual: usize },
    MissingRunningToken { row: usize },
    Completed,
    ConstraintViolation { token: TokenId },
    BatchConstraintViolation { row: usize, token: TokenId },
    BufferTooSmall { required: usize, actual: usize },
    NoValidToken,
    AllocationFailure { requested: usize },
    CorruptPreparedData,
}

enum TokenDecision {
    Rejected,
    Continue { lexer: LexerState },
    Accepted,
}

/// Consumes validated grammar tables and builds the single opaque runtime owner.
pub fn prepare(
    grammar: ValidatedGrammar,
    limits: PreparationLimits,
) -> Result<PreparedGrammar, PreparationError> {
    let mut work = WorkBudget::new(limits);
    let spanner = prepare_spanner_with_budget(&grammar, limits, &mut work)?;
    let parser = prepare_parser_with_budget(&grammar, &spanner, limits, &mut work)?;
    let token_count =
        usize::try_from(grammar.token_count()).map_err(|_| PreparationError::InvariantViolation)?;
    let mask_bytes = token_count
        .checked_add(7)
        .map(|rounded| rounded / 8)
        .ok_or(PreparationError::InvariantViolation)?;
    Ok(PreparedGrammar {
        grammar,
        spanner,
        parser,
        mask_bytes,
    })
}

impl PreparedGrammar {
    #[must_use]
    pub const fn token_count(&self) -> u32 {
        self.grammar.token_count()
    }

    #[must_use]
    pub const fn mask_bytes(&self) -> usize {
        self.mask_bytes
    }

    pub fn into_matcher(self, rows: usize) -> Result<Matcher, EngineError> {
        if rows == 0 {
            return Err(EngineError::EmptyBatch);
        }
        let mut states =
            reserved_vec(rows).map_err(|requested| EngineError::AllocationFailure { requested })?;
        let mut staging =
            reserved_vec(rows).map_err(|requested| EngineError::AllocationFailure { requested })?;
        for _ in 0..rows {
            states.push(self.initial_status()?);
            staging.push(MatcherStatus::empty());
        }
        Ok(Matcher {
            prepared: self,
            states,
            staging,
            scratch: Vec::new(),
        })
    }

    fn initial_status(&self) -> Result<MatcherStatus, EngineError> {
        let mut parser_stack =
            reserved_vec(1).map_err(|requested| EngineError::AllocationFailure { requested })?;
        parser_stack.push(self.grammar.lalr().start_state());
        Ok(MatcherStatus {
            phase: MatcherPhase::Running {
                lexer: LexerState::Start,
            },
            parser_stack,
        })
    }

    fn allows_status(&self, state: &MatcherStatus, token: TokenId) -> Result<bool, EngineError> {
        let lexer = running_lexer(state)?;
        let parser_stack = &state.parser_stack;
        self.check_token(token)?;
        let mut destination = Vec::new();
        Ok(!matches!(
            self.evaluate_token_into(lexer, parser_stack, token, &mut destination)?,
            TokenDecision::Rejected
        ))
    }

    fn next_status_into(
        &self,
        state: &MatcherStatus,
        token: TokenId,
        destination: &mut MatcherStatus,
    ) -> Result<AdvanceResult, EngineError> {
        let lexer = running_lexer(state)?;
        let parser_stack = &state.parser_stack;
        self.check_token(token)?;
        match self.evaluate_token_into(lexer, parser_stack, token, &mut destination.parser_stack)? {
            TokenDecision::Rejected => Err(EngineError::ConstraintViolation { token }),
            TokenDecision::Continue { lexer } => {
                destination.phase = MatcherPhase::Running { lexer };
                Ok(AdvanceResult::Continue)
            }
            TokenDecision::Accepted => {
                destination.set_accepted();
                Ok(AdvanceResult::Accepted)
            }
        }
    }

    fn fill_mask_with_scratch(
        &self,
        state: &MatcherStatus,
        output: &mut [u8],
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<(), EngineError> {
        let lexer = running_lexer(state)?;
        let parser_stack = &state.parser_stack;
        if output.len() < self.mask_bytes {
            return Err(EngineError::BufferTooSmall {
                required: self.mask_bytes,
                actual: output.len(),
            });
        }

        let buckets = self
            .spanner
            .inverse_buckets(lexer)
            .map_err(map_spanner_error)?;
        let ordinary_allowed = self.fill_ordinary_mask(parser_stack, buckets, output, scratch)?;
        let eos_allowed = self.fill_eos_mask(lexer, parser_stack, output, scratch)?;

        if !ordinary_allowed && !eos_allowed {
            return Err(EngineError::NoValidToken);
        }
        Ok(())
    }

    fn fill_ordinary_mask(
        &self,
        parser_stack: &[ParserStateId],
        buckets: &[InverseBucket],
        output: &mut [u8],
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        let mut any_allowed = false;
        for bucket in buckets {
            any_allowed |= self.fill_bucket_mask(parser_stack, bucket, output, scratch)?;
        }
        Ok(any_allowed)
    }

    fn fill_bucket_mask(
        &self,
        parser_stack: &[ParserStateId],
        bucket: &InverseBucket,
        output: &mut [u8],
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        if !self.sequence_allowed_with_scratch(parser_stack, bucket.sequence, scratch)? {
            return Ok(false);
        }

        for &token in &bucket.tokens {
            mask::set(output, token)?;
        }
        Ok(!bucket.tokens.is_empty())
    }

    fn fill_eos_mask(
        &self,
        lexer: LexerState,
        parser_stack: &[ParserStateId],
        output: &mut [u8],
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        let mut any_allowed = false;
        for token_index in 0..self.grammar.token_count() {
            any_allowed |= self.fill_eos_token(
                lexer,
                parser_stack,
                TokenId::new(token_index),
                output,
                scratch,
            )?;
        }
        Ok(any_allowed)
    }

    fn fill_eos_token(
        &self,
        lexer: LexerState,
        parser_stack: &[ParserStateId],
        token: TokenId,
        output: &mut [u8],
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        let execution = self
            .spanner
            .token_execution(lexer, token)
            .map_err(map_spanner_error)?;
        if let Some(TokenExecution::Finished { emitted, eof }) = execution {
            if *eof != self.grammar.lalr().eof_terminal() {
                return Err(EngineError::CorruptPreparedData);
            }
            if matches!(
                self.evaluate_eos_into(parser_stack, emitted, *eof, scratch)?,
                TokenDecision::Accepted
            ) {
                mask::set(output, token)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn evaluate_token_into(
        &self,
        lexer: LexerState,
        parser_stack: &[ParserStateId],
        token: TokenId,
        destination: &mut Vec<ParserStateId>,
    ) -> Result<TokenDecision, EngineError> {
        let execution = self
            .spanner
            .token_execution(lexer, token)
            .map_err(map_spanner_error)?;
        match execution {
            None => Ok(TokenDecision::Rejected),
            Some(TokenExecution::Continue { state, emitted }) => {
                if !self.ordinary_token_allowed_with_scratch(
                    lexer,
                    parser_stack,
                    token,
                    destination,
                )? {
                    return Ok(TokenDecision::Rejected);
                }
                let run = execute_terminals_concrete_into(
                    self.grammar.lalr(),
                    parser_stack,
                    emitted,
                    destination,
                )
                .map_err(map_parser_error)?;
                match run {
                    RunDecision::Continue => Ok(TokenDecision::Continue { lexer: *state }),
                    RunDecision::Rejected | RunDecision::Accepted | RunDecision::Dependent => {
                        Err(EngineError::CorruptPreparedData)
                    }
                }
            }
            Some(TokenExecution::Finished { emitted, eof }) => {
                if *eof != self.grammar.lalr().eof_terminal() {
                    return Err(EngineError::CorruptPreparedData);
                }
                self.evaluate_eos_into(parser_stack, emitted, *eof, destination)
            }
        }
    }

    fn ordinary_token_allowed_with_scratch(
        &self,
        lexer: LexerState,
        parser_stack: &[ParserStateId],
        token: TokenId,
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        let buckets = self
            .spanner
            .inverse_buckets(lexer)
            .map_err(map_spanner_error)?;
        for bucket in buckets {
            if bucket.tokens.contains(&token)
                && self.sequence_allowed_with_scratch(parser_stack, bucket.sequence, scratch)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn sequence_allowed_with_scratch(
        &self,
        parser_stack: &[ParserStateId],
        sequence: SequenceId,
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        let top = parser_stack
            .last()
            .copied()
            .ok_or(EngineError::CorruptPreparedData)?;
        let classification = self
            .parser
            .classification(top, sequence)
            .map_err(map_parser_error)?;
        match classification {
            SequenceClassification::AlwaysReadable => Ok(true),
            SequenceClassification::Rejected => Ok(false),
            SequenceClassification::Dependent => {
                let head = self
                    .spanner
                    .sequence(sequence)
                    .ok_or(EngineError::CorruptPreparedData)?;
                sequence_head_allowed_with_scratch(&self.grammar, parser_stack, head, scratch)
                    .map_err(map_parser_error)
            }
        }
    }

    fn evaluate_eos_into(
        &self,
        parser_stack: &[ParserStateId],
        emitted: &[crate::TerminalId],
        eof: crate::TerminalId,
        destination: &mut Vec<ParserStateId>,
    ) -> Result<TokenDecision, EngineError> {
        let emitted_run = execute_terminals_concrete_into(
            self.grammar.lalr(),
            parser_stack,
            emitted,
            destination,
        )
        .map_err(map_parser_error)?;
        match emitted_run {
            RunDecision::Rejected => return Ok(TokenDecision::Rejected),
            RunDecision::Continue => {}
            RunDecision::Accepted | RunDecision::Dependent => {
                return Err(EngineError::CorruptPreparedData);
            }
        }
        let eof_run = execute_terminals_concrete_in_place(self.grammar.lalr(), destination, &[eof])
            .map_err(map_parser_error)?;
        match eof_run {
            RunDecision::Accepted => Ok(TokenDecision::Accepted),
            RunDecision::Rejected | RunDecision::Continue => Ok(TokenDecision::Rejected),
            RunDecision::Dependent => Err(EngineError::CorruptPreparedData),
        }
    }

    fn check_token(&self, token: TokenId) -> Result<(), EngineError> {
        if token.get() >= self.grammar.token_count() {
            return Err(EngineError::InvalidToken { token });
        }
        Ok(())
    }
}

impl Matcher {
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.states.len()
    }

    #[must_use]
    pub const fn token_count(&self) -> u32 {
        self.prepared.token_count()
    }

    #[must_use]
    pub const fn mask_bytes(&self) -> usize {
        self.prepared.mask_bytes()
    }

    pub fn is_completed(&self, row: usize) -> Result<bool, EngineError> {
        Ok(matches!(self.state(row)?.phase, MatcherPhase::Accepted))
    }

    /// Writes one LSB-first packed vocabulary mask into caller-owned storage.
    /// Every byte is zero on failure and all unused tail bits remain zero.
    pub fn mask(&mut self, row: usize, output: &mut [u8]) -> Result<(), EngineError> {
        mask::clear(output);
        let result = self
            .states
            .get(row)
            .ok_or(EngineError::InvalidRow {
                row,
                rows: self.states.len(),
            })
            .and_then(|state| {
                self.prepared
                    .fill_mask_with_scratch(state, output, &mut self.scratch)
            });
        if result.is_err() {
            mask::clear(output);
        }
        result
    }

    /// Writes every row mask into one contiguous row-major packed buffer.
    pub fn masks(&mut self, output: &mut [u8]) -> Result<(), EngineError> {
        mask::clear(output);
        let required = self
            .states
            .len()
            .checked_mul(self.prepared.mask_bytes)
            .ok_or(EngineError::CorruptPreparedData)?;
        if output.len() < required {
            return Err(EngineError::BufferTooSmall {
                required,
                actual: output.len(),
            });
        }

        let result = fill_all_masks(&self.prepared, &self.states, &mut self.scratch, output);
        if result.is_err() {
            mask::clear(output);
        }
        result
    }

    pub fn allows(&self, row: usize, token: TokenId) -> Result<bool, EngineError> {
        self.prepared.allows_status(self.state(row)?, token)
    }

    pub fn advance(&mut self, row: usize, token: TokenId) -> Result<AdvanceResult, EngineError> {
        let rows = self.states.len();
        let state = self
            .states
            .get_mut(row)
            .ok_or(EngineError::InvalidRow { row, rows })?;
        let destination = self
            .staging
            .get_mut(row)
            .ok_or(EngineError::CorruptPreparedData)?;
        let result = self.prepared.next_status_into(state, token, destination)?;
        std::mem::swap(state, destination);
        Ok(result)
    }

    /// Advances every fixed-position row atomically.
    pub fn advance_batch(&mut self, tokens: &[TokenId]) -> Result<Vec<AdvanceResult>, EngineError> {
        if tokens.len() != self.states.len() {
            return Err(EngineError::BatchSizeMismatch {
                expected: self.states.len(),
                actual: tokens.len(),
            });
        }
        let mut results = reserved_vec(self.states.len())
            .map_err(|requested| EngineError::AllocationFailure { requested })?;
        stage_batch(
            &self.prepared,
            &self.states,
            &mut self.staging,
            tokens,
            &mut results,
        )?;
        std::mem::swap(&mut self.states, &mut self.staging);
        Ok(results)
    }

    /// Atomically advances running rows while retaining already accepted rows.
    /// `None` is valid only for a row which has already accepted.
    pub fn advance_active(
        &mut self,
        tokens: &[Option<TokenId>],
    ) -> Result<Vec<Option<AdvanceResult>>, EngineError> {
        if tokens.len() != self.states.len() {
            return Err(EngineError::BatchSizeMismatch {
                expected: self.states.len(),
                actual: tokens.len(),
            });
        }
        let mut results = reserved_vec(self.states.len())
            .map_err(|requested| EngineError::AllocationFailure { requested })?;
        stage_active(
            &self.prepared,
            &self.states,
            &mut self.staging,
            tokens,
            Some(&mut results),
            false,
        )?;
        std::mem::swap(&mut self.states, &mut self.staging);
        Ok(results)
    }

    /// Computes all successor masks and commits the fixed rows only when every
    /// running successor has a valid mask. Accepted rows leave a zero row for
    /// the caller to replace with its padding policy.
    pub fn advance_active_and_masks(
        &mut self,
        tokens: &[Option<TokenId>],
        output: &mut [u8],
    ) -> Result<Vec<Option<AdvanceResult>>, EngineError> {
        let mut results = reserved_vec(self.states.len())
            .map_err(|requested| EngineError::AllocationFailure { requested })?;
        self.advance_active_and_masks_impl(tokens, output, Some(&mut results), false)?;
        Ok(results)
    }

    /// Atomically advances running rows and writes successor masks without
    /// allocating or returning per-row results. `None` is valid only for a row
    /// which has already accepted. A rejected token identifies its batch row.
    ///
    /// The matcher retains one destination parser stack per row plus one shared
    /// scratch stack. Repeated calls allocate only when one of those stacks
    /// grows beyond its previous high-water capacity.
    pub fn advance_active_and_masks_no_result(
        &mut self,
        tokens: &[Option<TokenId>],
        output: &mut [u8],
    ) -> Result<(), EngineError> {
        self.advance_active_and_masks_impl(tokens, output, None, true)
    }

    fn advance_active_and_masks_impl(
        &mut self,
        tokens: &[Option<TokenId>],
        output: &mut [u8],
        results: Option<&mut Vec<Option<AdvanceResult>>>,
        report_rejected_row: bool,
    ) -> Result<(), EngineError> {
        mask::clear(output);
        if tokens.len() != self.states.len() {
            return Err(EngineError::BatchSizeMismatch {
                expected: self.states.len(),
                actual: tokens.len(),
            });
        }
        let required = self
            .states
            .len()
            .checked_mul(self.prepared.mask_bytes)
            .ok_or(EngineError::CorruptPreparedData)?;
        if output.len() < required {
            return Err(EngineError::BufferTooSmall {
                required,
                actual: output.len(),
            });
        }

        let staged = stage_active(
            &self.prepared,
            &self.states,
            &mut self.staging,
            tokens,
            results,
            report_rejected_row,
        )
        .and_then(|()| fill_staged_masks(&self.prepared, &self.staging, &mut self.scratch, output));
        match staged {
            Ok(()) => {
                std::mem::swap(&mut self.states, &mut self.staging);
                Ok(())
            }
            Err(error) => {
                mask::clear(output);
                Err(error)
            }
        }
    }

    fn state(&self, row: usize) -> Result<&MatcherStatus, EngineError> {
        self.states.get(row).ok_or(EngineError::InvalidRow {
            row,
            rows: self.states.len(),
        })
    }
}

fn fill_all_masks(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    scratch: &mut Vec<ParserStateId>,
    output: &mut [u8],
) -> Result<(), EngineError> {
    for (row, state) in states.iter().enumerate() {
        let row_output = mask_row(output, row, prepared.mask_bytes)?;
        prepared.fill_mask_with_scratch(state, row_output, scratch)?;
    }
    Ok(())
}

fn mask_row(output: &mut [u8], row: usize, mask_bytes: usize) -> Result<&mut [u8], EngineError> {
    let start = row
        .checked_mul(mask_bytes)
        .ok_or(EngineError::CorruptPreparedData)?;
    let end = start
        .checked_add(mask_bytes)
        .ok_or(EngineError::CorruptPreparedData)?;
    output
        .get_mut(start..end)
        .ok_or(EngineError::CorruptPreparedData)
}

fn stage_batch(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    staging: &mut [MatcherStatus],
    tokens: &[TokenId],
    results: &mut Vec<AdvanceResult>,
) -> Result<(), EngineError> {
    for (row, state) in states.iter().enumerate() {
        let token = *tokens.get(row).ok_or(EngineError::CorruptPreparedData)?;
        let destination = staging
            .get_mut(row)
            .ok_or(EngineError::CorruptPreparedData)?;
        results.push(prepared.next_status_into(state, token, destination)?);
    }
    Ok(())
}

fn stage_active(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    staging: &mut [MatcherStatus],
    tokens: &[Option<TokenId>],
    mut results: Option<&mut Vec<Option<AdvanceResult>>>,
    report_rejected_row: bool,
) -> Result<(), EngineError> {
    for (row, state) in states.iter().enumerate() {
        let token = *tokens.get(row).ok_or(EngineError::CorruptPreparedData)?;
        let result = match (state.phase, token) {
            (MatcherPhase::Accepted, None) => {
                staging
                    .get_mut(row)
                    .ok_or(EngineError::CorruptPreparedData)?
                    .set_accepted();
                None
            }
            (MatcherPhase::Accepted, Some(_)) => return Err(EngineError::Completed),
            (MatcherPhase::Running { .. }, None) => {
                return Err(EngineError::MissingRunningToken { row });
            }
            (MatcherPhase::Running { .. }, Some(token)) => {
                let destination = staging
                    .get_mut(row)
                    .ok_or(EngineError::CorruptPreparedData)?;
                Some(
                    prepared
                        .next_status_into(state, token, destination)
                        .map_err(|error| match error {
                            EngineError::ConstraintViolation { token } if report_rejected_row => {
                                EngineError::BatchConstraintViolation { row, token }
                            }
                            error => error,
                        })?,
                )
            }
        };
        if let Some(results) = results.as_mut() {
            results.push(result);
        }
    }
    Ok(())
}

fn fill_staged_masks(
    prepared: &PreparedGrammar,
    staging: &[MatcherStatus],
    scratch: &mut Vec<ParserStateId>,
    output: &mut [u8],
) -> Result<(), EngineError> {
    for (row, state) in staging.iter().enumerate() {
        if matches!(state.phase, MatcherPhase::Running { .. }) {
            let row_output = mask_row(output, row, prepared.mask_bytes)?;
            prepared.fill_mask_with_scratch(state, row_output, scratch)?;
        }
    }
    Ok(())
}

fn running_lexer(state: &MatcherStatus) -> Result<LexerState, EngineError> {
    match state.phase {
        MatcherPhase::Running { lexer } => Ok(lexer),
        MatcherPhase::Accepted => Err(EngineError::Completed),
    }
}

fn map_parser_error(error: ParserError) -> EngineError {
    match error {
        ParserError::AllocationFailure { requested } => {
            EngineError::AllocationFailure { requested }
        }
        ParserError::EmptyStack
        | ParserError::EmptySequenceHead
        | ParserError::InvalidState { .. }
        | ParserError::InvalidTerminal { .. }
        | ParserError::InvalidSequence { .. }
        | ParserError::StackUnderflow { .. }
        | ParserError::MissingGoto { .. }
        | ParserError::AcceptBeforeEnd { .. }
        | ParserError::InvalidReductionProgress { .. }
        | ParserError::InvariantViolation => EngineError::CorruptPreparedData,
    }
}

fn map_spanner_error(_error: SpannerQueryError) -> EngineError {
    EngineError::CorruptPreparedData
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Action, DfaStateId, LalrDimensions, LalrTable, LexerDfa, TerminalId, TokenEntry,
        UnvalidatedGrammar, ValidationLimits,
    };

    fn reusable_stack_grammar() -> ValidatedGrammar {
        let item = TerminalId::new(0);
        let eof = TerminalId::new(1);
        let sequence = crate::NonterminalId::new(0);
        let mut classes = vec![0; 256];
        classes[usize::from(b'a')] = 1;
        let lexer = LexerDfa::new(
            2,
            2,
            classes,
            vec![None, Some(DfaStateId::new(1)), None, None],
            DfaStateId::new(0),
            vec![None, Some(item)],
        );

        let mut actions = vec![Action::Error; 4 * 2];
        actions[0] = Action::Shift(ParserStateId::new(1));
        actions[2] = Action::Reduce {
            production: crate::ProductionId::new(0),
            rank: 0,
        };
        actions[3] = Action::Reduce {
            production: crate::ProductionId::new(0),
            rank: 0,
        };
        actions[4] = Action::Shift(ParserStateId::new(3));
        actions[5] = Action::Accept;
        actions[6] = Action::Reduce {
            production: crate::ProductionId::new(1),
            rank: 0,
        };
        actions[7] = Action::Reduce {
            production: crate::ProductionId::new(1),
            rank: 0,
        };
        let mut gotos = vec![None; 4];
        gotos[0] = Some(ParserStateId::new(2));
        let lalr = LalrTable::new(
            LalrDimensions::new(4, 2, 1),
            ParserStateId::new(0),
            eof,
            actions,
            gotos,
            vec![
                crate::Production::new(sequence, 1),
                crate::Production::new(sequence, 2),
            ],
        );

        UnvalidatedGrammar::new(
            vec![TokenEntry::Bytes(b"a".to_vec()), TokenEntry::Eos],
            lexer,
            lalr,
        )
        .validate(ValidationLimits::default())
        .expect("reusable-stack fixture validates")
    }

    #[test]
    fn no_result_path_matches_reference_and_reuses_stacks_after_high_water() {
        let mut matcher = prepare(reusable_stack_grammar(), PreparationLimits::default())
            .expect("preparation succeeds")
            .into_matcher(1)
            .expect("matcher allocation succeeds");
        let mut reference = prepare(reusable_stack_grammar(), PreparationLimits::default())
            .expect("reference preparation succeeds")
            .into_matcher(1)
            .expect("reference matcher allocation succeeds");
        let mut mask = vec![0; matcher.mask_bytes()];
        let mut reference_mask = vec![0; reference.mask_bytes()];
        let tokens = [Some(TokenId::new(0))];

        for _ in 0..4 {
            matcher
                .advance_active_and_masks_no_result(&tokens, &mut mask)
                .expect("warm-up advancement succeeds");
            assert_eq!(
                reference.advance_active_and_masks(&tokens, &mut reference_mask),
                Ok(vec![Some(AdvanceResult::Continue)])
            );
            assert_eq!(mask, reference_mask);
        }

        let state_ptr = matcher.states[0].parser_stack.as_ptr();
        let state_capacity = matcher.states[0].parser_stack.capacity();
        let staging_ptr = matcher.staging[0].parser_stack.as_ptr();
        let staging_capacity = matcher.staging[0].parser_stack.capacity();
        let scratch_ptr = matcher.scratch.as_ptr();
        let scratch_capacity = matcher.scratch.capacity();

        for _ in 0..2 {
            matcher
                .advance_active_and_masks_no_result(&tokens, &mut mask)
                .expect("steady-state advancement succeeds");
            assert_eq!(
                reference.advance_active_and_masks(&tokens, &mut reference_mask),
                Ok(vec![Some(AdvanceResult::Continue)])
            );
            assert_eq!(mask, reference_mask);
        }

        assert_eq!(matcher.states[0].parser_stack.as_ptr(), state_ptr);
        assert_eq!(matcher.states[0].parser_stack.capacity(), state_capacity);
        assert_eq!(matcher.staging[0].parser_stack.as_ptr(), staging_ptr);
        assert_eq!(matcher.staging[0].parser_stack.capacity(), staging_capacity);
        assert_eq!(matcher.scratch.as_ptr(), scratch_ptr);
        assert_eq!(matcher.scratch.capacity(), scratch_capacity);
    }

    #[test]
    fn caller_owned_mask_paths_reuse_the_matcher_scratch_stack() {
        let mut matcher = prepare(reusable_stack_grammar(), PreparationLimits::default())
            .expect("preparation succeeds")
            .into_matcher(2)
            .expect("matcher allocation succeeds");
        let mut one_mask = vec![0; matcher.mask_bytes()];
        let mut all_masks = vec![0; matcher.mask_bytes() * matcher.row_count()];

        matcher.mask(0, &mut one_mask).expect("row mask succeeds");
        matcher.masks(&mut all_masks).expect("batch masks succeed");
        assert!(matcher.scratch.capacity() > 0);
        let scratch_ptr = matcher.scratch.as_ptr();
        let scratch_capacity = matcher.scratch.capacity();

        matcher
            .mask(1, &mut one_mask)
            .expect("row mask reuses scratch");
        matcher
            .masks(&mut all_masks)
            .expect("batch masks reuse scratch");
        assert_eq!(matcher.scratch.as_ptr(), scratch_ptr);
        assert_eq!(matcher.scratch.capacity(), scratch_capacity);
    }
}
