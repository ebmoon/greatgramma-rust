use greatgramma_compile::{
    CompileError, CompileLimits, SourceGrammar, TerminalSpec, TokenSpec, TokenizerError,
    TokenizerManifest, compile, normalize,
};
use greatgramma_core::{AdvanceResult, LimitKind, TokenId, ValidationError};

fn byte_source() -> SourceGrammar {
    SourceGrammar::new(
        "%start S\n%token ZERO HIGH\n%%\nS: ZERO HIGH;\n",
        vec![
            TerminalSpec::new("ZERO", r"\x00", 0),
            TerminalSpec::new("HIGH", r"\xFF", 0),
        ],
    )
}

#[test]
fn preserves_nul_non_utf8_duplicates_and_multiple_eos_ids() {
    let manifest = TokenizerManifest::new(vec![
        TokenSpec::Bytes(vec![0]),
        TokenSpec::Bytes(vec![0xff]),
        TokenSpec::Bytes(vec![0, 0xff]),
        TokenSpec::Bytes(vec![0, 0xff]),
        TokenSpec::Eos,
        TokenSpec::Eos,
    ]);
    let prepared = compile(byte_source(), manifest, CompileLimits::default()).unwrap();
    let mut matcher = prepared.into_matcher(2).unwrap();
    let mut mask = vec![0; matcher.mask_bytes()];

    matcher.mask(0, &mut mask).unwrap();
    assert_eq!(mask, vec![0b0000_1101]);

    assert_eq!(
        matcher
            .advance_batch(&[TokenId::new(2), TokenId::new(3)])
            .unwrap(),
        vec![AdvanceResult::Continue, AdvanceResult::Continue]
    );
    matcher.mask(0, &mut mask).unwrap();
    assert_eq!(mask, vec![0b0011_0000]);
    assert_eq!(
        matcher.advance(0, TokenId::new(4)).unwrap(),
        AdvanceResult::Accepted
    );
    assert_eq!(
        matcher.advance(1, TokenId::new(5)).unwrap(),
        AdvanceResult::Accepted
    );
}

#[test]
fn rejects_empty_or_incomplete_exact_manifests_before_table_preparation() {
    let empty_token = TokenizerManifest::new(vec![TokenSpec::Bytes(Vec::new()), TokenSpec::Eos]);
    assert_eq!(
        normalize(byte_source(), empty_token, CompileLimits::default()),
        Err(CompileError::Tokenizer(
            TokenizerError::EmptyOrdinaryToken { token: 0 }
        ))
    );

    let no_eos = TokenizerManifest::new(vec![TokenSpec::Bytes(vec![0])]);
    assert_eq!(
        normalize(byte_source(), no_eos, CompileLimits::default()),
        Err(CompileError::Tokenizer(TokenizerError::MissingEos))
    );

    let no_ordinary = TokenizerManifest::new(vec![TokenSpec::Eos]);
    assert_eq!(
        normalize(byte_source(), no_ordinary, CompileLimits::default()),
        Err(CompileError::Tokenizer(
            TokenizerError::MissingOrdinaryToken
        ))
    );

    let empty = TokenizerManifest::new(Vec::new());
    assert_eq!(
        normalize(byte_source(), empty, CompileLimits::default()),
        Err(CompileError::Tokenizer(TokenizerError::EmptyManifest))
    );
}

#[test]
fn bounds_manifest_count_and_bytes_before_normalized_table_allocation() {
    let manifest = TokenizerManifest::new(vec![TokenSpec::Bytes(vec![0]), TokenSpec::Eos]);
    let mut count_limits = CompileLimits::default();
    count_limits.validation.max_tokens = 1;
    assert_eq!(
        normalize(byte_source(), manifest.clone(), count_limits),
        Err(CompileError::Validation(ValidationError::LimitExceeded {
            limit: LimitKind::Tokens,
            actual: 2,
            maximum: 1,
        }))
    );

    let mut byte_limits = CompileLimits::default();
    byte_limits.validation.max_token_bytes = 0;
    assert_eq!(
        normalize(byte_source(), manifest, byte_limits),
        Err(CompileError::Validation(ValidationError::LimitExceeded {
            limit: LimitKind::TokenBytes,
            actual: 1,
            maximum: 0,
        }))
    );
}
