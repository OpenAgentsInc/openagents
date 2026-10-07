use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    ops::Range,
};

use fancy_regex::Regex;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

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

const QWEN35_BYTE_LEVEL_BPE_PATTERN: &str = concat!(
    "(?i:'s|'t|'re|'ve|'m|'ll|'d)|",
    "[^\\r\\n\\p{L}\\p{N}]?[\\p{L}\\p{M}]+|",
    "\\p{N}|",
    " ?[^\\s\\p{L}\\p{M}\\p{N}]+[\\r\\n]*|",
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
    Gemma4Bpe(ByteLevelBpeRuntimeTokenizer),
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
            GgufTokenizerModel::Gemma4Bpe => GgufRuntimeTokenizerKind::Gemma4Bpe(
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
            GgufRuntimeTokenizerKind::Gemma4Bpe(tokenizer) => {
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
            GgufRuntimeTokenizerKind::Gemma4Bpe(tokenizer) => tokenizer.encode_with_defaults(text),
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
            GgufRuntimeTokenizerKind::Gemma4Bpe(tokenizer) => tokenizer.is_end_of_sequence(token),
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
            GgufRuntimeTokenizerKind::Gemma4Bpe(tokenizer) => tokenizer.decode(tokens),
            GgufRuntimeTokenizerKind::BertWordPiece(tokenizer) => tokenizer.decode(tokens),
        }
    }

    fn append_decoded_token(&self, text: &mut String, token: TokenId) {
        match &self.inner {
            GgufRuntimeTokenizerKind::SentencePiece(tokenizer) => {
                tokenizer.append_decoded_token(text, token)
            }
            GgufRuntimeTokenizerKind::Gpt2Bpe(tokenizer) => {
                tokenizer.append_decoded_token(text, token)
            }
            GgufRuntimeTokenizerKind::Gemma4Bpe(tokenizer) => {
                tokenizer.append_decoded_token(text, token)
            }
            GgufRuntimeTokenizerKind::BertWordPiece(tokenizer) => {
                tokenizer.append_decoded_token(text, token)
            }
        }
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        match &self.inner {
            GgufRuntimeTokenizerKind::SentencePiece(tokenizer) => tokenizer.vocabulary(),
            GgufRuntimeTokenizerKind::Gpt2Bpe(tokenizer) => tokenizer.vocabulary(),
            GgufRuntimeTokenizerKind::Gemma4Bpe(tokenizer) => tokenizer.vocabulary(),
            GgufRuntimeTokenizerKind::BertWordPiece(tokenizer) => tokenizer.vocabulary(),
        }
    }
}

#[derive(Clone, Debug)]
struct SentencePieceRuntimeTokenizer {
    vocabulary: TokenVocabulary,
    ordinary_pieces: HashMap<String, SentencePieceRuntimePiece>,
    special_encoder: HashMap<String, u32>,
    special_decoder: HashMap<u32, String>,
    special_regex: Option<Regex>,
    byte_fallback: HashMap<u8, TokenId>,
    max_piece_byte_len: usize,
    add_bos: bool,
    add_eos: bool,
    eos_token_ids: Vec<TokenId>,
}

#[derive(Clone, Copy, Debug)]
struct SentencePieceRuntimePiece {
    token: TokenId,
    score: f32,
}

impl SentencePieceRuntimeTokenizer {
    fn from_gguf(tokenizer: &GgufTokenizerMetadata) -> Self {
        let rank_like_scores = sentencepiece_scores_look_rank_like(tokenizer);
        let mut ordinary_pieces = HashMap::new();
        let mut special_encoder = HashMap::new();
        let mut special_decoder = HashMap::new();
        let mut byte_fallback = HashMap::new();
        let mut max_piece_byte_len = 0usize;

        for (index, token) in tokenizer.vocabulary.tokens().iter().enumerate() {
            let token_id = TokenId(index as u32);
            let token_type = tokenizer.token_types.get(index).copied();
            if gguf_token_is_special(token, token_type) {
                special_encoder.insert(token.clone(), token_id.as_u32());
                special_decoder.insert(token_id.as_u32(), token.clone());
                continue;
            }
            ordinary_pieces.insert(
                token.clone(),
                SentencePieceRuntimePiece {
                    token: token_id,
                    score: if rank_like_scores {
                        0.0
                    } else {
                        tokenizer.scores.get(index).copied().unwrap_or(0.0)
                    },
                },
            );
            if let Some(byte) = sentencepiece_byte_fallback_value(token) {
                byte_fallback.insert(byte, token_id);
            }
            max_piece_byte_len = max_piece_byte_len.max(token.len());
        }

        Self {
            vocabulary: runtime_vocabulary(tokenizer),
            ordinary_pieces,
            special_regex: build_special_regex(&special_encoder).ok().flatten(),
            special_encoder,
            special_decoder,
            byte_fallback,
            max_piece_byte_len,
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
        let mut start = 0usize;
        let mut ordinary_starts_at_input_boundary = true;
        while start < text.len() {
            let next_special = self.find_next_special(text, start);
            let end = next_special.map_or(text.len(), |(match_start, _, _)| match_start);
            self.encode_ordinary_segment(
                &text[start..end],
                ordinary_starts_at_input_boundary,
                &mut tokens,
            );
            match next_special {
                Some((_, special_end, token_id)) => {
                    tokens.push(TokenId(token_id));
                    start = special_end;
                    ordinary_starts_at_input_boundary = false;
                }
                None => break,
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

    fn find_next_special(&self, text: &str, start: usize) -> Option<(usize, usize, u32)> {
        let regex = self.special_regex.as_ref()?;
        let matched = regex.find_from_pos(text, start).ok().flatten()?;
        let token = self
            .special_encoder
            .get(&text[matched.start()..matched.end()])?;
        Some((matched.start(), matched.end(), *token))
    }

    fn encode_ordinary_segment(
        &self,
        text: &str,
        starts_at_input_boundary: bool,
        out: &mut Vec<TokenId>,
    ) {
        if text.is_empty() {
            return;
        }
        let normalized = normalize_sentencepiece_segment(text, starts_at_input_boundary);
        if normalized.is_empty() {
            return;
        }
        out.extend(self.encode_sentencepiece_segment(normalized.as_str()));
    }

    fn encode_sentencepiece_segment(&self, normalized: &str) -> Vec<TokenId> {
        if normalized.is_empty() {
            return Vec::new();
        }
        let boundaries = char_boundaries(normalized);
        let last = boundaries.len().saturating_sub(1);
        let mut states = vec![None; boundaries.len()];
        let mut choices = vec![None; boundaries.len()];
        states[0] = Some(SentencePiecePathState {
            score: 0.0,
            token_count: 0,
        });

        for boundary_index in 0..last {
            let Some(current_state) = states[boundary_index] else {
                continue;
            };
            let start = boundaries[boundary_index];
            let mut matched = false;
            for next_index in boundary_index + 1..=last {
                let end = boundaries[next_index];
                if self.max_piece_byte_len > 0
                    && end.saturating_sub(start) > self.max_piece_byte_len
                {
                    break;
                }
                let piece = &normalized[start..end];
                let Some(entry) = self.ordinary_pieces.get(piece) else {
                    continue;
                };
                matched = true;
                let candidate_state = SentencePiecePathState {
                    score: current_state.score + entry.score,
                    token_count: current_state.token_count + 1,
                };
                if sentencepiece_state_is_better(candidate_state, states[next_index]) {
                    states[next_index] = Some(candidate_state);
                    choices[next_index] = Some(SentencePiecePathChoice {
                        previous_index: boundary_index,
                        tokens: vec![entry.token],
                    });
                }
            }
            if matched {
                continue;
            }
            let next_index = boundary_index + 1;
            let piece = &normalized[start..boundaries[next_index]];
            let fallback_tokens = self
                .byte_fallback_tokens(piece)
                .unwrap_or_else(|| vec![self.vocabulary.unknown_id()]);
            let candidate_state = SentencePiecePathState {
                score: current_state.score - 1_000.0,
                token_count: current_state.token_count + fallback_tokens.len(),
            };
            if sentencepiece_state_is_better(candidate_state, states[next_index]) {
                states[next_index] = Some(candidate_state);
                choices[next_index] = Some(SentencePiecePathChoice {
                    previous_index: boundary_index,
                    tokens: fallback_tokens,
                });
            }
        }

        let mut reconstructed = Vec::new();
        let mut cursor = last;
        while cursor > 0 {
            let Some(choice) = choices[cursor].clone() else {
                return vec![self.vocabulary.unknown_id()];
            };
            for token in choice.tokens.iter().rev() {
                reconstructed.push(*token);
            }
            cursor = choice.previous_index;
        }
        reconstructed.reverse();
        reconstructed
    }

    fn byte_fallback_tokens(&self, piece: &str) -> Option<Vec<TokenId>> {
        let mut tokens = Vec::with_capacity(piece.len());
        for byte in piece.as_bytes() {
            tokens.push(*self.byte_fallback.get(byte)?);
        }
        Some(tokens)
    }
}

impl TokenizerBoundary for SentencePieceRuntimeTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, false, false)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        let mut decoded = Vec::new();
        for token in tokens {
            if is_runtime_special_token(&self.vocabulary, self.eos_token_ids.as_slice(), *token) {
                continue;
            }
            let token_id = token.as_u32();
            if let Some(special) = self.special_decoder.get(&token_id) {
                decoded.extend_from_slice(special.as_bytes());
                continue;
            }
            let Some(piece) = self.vocabulary.token(*token) else {
                continue;
            };
            if let Some(byte) = sentencepiece_byte_fallback_value(piece) {
                decoded.push(byte);
                continue;
            }
            let expanded = piece.replace('▁', " ");
            decoded.extend_from_slice(expanded.as_bytes());
        }
        let mut text = String::from_utf8_lossy(decoded.as_slice()).into_owned();
        if text.starts_with(' ') {
            text.remove(0);
        }
        text
    }

    fn append_decoded_token(&self, text: &mut String, token: TokenId) {
        if is_runtime_special_token(&self.vocabulary, self.eos_token_ids.as_slice(), token) {
            return;
        }
        let token_id = token.as_u32();
        if let Some(special) = self.special_decoder.get(&token_id) {
            text.push_str(special);
            return;
        }
        let Some(piece) = self.vocabulary.token(token) else {
            return;
        };
        if let Some(byte) = sentencepiece_byte_fallback_value(piece) {
            text.push_str(String::from_utf8_lossy(&[byte]).as_ref());
            return;
        }
        let expanded = piece.replace('▁', " ");
        if text.is_empty() {
            text.push_str(expanded.strip_prefix(' ').unwrap_or(expanded.as_str()));
        } else {
            text.push_str(expanded.as_str());
        }
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

    fn append_decoded_token(&self, text: &mut String, token: TokenId) {
        if is_runtime_special_token(&self.vocabulary, self.eos_token_ids.as_slice(), token) {
            return;
        }
        let Some(piece) = self.vocabulary.token(token) else {
            return;
        };
        if let Some(piece) = piece.strip_prefix("##") {
            text.push_str(piece);
        } else {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(piece);
        }
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
        let filtered = tokens
            .iter()
            .copied()
            .filter(|token| {
                !is_runtime_special_token(&self.vocabulary, self.eos_token_ids.as_slice(), *token)
            })
            .collect::<Vec<_>>();
        self.bpe
            .decode_utf8(filtered.as_slice())
            .unwrap_or_else(|| {
                filtered
                    .iter()
                    .filter_map(|token| self.vocabulary.token(*token))
                    .collect::<Vec<_>>()
                    .join("")
            })
    }

    fn append_decoded_token(&self, text: &mut String, token: TokenId) {
        if is_runtime_special_token(&self.vocabulary, self.eos_token_ids.as_slice(), token) {
            return;
        }
        text.push_str(self.decode(&[token]).as_str());
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
    normalize_nfc: bool,
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
            ordinary_encoder
                .entry(raw_bytes.clone())
                .or_insert(token_id);
            ordinary_decoder.insert(token_id, raw_bytes);
        }

        let special_regex = build_special_regex(&special_encoder)?;
        let normalize_nfc = match tokenizer.pretokenizer.as_ref() {
            Some(GgufTokenizerPretokenizer::Qwen35) => true,
            Some(GgufTokenizerPretokenizer::Custom(value)) => value == "qwen35",
            _ => false,
        };
        Ok(Self {
            ordinary_encoder,
            ordinary_decoder,
            special_encoder,
            special_decoder,
            ordinary_regex,
            special_regex,
            normalize_nfc,
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
        let text = if self.normalize_nfc {
            Cow::Owned(text.nfc().collect::<String>())
        } else {
            Cow::Borrowed(text)
        };
        let matches = match self
            .ordinary_regex
            .find_iter(text.as_ref())
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
        | Some(GgufTokenizerPretokenizer::Gemma4)
        | Some(GgufTokenizerPretokenizer::Qwen2)
        | Some(GgufTokenizerPretokenizer::Qwen3)
        | Some(GgufTokenizerPretokenizer::Refact)
        | Some(GgufTokenizerPretokenizer::Tekken) => Ok(GENERIC_BYTE_LEVEL_BPE_PATTERN),
        Some(GgufTokenizerPretokenizer::Qwen35) => Ok(QWEN35_BYTE_LEVEL_BPE_PATTERN),
        Some(GgufTokenizerPretokenizer::Custom(value))
            if matches!(
                value.as_str(),
                "gpt-4o" | "default" | "qwen2" | "qwen3" | "llama-bpe" | "llama"
            ) =>
        {
            Ok(GENERIC_BYTE_LEVEL_BPE_PATTERN)
        }
        Some(GgufTokenizerPretokenizer::Custom(value)) if value == "qwen35" => {
            Ok(QWEN35_BYTE_LEVEL_BPE_PATTERN)
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
    let mut raw_bytes = Vec::with_capacity(token.len());
    for character in token.chars() {
        if let Some(byte) = unicode_to_byte.get(&character).copied() {
            raw_bytes.push(byte);
            continue;
        }
        let mut utf8 = [0u8; 4];
        raw_bytes.extend_from_slice(character.encode_utf8(&mut utf8).as_bytes());
    }
    Ok(raw_bytes)
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

#[derive(Clone, Copy, Debug)]
struct SentencePiecePathState {
    score: f32,
    token_count: usize,
}

#[derive(Clone, Debug)]
struct SentencePiecePathChoice {
    previous_index: usize,
    tokens: Vec<TokenId>,
}

fn sentencepiece_state_is_better(
    candidate: SentencePiecePathState,
    incumbent: Option<SentencePiecePathState>,
) -> bool {
    let Some(incumbent) = incumbent else {
        return true;
    };
    candidate.score > incumbent.score
        || ((candidate.score - incumbent.score).abs() <= f32::EPSILON
            && candidate.token_count < incumbent.token_count)
}

fn sentencepiece_scores_look_rank_like(tokenizer: &GgufTokenizerMetadata) -> bool {
    let mut ordinary_sample_count = 0usize;
    let mut rank_like_count = 0usize;
    for (index, token) in tokenizer.vocabulary.tokens().iter().enumerate() {
        let token_type = tokenizer.token_types.get(index).copied();
        if gguf_token_is_special(token, token_type) {
            continue;
        }
        let Some(score) = tokenizer.scores.get(index).copied() else {
            continue;
        };
        ordinary_sample_count += 1;
        if (score - index as f32).abs() <= f32::EPSILON {
            rank_like_count += 1;
        }
        if ordinary_sample_count >= 4096 {
            break;
        }
    }
    ordinary_sample_count >= 512
        && rank_like_count.saturating_mul(100) >= ordinary_sample_count.saturating_mul(95)
}

fn normalize_sentencepiece_segment(text: &str, starts_at_input_boundary: bool) -> String {
    let mut normalized = String::with_capacity(text.len().saturating_add(1));
    let mut pending_dummy_prefix = starts_at_input_boundary;
    let mut previous_was_space = false;
    for character in text.chars() {
        match character {
            ' ' | '\t' | '\r' => {
                if !previous_was_space {
                    normalized.push('▁');
                    previous_was_space = true;
                }
                pending_dummy_prefix = false;
            }
            _ => {
                if pending_dummy_prefix && !character.is_whitespace() {
                    normalized.push('▁');
                }
                normalized.push(character);
                pending_dummy_prefix = false;
                previous_was_space = false;
            }
        }
    }
    normalized
}

fn char_boundaries(text: &str) -> Vec<usize> {
    let mut boundaries = text
        .char_indices()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    boundaries.push(text.len());
    boundaries
}

fn sentencepiece_byte_fallback_value(token: &str) -> Option<u8> {
    let hex = token.strip_prefix("<0x")?.strip_suffix('>')?;
    if hex.len() != 2 {
        return None;
    }
    u8::from_str_radix(hex, 16).ok()
}

#[cfg(test)]
mod tests {
    use crate::{
        GgufContent, GgufRuntimeTokenizer, GgufTokenizerMetadata, GgufTokenizerModel, TokenId,
        TokenizerBoundary,
    };

    use super::{
        QWEN35_BYTE_LEVEL_BPE_PATTERN, SentencePieceRuntimeTokenizer, unicode_to_byte_map,
    };

    fn qwen35_pretokenized_pieces(input: &str) -> Vec<String> {
        fancy_regex::Regex::new(QWEN35_BYTE_LEVEL_BPE_PATTERN)
            .expect("qwen35 regex")
            .find_iter(input)
            .collect::<Result<Vec<_>, _>>()
            .expect("qwen35 matches")
            .into_iter()
            .map(|matched| matched.as_str().to_string())
            .collect()
    }

    #[test]
    fn qwen38_tokenizer_qwen35_pretokenizer_matches_published_unicode_and_digit_boundaries() {
        let ascii = "1 12 123 123456";
        let ascii_pieces = qwen35_pretokenized_pieces(ascii);
        assert_eq!(ascii_pieces.concat(), ascii);
        assert_eq!(
            ascii_pieces
                .iter()
                .filter(|piece| piece.chars().all(|character| character.is_numeric()))
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["1", "1", "2", "1", "2", "3", "1", "2", "3", "4", "5", "6"]
        );

        let unicode = "١٢ ４５";
        let unicode_pieces = qwen35_pretokenized_pieces(unicode);
        assert_eq!(unicode_pieces.concat(), unicode);
        assert_eq!(
            unicode_pieces
                .iter()
                .filter(|piece| piece.chars().all(|character| character.is_numeric()))
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["١", "٢", "４", "５"]
        );

        for text in [
            " cafe\u{301} résumé",
            "I'm WE'LL she'd",
            "word!!!\n\nnext",
            " leading \n  trailing ",
        ] {
            assert_eq!(qwen35_pretokenized_pieces(text).concat(), text);
        }
        assert!(
            qwen35_pretokenized_pieces(" cafe\u{301}")
                .iter()
                .any(|piece| piece == " cafe\u{301}")
        );
    }

    #[test]
    fn qwen38_tokenizer_qwen35_normalizes_ordinary_segments_to_nfc() {
        let mut tokens = vec![String::new(); 256];
        for (character, byte) in unicode_to_byte_map() {
            tokens[usize::from(byte)] = character.to_string();
        }
        let tokenizer = GgufRuntimeTokenizer::from_gguf(&GgufTokenizerMetadata {
            model: GgufTokenizerModel::Gpt2Bpe,
            vocabulary: crate::GgufTokenizerVocabulary {
                tokens,
                bos_token_id: None,
                eos_token_ids: Vec::new(),
                pad_token_id: None,
                unknown_token_id: None,
            },
            scores: vec![0.0; 256],
            token_types: vec![1; 256],
            merges: Vec::new(),
            add_bos: false,
            add_eos: false,
            pretokenizer: Some(crate::GgufTokenizerPretokenizer::Qwen35),
            token_type_count: None,
            digest: String::from("qwen35-nfc-test"),
        })
        .expect("qwen35 runtime tokenizer");

        assert_eq!(tokenizer.encode("é"), tokenizer.encode("e\u{301}"));
        assert_eq!(
            tokenizer.encode("é").as_slice(),
            &[TokenId(0xc3), TokenId(0xa9)]
        );
    }

    fn sentencepiece_test_tokenizer() -> GgufTokenizerMetadata {
        GgufTokenizerMetadata {
            model: GgufTokenizerModel::SentencePiece,
            vocabulary: crate::GgufTokenizerVocabulary {
                tokens: vec![
                    String::from("<unk>"),
                    String::from("<bos>"),
                    String::from("<eos>"),
                    String::from("<pad>"),
                    String::from("<|turn>"),
                    String::from("<turn|>"),
                    String::from("developer"),
                    String::from("\n"),
                    String::from("Be"),
                    String::from("▁terse"),
                    String::from("."),
                    String::from("▁Reply"),
                    String::from("▁with"),
                    String::from("▁exact"),
                    String::from("ly"),
                    String::from("▁one"),
                    String::from("▁short"),
                    String::from("▁grammatical"),
                    String::from("▁English"),
                    String::from("▁sentence"),
                ],
                bos_token_id: Some(TokenId(1)),
                eos_token_ids: vec![TokenId(2)],
                pad_token_id: Some(TokenId(3)),
                unknown_token_id: Some(TokenId(0)),
            },
            scores: vec![
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -0.1, -0.2, -0.3, -0.4, -0.5, -0.1, -0.2, -0.3, -0.4,
                -0.5, -0.6, -0.7, -0.8, -0.9,
            ],
            token_types: vec![2, 3, 3, 3, 3, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            merges: Vec::new(),
            add_bos: false,
            add_eos: false,
            pretokenizer: Some(crate::GgufTokenizerPretokenizer::Gemma4),
            token_type_count: None,
            digest: String::from("sentencepiece-test"),
        }
    }

    #[test]
    fn sentencepiece_runtime_encodes_special_tokens_and_subwords() {
        let tokenizer = SentencePieceRuntimeTokenizer::from_gguf(&sentencepiece_test_tokenizer());
        let encoded = tokenizer.encode("<|turn>developer\nBe terse.<turn|>");
        assert_eq!(
            encoded.as_slice(),
            &[
                TokenId(4),
                TokenId(6),
                TokenId(7),
                TokenId(8),
                TokenId(9),
                TokenId(10),
                TokenId(5),
            ]
        );
    }

    #[test]
    fn sentencepiece_runtime_decodes_without_inserting_fake_spaces() {
        let tokenizer = SentencePieceRuntimeTokenizer::from_gguf(&sentencepiece_test_tokenizer());
        let decoded = tokenizer.decode(&[
            TokenId(11),
            TokenId(12),
            TokenId(13),
            TokenId(14),
            TokenId(15),
            TokenId(16),
            TokenId(17),
            TokenId(18),
            TokenId(19),
            TokenId(10),
        ]);
        assert_eq!(
            decoded,
            "Reply with exactly one short grammatical English sentence."
        );
    }

    #[test]
    fn byte_level_gemma4_runtime_accepts_literal_newline_tokens() {
        let tokenizer = GgufRuntimeTokenizer::from_gguf(&GgufTokenizerMetadata {
            model: GgufTokenizerModel::Gemma4Bpe,
            vocabulary: crate::GgufTokenizerVocabulary {
                tokens: vec![
                    String::from("<bos>"),
                    String::from("<eos>"),
                    String::from("hello"),
                    String::from("\n"),
                    String::from("world"),
                ],
                bos_token_id: Some(TokenId(0)),
                eos_token_ids: vec![TokenId(1)],
                pad_token_id: None,
                unknown_token_id: None,
            },
            scores: Vec::new(),
            token_types: Vec::new(),
            merges: vec![String::from("h e")],
            add_bos: false,
            add_eos: false,
            pretokenizer: Some(crate::GgufTokenizerPretokenizer::Gemma4),
            token_type_count: None,
            digest: String::from("gemma4-byte-level-test"),
        })
        .expect("gemma4 runtime tokenizer");
        let encoded = tokenizer.encode("hello\nworld");
        assert!(!encoded.as_slice().is_empty());
        assert_eq!(tokenizer.decode(encoded.as_slice()), "hello\nworld");
    }

    #[test]
    fn byte_level_gemma4_runtime_tolerates_duplicate_byte_equivalent_tokens() {
        let tokenizer = GgufRuntimeTokenizer::from_gguf(&GgufTokenizerMetadata {
            model: GgufTokenizerModel::Gemma4Bpe,
            vocabulary: crate::GgufTokenizerVocabulary {
                tokens: vec![
                    String::from("<bos>"),
                    String::from("<eos>"),
                    String::from("\u{010A}"),
                    String::from("\n"),
                ],
                bos_token_id: Some(TokenId(0)),
                eos_token_ids: vec![TokenId(1)],
                pad_token_id: None,
                unknown_token_id: None,
            },
            scores: Vec::new(),
            token_types: Vec::new(),
            merges: Vec::new(),
            add_bos: false,
            add_eos: false,
            pretokenizer: Some(crate::GgufTokenizerPretokenizer::Gemma4),
            token_type_count: None,
            digest: String::from("gemma4-duplicate-byte-token-test"),
        })
        .expect("gemma4 runtime tokenizer");
        let encoded = tokenizer.encode("\n");
        assert!(!encoded.as_slice().is_empty());
        assert_eq!(tokenizer.decode(&[TokenId(2)]), "\n");
        assert_eq!(tokenizer.decode(&[TokenId(3)]), "\n");
    }

    #[test]
    fn byte_level_runtime_decode_ignores_runtime_special_tokens() {
        let tokenizer = GgufRuntimeTokenizer::from_gguf(&GgufTokenizerMetadata {
            model: GgufTokenizerModel::Gpt2Bpe,
            vocabulary: crate::GgufTokenizerVocabulary {
                tokens: vec![
                    String::from("<bos>"),
                    String::from("<eos>"),
                    String::from("Ġhello"),
                    String::from("Ġworld"),
                ],
                bos_token_id: Some(TokenId(0)),
                eos_token_ids: vec![TokenId(1)],
                pad_token_id: None,
                unknown_token_id: None,
            },
            scores: Vec::new(),
            token_types: vec![3, 3, 1, 1],
            merges: Vec::new(),
            add_bos: false,
            add_eos: false,
            pretokenizer: Some(crate::GgufTokenizerPretokenizer::Qwen35),
            token_type_count: None,
            digest: String::from("qwen35-byte-level-special-decode-test"),
        })
        .expect("qwen35 runtime tokenizer");
        assert_eq!(
            tokenizer.decode(&[TokenId(2), TokenId(1), TokenId(3)]),
            " hello world"
        );
    }

    #[test]
    fn gemma4_real_runtime_sentencepiece_prompt_avoids_unknown_collapse_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let Some(path) = std::env::var("PSIONIC_GEMMA4_PILOT_GGUF_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };
        if !std::path::Path::new(path.as_str()).exists() {
            return Ok(());
        }
        let content = GgufContent::read_path(std::path::Path::new(path.as_str()))?;
        let tokenizer = GgufRuntimeTokenizer::from_gguf(&content.load_tokenizer()?)?;
        let encoded =
            tokenizer.encode("Reply with exactly one short grammatical English sentence.");
        assert!(!encoded.as_slice().is_empty());
        assert!(
            encoded
                .as_slice()
                .iter()
                .all(|token| *token != tokenizer.vocabulary().unknown_id())
        );
        Ok(())
    }

    #[test]
    fn gemma4_real_runtime_sentencepiece_prefers_whole_pieces_when_scores_are_rank_like()
    -> Result<(), Box<dyn std::error::Error>> {
        let Some(path) = std::env::var("PSIONIC_GEMMA4_PILOT_GGUF_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };
        if !std::path::Path::new(path.as_str()).exists() {
            return Ok(());
        }
        let content = GgufContent::read_path(std::path::Path::new(path.as_str()))?;
        let tokenizer = GgufRuntimeTokenizer::from_gguf(&content.load_tokenizer()?)?;
        let encoded =
            tokenizer.encode("Reply with exactly one short grammatical English sentence.");
        assert!(
            encoded.len() < 20,
            "expected subword pieces, got {encoded:?}"
        );
        let first_token = encoded.as_slice()[0];
        let first_piece = &tokenizer.vocabulary().tokens()[first_token.as_u32() as usize];
        assert_eq!(first_piece, "▁Reply");
        Ok(())
    }
}
