//! Private PyO3 boundary for the typed `greatgramma` Python package.

#![forbid(unsafe_code)]

mod boundary;
mod compiler;
mod error;
mod matcher;

use pyo3::{prelude::*, types::PyModule};

#[pymodule]
fn _native(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("NativeError", py.get_type::<error::NativeError>())?;
    module.add_class::<matcher::NativeGrammar>()?;
    module.add_class::<matcher::NativeBatch>()?;
    module.add_function(wrap_pyfunction!(compiler::_compile_yacc, module)?)?;
    module.add_function(wrap_pyfunction!(compiler::_token_bytes_from_json, module)?)?;
    #[cfg(feature = "panic-test-hook")]
    module.add_function(wrap_pyfunction!(error::_test_panic, module)?)?;
    Ok(())
}
