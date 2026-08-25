use greatgramma_compile::{
    CompileError, TokenSpec, TokenizerError, TokenizerJsonLimits, TokenizerManifest,
};
use serde_json::{Map, Value, json};

fn byte_level_piece(byte: u8) -> String {
    let direct = (b'!'..=b'~').contains(&byte)
        || (0xa1..=0xac).contains(&byte)
        || (0xae..=0xff).contains(&byte);
    let codepoint = if direct {
        u32::from(byte)
    } else {
        let earlier_missing = (0_u16..u16::from(byte))
            .filter(|candidate| {
                let candidate = *candidate as u8;
                !((b'!'..=b'~').contains(&candidate)
                    || (0xa1..=0xac).contains(&candidate)
                    || (0xae..=0xff).contains(&candidate))
            })
            .count();
        256 + earlier_missing as u32
    };
    char::from_u32(codepoint)
        .expect("ByteLevel codepoint is valid")
        .to_string()
}

fn tokenizer_json(extra: &[(&str, u32)], added_tokens: Value) -> String {
    let mut vocab = Map::new();
    for byte in 0_u16..=255 {
        vocab.insert(byte_level_piece(byte as u8), json!(byte));
    }
    for (piece, id) in extra {
        vocab.insert((*piece).to_owned(), json!(id));
    }
    json!({
        "decoder": {"type": "ByteLevel"},
        "model": {"type": "BPE", "vocab": vocab},
        "added_tokens": added_tokens,
    })
    .to_string()
}

#[test]
fn extracts_exact_bytelevel_entries_and_reports_the_profile() {
    let json = tokenizer_json(
        &[("🙂", 256), ("<eos>", 257)],
        json!([{"id": 257, "content": "<eos>", "special": true}]),
    );
    let (manifest, report) =
        TokenizerManifest::from_tokenizer_json(&json, &[257]).expect("profile is supported");

    assert_eq!(manifest.len(), 258);
    for byte in 0_u16..=255 {
        assert_eq!(
            manifest.tokens()[usize::from(byte)],
            TokenSpec::Bytes(vec![byte as u8])
        );
    }
    assert_eq!(
        manifest.tokens()[256],
        TokenSpec::Bytes("🙂".as_bytes().to_vec())
    );
    assert_eq!(manifest.tokens()[257], TokenSpec::Eos);
    assert_eq!(report.model_type(), "BPE");
    assert_eq!(report.decoder_type(), "ByteLevel");
    assert_eq!(report.vocab_size(), 258);
    assert_eq!(report.eos_token_ids(), &[257]);
}

#[test]
fn rejects_decoder_chains_holes_and_non_eos_special_tokens() {
    let sequence = json!({
        "decoder": {"type": "Sequence", "decoders": [{"type": "ByteLevel"}]},
        "model": {"type": "BPE", "vocab": {}},
    })
    .to_string();
    assert!(matches!(
        TokenizerManifest::from_tokenizer_json(&sequence, &[0]),
        Err(CompileError::Tokenizer(
            TokenizerError::IncompatibleJson { .. }
        ))
    ));

    let mut hole_value: Value = serde_json::from_str(&tokenizer_json(
        &[("<eos>", 256)],
        json!([{"id": 256, "content": "<eos>", "special": true}]),
    ))
    .expect("fixture JSON");
    hole_value["model"]["vocab"]
        .as_object_mut()
        .expect("fixture vocab")
        .remove(&byte_level_piece(17));
    assert!(matches!(
        TokenizerManifest::from_tokenizer_json(&hole_value.to_string(), &[256]),
        Err(CompileError::Tokenizer(
            TokenizerError::IncompatibleJson { .. }
        ))
    ));

    let special = tokenizer_json(
        &[("<eos>", 256)],
        json!([
            {"id": 256, "content": "<eos>", "special": true},
            {"id": 257, "content": "<tool>", "special": true}
        ]),
    );
    assert!(matches!(
        TokenizerManifest::from_tokenizer_json(&special, &[256]),
        Err(CompileError::Tokenizer(
            TokenizerError::IncompatibleJson { .. }
        ))
    ));
}

#[test]
fn rejects_missing_singleton_coverage_and_duplicate_eos_ids() {
    let mut value: Value = serde_json::from_str(&tokenizer_json(
        &[("<eos>", 256)],
        json!([{"id": 256, "content": "<eos>", "special": true}]),
    ))
    .expect("fixture JSON");
    let vocab = value["model"]["vocab"]
        .as_object_mut()
        .expect("fixture vocab");
    vocab.remove(&byte_level_piece(42));
    vocab.insert("xy".to_owned(), json!(42));
    assert!(matches!(
        TokenizerManifest::from_tokenizer_json(&value.to_string(), &[256]),
        Err(CompileError::Tokenizer(
            TokenizerError::MissingSingletonByte { byte: 42 }
        ))
    ));

    let complete = tokenizer_json(
        &[("<eos>", 256)],
        json!([{"id": 256, "content": "<eos>", "special": true}]),
    );
    assert!(matches!(
        TokenizerManifest::from_tokenizer_json(&complete, &[256, 256]),
        Err(CompileError::Tokenizer(
            TokenizerError::IncompatibleJson { .. }
        ))
    ));
}

#[test]
fn enforces_tokenizer_json_parse_and_decoded_output_budgets_at_the_boundary() {
    let json = tokenizer_json(
        &[("<eos>", 256)],
        json!([{"id": 256, "content": "<eos>", "special": true}]),
    );

    let mut parse_limits = TokenizerJsonLimits {
        max_json_bytes: json.len() - 1,
        ..TokenizerJsonLimits::default()
    };
    assert_eq!(
        TokenizerManifest::from_tokenizer_json_with_limits(&json, &[256], parse_limits),
        Err(CompileError::Tokenizer(TokenizerError::JsonTooLarge {
            bytes: json.len(),
            maximum: json.len() - 1,
        }))
    );
    parse_limits.max_json_bytes = json.len();
    assert!(
        TokenizerManifest::from_tokenizer_json_with_limits(&json, &[256], parse_limits).is_ok()
    );

    let mut output_limits = TokenizerJsonLimits {
        max_decoded_token_bytes: 255,
        ..TokenizerJsonLimits::default()
    };
    assert_eq!(
        TokenizerManifest::from_tokenizer_json_with_limits(&json, &[256], output_limits),
        Err(CompileError::Tokenizer(
            TokenizerError::DecodedOutputTooLarge {
                bytes: 256,
                maximum: 255,
            }
        ))
    );
    output_limits.max_decoded_token_bytes = 256;
    assert!(
        TokenizerManifest::from_tokenizer_json_with_limits(&json, &[256], output_limits).is_ok()
    );
}

#[test]
fn tokenizer_json_work_budget_is_monotone_and_exact_at_its_boundary() {
    let json = tokenizer_json(
        &[("<eos>", 256)],
        json!([{"id": 256, "content": "<eos>", "special": true}]),
    );
    let defaults = TokenizerJsonLimits::default();
    let mut low = 0_u64;
    let mut high = defaults.max_work;
    while low < high {
        let middle = low + (high - low) / 2;
        let limits = TokenizerJsonLimits {
            max_work: middle,
            ..defaults
        };
        if TokenizerManifest::from_tokenizer_json_with_limits(&json, &[256], limits).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    assert!(low > 0, "the fixture must consume tokenizer work");

    let just_below = TokenizerJsonLimits {
        max_work: low - 1,
        ..defaults
    };
    assert!(matches!(
        TokenizerManifest::from_tokenizer_json_with_limits(&json, &[256], just_below),
        Err(CompileError::Tokenizer(TokenizerError::WorkLimit {
            required,
            maximum,
        })) if required > maximum && maximum == low - 1
    ));
    let exact = TokenizerJsonLimits {
        max_work: low,
        ..defaults
    };
    assert!(TokenizerManifest::from_tokenizer_json_with_limits(&json, &[256], exact).is_ok());
}

#[test]
fn rejects_an_eos_set_larger_than_the_vocabulary_budget_before_index_allocation() {
    let json = tokenizer_json(
        &[("<eos>", 256)],
        json!([{"id": 256, "content": "<eos>", "special": true}]),
    );
    let limits = TokenizerJsonLimits {
        max_vocab_size: 257,
        ..TokenizerJsonLimits::default()
    };
    let eos_ids = vec![0; 258];

    assert!(matches!(
        TokenizerManifest::from_tokenizer_json_with_limits(&json, &eos_ids, limits),
        Err(CompileError::Tokenizer(
            TokenizerError::IncompatibleJson { reason }
        )) if reason.contains("EOS token count")
    ));
}
