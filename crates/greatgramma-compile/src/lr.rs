use cfgrammar::{PIdx, Symbol, yacc::YaccGrammar};
use greatgramma_core::{
    Action, LalrDimensions, LalrTable, NonterminalId, ParserStateId, Production, ProductionId,
    TerminalId, ValidationLimits,
};
use lrtable::{Action as LrAction, Minimiser, StateTableErrorKind, from_yacc};

use crate::{CompileError, grammar::ParsedGrammar};

#[derive(Clone, Copy)]
struct ProgressEdge {
    destination: usize,
    strict: bool,
}

pub(crate) fn lower_lalr(
    parsed: &ParsedGrammar,
    limits: ValidationLimits,
) -> Result<LalrTable, CompileError> {
    let grammar = &parsed.yacc;
    check_grammar_dimensions(grammar, limits)?;
    let mut work = 0_u64;
    let (state_graph, state_table) =
        from_yacc(grammar, Minimiser::Pager).map_err(|error| match &error.kind {
            StateTableErrorKind::AcceptReduceConflict(_) => CompileError::GrammarConflict {
                shift_reduce: 1,
                reduce_reduce: 0,
            },
            _ => CompileError::GrammarTable {
                message: error.to_string(),
            },
        })?;

    if let Some(conflicts) = state_table.conflicts() {
        return Err(CompileError::GrammarConflict {
            shift_reduce: conflicts.sr_len(),
            reduce_reduce: conflicts.rr_len(),
        });
    }

    let state_count = usize::from(state_graph.all_states_len());
    let terminal_count = usize::from(grammar.tokens_len());
    let nonterminal_count = usize::from(grammar.rules_len());
    if state_count as u64 > limits.max_parser_states {
        return Err(CompileError::TooManyParserStates {
            required: state_count,
            maximum: limits.max_parser_states,
        });
    }
    check_cell_limit(state_count, terminal_count, nonterminal_count, limits)?;

    let action_cells =
        state_count
            .checked_mul(terminal_count)
            .ok_or(CompileError::AllocationFailure {
                requested: usize::MAX,
            })?;
    let goto_cells =
        state_count
            .checked_mul(nonterminal_count)
            .ok_or(CompileError::AllocationFailure {
                requested: usize::MAX,
            })?;
    let mut actions = filled_vec(action_cells, Action::Error)?;
    let mut gotos = filled_vec(goto_cells, None)?;

    for state in state_graph.iter_stidxs() {
        let state_index = usize::from(state);
        charge_dense(&mut work, terminal_count as u64, limits.max_work)?;
        for terminal in state_table.state_actions(state) {
            actions[state_index * terminal_count + usize::from(terminal)] =
                match state_table.action(state, terminal) {
                    LrAction::Shift(destination) => {
                        Action::Shift(ParserStateId::new(u32::from(destination)))
                    }
                    LrAction::Reduce(production) => Action::Reduce {
                        production: ProductionId::new(u32::from(production)),
                        rank: 0,
                    },
                    LrAction::Accept => Action::Accept,
                    LrAction::Error => Action::Error,
                };
        }
        charge_dense(&mut work, nonterminal_count as u64, limits.max_work)?;
        for (symbol, destination) in state_graph.edges(state) {
            if let Symbol::Rule(rule) = symbol {
                gotos[state_index * nonterminal_count + usize::from(*rule)] =
                    Some(ParserStateId::new(u32::from(*destination)));
            }
        }
    }

    let start_state = ParserStateId::new(u32::from(state_table.start_state()));
    drop(state_table);
    drop(state_graph);

    let ranks = reduction_ranks(
        grammar,
        &actions,
        &gotos,
        state_count,
        terminal_count,
        limits,
        &mut work,
    )?;
    for (cell, rank) in actions.iter_mut().zip(ranks) {
        if let Action::Reduce { rank: stored, .. } = cell {
            *stored = rank.ok_or(CompileError::NoReductionProgressWitness)?;
        }
    }

    let mut productions = reserved_vec(usize::from(grammar.prods_len()))?;
    for production in grammar.iter_pidxs() {
        fallible_push(
            &mut productions,
            Production::new(
                NonterminalId::new(u32::from(grammar.prod_to_rule(production))),
                u32::from(grammar.prod_len(production)),
            ),
        )?;
    }
    let ignored = fallible_copy(&parsed.ignored)?;

    Ok(LalrTable::new(
        LalrDimensions::new(
            state_count as u32,
            terminal_count as u32,
            nonterminal_count as u32,
        ),
        start_state,
        TerminalId::new(u32::from(grammar.eof_token_idx())),
        actions,
        gotos,
        productions,
    )
    .with_ignored_terminals(ignored))
}

fn check_grammar_dimensions(
    grammar: &YaccGrammar<u32>,
    limits: ValidationLimits,
) -> Result<(), CompileError> {
    let terminals = u64::from(u32::from(grammar.tokens_len()));
    let nonterminals = u64::from(u32::from(grammar.rules_len()));
    let productions = u64::from(u32::from(grammar.prods_len()));
    if terminals > limits.max_terminals
        || nonterminals > limits.max_nonterminals
        || productions > limits.max_productions
    {
        return Err(CompileError::GrammarTooLarge {
            terminals,
            nonterminals,
            productions,
        });
    }
    check_cell_limit(1, terminals as usize, nonterminals as usize, limits)
}

fn filled_vec<T: Clone>(length: usize, value: T) -> Result<Vec<T>, CompileError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| CompileError::AllocationFailure { requested: length })?;
    output.resize(length, value);
    Ok(output)
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

fn fallible_copy<T: Copy>(source: &[T]) -> Result<Vec<T>, CompileError> {
    let mut output = reserved_vec(source.len())?;
    for value in source {
        fallible_push(&mut output, *value)?;
    }
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

fn charge(work: &mut u64, amount: u64, maximum: u64) -> Result<(), CompileError> {
    *work = work.saturating_add(amount);
    if *work > maximum {
        return Err(CompileError::CompilerWorkLimit {
            required: *work,
            maximum,
        });
    }
    Ok(())
}

fn charge_dense(work: &mut u64, amount: u64, maximum: u64) -> Result<(), CompileError> {
    if maximum == u64::MAX {
        *work = work.saturating_add(amount);
        return Ok(());
    }

    let remaining = maximum.saturating_sub(*work);
    if amount > remaining {
        *work = maximum + 1;
        return Err(CompileError::CompilerWorkLimit {
            required: *work,
            maximum,
        });
    }

    *work += amount;
    Ok(())
}

fn check_cell_limit(
    states: usize,
    terminals: usize,
    nonterminals: usize,
    limits: ValidationLimits,
) -> Result<(), CompileError> {
    let required =
        (states as u64).saturating_mul((terminals as u64).saturating_add(nonterminals as u64));
    if required > limits.max_parser_cells {
        return Err(CompileError::TooManyParserCells {
            required,
            maximum: limits.max_parser_cells,
        });
    }
    Ok(())
}

fn reduction_ranks(
    grammar: &YaccGrammar<u32>,
    actions: &[Action],
    gotos: &[Option<ParserStateId>],
    state_count: usize,
    terminal_count: usize,
    limits: ValidationLimits,
    work: &mut u64,
) -> Result<Vec<Option<u32>>, CompileError> {
    let mut node_for_cell = filled_vec(actions.len(), None)?;
    let mut cells = Vec::new();
    for (cell, action) in actions.iter().enumerate() {
        charge(work, 1, limits.max_work)?;
        if matches!(action, Action::Reduce { .. }) {
            node_for_cell[cell] = Some(cells.len());
            fallible_push(&mut cells, cell)?;
        }
    }

    let nonterminal_count = usize::from(grammar.rules_len());
    let mut edges = filled_vec(cells.len(), Vec::new())?;
    for (node, cell) in cells.iter().copied().enumerate() {
        charge(work, 1, limits.max_work)?;
        let Action::Reduce { production, .. } = actions[cell] else {
            continue;
        };
        let state = cell / terminal_count;
        let terminal = cell % terminal_count;
        let production = PIdx(production.get());
        let lhs = usize::from(grammar.prod_to_rule(production));
        let pop_len = usize::from(grammar.prod_len(production));
        let strict = pop_len < 2;
        if pop_len == 0 {
            add_progress_edge(
                &mut edges[node],
                gotos,
                state,
                lhs,
                terminal,
                terminal_count,
                nonterminal_count,
                &node_for_cell,
                strict,
                work,
                limits.max_work,
            )?;
        } else {
            for source in 0..state_count {
                charge(work, 1, limits.max_work)?;
                add_progress_edge(
                    &mut edges[node],
                    gotos,
                    source,
                    lhs,
                    terminal,
                    terminal_count,
                    nonterminal_count,
                    &node_for_cell,
                    strict,
                    work,
                    limits.max_work,
                )?;
            }
        }
    }

    let ranks = progress_ranks(&edges, work, limits.max_work)?;
    let mut by_cell = filled_vec(actions.len(), None)?;
    for (node, cell) in cells.into_iter().enumerate() {
        charge(work, 1, limits.max_work)?;
        by_cell[cell] = Some(ranks[node]);
    }
    Ok(by_cell)
}

#[allow(clippy::too_many_arguments)]
fn add_progress_edge(
    edges: &mut Vec<ProgressEdge>,
    gotos: &[Option<ParserStateId>],
    source: usize,
    lhs: usize,
    terminal: usize,
    terminal_count: usize,
    nonterminal_count: usize,
    node_for_cell: &[Option<usize>],
    strict: bool,
    work: &mut u64,
    maximum_work: u64,
) -> Result<(), CompileError> {
    charge(work, 2, maximum_work)?;
    let Some(destination) = gotos[source * nonterminal_count + lhs] else {
        return Ok(());
    };
    let next_cell = destination.get() as usize * terminal_count + terminal;
    let Some(next_node) = node_for_cell[next_cell] else {
        return Ok(());
    };
    charge(work, edges.len() as u64, maximum_work)?;
    if !edges.iter().any(|edge| edge.destination == next_node) {
        fallible_push(
            edges,
            ProgressEdge {
                destination: next_node,
                strict,
            },
        )?;
    }
    Ok(())
}

fn progress_ranks(
    edges: &[Vec<ProgressEdge>],
    work: &mut u64,
    maximum_work: u64,
) -> Result<Vec<u32>, CompileError> {
    let components = strongly_connected_components(edges, work, maximum_work)?;
    for (source, outgoing) in edges.iter().enumerate() {
        for edge in outgoing {
            charge(work, 1, maximum_work)?;
            if edge.strict && components[source] == components[edge.destination] {
                return Err(CompileError::NoReductionProgressWitness);
            }
        }
    }

    charge(work, components.len() as u64, maximum_work)?;
    let component_count = components
        .iter()
        .copied()
        .max()
        .map_or(0, |maximum| maximum.saturating_add(1));
    let mut component_edges: Vec<Vec<ProgressEdge>> = filled_vec(component_count, Vec::new())?;
    for (source, outgoing) in edges.iter().enumerate() {
        let source_component = components[source];
        for edge in outgoing {
            charge(work, 1, maximum_work)?;
            let destination_component = components[edge.destination];
            if source_component == destination_component {
                continue;
            }
            let known_edges = &mut component_edges[source_component];
            charge(work, known_edges.len() as u64, maximum_work)?;
            match known_edges
                .iter_mut()
                .find(|known| known.destination == destination_component)
            {
                Some(known) => known.strict |= edge.strict,
                None => fallible_push(
                    known_edges,
                    ProgressEdge {
                        destination: destination_component,
                        strict: edge.strict,
                    },
                )?,
            }
        }
    }

    let component_ranks = dag_ranks(&component_edges, work, maximum_work)?;
    let mut ranks = reserved_vec(components.len())?;
    for component in components {
        charge(work, 1, maximum_work)?;
        let rank = *component_ranks
            .get(component)
            .ok_or(CompileError::NoReductionProgressWitness)?;
        fallible_push(&mut ranks, rank)?;
    }
    Ok(ranks)
}

fn strongly_connected_components(
    edges: &[Vec<ProgressEdge>],
    work: &mut u64,
    maximum_work: u64,
) -> Result<Vec<usize>, CompileError> {
    let order = finishing_order(edges, work, maximum_work)?;
    let mut reverse = filled_vec(edges.len(), Vec::new())?;
    for (source, outgoing) in edges.iter().enumerate() {
        for edge in outgoing {
            charge(work, 1, maximum_work)?;
            fallible_push(&mut reverse[edge.destination], source)?;
        }
    }

    let mut components = filled_vec(edges.len(), usize::MAX)?;
    let mut component = 0_usize;
    for start in order.into_iter().rev() {
        charge(work, 1, maximum_work)?;
        if components[start] != usize::MAX {
            continue;
        }
        components[start] = component;
        let mut stack = reserved_vec(1)?;
        fallible_push(&mut stack, start)?;
        while let Some(node) = stack.pop() {
            charge(work, 1, maximum_work)?;
            for predecessor in &reverse[node] {
                charge(work, 1, maximum_work)?;
                if components[*predecessor] == usize::MAX {
                    components[*predecessor] = component;
                    fallible_push(&mut stack, *predecessor)?;
                }
            }
        }
        component = component.saturating_add(1);
    }
    Ok(components)
}

fn finishing_order(
    edges: &[Vec<ProgressEdge>],
    work: &mut u64,
    maximum_work: u64,
) -> Result<Vec<usize>, CompileError> {
    let mut visited = filled_vec(edges.len(), false)?;
    let mut order = reserved_vec(edges.len())?;
    for start in 0..edges.len() {
        charge(work, 1, maximum_work)?;
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = reserved_vec(1)?;
        fallible_push(&mut stack, (start, 0_usize))?;
        while !stack.is_empty() {
            charge(work, 1, maximum_work)?;
            let last = stack.len() - 1;
            let (node, next_index) = stack[last];
            if let Some(edge) = edges[node].get(next_index) {
                stack[last].1 += 1;
                if !visited[edge.destination] {
                    visited[edge.destination] = true;
                    fallible_push(&mut stack, (edge.destination, 0))?;
                }
            } else {
                stack.pop();
                fallible_push(&mut order, node)?;
            }
        }
    }
    Ok(order)
}

fn dag_ranks(
    edges: &[Vec<ProgressEdge>],
    work: &mut u64,
    maximum_work: u64,
) -> Result<Vec<u32>, CompileError> {
    let mut indegree = filled_vec(edges.len(), 0_usize)?;
    for destinations in edges {
        for edge in destinations {
            charge(work, 1, maximum_work)?;
            indegree[edge.destination] = indegree[edge.destination].saturating_add(1);
        }
    }
    let mut queue = Vec::new();
    for (node, degree) in indegree.iter().copied().enumerate() {
        charge(work, 1, maximum_work)?;
        if degree == 0 {
            fallible_push(&mut queue, node)?;
        }
    }
    let mut order = reserved_vec(edges.len())?;
    let mut cursor = 0_usize;
    while cursor < queue.len() {
        charge(work, 1, maximum_work)?;
        let node = queue[cursor];
        cursor += 1;
        fallible_push(&mut order, node)?;
        for edge in &edges[node] {
            charge(work, 1, maximum_work)?;
            indegree[edge.destination] -= 1;
            if indegree[edge.destination] == 0 {
                fallible_push(&mut queue, edge.destination)?;
            }
        }
    }
    if order.len() != edges.len() {
        return Err(CompileError::NoReductionProgressWitness);
    }

    let mut ranks = filled_vec(edges.len(), 0_u32)?;
    for node in order.into_iter().rev() {
        charge(work, 1, maximum_work)?;
        for edge in &edges[node] {
            charge(work, 1, maximum_work)?;
            let increment = u32::from(edge.strict);
            ranks[node] = ranks[node].max(
                ranks[edge.destination]
                    .checked_add(increment)
                    .ok_or(CompileError::NoReductionProgressWitness)?,
            );
        }
    }
    Ok(ranks)
}

#[cfg(test)]
mod tests {
    use super::{CompileError, ProgressEdge, progress_ranks};

    fn ranks(edges: &[Vec<ProgressEdge>]) -> Result<Vec<u32>, CompileError> {
        progress_ranks(edges, &mut 0, u64::MAX)
    }

    #[test]
    fn equal_rank_cycles_are_allowed_only_for_stack_shrinking_reductions() {
        let shrinking_cycle = vec![vec![ProgressEdge {
            destination: 0,
            strict: false,
        }]];
        assert_eq!(ranks(&shrinking_cycle), Ok(vec![0]));

        let nonshrinking_cycle = vec![vec![ProgressEdge {
            destination: 0,
            strict: true,
        }]];
        assert_eq!(
            ranks(&nonshrinking_cycle),
            Err(CompileError::NoReductionProgressWitness)
        );
    }

    #[test]
    fn strict_edges_receive_descending_ranks() {
        let edges = vec![
            vec![ProgressEdge {
                destination: 1,
                strict: false,
            }],
            vec![ProgressEdge {
                destination: 2,
                strict: true,
            }],
            Vec::new(),
        ];
        assert_eq!(ranks(&edges), Ok(vec![1, 1, 0]));
    }

    #[test]
    fn graph_scans_are_charged_to_the_work_limit() {
        let edges = vec![vec![ProgressEdge {
            destination: 0,
            strict: false,
        }]];
        assert!(matches!(
            progress_ranks(&edges, &mut 0, 0),
            Err(CompileError::CompilerWorkLimit { .. })
        ));
    }
}
