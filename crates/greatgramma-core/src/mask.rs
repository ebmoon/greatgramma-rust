use crate::{EngineError, TokenId};

pub(crate) fn clear(output: &mut [u8]) {
    let mut index = 0_usize;
    while index < output.len() {
        output[index] = 0;
        index += 1;
    }
}

pub(crate) fn set(output: &mut [u8], token: TokenId) -> Result<(), EngineError> {
    let token_index = match usize::try_from(token.get()) {
        Ok(token_index) => token_index,
        Err(_) => return Err(EngineError::CorruptPreparedData),
    };
    let byte_index = token_index / 8;
    let bit_index = token_index % 8;
    match output.get_mut(byte_index) {
        Some(byte) => {
            *byte |= 1_u8 << bit_index;
            Ok(())
        }
        None => Err(EngineError::CorruptPreparedData),
    }
}
