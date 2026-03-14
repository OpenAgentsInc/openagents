use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    ops::Range,
};

use fancy_regex::Regex;
use thiserror::Error;

use crate::{
    GgufTokenizerMetadata, GgufTokenizerModel, GgufTokenizerPretokenizer, TokenId, TokenSequence,
    TokenVocabulary, TokenizerBoundary,
};

const GENERIC_BYTE_LEVEL_BPE_PATTERN: &str = concat!(
    "[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]*",
    "[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)?|",
    "[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]+",
    "[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)?|",
    "\\p{N}{1,3}|",
    " ?[^\\s\\p{L}\\p{N}]+[\\r\\n/]*|",
    "\\s*[\\r\\n]+|",
    "\\s+(?!\\S)|",
    "\\s+"
);

const LLAMA_TOKEN_TYPE_UNKNOWN: i32 = 2;
const LLAMA_TOKEN_TYPE_CONTROL: i32 = 3;
const LLAMA_TOKEN_TYPE_USER_DEFINED: i32 = 4;
const LLAMA_TOKEN_TYPE_UNUSED: i32 = 5;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GgufRuntimeTokenizerError {
    #[error("unsupported gguf runtime pretokenizer `{pretokenizer}`")]
    UnsupportedPretokenizer { pretokenizer: String },
    #[error("invalid gguf runtime tokenizer: {message}")]
    InvalidTokenizer { message: String },
}

#[derive(Clone, Debug)]
pub struct GgufRuntimeTokenizer {
    inner: GgufRuntimeTokenizerKind,
}

#[derive(Clone, Debug)]
enum GgufRuntimeTokenizerKind {
    SentencePiece(SentencePieceRuntimeTokenizer),
    Gpt2Bpe(ByteLevelBpeRuntimeTokenizer),
    BertWordPiece(WordPieceRuntimeTokenizer),
}

impl GgufRuntimeTokenizer {
    pub fn from_gguf(tokenizer: &GgufTokenizerMetadata) -> Result<Self, GgufRuntimeTokenizerError> {
        let inner = match tokenizer.model {
            GgufTokenizerModel::SentencePiece => GgufRuntimeTokenizerKind::SentencePiece(
                SentencePieceRuntimeTokenizer::from_gguf(tokenizer),
            ),
            GgufTokenizerModel::Gpt2Bpe => GgufRuntimeTokenizerKind::Gpt2Bpe(
                ByteLevelBpeRuntimeTokenizer::from_gguf(tokenizer)?,
            ),
            GgufTokenizerModel::BertWordPiece => GgufRuntimeTokenizerKind::BertWordPiece(
                WordPieceRuntimeTokenizer::from_gguf(tokenizer),
            ),
        };
        Ok(Self { inner })
    }

    #[must_use]
    pub fn encode_with_special_tokens(
        &self,
        text: &str,
        add_bos: bool,
        add_eos: bool,
    ) -> TokenSequence {
        match &self.inner {
            GgufRuntimeTokenizerKind::SentencePiece(tokenizer) => {
                tokenizer.encode_with_special_tokens(text, add_bos, add_eos)
            }
            GgufRuntimeTokenizerKind::Gpt2Bpe(tokenizer) => {
                tokenizer.encode_with_special_tokens(text, add_bos, add_eos)
            }
            GgufRuntimeTokenizerKind::BertWordPiece(tokenizer) => {
                tokenizer.encode_with_special_tokens(text, add_bos, add_eos)
            }
        }
    }

    #[must_use]
    pub fn encode_with_defaults(&self, text: &str) -> TokenSequence {
        match &self.inner {
            GgufRuntimeTokenizerKind::SentencePiece(tokenizer) => {
                tokenizer.encode_with_defaults(text)
            }
            GgufRuntimeTokenizerKind::Gpt2Bpe(tokenizer) => tokenizer.encode_with_defaults(text),
            GgufRuntimeTokenizerKind::BertWordPiece(tokenizer) => {
                tokenizer.encode_with_defaults(text)
            }
        }
    }

    #[must_use]
    pub fn is_end_of_sequence(&self, token: TokenId) -> bool {
        match &self.inner {
            GgufRuntimeTokenizerKind::SentencePiece(tokenizer) => {
                tokenizer.is_end_of_sequence(token)
            }
            GgufRuntimeTokenizerKind::Gpt2Bpe(tokenizer) => tokenizer.is_end_of_sequence(token),
            GgufRuntimeTokenizerKind::BertWordPiece(tokenizer) => {
                tokenizer.is_end_of_sequence(token)
            }
        }
    }
}

impl TokenizerBoundary for GgufRuntimeTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, false, false)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        match &self.inner {
            GgufRuntimeTokenizerKind::SentencePiece(tokenizer) => tokenizer.decode(tokens),
            GgufRuntimeTokenizerKind::Gpt2Bpe(tokenizer) => tokenizer.decode(tokens),
            GgufRuntimeTokenizerKind::BertWordPiece(tokenizer) => tokenizer.decode(tokens),
        }
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        match &self.inner {
            GgufRuntimeTokenizerKind::SentencePiece(tokenizer) => tokenizer.vocabulary(),
            GgufRuntimeTokenizerKind::Gpt2Bpe(tokenizer) => tokenizer.vocabulary(),
            GgufRuntimeTokenizerKind::BertWordPiece(tokenizer) => tokenizer.vocabulary(),
        }
    }
}

#[derive(Clone, Debug)]
struct SentencePieceRuntimeTokenizer {
    vocabulary: TokenVocabulary,
    lookup: BTreeMap<String, TokenId>,
    add_bos: bool,
    add_eos: bool,
    eos_token_ids: Vec<TokenId>,
}

impl SentencePieceRuntimeTokenizer {
    fn from_gguf(tokenizer: &GgufTokenizerMetadata) -> Self {
        Self {
            vocabulary: runtime_vocabulary(tokenizer),
            lookup: runtime_lookup(tokenizer),
            add_bos: tokenizer.add_bos,
            add_eos: tokenizer.add_eos,
            eos_token_ids: tokenizer.vocabulary.eos_token_ids().to_vec(),
        }
    }

    #[must_use]
    fn encode_with_special_tokens(
        &self,
        text: &str,
        add_bos: bool,
        add_eos: bool,
    ) -> TokenSequence {
        let mut tokens = Vec::new();
        if add_bos {
            tokens.push(self.vocabulary.bos_id());
        }
        for piece in text.split_whitespace() {
            let normalized = normalize_piece(piece);
            if normalized.is_empty() {
                continue;
            }
            if let Some(token) = self.lookup.get(normalized.as_str()) {
                tokens.push(*token);
                continue;
            }
            let with_boundary = format!("▁{normalized}");
            if let Some(token) = self.lookup.get(with_boundary.as_str()) {
                tokens.push(*token);
                continue;
            }
            tokens.push(self.vocabulary.unknown_id());
        }
        if add_eos {
            tokens.push(self.vocabulary.eos_id());
        }
        TokenSequence::new(tokens)
    }

    #[must_use]
    fn encode_with_defaults(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, self.add_bos, self.add_eos)
    }

    #[must_use]
    fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.eos_token_ids.contains(&token) || token == self.vocabulary.eos_id()
    }
}

impl TokenizerBoundary for SentencePieceRuntimeTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, false, false)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        let mut pieces = Vec::new();
        for token in tokens {
            if is_runtime_special_token(&self.vocabulary, self.eos_token_ids.as_slice(), *token) {
                continue;
            }
            let Some(piece) = self.vocabulary.token(*token) else {
                continue;
            };
            pieces.push(piece.trim_start_matches('▁').to_string());
        }
        pieces.join(" ")
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        &self.vocabulary
    }
}

#[derive(Clone, Debug)]
struct WordPieceRuntimeTokenizer {
    vocabulary: TokenVocabulary,
    lookup: BTreeMap<String, TokenId>,
    add_bos: bool,
    add_eos: bool,
    eos_token_ids: Vec<TokenId>,
}

impl WordPieceRuntimeTokenizer {
    fn from_gguf(tokenizer: &GgufTokenizerMetadata) -> Self {
        Self {
            vocabulary: runtime_vocabulary(tokenizer),
            lookup: runtime_lookup(tokenizer),
            add_bos: tokenizer.add_bos,
            add_eos: tokenizer.add_eos,
            eos_token_ids: tokenizer.vocabulary.eos_token_ids().to_vec(),
        }
    }

    #[must_use]
    fn encode_with_special_tokens(
        &self,
        text: &str,
        add_bos: bool,
        add_eos: bool,
    ) -> TokenSequence {
        let mut tokens = Vec::new();
        if add_bos {
            tokens.push(self.vocabulary.bos_id());
        }
        for word in text.split_whitespace() {
            let normalized = normalize_piece(word);
            if normalized.is_empty() {
                continue;
            }
            let mut remaining = normalized.as_str();
            let mut first_piece = true;
            while !remaining.is_empty() {
                let mut matched = None;
                for end in (1..=remaining.len()).rev() {
                    if !remaining.is_char_boundary(end) {
                        continue;
                    }
                    let candidate = if first_piece {
                        Cow::Borrowed(&remaining[..end])
                    } else {
                        Cow::Owned(format!("##{}", &remaining[..end]))
                    };
                    if let Some(token) = self.lookup.get(candidate.as_ref()) {
                        matched = Some((*token, end));
                        break;
                    }
                }
                if let Some((token, end)) = matched {
                    tokens.push(token);
                    remaining = &remaining[end..];
                    first_piece = false;
                } else {
                    tokens.push(self.vocabulary.unknown_id());
                    break;
                }
            }
        }
        if add_eos {
            tokens.push(self.vocabulary.eos_id());
        }
        TokenSequence::new(tokens)
    }

    #[must_use]
    fn encode_with_defaults(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, self.add_bos, self.add_eos)
    }

    #[must_use]
    fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.eos_token_ids.contains(&token) || token == self.vocabulary.eos_id()
    }
}

impl TokenizerBoundary for WordPieceRuntimeTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, false, false)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        let mut out = String::new();
        for token in tokens {
            if is_runtime_special_token(&self.vocabulary, self.eos_token_ids.as_slice(), *token) {
                continue;
            }
            let Some(piece) = self.vocabulary.token(*token) else {
                continue;
            };
            if let Some(piece) = piece.strip_prefix("##") {
                out.push_str(piece);
            } else {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(piece);
            }
        }
        out
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        &self.vocabulary
    }
}

#[derive(Clone, Debug)]
struct ByteLevelBpeRuntimeTokenizer {
    bpe: ByteLevelBpeTokenizerCore,
    vocabulary: TokenVocabulary,
    add_bos: bool,
    add_eos: bool,
    eos_token_ids: Vec<TokenId>,
}

impl ByteLevelBpeRuntimeTokenizer {
    fn from_gguf(tokenizer: &GgufTokenizerMetadata) -> Result<Self, GgufRuntimeTokenizerError> {
        Ok(Self {
            bpe: ByteLevelBpeTokenizerCore::from_gguf(tokenizer)?,
            vocabulary: runtime_vocabulary(tokenizer),
            add_bos: tokenizer.add_bos,
            add_eos: tokenizer.add_eos,
            eos_token_ids: tokenizer.vocabulary.eos_token_ids().to_vec(),
        })
    }

    #[must_use]
    fn encode_with_special_tokens(
        &self,
        text: &str,
        add_bos: bool,
        add_eos: bool,
    ) -> TokenSequence {
        let mut tokens = Vec::new();
        if add_bos {
            tokens.push(self.vocabulary.bos_id());
        }
        tokens.extend(
            self.bpe
                .encode_with_special_tokens(text)
                .into_iter()
                .map(TokenId),
        );
        if add_eos {
            tokens.push(self.vocabulary.eos_id());
        }
        TokenSequence::new(tokens)
    }

    #[must_use]
    fn encode_with_defaults(&self, text: &str) -> TokenSequence {
        let add_bos = self.add_bos && !self.bpe.starts_with_special_token(text);
        self.encode_with_special_tokens(text, add_bos, self.add_eos)
    }

    #[must_use]
    fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.eos_token_ids.contains(&token) || token == self.vocabulary.eos_id()
    }
}

impl TokenizerBoundary for ByteLevelBpeRuntimeTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, false, false)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        self.bpe.decode_utf8(tokens).unwrap_or_else(|| {
            tokens
                .iter()
                .filter(|token| {
                    !is_runtime_special_token(
                        &self.vocabulary,
                        self.eos_token_ids.as_slice(),
                        **token,
                    )
                })
                .filter_map(|token| self.vocabulary.token(*token))
                .collect::<Vec<_>>()
                .join("")
        })
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        &self.vocabulary
    }
}

#[derive(Clone, Debug)]
struct ByteLevelBpeTokenizerCore {
    ordinary_encoder: HashMap<Vec<u8>, u32>,
    ordinary_decoder: HashMap<u32, Vec<u8>>,
    special_encoder: HashMap<String, u32>,
    special_decoder: HashMap<u32, String>,
    ordinary_regex: Regex,
    special_regex: Option<Regex>,
}

impl ByteLevelBpeTokenizerCore {
    fn from_gguf(tokenizer: &GgufTokenizerMetadata) -> Result<Self, GgufRuntimeTokenizerError> {
        let ordinary_regex = Regex::new(byte_level_bpe_pattern(tokenizer.pretokenizer.as_ref())?)
            .map_err(|error| GgufRuntimeTokenizerError::InvalidTokenizer {
            message: format!("failed to compile byte-level tokenizer regex: {error}"),
        })?;
        let unicode_to_byte = unicode_to_byte_map();
        let mut ordinary_encoder = HashMap::new();
        let mut ordinary_decoder = HashMap::new();
        let mut special_encoder = HashMap::new();
        let mut special_decoder = HashMap::new();

        for (index, token) in tokenizer.vocabulary.tokens().iter().enumerate() {
            let token_id = index as u32;
            let token_type = tokenizer.token_types.get(index).copied();
            if gguf_token_is_special(token, token_type) {
                if special_encoder.insert(token.clone(), token_id).is_some() {
                    return Err(GgufRuntimeTokenizerError::InvalidTokenizer {
                        message: format!("duplicate special token `{token}` in GGUF tokenizer"),
                    });
                }
                special_decoder.insert(token_id, token.clone());
                continue;
            }

            let raw_bytes = gguf_token_to_raw_bytes(token, &unicode_to_byte)?;
            if ordinary_encoder
                .insert(raw_bytes.clone(), token_id)
                .is_some()
            {
                return Err(GgufRuntimeTokenizerError::InvalidTokenizer {
                    message: format!(
                        "duplicate ordinary token bytes for GGUF token id {token_id} (`{token}`)"
                    ),
                });
            }
            ordinary_decoder.insert(token_id, raw_bytes);
        }

        let special_regex = build_special_regex(&special_encoder)?;
        Ok(Self {
            ordinary_encoder,
            ordinary_decoder,
            special_encoder,
            special_decoder,
            ordinary_regex,
            special_regex,
        })
    }

    fn encode_with_special_tokens(&self, text: &str) -> Vec<u32> {
        let mut tokens = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let next_special = self.find_next_special(text, start);
            let end = next_special.map_or(text.len(), |(match_start, _, _)| match_start);
            self.encode_ordinary_segment(&text[start..end], &mut tokens);
            match next_special {
                Some((_, match_end, token_id)) => {
                    tokens.push(token_id);
                    start = match_end;
                }
                None => break,
            }
        }
        tokens
    }

    fn decode_utf8(&self, tokens: &[TokenId]) -> Option<String> {
        let mut bytes = Vec::new();
        for token in tokens {
            let token_id = token.as_u32();
            if let Some(raw_bytes) = self.ordinary_decoder.get(&token_id) {
                bytes.extend_from_slice(raw_bytes);
                continue;
            }
            if let Some(special) = self.special_decoder.get(&token_id) {
                bytes.extend_from_slice(special.as_bytes());
                continue;
            }
            return None;
        }
        String::from_utf8(bytes).ok()
    }

    fn starts_with_special_token(&self, text: &str) -> bool {
        self.special_token_prefix_range(text)
            .map(|range| range.start == 0)
            .unwrap_or(false)
    }

    fn special_token_prefix_range(&self, text: &str) -> Option<Range<usize>> {
        self.find_next_special(text, 0)
            .and_then(|(start, end, _)| (start == 0).then_some(start..end))
    }

    fn find_next_special(&self, text: &str, start: usize) -> Option<(usize, usize, u32)> {
        let regex = self.special_regex.as_ref()?;
        let matched = regex.find_from_pos(text, start).ok().flatten()?;
        let token = self
            .special_encoder
            .get(&text[matched.start()..matched.end()])?;
        Some((matched.start(), matched.end(), *token))
    }

    fn encode_ordinary_segment(&self, text: &str, out: &mut Vec<u32>) {
        if text.is_empty() {
            return;
        }
        let matches = match self
            .ordinary_regex
            .find_iter(text)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(matches) => matches,
            Err(_) => {
                out.extend(byte_pair_encode(text.as_bytes(), &self.ordinary_encoder));
                return;
            }
        };
        for matched in matches {
            let piece = matched.as_str().as_bytes();
            if let Some(token) = self.ordinary_encoder.get(piece) {
                out.push(*token);
                continue;
            }
            out.extend(byte_pair_encode(piece, &self.ordinary_encoder));
        }
    }
}

fn runtime_vocabulary(tokenizer: &GgufTokenizerMetadata) -> TokenVocabulary {
    let bos_id = tokenizer
        .vocabulary
        .bos_token_id()
        .or_else(|| tokenizer.vocabulary.pad_token_id())
        .or_else(|| tokenizer.vocabulary.unknown_token_id())
        .unwrap_or(TokenId(0));
    let eos_id = tokenizer
        .vocabulary
        .eos_token_ids()
        .first()
        .copied()
        .or_else(|| tokenizer.vocabulary.pad_token_id())
        .or_else(|| tokenizer.vocabulary.bos_token_id())
        .unwrap_or(TokenId(0));
    let pad_id = tokenizer
        .vocabulary
        .pad_token_id()
        .or_else(|| tokenizer.vocabulary.bos_token_id())
        .or_else(|| tokenizer.vocabulary.unknown_token_id())
        .unwrap_or(eos_id);
    let unknown_id = tokenizer
        .vocabulary
        .unknown_token_id()
        .or_else(|| tokenizer.vocabulary.pad_token_id())
        .or_else(|| tokenizer.vocabulary.bos_token_id())
        .unwrap_or(eos_id);
    TokenVocabulary::new(
        tokenizer.vocabulary.tokens().to_vec(),
        pad_id,
        bos_id,
        eos_id,
        unknown_id,
    )
}

fn runtime_lookup(tokenizer: &GgufTokenizerMetadata) -> BTreeMap<String, TokenId> {
    tokenizer
        .vocabulary
        .tokens()
        .iter()
        .enumerate()
        .map(|(index, token)| (token.clone(), TokenId(index as u32)))
        .collect()
}

fn is_runtime_special_token(
    vocabulary: &TokenVocabulary,
    eos_token_ids: &[TokenId],
    token: TokenId,
) -> bool {
    token == vocabulary.pad_id()
        || token == vocabulary.bos_id()
        || token == vocabulary.unknown_id()
        || token == vocabulary.eos_id()
        || eos_token_ids.contains(&token)
}

fn byte_level_bpe_pattern(
    pretokenizer: Option<&GgufTokenizerPretokenizer>,
) -> Result<&'static str, GgufRuntimeTokenizerError> {
    match pretokenizer {
        None
        | Some(GgufTokenizerPretokenizer::Default)
        | Some(GgufTokenizerPretokenizer::Llama)
        | Some(GgufTokenizerPretokenizer::Qwen2)
        | Some(GgufTokenizerPretokenizer::Refact)
        | Some(GgufTokenizerPretokenizer::Tekken) => Ok(GENERIC_BYTE_LEVEL_BPE_PATTERN),
        Some(GgufTokenizerPretokenizer::Custom(value))
            if matches!(
                value.as_str(),
                "gpt-4o" | "default" | "qwen2" | "llama-bpe" | "llama"
            ) =>
        {
            Ok(GENERIC_BYTE_LEVEL_BPE_PATTERN)
        }
        Some(GgufTokenizerPretokenizer::Custom(value)) => {
            Err(GgufRuntimeTokenizerError::UnsupportedPretokenizer {
                pretokenizer: value.clone(),
            })
        }
    }
}

fn gguf_token_is_special(token: &str, token_type: Option<i32>) -> bool {
    matches!(
        token_type,
        Some(
            LLAMA_TOKEN_TYPE_UNKNOWN
                | LLAMA_TOKEN_TYPE_CONTROL
                | LLAMA_TOKEN_TYPE_USER_DEFINED
                | LLAMA_TOKEN_TYPE_UNUSED
        )
    ) || token.starts_with("<|") && token.ends_with("|>")
}

fn build_special_regex(
    special_encoder: &HashMap<String, u32>,
) -> Result<Option<Regex>, GgufRuntimeTokenizerError> {
    if special_encoder.is_empty() {
        return Ok(None);
    }
    let mut tokens = special_encoder
        .keys()
        .map(|token| fancy_regex::escape(token))
        .collect::<Vec<_>>();
    tokens.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    Regex::new(&tokens.join("|")).map(Some).map_err(|error| {
        GgufRuntimeTokenizerError::InvalidTokenizer {
            message: format!("failed to compile special-token regex: {error}"),
        }
    })
}

fn unicode_to_byte_map() -> HashMap<char, u8> {
    let mut mapping = HashMap::with_capacity(256);
    let mut assigned = [false; 256];
    for byte in 0x21_u32..=0x7e {
        let character = char::from_u32(byte).unwrap_or('\0');
        mapping.insert(character, byte as u8);
        assigned[byte as usize] = true;
    }
    for byte in 0xa1_u32..=0xac {
        let character = char::from_u32(byte).unwrap_or('\0');
        mapping.insert(character, byte as u8);
        assigned[byte as usize] = true;
    }
    for byte in 0xae_u32..=0xff {
        let character = char::from_u32(byte).unwrap_or('\0');
        mapping.insert(character, byte as u8);
        assigned[byte as usize] = true;
    }
    let mut next_codepoint = 256_u32;
    for (byte, is_assigned) in assigned.iter().enumerate() {
        if *is_assigned {
            continue;
        }
        let character = char::from_u32(next_codepoint).unwrap_or('\0');
        mapping.insert(character, byte as u8);
        next_codepoint += 1;
    }
    mapping
}

fn gguf_token_to_raw_bytes(
    token: &str,
    unicode_to_byte: &HashMap<char, u8>,
) -> Result<Vec<u8>, GgufRuntimeTokenizerError> {
    token
        .chars()
        .map(|character| {
            unicode_to_byte.get(&character).copied().ok_or_else(|| {
                GgufRuntimeTokenizerError::InvalidTokenizer {
                    message: format!(
                        "GGUF token contains non-byte-mapped character U+{:04X} in `{token}`",
                        character as u32
                    ),
                }
            })
        })
        .collect()
}

fn byte_pair_encode(piece: &[u8], ranks: &HashMap<Vec<u8>, u32>) -> Vec<u32> {
    if piece.is_empty() {
        return Vec::new();
    }
    if piece.len() == 1 {
        return ranks
            .get(piece)
            .copied()
            .map_or_else(Vec::new, |rank| vec![rank]);
    }
    byte_pair_merge(piece, ranks)
        .windows(2)
        .flat_map(|part| {
            let segment = &piece[part[0].0..part[1].0];
            ranks
                .get(segment)
                .copied()
                .map_or_else(|| encode_bytes_as_tokens(segment, ranks), |rank| vec![rank])
        })
        .collect()
}

fn encode_bytes_as_tokens(bytes: &[u8], ranks: &HashMap<Vec<u8>, u32>) -> Vec<u32> {
    bytes
        .iter()
        .filter_map(|byte| ranks.get(&vec![*byte]).copied())
        .collect()
}

fn byte_pair_merge(piece: &[u8], ranks: &HashMap<Vec<u8>, u32>) -> Vec<(usize, u32)> {
    let mut parts = Vec::with_capacity(piece.len() + 1);
    let mut min_rank = (u32::MAX, usize::MAX);
    for index in 0..piece.len().saturating_sub(1) {
        let rank = *ranks.get(&piece[index..index + 2]).unwrap_or(&u32::MAX);
        if rank < min_rank.0 {
            min_rank = (rank, index);
        }
        parts.push((index, rank));
    }
    parts.push((piece.len().saturating_sub(1), u32::MAX));
    parts.push((piece.len(), u32::MAX));

    let get_rank = |parts: &Vec<(usize, u32)>, index: usize| {
        if index + 3 < parts.len() {
            *ranks
                .get(&piece[parts[index].0..parts[index + 3].0])
                .unwrap_or(&u32::MAX)
        } else {
            u32::MAX
        }
    };

    while min_rank.0 != u32::MAX {
        let index = min_rank.1;
        if index > 0 {
            parts[index - 1].1 = get_rank(&parts, index - 1);
        }
        parts[index].1 = get_rank(&parts, index);
        parts.remove(index + 1);

        min_rank = (u32::MAX, usize::MAX);
        for (scan_index, &(_, rank)) in parts[..parts.len().saturating_sub(1)].iter().enumerate() {
            if rank < min_rank.0 {
                min_rank = (rank, scan_index);
            }
        }
    }

    parts
}

fn normalize_piece(piece: &str) -> String {
    piece
        .trim_matches(|character: char| character.is_ascii_punctuation())
        .to_ascii_lowercase()
}
