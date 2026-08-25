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
        .and_then(|rounded| rounded.checked_div(8))
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
        let mut error = None;
        let mut row = 0_usize;
        while row < rows && error.is_none() {
            error = match self.initial_status() {
                Ok(status) => {
                    states.push(status);
                    staging.push(MatcherStatus::empty());
                    None
                }
                Err(initialization_error) => Some(initialization_error),
            };
            row += 1;
        }
        match error {
            Some(error) => Err(error),
            None => Ok(Matcher {
                prepared: self,
                states,
                staging,
                scratch: Vec::new(),
            }),
        }
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

        let buckets = match self.spanner.inverse_buckets(lexer) {
            Ok(buckets) => buckets,
            Err(error) => return Err(map_spanner_error(error)),
        };
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
        let mut error = None;
        let mut bucket_index = 0_usize;
        while bucket_index < buckets.len() && error.is_none() {
            match self.fill_bucket_at(parser_stack, buckets, bucket_index, output, scratch) {
                Ok(bucket_allowed) => any_allowed |= bucket_allowed,
                Err(bucket_error) => error = Some(bucket_error),
            }
            bucket_index += 1;
        }
        match error {
            Some(error) => Err(error),
            None => Ok(any_allowed),
        }
    }

    fn fill_bucket_at(
        &self,
        parser_stack: &[ParserStateId],
        buckets: &[InverseBucket],
        bucket_index: usize,
        output: &mut [u8],
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        let bucket = match buckets.get(bucket_index) {
            Some(bucket) => bucket,
            None => return Err(EngineError::CorruptPreparedData),
        };
        self.fill_bucket_mask(parser_stack, bucket, output, scratch)
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

        let mut any_allowed = false;
        let mut error = None;
        let mut token_index = 0_usize;
        while token_index < bucket.tokens.len() && error.is_none() {
            error = match bucket.tokens.get(token_index).copied() {
                Some(token) => match mask::set(output, token) {
                    Ok(()) => {
                        any_allowed = true;
                        None
                    }
                    Err(mask_error) => Some(mask_error),
                },
                None => Some(EngineError::CorruptPreparedData),
            };
            token_index += 1;
        }
        match error {
            Some(error) => Err(error),
            None => Ok(any_allowed),
        }
    }

    fn fill_eos_mask(
        &self,
        lexer: LexerState,
        parser_stack: &[ParserStateId],
        output: &mut [u8],
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        let mut any_allowed = false;
        let mut error = None;
        let mut token_index = 0_u32;
        while token_index < self.grammar.token_count() && error.is_none() {
            match self.fill_eos_token(
                lexer,
                parser_stack,
                TokenId::new(token_index),
                output,
                scratch,
            ) {
                Ok(token_allowed) => any_allowed |= token_allowed,
                Err(token_error) => error = Some(token_error),
            }
            token_index += 1;
        }
        match error {
            Some(error) => Err(error),
            None => Ok(any_allowed),
        }
    }

    fn fill_eos_token(
        &self,
        lexer: LexerState,
        parser_stack: &[ParserStateId],
        token: TokenId,
        output: &mut [u8],
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        let execution = match self.spanner.token_execution(lexer, token) {
            Ok(execution) => execution,
            Err(error) => return Err(map_spanner_error(error)),
        };
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
        let execution = match self.spanner.token_execution(lexer, token) {
            Ok(execution) => execution,
            Err(error) => return Err(map_spanner_error(error)),
        };
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
                let run = match execute_terminals_concrete_into(
                    self.grammar.lalr(),
                    parser_stack,
                    emitted,
                    destination,
                ) {
                    Ok(run) => run,
                    Err(error) => return Err(map_parser_error(error)),
                };
                match run {
                    RunDecision::Rejected => Err(EngineError::CorruptPreparedData),
                    RunDecision::Continue => Ok(TokenDecision::Continue { lexer: *state }),
                    RunDecision::Accepted | RunDecision::Dependent => {
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
        let buckets = match self.spanner.inverse_buckets(lexer) {
            Ok(buckets) => buckets,
            Err(error) => return Err(map_spanner_error(error)),
        };
        let mut allowed = false;
        let mut error = None;
        let mut bucket_index = 0_usize;
        while bucket_index < buckets.len() && !allowed && error.is_none() {
            match self.bucket_allows_token(parser_stack, buckets, bucket_index, token, scratch) {
                Ok(bucket_allowed) => allowed = bucket_allowed,
                Err(bucket_error) => error = Some(bucket_error),
            }
            bucket_index += 1;
        }
        match error {
            Some(error) => Err(error),
            None => Ok(allowed),
        }
    }

    fn bucket_allows_token(
        &self,
        parser_stack: &[ParserStateId],
        buckets: &[InverseBucket],
        bucket_index: usize,
        token: TokenId,
        scratch: &mut Vec<ParserStateId>,
    ) -> Result<bool, EngineError> {
        let bucket = match buckets.get(bucket_index) {
            Some(bucket) => bucket,
            None => return Err(EngineError::CorruptPreparedData),
        };
        match bucket_sequence_for_token(bucket, token) {
            Some(sequence) => self.sequence_allowed_with_scratch(parser_stack, sequence, scratch),
            None => Ok(false),
        }
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
        let classification = match self.parser.classification(top, sequence) {
            Ok(classification) => classification,
            Err(error) => return Err(map_parser_error(error)),
        };
        match classification {
            SequenceClassification::AlwaysReadable => Ok(true),
            SequenceClassification::Rejected => Ok(false),
            SequenceClassification::Dependent => {
                let head = self
                    .spanner
                    .sequence(sequence)
                    .ok_or(EngineError::CorruptPreparedData)?;
                match sequence_head_allowed_with_scratch(&self.grammar, parser_stack, head, scratch)
                {
                    Ok(allowed) => Ok(allowed),
                    Err(error) => Err(map_parser_error(error)),
                }
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
        let emitted_run = match execute_terminals_concrete_into(
            self.grammar.lalr(),
            parser_stack,
            emitted,
            destination,
        ) {
            Ok(run) => run,
            Err(error) => return Err(map_parser_error(error)),
        };
        match emitted_run {
            RunDecision::Rejected => return Ok(TokenDecision::Rejected),
            RunDecision::Continue => {}
            RunDecision::Accepted | RunDecision::Dependent => {
                return Err(EngineError::CorruptPreparedData);
            }
        };
        let eof_run =
            match execute_terminals_concrete_in_place(self.grammar.lalr(), destination, &[eof]) {
                Ok(run) => run,
                Err(error) => return Err(map_parser_error(error)),
            };
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

fn bucket_contains_token(bucket: &InverseBucket, token: TokenId) -> bool {
    let mut found = false;
    let mut member_index = 0_usize;
    while member_index < bucket.tokens.len() && !found {
        found = bucket.tokens.get(member_index).copied() == Some(token);
        member_index += 1;
    }
    found
}

fn bucket_sequence_for_token(bucket: &InverseBucket, token: TokenId) -> Option<SequenceId> {
    if bucket_contains_token(bucket, token) {
        Some(bucket.sequence)
    } else {
        None
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
        let rows = self.states.len();
        let prepared = &self.prepared;
        let states = &self.states;
        let scratch = &mut self.scratch;
        let result = fill_selected_mask(prepared, states, scratch, output, row, rows);
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

        let prepared = &self.prepared;
        let states = &self.states;
        let scratch = &mut self.scratch;
        let result = fill_all_masks(prepared, states, scratch, output);
        if result.is_err() {
            mask::clear(output);
        }
        result
    }

    pub fn allows(&self, row: usize, token: TokenId) -> Result<bool, EngineError> {
        self.prepared.allows_status(self.state(row)?, token)
    }

    #[allow(clippy::question_mark)]
    pub fn advance(&mut self, row: usize, token: TokenId) -> Result<AdvanceResult, EngineError> {
        let rows = self.states.len();
        let state = match self.states.get(row) {
            Some(state) => state,
            None => return Err(EngineError::InvalidRow { row, rows }),
        };
        let result = match next_status_at(&self.prepared, state, &mut self.staging, row, token) {
            Ok(result) => result,
            Err(error) => return Err(error),
        };
        match swap_status_at(&mut self.states, &mut self.staging, row) {
            Ok(()) => Ok(result),
            Err(error) => Err(error),
        }
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
        stage_active_with_results(
            &self.prepared,
            &self.states,
            &mut self.staging,
            tokens,
            &mut results,
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

        let staged_rows = match results {
            Some(results) => stage_active_with_results(
                &self.prepared,
                &self.states,
                &mut self.staging,
                tokens,
                results,
                report_rejected_row,
            ),
            None => stage_active_without_results(
                &self.prepared,
                &self.states,
                &mut self.staging,
                tokens,
                report_rejected_row,
            ),
        };
        let staged = match staged_rows {
            Ok(()) => fill_staged_masks(&self.prepared, &self.staging, &mut self.scratch, output),
            Err(error) => Err(error),
        };
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
    let mut error = None;
    let mut row = 0_usize;
    while row < states.len() && error.is_none() {
        error = fill_mask_row(prepared, states, scratch, output, row).err();
        row += 1;
    }
    engine_result(error)
}

fn fill_selected_mask(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    scratch: &mut Vec<ParserStateId>,
    output: &mut [u8],
    row: usize,
    rows: usize,
) -> Result<(), EngineError> {
    let state = match states.get(row) {
        Some(state) => state,
        None => return Err(EngineError::InvalidRow { row, rows }),
    };
    prepared.fill_mask_with_scratch(state, output, scratch)
}

fn fill_mask_row(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    scratch: &mut Vec<ParserStateId>,
    output: &mut [u8],
    row: usize,
) -> Result<(), EngineError> {
    let start = row
        .checked_mul(prepared.mask_bytes)
        .ok_or(EngineError::CorruptPreparedData)?;
    let end = start
        .checked_add(prepared.mask_bytes)
        .ok_or(EngineError::CorruptPreparedData)?;
    let state = match states.get(row) {
        Some(state) => state,
        None => return Err(EngineError::CorruptPreparedData),
    };
    let row_output = match output.get_mut(start..end) {
        Some(row_output) => row_output,
        None => return Err(EngineError::CorruptPreparedData),
    };
    match prepared.fill_mask_with_scratch(state, row_output, scratch) {
        Ok(()) => Ok(()),
        Err(error) => Err(error),
    }
}

fn stage_batch(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    staging: &mut [MatcherStatus],
    tokens: &[TokenId],
    results: &mut Vec<AdvanceResult>,
) -> Result<(), EngineError> {
    let mut error = None;
    let mut row = 0_usize;
    while row < states.len() && error.is_none() {
        error = stage_batch_row(prepared, states, staging, tokens, results, row).err();
        row += 1;
    }
    engine_result(error)
}

fn stage_batch_row(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    staging: &mut [MatcherStatus],
    tokens: &[TokenId],
    results: &mut Vec<AdvanceResult>,
    row: usize,
) -> Result<(), EngineError> {
    let token = match tokens.get(row).copied() {
        Some(token) => token,
        None => return Err(EngineError::CorruptPreparedData),
    };
    let state = match states.get(row) {
        Some(state) => state,
        None => return Err(EngineError::CorruptPreparedData),
    };
    match next_status_at(prepared, state, staging, row, token) {
        Ok(result) => {
            results.push(result);
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn stage_active_with_results(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    staging: &mut [MatcherStatus],
    tokens: &[Option<TokenId>],
    results: &mut Vec<Option<AdvanceResult>>,
    report_rejected_row: bool,
) -> Result<(), EngineError> {
    let mut error = None;
    let mut row = 0_usize;
    while row < states.len() && error.is_none() {
        match stage_active_row(prepared, states, staging, tokens, row, report_rejected_row) {
            Ok(result) => results.push(result),
            Err(row_error) => error = Some(row_error),
        }
        row += 1;
    }
    engine_result(error)
}

fn stage_active_without_results(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    staging: &mut [MatcherStatus],
    tokens: &[Option<TokenId>],
    report_rejected_row: bool,
) -> Result<(), EngineError> {
    let mut error = None;
    let mut row = 0_usize;
    while row < states.len() && error.is_none() {
        error = stage_active_row(prepared, states, staging, tokens, row, report_rejected_row).err();
        row += 1;
    }
    engine_result(error)
}

fn stage_active_row(
    prepared: &PreparedGrammar,
    states: &[MatcherStatus],
    staging: &mut [MatcherStatus],
    tokens: &[Option<TokenId>],
    row: usize,
    report_rejected_row: bool,
) -> Result<Option<AdvanceResult>, EngineError> {
    let state = match states.get(row) {
        Some(state) => state,
        None => return Err(EngineError::CorruptPreparedData),
    };
    let token = match tokens.get(row).copied() {
        Some(token) => token,
        None => return Err(EngineError::CorruptPreparedData),
    };
    match (state.phase, token) {
        (MatcherPhase::Accepted, None) => match set_staging_accepted(staging, row) {
            Ok(()) => Ok(None),
            Err(error) => Err(error),
        },
        (MatcherPhase::Accepted, Some(_)) => Err(EngineError::Completed),
        (MatcherPhase::Running { .. }, None) => Err(EngineError::MissingRunningToken { row }),
        (MatcherPhase::Running { .. }, Some(token)) => {
            match next_status_at(prepared, state, staging, row, token) {
                Ok(result) => Ok(Some(result)),
                Err(EngineError::ConstraintViolation { token }) if report_rejected_row => {
                    Err(EngineError::BatchConstraintViolation { row, token })
                }
                Err(error) => Err(error),
            }
        }
    }
}

#[allow(clippy::needless_match)]
fn next_status_at(
    prepared: &PreparedGrammar,
    state: &MatcherStatus,
    staging: &mut [MatcherStatus],
    row: usize,
    token: TokenId,
) -> Result<AdvanceResult, EngineError> {
    let destination = match staging.get_mut(row) {
        Some(destination) => destination,
        None => return Err(EngineError::CorruptPreparedData),
    };
    match prepared.next_status_into(state, token, destination) {
        Ok(result) => Ok(result),
        Err(error) => Err(error),
    }
}

fn set_staging_accepted(staging: &mut [MatcherStatus], row: usize) -> Result<(), EngineError> {
    match staging.get_mut(row) {
        Some(destination) => {
            destination.set_accepted();
            Ok(())
        }
        None => Err(EngineError::CorruptPreparedData),
    }
}

fn swap_status_at(
    states: &mut [MatcherStatus],
    staging: &mut [MatcherStatus],
    row: usize,
) -> Result<(), EngineError> {
    let state = match states.get_mut(row) {
        Some(state) => state,
        None => return Err(EngineError::CorruptPreparedData),
    };
    let destination = match staging.get_mut(row) {
        Some(destination) => destination,
        None => return Err(EngineError::CorruptPreparedData),
    };
    std::mem::swap(state, destination);
    Ok(())
}

fn fill_staged_masks(
    prepared: &PreparedGrammar,
    staging: &[MatcherStatus],
    scratch: &mut Vec<ParserStateId>,
    output: &mut [u8],
) -> Result<(), EngineError> {
    let mut error = None;
    let mut row = 0_usize;
    while row < staging.len() && error.is_none() {
        error = fill_staged_mask_row(prepared, staging, scratch, output, row).err();
        row += 1;
    }
    engine_result(error)
}

fn fill_staged_mask_row(
    prepared: &PreparedGrammar,
    staging: &[MatcherStatus],
    scratch: &mut Vec<ParserStateId>,
    output: &mut [u8],
    row: usize,
) -> Result<(), EngineError> {
    let state = match staging.get(row) {
        Some(state) => state,
        None => return Err(EngineError::CorruptPreparedData),
    };
    if matches!(state.phase, MatcherPhase::Running { .. }) {
        let start = row
            .checked_mul(prepared.mask_bytes)
            .ok_or(EngineError::CorruptPreparedData)?;
        let end = start
            .checked_add(prepared.mask_bytes)
            .ok_or(EngineError::CorruptPreparedData)?;
        let row_output = match output.get_mut(start..end) {
            Some(row_output) => row_output,
            None => return Err(EngineError::CorruptPreparedData),
        };
        match prepared.fill_mask_with_scratch(state, row_output, scratch) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn engine_result(error: Option<EngineError>) -> Result<(), EngineError> {
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
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
