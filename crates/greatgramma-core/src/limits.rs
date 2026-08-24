/// Finite resource limits applied before normalized tables become trusted input.
///
/// Counts and cell budgets are logical limits, independent of platform object
/// layout. `max_logical_bytes` charges one byte per token tag, every raw token
/// byte, four bytes per ID/count/class cell, eight bytes per action/production,
/// and all fixed table scalars. `max_work` bounds the logical entries and bytes
/// inspected during validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationLimits {
    pub max_tokens: u64,
    pub max_token_bytes: u64,
    pub max_dfa_states: u64,
    pub max_dfa_classes: u64,
    pub max_dfa_cells: u64,
    pub max_parser_states: u64,
    pub max_terminals: u64,
    pub max_nonterminals: u64,
    pub max_productions: u64,
    pub max_parser_cells: u64,
    pub max_logical_bytes: u64,
    pub max_work: u64,
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            max_tokens: 1_000_000,
            max_token_bytes: 1 << 30,
            max_dfa_states: 1_000_000,
            max_dfa_classes: 256,
            max_dfa_cells: 64_000_000,
            max_parser_states: 1_000_000,
            max_terminals: 65_536,
            max_nonterminals: 65_536,
            max_productions: 1_000_000,
            max_parser_cells: 64_000_000,
            max_logical_bytes: 4 << 30,
            max_work: 1_000_000_000,
        }
    }
}

/// Finite resource limits for deterministic token-step preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparationLimits {
    pub max_trie_nodes: u64,
    pub max_trie_edges: u64,
    pub max_logical_token_bytes: u64,
    pub max_source_token_cells: u64,
    pub max_output_pool_terminals: u64,
    pub max_row_interning_work: u64,
    pub max_output_interning_work: u64,
    pub max_work: u64,
}

impl Default for PreparationLimits {
    fn default() -> Self {
        Self {
            max_trie_nodes: 1_000_000,
            max_trie_edges: 1_000_000,
            max_logical_token_bytes: 1 << 30,
            max_source_token_cells: 64_000_000,
            max_output_pool_terminals: 64_000_000,
            max_row_interning_work: 1_000_000_000,
            max_output_interning_work: 1_000_000_000,
            max_work: 1_000_000_000,
        }
    }
}
