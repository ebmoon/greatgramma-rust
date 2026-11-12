//! Borrowed Python input validation before allocating owned compiler inputs.

use greatgramma::{CompileLimits, TokenizerJsonLimits};
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyBytes, PyList, PyString, PyTuple},
};

use super::error::NativeFailure;

#[derive(Clone, Copy)]
pub(super) struct BoundaryLimits {
    max_json_bytes: usize,
    pub(super) max_vocab_size: usize,
    max_token_bytes: usize,
    max_source_bytes: usize,
    max_terminal_specs: usize,
    max_ignored_terminals: usize,
    max_regex_bytes: usize,
}

impl Default for BoundaryLimits {
    fn default() -> Self {
        let compile = CompileLimits::default();
        let tokenizer_json = TokenizerJsonLimits::default();
        Self {
            max_json_bytes: tokenizer_json.max_json_bytes,
            max_vocab_size: tokenizer_json.max_vocab_size,
            max_token_bytes: usize::try_from(compile.validation.max_token_bytes)
                .unwrap_or(usize::MAX),
            max_source_bytes: compile.max_source_bytes,
            max_terminal_specs: compile.max_terminal_specs,
            max_ignored_terminals: compile.max_ignored_terminals,
            max_regex_bytes: compile.max_regex_bytes,
        }
    }
}

fn check_json_input_lengths(
    json_bytes: usize,
    eos_count: usize,
    limits: BoundaryLimits,
) -> Result<(), NativeFailure> {
    if json_bytes > limits.max_json_bytes {
        return Err(NativeFailure::new(
            "tokenizer_compatibility",
            format!(
                "tokenizer JSON has {json_bytes} bytes, maximum is {}",
                limits.max_json_bytes
            ),
        ));
    }
    if eos_count > limits.max_vocab_size {
        return Err(NativeFailure::new(
            "tokenizer_compatibility",
            format!(
                "EOS token IDs contain {eos_count} entries, maximum is {}",
                limits.max_vocab_size
            ),
        ));
    }
    Ok(())
}

fn check_compile_input_lengths(
    source_bytes: usize,
    terminal_count: usize,
    ignored_count: usize,
    token_count: usize,
    token_bytes: usize,
    regex_bytes: usize,
    limits: BoundaryLimits,
) -> Result<(), NativeFailure> {
    if source_bytes > limits.max_source_bytes {
        return Err(NativeFailure::new(
            "compile_error",
            format!(
                "grammar source has {source_bytes} bytes, maximum is {}",
                limits.max_source_bytes
            ),
        ));
    }
    if terminal_count > limits.max_terminal_specs {
        return Err(NativeFailure::new(
            "compile_error",
            format!(
                "grammar has {terminal_count} terminal specifications, maximum is {}",
                limits.max_terminal_specs
            ),
        ));
    }
    if ignored_count > limits.max_ignored_terminals {
        return Err(NativeFailure::new(
            "compile_error",
            format!(
                "grammar has {ignored_count} ignored terminals, maximum is {}",
                limits.max_ignored_terminals
            ),
        ));
    }
    if regex_bytes > limits.max_regex_bytes {
        return Err(NativeFailure::new(
            "compile_error",
            format!(
                "terminal regexes have {regex_bytes} source bytes, maximum is {}",
                limits.max_regex_bytes
            ),
        ));
    }
    if token_count > limits.max_vocab_size {
        return Err(NativeFailure::new(
            "tokenizer_compatibility",
            format!(
                "the vocabulary has {token_count} entries, maximum is {}",
                limits.max_vocab_size
            ),
        ));
    }
    if token_bytes > limits.max_token_bytes {
        return Err(NativeFailure::new(
            "tokenizer_compatibility",
            format!(
                "ordinary token bytes contain {token_bytes} bytes, maximum is {}",
                limits.max_token_bytes
            ),
        ));
    }
    Ok(())
}

pub(super) fn preflight_json_input(
    json: &Bound<'_, PyString>,
    eos_token_ids: &Bound<'_, PyList>,
    limits: BoundaryLimits,
) -> PyResult<()> {
    let json_bytes = json.to_cow()?.len();
    check_json_input_lengths(json_bytes, eos_token_ids.len(), limits)
        .map_err(NativeFailure::into_pyerr)
}

pub(super) fn preflight_compile_inputs(
    yacc: &Bound<'_, PyString>,
    terminals: &Bound<'_, PyList>,
    tokens: &Bound<'_, PyList>,
    ignored: &Bound<'_, PyList>,
    limits: BoundaryLimits,
) -> PyResult<()> {
    let terminal_count = terminals.len();
    let ignored_count = ignored.len();
    let token_count = tokens.len();
    let check = |source_bytes, token_bytes, regex_bytes| {
        check_compile_input_lengths(
            source_bytes,
            terminal_count,
            ignored_count,
            token_count,
            token_bytes,
            regex_bytes,
            limits,
        )
        .map_err(NativeFailure::into_pyerr)
    };
    check(0, 0, 0)?;

    let mut source_bytes = yacc.to_cow()?.len();
    let mut regex_bytes = 0_usize;
    check(source_bytes, 0, regex_bytes)?;

    for terminal in terminals.iter() {
        let terminal = terminal.cast::<PyTuple>()?;
        if terminal.len() != 3 {
            return Err(PyTypeError::new_err(
                "terminal specifications must be three-item tuples",
            ));
        }
        let name = terminal.get_item(0)?.cast_into::<PyString>()?;
        let pattern = terminal.get_item(1)?.cast_into::<PyString>()?;
        let name_bytes = name.to_cow()?.len();
        let pattern_bytes = pattern.to_cow()?.len();
        source_bytes = source_bytes
            .saturating_add(name_bytes)
            .saturating_add(pattern_bytes);
        regex_bytes = regex_bytes.saturating_add(pattern_bytes);
        check(source_bytes, 0, regex_bytes)?;
    }

    for name in ignored.iter() {
        let name = name.cast_into::<PyString>()?;
        source_bytes = source_bytes.saturating_add(name.to_cow()?.len());
        check(source_bytes, 0, regex_bytes)?;
    }

    let mut token_bytes = 0_usize;
    for token in tokens.iter() {
        if !token.is_none() {
            token_bytes =
                token_bytes.saturating_add(token.cast_into::<PyBytes>()?.as_bytes().len());
            check(source_bytes, token_bytes, regex_bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_boundary_limits() -> BoundaryLimits {
        BoundaryLimits {
            max_json_bytes: 4,
            max_vocab_size: 2,
            max_token_bytes: 5,
            max_source_bytes: 6,
            max_terminal_specs: 2,
            max_ignored_terminals: 2,
            max_regex_bytes: 3,
        }
    }

    fn native_code(py: Python<'_>, error: &PyErr) -> String {
        error
            .value(py)
            .getattr("args")
            .expect("native error has args")
            .get_item(0)
            .expect("native error has a code")
            .extract::<String>()
            .expect("native error code is a string")
    }

    #[test]
    fn boundary_lengths_accept_the_exact_limits() {
        let limits = small_boundary_limits();
        check_json_input_lengths(4, 2, limits).expect("exact JSON limits are accepted");
        check_compile_input_lengths(6, 2, 2, 2, 5, 3, limits)
            .expect("exact compiler limits are accepted");
    }

    #[test]
    fn json_boundary_lengths_fail_with_tokenizer_code() {
        let limits = small_boundary_limits();
        assert_eq!(
            check_json_input_lengths(5, 2, limits).unwrap_err().code,
            "tokenizer_compatibility"
        );
        assert_eq!(
            check_json_input_lengths(4, 3, limits).unwrap_err().code,
            "tokenizer_compatibility"
        );
    }

    #[test]
    fn compiler_boundary_lengths_preserve_source_and_tokenizer_codes() {
        let limits = small_boundary_limits();
        assert_eq!(
            check_compile_input_lengths(7, 2, 2, 2, 5, 3, limits)
                .unwrap_err()
                .code,
            "compile_error"
        );
        assert_eq!(
            check_compile_input_lengths(6, 2, 2, 3, 5, 3, limits)
                .unwrap_err()
                .code,
            "tokenizer_compatibility"
        );
        assert_eq!(
            check_compile_input_lengths(6, 2, 2, 2, 6, 3, limits)
                .unwrap_err()
                .code,
            "tokenizer_compatibility"
        );
        assert_eq!(
            check_compile_input_lengths(6, 2, 2, 2, 5, 4, limits)
                .unwrap_err()
                .code,
            "compile_error"
        );
    }

    #[test]
    fn borrowed_json_preflight_counts_utf8_before_owned_extraction() {
        Python::initialize();
        Python::attach(|py| {
            let json = PyString::new(py, "ééé");
            let eos = PyList::new(py, [0_u32]).expect("EOS fixture");
            let error = preflight_json_input(&json, &eos, small_boundary_limits())
                .expect_err("six UTF-8 bytes exceed the four-byte test limit");
            assert_eq!(native_code(py, &error), "tokenizer_compatibility");
        });
    }

    #[test]
    fn borrowed_compile_preflight_counts_token_bytes_before_owned_extraction() {
        Python::initialize();
        Python::attach(|py| {
            let yacc = PyString::new(py, "S: A");
            let terminals = PyList::empty(py);
            let tokens = PyList::empty(py);
            tokens
                .append(PyBytes::new(py, b"aaa"))
                .expect("first token");
            tokens
                .append(PyBytes::new(py, b"aaa"))
                .expect("second token");
            let ignored = PyList::empty(py);

            let error = preflight_compile_inputs(
                &yacc,
                &terminals,
                &tokens,
                &ignored,
                small_boundary_limits(),
            )
            .expect_err("six token bytes exceed the five-byte test limit");
            assert_eq!(native_code(py, &error), "tokenizer_compatibility");
        });
    }
}
