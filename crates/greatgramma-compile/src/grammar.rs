use std::collections::HashMap;

use cfgrammar::{
    Symbol, TIdx,
    yacc::{YaccGrammar, YaccKind, YaccOriginalActionKind},
};
use greatgramma_core::TerminalId;

use crate::{CompileError, SourceGrammar, TerminalSpec};

pub(crate) struct ResolvedTerminal {
    pub(crate) name: String,
    pub(crate) pattern: String,
    pub(crate) priority: u32,
    pub(crate) terminal: TerminalId,
}

pub(crate) struct ParsedGrammar {
    pub(crate) yacc: YaccGrammar<u32>,
    pub(crate) terminals: Vec<ResolvedTerminal>,
    pub(crate) ignored: Vec<TerminalId>,
}

struct TerminalLookup {
    token: TIdx<u32>,
    spec: Option<usize>,
    ignored: bool,
    used: bool,
}

pub(crate) fn parse_source(
    source: SourceGrammar,
    maximum_terminals: usize,
) -> Result<ParsedGrammar, CompileError> {
    reject_unsupported_directives(&source.yacc)?;
    let yacc = YaccGrammar::<u32>::new(
        YaccKind::Original(YaccOriginalActionKind::NoAction),
        &source.yacc,
    )
    .map_err(|errors| CompileError::GrammarSyntax {
        messages: errors.into_iter().map(|error| error.to_string()).collect(),
    })?;
    if yacc
        .iter_pidxs()
        .any(|production| yacc.action(production).is_some())
        || yacc
            .iter_rules()
            .any(|rule| yacc.actiontype(rule).is_some())
        || yacc.parse_param().is_some()
        || yacc.parse_generics().is_some()
        || yacc.programs().is_some()
    {
        return Err(CompileError::UnsupportedGrammar {
            feature: "semantic actions".to_owned(),
        });
    }
    if yacc.expect().is_some()
        || yacc.expectrr().is_some()
        || yacc.iter_tidxs().any(|token| yacc.avoid_insert(token))
    {
        return Err(CompileError::UnsupportedGrammar {
            feature: "Yacc metadata directives".to_owned(),
        });
    }
    if yacc
        .iter_tidxs()
        .any(|token| yacc.token_precedence(token).is_some())
        || yacc
            .iter_pidxs()
            .any(|production| yacc.prod_precedence(production).is_some())
    {
        return Err(CompileError::UnsupportedGrammar {
            feature: "precedence declarations".to_owned(),
        });
    }

    let named_terminal_count = yacc
        .iter_tidxs()
        .filter(|token| yacc.token_name(*token).is_some())
        .count();
    if named_terminal_count > maximum_terminals {
        return Err(CompileError::TooManyTerminalSpecs {
            count: named_terminal_count,
            maximum: maximum_terminals,
        });
    }
    let mut index = HashMap::new();
    index
        .try_reserve(named_terminal_count)
        .map_err(|_| CompileError::AllocationFailure {
            requested: named_terminal_count,
        })?;
    for token in yacc.iter_tidxs() {
        if let Some(name) = yacc.token_name(token) {
            index.insert(
                name,
                TerminalLookup {
                    token,
                    spec: None,
                    ignored: false,
                    used: false,
                },
            );
        }
    }
    for production in yacc.iter_pidxs() {
        for symbol in yacc.prod(production) {
            if let Symbol::Token(token) = symbol
                && let Some(name) = yacc.token_name(*token)
                && let Some(entry) = index.get_mut(name)
            {
                entry.used = true;
            }
        }
    }
    for (spec_index, spec) in source.terminals.iter().enumerate() {
        let entry =
            index
                .get_mut(spec.name.as_str())
                .ok_or_else(|| CompileError::UnknownTerminal {
                    name: spec.name.clone(),
                })?;
        if entry.spec.replace(spec_index).is_some() {
            return Err(CompileError::DuplicateTerminal {
                name: spec.name.clone(),
            });
        }
    }

    let mut terminals = reserved_vec(named_terminal_count)?;
    for token in yacc.iter_tidxs() {
        let Some(name) = yacc.token_name(token) else {
            continue;
        };
        let spec_index = index
            .get(name)
            .and_then(|entry| entry.spec)
            .ok_or_else(|| CompileError::MissingTerminal {
                name: name.to_owned(),
            })?;
        terminals.push(resolve_terminal(&source.terminals[spec_index], token));
    }

    let ignored = resolve_ignored(&mut index, &source.ignored_terminals)?;
    Ok(ParsedGrammar {
        yacc,
        terminals,
        ignored,
    })
}

fn reject_unsupported_directives(source: &str) -> Result<(), CompileError> {
    const DECLARATIONS: [&str; 4] = ["%left", "%right", "%nonassoc", "%precedence"];
    for word in source.split_whitespace() {
        if word == "%start" || word == "%token" || word == "%%" {
            continue;
        }
        if word == "%prec"
            || DECLARATIONS
                .iter()
                .any(|declaration| word.starts_with(declaration))
        {
            return Err(CompileError::UnsupportedGrammar {
                feature: "precedence declarations".to_owned(),
            });
        }
        if word.starts_with('%') {
            return Err(CompileError::UnsupportedGrammar {
                feature: format!("Yacc directive {word}"),
            });
        }
    }
    Ok(())
}

fn resolve_terminal(spec: &TerminalSpec, token: TIdx<u32>) -> ResolvedTerminal {
    ResolvedTerminal {
        name: spec.name.clone(),
        pattern: spec.pattern.clone(),
        priority: spec.priority,
        terminal: TerminalId::new(u32::from(token)),
    }
}

fn resolve_ignored(
    index: &mut HashMap<&str, TerminalLookup>,
    names: &[String],
) -> Result<Vec<TerminalId>, CompileError> {
    let mut ignored = reserved_vec(names.len())?;
    for name in names {
        let entry = index
            .get_mut(name.as_str())
            .ok_or_else(|| CompileError::UnknownIgnoredTerminal { name: name.clone() })?;
        if entry.ignored {
            return Err(CompileError::DuplicateIgnoredTerminal { name: name.clone() });
        }
        entry.ignored = true;
        if entry.used {
            return Err(CompileError::IgnoredTerminalUsed { name: name.clone() });
        }
        ignored.push(TerminalId::new(u32::from(entry.token)));
    }
    Ok(ignored)
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
