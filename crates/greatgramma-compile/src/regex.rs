use derivre::{Regex, RegexBuilder};
use greatgramma_core::TerminalId;
use regex_syntax::ParserBuilder as SyntaxParserBuilder;

use crate::{CompileError, grammar::ResolvedTerminal};

pub(crate) struct CompiledTerminal {
    pub(crate) name: String,
    pub(crate) priority: u32,
    pub(crate) terminal: TerminalId,
    pub(crate) regex: Regex,
}

pub(crate) fn compile_regexes(
    terminals: Vec<ResolvedTerminal>,
    max_fuel: u64,
    max_output_bytes: usize,
) -> Result<Vec<CompiledTerminal>, CompileError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(terminals.len())
        .map_err(|_| CompileError::AllocationFailure {
            requested: terminals.len(),
        })?;
    let mut work = 0_u64;
    let mut output_bytes = 0_usize;
    for terminal in terminals {
        let remaining = max_fuel.saturating_sub(work);
        let compiled = compile_regex(terminal, remaining, work, max_fuel)?;
        work = work.saturating_add(compiled.regex.cost());
        if work > max_fuel {
            return Err(CompileError::CompilerWorkLimit {
                required: work,
                maximum: max_fuel,
            });
        }
        output_bytes = output_bytes.saturating_add(compiled.regex.num_bytes());
        if output_bytes > max_output_bytes {
            return Err(CompileError::RegexOutputLimit {
                required: output_bytes,
                maximum: max_output_bytes,
            });
        }
        output.push(compiled);
    }
    Ok(output)
}

fn compile_regex(
    terminal: ResolvedTerminal,
    remaining_fuel: u64,
    spent_fuel: u64,
    maximum_fuel: u64,
) -> Result<CompiledTerminal, CompileError> {
    if let Some(feature) = unsupported_feature(&terminal.pattern) {
        return Err(CompileError::UnsupportedRegex {
            name: terminal.name,
            feature: feature.to_owned(),
        });
    }

    let mut builder = RegexBuilder::new();
    builder.unicode(false).utf8(false);
    let expression =
        builder
            .mk_regex(&terminal.pattern)
            .map_err(|error| CompileError::InvalidRegex {
                name: terminal.name.clone(),
                message: error.to_string(),
            })?;
    if builder.is_nullable(expression) {
        return Err(CompileError::NullableTerminal {
            name: terminal.name,
        });
    }
    let mut regex = builder
        .into_regex_limited(expression, remaining_fuel)
        .map_err(|_| CompileError::CompilerWorkLimit {
            required: spent_fuel.saturating_add(remaining_fuel).saturating_add(1),
            maximum: maximum_fuel,
        })?;
    if regex.always_empty() {
        return Err(CompileError::UnmatchableTerminal {
            name: terminal.name,
        });
    }

    Ok(CompiledTerminal {
        name: terminal.name,
        priority: terminal.priority,
        terminal: terminal.terminal,
        regex,
    })
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
