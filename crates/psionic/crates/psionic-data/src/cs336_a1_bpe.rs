use std::{collections::HashMap, fs, path::Path};

use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{TokenizerDigest, TokenizerFamily};

/// Stable schema version for the bounded CS336 A1 BPE artifact bundle.
pub const CS336_A1_BYTE_PAIR_ENCODING_ARTIFACTS_SCHEMA_VERSION: &str =
    "psion.cs336_a1.byte_pair_encoding_artifacts.v1";

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

/// One retained byte-level BPE artifact bundle for the CS336 A1 reference lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A1BytePairEncodingArtifacts {
    /// Stable schema version.
    pub schema_version: String,
    /// Requested vocabulary size.
    pub vocab_size: u32,
    /// Stable special-token inventory.
    pub special_tokens: Vec<String>,
    /// Number of special tokens anchored at the start of the vocabulary.
    pub special_token_count: u32,
    /// Vocabulary entries in stable token-id order.
    ///
    /// Ordinary byte-level entries use the GPT-2 printable-unicode byte remap.
    /// Special-token entries remain literal UTF-8 strings.
    pub vocab: Vec<String>,
    /// Ordered merge list in stable training order.
    pub merges: Vec<[String; 2]>,
    /// Stable digest over the training corpus bytes.
    pub corpus_digest: String,
    /// Stable digest over the vocabulary bytes.
    pub vocab_digest: String,
    /// Stable digest over the ordered merge bytes.
    pub merges_digest: String,
    /// Stable tokenizer digest surfaced to later dataset manifests.
    pub tokenizer_digest: TokenizerDigest,
}

impl Cs336A1BytePairEncodingArtifacts {
    /// Reconstructs the raw vocabulary bytes in stable token-id order.
    pub fn vocabulary_bytes(&self) -> Result<Vec<Vec<u8>>, Cs336A1BytePairEncodingError> {
        let special_count = self.special_token_count as usize;
        self.vocab
            .iter()
            .enumerate()
            .map(|(index, token)| {
                if index < special_count {
                    Ok(token.as_bytes().to_vec())
                } else {
                    decode_gpt2_byte_string(token)
                }
            })
            .collect()
    }

    /// Reconstructs the raw merge-byte pairs in stable training order.
    pub fn merge_pairs_bytes(
        &self,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Cs336A1BytePairEncodingError> {
        self.merges
            .iter()
            .map(|pair| {
                Ok((
                    decode_gpt2_byte_string(pair[0].as_str())?,
                    decode_gpt2_byte_string(pair[1].as_str())?,
                ))
            })
            .collect()
    }
}

/// Errors returned by the bounded CS336 A1 BPE trainer.
#[derive(Debug, Error)]
pub enum Cs336A1BytePairEncodingError {
    #[error("failed to read training corpus `{path}`: {source}")]
    ReadCorpus {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to compile CS336 A1 byte-level BPE regex: {0}")]
    InvalidPattern(String),
    #[error("invalid CS336 A1 BPE configuration: {0}")]
    InvalidConfig(String),
    #[error("failed to tokenize the training corpus: {0}")]
    Pretokenization(String),
    #[error("failed to decode GPT-2 byte string `{token}` back into raw bytes")]
    InvalidEncodedToken { token: String },
}

/// Trains one bounded byte-level BPE tokenizer from a corpus path.
pub fn train_cs336_a1_byte_pair_encoding_from_path(
    input_path: impl AsRef<Path>,
    vocab_size: u32,
    special_tokens: &[String],
) -> Result<Cs336A1BytePairEncodingArtifacts, Cs336A1BytePairEncodingError> {
    let input_path = input_path.as_ref();
    let corpus = fs::read_to_string(input_path).map_err(|source| {
        Cs336A1BytePairEncodingError::ReadCorpus {
            path: input_path.display().to_string(),
            source,
        }
    })?;
    train_cs336_a1_byte_pair_encoding_from_text(corpus.as_str(), vocab_size, special_tokens)
}

/// Trains one bounded byte-level BPE tokenizer from in-memory text.
pub fn train_cs336_a1_byte_pair_encoding_from_text(
    corpus: &str,
    vocab_size: u32,
    special_tokens: &[String],
) -> Result<Cs336A1BytePairEncodingArtifacts, Cs336A1BytePairEncodingError> {
    validate_special_tokens(special_tokens)?;

    let special_token_count = special_tokens.len() as u32;
    let minimum_vocab_size = 256_u32 + special_token_count;
    if vocab_size < minimum_vocab_size {
        return Err(Cs336A1BytePairEncodingError::InvalidConfig(format!(
            "vocab_size {vocab_size} is smaller than the required base size {minimum_vocab_size}"
        )));
    }

    let regex = Regex::new(CS336_A1_BYTE_LEVEL_BPE_PATTERN)
        .map_err(|error| Cs336A1BytePairEncodingError::InvalidPattern(error.to_string()))?;
    let mut token_entries = special_tokens
        .iter()
        .map(|token| token.as_bytes().to_vec())
        .collect::<Vec<_>>();
    let mut token_lookup = HashMap::new();
    for (index, token) in token_entries.iter().enumerate() {
        token_lookup.insert(token.clone(), index as u32);
    }
    for byte in u8::MIN..=u8::MAX {
        let token_id = token_entries.len() as u32;
        let token = vec![byte];
        token_lookup.insert(token.clone(), token_id);
        token_entries.push(token);
    }

    let mut words = build_word_frequencies(corpus, &regex, special_token_count)?;
    let target_merges = (vocab_size - minimum_vocab_size) as usize;
    let mut merges = Vec::with_capacity(target_merges);

    while merges.len() < target_merges {
        let pair_counts = count_pairs(words.as_slice());
        let Some((left, right)) =
            select_best_pair(pair_counts, token_entries.as_slice(), &token_lookup)
        else {
            break;
        };

        let merged_bytes = concat_bytes(
            token_entries[left as usize].as_slice(),
            token_entries[right as usize].as_slice(),
        );
        let merged_id = token_entries.len() as u32;
        token_lookup.insert(merged_bytes.clone(), merged_id);
        token_entries.push(merged_bytes.clone());
        merges.push((
            token_entries[left as usize].clone(),
            token_entries[right as usize].clone(),
        ));

        for word in &mut words {
            word.replace_pair(left, right, merged_id);
        }
    }

    let vocab = token_entries
        .iter()
        .enumerate()
        .map(|(index, token)| {
            if index < special_token_count as usize {
                String::from_utf8(token.clone()).map_err(|_| {
                    Cs336A1BytePairEncodingError::InvalidConfig(
                        "special tokens must be valid UTF-8".to_string(),
                    )
                })
            } else {
                Ok(encode_gpt2_byte_string(token))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let merge_strings = merges
        .iter()
        .map(|(left, right)| {
            [
                encode_gpt2_byte_string(left),
                encode_gpt2_byte_string(right),
            ]
        })
        .collect::<Vec<_>>();

    let corpus_digest = stable_digest_for_bytes(
        b"psion.cs336_a1.byte_pair_encoding.corpus",
        corpus.as_bytes(),
    );
    let vocab_digest = stable_digest_for_tokens(
        b"psion.cs336_a1.byte_pair_encoding.vocab",
        token_entries.as_slice(),
    );
    let merges_digest = stable_digest_for_merges(
        b"psion.cs336_a1.byte_pair_encoding.merges",
        merges.as_slice(),
    );
    let special_digest = stable_digest_for_strings(
        b"psion.cs336_a1.byte_pair_encoding.special_tokens",
        special_tokens,
    );
    let tokenizer_contract_digest = stable_digest_for_strings(
        b"psion.cs336_a1.byte_pair_encoding.contract",
        &[
            corpus_digest.clone(),
            vocab_digest.clone(),
            merges_digest.clone(),
            special_digest.clone(),
        ],
    );

    Ok(Cs336A1BytePairEncodingArtifacts {
        schema_version: String::from(CS336_A1_BYTE_PAIR_ENCODING_ARTIFACTS_SCHEMA_VERSION),
        vocab_size: token_entries.len() as u32,
        special_tokens: special_tokens.to_vec(),
        special_token_count,
        vocab,
        merges: merge_strings,
        corpus_digest,
        vocab_digest,
        merges_digest,
        tokenizer_digest: TokenizerDigest::new(
            TokenizerFamily::BytePairEncoding,
            tokenizer_contract_digest,
            token_entries.len() as u32,
        )
        .with_special_tokens_digest(special_digest),
    })
}

#[derive(Clone, Debug)]
struct WordFrequency {
    tokens: Vec<u32>,
    count: u64,
}

impl WordFrequency {
    fn replace_pair(&mut self, left: u32, right: u32, merged_id: u32) {
        if self.tokens.len() < 2 {
            return;
        }
        let mut replaced = Vec::with_capacity(self.tokens.len());
        let mut index = 0_usize;
        while index < self.tokens.len() {
            if index + 1 < self.tokens.len()
                && self.tokens[index] == left
                && self.tokens[index + 1] == right
            {
                replaced.push(merged_id);
                index += 2;
            } else {
                replaced.push(self.tokens[index]);
                index += 1;
            }
        }
        self.tokens = replaced;
    }
}

fn validate_special_tokens(special_tokens: &[String]) -> Result<(), Cs336A1BytePairEncodingError> {
    let mut seen = HashMap::new();
    for token in special_tokens {
        if token.is_empty() {
            return Err(Cs336A1BytePairEncodingError::InvalidConfig(
                "special tokens must be non-empty".to_string(),
            ));
        }
        if seen.insert(token.as_str(), ()).is_some() {
            return Err(Cs336A1BytePairEncodingError::InvalidConfig(format!(
                "duplicate special token `{token}`"
            )));
        }
    }
    Ok(())
}

fn build_word_frequencies(
    corpus: &str,
    regex: &Regex,
    special_token_count: u32,
) -> Result<Vec<WordFrequency>, Cs336A1BytePairEncodingError> {
    let matches = regex
        .find_iter(corpus)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| Cs336A1BytePairEncodingError::Pretokenization(error.to_string()))?;
    let mut counts = HashMap::<Vec<u32>, u64>::new();
    for matched in matches {
        let piece = matched.as_str().as_bytes();
        if piece.is_empty() {
            continue;
        }
        let tokenized = piece
            .iter()
            .map(|byte| special_token_count + u32::from(*byte))
            .collect::<Vec<_>>();
        *counts.entry(tokenized).or_insert(0) += 1;
    }
    Ok(counts
        .into_iter()
        .map(|(tokens, count)| WordFrequency { tokens, count })
        .collect())
}

fn count_pairs(words: &[WordFrequency]) -> HashMap<(u32, u32), u64> {
    let mut counts = HashMap::new();
    for word in words {
        for pair in word.tokens.windows(2) {
            *counts.entry((pair[0], pair[1])).or_insert(0) += word.count;
        }
    }
    counts
}

fn select_best_pair(
    pair_counts: HashMap<(u32, u32), u64>,
    token_entries: &[Vec<u8>],
    token_lookup: &HashMap<Vec<u8>, u32>,
) -> Option<(u32, u32)> {
    let mut best_pair = None;
    let mut best_count = 0_u64;
    for (pair, count) in pair_counts {
        let merged = concat_bytes(
            token_entries[pair.0 as usize].as_slice(),
            token_entries[pair.1 as usize].as_slice(),
        );
        if token_lookup.contains_key(&merged) {
            continue;
        }
        let replace = match best_pair {
            None => true,
            Some(_) if count > best_count => true,
            Some(current) if count == best_count => {
                compare_pair_bytes(pair, current, token_entries).is_gt()
            }
            Some(_) => false,
        };
        if replace {
            best_pair = Some(pair);
            best_count = count;
        }
    }
    best_pair
}

fn compare_pair_bytes(
    left: (u32, u32),
    right: (u32, u32),
    token_entries: &[Vec<u8>],
) -> std::cmp::Ordering {
    token_entries[left.0 as usize]
        .cmp(&token_entries[right.0 as usize])
        .then_with(|| token_entries[left.1 as usize].cmp(&token_entries[right.1 as usize]))
}

fn concat_bytes(left: &[u8], right: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(left.len() + right.len());
    out.extend_from_slice(left);
    out.extend_from_slice(right);
    out
}

fn stable_digest_for_bytes(namespace: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace);
    hasher.update(b"|");
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn stable_digest_for_strings(namespace: &[u8], values: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace);
    for value in values {
        hasher.update(b"|");
        hasher.update(value.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_digest_for_tokens(namespace: &[u8], values: &[Vec<u8>]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace);
    for value in values {
        hasher.update(b"|");
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value);
    }
    hex::encode(hasher.finalize())
}

fn stable_digest_for_merges(namespace: &[u8], values: &[(Vec<u8>, Vec<u8>)]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace);
    for (left, right) in values {
        hasher.update(b"|");
        hasher.update((left.len() as u64).to_le_bytes());
        hasher.update(left);
        hasher.update(b"|");
        hasher.update((right.len() as u64).to_le_bytes());
        hasher.update(right);
    }
    hex::encode(hasher.finalize())
}

fn encode_gpt2_byte_string(bytes: &[u8]) -> String {
    let byte_to_unicode = gpt2_byte_to_unicode_map();
    bytes
        .iter()
        .map(|byte| byte_to_unicode[*byte as usize])
        .collect::<String>()
}

fn decode_gpt2_byte_string(token: &str) -> Result<Vec<u8>, Cs336A1BytePairEncodingError> {
    let unicode_to_byte = unicode_to_gpt2_byte_map();
    token
        .chars()
        .map(|character| {
            unicode_to_byte.get(&character).copied().ok_or_else(|| {
                Cs336A1BytePairEncodingError::InvalidEncodedToken {
                    token: token.to_string(),
                }
            })
        })
        .collect()
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

fn unicode_to_gpt2_byte_map() -> HashMap<char, u8> {
    let mut mapping = HashMap::new();
    for (byte, character) in gpt2_byte_to_unicode_map().iter().enumerate() {
        mapping.insert(*character, byte as u8);
    }
    mapping
}

#[cfg(test)]
mod tests {
    use super::{
        decode_gpt2_byte_string, train_cs336_a1_byte_pair_encoding_from_text,
        Cs336A1BytePairEncodingArtifacts, CS336_A1_BYTE_PAIR_ENCODING_ARTIFACTS_SCHEMA_VERSION,
    };

    #[test]
    fn trainer_requires_base_vocabulary_room() {
        let error = train_cs336_a1_byte_pair_encoding_from_text("", 255, &[])
            .expect_err("vocab smaller than the byte inventory should fail");
        assert!(error
            .to_string()
            .contains("smaller than the required base size"));
    }

    #[test]
    fn trainer_uses_lexicographically_greatest_pair_for_ties() {
        let artifacts = train_cs336_a1_byte_pair_encoding_from_text("ab ac", 257, &[])
            .expect("trainer should succeed");
        let first_merge = artifacts
            .merge_pairs_bytes()
            .expect("merge bytes should decode")
            .into_iter()
            .next()
            .expect("at least one merge should exist");
        assert_eq!(first_merge, (b"a".to_vec(), b"c".to_vec()));
    }

    #[test]
    fn trainer_emits_reconstructible_artifacts() {
        let artifacts = train_cs336_a1_byte_pair_encoding_from_text(
            "the theater is in the attic\n<|endoftext|> the theater is in the attic\n",
            270,
            &[String::from("<|endoftext|>")],
        )
        .expect("trainer should succeed");
        assert_eq!(
            artifacts.schema_version,
            CS336_A1_BYTE_PAIR_ENCODING_ARTIFACTS_SCHEMA_VERSION
        );
        assert_eq!(artifacts.special_token_count, 1);
        assert_eq!(
            artifacts.vocab.first().expect("special token"),
            "<|endoftext|>"
        );
        assert_eq!(
            artifacts.vocabulary_bytes().expect("decode vocab")[0],
            b"<|endoftext|>"
        );
        assert!(
            !artifacts.merges.is_empty(),
            "larger vocab should include at least one merge"
        );
        assert_eq!(
            artifacts.tokenizer_digest.family,
            crate::TokenizerFamily::BytePairEncoding
        );
    }

    #[test]
    fn ordinary_tokens_round_trip_through_gpt2_byte_mapping() {
        let artifacts = Cs336A1BytePairEncodingArtifacts {
            schema_version: String::from(CS336_A1_BYTE_PAIR_ENCODING_ARTIFACTS_SCHEMA_VERSION),
            vocab_size: 257,
            special_tokens: Vec::new(),
            special_token_count: 0,
            vocab: vec![String::from("hello"), String::from("Ġworld")],
            merges: vec![[String::from("Ġ"), String::from("w")]],
            corpus_digest: String::from("corpus"),
            vocab_digest: String::from("vocab"),
            merges_digest: String::from("merges"),
            tokenizer_digest: crate::TokenizerDigest::new(
                crate::TokenizerFamily::BytePairEncoding,
                "digest",
                257,
            ),
        };
        let vocabulary_bytes = artifacts.vocabulary_bytes().expect("decode vocab");
        assert_eq!(vocabulary_bytes[0], b"hello");
        assert_eq!(vocabulary_bytes[1], b" world");
        let merge_bytes = artifacts.merge_pairs_bytes().expect("decode merges");
        assert_eq!(merge_bytes[0], (b" ".to_vec(), b"w".to_vec()));
        assert_eq!(
            decode_gpt2_byte_string("HÃ©").expect("decode mapped token"),
            "Hé".as_bytes()
        );
    }
}
