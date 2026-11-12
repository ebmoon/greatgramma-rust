use std::error::Error;

use greatgramma::{
    CompileError, EngineError, LexerError, ParserError, PreparationError, SpannerQueryError,
    TokenStepError, TokenizerError, ValidationError,
};

#[test]
fn public_errors_implement_standard_error_traits() {
    fn assert_error<T: Error + Send + Sync + 'static>() {}
    assert_error::<EngineError>();
    assert_error::<ValidationError>();
    assert_error::<PreparationError>();
    assert_error::<ParserError>();
    assert_error::<LexerError>();
    assert_error::<TokenStepError>();
    assert_error::<SpannerQueryError>();
    assert_error::<CompileError>();
    assert_error::<TokenizerError>();
}

#[test]
fn messages_include_useful_context() {
    use greatgramma::{DfaStateId, ProductionId, SequenceId, TokenId, ValidationTable};

    assert_eq!(
        EngineError::BatchConstraintViolation {
            row: 2,
            token: TokenId::new(7)
        }
        .to_string(),
        "token 7 violates the grammar constraint in row 2"
    );
    assert_eq!(
        ValidationError::WrongTableLength {
            table: ValidationTable::LexerTransitions,
            expected: 4,
            actual: 3,
        }
        .to_string(),
        "lexer transition table has 3 entries, expected 4"
    );
    assert_eq!(
        PreparationError::TooLarge {
            required: 12,
            maximum: 10
        }
        .to_string(),
        "grammar preparation requires 12 items or work units, maximum is 10"
    );
    assert_eq!(
        ParserError::StackUnderflow {
            production: ProductionId::new(3),
            pop_len: 2,
            stack_len: 2,
        }
        .to_string(),
        "production 3 cannot pop 2 states from a parser stack of length 2 while preserving its base"
    );
    assert_eq!(
        LexerError::CannotBeginLexeme { byte: 0xff }.to_string(),
        "byte 0xff cannot begin a lexeme"
    );
    assert_eq!(
        TokenStepError::InvalidState {
            state: DfaStateId::new(9)
        }
        .to_string(),
        "invalid lexer state 9 for token execution"
    );
    assert_eq!(
        SpannerQueryError::InvalidSequence {
            sequence: SequenceId::new(5)
        }
        .to_string(),
        "invalid sequence 5 for spanner query"
    );
}

#[test]
fn compiler_preserves_nested_error_sources() {
    let errors = [
        CompileError::Tokenizer(TokenizerError::MissingEos),
        CompileError::Validation(ValidationError::MissingEosToken),
        CompileError::Preparation(PreparationError::InvariantViolation),
    ];
    let messages = [
        "tokenizer manifest: the manifest has no EOS token",
        "normalized grammar validation: the token table has no EOS token",
        "grammar preparation: prepared grammar invariant violated",
    ];
    for (error, message) in errors.iter().zip(messages) {
        assert_eq!(error.to_string(), message);
        let source = error.source().expect("wrapped errors retain their source");
        assert!(message.ends_with(&source.to_string()));
        assert!(source.source().is_none());
    }
    assert!(errors[0].source().unwrap().is::<TokenizerError>());
    assert!(errors[1].source().unwrap().is::<ValidationError>());
    assert!(errors[2].source().unwrap().is::<PreparationError>());
    assert!(CompileError::NoReductionProgressWitness.source().is_none());
}

#[test]
fn errors_propagate_into_boxed_standard_errors() {
    fn run() -> Result<(), Box<dyn Error>> {
        Err(EngineError::EmptyBatch)?
    }
    let error = run().expect_err("the matcher error should propagate");
    assert!(error.is::<EngineError>());
    assert!(error.source().is_none());
}
