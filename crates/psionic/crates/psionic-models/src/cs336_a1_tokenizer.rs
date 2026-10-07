use std::{collections::HashMap, io::Read};

use fancy_regex::Regex;
use thiserror::Error;

use crate::{TokenId, TokenSequence, TokenVocabulary, TokenizerBoundary};

const CS336_A1_BYTE_LEVEL_BPE_PATTERN: &str = concat!(
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Cs336A1BytePairTokenizerError {
    #[error("invalid CS336 A1 tokenizer contract: {0}")]
    InvalidContract(String),
    #[error("failed to compile the CS336 A1 tokenizer regex: {0}")]
    InvalidPattern(String),
    #[error("failed to read streaming tokenizer input: {0}")]
    ReadStream(String),
}

#[derive(Clone, Debug)]
pub struct Cs336A1BytePairTokenizer {
    core: Cs336A1BytePairTokenizerCore,
    vocabulary: TokenVocabulary,
}

impl Cs336A1BytePairTokenizer {
    pub fn from_vocab_and_merges(
        vocab: &[Vec<u8>],
        merges: &[(Vec<u8>, Vec<u8>)],
        special_tokens: &[String],
    ) -> Result<Self, Cs336A1BytePairTokenizerError> {
        validate_contract(vocab, merges, special_tokens)?;
        let special_count = special_tokens.len();
        let ordinary_regex = Regex::new(CS336_A1_BYTE_LEVEL_BPE_PATTERN)
            .map_err(|error| Cs336A1BytePairTokenizerError::InvalidPattern(error.to_string()))?;

        let mut ordinary_encoder = HashMap::new();
        let mut ordinary_decoder = HashMap::new();
        let mut special_encoder = HashMap::new();
        let mut special_decoder = HashMap::new();

        for (index, token) in vocab.iter().enumerate() {
            let token_id = index as u32;
            if index < special_count {
                let special = String::from_utf8(token.clone()).map_err(|_| {
                    Cs336A1BytePairTokenizerError::InvalidContract(
                        "special tokens must be valid UTF-8".to_string(),
                    )
                })?;
                special_encoder.insert(special.clone(), token_id);
                special_decoder.insert(token_id, special);
            } else {
                ordinary_encoder.insert(token.clone(), token_id);
                ordinary_decoder.insert(token_id, token.clone());
            }
        }

        let special_regex = build_special_regex(&special_encoder)?;
        let display_tokens = vocab
            .iter()
            .enumerate()
            .map(|(index, token)| {
                if index < special_count {
                    String::from_utf8(token.clone()).map_err(|_| {
                        Cs336A1BytePairTokenizerError::InvalidContract(
                            "special tokens must be valid UTF-8".to_string(),
                        )
                    })
                } else {
                    Ok(encode_gpt2_byte_string(token))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let fallback_id = if special_count > 0 {
            0
        } else {
            special_count as u32
        };
        Ok(Self {
            core: Cs336A1BytePairTokenizerCore {
                ordinary_encoder,
                ordinary_decoder,
                special_encoder,
                special_decoder,
                ordinary_regex,
                special_regex,
            },
            vocabulary: TokenVocabulary::new(
                display_tokens,
                TokenId(fallback_id),
                TokenId(fallback_id),
                TokenId(fallback_id),
                TokenId(fallback_id),
            ),
        })
    }

    #[must_use]
    pub fn encode_with_special_tokens(&self, text: &str) -> TokenSequence {
        TokenSequence::new(
            self.core
                .encode_with_special_tokens(text)
                .into_iter()
                .map(TokenId)
                .collect::<Vec<_>>(),
        )
    }

    pub fn encode_iterable<R: Read>(
        &self,
        mut reader: R,
    ) -> Result<std::vec::IntoIter<TokenId>, Cs336A1BytePairTokenizerError> {
        let mut text = String::new();
        reader
            .read_to_string(&mut text)
            .map_err(|error| Cs336A1BytePairTokenizerError::ReadStream(error.to_string()))?;
        Ok(self.encode(text.as_str()).as_slice().to_vec().into_iter())
    }
}

impl TokenizerBoundary for Cs336A1BytePairTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        self.core.decode(tokens)
    }

    fn append_decoded_token(&self, text: &mut String, token: TokenId) {
        text.push_str(self.decode(&[token]).as_str());
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        &self.vocabulary
    }
}

#[derive(Clone, Debug)]
struct Cs336A1BytePairTokenizerCore {
    ordinary_encoder: HashMap<Vec<u8>, u32>,
    ordinary_decoder: HashMap<u32, Vec<u8>>,
    special_encoder: HashMap<String, u32>,
    special_decoder: HashMap<u32, String>,
    ordinary_regex: Regex,
    special_regex: Option<Regex>,
}

impl Cs336A1BytePairTokenizerCore {
    fn encode_with_special_tokens(&self, text: &str) -> Vec<u32> {
        let mut tokens = Vec::new();
        let mut start = 0_usize;
        while start < text.len() {
            let next_special = self.find_next_special(text, start);
            let end = next_special.map_or(text.len(), |(special_start, _, _)| special_start);
            self.encode_ordinary_segment(&text[start..end], &mut tokens);
            match next_special {
                Some((_, special_end, token_id)) => {
                    tokens.push(token_id);
                    start = special_end;
                }
                None => break,
            }
        }
        tokens
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        self.decode_utf8(tokens).unwrap_or_else(|| {
            tokens
                .iter()
                .filter_map(|token| {
                    let token_id = token.as_u32();
                    self.special_decoder.get(&token_id).cloned().or_else(|| {
                        self.ordinary_decoder
                            .get(&token_id)
                            .map(|bytes| String::from_utf8_lossy(bytes).to_string())
                    })
                })
                .collect::<Vec<_>>()
                .join("")
        })
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

fn validate_contract(
    vocab: &[Vec<u8>],
    merges: &[(Vec<u8>, Vec<u8>)],
    special_tokens: &[String],
) -> Result<(), Cs336A1BytePairTokenizerError> {
    let special_count = special_tokens.len();
    let expected_len = special_count + 256 + merges.len();
    if vocab.len() != expected_len {
        return Err(Cs336A1BytePairTokenizerError::InvalidContract(format!(
            "expected vocab length {expected_len}, got {}",
            vocab.len()
        )));
    }
    for (index, token) in special_tokens.iter().enumerate() {
        if vocab[index].as_slice() != token.as_bytes() {
            return Err(Cs336A1BytePairTokenizerError::InvalidContract(format!(
                "special token `{token}` is not anchored at token id {index}"
            )));
        }
    }
    for byte in u8::MIN..=u8::MAX {
        let token_index = special_count + byte as usize;
        if vocab[token_index].as_slice() != [byte] {
            return Err(Cs336A1BytePairTokenizerError::InvalidContract(format!(
                "expected raw byte token {:02x} at token id {token_index}",
                byte
            )));
        }
    }
    for (merge_index, (left, right)) in merges.iter().enumerate() {
        let token_index = special_count + 256 + merge_index;
        let mut expected = Vec::with_capacity(left.len() + right.len());
        expected.extend_from_slice(left);
        expected.extend_from_slice(right);
        if vocab[token_index] != expected {
            return Err(Cs336A1BytePairTokenizerError::InvalidContract(format!(
                "merge token at id {token_index} does not match the ordered merge list"
            )));
        }
    }
    Ok(())
}

fn build_special_regex(
    special_encoder: &HashMap<String, u32>,
) -> Result<Option<Regex>, Cs336A1BytePairTokenizerError> {
    if special_encoder.is_empty() {
        return Ok(None);
    }
    let mut tokens = special_encoder
        .keys()
        .map(|token| fancy_regex::escape(token))
        .collect::<Vec<_>>();
    tokens.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    Regex::new(tokens.join("|").as_str())
        .map(Some)
        .map_err(|error| Cs336A1BytePairTokenizerError::InvalidPattern(error.to_string()))
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

fn encode_gpt2_byte_string(bytes: &[u8]) -> String {
    let mapping = gpt2_byte_to_unicode_map();
    bytes.iter().map(|byte| mapping[*byte as usize]).collect()
}

fn gpt2_byte_to_unicode_map() -> [char; 256] {
    let mut characters = ['\0'; 256];
    let mut assigned = [false; 256];
    for byte in 0x21_u32..=0x7e {
        characters[byte as usize] = char::from_u32(byte).unwrap_or('\0');
        assigned[byte as usize] = true;
    }
    for byte in 0xa1_u32..=0xac {
        characters[byte as usize] = char::from_u32(byte).unwrap_or('\0');
        assigned[byte as usize] = true;
    }
    for byte in 0xae_u32..=0xff {
        characters[byte as usize] = char::from_u32(byte).unwrap_or('\0');
        assigned[byte as usize] = true;
    }
    let mut next_codepoint = 256_u32;
    for (byte, is_assigned) in assigned.iter().enumerate() {
        if *is_assigned {
            continue;
        }
        characters[byte] = char::from_u32(next_codepoint).unwrap_or('\0');
        next_codepoint += 1;
    }
    characters
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{Cs336A1BytePairTokenizer, TokenizerBoundary};

    fn manual_vocab_with_merge(
        special_tokens: &[String],
    ) -> (Vec<Vec<u8>>, Vec<(Vec<u8>, Vec<u8>)>) {
        let mut vocab = special_tokens
            .iter()
            .map(|token| token.as_bytes().to_vec())
            .collect::<Vec<_>>();
        for byte in u8::MIN..=u8::MAX {
            vocab.push(vec![byte]);
        }
        let merges = vec![(b"h".to_vec(), b"e".to_vec())];
        vocab.push(b"he".to_vec());
        (vocab, merges)
    }

    #[test]
    fn tokenizer_round_trips_text_and_special_tokens() {
        let special_tokens = vec![String::from("<|endoftext|>")];
        let (vocab, merges) = manual_vocab_with_merge(&special_tokens);
        let tokenizer = Cs336A1BytePairTokenizer::from_vocab_and_merges(
            vocab.as_slice(),
            merges.as_slice(),
            special_tokens.as_slice(),
        )
        .expect("tokenizer should build");
        let text = "he<|endoftext|>he";
        let encoded = tokenizer.encode(text);
        assert_eq!(encoded.len(), 3);
        assert_eq!(tokenizer.decode(encoded.as_slice()), text);
    }

    #[test]
    fn tokenizer_prefers_the_longest_special_token() {
        let special_tokens = vec![
            String::from("<|endoftext|>"),
            String::from("<|endoftext|><|endoftext|>"),
        ];
        let (vocab, merges) = manual_vocab_with_merge(&special_tokens);
        let tokenizer = Cs336A1BytePairTokenizer::from_vocab_and_merges(
            vocab.as_slice(),
            merges.as_slice(),
            special_tokens.as_slice(),
        )
        .expect("tokenizer should build");
        let text = "he<|endoftext|><|endoftext|>he";
        let encoded = tokenizer.encode(text);
        assert_eq!(encoded.len(), 3);
        assert_eq!(tokenizer.decode(encoded.as_slice()), text);
    }

    #[test]
    fn tokenizer_streaming_surface_matches_direct_encoding() {
        let special_tokens = vec![String::from("<|endoftext|>")];
        let (vocab, merges) = manual_vocab_with_merge(&special_tokens);
        let tokenizer = Cs336A1BytePairTokenizer::from_vocab_and_merges(
            vocab.as_slice(),
            merges.as_slice(),
            special_tokens.as_slice(),
        )
        .expect("tokenizer should build");
        let text = "he\n<|endoftext|>\nhe";
        let streamed = tokenizer
            .encode_iterable(Cursor::new(text.as_bytes()))
            .expect("streaming encode should succeed")
            .collect::<Vec<_>>();
        assert_eq!(streamed, tokenizer.encode(text).as_slice());
    }
}
