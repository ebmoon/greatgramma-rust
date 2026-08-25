use cfgrammar::{PIdx, Symbol, yacc::YaccGrammar};
use greatgramma_core::{
    Action, LalrDimensions, LalrTable, NonterminalId, ParserStateId, Production, ProductionId,
    TerminalId, ValidationLimits,
};

use crate::{CompileError, grammar::ParsedGrammar};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Item {
    production: PIdx<u32>,
    dot: usize,
}

type State = Vec<Item>;
type StateTransitions = Vec<(Symbol<u32>, usize)>;

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
    let (states, transitions) = build_slr_states(grammar, limits, &mut work)?;

    let state_count = states.len();
    let terminal_count = usize::from(grammar.tokens_len());
    let nonterminal_count = usize::from(grammar.rules_len());
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
    let mut shift_reduce = 0_usize;
    let mut reduce_reduce = 0_usize;

    for (state, edges) in transitions.iter().enumerate() {
        charge(&mut work, edges.len() as u64, limits.max_work)?;
        for &(symbol, destination) in edges {
            match symbol {
                Symbol::Token(terminal) => set_action(
                    &mut actions[state * terminal_count + usize::from(terminal)],
                    Action::Shift(ParserStateId::new(destination as u32)),
                    &mut shift_reduce,
                    &mut reduce_reduce,
                ),
                Symbol::Rule(rule) => {
                    gotos[state * nonterminal_count + usize::from(rule)] =
                        Some(ParserStateId::new(destination as u32));
                }
            }
        }
    }

    let follows = compute_follows(grammar, limits, &mut work)?;
    for (state, items) in states.iter().enumerate() {
        charge(&mut work, items.len() as u64, limits.max_work)?;
        for item in items {
            if item.dot != grammar.prod(item.production).len() {
                continue;
            }
            if item.production == grammar.start_prod() {
                let cell = state * terminal_count + usize::from(grammar.eof_token_idx());
                set_action(
                    &mut actions[cell],
                    Action::Accept,
                    &mut shift_reduce,
                    &mut reduce_reduce,
                );
                continue;
            }
            let lhs = grammar.prod_to_rule(item.production);
            for &terminal in &follows[usize::from(lhs)] {
                charge(&mut work, 1, limits.max_work)?;
                let cell = state * terminal_count + terminal;
                set_action(
                    &mut actions[cell],
                    Action::Reduce {
                        production: ProductionId::new(u32::from(item.production)),
                        rank: 0,
                    },
                    &mut shift_reduce,
                    &mut reduce_reduce,
                );
            }
        }
    }

    if shift_reduce != 0 || reduce_reduce != 0 {
        return Err(CompileError::GrammarConflict {
            shift_reduce,
            reduce_reduce,
        });
    }

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
        ParserStateId::new(0),
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

fn compute_follows(
    grammar: &YaccGrammar<u32>,
    limits: ValidationLimits,
    work: &mut u64,
) -> Result<Vec<Vec<usize>>, CompileError> {
    let rule_count = usize::from(grammar.rules_len());
    let mut nullable = filled_vec(rule_count, false)?;
    loop {
        let mut changed = false;
        for production in grammar.iter_pidxs() {
            charge(work, 1, limits.max_work)?;
            let mut production_nullable = true;
            for symbol in grammar.prod(production) {
                charge(work, 1, limits.max_work)?;
                match symbol {
                    Symbol::Token(_) => {
                        production_nullable = false;
                        break;
                    }
                    Symbol::Rule(rule) if !nullable[usize::from(*rule)] => {
                        production_nullable = false;
                        break;
                    }
                    Symbol::Rule(_) => {}
                }
            }
            let lhs = usize::from(grammar.prod_to_rule(production));
            if production_nullable && !nullable[lhs] {
                nullable[lhs] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut fact_count = 0_u64;
    let mut first = filled_vec(rule_count, Vec::new())?;
    loop {
        let mut changed = false;
        for production in grammar.iter_pidxs() {
            let lhs = usize::from(grammar.prod_to_rule(production));
            for symbol in grammar.prod(production) {
                charge(work, 1, limits.max_work)?;
                match symbol {
                    Symbol::Token(terminal) => {
                        changed |= insert_fact(
                            &mut first[lhs],
                            usize::from(*terminal),
                            work,
                            limits.max_work,
                            &mut fact_count,
                            limits.max_parser_cells,
                        )?;
                        break;
                    }
                    Symbol::Rule(rule) => {
                        let source_ref = &first[usize::from(*rule)];
                        charge(work, source_ref.len() as u64, limits.max_work)?;
                        let source = fallible_copy(source_ref)?;
                        for terminal in source {
                            charge(work, 1, limits.max_work)?;
                            changed |= insert_fact(
                                &mut first[lhs],
                                terminal,
                                work,
                                limits.max_work,
                                &mut fact_count,
                                limits.max_parser_cells,
                            )?;
                        }
                        if !nullable[usize::from(*rule)] {
                            break;
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut follows = filled_vec(rule_count, Vec::new())?;
    insert_fact(
        &mut follows[usize::from(grammar.start_rule_idx())],
        usize::from(grammar.eof_token_idx()),
        work,
        limits.max_work,
        &mut fact_count,
        limits.max_parser_cells,
    )?;
    loop {
        let mut changed = false;
        for production in grammar.iter_pidxs() {
            let lhs = usize::from(grammar.prod_to_rule(production));
            let symbols = grammar.prod(production);
            for (position, symbol) in symbols.iter().enumerate() {
                let Symbol::Rule(target) = symbol else {
                    continue;
                };
                charge(work, 1, limits.max_work)?;
                let target = usize::from(*target);
                let mut suffix_nullable = true;
                for suffix in &symbols[position + 1..] {
                    charge(work, 1, limits.max_work)?;
                    match suffix {
                        Symbol::Token(terminal) => {
                            changed |= insert_fact(
                                &mut follows[target],
                                usize::from(*terminal),
                                work,
                                limits.max_work,
                                &mut fact_count,
                                limits.max_parser_cells,
                            )?;
                            suffix_nullable = false;
                            break;
                        }
                        Symbol::Rule(rule) => {
                            let source_ref = &first[usize::from(*rule)];
                            charge(work, source_ref.len() as u64, limits.max_work)?;
                            let source = fallible_copy(source_ref)?;
                            for terminal in source {
                                charge(work, 1, limits.max_work)?;
                                changed |= insert_fact(
                                    &mut follows[target],
                                    terminal,
                                    work,
                                    limits.max_work,
                                    &mut fact_count,
                                    limits.max_parser_cells,
                                )?;
                            }
                            if !nullable[usize::from(*rule)] {
                                suffix_nullable = false;
                                break;
                            }
                        }
                    }
                }
                if suffix_nullable {
                    let source_ref = &follows[lhs];
                    charge(work, source_ref.len() as u64, limits.max_work)?;
                    let source = fallible_copy(source_ref)?;
                    for terminal in source {
                        charge(work, 1, limits.max_work)?;
                        changed |= insert_fact(
                            &mut follows[target],
                            terminal,
                            work,
                            limits.max_work,
                            &mut fact_count,
                            limits.max_parser_cells,
                        )?;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(follows)
}

fn insert_fact(
    set: &mut Vec<usize>,
    terminal: usize,
    work: &mut u64,
    maximum_work: u64,
    fact_count: &mut u64,
    maximum: u64,
) -> Result<bool, CompileError> {
    charge(work, set.len() as u64, maximum_work)?;
    if set.contains(&terminal) {
        return Ok(false);
    }
    *fact_count = fact_count.saturating_add(1);
    if *fact_count > maximum {
        return Err(CompileError::TooManyParserCells {
            required: *fact_count,
            maximum,
        });
    }
    fallible_push(set, terminal)?;
    Ok(true)
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

fn build_slr_states(
    grammar: &YaccGrammar<u32>,
    limits: ValidationLimits,
    work: &mut u64,
) -> Result<(Vec<State>, Vec<StateTransitions>), CompileError> {
    let mut start_items = reserved_vec(1)?;
    fallible_push(
        &mut start_items,
        Item {
            production: grammar.start_prod(),
            dot: 0,
        },
    )?;
    let start = closure(grammar, start_items, work, limits.max_work)?;
    let mut states = reserved_vec(1)?;
    fallible_push(&mut states, start)?;
    let mut transitions = Vec::new();
    let mut cursor = 0_usize;

    while cursor < states.len() {
        let mut kernels: Vec<(Symbol<u32>, Vec<Item>)> = Vec::new();
        for item in &states[cursor] {
            charge(work, 1, limits.max_work)?;
            let Some(&symbol) = grammar.prod(item.production).get(item.dot) else {
                continue;
            };
            charge(work, kernels.len() as u64, limits.max_work)?;
            let kernel = if let Some(index) = kernels.iter().position(|(known, _)| *known == symbol)
            {
                &mut kernels[index].1
            } else {
                fallible_push(&mut kernels, (symbol, Vec::new()))?;
                &mut kernels.last_mut().expect("just inserted").1
            };
            fallible_push(
                kernel,
                Item {
                    production: item.production,
                    dot: item.dot + 1,
                },
            )?;
        }
        charge_sort(work, kernels.len(), limits.max_work)?;
        kernels.sort_by_key(|(symbol, _)| symbol_key(*symbol));

        let mut edges = reserved_vec(kernels.len())?;
        for (symbol, kernel) in kernels {
            let destination = closure(grammar, kernel, work, limits.max_work)?;
            let mut existing = None;
            for (index, state) in states.iter().enumerate() {
                charge(
                    work,
                    (state.len() as u64).saturating_add(1),
                    limits.max_work,
                )?;
                if *state == destination {
                    existing = Some(index);
                    break;
                }
            }
            let destination_index = match existing {
                Some(index) => index,
                None => {
                    let required_states = states.len() + 1;
                    if required_states as u64 > limits.max_parser_states {
                        return Err(CompileError::TooManyParserStates {
                            required: required_states,
                            maximum: limits.max_parser_states,
                        });
                    }
                    check_cell_limit(
                        required_states,
                        usize::from(grammar.tokens_len()),
                        usize::from(grammar.rules_len()),
                        limits,
                    )?;
                    fallible_push(&mut states, destination)?;
                    required_states - 1
                }
            };
            fallible_push(&mut edges, (symbol, destination_index))?;
        }
        fallible_push(&mut transitions, edges)?;
        cursor += 1;
    }
    Ok((states, transitions))
}

fn closure(
    grammar: &YaccGrammar<u32>,
    mut items: Vec<Item>,
    work: &mut u64,
    maximum_work: u64,
) -> Result<Vec<Item>, CompileError> {
    charge_sort(work, items.len(), maximum_work)?;
    items.sort_unstable();
    charge(work, items.len() as u64, maximum_work)?;
    items.dedup();
    let mut cursor = 0_usize;
    while cursor < items.len() {
        charge(work, 1, maximum_work)?;
        let item = items[cursor];
        if let Some(Symbol::Rule(rule)) = grammar.prod(item.production).get(item.dot).copied() {
            for &production in grammar.rule_to_prods(rule) {
                charge(work, 1, maximum_work)?;
                let candidate = Item { production, dot: 0 };
                charge(work, items.len() as u64, maximum_work)?;
                if !items.contains(&candidate) {
                    fallible_push(&mut items, candidate)?;
                }
            }
        }
        cursor += 1;
    }
    charge_sort(work, items.len(), maximum_work)?;
    items.sort_unstable();
    Ok(items)
}

const fn symbol_key(symbol: Symbol<u32>) -> (u8, u32) {
    match symbol {
        Symbol::Token(terminal) => (0, terminal.0),
        Symbol::Rule(rule) => (1, rule.0),
    }
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

fn charge_sort(work: &mut u64, length: usize, maximum: u64) -> Result<(), CompileError> {
    let rounds = usize::BITS - length.saturating_sub(1).leading_zeros();
    let comparisons = (length as u64).saturating_mul(u64::from(rounds).saturating_add(1));
    charge(work, comparisons, maximum)
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

fn set_action(
    cell: &mut Action,
    action: Action,
    shift_reduce: &mut usize,
    reduce_reduce: &mut usize,
) {
    if matches!(cell, Action::Error) {
        *cell = action;
        return;
    }
    if *cell == action {
        return;
    }
    match (&*cell, &action) {
        (Action::Reduce { .. }, Action::Reduce { .. }) => *reduce_reduce += 1,
        (Action::Shift(_), Action::Reduce { .. }) | (Action::Reduce { .. }, Action::Shift(_)) => {
            *shift_reduce += 1
        }
        _ => *shift_reduce += 1,
    }
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
    use super::{CompileError, ProgressEdge, insert_fact, progress_ranks};

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
    fn fact_and_graph_scans_are_charged_to_the_work_limit() {
        let mut facts = vec![0, 1, 2];
        let mut fact_count = 3;
        let mut work = 0;
        assert_eq!(
            insert_fact(&mut facts, 3, &mut work, 2, &mut fact_count, 10),
            Err(CompileError::CompilerWorkLimit {
                required: 3,
                maximum: 2,
            })
        );

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
