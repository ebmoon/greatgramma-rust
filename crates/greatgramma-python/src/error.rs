//! Structured native failures and panic containment at the Python ABI boundary.

use std::panic::{AssertUnwindSafe, catch_unwind};

use greatgramma::EngineError;
use pyo3::{create_exception, exceptions::PyException, prelude::*, types::PyBytes};

create_exception!(_native, NativeError, PyException);

#[derive(Debug)]
pub(super) struct NativeFailure {
    pub(super) code: &'static str,
    message: String,
    pub(super) row: Option<usize>,
    pub(super) token: Option<u32>,
}

impl NativeFailure {
    pub(super) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            row: None,
            token: None,
        }
    }

    pub(super) fn at_row(mut self, row: usize) -> Self {
        self.row = Some(row);
        self
    }

    pub(super) fn for_token(mut self, token: u32) -> Self {
        self.token = Some(token);
        self
    }

    pub(super) fn internal(message: impl Into<String>) -> Self {
        Self::new("internal_error", message)
    }

    pub(super) fn into_pyerr(self) -> PyErr {
        NativeError::new_err((self.code, self.message, self.row, self.token))
    }
}

pub(super) fn detached<T, F>(py: Python<'_>, operation: F) -> Result<T, NativeFailure>
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

pub(super) fn fallible_py_bytes(py: Python<'_>, bytes: &[u8]) -> PyResult<Py<PyBytes>> {
    PyBytes::new_with(py, bytes.len(), |output| {
        output.copy_from_slice(bytes);
        Ok(())
    })
    .map(Bound::unbind)
    .map_err(|_| NativeFailure::internal("could not allocate Python mask bytes").into_pyerr())
}

pub(super) fn map_engine_error(error: EngineError) -> NativeFailure {
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

#[cfg(feature = "panic-test-hook")]
#[pyfunction]
pub(super) fn _test_panic(py: Python<'_>) -> PyResult<()> {
    detached(py, || -> Result<(), NativeFailure> {
        panic!("intentional ABI panic test")
    })
    .map_err(NativeFailure::into_pyerr)
}
