use crate::{
    Action, ArithmeticKind, DfaStateId, IdKind, LalrTable, LexerDfa, LimitKind, ParserStateId,
    TerminalId, TokenEntry, TokenId, UnvalidatedGrammar, ValidatedGrammar, ValidatedLalr,
    ValidatedLexer, ValidationError, ValidationLimits, ValidationTable,
};

const BYTE_CLASS_TABLE_LEN: usize = 256;
const LOGICAL_ID_BYTES: u64 = 4;
const LOGICAL_ACTION_BYTES: u64 = 12;
const LOGICAL_ACTION_WORK: u64 = 2;
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
    ignored_terminal_count: u32,
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

    let sizes = validate_declared_sizes(&tokens, &lexer, &lalr, &limits)?;
    let token_count = sizes.token_count;
    let lexer_cells = sizes.lexer_cells;
    let parser_action_cells = sizes.parser_action_cells;
    let parser_goto_cells = sizes.parser_goto_cells;
    let production_count = sizes.production_count;

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
    validate_ignored_terminals(
        &lalr.ignored_terminals,
        lalr.dimensions.terminal_count,
        lalr.eof_terminal,
    )?;

    validate_byte_classes(&lexer.byte_classes, lexer.class_count)?;
    validate_transitions(&lexer.transitions, lexer.state_count)?;
    validate_lexer_terminals(
        &lexer.terminals,
        lexer.start_state,
        lalr.dimensions.terminal_count,
        lalr.eof_terminal,
    )?;

    validate_actions(
        &lalr.actions,
        lalr.dimensions.state_count,
        lalr.dimensions.terminal_count,
        production_count,
        lalr.eof_terminal,
    )?;
    validate_gotos(&lalr.gotos, lalr.dimensions.state_count)?;
    validate_productions(&lalr.productions, lalr.dimensions.nonterminal_count)?;

    let ignored_terminals =
        build_ignored_terminal_table(&lalr.ignored_terminals, lalr.dimensions.terminal_count)?;

    Ok(ValidatedGrammar::from_parts(
        token_count,
        tokens,
        ValidatedLexer::from_unvalidated(lexer),
        ValidatedLalr::from_unvalidated(lalr, production_count, ignored_terminals),
    ))
}

fn validate_declared_sizes(
    tokens: &[TokenEntry],
    lexer_table: &LexerDfa,
    parser_table: &LalrTable,
    limits: &ValidationLimits,
) -> Result<DeclaredSizes, ValidationError> {
    if tokens.is_empty() {
        return Err(ValidationError::EmptyTokenTable);
    }
    let token_count = checked_len(tokens.len(), ArithmeticKind::TokenCount)?;
    check_limit(LimitKind::Tokens, u64::from(token_count), limits.max_tokens)?;

    check_limit(
        LimitKind::DfaStates,
        u64::from(lexer_table.state_count),
        limits.max_dfa_states,
    )?;
    check_limit(
        LimitKind::DfaClasses,
        u64::from(lexer_table.class_count),
        limits.max_dfa_classes,
    )?;
    check_limit(
        LimitKind::ParserStates,
        u64::from(parser_table.dimensions.state_count),
        limits.max_parser_states,
    )?;
    check_limit(
        LimitKind::Terminals,
        u64::from(parser_table.dimensions.terminal_count),
        limits.max_terminals,
    )?;
    check_limit(
        LimitKind::Nonterminals,
        u64::from(parser_table.dimensions.nonterminal_count),
        limits.max_nonterminals,
    )?;
    let production_count = checked_len(
        parser_table.productions.len(),
        ArithmeticKind::ProductionCount,
    )?;
    let ignored_terminal_count = checked_len(
        parser_table.ignored_terminals.len(),
        ArithmeticKind::IgnoredTerminalCount,
    )?;
    check_limit(
        LimitKind::Productions,
        u64::from(production_count),
        limits.max_productions,
    )?;

    let lexer_cells = checked_product(
        lexer_table.state_count,
        lexer_table.class_count,
        ArithmeticKind::LexerCells,
    )?;
    check_limit(
        LimitKind::DfaCells,
        u64::from(lexer_cells),
        limits.max_dfa_cells,
    )?;
    let parser_action_cells = checked_product(
        parser_table.dimensions.state_count,
        parser_table.dimensions.terminal_count,
        ArithmeticKind::ParserActionCells,
    )?;
    let parser_goto_cells = checked_product(
        parser_table.dimensions.state_count,
        parser_table.dimensions.nonterminal_count,
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
        lexer_states: lexer_table.state_count,
        lexer_cells,
        parser_action_cells,
        parser_goto_cells,
        production_count,
        ignored_terminal_count,
    };
    let base_work = validation_base_work(sizes_without_tokens)?;
    check_limit(LimitKind::Work, base_work, limits.max_work)?;
    let token_bytes = validate_tokens(tokens, base_work, limits)?;

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
    Ok(sizes)
}

fn validate_transitions(
    transitions: &[Option<DfaStateId>],
    state_count: u32,
) -> Result<(), ValidationError> {
    let mut validation_error = None;
    let mut index = 0_usize;
    while index < transitions.len() && validation_error.is_none() {
        validation_error = validate_transition(transitions[index], index, state_count).err();
        index += 1;
    }
    validation_result(validation_error)
}

fn validate_transition(
    destination: Option<DfaStateId>,
    index: usize,
    state_count: u32,
) -> Result<(), ValidationError> {
    match destination {
        Some(destination) => validate_id(
            ValidationTable::LexerTransitions,
            Some(index),
            IdKind::DfaState,
            destination.get(),
            state_count,
        ),
        None => Ok(()),
    }
}

fn validate_lexer_terminals(
    terminals: &[Option<TerminalId>],
    start_state: DfaStateId,
    terminal_count: u32,
    eof_terminal: TerminalId,
) -> Result<(), ValidationError> {
    let mut validation_error = None;
    let mut index = 0_usize;
    while index < terminals.len() && validation_error.is_none() {
        validation_error = validate_lexer_terminal(
            terminals[index],
            index,
            start_state,
            terminal_count,
            eof_terminal,
        )
        .err();
        index += 1;
    }
    validation_result(validation_error)
}

fn validate_lexer_terminal(
    terminal: Option<TerminalId>,
    index: usize,
    start_state: DfaStateId,
    terminal_count: u32,
    eof_terminal: TerminalId,
) -> Result<(), ValidationError> {
    match terminal {
        Some(terminal) => {
            validate_id(
                ValidationTable::LexerTerminals,
                Some(index),
                IdKind::Terminal,
                terminal.get(),
                terminal_count,
            )?;
            let state = DfaStateId::new(index_as_u32(index, ArithmeticKind::LexerCells)?);
            if state == start_state {
                Err(ValidationError::AcceptingLexerStart { state, terminal })
            } else if terminal == eof_terminal {
                Err(ValidationError::LexerTerminalIsParserEof { state, terminal })
            } else {
                Ok(())
            }
        }
        None => Ok(()),
    }
}

fn validate_gotos(
    gotos: &[Option<ParserStateId>],
    state_count: u32,
) -> Result<(), ValidationError> {
    let mut validation_error = None;
    let mut index = 0_usize;
    while index < gotos.len() && validation_error.is_none() {
        validation_error = validate_goto(gotos[index], index, state_count).err();
        index += 1;
    }
    validation_result(validation_error)
}

fn validate_goto(
    destination: Option<ParserStateId>,
    index: usize,
    state_count: u32,
) -> Result<(), ValidationError> {
    match destination {
        Some(destination) => validate_id(
            ValidationTable::ParserGotos,
            Some(index),
            IdKind::ParserState,
            destination.get(),
            state_count,
        ),
        None => Ok(()),
    }
}

fn validate_productions(
    productions: &[crate::Production],
    nonterminal_count: u32,
) -> Result<(), ValidationError> {
    let mut validation_error = None;
    let mut index = 0_usize;
    while index < productions.len() && validation_error.is_none() {
        validation_error = validate_id(
            ValidationTable::Productions,
            Some(index),
            IdKind::Nonterminal,
            productions[index].lhs().get(),
            nonterminal_count,
        )
        .err();
        index += 1;
    }
    validation_result(validation_error)
}

fn build_ignored_terminal_table(
    ignored_terminals: &[TerminalId],
    terminal_count: u32,
) -> Result<Vec<u8>, ValidationError> {
    let requested = count_as_usize(terminal_count, ArithmeticKind::IgnoredTerminalCount)?;
    let mut table = Vec::new();
    table
        .try_reserve_exact(requested)
        .map_err(|_| ValidationError::AllocationFailure {
            table: ValidationTable::ParserIgnoredTerminals,
            requested,
        })?;
    table.resize(requested, 0);

    let mut validation_error = None;
    let mut index = 0_usize;
    while index < ignored_terminals.len() && validation_error.is_none() {
        validation_error = mark_ignored_terminal(&mut table, ignored_terminals[index]).err();
        index += 1;
    }
    match validation_error {
        Some(validation_error) => Err(validation_error),
        None => Ok(table),
    }
}

fn mark_ignored_terminal(table: &mut [u8], terminal: TerminalId) -> Result<(), ValidationError> {
    let terminal_index = match usize::try_from(terminal.get()) {
        Ok(terminal_index) => terminal_index,
        Err(_) => {
            return Err(ValidationError::ArithmeticOverflow {
                calculation: ArithmeticKind::IgnoredTerminalCount,
            });
        }
    };
    match table.get_mut(terminal_index) {
        Some(ignored) => {
            *ignored = 1;
            Ok(())
        }
        None => Err(ValidationError::ArithmeticOverflow {
            calculation: ArithmeticKind::IgnoredTerminalCount,
        }),
    }
}

struct TokenValidation {
    has_ordinary: bool,
    has_eos: bool,
    total_bytes: u64,
    work: u64,
}

fn validate_tokens(
    tokens: &[TokenEntry],
    initial_work: u64,
    limits: &ValidationLimits,
) -> Result<u64, ValidationError> {
    let mut state = TokenValidation {
        has_ordinary: false,
        has_eos: false,
        total_bytes: 0,
        work: initial_work,
    };
    let mut validation_error = None;
    let mut index = 0;
    while index < tokens.len() && validation_error.is_none() {
        validation_error = validate_token_entry(&tokens[index], index, &mut state, limits).err();
        index += 1;
    }

    match validation_error {
        Some(validation_error) => Err(validation_error),
        None => {
            if !state.has_ordinary {
                Err(ValidationError::MissingOrdinaryToken)
            } else if !state.has_eos {
                Err(ValidationError::MissingEosToken)
            } else {
                Ok(state.total_bytes)
            }
        }
    }
}

fn validate_token_entry(
    token: &TokenEntry,
    index: usize,
    state: &mut TokenValidation,
    limits: &ValidationLimits,
) -> Result<(), ValidationError> {
    state.work = checked_add(state.work, 1, ArithmeticKind::Work)?;
    check_limit(LimitKind::Work, state.work, limits.max_work)?;

    match token {
        TokenEntry::Bytes(bytes) => {
            state.has_ordinary = true;
            if bytes.is_empty() {
                return Err(ValidationError::EmptyTokenBytes {
                    token: TokenId::new(index_as_u32(index, ArithmeticKind::TokenCount)?),
                });
            }
            let byte_count =
                u64::try_from(bytes.len()).map_err(|_| ValidationError::ArithmeticOverflow {
                    calculation: ArithmeticKind::TokenBytes,
                })?;
            state.total_bytes =
                checked_add(state.total_bytes, byte_count, ArithmeticKind::TokenBytes)?;
            check_limit(
                LimitKind::TokenBytes,
                state.total_bytes,
                limits.max_token_bytes,
            )?;
            state.work = checked_add(state.work, byte_count, ArithmeticKind::Work)?;
            check_limit(LimitKind::Work, state.work, limits.max_work)
        }
        TokenEntry::Eos => {
            state.has_eos = true;
            Ok(())
        }
    }
}

fn validate_byte_classes(classes: &[u32], class_count: u32) -> Result<(), ValidationError> {
    let mut validation_error = None;
    let mut index = 0;
    while index < classes.len() && validation_error.is_none() {
        validation_error = validate_byte_class(index, classes[index], class_count).err();
        index += 1;
    }
    validation_result(validation_error)
}

fn validate_byte_class(
    index: usize,
    byte_class: u32,
    class_count: u32,
) -> Result<(), ValidationError> {
    if byte_class >= class_count {
        Err(ValidationError::ByteClassOutOfRange {
            byte: u8::try_from(index).map_err(|_| ValidationError::ArithmeticOverflow {
                calculation: ArithmeticKind::LexerCells,
            })?,
            class: byte_class,
            class_count,
        })
    } else {
        Ok(())
    }
}

fn validate_actions(
    actions: &[Action],
    state_count: u32,
    terminal_count: u32,
    production_count: u32,
    eof_terminal: TerminalId,
) -> Result<(), ValidationError> {
    let terminal_width = count_as_usize(terminal_count, ArithmeticKind::ParserActionCells)?;
    let mut validation_error = None;
    let mut index = 0;
    while index < actions.len() && validation_error.is_none() {
        validation_error = validate_action(
            actions[index],
            index,
            terminal_width,
            state_count,
            production_count,
            eof_terminal,
        )
        .err();
        index += 1;
    }
    validation_result(validation_error)
}

fn validate_action(
    action: Action,
    index: usize,
    terminal_width: usize,
    state_count: u32,
    production_count: u32,
    eof_terminal: TerminalId,
) -> Result<(), ValidationError> {
    match action {
        Action::Error => Ok(()),
        Action::Shift(destination) => validate_id(
            ValidationTable::ParserActions,
            Some(index),
            IdKind::ParserState,
            destination.get(),
            state_count,
        ),
        Action::Reduce {
            production,
            rank: _,
        } => validate_id(
            ValidationTable::ParserActions,
            Some(index),
            IdKind::Production,
            production.get(),
            production_count,
        ),
        Action::Accept => {
            let column = index % terminal_width;
            let terminal =
                TerminalId::new(index_as_u32(column, ArithmeticKind::ParserActionCells)?);
            if terminal != eof_terminal {
                let row = index / terminal_width;
                Err(ValidationError::AcceptOnNonEof {
                    state: ParserStateId::new(index_as_u32(
                        row,
                        ArithmeticKind::ParserActionCells,
                    )?),
                    terminal,
                })
            } else {
                Ok(())
            }
        }
    }
}

fn validate_ignored_terminals(
    ignored_terminals: &[TerminalId],
    terminal_count: u32,
    eof_terminal: TerminalId,
) -> Result<(), ValidationError> {
    let mut validation_error = None;
    let mut index = 0;
    while index < ignored_terminals.len() && validation_error.is_none() {
        validation_error = validate_ignored_terminal(
            ignored_terminals[index],
            index,
            terminal_count,
            eof_terminal,
        )
        .err();
        index += 1;
    }
    validation_result(validation_error)
}

fn validate_ignored_terminal(
    terminal: TerminalId,
    index: usize,
    terminal_count: u32,
    eof_terminal: TerminalId,
) -> Result<(), ValidationError> {
    validate_id(
        ValidationTable::ParserIgnoredTerminals,
        Some(index),
        IdKind::Terminal,
        terminal.get(),
        terminal_count,
    )?;
    if terminal == eof_terminal {
        Err(ValidationError::IgnoredTerminalIsParserEof { index, terminal })
    } else {
        Ok(())
    }
}

fn validation_result(validation_error: Option<ValidationError>) -> Result<(), ValidationError> {
    match validation_error {
        Some(validation_error) => Err(validation_error),
        None => Ok(()),
    }
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
    total = checked_add(
        total,
        checked_mul(
            u64::from(sizes.production_count),
            LOGICAL_PRODUCTION_BYTES,
            calculation,
        )?,
        calculation,
    )?;
    checked_add(
        total,
        checked_mul(
            u64::from(sizes.ignored_terminal_count),
            LOGICAL_ID_BYTES,
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
    total = checked_add(
        total,
        checked_mul(
            u64::from(sizes.parser_action_cells),
            LOGICAL_ACTION_WORK,
            calculation,
        )?,
        calculation,
    )?;
    total = checked_add(total, u64::from(sizes.parser_goto_cells), calculation)?;
    total = checked_add(total, u64::from(sizes.production_count), calculation)?;
    checked_add(total, u64::from(sizes.ignored_terminal_count), calculation)
}
