use std::fmt;

use serde_json::{Map, Value};

use crate::{CompileError, TokenSpec, TokenizerError, TokenizerManifest};

const BYTE_LEVEL_INVERSE_LEN: usize = 324;
const UNMAPPED_BYTE: u16 = 256;
const BYTE_LEVEL_INVERSE: [u16; BYTE_LEVEL_INVERSE_LEN] = build_byte_level_inverse();

/// Finite parsing and extraction budgets for the strict tokenizer.json profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenizerJsonLimits {
    pub max_json_bytes: usize,
    pub max_vocab_size: usize,
    pub max_decoded_token_bytes: usize,
    pub max_work: u64,
}

impl Default for TokenizerJsonLimits {
    fn default() -> Self {
        Self {
            max_json_bytes: 64 * 1024 * 1024,
            max_vocab_size: 1_000_000,
            max_decoded_token_bytes: 64 * 1024 * 1024,
            max_work: 256 * 1024 * 1024,
        }
    }
}

/// Auditable description of an accepted tokenizer.json byte profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityReport {
    model_type: String,
    decoder_type: String,
    vocab_size: usize,
    eos_token_ids: Vec<u32>,
}

impl CompatibilityReport {
    #[must_use]
    pub fn model_type(&self) -> &str {
        &self.model_type
    }

    #[must_use]
    pub fn decoder_type(&self) -> &str {
        &self.decoder_type
    }

    #[must_use]
    pub const fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    #[must_use]
    pub fn eos_token_ids(&self) -> &[u32] {
        &self.eos_token_ids
    }
}

impl fmt::Display for CompatibilityReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} vocabulary with {} decoder: {} entries, EOS {:?}, complete byte-singleton coverage",
            self.model_type, self.decoder_type, self.vocab_size, self.eos_token_ids
        )
    }
}

pub(crate) fn extract(
    json: &str,
    eos_token_ids: &[u32],
    limits: TokenizerJsonLimits,
) -> Result<(TokenizerManifest, CompatibilityReport), CompileError> {
    if json.len() > limits.max_json_bytes {
        return tokenizer_error(TokenizerError::JsonTooLarge {
            bytes: json.len(),
            maximum: limits.max_json_bytes,
        });
    }
    if eos_token_ids.is_empty() {
        return tokenizer_error(TokenizerError::MissingEos);
    }

    let mut budget = ExtractionBudget::new(limits);
    budget.charge_work(as_u64(json.len()))?;
    let root: Value = serde_json::from_str(json).map_err(|error| {
        CompileError::Tokenizer(TokenizerError::InvalidJson {
            message: error.to_string(),
        })
    })?;
    let root = object(&root, "top-level value")?;
    validate_decoder(root.get("decoder"), &mut budget)?;

    let model = object(
        root.get("model")
            .ok_or_else(|| incompatible("missing model"))?,
        "model",
    )?;
    let model_type = string_field(model, "type", "model")?;
    let mut entries = match model_type {
        "BPE" | "WordPiece" | "WordLevel" => object_vocabulary(model, &mut budget)?,
        "Unigram" => unigram_vocabulary(model, &mut budget)?,
        other => return Err(incompatible(format!("model type {other}"))),
    };
    let eos = EosSet::new(eos_token_ids, &mut budget)?;
    merge_added_tokens(root.get("added_tokens"), &mut entries, &eos, &mut budget)?;
    if entries.is_empty() || entries.len() > limits.max_vocab_size {
        return Err(incompatible(format!(
            "vocabulary size {} is outside 1..={}",
            entries.len(),
            limits.max_vocab_size
        )));
    }
    for id in eos.ids() {
        budget.charge_work(1)?;
        if usize::try_from(*id).map_or(true, |id| id >= entries.len()) {
            return Err(incompatible("EOS token ID is outside the dense vocabulary"));
        }
    }

    let mut tokens = reserved_vec(entries.len())?;
    let mut singleton = [false; 256];
    for (id, entry) in entries.into_iter().enumerate() {
        budget.charge_work(1)?;
        let entry = entry.ok_or_else(|| incompatible(format!("missing vocabulary ID {id}")))?;
        if entry.special && !eos.contains(id) {
            return Err(incompatible(format!("non-EOS special token at ID {id}")));
        }
        if eos.contains(id) {
            tokens.push(TokenSpec::Eos);
            continue;
        }
        let bytes = decode_byte_level(&entry.piece, &mut budget)?;
        if bytes.is_empty() {
            return tokenizer_error(TokenizerError::EmptyOrdinaryToken { token: id });
        }
        if bytes.len() == 1 {
            singleton[usize::from(bytes[0])] = true;
        }
        tokens.push(TokenSpec::Bytes(bytes));
    }
    budget.charge_work(256)?;
    for (byte, covered) in singleton.into_iter().enumerate() {
        if !covered {
            return tokenizer_error(TokenizerError::MissingSingletonByte { byte: byte as u8 });
        }
    }

    let report = CompatibilityReport {
        model_type: model_type.to_owned(),
        decoder_type: "ByteLevel".to_owned(),
        vocab_size: tokens.len(),
        eos_token_ids: eos.into_ids(),
    };
    Ok((TokenizerManifest::new(tokens), report))
}

#[derive(Clone)]
struct Entry {
    piece: String,
    special: bool,
}

struct EosSet {
    membership: Vec<bool>,
    ids: Vec<u32>,
}

impl EosSet {
    fn new(ids: &[u32], budget: &mut ExtractionBudget) -> Result<Self, CompileError> {
        if ids.len() > budget.limits.max_vocab_size {
            return Err(incompatible(format!(
                "EOS token count {} exceeds the vocabulary limit {}",
                ids.len(),
                budget.limits.max_vocab_size
            )));
        }
        budget.charge_work(as_u64(ids.len()))?;
        let maximum_id = ids
            .iter()
            .copied()
            .max()
            .and_then(|id| usize::try_from(id).ok())
            .unwrap_or(usize::MAX);
        if maximum_id >= budget.limits.max_vocab_size {
            return Err(incompatible("EOS token ID exceeds the vocabulary limit"));
        }
        let length = maximum_id.saturating_add(1);
        let mut membership = reserved_vec(length)?;
        membership.resize(length, false);
        budget.charge_work(as_u64(ids.len()))?;
        for id in ids {
            let id = usize::try_from(*id)
                .map_err(|_| incompatible("EOS token ID exceeds the vocabulary limit"))?;
            if membership[id] {
                return Err(incompatible("duplicate EOS token ID"));
            }
            membership[id] = true;
        }
        budget.charge_work(as_u64(length))?;
        let mut sorted = reserved_vec(ids.len())?;
        for (id, present) in membership.iter().copied().enumerate() {
            if present {
                sorted.push(
                    u32::try_from(id)
                        .map_err(|_| incompatible("EOS token ID exceeds the vocabulary limit"))?,
                );
            }
        }
        Ok(Self {
            membership,
            ids: sorted,
        })
    }

    fn contains(&self, id: usize) -> bool {
        self.membership.get(id).copied().unwrap_or(false)
    }

    fn ids(&self) -> &[u32] {
        &self.ids
    }

    fn into_ids(self) -> Vec<u32> {
        self.ids
    }
}

struct ExtractionBudget {
    limits: TokenizerJsonLimits,
    work: u64,
    decoded_bytes: usize,
}

impl ExtractionBudget {
    const fn new(limits: TokenizerJsonLimits) -> Self {
        Self {
            limits,
            work: 0,
            decoded_bytes: 0,
        }
    }

    fn charge_work(&mut self, amount: u64) -> Result<(), CompileError> {
        self.work = self.work.saturating_add(amount);
        if self.work > self.limits.max_work {
            return tokenizer_error(TokenizerError::WorkLimit {
                required: self.work,
                maximum: self.limits.max_work,
            });
        }
        Ok(())
    }

    fn charge_decoded_bytes(&mut self, bytes: usize) -> Result<(), CompileError> {
        self.decoded_bytes = self.decoded_bytes.saturating_add(bytes);
        if self.decoded_bytes > self.limits.max_decoded_token_bytes {
            return tokenizer_error(TokenizerError::DecodedOutputTooLarge {
                bytes: self.decoded_bytes,
                maximum: self.limits.max_decoded_token_bytes,
            });
        }
        Ok(())
    }
}

fn validate_decoder(
    value: Option<&Value>,
    budget: &mut ExtractionBudget,
) -> Result<(), CompileError> {
    let decoder = object(
        value.ok_or_else(|| incompatible("missing decoder"))?,
        "decoder",
    )?;
    budget.charge_work(as_u64(decoder.len()))?;
    if string_field(decoder, "type", "decoder")? != "ByteLevel" {
        return Err(incompatible("only an exact ByteLevel decoder is supported"));
    }
    const ALLOWED: [&str; 4] = ["type", "add_prefix_space", "trim_offsets", "use_regex"];
    if let Some(field) = decoder
        .keys()
        .find(|field| !ALLOWED.contains(&field.as_str()))
    {
        return Err(incompatible(format!(
            "unknown ByteLevel decoder field {field}"
        )));
    }
    Ok(())
}

fn object_vocabulary(
    model: &Map<String, Value>,
    budget: &mut ExtractionBudget,
) -> Result<Vec<Option<Entry>>, CompileError> {
    let vocab = object(
        model
            .get("vocab")
            .ok_or_else(|| incompatible("model has no vocabulary"))?,
        "model.vocab",
    )?;
    check_vocab_size(vocab.len(), budget.limits.max_vocab_size)?;
    let mut pairs = reserved_vec(vocab.len())?;
    for (piece, id) in vocab {
        budget.charge_work(as_u64(piece.len()).saturating_add(1))?;
        let id = id
            .as_u64()
            .and_then(|id| usize::try_from(id).ok())
            .ok_or_else(|| incompatible(format!("invalid ID for vocabulary piece {piece:?}")))?;
        pairs.push((id, fallible_string_copy(piece)?));
    }
    build_dense(pairs, budget)
}

fn unigram_vocabulary(
    model: &Map<String, Value>,
    budget: &mut ExtractionBudget,
) -> Result<Vec<Option<Entry>>, CompileError> {
    let vocab = model
        .get("vocab")
        .and_then(Value::as_array)
        .ok_or_else(|| incompatible("Unigram model.vocab is not an array"))?;
    check_vocab_size(vocab.len(), budget.limits.max_vocab_size)?;
    let mut output = reserved_vec(vocab.len())?;
    for (id, item) in vocab.iter().enumerate() {
        budget.charge_work(1)?;
        let pair = item
            .as_array()
            .ok_or_else(|| incompatible(format!("Unigram vocabulary item {id} is not an array")))?;
        let piece = pair.first().and_then(Value::as_str).ok_or_else(|| {
            incompatible(format!("Unigram vocabulary item {id} has no string piece"))
        })?;
        budget.charge_work(as_u64(piece.len()))?;
        output.push(Some(Entry {
            piece: fallible_string_copy(piece)?,
            special: false,
        }));
    }
    Ok(output)
}

fn build_dense(
    pairs: Vec<(usize, String)>,
    budget: &mut ExtractionBudget,
) -> Result<Vec<Option<Entry>>, CompileError> {
    budget.charge_work(as_u64(pairs.len()))?;
    let size = pairs
        .iter()
        .map(|(id, _)| id.saturating_add(1))
        .max()
        .unwrap_or(0);
    check_vocab_size(size, budget.limits.max_vocab_size)?;
    let mut output = reserved_vec(size)?;
    output.resize_with(size, || None);
    for (id, piece) in pairs {
        if output[id].is_some() {
            return Err(incompatible(format!("duplicate vocabulary ID {id}")));
        }
        output[id] = Some(Entry {
            piece,
            special: false,
        });
    }
    Ok(output)
}

fn merge_added_tokens(
    value: Option<&Value>,
    entries: &mut Vec<Option<Entry>>,
    eos: &EosSet,
    budget: &mut ExtractionBudget,
) -> Result<(), CompileError> {
    let Some(value) = value else {
        return Ok(());
    };
    let added = value
        .as_array()
        .ok_or_else(|| incompatible("added_tokens is not an array"))?;
    budget.charge_work(as_u64(added.len()))?;
    for token in added {
        let token = object(token, "added token")?;
        let id = token
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|id| usize::try_from(id).ok())
            .ok_or_else(|| incompatible("added token has an invalid ID"))?;
        if id >= budget.limits.max_vocab_size {
            return Err(incompatible(format!("added token ID {id} is too large")));
        }
        let piece = string_field(token, "content", "added token")?;
        budget.charge_work(as_u64(piece.len()))?;
        let special = token
            .get("special")
            .and_then(Value::as_bool)
            .ok_or_else(|| incompatible(format!("added token {id} has no boolean special flag")))?;
        if special && !eos.contains(id) {
            return Err(incompatible(format!("non-EOS special token at ID {id}")));
        }
        if entries.len() <= id {
            let required = id.saturating_add(1);
            entries
                .try_reserve_exact(required - entries.len())
                .map_err(|_| CompileError::AllocationFailure {
                    requested: required,
                })?;
            entries.resize_with(required, || None);
        }
        match &mut entries[id] {
            Some(existing) if existing.piece != piece => {
                return Err(incompatible(format!(
                    "added token {id} conflicts with the model vocabulary"
                )));
            }
            Some(existing) => existing.special |= special,
            slot @ None => {
                *slot = Some(Entry {
                    piece: fallible_string_copy(piece)?,
                    special,
                });
            }
        }
    }
    Ok(())
}

fn decode_byte_level(piece: &str, budget: &mut ExtractionBudget) -> Result<Vec<u8>, CompileError> {
    let mut decoded_length = 0_usize;
    let mut all_mapped = true;
    for character in piece.chars() {
        budget.charge_work(1)?;
        decoded_length = decoded_length.saturating_add(1);
        all_mapped &= inverse_byte_level(character).is_some();
    }
    let output_length = if all_mapped {
        decoded_length
    } else {
        piece.len()
    };
    budget.charge_decoded_bytes(output_length)?;
    let mut output = reserved_vec(output_length)?;
    if all_mapped {
        for character in piece.chars() {
            budget.charge_work(1)?;
            if let Some(byte) = inverse_byte_level(character) {
                output.push(byte);
            }
        }
    } else {
        budget.charge_work(as_u64(piece.len()))?;
        output.extend_from_slice(piece.as_bytes());
    }
    Ok(output)
}

fn inverse_byte_level(character: char) -> Option<u8> {
    let codepoint = character as usize;
    let byte = BYTE_LEVEL_INVERSE.get(codepoint).copied()?;
    u8::try_from(byte).ok()
}

const fn build_byte_level_inverse() -> [u16; BYTE_LEVEL_INVERSE_LEN] {
    let mut output = [UNMAPPED_BYTE; BYTE_LEVEL_INVERSE_LEN];
    let mut byte = 0_u16;
    let mut replacement = 0_usize;
    while byte <= 255 {
        let as_byte = byte as u8;
        let codepoint = if is_direct_byte(as_byte) {
            byte as usize
        } else {
            let codepoint = 256 + replacement;
            replacement += 1;
            codepoint
        };
        output[codepoint] = byte;
        byte += 1;
    }
    output
}

const fn is_direct_byte(byte: u8) -> bool {
    (byte >= b'!' && byte <= b'~') || (byte >= 0xa1 && byte <= 0xac) || (byte >= 0xae)
}

fn check_vocab_size(actual: usize, maximum: usize) -> Result<(), CompileError> {
    if actual > maximum {
        return Err(incompatible(format!(
            "vocabulary size {actual} exceeds {maximum}"
        )));
    }
    Ok(())
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

fn as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn object<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, CompileError> {
    value
        .as_object()
        .ok_or_else(|| incompatible(format!("{label} is not an object")))
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<&'a str, CompileError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| incompatible(format!("{label}.{field} is not a string")))
}

fn incompatible(reason: impl Into<String>) -> CompileError {
    CompileError::Tokenizer(TokenizerError::IncompatibleJson {
        reason: reason.into(),
    })
}

fn tokenizer_error<T>(error: TokenizerError) -> Result<T, CompileError> {
    Err(CompileError::Tokenizer(error))
}
