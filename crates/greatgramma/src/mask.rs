use crate::{EngineError, TokenId};

pub(crate) fn clear(output: &mut [u8]) {
    output.fill(0);
}

pub(crate) fn set(output: &mut [u8], token: TokenId) -> Result<(), EngineError> {
    let token_index = usize::try_from(token.get()).map_err(|_| EngineError::CorruptPreparedData)?;
    let byte = output
        .get_mut(token_index / 8)
        .ok_or(EngineError::CorruptPreparedData)?;
    *byte |= 1_u8 << (token_index % 8);
    Ok(())
}
