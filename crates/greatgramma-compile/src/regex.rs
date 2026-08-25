use greatgramma_core::TerminalId;
use llguidance::{
    api::ParserLimits,
    derivre::RegexAst,
    earley::{
        lexerspec::LexerSpec,
        regexvec::{RegexVec, StateID},
    },
};
use regex_syntax::ParserBuilder as SyntaxParserBuilder;

use crate::{CompileError, grammar::ResolvedTerminal};

pub(crate) struct TerminalMeta {
    pub(crate) name: String,
    pub(crate) priority: u32,
    pub(crate) terminal: TerminalId,
}

pub(crate) struct CompiledRegexes {
    pub(crate) regexes: RegexVec,
    pub(crate) start: StateID,
    pub(crate) aliases: Vec<Option<TerminalMeta>>,
    pub(crate) max_output_bytes: usize,
}

pub(crate) fn compile_regexes(
    terminals: Vec<ResolvedTerminal>,
    max_fuel: u64,
    max_output_bytes: usize,
) -> Result<CompiledRegexes, CompileError> {
    let mut spec = LexerSpec::new().map_err(|error| internal_regex_error(error.to_string()))?;
    spec.regex_builder.unicode(false).utf8(false);
    spec.setup_lexeme_class(RegexAst::NoMatch)
        .map_err(|error| internal_regex_error(error.to_string()))?;
    check_regex_fuel(spec.cost(), max_fuel)?;

    let mut bindings = reserved_vec(terminals.len())?;
    let mut registration_scan_cost = 0_u64;
    for terminal in terminals {
        if let Some(feature) = unsupported_feature(&terminal.pattern) {
            return Err(CompileError::UnsupportedRegex {
                name: terminal.name,
                feature: fallible_string_copy(feature)?,
            });
        }

        let lexeme_name = fallible_string_copy(&terminal.name)?;
        registration_scan_cost = registration_scan_cost
            .saturating_add(u64::try_from(spec.lexemes.len()).unwrap_or(u64::MAX));
        check_regex_fuel(registration_scan_cost.saturating_add(spec.cost()), max_fuel)?;
        let index = match spec.add_greedy_lexeme(
            lexeme_name,
            RegexAst::Regex(terminal.pattern),
            false,
            None,
            usize::MAX,
        ) {
            Ok(index) => index,
            Err(error) => {
                return Err(CompileError::InvalidRegex {
                    name: terminal.name,
                    message: error.to_string(),
                });
            }
        };
        check_regex_fuel(registration_scan_cost.saturating_add(spec.cost()), max_fuel)?;
        if spec.is_nullable(index) {
            return Err(CompileError::NullableTerminal {
                name: terminal.name,
            });
        }
        fallible_push(
            &mut bindings,
            (
                index,
                TerminalMeta {
                    name: terminal.name,
                    priority: terminal.priority,
                    terminal: terminal.terminal,
                },
            ),
        )?;
    }

    let mut selected = spec.alloc_lexeme_set();
    let mut aliases = reserved_vec(spec.lexemes.len())?;
    for _ in 0..spec.lexemes.len() {
        fallible_push(&mut aliases, Vec::<TerminalMeta>::new())?;
    }
    for (index, terminal) in bindings {
        selected.add(index);
        fallible_push(&mut aliases[index.as_usize()], terminal)?;
    }

    let registration_and_spec_fuel = registration_scan_cost.saturating_add(spec.cost());
    check_regex_fuel(registration_and_spec_fuel, max_fuel)?;
    let mut limits = ParserLimits {
        initial_lexer_fuel: max_fuel - registration_and_spec_fuel,
        max_lexer_states: usize::MAX,
        ..ParserLimits::default()
    };
    let mut regexes = match spec.to_regex_vec(&mut limits) {
        Ok(regexes) => regexes,
        Err(error) => {
            let message = error.to_string();
            // llguidance 1.8.0 erases the private relevance error into this message.
            if message.starts_with("fuel exhausted when checking relevance of lexemes") {
                return Err(CompileError::CompilerWorkLimit {
                    required: max_fuel.saturating_add(1),
                    maximum: max_fuel,
                });
            }
            return Err(internal_regex_error(message));
        }
    };
    let required_fuel = registration_scan_cost.saturating_add(regexes.total_fuel_spent());
    check_regex_fuel(required_fuel, max_fuel)?;

    let start = regexes.initial_state(&selected);
    for index in selected.iter() {
        if !regexes.state_desc(start).possible.contains(index) {
            let terminal = aliases[index.as_usize()]
                .first()
                .expect("selected lexeme has a terminal alias");
            return Err(CompileError::UnmatchableTerminal {
                name: fallible_string_copy(&terminal.name)?,
            });
        }
    }
    let aliases = collapse_aliases(aliases)?;

    Ok(CompiledRegexes {
        regexes,
        start,
        aliases,
        max_output_bytes,
    })
}

fn collapse_aliases(
    aliases: Vec<Vec<TerminalMeta>>,
) -> Result<Vec<Option<TerminalMeta>>, CompileError> {
    let mut collapsed = reserved_vec(aliases.len())?;
    for mut group in aliases {
        if group.is_empty() {
            fallible_push(&mut collapsed, None)?;
            continue;
        }

        let winner_index = group
            .iter()
            .enumerate()
            .min_by_key(|(_, terminal)| terminal.priority)
            .map(|(index, _)| index)
            .expect("nonempty alias group has a priority winner");
        let winner_priority = group[winner_index].priority;
        if let Some((_, tied)) = group.iter().enumerate().find(|(index, terminal)| {
            *index != winner_index && terminal.priority == winner_priority
        }) {
            return Err(CompileError::AmbiguousPriority {
                first: fallible_string_copy(&group[winner_index].name)?,
                second: fallible_string_copy(&tied.name)?,
                priority: winner_priority,
            });
        }

        fallible_push(&mut collapsed, Some(group.swap_remove(winner_index)))?;
    }
    Ok(collapsed)
}

fn internal_regex_error(message: String) -> CompileError {
    CompileError::InvalidRegex {
        name: "<lexer>".to_owned(),
        message,
    }
}

fn check_regex_fuel(required: u64, maximum: u64) -> Result<(), CompileError> {
    if required > maximum {
        return Err(CompileError::CompilerWorkLimit { required, maximum });
    }
    Ok(())
}

fn unsupported_feature(pattern: &str) -> Option<&'static str> {
    if pattern.contains("(?") {
        return Some("lookaround or inline flags");
    }
    if pattern.contains("[^")
        || pattern.contains(r"\P{")
        || pattern.contains(r"\D")
        || pattern.contains(r"\S")
        || pattern.contains(r"\W")
    {
        return Some("complement character classes");
    }
    let mut parser = SyntaxParserBuilder::new();
    parser.unicode(false).utf8(false);
    if parser
        .build()
        .parse(pattern)
        .is_ok_and(|expression| !expression.properties().look_set().is_empty())
    {
        return Some("anchors or boundary assertions");
    }
    None
}

fn reserved_vec<T>(capacity: usize) -> Result<Vec<T>, CompileError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| CompileError::AllocationFailure {
            requested: capacity,
        })?;
    Ok(output)
}

fn fallible_push<T>(output: &mut Vec<T>, value: T) -> Result<(), CompileError> {
    if output.len() == output.capacity() {
        output
            .try_reserve(1)
            .map_err(|_| CompileError::AllocationFailure {
                requested: output.len().saturating_add(1),
            })?;
    }
    output.push(value);
    Ok(())
}

fn fallible_string_copy(source: &str) -> Result<String, CompileError> {
    let mut output = String::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| CompileError::AllocationFailure {
            requested: source.len(),
        })?;
    output.push_str(source);
    Ok(output)
}
