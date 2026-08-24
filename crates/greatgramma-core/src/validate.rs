use crate::{
    Action, ArithmeticKind, IdKind, LimitKind, ParserStateId, TerminalId, TokenEntry, TokenId,
    UnvalidatedGrammar, ValidatedGrammar, ValidatedLalr, ValidatedLexer, ValidationError,
    ValidationLimits, ValidationTable,
};

const BYTE_CLASS_TABLE_LEN: usize = 256;
const LOGICAL_ID_BYTES: u64 = 4;
const LOGICAL_ACTION_BYTES: u64 = 8;
const LOGICAL_PRODUCTION_BYTES: u64 = 8;
const LOGICAL_FIXED_SCALARS: u64 = 8;

#[derive(Clone, Copy)]
struct DeclaredSizes {
    token_count: u32,
    token_bytes: u64,
    lexer_states: u32,
    lexer_cells: u32,
    parser_action_cells: u32,
    parser_goto_cells: u32,
    production_count: u32,
}

pub(crate) fn validate(
    grammar: UnvalidatedGrammar,
    limits: ValidationLimits,
) -> Result<ValidatedGrammar, ValidationError> {
    let UnvalidatedGrammar {
        tokens,
        lexer,
        lalr,
    } = grammar;

    if tokens.is_empty() {
        return Err(ValidationError::EmptyTokenTable);
    }
    let token_count = checked_len(tokens.len(), ArithmeticKind::TokenCount)?;
    check_limit(LimitKind::Tokens, u64::from(token_count), limits.max_tokens)?;

    check_limit(
        LimitKind::DfaStates,
        u64::from(lexer.state_count),
        limits.max_dfa_states,
    )?;
    check_limit(
        LimitKind::DfaClasses,
        u64::from(lexer.class_count),
        limits.max_dfa_classes,
    )?;
    check_limit(
        LimitKind::ParserStates,
        u64::from(lalr.dimensions.state_count),
        limits.max_parser_states,
    )?;
    check_limit(
        LimitKind::Terminals,
        u64::from(lalr.dimensions.terminal_count),
        limits.max_terminals,
    )?;
    check_limit(
        LimitKind::Nonterminals,
        u64::from(lalr.dimensions.nonterminal_count),
        limits.max_nonterminals,
    )?;
    let production_count = checked_len(lalr.productions.len(), ArithmeticKind::ProductionCount)?;
    check_limit(
        LimitKind::Productions,
        u64::from(production_count),
        limits.max_productions,
    )?;

    let lexer_cells = checked_product(
        lexer.state_count,
        lexer.class_count,
        ArithmeticKind::LexerCells,
    )?;
    check_limit(
        LimitKind::DfaCells,
        u64::from(lexer_cells),
        limits.max_dfa_cells,
    )?;
    let parser_action_cells = checked_product(
        lalr.dimensions.state_count,
        lalr.dimensions.terminal_count,
        ArithmeticKind::ParserActionCells,
    )?;
    let parser_goto_cells = checked_product(
        lalr.dimensions.state_count,
        lalr.dimensions.nonterminal_count,
        ArithmeticKind::ParserGotoCells,
    )?;
    let parser_cells = parser_action_cells.checked_add(parser_goto_cells).ok_or(
        ValidationError::ArithmeticOverflow {
            calculation: ArithmeticKind::ParserCells,
        },
    )?;
    check_limit(
        LimitKind::ParserCells,
        u64::from(parser_cells),
        limits.max_parser_cells,
    )?;

    let sizes_without_tokens = DeclaredSizes {
        token_count: 0,
        token_bytes: 0,
        lexer_states: lexer.state_count,
        lexer_cells,
        parser_action_cells,
        parser_goto_cells,
        production_count,
    };
    let base_work = validation_base_work(sizes_without_tokens)?;
    check_limit(LimitKind::Work, base_work, limits.max_work)?;
    let token_bytes = validate_tokens(&tokens, base_work, &limits)?;

    let sizes = DeclaredSizes {
        token_count,
        token_bytes,
        ..sizes_without_tokens
    };
    let logical_bytes = logical_bytes(sizes)?;
    check_limit(
        LimitKind::LogicalBytes,
        logical_bytes,
        limits.max_logical_bytes,
    )?;
    check_length(
        ValidationTable::LexerByteClasses,
        BYTE_CLASS_TABLE_LEN,
        lexer.byte_classes.len(),
    )?;
    check_length(
        ValidationTable::LexerTransitions,
        count_as_usize(lexer_cells, ArithmeticKind::LexerCells)?,
        lexer.transitions.len(),
    )?;
    check_length(
        ValidationTable::LexerTerminals,
        count_as_usize(lexer.state_count, ArithmeticKind::LexerCells)?,
        lexer.terminals.len(),
    )?;
    check_length(
        ValidationTable::ParserActions,
        count_as_usize(parser_action_cells, ArithmeticKind::ParserActionCells)?,
        lalr.actions.len(),
    )?;
    check_length(
        ValidationTable::ParserGotos,
        count_as_usize(parser_goto_cells, ArithmeticKind::ParserGotoCells)?,
        lalr.gotos.len(),
    )?;

    validate_id(
        ValidationTable::LexerStart,
        None,
        IdKind::DfaState,
        lexer.start_state.get(),
        lexer.state_count,
    )?;
    validate_id(
        ValidationTable::ParserStart,
        None,
        IdKind::ParserState,
        lalr.start_state.get(),
        lalr.dimensions.state_count,
    )?;
    validate_id(
        ValidationTable::ParserEof,
        None,
        IdKind::Terminal,
        lalr.eof_terminal.get(),
        lalr.dimensions.terminal_count,
    )?;

    validate_byte_classes(&lexer.byte_classes, lexer.class_count)?;
    let mut index = 0;
    while index < lexer.transitions.len() {
        if let Some(destination) = lexer.transitions[index] {
            validate_id(
                ValidationTable::LexerTransitions,
                Some(index),
                IdKind::DfaState,
                destination.get(),
                lexer.state_count,
            )?;
        }
        index += 1;
    }
    index = 0;
    while index < lexer.terminals.len() {
        if let Some(terminal) = lexer.terminals[index] {
            validate_id(
                ValidationTable::LexerTerminals,
                Some(index),
                IdKind::Terminal,
                terminal.get(),
                lalr.dimensions.terminal_count,
            )?;
        }
        index += 1;
    }

    validate_actions(
        &lalr.actions,
        lalr.dimensions.state_count,
        lalr.dimensions.terminal_count,
        production_count,
        lalr.eof_terminal,
    )?;
    index = 0;
    while index < lalr.gotos.len() {
        if let Some(destination) = lalr.gotos[index] {
            validate_id(
                ValidationTable::ParserGotos,
                Some(index),
                IdKind::ParserState,
                destination.get(),
                lalr.dimensions.state_count,
            )?;
        }
        index += 1;
    }
    index = 0;
    while index < lalr.productions.len() {
        let production = lalr.productions[index];
        validate_id(
            ValidationTable::Productions,
            Some(index),
            IdKind::Nonterminal,
            production.lhs().get(),
            lalr.dimensions.nonterminal_count,
        )?;
        index += 1;
    }

    Ok(ValidatedGrammar::from_parts(
        token_count,
        tokens,
        ValidatedLexer::from_unvalidated(lexer),
        ValidatedLalr::from_unvalidated(lalr, production_count),
    ))
}

fn validate_tokens(
    tokens: &[TokenEntry],
    initial_work: u64,
    limits: &ValidationLimits,
) -> Result<u64, ValidationError> {
    let mut has_ordinary = false;
    let mut has_eos = false;
    let mut total_bytes = 0_u64;
    let mut work = initial_work;
    let mut index = 0;
    while index < tokens.len() {
        work = checked_add(work, 1, ArithmeticKind::Work)?;
        check_limit(LimitKind::Work, work, limits.max_work)?;

        match &tokens[index] {
            TokenEntry::Bytes(bytes) => {
                has_ordinary = true;
                if bytes.is_empty() {
                    return Err(ValidationError::EmptyTokenBytes {
                        token: TokenId::new(index_as_u32(index, ArithmeticKind::TokenCount)?),
                    });
                }
                let byte_count = u64::try_from(bytes.len()).map_err(|_| {
                    ValidationError::ArithmeticOverflow {
                        calculation: ArithmeticKind::TokenBytes,
                    }
                })?;
                total_bytes = checked_add(total_bytes, byte_count, ArithmeticKind::TokenBytes)?;
                check_limit(LimitKind::TokenBytes, total_bytes, limits.max_token_bytes)?;
                work = checked_add(work, byte_count, ArithmeticKind::Work)?;
                check_limit(LimitKind::Work, work, limits.max_work)?;
            }
            TokenEntry::Eos => has_eos = true,
        }
        index += 1;
    }

    if !has_ordinary {
        return Err(ValidationError::MissingOrdinaryToken);
    }
    if !has_eos {
        return Err(ValidationError::MissingEosToken);
    }
    Ok(total_bytes)
}

fn validate_byte_classes(classes: &[u32], class_count: u32) -> Result<(), ValidationError> {
    let mut index = 0;
    while index < classes.len() {
        let class = classes[index];
        if class >= class_count {
            return Err(ValidationError::ByteClassOutOfRange {
                byte: u8::try_from(index).map_err(|_| ValidationError::ArithmeticOverflow {
                    calculation: ArithmeticKind::LexerCells,
                })?,
                class,
                class_count,
            });
        }
        index += 1;
    }
    Ok(())
}

fn validate_actions(
    actions: &[Action],
    state_count: u32,
    terminal_count: u32,
    production_count: u32,
    eof_terminal: TerminalId,
) -> Result<(), ValidationError> {
    let terminal_width = count_as_usize(terminal_count, ArithmeticKind::ParserActionCells)?;
    let mut index = 0;
    while index < actions.len() {
        let action = actions[index];
        match action {
            Action::Error => {}
            Action::Shift(destination) => validate_id(
                ValidationTable::ParserActions,
                Some(index),
                IdKind::ParserState,
                destination.get(),
                state_count,
            )?,
            Action::Reduce(production) => validate_id(
                ValidationTable::ParserActions,
                Some(index),
                IdKind::Production,
                production.get(),
                production_count,
            )?,
            Action::Accept => {
                let column = index % terminal_width;
                let terminal =
                    TerminalId::new(index_as_u32(column, ArithmeticKind::ParserActionCells)?);
                if terminal != eof_terminal {
                    let row = index / terminal_width;
                    return Err(ValidationError::AcceptOnNonEof {
                        state: ParserStateId::new(index_as_u32(
                            row,
                            ArithmeticKind::ParserActionCells,
                        )?),
                        terminal,
                    });
                }
            }
        }
        index += 1;
    }
    Ok(())
}

fn validate_id(
    table: ValidationTable,
    index: Option<usize>,
    kind: IdKind,
    id: u32,
    count: u32,
) -> Result<(), ValidationError> {
    if id >= count {
        return Err(ValidationError::IdOutOfRange {
            table,
            index,
            kind,
            id,
            count,
        });
    }
    Ok(())
}

fn check_length(
    table: ValidationTable,
    expected: usize,
    actual: usize,
) -> Result<(), ValidationError> {
    if actual != expected {
        return Err(ValidationError::WrongTableLength {
            table,
            expected,
            actual,
        });
    }
    Ok(())
}

fn check_limit(limit: LimitKind, actual: u64, maximum: u64) -> Result<(), ValidationError> {
    if actual > maximum {
        return Err(ValidationError::LimitExceeded {
            limit,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn checked_len(length: usize, calculation: ArithmeticKind) -> Result<u32, ValidationError> {
    index_as_u32(length, calculation)
}

fn checked_product(
    left: u32,
    right: u32,
    calculation: ArithmeticKind,
) -> Result<u32, ValidationError> {
    left.checked_mul(right)
        .ok_or(ValidationError::ArithmeticOverflow { calculation })
}

fn index_as_u32(index: usize, calculation: ArithmeticKind) -> Result<u32, ValidationError> {
    u32::try_from(index).map_err(|_| ValidationError::ArithmeticOverflow { calculation })
}

fn count_as_usize(count: u32, calculation: ArithmeticKind) -> Result<usize, ValidationError> {
    usize::try_from(count).map_err(|_| ValidationError::ArithmeticOverflow { calculation })
}

fn checked_add(left: u64, right: u64, calculation: ArithmeticKind) -> Result<u64, ValidationError> {
    left.checked_add(right)
        .ok_or(ValidationError::ArithmeticOverflow { calculation })
}

fn checked_mul(left: u64, right: u64, calculation: ArithmeticKind) -> Result<u64, ValidationError> {
    left.checked_mul(right)
        .ok_or(ValidationError::ArithmeticOverflow { calculation })
}

fn logical_bytes(sizes: DeclaredSizes) -> Result<u64, ValidationError> {
    let calculation = ArithmeticKind::LogicalBytes;
    let mut total = checked_add(u64::from(sizes.token_count), sizes.token_bytes, calculation)?;
    total = checked_add(
        total,
        checked_mul(LOGICAL_FIXED_SCALARS, LOGICAL_ID_BYTES, calculation)?,
        calculation,
    )?;
    total = checked_add(
        total,
        checked_mul(BYTE_CLASS_TABLE_LEN as u64, LOGICAL_ID_BYTES, calculation)?,
        calculation,
    )?;
    total = checked_add(
        total,
        checked_mul(u64::from(sizes.lexer_cells), LOGICAL_ID_BYTES, calculation)?,
        calculation,
    )?;
    total = checked_add(
        total,
        checked_mul(u64::from(sizes.lexer_states), LOGICAL_ID_BYTES, calculation)?,
        calculation,
    )?;
    total = checked_add(
        total,
        checked_mul(
            u64::from(sizes.parser_action_cells),
            LOGICAL_ACTION_BYTES,
            calculation,
        )?,
        calculation,
    )?;
    total = checked_add(
        total,
        checked_mul(
            u64::from(sizes.parser_goto_cells),
            LOGICAL_ID_BYTES,
            calculation,
        )?,
        calculation,
    )?;
    checked_add(
        total,
        checked_mul(
            u64::from(sizes.production_count),
            LOGICAL_PRODUCTION_BYTES,
            calculation,
        )?,
        calculation,
    )
}

fn validation_base_work(sizes: DeclaredSizes) -> Result<u64, ValidationError> {
    let calculation = ArithmeticKind::Work;
    let mut total = checked_add(0, LOGICAL_FIXED_SCALARS, calculation)?;
    total = checked_add(total, BYTE_CLASS_TABLE_LEN as u64, calculation)?;
    total = checked_add(total, u64::from(sizes.lexer_states), calculation)?;
    total = checked_add(total, u64::from(sizes.lexer_cells), calculation)?;
    total = checked_add(total, u64::from(sizes.parser_action_cells), calculation)?;
    total = checked_add(total, u64::from(sizes.parser_goto_cells), calculation)?;
    checked_add(total, u64::from(sizes.production_count), calculation)
}
