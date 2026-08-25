//! Private PyO3 boundary for the typed `greatgramma` Python package.

#![forbid(unsafe_code)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use greatgramma_compile::{
    CompileLimits, SourceGrammar, TerminalSpec, TokenSpec, TokenizerJsonLimits, TokenizerManifest,
    compile,
};
use greatgramma_core::{EngineError, Matcher, PreparedGrammar, TokenId};
use pyo3::{
    create_exception,
    exceptions::{PyException, PyTypeError},
    prelude::*,
    types::{PyBytes, PyList, PyModule, PyString, PyTuple},
};

create_exception!(_native, NativeError, PyException);

const MAX_BATCH_ROWS: usize = 4096;
const MAX_PACKED_MASK_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy)]
struct BoundaryLimits {
    max_json_bytes: usize,
    max_vocab_size: usize,
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

#[derive(Debug)]
struct NativeFailure {
    code: &'static str,
    message: String,
    row: Option<usize>,
    token: Option<u32>,
}

impl NativeFailure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            row: None,
            token: None,
        }
    }

    fn at_row(mut self, row: usize) -> Self {
        self.row = Some(row);
        self
    }

    fn for_token(mut self, token: u32) -> Self {
        self.token = Some(token);
        self
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new("internal_error", message)
    }

    fn into_pyerr(self) -> PyErr {
        NativeError::new_err((self.code, self.message, self.row, self.token))
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

fn preflight_json_input(
    json: &Bound<'_, PyString>,
    eos_token_ids: &Bound<'_, PyList>,
    limits: BoundaryLimits,
) -> PyResult<()> {
    let json_bytes = json.to_cow()?.len();
    check_json_input_lengths(json_bytes, eos_token_ids.len(), limits)
        .map_err(NativeFailure::into_pyerr)
}

fn preflight_compile_inputs(
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

fn detached<T, F>(py: Python<'_>, operation: F) -> Result<T, NativeFailure>
where
    T: Send,
    F: FnOnce() -> Result<T, NativeFailure> + Send,
{
    match py.detach(|| catch_unwind(AssertUnwindSafe(operation))) {
        Ok(result) => result,
        Err(_) => Err(NativeFailure::internal(
            "native operation panicked and was stopped at the ABI boundary",
        )),
    }
}

#[pyclass(module = "greatgramma._native")]
struct NativeGrammar {
    prepared: Option<PreparedGrammar>,
    vocab_size: u32,
}

impl NativeGrammar {
    fn new_batch_inner(
        &mut self,
        rows: usize,
        pad_token_id: Option<u32>,
    ) -> Result<NativeBatch, NativeFailure> {
        if rows == 0 {
            return Err(NativeFailure::new(
                "configuration_error",
                "the generation batch must contain at least one row",
            ));
        }
        if rows > MAX_BATCH_ROWS {
            return Err(NativeFailure::new(
                "configuration_error",
                format!("the generation batch exceeds {MAX_BATCH_ROWS} rows"),
            ));
        }
        if rows > 1 && pad_token_id.is_none() {
            return Err(NativeFailure::new(
                "configuration_error",
                "batches larger than one require a pad token ID",
            ));
        }
        if let Some(pad) = pad_token_id
            && pad >= self.vocab_size
        {
            return Err(NativeFailure::new(
                "configuration_error",
                "the pad token ID is outside the compiled vocabulary",
            )
            .for_token(pad));
        }
        let packed_bytes = rows
            .checked_mul(
                self.prepared
                    .as_ref()
                    .ok_or_else(|| {
                        NativeFailure::new(
                            "sequence_discontinuity",
                            "a compiled grammar can create only one active generation session",
                        )
                    })?
                    .mask_bytes(),
            )
            .ok_or_else(|| {
                NativeFailure::new("configuration_error", "packed mask size overflow")
            })?;
        if packed_bytes > MAX_PACKED_MASK_BYTES {
            return Err(NativeFailure::new(
                "configuration_error",
                format!(
                    "packed batch mask needs {packed_bytes} bytes, maximum is {MAX_PACKED_MASK_BYTES}"
                ),
            ));
        }
        let mut active_tokens = Vec::new();
        active_tokens
            .try_reserve_exact(rows)
            .map_err(|_| NativeFailure::internal("could not reserve native batch tokens"))?;
        active_tokens.resize(rows, None);
        let mask_buffer = zeroed_bytes(packed_bytes)?;
        let prepared = self.prepared.take().ok_or_else(|| {
            NativeFailure::new(
                "sequence_discontinuity",
                "a compiled grammar can create only one active generation session",
            )
        })?;
        let matcher = prepared.into_matcher(rows).map_err(map_engine_error)?;
        Ok(NativeBatch {
            matcher,
            pad_token_id,
            active_tokens,
            mask_buffer,
            poisoned: false,
        })
    }
}

#[pymethods]
impl NativeGrammar {
    #[getter]
    fn vocab_size(&self, py: Python<'_>) -> PyResult<u32> {
        detached(py, || Ok(self.vocab_size)).map_err(NativeFailure::into_pyerr)
    }

    fn new_batch(
        &mut self,
        py: Python<'_>,
        rows: usize,
        pad_token_id: Option<u32>,
    ) -> PyResult<NativeBatch> {
        detached(py, || self.new_batch_inner(rows, pad_token_id)).map_err(NativeFailure::into_pyerr)
    }
}

#[pyclass(module = "greatgramma._native")]
struct NativeBatch {
    matcher: Matcher,
    pad_token_id: Option<u32>,
    active_tokens: Vec<Option<TokenId>>,
    mask_buffer: Vec<u8>,
    poisoned: bool,
}

impl NativeBatch {
    fn ensure_live(&self) -> Result<(), NativeFailure> {
        if self.poisoned {
            return Err(NativeFailure::internal(
                "the native batch is poisoned after an earlier internal failure",
            ));
        }
        Ok(())
    }

    fn initial_masks_inner(&mut self) -> Result<(), NativeFailure> {
        self.ensure_live()?;
        self.matcher
            .masks(&mut self.mask_buffer)
            .map_err(map_engine_error)
    }

    fn advance_and_masks_inner(&mut self, tokens: &[u32]) -> Result<(), NativeFailure> {
        self.ensure_live()?;
        if tokens.len() != self.matcher.row_count() {
            return Err(NativeFailure::new(
                "sequence_discontinuity",
                format!(
                    "expected {} fixed rows, received {}",
                    self.matcher.row_count(),
                    tokens.len()
                ),
            ));
        }

        let mut row = 0_usize;
        while row < tokens.len() {
            let token = *tokens
                .get(row)
                .ok_or_else(|| NativeFailure::internal("missing copied token"))?;
            if token >= self.matcher.token_count() {
                return Err(NativeFailure::new(
                    "constraint_violation",
                    "token ID is outside the compiled vocabulary",
                )
                .at_row(row)
                .for_token(token));
            }
            if self
                .matcher
                .is_completed(row)
                .map_err(|error| map_engine_error(error).at_row(row))?
            {
                if self.pad_token_id == Some(token) {
                    *self.active_tokens.get_mut(row).ok_or_else(|| {
                        NativeFailure::internal("missing native batch token slot")
                    })? = None;
                } else {
                    return Err(NativeFailure::new(
                        "sequence_completed",
                        "an accepted row permits only its configured pad token",
                    )
                    .at_row(row)
                    .for_token(token));
                }
            } else {
                *self
                    .active_tokens
                    .get_mut(row)
                    .ok_or_else(|| NativeFailure::internal("missing native batch token slot"))? =
                    Some(TokenId::new(token));
            }
            row += 1;
        }

        let width = self.matcher.mask_bytes();
        let length = self
            .matcher
            .row_count()
            .checked_mul(width)
            .ok_or_else(|| NativeFailure::internal("packed mask size overflow"))?;
        if self.mask_buffer.len() != length {
            return Err(NativeFailure::internal(
                "native mask buffer has an invalid fixed size",
            ));
        }
        self.matcher
            .advance_active_and_masks_no_result(&self.active_tokens, &mut self.mask_buffer)
            .map_err(map_engine_error)?;

        let mut running = 0_usize;
        let mut row = 0_usize;
        while row < self.matcher.row_count() {
            if self
                .matcher
                .is_completed(row)
                .map_err(|error| map_engine_error(error).at_row(row))?
            {
                let pad = self.pad_token_id.ok_or_else(|| {
                    NativeFailure::new(
                        "sequence_completed",
                        "an accepted row has no configured pad token",
                    )
                    .at_row(row)
                })?;
                let start = row
                    .checked_mul(width)
                    .ok_or_else(|| NativeFailure::internal("packed mask offset overflow"))?;
                let end = start
                    .checked_add(width)
                    .ok_or_else(|| NativeFailure::internal("packed mask offset overflow"))?;
                let row_output = self
                    .mask_buffer
                    .get_mut(start..end)
                    .ok_or_else(|| NativeFailure::internal("invalid packed mask row"))?;
                set_packed_bit(row_output, pad).map_err(|error| error.at_row(row))?;
            } else {
                running += 1;
            }
            row += 1;
        }
        if running == 0 {
            return Err(NativeFailure::new(
                "sequence_completed",
                "every generation row has already accepted EOS",
            ));
        }
        Ok(())
    }
}

#[pymethods]
impl NativeBatch {
    #[getter]
    fn mask_bytes(&self, py: Python<'_>) -> PyResult<usize> {
        detached(py, || Ok(self.matcher.mask_bytes())).map_err(NativeFailure::into_pyerr)
    }

    fn initial_masks(&mut self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let result = detached(py, || self.initial_masks_inner());
        finish_batch_call(self, py, result)
    }

    fn advance_and_masks(&mut self, py: Python<'_>, tokens: Vec<u32>) -> PyResult<Py<PyBytes>> {
        let result = detached(py, || self.advance_and_masks_inner(&tokens));
        finish_batch_call(self, py, result)
    }
}

fn finish_batch_call(
    batch: &mut NativeBatch,
    py: Python<'_>,
    result: Result<(), NativeFailure>,
) -> PyResult<Py<PyBytes>> {
    match result {
        Ok(()) => match fallible_py_bytes(py, &batch.mask_buffer) {
            Ok(output) => Ok(output),
            Err(error) => {
                batch.poisoned = true;
                Err(error)
            }
        },
        Err(error) => {
            if error.code == "internal_error" {
                batch.poisoned = true;
            }
            Err(error.into_pyerr())
        }
    }
}

fn fallible_py_bytes(py: Python<'_>, bytes: &[u8]) -> PyResult<Py<PyBytes>> {
    PyBytes::new_with(py, bytes.len(), |output| {
        output.copy_from_slice(bytes);
        Ok(())
    })
    .map(Bound::unbind)
    .map_err(|_| NativeFailure::internal("could not allocate Python mask bytes").into_pyerr())
}

fn set_packed_bit(output: &mut [u8], token: u32) -> Result<(), NativeFailure> {
    let token = usize::try_from(token)
        .map_err(|_| NativeFailure::internal("token index conversion failed"))?;
    let byte = output
        .get_mut(token / 8)
        .ok_or_else(|| NativeFailure::internal("pad token mask index is invalid"))?;
    *byte |= 1_u8 << (token % 8);
    Ok(())
}

fn zeroed_bytes(length: usize) -> Result<Vec<u8>, NativeFailure> {
    if length > MAX_PACKED_MASK_BYTES {
        return Err(NativeFailure::new(
            "configuration_error",
            format!("packed batch mask needs {length} bytes, maximum is {MAX_PACKED_MASK_BYTES}"),
        ));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| NativeFailure::internal("could not reserve the packed batch mask"))?;
    output.resize(length, 0);
    Ok(output)
}

fn map_engine_error(error: EngineError) -> NativeFailure {
    match error {
        EngineError::BatchConstraintViolation { row, token } => {
            NativeFailure::new("constraint_violation", "token is rejected by the grammar")
                .at_row(row)
                .for_token(token.get())
        }
        EngineError::InvalidToken { token } | EngineError::ConstraintViolation { token } => {
            NativeFailure::new(
                "constraint_violation",
                format!("core rejected token {token:?}"),
            )
            .for_token(token.get())
        }
        EngineError::Completed => {
            NativeFailure::new("sequence_completed", "the grammar has already accepted EOS")
        }
        EngineError::InvalidRow { row, rows } => NativeFailure::internal(format!(
            "core row {row} is outside a matcher with {rows} rows"
        )),
        EngineError::EmptyBatch => {
            NativeFailure::new("configuration_error", "the matcher batch is empty")
        }
        EngineError::BatchSizeMismatch { expected, actual } => NativeFailure::new(
            "sequence_discontinuity",
            format!("expected {expected} rows, received {actual}"),
        ),
        EngineError::MissingRunningToken { row } => {
            NativeFailure::internal(format!("native batch omitted running row {row}"))
        }
        EngineError::BufferTooSmall { required, actual } => NativeFailure::internal(format!(
            "native mask buffer has {actual} bytes but core requires {required}"
        )),
        EngineError::NoValidToken => {
            NativeFailure::new("no_valid_token", "the grammar has no legal next token")
        }
        EngineError::AllocationFailure { requested } => {
            NativeFailure::internal(format!("native allocation failed for {requested} items"))
        }
        EngineError::CorruptPreparedData => {
            NativeFailure::internal("core reported corrupt prepared data")
        }
    }
}

fn compile_inner(
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
    Ok(NativeGrammar {
        prepared: Some(prepared),
        vocab_size,
    })
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
fn _compile_yacc(
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
fn _token_bytes_from_json(
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

#[cfg(feature = "panic-test-hook")]
#[pyfunction]
fn _test_panic(py: Python<'_>) -> PyResult<()> {
    detached(py, || -> Result<(), NativeFailure> {
        panic!("intentional ABI panic test")
    })
    .map_err(NativeFailure::into_pyerr)
}

#[pymodule]
fn _native(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("NativeError", py.get_type::<NativeError>())?;
    module.add_class::<NativeGrammar>()?;
    module.add_class::<NativeBatch>()?;
    module.add_function(wrap_pyfunction!(_compile_yacc, module)?)?;
    module.add_function(wrap_pyfunction!(_token_bytes_from_json, module)?)?;
    #[cfg(feature = "panic-test-hook")]
    module.add_function(wrap_pyfunction!(_test_panic, module)?)?;
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

    fn grammar() -> NativeGrammar {
        compile_inner(
            "%start S\n%token ITEM CLOSE\n%%\nS: ITEM CLOSE;\n".to_owned(),
            vec![
                ("ITEM".to_owned(), "i".to_owned(), 0),
                ("CLOSE".to_owned(), "c".to_owned(), 0),
            ],
            vec![
                Some(b"i".to_vec()),
                Some(b"c".to_vec()),
                Some(b"ic".to_vec()),
                None,
                None,
            ],
            Vec::new(),
        )
        .expect("native fixture compiles")
    }

    #[test]
    fn batch_masks_are_packed_and_advances_are_transactional() {
        let mut grammar = grammar();
        let mut batch = grammar.new_batch_inner(2, Some(0)).expect("batch starts");
        batch.initial_masks_inner().unwrap();
        assert_eq!(batch.mask_buffer, [0b0000_0101; 2]);

        let rejected = batch.advance_and_masks_inner(&[2, 1]);
        let rejected = rejected.unwrap_err();
        assert_eq!(rejected.code, "constraint_violation");
        assert_eq!(rejected.row, Some(1));
        assert_eq!(rejected.token, Some(1));
        batch.initial_masks_inner().unwrap();
        assert_eq!(batch.mask_buffer, [0b0000_0101; 2]);

        batch.advance_and_masks_inner(&[2, 0]).unwrap();
        assert_eq!(batch.mask_buffer, [0b0001_1000, 0b0000_0010]);
    }

    #[test]
    fn native_batch_reuses_its_hot_path_buffers() {
        let mut grammar = grammar();
        let mut batch = grammar.new_batch_inner(2, Some(0)).expect("batch starts");
        let active = batch.active_tokens.as_ptr();
        let masks = batch.mask_buffer.as_ptr();

        batch.initial_masks_inner().expect("initial masks");
        batch
            .advance_and_masks_inner(&[2, 0])
            .expect("valid advancement");

        assert_eq!(batch.active_tokens.as_ptr(), active);
        assert_eq!(batch.mask_buffer.as_ptr(), masks);
    }

    #[test]
    fn finished_rows_accept_only_pad_while_other_rows_continue() {
        let mut grammar = grammar();
        let mut batch = grammar.new_batch_inner(2, Some(0)).expect("batch starts");
        batch.advance_and_masks_inner(&[2, 0]).unwrap();
        batch.advance_and_masks_inner(&[3, 1]).unwrap();
        assert_eq!(batch.mask_buffer, [0b0000_0001, 0b0001_1000]);
        assert_eq!(
            batch.advance_and_masks_inner(&[1, 3]).unwrap_err().code,
            "sequence_completed"
        );
        assert_eq!(
            batch.advance_and_masks_inner(&[0, 4]).unwrap_err().code,
            "sequence_completed"
        );
    }

    #[test]
    fn one_compiled_owner_creates_only_one_batch() {
        let mut grammar = grammar();
        grammar.new_batch_inner(1, None).expect("first batch");
        let error = match grammar.new_batch_inner(1, None) {
            Ok(_) => panic!("second batch unexpectedly succeeded"),
            Err(error) => error,
        };
        assert_eq!(error.code, "sequence_discontinuity");
    }

    #[test]
    fn an_oversized_batch_is_rejected_before_consuming_the_grammar() {
        let mut grammar = grammar();
        let error = match grammar.new_batch_inner(MAX_BATCH_ROWS + 1, Some(0)) {
            Ok(_) => panic!("oversized batch unexpectedly succeeded"),
            Err(error) => error,
        };
        assert_eq!(error.code, "configuration_error");
        grammar
            .new_batch_inner(1, None)
            .expect("failed preflight preserves the compiled grammar");
    }

    #[test]
    fn a_dead_end_next_mask_rolls_back_the_native_batch() {
        let mut grammar = compile_inner(
            "%start S\n%token A B\n%%\nS: A B;\n".to_owned(),
            vec![
                ("A".to_owned(), "a".to_owned(), 0),
                ("B".to_owned(), "b".to_owned(), 0),
            ],
            vec![Some(b"a".to_vec()), None],
            Vec::new(),
        )
        .expect("dead-end fixture compiles");
        let mut batch = grammar.new_batch_inner(1, None).expect("batch starts");
        batch.initial_masks_inner().expect("initial mask");
        let initial = batch.mask_buffer.clone();
        assert_eq!(initial, [0b0000_0001]);

        assert_eq!(
            batch.advance_and_masks_inner(&[0]).unwrap_err().code,
            "no_valid_token"
        );
        batch.initial_masks_inner().expect("failed call rolls back");
        assert_eq!(batch.mask_buffer, initial);
    }
}
