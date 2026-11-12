//! Python entry points for grammar compilation and tokenizer-byte extraction.

use greatgramma::{
    CompileLimits, SourceGrammar, TerminalSpec, TokenSpec, TokenizerManifest, compile,
};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyList, PyString},
};

use super::{
    boundary::{BoundaryLimits, preflight_compile_inputs, preflight_json_input},
    error::{NativeFailure, detached, fallible_py_bytes},
    matcher::NativeGrammar,
};

pub(super) fn compile_inner(
    yacc: String,
    terminals: Vec<(String, String, u32)>,
    tokens: Vec<Option<Vec<u8>>>,
    ignored: Vec<String>,
) -> Result<NativeGrammar, NativeFailure> {
    let max_vocab_size = BoundaryLimits::default().max_vocab_size;
    if tokens.len() > max_vocab_size {
        return Err(NativeFailure::new(
            "tokenizer_compatibility",
            format!("the vocabulary exceeds {max_vocab_size} entries"),
        ));
    }
    let vocab_size = u32::try_from(tokens.len()).map_err(|_| {
        NativeFailure::new(
            "tokenizer_compatibility",
            "the vocabulary is larger than the supported token-ID domain",
        )
    })?;
    let mut terminal_specs = Vec::new();
    terminal_specs
        .try_reserve_exact(terminals.len())
        .map_err(|_| NativeFailure::internal("could not reserve terminal specifications"))?;
    for (name, pattern, priority) in terminals {
        terminal_specs.push(TerminalSpec::new(name, pattern, priority));
    }
    let mut token_specs = Vec::new();
    token_specs
        .try_reserve_exact(tokens.len())
        .map_err(|_| NativeFailure::internal("could not reserve token specifications"))?;
    for token in tokens {
        match token {
            Some(bytes) => token_specs.push(TokenSpec::Bytes(bytes)),
            None => token_specs.push(TokenSpec::Eos),
        }
    }
    let source = SourceGrammar::new(yacc, terminal_specs).with_ignored_terminals(ignored);
    let prepared = compile(
        source,
        TokenizerManifest::new(token_specs),
        CompileLimits::default(),
    )
    .map_err(|error| NativeFailure::new("compile_error", error.to_string()))?;
    Ok(NativeGrammar::from_prepared(prepared, vocab_size))
}

fn token_bytes_from_json_inner(
    json: String,
    eos_token_ids: Vec<u32>,
) -> Result<Vec<Vec<u8>>, NativeFailure> {
    let (manifest, _) = TokenizerManifest::from_tokenizer_json(&json, &eos_token_ids)
        .map_err(|error| NativeFailure::new("tokenizer_compatibility", error.to_string()))?;
    Ok(manifest
        .into_tokens()
        .into_iter()
        .map(|token| match token {
            TokenSpec::Bytes(bytes) => bytes,
            TokenSpec::Eos => Vec::new(),
        })
        .collect())
}

#[pyfunction]
pub(super) fn _compile_yacc(
    py: Python<'_>,
    yacc: &Bound<'_, PyString>,
    terminals: &Bound<'_, PyList>,
    tokens: &Bound<'_, PyList>,
    ignored: &Bound<'_, PyList>,
) -> PyResult<NativeGrammar> {
    preflight_compile_inputs(yacc, terminals, tokens, ignored, BoundaryLimits::default())?;
    let yacc = yacc.extract::<String>()?;
    let terminals = terminals.extract::<Vec<(String, String, u32)>>()?;
    let tokens = tokens.extract::<Vec<Option<Vec<u8>>>>()?;
    let ignored = ignored.extract::<Vec<String>>()?;
    detached(py, move || compile_inner(yacc, terminals, tokens, ignored))
        .map_err(NativeFailure::into_pyerr)
}

#[pyfunction]
pub(super) fn _token_bytes_from_json(
    py: Python<'_>,
    json: &Bound<'_, PyString>,
    eos_token_ids: &Bound<'_, PyList>,
) -> PyResult<Vec<Py<PyBytes>>> {
    preflight_json_input(json, eos_token_ids, BoundaryLimits::default())?;
    let json = json.extract::<String>()?;
    let eos_token_ids = eos_token_ids.extract::<Vec<u32>>()?;
    let tokens = detached(py, move || token_bytes_from_json_inner(json, eos_token_ids))
        .map_err(NativeFailure::into_pyerr)?;
    tokens
        .iter()
        .map(|bytes| fallible_py_bytes(py, bytes))
        .collect()
}
