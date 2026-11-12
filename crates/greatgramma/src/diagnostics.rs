use std::{error::Error, fmt};

use crate::{
    ArithmeticKind, EngineError, IdKind, LexerError, LimitKind, ParserError, PreparationError,
    SpannerQueryError, TokenStepError, ValidationError, ValidationTable,
};

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidToken { token } => write!(f, "invalid token {}", token.get()),
            Self::InvalidRow { row, rows } => {
                write!(f, "row {row} is outside the batch of {rows} rows")
            }
            Self::EmptyBatch => f.write_str("a matcher batch must contain at least one row"),
            Self::BatchSizeMismatch { expected, actual } => {
                write!(f, "batch has {actual} entries, expected {expected}")
            }
            Self::MissingRunningToken { row } => write!(f, "running row {row} requires a token"),
            Self::Completed => f.write_str("the matcher row has already completed"),
            Self::ConstraintViolation { token } => {
                write!(f, "token {} violates the grammar constraint", token.get())
            }
            Self::BatchConstraintViolation { row, token } => write!(
                f,
                "token {} violates the grammar constraint in row {row}",
                token.get()
            ),
            Self::BufferTooSmall { required, actual } => {
                write!(
                    f,
                    "output buffer has {actual} bytes, at least {required} are required"
                )
            }
            Self::NoValidToken => f.write_str("no token satisfies the grammar constraint"),
            Self::AllocationFailure { requested } => {
                write!(f, "matcher allocation failed for {requested} entries")
            }
            Self::CorruptPreparedData => f.write_str("prepared grammar data is inconsistent"),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTokenTable => f.write_str("the token table is empty"),
            Self::MissingOrdinaryToken => f.write_str("the token table has no ordinary token"),
            Self::MissingEosToken => f.write_str("the token table has no EOS token"),
            Self::EmptyTokenBytes { token } => {
                write!(f, "ordinary token {} has no bytes", token.get())
            }
            Self::WrongTableLength {
                table,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "{} has {actual} entries, expected {expected}",
                    table_name(*table)
                )
            }
            Self::IdOutOfRange {
                table,
                index,
                kind,
                id,
                count,
            } => {
                write!(f, "{}", table_name(*table))?;
                if let Some(index) = index {
                    write!(f, " entry {index}")?;
                }
                write!(
                    f,
                    " references {} {id}, but the count is {count}",
                    id_name(*kind)
                )
            }
            Self::ByteClassOutOfRange {
                byte,
                class,
                class_count,
            } => write!(
                f,
                "byte 0x{byte:02x} maps to class {class}, but the class count is {class_count}"
            ),
            Self::AcceptingLexerStart { state, terminal } => write!(
                f,
                "lexer start state {} accepts terminal {}, allowing an empty lexeme",
                state.get(),
                terminal.get()
            ),
            Self::LexerTerminalIsParserEof { state, terminal } => write!(
                f,
                "lexer state {} uses parser EOF terminal {} as a lexeme",
                state.get(),
                terminal.get()
            ),
            Self::IgnoredTerminalIsParserEof { index, terminal } => write!(
                f,
                "ignored terminal entry {index} refers to parser EOF terminal {}",
                terminal.get()
            ),
            Self::AcceptOnNonEof { state, terminal } => write!(
                f,
                "parser state {} accepts non-EOF terminal {}",
                state.get(),
                terminal.get()
            ),
            Self::ArithmeticOverflow { calculation } => {
                write!(
                    f,
                    "overflow while calculating {}",
                    calculation_name(*calculation)
                )
            }
            Self::LimitExceeded {
                limit,
                actual,
                maximum,
            } => {
                write!(
                    f,
                    "{} is {actual}, maximum is {maximum}",
                    limit_name(*limit)
                )
            }
            Self::AllocationFailure { table, requested } => {
                write!(
                    f,
                    "allocation failed for {requested} entries in {}",
                    table_name(*table)
                )
            }
        }
    }
}

impl fmt::Display for PreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { required, maximum } => write!(
                f,
                "grammar preparation requires {required} items or work units, maximum is {maximum}"
            ),
            Self::AllocationFailure { requested } => {
                write!(
                    f,
                    "grammar preparation allocation failed for {requested} entries"
                )
            }
            Self::InvariantViolation => f.write_str("prepared grammar invariant violated"),
        }
    }
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStack => f.write_str("the parser stack is empty"),
            Self::EmptySequenceHead => f.write_str("the parser sequence head is empty"),
            Self::InvalidState { state } => write!(f, "invalid parser state {}", state.get()),
            Self::InvalidTerminal { terminal } => write!(f, "invalid terminal {}", terminal.get()),
            Self::InvalidSequence { sequence } => write!(f, "invalid sequence {}", sequence.get()),
            Self::StackUnderflow {
                production,
                pop_len,
                stack_len,
            } => write!(
                f,
                "production {} cannot pop {pop_len} states from a parser stack of length {stack_len} while preserving its base",
                production.get()
            ),
            Self::MissingGoto {
                state,
                nonterminal,
                production,
            } => write!(
                f,
                "parser state {} has no goto for nonterminal {} after reducing production {}",
                state.get(),
                nonterminal.get(),
                production.get()
            ),
            Self::AcceptBeforeEnd { terminal_index } => {
                write!(
                    f,
                    "parser accepted at terminal index {terminal_index} before the end of input"
                )
            }
            Self::InvalidReductionProgress { state, terminal } => write!(
                f,
                "parser reduction makes no valid progress in state {} on terminal {}",
                state.get(),
                terminal.get()
            ),
            Self::AllocationFailure { requested } => {
                write!(f, "parser allocation failed for {requested} states")
            }
            Self::InvariantViolation => f.write_str("parser table or stack invariant violated"),
        }
    }
}

impl fmt::Display for LexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState { state } => write!(f, "invalid lexer state {}", state.get()),
            Self::CannotBeginLexeme { byte } => {
                write!(f, "byte 0x{byte:02x} cannot begin a lexeme")
            }
            Self::ByteAfterUnfinishedResidual { state, byte } => write!(
                f,
                "byte 0x{byte:02x} cannot continue the unfinished lexeme in lexer state {}",
                state.get()
            ),
            Self::EosAfterUnfinishedResidual { state } => {
                write!(
                    f,
                    "EOS reached with an unfinished lexeme in lexer state {}",
                    state.get()
                )
            }
        }
    }
}

impl fmt::Display for TokenStepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidToken { token } => {
                write!(f, "invalid token {} for token execution", token.get())
            }
            Self::InvalidState { state } => {
                write!(f, "invalid lexer state {} for token execution", state.get())
            }
            Self::AllocationFailure { requested } => {
                write!(
                    f,
                    "token execution allocation failed for {requested} terminals"
                )
            }
        }
    }
}

impl fmt::Display for SpannerQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState { state } => {
                write!(f, "invalid lexer state {} for spanner query", state.get())
            }
            Self::InvalidSequence { sequence } => {
                write!(f, "invalid sequence {} for spanner query", sequence.get())
            }
        }
    }
}

impl Error for EngineError {}
impl Error for ValidationError {}
impl Error for PreparationError {}
impl Error for ParserError {}
impl Error for LexerError {}
impl Error for TokenStepError {}
impl Error for SpannerQueryError {}

fn table_name(table: ValidationTable) -> &'static str {
    match table {
        ValidationTable::Tokens => "token table",
        ValidationTable::LexerByteClasses => "lexer byte-class table",
        ValidationTable::LexerTransitions => "lexer transition table",
        ValidationTable::LexerStart => "lexer start state",
        ValidationTable::LexerTerminals => "lexer terminal table",
        ValidationTable::ParserActions => "parser action table",
        ValidationTable::ParserGotos => "parser goto table",
        ValidationTable::ParserStart => "parser start state",
        ValidationTable::ParserEof => "parser EOF terminal",
        ValidationTable::ParserIgnoredTerminals => "ignored terminal table",
        ValidationTable::Productions => "production table",
    }
}

fn id_name(kind: IdKind) -> &'static str {
    match kind {
        IdKind::DfaState => "lexer state",
        IdKind::Terminal => "terminal",
        IdKind::ParserState => "parser state",
        IdKind::Nonterminal => "nonterminal",
        IdKind::Production => "production",
    }
}

fn calculation_name(kind: ArithmeticKind) -> &'static str {
    match kind {
        ArithmeticKind::TokenCount => "the token count",
        ArithmeticKind::TokenBytes => "token bytes",
        ArithmeticKind::ProductionCount => "the production count",
        ArithmeticKind::IgnoredTerminalCount => "the ignored terminal count",
        ArithmeticKind::LexerCells => "lexer table cells",
        ArithmeticKind::ParserActionCells => "parser action cells",
        ArithmeticKind::ParserGotoCells => "parser goto cells",
        ArithmeticKind::ParserCells => "parser table cells",
        ArithmeticKind::LogicalBytes => "logical storage bytes",
        ArithmeticKind::Work => "validation work",
    }
}

fn limit_name(kind: LimitKind) -> &'static str {
    match kind {
        LimitKind::Tokens => "token count",
        LimitKind::TokenBytes => "token byte count",
        LimitKind::DfaStates => "lexer state count",
        LimitKind::DfaClasses => "lexer class count",
        LimitKind::DfaCells => "lexer table cell count",
        LimitKind::ParserStates => "parser state count",
        LimitKind::Terminals => "terminal count",
        LimitKind::Nonterminals => "nonterminal count",
        LimitKind::Productions => "production count",
        LimitKind::ParserCells => "parser table cell count",
        LimitKind::LogicalBytes => "logical storage size in bytes",
        LimitKind::Work => "validation work",
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{
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
        use crate::{DfaStateId, ProductionId, SequenceId, TokenId, ValidationTable};

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
}
