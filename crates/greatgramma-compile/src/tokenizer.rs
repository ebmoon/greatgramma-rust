use greatgramma_core::{LimitKind, TokenEntry, ValidationError, ValidationLimits};

use crate::{CompileError, TokenizerError};

/// One exact normalized model-token entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenSpec {
    Bytes(Vec<u8>),
    Eos,
}

/// Exact token-local bytes supplied by a tokenizer compatibility layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenizerManifest {
    tokens: Vec<TokenSpec>,
}

impl TokenizerManifest {
    #[must_use]
    pub fn new(tokens: Vec<TokenSpec>) -> Self {
        Self { tokens }
    }

    #[must_use]
    pub fn tokens(&self) -> &[TokenSpec] {
        &self.tokens
    }

    #[must_use]
    pub fn into_tokens(self) -> Vec<TokenSpec> {
        self.tokens
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Extracts exact token-local bytes from the strict tokenizer.json profile.
    pub fn from_tokenizer_json(
        json: &str,
        eos_token_ids: &[u32],
    ) -> Result<(Self, crate::CompatibilityReport), CompileError> {
        Self::from_tokenizer_json_with_limits(
            json,
            eos_token_ids,
            crate::TokenizerJsonLimits::default(),
        )
    }

    /// Extracts strict tokenizer bytes under caller-supplied finite budgets.
    pub fn from_tokenizer_json_with_limits(
        json: &str,
        eos_token_ids: &[u32],
        limits: crate::TokenizerJsonLimits,
    ) -> Result<(Self, crate::CompatibilityReport), CompileError> {
        crate::tokenizer_json::extract(json, eos_token_ids, limits)
    }
}

pub(crate) fn normalize_manifest(
    manifest: TokenizerManifest,
    limits: ValidationLimits,
) -> Result<Vec<TokenEntry>, CompileError> {
    if manifest.tokens.is_empty() {
        return Err(CompileError::Tokenizer(TokenizerError::EmptyManifest));
    }

    let token_count = u64::try_from(manifest.tokens.len()).unwrap_or(u64::MAX);
    check_validation_limit(LimitKind::Tokens, token_count, limits.max_tokens)?;
    let token_bytes = manifest.tokens.iter().fold(0_u64, |total, token| {
        let bytes = match token {
            TokenSpec::Bytes(bytes) => u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            TokenSpec::Eos => 0,
        };
        total.saturating_add(bytes)
    });
    check_validation_limit(LimitKind::TokenBytes, token_bytes, limits.max_token_bytes)?;
    let mut has_ordinary = false;
    let mut has_eos = false;
    let mut output = Vec::new();
    output
        .try_reserve_exact(manifest.tokens.len())
        .map_err(|_| CompileError::AllocationFailure {
            requested: manifest.tokens.len(),
        })?;
    for (index, token) in manifest.tokens.into_iter().enumerate() {
        match token {
            TokenSpec::Bytes(bytes) => {
                if bytes.is_empty() {
                    return Err(CompileError::Tokenizer(
                        TokenizerError::EmptyOrdinaryToken { token: index },
                    ));
                }
                has_ordinary = true;
                output.push(TokenEntry::Bytes(bytes));
            }
            TokenSpec::Eos => {
                has_eos = true;
                output.push(TokenEntry::Eos);
            }
        }
    }

    if !has_ordinary {
        return Err(CompileError::Tokenizer(
            TokenizerError::MissingOrdinaryToken,
        ));
    }
    if !has_eos {
        return Err(CompileError::Tokenizer(TokenizerError::MissingEos));
    }
    Ok(output)
}

fn check_validation_limit(limit: LimitKind, actual: u64, maximum: u64) -> Result<(), CompileError> {
    if actual > maximum {
        return Err(CompileError::Validation(ValidationError::LimitExceeded {
            limit,
            actual,
            maximum,
        }));
    }
    Ok(())
}
