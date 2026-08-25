use greatgramma_core::{
    PreparationLimits, PreparedGrammar, UnvalidatedGrammar, ValidationLimits, prepare,
};

use crate::{
    CompileError, SourceGrammar, TokenizerManifest, dfa::build_product_dfa, grammar::parse_source,
    lr::lower_lalr, regex::compile_regexes, tokenizer::normalize_manifest,
};

/// Resource limits for compiler-owned tables and core validation/preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileLimits {
    pub validation: ValidationLimits,
    pub preparation: PreparationLimits,
    /// Total bytes in the Yacc text, terminal names and patterns, and ignored names.
    pub max_source_bytes: usize,
    pub max_terminal_specs: usize,
    pub max_ignored_terminals: usize,
    /// Total pattern bytes across all terminal specifications.
    pub max_regex_bytes: usize,
    /// Total memory reported by the compiled `derivre` regexes.
    pub max_regex_output_bytes: usize,
    pub max_dfa_states: usize,
    /// Shared `derivre` work budget across every terminal, not a per-regex budget.
    pub max_regex_fuel: u64,
}

impl Default for CompileLimits {
    fn default() -> Self {
        Self {
            validation: ValidationLimits::default(),
            preparation: PreparationLimits::default(),
            max_source_bytes: 64 * 1024 * 1024,
            max_terminal_specs: 65_536,
            max_ignored_terminals: 65_536,
            max_regex_bytes: 16 * 1024 * 1024,
            max_regex_output_bytes: 256 * 1024 * 1024,
            max_dfa_states: 100_000,
            max_regex_fuel: 1_000_000,
        }
    }
}

/// Compiles owned source inputs into the core's untrusted normalized schema.
pub fn normalize(
    source: SourceGrammar,
    tokenizer: TokenizerManifest,
    limits: CompileLimits,
) -> Result<UnvalidatedGrammar, CompileError> {
    check_source_limits(&source, limits)?;
    let tokens = normalize_manifest(tokenizer, limits.validation)?;
    let parsed = parse_source(source, limits.max_terminal_specs)?;
    let lalr = lower_lalr(&parsed, limits.validation)?;
    let lexer = build_product_dfa(
        compile_regexes(
            parsed.terminals,
            limits.max_regex_fuel,
            limits.max_regex_output_bytes,
        )?,
        limits.max_dfa_states,
        limits.validation.max_dfa_cells,
        limits.validation.max_work,
    )?;
    Ok(UnvalidatedGrammar::new(tokens, lexer, lalr))
}

/// Normalizes, structurally validates, and prepares a matcher in one fail-closed call.
pub fn compile(
    source: SourceGrammar,
    tokenizer: TokenizerManifest,
    limits: CompileLimits,
) -> Result<PreparedGrammar, CompileError> {
    let grammar = normalize(source, tokenizer, limits)?
        .validate(limits.validation)
        .map_err(CompileError::Validation)?;
    prepare(grammar, limits.preparation).map_err(CompileError::Preparation)
}

fn check_source_limits(source: &SourceGrammar, limits: CompileLimits) -> Result<(), CompileError> {
    if source.terminals.len() > limits.max_terminal_specs {
        return Err(CompileError::TooManyTerminalSpecs {
            count: source.terminals.len(),
            maximum: limits.max_terminal_specs,
        });
    }
    if source.ignored_terminals.len() > limits.max_ignored_terminals {
        return Err(CompileError::TooManyIgnoredTerminals {
            count: source.ignored_terminals.len(),
            maximum: limits.max_ignored_terminals,
        });
    }

    let mut source_bytes = source.yacc.len();
    let mut regex_bytes = 0_usize;
    for terminal in &source.terminals {
        source_bytes = source_bytes
            .saturating_add(terminal.name.len())
            .saturating_add(terminal.pattern.len());
        regex_bytes = regex_bytes.saturating_add(terminal.pattern.len());
    }
    for ignored in &source.ignored_terminals {
        source_bytes = source_bytes.saturating_add(ignored.len());
    }
    if source_bytes > limits.max_source_bytes {
        return Err(CompileError::SourceTooLarge {
            bytes: source_bytes,
            maximum: limits.max_source_bytes,
        });
    }
    if regex_bytes > limits.max_regex_bytes {
        return Err(CompileError::RegexSourceTooLarge {
            bytes: regex_bytes,
            maximum: limits.max_regex_bytes,
        });
    }
    Ok(())
}
