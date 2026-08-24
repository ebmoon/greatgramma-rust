#![allow(dead_code)]

use crate::{
    DfaStateId, LexerError, LexerInput, LexerState, LexerStep, TerminalId, TokenEntry, TokenId,
    ValidatedGrammar, lexer_step,
};

use crate::PreparationLimits;

/// The result of composing one complete model token with lexer execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenExecution {
    /// An ordinary token consumed all of its bytes and left a residual lexer state.
    Continue {
        state: LexerState,
        emitted: Vec<TerminalId>,
    },
    /// An EOS token finished lexer execution. `emitted` contains only a
    /// residual accepted terminal; EOF remains explicit and final.
    Finished {
        emitted: Vec<TerminalId>,
        eof: TerminalId,
    },
}

/// A fail-closed error for invalid public token-composition inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenStepError {
    InvalidToken { token: TokenId },
    InvalidState { state: DfaStateId },
    ArithmeticOverflow,
    AllocationFailure { requested: usize },
}

/// Identifies a finite resource budget used while preparing direct token steps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationLimitKind {
    TrieNodes,
    TrieEdges,
    LogicalTokenBytes,
    SourceTokenCells,
    OutputPoolTerminals,
    RowInterningWork,
    OutputInterningWork,
    Work,
}

/// Identifies a checked preparation calculation that overflowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationArithmeticKind {
    CountConversion,
    SourceTokenCells,
    Counter,
    OutputRange,
    RowRange,
    OutputId,
    RowId,
}

/// Identifies derived storage whose allocation could not be reserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationStorage {
    TrieNodes,
    TrieEdges,
    TokenEnds,
    TraversalStates,
    TraversalOutputs,
    CandidateCells,
    Cells,
    Rows,
    SourceRows,
    OutputPool,
    Outputs,
}

/// A fail-closed deterministic direct-table preparation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparationError {
    LimitExceeded {
        limit: PreparationLimitKind,
        actual: u64,
        maximum: u64,
    },
    ArithmeticOverflow {
        calculation: PreparationArithmeticKind,
    },
    AllocationFailure {
        storage: PreparationStorage,
        requested: usize,
    },
    TokenStep {
        error: TokenStepError,
    },
    InvalidDerivedState {
        state: DfaStateId,
    },
}

/// Composes exactly one normalized model token with the corrected lexer.
///
/// `Ok(None)` means that ordinary lexer rejection occurred; it never exposes a
/// partial successor or partial terminal sequence. Invalid token IDs and DFA
/// states are reported separately.
pub fn execute_token(
    grammar: &ValidatedGrammar,
    source: LexerState,
    token: TokenId,
) -> Result<Option<TokenExecution>, TokenStepError> {
    check_source(grammar, source)?;
    let entry = grammar
        .token(token)
        .ok_or(TokenStepError::InvalidToken { token })?;

    match entry {
        TokenEntry::Bytes(bytes) => execute_bytes(grammar, source, bytes),
        TokenEntry::Eos => execute_eos(grammar, source),
    }
}

fn check_source(grammar: &ValidatedGrammar, source: LexerState) -> Result<(), TokenStepError> {
    match source {
        LexerState::Start => Ok(()),
        LexerState::Dfa(state) => {
            if grammar.lexer().terminal(state).is_some() {
                Ok(())
            } else {
                Err(TokenStepError::InvalidState { state })
            }
        }
    }
}

fn execute_bytes(
    grammar: &ValidatedGrammar,
    source: LexerState,
    bytes: &[u8],
) -> Result<Option<TokenExecution>, TokenStepError> {
    let mut state = source;
    let mut emitted = Vec::new();
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        match lexer_step(grammar, state, LexerInput::Byte(byte)) {
            Ok(LexerStep::Continue {
                state: destination,
                emitted: terminal,
            }) => {
                if let Some(terminal) = terminal {
                    let requested =
                        emitted
                            .len()
                            .checked_add(1)
                            .ok_or(TokenStepError::AllocationFailure {
                                requested: usize::MAX,
                            })?;
                    emitted
                        .try_reserve(1)
                        .map_err(|_| TokenStepError::AllocationFailure { requested })?;
                    emitted.push(terminal);
                }
                state = destination;
            }
            Ok(LexerStep::Finished { .. }) => return Ok(None),
            Err(error) => return map_lexer_rejection(error),
        }
        index = index
            .checked_add(1)
            .ok_or(TokenStepError::ArithmeticOverflow)?;
    }
    Ok(Some(TokenExecution::Continue { state, emitted }))
}

fn execute_eos(
    grammar: &ValidatedGrammar,
    source: LexerState,
) -> Result<Option<TokenExecution>, TokenStepError> {
    match lexer_step(grammar, source, LexerInput::Eos) {
        Ok(LexerStep::Finished { terminal, eof }) => {
            let mut emitted = Vec::new();
            if let Some(terminal) = terminal {
                emitted
                    .try_reserve(1)
                    .map_err(|_| TokenStepError::AllocationFailure { requested: 1 })?;
                emitted.push(terminal);
            }
            Ok(Some(TokenExecution::Finished { emitted, eof }))
        }
        Ok(LexerStep::Continue { .. }) => Ok(None),
        Err(error) => map_lexer_rejection(error),
    }
}

fn map_lexer_rejection(error: LexerError) -> Result<Option<TokenExecution>, TokenStepError> {
    match error {
        LexerError::InvalidState { state } => Err(TokenStepError::InvalidState { state }),
        LexerError::CannotBeginLexeme { .. }
        | LexerError::ByteAfterUnfinishedResidual { .. }
        | LexerError::EosAfterUnfinishedResidual { .. } => Ok(None),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutputId(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RowId(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutputRange {
    start: usize,
    len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparedRow {
    start: usize,
    len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreparedCell {
    Rejected,
    Continue { state: LexerState, output: OutputId },
    Finished { output: OutputId, eof: TerminalId },
}

/// Deterministically interned direct token relation. It remains crate-private
/// until the owned prepared-grammar API is introduced.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedTokenTable {
    token_count: usize,
    source_rows: Vec<RowId>,
    rows: Vec<PreparedRow>,
    cells: Vec<PreparedCell>,
    output_pool: Vec<TerminalId>,
    outputs: Vec<OutputRange>,
}

/// A borrowed view of one prepared direct token cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedTokenExecution<'a> {
    Rejected,
    Continue {
        state: LexerState,
        emitted: &'a [TerminalId],
    },
    Finished {
        emitted: &'a [TerminalId],
        eof: TerminalId,
    },
}

impl PreparedTokenTable {
    pub(crate) fn source_count(&self) -> usize {
        self.source_rows.len()
    }

    pub(crate) fn source_for_row(&self, row: usize) -> Option<LexerState> {
        if row == 0 {
            return Some(LexerState::Start);
        }
        let dfa = u32::try_from(row.checked_sub(1)?).ok()?;
        Some(LexerState::Dfa(DfaStateId::new(dfa)))
    }

    pub(crate) fn cell(&self, row: usize, token: TokenId) -> Option<PreparedTokenExecution<'_>> {
        let row_id = *self.source_rows.get(row)?;
        let row_index = usize::try_from(row_id.0).ok()?;
        let prepared_row = *self.rows.get(row_index)?;
        let token_index = usize::try_from(token.get()).ok()?;
        if token_index >= self.token_count || token_index >= prepared_row.len {
            return None;
        }
        let cell_index = prepared_row.start.checked_add(token_index)?;
        match *self.cells.get(cell_index)? {
            PreparedCell::Rejected => Some(PreparedTokenExecution::Rejected),
            PreparedCell::Continue { state, output } => Some(PreparedTokenExecution::Continue {
                state,
                emitted: self.output(output)?,
            }),
            PreparedCell::Finished { output, eof } => Some(PreparedTokenExecution::Finished {
                emitted: self.output(output)?,
                eof,
            }),
        }
    }

    fn output(&self, id: OutputId) -> Option<&[TerminalId]> {
        let output = *self.outputs.get(usize::try_from(id.0).ok()?)?;
        let end = output.start.checked_add(output.len)?;
        self.output_pool.get(output.start..end)
    }
}

#[derive(Clone, Copy)]
struct TrieNode {
    parent: Option<usize>,
    byte: u8,
}

#[derive(Clone, Copy)]
struct TrieEdge {
    parent: usize,
    byte: u8,
    child: usize,
}

struct FlatByteTrie {
    nodes: Vec<TrieNode>,
    edges: Vec<TrieEdge>,
    token_ends: Vec<Option<usize>>,
}

#[derive(Clone, Copy)]
struct TraceTerminal {
    parent: Option<usize>,
    terminal: TerminalId,
}

#[derive(Clone, Copy)]
struct NodeExecution {
    state: Option<LexerState>,
    trace: Option<usize>,
}

struct Budget<'a> {
    limits: &'a PreparationLimits,
    trie_nodes: u64,
    trie_edges: u64,
    logical_token_bytes: u64,
    source_token_cells: u64,
    output_pool_terminals: u64,
    row_interning_work: u64,
    output_interning_work: u64,
    work: u64,
}

impl<'a> Budget<'a> {
    fn new(limits: &'a PreparationLimits) -> Self {
        Self {
            limits,
            trie_nodes: 0,
            trie_edges: 0,
            logical_token_bytes: 0,
            source_token_cells: 0,
            output_pool_terminals: 0,
            row_interning_work: 0,
            output_interning_work: 0,
            work: 0,
        }
    }

    fn charge(&mut self, kind: PreparationLimitKind, amount: u64) -> Result<(), PreparationError> {
        let (counter, maximum) = match kind {
            PreparationLimitKind::TrieNodes => (&mut self.trie_nodes, self.limits.max_trie_nodes),
            PreparationLimitKind::TrieEdges => (&mut self.trie_edges, self.limits.max_trie_edges),
            PreparationLimitKind::LogicalTokenBytes => (
                &mut self.logical_token_bytes,
                self.limits.max_logical_token_bytes,
            ),
            PreparationLimitKind::SourceTokenCells => (
                &mut self.source_token_cells,
                self.limits.max_source_token_cells,
            ),
            PreparationLimitKind::OutputPoolTerminals => (
                &mut self.output_pool_terminals,
                self.limits.max_output_pool_terminals,
            ),
            PreparationLimitKind::RowInterningWork => (
                &mut self.row_interning_work,
                self.limits.max_row_interning_work,
            ),
            PreparationLimitKind::OutputInterningWork => (
                &mut self.output_interning_work,
                self.limits.max_output_interning_work,
            ),
            PreparationLimitKind::Work => (&mut self.work, self.limits.max_work),
        };
        let actual = counter
            .checked_add(amount)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::Counter,
            })?;
        if actual > maximum {
            return Err(PreparationError::LimitExceeded {
                limit: kind,
                actual,
                maximum,
            });
        }
        *counter = actual;
        if kind != PreparationLimitKind::Work {
            self.charge(PreparationLimitKind::Work, amount)?;
        }
        Ok(())
    }
}

/// Builds the crate-private direct token table from the closed validated
/// boundary. Every source row and every token ID receives a distinct cell.
pub(crate) fn prepare_token_table(
    grammar: &ValidatedGrammar,
    limits: PreparationLimits,
) -> Result<PreparedTokenTable, PreparationError> {
    let mut budget = Budget::new(&limits);
    let source_count = u64::from(grammar.lexer().state_count())
        .checked_add(1)
        .ok_or(PreparationError::ArithmeticOverflow {
            calculation: PreparationArithmeticKind::Counter,
        })?;
    let token_count = u64::from(grammar.token_count());
    let source_cells =
        source_count
            .checked_mul(token_count)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::SourceTokenCells,
            })?;
    budget.charge(PreparationLimitKind::SourceTokenCells, source_cells)?;

    let trie = build_trie(grammar, &mut budget)?;
    let source_count = count_as_usize(source_count)?;
    let token_count = count_as_usize(token_count)?;
    let mut table = PreparedTokenTable {
        token_count,
        source_rows: Vec::new(),
        rows: Vec::new(),
        cells: Vec::new(),
        output_pool: Vec::new(),
        outputs: Vec::new(),
    };
    reserve(
        &mut table.source_rows,
        source_count,
        PreparationStorage::SourceRows,
    )?;

    let mut row = 0;
    while row < source_count {
        let source = table
            .source_for_row(row)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::CountConversion,
            })?;
        let candidate = build_source_cells(grammar, source, &trie, &mut table, &mut budget)?;
        let row_id = intern_row(&mut table, candidate, &mut budget)?;
        table.source_rows.push(row_id);
        row = next_index(row)?;
    }
    Ok(table)
}

fn build_trie(
    grammar: &ValidatedGrammar,
    budget: &mut Budget<'_>,
) -> Result<FlatByteTrie, PreparationError> {
    let token_count = count_as_usize(u64::from(grammar.token_count()))?;
    budget.charge(PreparationLimitKind::TrieNodes, 1)?;
    let mut trie = FlatByteTrie {
        nodes: Vec::new(),
        edges: Vec::new(),
        token_ends: Vec::new(),
    };
    reserve(&mut trie.nodes, 1, PreparationStorage::TrieNodes)?;
    trie.nodes.push(TrieNode {
        parent: None,
        byte: 0,
    });
    reserve(
        &mut trie.token_ends,
        token_count,
        PreparationStorage::TokenEnds,
    )?;

    let mut token_index = 0;
    while token_index < token_count {
        let token = TokenId::new(u32::try_from(token_index).map_err(|_| {
            PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::CountConversion,
            }
        })?);
        let end = match grammar.token(token) {
            Some(TokenEntry::Bytes(bytes)) => {
                let byte_len = u64::try_from(bytes.len()).map_err(|_| {
                    PreparationError::ArithmeticOverflow {
                        calculation: PreparationArithmeticKind::CountConversion,
                    }
                })?;
                budget.charge(PreparationLimitKind::LogicalTokenBytes, byte_len)?;
                let mut node = 0;
                let mut byte_index = 0;
                while byte_index < bytes.len() {
                    let byte =
                        *bytes
                            .get(byte_index)
                            .ok_or(PreparationError::ArithmeticOverflow {
                                calculation: PreparationArithmeticKind::CountConversion,
                            })?;
                    node = trie_child_or_insert(&mut trie, node, byte, budget)?;
                    byte_index = next_index(byte_index)?;
                }
                Some(node)
            }
            Some(TokenEntry::Eos) => None,
            None => {
                return Err(PreparationError::ArithmeticOverflow {
                    calculation: PreparationArithmeticKind::CountConversion,
                });
            }
        };
        trie.token_ends.push(end);
        token_index = next_index(token_index)?;
    }
    Ok(trie)
}

fn trie_child_or_insert(
    trie: &mut FlatByteTrie,
    parent: usize,
    byte: u8,
    budget: &mut Budget<'_>,
) -> Result<usize, PreparationError> {
    let mut edge_index = 0;
    while edge_index < trie.edges.len() {
        budget.charge(PreparationLimitKind::Work, 1)?;
        let edge = *trie
            .edges
            .get(edge_index)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::CountConversion,
            })?;
        if edge.parent == parent && edge.byte == byte {
            return Ok(edge.child);
        }
        edge_index = next_index(edge_index)?;
    }
    budget.charge(PreparationLimitKind::TrieNodes, 1)?;
    budget.charge(PreparationLimitKind::TrieEdges, 1)?;
    let child = trie.nodes.len();
    reserve(&mut trie.nodes, 1, PreparationStorage::TrieNodes)?;
    reserve(&mut trie.edges, 1, PreparationStorage::TrieEdges)?;
    trie.nodes.push(TrieNode {
        parent: Some(parent),
        byte,
    });
    trie.edges.push(TrieEdge {
        parent,
        byte,
        child,
    });
    Ok(child)
}

fn build_source_cells(
    grammar: &ValidatedGrammar,
    source: LexerState,
    trie: &FlatByteTrie,
    table: &mut PreparedTokenTable,
    budget: &mut Budget<'_>,
) -> Result<Vec<PreparedCell>, PreparationError> {
    let mut executions = Vec::new();
    reserve(
        &mut executions,
        trie.nodes.len(),
        PreparationStorage::TraversalStates,
    )?;
    budget.charge(
        PreparationLimitKind::Work,
        u64::try_from(trie.nodes.len()).map_err(|_| PreparationError::ArithmeticOverflow {
            calculation: PreparationArithmeticKind::CountConversion,
        })?,
    )?;
    executions.push(NodeExecution {
        state: Some(source),
        trace: None,
    });
    let mut traces = Vec::new();
    let mut node_index = 1;
    while node_index < trie.nodes.len() {
        let node = *trie
            .nodes
            .get(node_index)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::CountConversion,
            })?;
        let parent = node.parent.ok_or(PreparationError::ArithmeticOverflow {
            calculation: PreparationArithmeticKind::CountConversion,
        })?;
        let parent_execution =
            *executions
                .get(parent)
                .ok_or(PreparationError::ArithmeticOverflow {
                    calculation: PreparationArithmeticKind::CountConversion,
                })?;
        let execution = match parent_execution.state {
            None => NodeExecution {
                state: None,
                trace: None,
            },
            Some(parent_state) => {
                match lexer_step(grammar, parent_state, LexerInput::Byte(node.byte)) {
                    Ok(LexerStep::Continue {
                        state,
                        emitted: terminal,
                    }) => {
                        let trace = if let Some(terminal) = terminal {
                            budget.charge(PreparationLimitKind::Work, 1)?;
                            reserve(&mut traces, 1, PreparationStorage::TraversalOutputs)?;
                            let next = traces.len();
                            traces.push(TraceTerminal {
                                parent: parent_execution.trace,
                                terminal,
                            });
                            Some(next)
                        } else {
                            parent_execution.trace
                        };
                        NodeExecution {
                            state: Some(state),
                            trace,
                        }
                    }
                    Ok(LexerStep::Finished { .. }) => NodeExecution {
                        state: None,
                        trace: None,
                    },
                    Err(LexerError::InvalidState { state }) => {
                        return Err(PreparationError::InvalidDerivedState { state });
                    }
                    Err(
                        LexerError::CannotBeginLexeme { .. }
                        | LexerError::ByteAfterUnfinishedResidual { .. }
                        | LexerError::EosAfterUnfinishedResidual { .. },
                    ) => NodeExecution {
                        state: None,
                        trace: None,
                    },
                }
            }
        };
        executions.push(execution);
        node_index = next_index(node_index)?;
    }

    let mut candidate = Vec::new();
    reserve(
        &mut candidate,
        trie.token_ends.len(),
        PreparationStorage::CandidateCells,
    )?;
    let mut token_index = 0;
    while token_index < trie.token_ends.len() {
        budget.charge(PreparationLimitKind::Work, 1)?;
        let token = TokenId::new(u32::try_from(token_index).map_err(|_| {
            PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::CountConversion,
            }
        })?);
        let token_end =
            *trie
                .token_ends
                .get(token_index)
                .ok_or(PreparationError::ArithmeticOverflow {
                    calculation: PreparationArithmeticKind::CountConversion,
                })?;
        let cell = match token_end {
            Some(end) => match executions
                .get(end)
                .ok_or(PreparationError::ArithmeticOverflow {
                    calculation: PreparationArithmeticKind::CountConversion,
                })?
                .state
            {
                None => PreparedCell::Rejected,
                Some(state) => {
                    let trace = executions
                        .get(end)
                        .ok_or(PreparationError::ArithmeticOverflow {
                            calculation: PreparationArithmeticKind::CountConversion,
                        })?
                        .trace;
                    let output = materialize_trace(trace, &traces, budget)?;
                    let output = intern_output(table, &output, budget)?;
                    PreparedCell::Continue { state, output }
                }
            },
            None => match execute_token(grammar, source, token)
                .map_err(|error| PreparationError::TokenStep { error })?
            {
                None => PreparedCell::Rejected,
                Some(TokenExecution::Continue { state, emitted }) => {
                    let output = intern_output(table, &emitted, budget)?;
                    PreparedCell::Continue { state, output }
                }
                Some(TokenExecution::Finished { emitted, eof }) => {
                    let output = intern_output(table, &emitted, budget)?;
                    PreparedCell::Finished { output, eof }
                }
            },
        };
        candidate.push(cell);
        token_index = next_index(token_index)?;
    }
    Ok(candidate)
}

fn materialize_trace(
    trace: Option<usize>,
    traces: &[TraceTerminal],
    budget: &mut Budget<'_>,
) -> Result<Vec<TerminalId>, PreparationError> {
    let mut reversed = Vec::new();
    let mut cursor = trace;
    while let Some(index) = cursor {
        budget.charge(PreparationLimitKind::Work, 1)?;
        let item = *traces
            .get(index)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::CountConversion,
            })?;
        reserve(&mut reversed, 1, PreparationStorage::TraversalOutputs)?;
        reversed.push(item.terminal);
        cursor = item.parent;
    }
    reversed.reverse();
    Ok(reversed)
}

fn intern_output(
    table: &mut PreparedTokenTable,
    output: &[TerminalId],
    budget: &mut Budget<'_>,
) -> Result<OutputId, PreparationError> {
    let mut index = 0;
    while index < table.outputs.len() {
        budget.charge(PreparationLimitKind::OutputInterningWork, 1)?;
        let range = *table
            .outputs
            .get(index)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::OutputRange,
            })?;
        if range.len == output.len() {
            let mut terminal_index = 0;
            let mut equal = true;
            while terminal_index < output.len() {
                budget.charge(PreparationLimitKind::OutputInterningWork, 1)?;
                let pool_index = range.start.checked_add(terminal_index).ok_or(
                    PreparationError::ArithmeticOverflow {
                        calculation: PreparationArithmeticKind::OutputRange,
                    },
                )?;
                if table.output_pool.get(pool_index) != output.get(terminal_index) {
                    equal = false;
                    break;
                }
                terminal_index = next_index(terminal_index)?;
            }
            if equal {
                return output_id(index);
            }
        }
        index = next_index(index)?;
    }
    let start = table.output_pool.len();
    budget.charge(
        PreparationLimitKind::OutputPoolTerminals,
        u64::try_from(output.len()).map_err(|_| PreparationError::ArithmeticOverflow {
            calculation: PreparationArithmeticKind::CountConversion,
        })?,
    )?;
    reserve(
        &mut table.output_pool,
        output.len(),
        PreparationStorage::OutputPool,
    )?;
    reserve(&mut table.outputs, 1, PreparationStorage::Outputs)?;
    let mut terminal_index = 0;
    while terminal_index < output.len() {
        let terminal = *output
            .get(terminal_index)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::OutputRange,
            })?;
        table.output_pool.push(terminal);
        terminal_index = next_index(terminal_index)?;
    }
    table.outputs.push(OutputRange {
        start,
        len: output.len(),
    });
    output_id(
        table
            .outputs
            .len()
            .checked_sub(1)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::OutputId,
            })?,
    )
}

fn intern_row(
    table: &mut PreparedTokenTable,
    candidate: Vec<PreparedCell>,
    budget: &mut Budget<'_>,
) -> Result<RowId, PreparationError> {
    let mut row_index = 0;
    while row_index < table.rows.len() {
        budget.charge(PreparationLimitKind::RowInterningWork, 1)?;
        let row = *table
            .rows
            .get(row_index)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::RowRange,
            })?;
        if row.len == candidate.len() {
            let mut cell_index = 0;
            let mut equal = true;
            while cell_index < candidate.len() {
                budget.charge(PreparationLimitKind::RowInterningWork, 1)?;
                let existing_index = row.start.checked_add(cell_index).ok_or(
                    PreparationError::ArithmeticOverflow {
                        calculation: PreparationArithmeticKind::RowRange,
                    },
                )?;
                if table.cells.get(existing_index) != candidate.get(cell_index) {
                    equal = false;
                    break;
                }
                cell_index = next_index(cell_index)?;
            }
            if equal {
                return row_id(row_index);
            }
        }
        row_index = next_index(row_index)?;
    }
    let start = table.cells.len();
    reserve(&mut table.cells, candidate.len(), PreparationStorage::Cells)?;
    reserve(&mut table.rows, 1, PreparationStorage::Rows)?;
    table.cells.extend(candidate);
    table.rows.push(PreparedRow {
        start,
        len: table.token_count,
    });
    row_id(
        table
            .rows
            .len()
            .checked_sub(1)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::RowId,
            })?,
    )
}

fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    storage: PreparationStorage,
) -> Result<(), PreparationError> {
    let requested =
        values
            .len()
            .checked_add(additional)
            .ok_or(PreparationError::ArithmeticOverflow {
                calculation: PreparationArithmeticKind::CountConversion,
            })?;
    values
        .try_reserve(additional)
        .map_err(|_| PreparationError::AllocationFailure { storage, requested })
}

fn count_as_usize(value: u64) -> Result<usize, PreparationError> {
    usize::try_from(value).map_err(|_| PreparationError::ArithmeticOverflow {
        calculation: PreparationArithmeticKind::CountConversion,
    })
}

fn next_index(index: usize) -> Result<usize, PreparationError> {
    index
        .checked_add(1)
        .ok_or(PreparationError::ArithmeticOverflow {
            calculation: PreparationArithmeticKind::CountConversion,
        })
}

fn output_id(index: usize) -> Result<OutputId, PreparationError> {
    Ok(OutputId(u32::try_from(index).map_err(|_| {
        PreparationError::ArithmeticOverflow {
            calculation: PreparationArithmeticKind::OutputId,
        }
    })?))
}

fn row_id(index: usize) -> Result<RowId, PreparationError> {
    Ok(RowId(u32::try_from(index).map_err(|_| {
        PreparationError::ArithmeticOverflow {
            calculation: PreparationArithmeticKind::RowId,
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

    fn fixture() -> ValidatedGrammar {
        let mut classes = vec![0; 256];
        classes[usize::from(b'a')] = 1;
        classes[usize::from(b'b')] = 2;
        let class_count = 3;
        let mut transitions = vec![None; 3 * class_count];
        transitions[1] = Some(DfaStateId::new(1));
        transitions[class_count + 2] = Some(DfaStateId::new(0));
        let lexer = LexerDfa::new(
            3,
            u32::try_from(class_count).expect("fixture width fits"),
            classes,
            transitions,
            DfaStateId::new(0),
            vec![None, Some(TerminalId::new(0)), None],
        );
        let lalr = LalrTable::new(
            LalrDimensions::new(1, 2, 0),
            ParserStateId::new(0),
            TerminalId::new(1),
            vec![Action::Error, Action::Accept],
            Vec::new(),
            Vec::new(),
        );
        UnvalidatedGrammar::new(
            vec![
                TokenEntry::Bytes(b"a".to_vec()),
                TokenEntry::Bytes(b"aa".to_vec()),
                TokenEntry::Bytes(b"aa".to_vec()),
                TokenEntry::Eos,
                TokenEntry::Eos,
            ],
            lexer,
            lalr,
        )
        .validate(ValidationLimits::default())
        .expect("fixture validates")
    }

    #[test]
    fn prepared_table_matches_direct_relation_for_every_source_and_token_id() {
        let grammar = fixture();
        let table = prepare_token_table(&grammar, PreparationLimits::default())
            .expect("preparation succeeds");
        assert_eq!(
            table.source_count(),
            usize::try_from(grammar.lexer().state_count()).expect("fixture count fits") + 1
        );
        let mut row = 0;
        while row < table.source_count() {
            let source = table.source_for_row(row).expect("source row exists");
            let mut token_index = 0;
            while token_index < grammar.token_count() {
                let token = TokenId::new(token_index);
                let direct = execute_token(&grammar, source, token).expect("valid derived input");
                let prepared = table.cell(row, token).expect("prepared cell exists");
                assert!(
                    prepared_matches_direct(prepared, direct),
                    "row {row}, token {}",
                    token.get(),
                );
                token_index += 1;
            }
            row += 1;
        }
    }

    fn prepared_matches_direct(
        prepared: PreparedTokenExecution<'_>,
        direct: Option<TokenExecution>,
    ) -> bool {
        match (prepared, direct) {
            (PreparedTokenExecution::Rejected, None) => true,
            (
                PreparedTokenExecution::Continue { state, emitted },
                Some(TokenExecution::Continue {
                    state: direct_state,
                    emitted: direct_emitted,
                }),
            ) => state == direct_state && emitted == direct_emitted,
            (
                PreparedTokenExecution::Finished { emitted, eof },
                Some(TokenExecution::Finished {
                    emitted: direct_emitted,
                    eof: direct_eof,
                }),
            ) => emitted == direct_emitted && eof == direct_eof,
            _ => false,
        }
    }

    #[test]
    fn preparation_fails_before_trie_or_cell_budget_growth() {
        let grammar = fixture();
        let limits = PreparationLimits {
            max_trie_nodes: 0,
            ..PreparationLimits::default()
        };
        assert_eq!(
            prepare_token_table(&grammar, limits),
            Err(PreparationError::LimitExceeded {
                limit: PreparationLimitKind::TrieNodes,
                actual: 1,
                maximum: 0,
            })
        );
        let limits = PreparationLimits {
            max_source_token_cells: 1,
            ..PreparationLimits::default()
        };
        assert_eq!(
            prepare_token_table(&grammar, limits),
            Err(PreparationError::LimitExceeded {
                limit: PreparationLimitKind::SourceTokenCells,
                actual: 20,
                maximum: 1,
            })
        );
    }

    #[test]
    fn preparation_limits_cover_bytes_outputs_and_interning_work() {
        let grammar = fixture();
        let limits = PreparationLimits {
            max_logical_token_bytes: 0,
            ..PreparationLimits::default()
        };
        assert_eq!(
            prepare_token_table(&grammar, limits),
            Err(PreparationError::LimitExceeded {
                limit: PreparationLimitKind::LogicalTokenBytes,
                actual: 1,
                maximum: 0,
            })
        );
        let limits = PreparationLimits {
            max_output_pool_terminals: 0,
            ..PreparationLimits::default()
        };
        assert_eq!(
            prepare_token_table(&grammar, limits),
            Err(PreparationError::LimitExceeded {
                limit: PreparationLimitKind::OutputPoolTerminals,
                actual: 1,
                maximum: 0,
            })
        );
        let limits = PreparationLimits {
            max_output_interning_work: 0,
            ..PreparationLimits::default()
        };
        assert_eq!(
            prepare_token_table(&grammar, limits),
            Err(PreparationError::LimitExceeded {
                limit: PreparationLimitKind::OutputInterningWork,
                actual: 1,
                maximum: 0,
            })
        );
        let limits = PreparationLimits {
            max_row_interning_work: 0,
            ..PreparationLimits::default()
        };
        assert_eq!(
            prepare_token_table(&grammar, limits),
            Err(PreparationError::LimitExceeded {
                limit: PreparationLimitKind::RowInterningWork,
                actual: 1,
                maximum: 0,
            })
        );
    }
}
