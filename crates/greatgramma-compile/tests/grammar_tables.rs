use greatgramma_compile::{
    CompileError, CompileLimits, SourceGrammar, TerminalSpec, TokenSpec, TokenizerManifest,
    compile, normalize,
};
use greatgramma_core::{AdvanceResult, LimitKind, TokenId, ValidationError};

fn source(yacc: &str, terminals: Vec<TerminalSpec>) -> SourceGrammar {
    SourceGrammar::new(yacc, terminals)
}

fn terminal(name: &str, pattern: &str, priority: u32) -> TerminalSpec {
    TerminalSpec::new(name, pattern, priority)
}

fn item_close_source() -> SourceGrammar {
    source(
        "%start S\n%token ITEM CLOSE\n%%\nS: ITEM CLOSE;\n",
        vec![terminal("ITEM", "i", 0), terminal("CLOSE", "c", 0)],
    )
}

fn item_close_tokens() -> TokenizerManifest {
    TokenizerManifest::new(vec![
        TokenSpec::Bytes(b"i".to_vec()),
        TokenSpec::Bytes(b"c".to_vec()),
        TokenSpec::Bytes(b"ic".to_vec()),
        TokenSpec::Eos,
    ])
}

fn compile_error(source: SourceGrammar, limits: CompileLimits) -> CompileError {
    match compile(source, item_close_tokens(), limits) {
        Ok(_) => panic!("compilation unexpectedly succeeded"),
        Err(error) => error,
    }
}

#[test]
fn compiles_yacc_regexes_and_exact_token_bytes_into_a_matcher() {
    let prepared = compile(
        item_close_source(),
        item_close_tokens(),
        CompileLimits::default(),
    )
    .expect("supported grammar should compile");
    let mut matcher = prepared.into_matcher(1).unwrap();
    let mut mask = vec![0; matcher.mask_bytes()];

    matcher.mask(0, &mut mask).unwrap();
    assert_eq!(mask, vec![0b0000_0101]);

    assert_eq!(
        matcher.advance(0, TokenId::new(2)).unwrap(),
        AdvanceResult::Continue
    );
    matcher.mask(0, &mut mask).unwrap();
    assert_eq!(mask, vec![0b0000_1000]);

    assert_eq!(
        matcher.advance(0, TokenId::new(3)).unwrap(),
        AdvanceResult::Accepted
    );
}

#[test]
fn accepting_lexer_residual_can_finish_through_eos() {
    let source = source(
        "%start S\n%token ITEM\n%%\nS: ITEM;\n",
        vec![terminal("ITEM", "i+", 0)],
    );
    let tokens = TokenizerManifest::new(vec![
        TokenSpec::Bytes(b"i".to_vec()),
        TokenSpec::Bytes(b"ii".to_vec()),
        TokenSpec::Eos,
    ]);
    let prepared = compile(source, tokens, CompileLimits::default()).expect("supported grammar");
    let mut matcher = prepared.into_matcher(1).expect("matcher");
    let mut mask = [0_u8];
    matcher
        .mask(0, &mut mask)
        .expect("ordinary tokens can leave an accepting lexer residual");
    assert_eq!(mask, [0b0000_0011]);
}

#[test]
fn rejects_nullable_and_unsupported_regexes() {
    let nullable = source(
        "%start S\n%token ITEM\n%%\nS: ITEM;\n",
        vec![terminal("ITEM", "i*", 0)],
    );
    assert_eq!(
        compile_error(nullable, CompileLimits::default()),
        CompileError::NullableTerminal {
            name: "ITEM".to_owned(),
        }
    );

    let lookahead = source(
        "%start S\n%token ITEM\n%%\nS: ITEM;\n",
        vec![terminal("ITEM", "i(?=c)", 0)],
    );
    assert!(matches!(
        compile_error(lookahead, CompileLimits::default()),
        CompileError::UnsupportedRegex { name, .. } if name == "ITEM"
    ));
}

#[test]
fn rejects_missing_unknown_and_duplicate_terminal_specs() {
    let missing = source(
        "%start S\n%token ITEM CLOSE\n%%\nS: ITEM CLOSE;\n",
        vec![terminal("ITEM", "i", 0)],
    );
    assert_eq!(
        compile_error(missing, CompileLimits::default()),
        CompileError::MissingTerminal {
            name: "CLOSE".to_owned(),
        }
    );

    let unknown = source(
        "%start S\n%token ITEM\n%%\nS: ITEM;\n",
        vec![terminal("ITEM", "i", 0), terminal("EXTRA", "x", 1)],
    );
    assert_eq!(
        compile_error(unknown, CompileLimits::default()),
        CompileError::UnknownTerminal {
            name: "EXTRA".to_owned(),
        }
    );

    let duplicate = source(
        "%start S\n%token ITEM\n%%\nS: ITEM;\n",
        vec![terminal("ITEM", "i", 0), terminal("ITEM", "j", 1)],
    );
    assert_eq!(
        compile_error(duplicate, CompileLimits::default()),
        CompileError::DuplicateTerminal {
            name: "ITEM".to_owned(),
        }
    );
}

#[test]
fn rejects_equal_priority_overlap_and_dfa_growth() {
    let ambiguous = source(
        "%start S\n%token A B\n%%\nS: A | B;\n",
        vec![terminal("A", "x", 7), terminal("B", "[x]", 7)],
    );
    assert_eq!(
        compile_error(ambiguous, CompileLimits::default()),
        CompileError::AmbiguousPriority {
            first: "A".to_owned(),
            second: "B".to_owned(),
            priority: 7,
        }
    );

    let limits = CompileLimits {
        max_dfa_states: 1,
        ..CompileLimits::default()
    };
    let too_large = source(
        "%start S\n%token ITEM\n%%\nS: ITEM;\n",
        vec![terminal("ITEM", "ab", 0)],
    );
    assert_eq!(
        compile_error(too_large, limits),
        CompileError::TooManyDfaStates {
            required: 2,
            maximum: 1,
        }
    );
}

#[test]
fn enforces_dfa_cell_limit_during_normalization() {
    let mut limits = CompileLimits::default();
    limits.validation.max_dfa_cells = 255;

    assert_eq!(
        normalize(item_close_source(), item_close_tokens(), limits).unwrap_err(),
        CompileError::Validation(ValidationError::LimitExceeded {
            limit: LimitKind::DfaCells,
            actual: 256,
            maximum: 255,
        })
    );
}

#[test]
fn enforces_dfa_work_limit_during_normalization() {
    let long_product = source(
        "%start S\n%token ITEM\n%%\nS: ITEM;\n",
        vec![terminal("ITEM", "a{100}", 0)],
    );
    let tokens = TokenizerManifest::new(vec![TokenSpec::Bytes(vec![b'a'; 100]), TokenSpec::Eos]);
    let mut limits = CompileLimits::default();
    limits.validation.max_work = 10_000;

    assert!(matches!(
        normalize(long_product, tokens, limits).unwrap_err(),
        CompileError::CompilerWorkLimit {
            required,
            maximum: 10_000,
        } if required > 10_000
    ));
}

#[test]
fn rejects_lexer_products_that_need_multi_byte_last_accept_rollback() {
    let fallback = source(
        "%start S\n%token A ABC B\n%%\nS: A B;\n",
        vec![
            terminal("A", "a", 0),
            terminal("ABC", "abc", 0),
            terminal("B", "b", 0),
        ],
    );
    assert!(matches!(
        compile_error(fallback, CompileLimits::default()),
        CompileError::UnsupportedLexerBoundary { .. }
    ));
}

#[test]
fn rejects_lalr_conflicts_and_precedence_declarations() {
    let conflict = source(
        "%start S\n%token A\n%%\nS: S S | A;\n",
        vec![terminal("A", "a", 0)],
    );
    assert!(matches!(
        compile_error(conflict, CompileLimits::default()),
        CompileError::GrammarConflict { .. }
    ));

    let precedence = source(
        "%start S\n%token A\n%left A\n%%\nS: S A S | A;\n",
        vec![terminal("A", "a", 0)],
    );
    assert_eq!(
        compile_error(precedence, CompileLimits::default()),
        CompileError::UnsupportedGrammar {
            feature: "precedence declarations".to_owned(),
        }
    );

    let compact_precedence = source(
        "%start E\n%token NUM PLUS\n%leftPLUS\n%%\nE: E PLUS E | NUM;\n",
        vec![terminal("NUM", "n", 0), terminal("PLUS", r"\+", 0)],
    );
    assert_eq!(
        compile_error(compact_precedence, CompileLimits::default()),
        CompileError::UnsupportedGrammar {
            feature: "precedence declarations".to_owned(),
        }
    );
}

#[test]
fn rejects_semantic_actions_instead_of_silently_ignoring_them() {
    let with_action = source(
        "%start S\n%token ITEM\n%%\nS: ITEM { semantic_action(); };\n",
        vec![terminal("ITEM", "i", 0)],
    );

    assert_eq!(
        compile_error(with_action, CompileLimits::default()),
        CompileError::UnsupportedGrammar {
            feature: "semantic actions".to_owned(),
        }
    );
}

#[test]
fn rejects_code_epilogues_and_unlisted_yacc_metadata() {
    for yacc in [
        "%start S\n%token ITEM\n%%\nS: ITEM;\n%%\nfn ignored() {}\n",
        "%start S\n%token ITEM\n%actiontype Value\n%%\nS: ITEM;\n",
        "%start S\n%token ITEM\n%parse-param context: Context\n%%\nS: ITEM;\n",
        "%start S\n%token ITEM\n%parse-generics T\n%%\nS: ITEM;\n",
        "%start S\n%token ITEM\n%expect 0\n%%\nS: ITEM;\n",
        "%start S\n%token ITEM\n%expect-rr 0\n%%\nS: ITEM;\n",
        "%start S\n%token ITEM\n%epp ITEM \"item\"\n%%\nS: ITEM;\n",
        "%start S\n%token ITEM\n%avoid_insert ITEM\n%%\nS: ITEM;\n",
    ] {
        assert!(matches!(
            compile_error(
                source(yacc, vec![terminal("ITEM", "i", 0)]),
                CompileLimits::default(),
            ),
            CompileError::UnsupportedGrammar { .. }
        ));
    }
}

#[test]
fn rejects_all_complement_class_spellings() {
    for pattern in [r"\D+", r"\S+", r"\W+"] {
        let complement = source(
            "%start S\n%token ITEM\n%%\nS: ITEM;\n",
            vec![terminal("ITEM", pattern, 0)],
        );
        assert!(matches!(
            compile_error(complement, CompileLimits::default()),
            CompileError::UnsupportedRegex { name, .. } if name == "ITEM"
        ));
    }
}

#[test]
fn rejects_both_word_boundary_assertions() {
    for pattern in [r"a\b", r"a\B"] {
        let boundary = source(
            "%start S\n%token ITEM\n%%\nS: ITEM;\n",
            vec![terminal("ITEM", pattern, 0)],
        );
        assert!(matches!(
            compile_error(boundary, CompileLimits::default()),
            CompileError::UnsupportedRegex { name, .. } if name == "ITEM"
        ));
    }
}

#[test]
fn enforces_parser_cell_and_work_limits_during_state_construction() {
    let mut cell_limits = CompileLimits::default();
    cell_limits.validation.max_parser_cells = 1;
    assert_eq!(
        compile_error(item_close_source(), cell_limits),
        CompileError::TooManyParserCells {
            required: 5,
            maximum: 1,
        }
    );

    let mut work_limits = CompileLimits::default();
    work_limits.validation.max_work = 0;
    assert_eq!(
        compile_error(item_close_source(), work_limits),
        CompileError::CompilerWorkLimit {
            required: 1,
            maximum: 0,
        }
    );
}

#[test]
fn derives_progress_ranks_for_nullable_and_recursive_lalr_grammars() {
    let recursive = source(
        "%start S\n%token A\n%%\nS: Items;\nItems: Items A | A;\n",
        vec![terminal("A", "a", 0)],
    );
    let tokens = TokenizerManifest::new(vec![
        TokenSpec::Bytes(b"a".to_vec()),
        TokenSpec::Bytes(b"aa".to_vec()),
        TokenSpec::Eos,
    ]);
    assert!(compile(recursive, tokens, CompileLimits::default()).is_ok());

    let nullable = source(
        "%start S\n%token A\n%%\nS: Items;\nItems: | A Items;\n",
        vec![terminal("A", "a", 0)],
    );
    let tokens = TokenizerManifest::new(vec![TokenSpec::Bytes(b"a".to_vec()), TokenSpec::Eos]);
    let result = compile(nullable, tokens, CompileLimits::default());
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn computes_follow_through_nullable_suffixes() {
    let nullable_suffix = source(
        "%start S\n%token X Y\n%%\nS: A N T;\nA: X;\nN: ;\nT: Y;\n",
        vec![terminal("X", "x", 0), terminal("Y", "y", 0)],
    );
    let tokens = TokenizerManifest::new(vec![
        TokenSpec::Bytes(b"x".to_vec()),
        TokenSpec::Bytes(b"y".to_vec()),
        TokenSpec::Bytes(b"xy".to_vec()),
        TokenSpec::Eos,
    ]);
    let prepared = compile(nullable_suffix, tokens, CompileLimits::default())
        .expect("SLR follow sets include FIRST after nullable symbols");
    let mut matcher = prepared.into_matcher(1).expect("matcher");
    assert_eq!(
        matcher.allows(0, greatgramma_core::TokenId::new(0)),
        Ok(true)
    );
    assert_eq!(
        matcher.allows(0, greatgramma_core::TokenId::new(2)),
        Ok(true)
    );
    assert_eq!(
        matcher.advance(0, greatgramma_core::TokenId::new(2)),
        Ok(AdvanceResult::Continue)
    );
    assert_eq!(
        matcher.advance(0, greatgramma_core::TokenId::new(3)),
        Ok(AdvanceResult::Accepted)
    );
}

#[test]
fn resolves_priority_and_accepts_declared_parser_noop_terminals() {
    let priority = source(
        "%start S\n%token KEYWORD IDENTIFIER\n%%\nS: KEYWORD;\n",
        vec![
            terminal("KEYWORD", "if", 0),
            terminal("IDENTIFIER", "[a-z]+", 1),
        ],
    );
    let tokens = TokenizerManifest::new(vec![TokenSpec::Bytes(b"if".to_vec()), TokenSpec::Eos]);
    assert!(compile(priority, tokens, CompileLimits::default()).is_ok());

    let ignored = source(
        "%start S\n%token ITEM SPACE\n%%\nS: ITEM;\n",
        vec![terminal("ITEM", "i", 0), terminal("SPACE", " +", 1)],
    )
    .with_ignored_terminals(vec!["SPACE".to_owned()]);
    let tokens = TokenizerManifest::new(vec![TokenSpec::Bytes(b" i".to_vec()), TokenSpec::Eos]);
    assert!(compile(ignored, tokens, CompileLimits::default()).is_ok());
}

fn source_input_bytes(source: &SourceGrammar) -> usize {
    source.yacc().len()
        + source
            .terminals()
            .iter()
            .map(|terminal| terminal.name().len() + terminal.pattern().len())
            .sum::<usize>()
        + source
            .ignored_terminals()
            .iter()
            .map(String::len)
            .sum::<usize>()
}

#[test]
fn enforces_compiler_input_limits_before_parsing_yacc() {
    let supported = item_close_source();
    let source_bytes = source_input_bytes(&supported);

    let mut source_limits = CompileLimits {
        max_source_bytes: source_bytes - 1,
        ..CompileLimits::default()
    };
    assert_eq!(
        compile_error(supported.clone(), source_limits),
        CompileError::SourceTooLarge {
            bytes: source_bytes,
            maximum: source_bytes - 1,
        }
    );
    source_limits.max_source_bytes = source_bytes;
    assert!(compile(supported.clone(), item_close_tokens(), source_limits).is_ok());

    let mut terminal_limits = CompileLimits {
        max_terminal_specs: 1,
        ..CompileLimits::default()
    };
    assert_eq!(
        compile_error(supported.clone(), terminal_limits),
        CompileError::TooManyTerminalSpecs {
            count: 2,
            maximum: 1,
        }
    );
    terminal_limits.max_terminal_specs = 2;
    assert!(compile(supported, item_close_tokens(), terminal_limits).is_ok());

    let ignored = source(
        "%start S\n%token ITEM SPACE\n%%\nS: ITEM;\n",
        vec![terminal("ITEM", "i", 0), terminal("SPACE", " +", 1)],
    )
    .with_ignored_terminals(vec!["SPACE".to_owned()]);
    let mut ignored_limits = CompileLimits {
        max_ignored_terminals: 0,
        ..CompileLimits::default()
    };
    assert_eq!(
        compile_error(ignored.clone(), ignored_limits),
        CompileError::TooManyIgnoredTerminals {
            count: 1,
            maximum: 0,
        }
    );
    ignored_limits.max_ignored_terminals = 1;
    assert!(compile(ignored, item_close_tokens(), ignored_limits).is_ok());
}

#[test]
fn enforces_aggregate_regex_source_output_and_work_limits() {
    let one = source(
        "%start S\n%token A\n%%\nS: A;\n",
        vec![terminal("A", "a", 0)],
    );
    let two = source(
        "%start S\n%token A B\n%%\nS: A B;\n",
        vec![terminal("A", "a", 0), terminal("B", "b", 0)],
    );

    let source_limits = CompileLimits {
        max_regex_bytes: 1,
        ..CompileLimits::default()
    };
    assert!(normalize(one.clone(), item_close_tokens(), source_limits).is_ok());
    assert_eq!(
        normalize(two.clone(), item_close_tokens(), source_limits).unwrap_err(),
        CompileError::RegexSourceTooLarge {
            bytes: 2,
            maximum: 1,
        }
    );

    let mut output_probe = CompileLimits {
        max_regex_output_bytes: 0,
        ..CompileLimits::default()
    };
    let output_required = match normalize(one.clone(), item_close_tokens(), output_probe) {
        Err(CompileError::RegexOutputLimit {
            required,
            maximum: 0,
        }) => required,
        result => panic!("expected regex output limit, got {result:?}"),
    };
    output_probe.max_regex_output_bytes = output_required;
    assert!(normalize(one.clone(), item_close_tokens(), output_probe).is_ok());
    assert!(matches!(
        normalize(two.clone(), item_close_tokens(), output_probe),
        Err(CompileError::RegexOutputLimit {
            required,
            maximum,
        }) if required > maximum && maximum == output_required
    ));

    let mut work_probe = CompileLimits {
        max_regex_fuel: 0,
        ..CompileLimits::default()
    };
    let work_required = match normalize(one.clone(), item_close_tokens(), work_probe) {
        Err(CompileError::CompilerWorkLimit {
            required,
            maximum: 0,
        }) => required,
        result => panic!("expected regex work limit, got {result:?}"),
    };
    work_probe.max_regex_fuel = work_required;
    assert!(normalize(one, item_close_tokens(), work_probe).is_ok());
    assert!(matches!(
        normalize(two, item_close_tokens(), work_probe),
        Err(CompileError::CompilerWorkLimit {
            required,
            maximum,
        }) if required > maximum && maximum == work_required
    ));
}
