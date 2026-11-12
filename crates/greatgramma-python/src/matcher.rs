//! Owning Python grammar and batch classes, including transactional mask updates.

use greatgramma::{Matcher, PreparedGrammar, TokenId};
use pyo3::{prelude::*, types::PyBytes};

use super::error::{NativeFailure, detached, fallible_py_bytes, map_engine_error};

const MAX_BATCH_ROWS: usize = 4096;
const MAX_PACKED_MASK_BYTES: usize = 256 * 1024 * 1024;

#[pyclass(module = "greatgramma._native")]
pub(super) struct NativeGrammar {
    prepared: Option<PreparedGrammar>,
    vocab_size: u32,
}

impl NativeGrammar {
    pub(super) fn from_prepared(prepared: PreparedGrammar, vocab_size: u32) -> Self {
        Self {
            prepared: Some(prepared),
            vocab_size,
        }
    }

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
pub(super) struct NativeBatch {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile_inner;

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
