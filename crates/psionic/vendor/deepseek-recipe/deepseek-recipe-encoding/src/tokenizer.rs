//! Encode rendered prompts into token IDs.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use tokenizers::{AddedVocabulary, Tokenizer};

/// An encoder that maps prompt text to model token IDs.
///
/// Implemented for [`PromptTokenizer`]; other tokenizers can implement it to
/// provide token encoding for a prompt encoding.
pub trait TokenizerEncoder: Send + Sync {
    /// Encode `text` into token IDs.
    ///
    /// `special_ranges` are the byte ranges of `text` that hold explicit
    /// special tokens. The caller must pass ranges in ascending order that do
    /// not overlap, lie within `text`, and start and end on `char` boundaries;
    /// [`RenderedPrompt::special_ranges`] meets these conditions.
    ///
    /// The implementation must encode each range as the token ID of its
    /// spelling and the plain text between the ranges without the added
    /// vocabulary, so a special-token spelling in plain text encodes as the
    /// token IDs of its characters. The implementation must add no tokens,
    /// such as a BOS token, beyond those that the text encodes to.
    ///
    /// # Errors
    ///
    /// Returns an error message when the implementation cannot encode `text`.
    ///
    /// [`RenderedPrompt::special_ranges`]: crate::RenderedPrompt::special_ranges
    fn encode_ids(&self, text: &str, special_ranges: &[Range<usize>]) -> Result<Vec<u32>, String>;
}

/// A tokenizer prepared for prompt encoding.
///
/// It holds a copy of the tokenizer without its added vocabulary, which
/// encodes plain text, and the added vocabulary, which supplies special token
/// IDs.
#[derive(Clone)]
pub struct PromptTokenizer {
    plain_tokenizer: Tokenizer,
    added_vocabulary: HashMap<String, u32>,
}

impl PromptTokenizer {
    /// Prepare `tokenizer` for prompt encoding. This clones the tokenizer once.
    pub fn new(tokenizer: &Tokenizer) -> Self {
        let added_vocabulary = tokenizer
            .get_added_vocabulary()
            .get_vocab()
            .iter()
            .map(|(token, &id)| (token.clone(), id))
            .collect();
        let mut plain_tokenizer = tokenizer.clone();
        plain_tokenizer.with_added_vocabulary(AddedVocabulary::new());
        Self {
            plain_tokenizer,
            added_vocabulary,
        }
    }

    fn special_token_id(&self, token: &str) -> Result<u32, String> {
        self.added_vocabulary
            .get(token)
            .copied()
            .ok_or_else(|| format!("special token {token:?} is not in the added vocabulary"))
    }
}

impl TokenizerEncoder for PromptTokenizer {
    /// Encode the plain text regions as one batch with the tokenizer without
    /// its added vocabulary, and each special range as the added-vocabulary
    /// token ID of its spelling. The batch runs in parallel unless the
    /// `TOKENIZERS_PARALLELISM` environment variable disables it. The
    /// tokenizer adds no tokens, such as a BOS token, beyond those that the
    /// text encodes to, provided that it has no padding configured.
    ///
    /// # Errors
    ///
    /// Returns an error message when the tokenizer fails to encode a plain
    /// text region or the spelling of a special range is not in the added
    /// vocabulary.
    ///
    /// # Panics
    ///
    /// Panics if a range starts before the end of the previous range, ends
    /// before it starts, extends past the end of `text`, or does not start and
    /// end on `char` boundaries.
    fn encode_ids(&self, text: &str, special_ranges: &[Range<usize>]) -> Result<Vec<u32>, String> {
        // The plain region before each special range, then the one after the
        // last range; a region can be empty.
        let mut regions = Vec::with_capacity(special_ranges.len() + 1);
        let mut start = 0;
        for range in special_ranges {
            regions.push(&text[start..range.start]);
            start = range.end;
        }
        regions.push(&text[start..]);
        let encodings = self
            .plain_tokenizer
            .encode_batch_fast(regions, false)
            .map_err(|error| error.to_string())?;

        let mut ids = Vec::new();
        let mut encodings = encodings.iter();
        for range in special_ranges {
            if let Some(encoding) = encodings.next() {
                ids.extend_from_slice(encoding.get_ids());
            }
            ids.push(self.special_token_id(&text[range.clone()])?);
        }
        if let Some(encoding) = encodings.next() {
            ids.extend_from_slice(encoding.get_ids());
        }
        Ok(ids)
    }
}

impl<T: TokenizerEncoder + ?Sized> TokenizerEncoder for Arc<T> {
    fn encode_ids(&self, text: &str, special_ranges: &[Range<usize>]) -> Result<Vec<u32>, String> {
        (**self).encode_ids(text, special_ranges)
    }
}
