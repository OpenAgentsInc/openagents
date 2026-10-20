//! Decode token IDs into text for the stream processor.

use std::sync::Arc;

use tokenizers::Tokenizer;

use super::StreamError;
use super::logprobs::{InferenceTokenLogprob, TokenLogprob, create_logprobs};

/// Replacement character that decoding produces while a multi-token character
/// is incomplete. Decoded text that ends with it stays buffered, so a token
/// that decodes to U+FFFD itself is held until a later token ID completes the
/// text, and is dropped at the end of input.
const REPLACEMENT_CHARACTER: char = '\u{FFFD}';

/// Decode token IDs into text.
///
/// Implemented for [`Tokenizer`]; other tokenizer implementations can implement
/// it to provide decoding for a stream processor.
///
/// # Implementor obligations
///
/// - [`decode_ids`](Self::decode_ids) must decode an incomplete UTF-8 sequence
///   at the end of `ids` as U+FFFD, so that the stream processor buffers the
///   token IDs until the character is complete.
/// - With `skip_special_tokens` set to `false`, `decode_ids` must keep special
///   tokens in the text, because output parsing recognizes them.
/// - [`id_to_token`](Self::id_to_token) must return the byte-level vocabulary
///   string, such as `Ġhi`, from which log probabilities recover token bytes.
/// - [`is_special_token`](Self::is_special_token) must return `true` for every
///   token ID of the added vocabulary, so that the stream processor decodes
///   each special token as its own piece and output parsing matches a marker
///   only against a token, not against text that spells the same.
pub trait StreamTokenizer: Send {
    /// Decode `ids` into text, skipping special tokens when requested.
    ///
    /// # Errors
    ///
    /// Returns the tokenizer's failure message when `ids` cannot be decoded.
    fn decode_ids(&self, ids: &[u32], skip_special_tokens: bool) -> Result<String, String>;

    /// Return the vocabulary string of `id`, used for log probabilities.
    /// The default implementation returns `None`; the log probability of such
    /// a token then has an empty `token` and `bytes`.
    fn id_to_token(&self, _id: u32) -> Option<String> {
        None
    }

    /// Return the number of tokens `text` encodes to without added special
    /// tokens, used for the reasoning token count. The default implementation
    /// returns `None`, so `reasoning_tokens` stays `None`.
    fn count_tokens(&self, _text: &str) -> Option<usize> {
        None
    }

    /// Return whether `id` is a token of the added vocabulary, such as a
    /// special token. The default implementation returns `false`, so output
    /// parsing matches every marker by spelling.
    fn is_special_token(&self, _id: u32) -> bool {
        false
    }
}

impl StreamTokenizer for Tokenizer {
    fn decode_ids(&self, ids: &[u32], skip_special_tokens: bool) -> Result<String, String> {
        self.decode(ids, skip_special_tokens)
            .map_err(|error| error.to_string())
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        // Call the inherent method of the `Deref` target: a plain
        // `self.id_to_token(id)` resolves to this trait method and recurses.
        (**self).id_to_token(id)
    }

    fn count_tokens(&self, text: &str) -> Option<usize> {
        self.encode_fast(text, false)
            .ok()
            .map(|encoding| encoding.len())
    }

    fn is_special_token(&self, id: u32) -> bool {
        self.get_added_vocabulary()
            .get_added_tokens_decoder()
            .contains_key(&id)
    }
}

impl<T> StreamTokenizer for Arc<T>
where
    T: StreamTokenizer + Sync,
{
    fn decode_ids(&self, ids: &[u32], skip_special_tokens: bool) -> Result<String, String> {
        (**self).decode_ids(ids, skip_special_tokens)
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        (**self).id_to_token(id)
    }

    fn count_tokens(&self, text: &str) -> Option<usize> {
        (**self).count_tokens(text)
    }

    fn is_special_token(&self, id: u32) -> bool {
        (**self).is_special_token(id)
    }
}

/// Text decoded from buffered token IDs.
pub(super) struct DecodedText {
    pub content: String,
    /// Number of token IDs the text accounts for.
    pub token_count: usize,
    /// Log probabilities of those token IDs, when every pushed token ID had one.
    pub logprobs: Option<Vec<TokenLogprob>>,
    /// Whether the text is one special token, decoded as its own piece.
    pub is_special_token: bool,
}

/// Buffer token IDs until they decode into text.
pub(super) struct StreamDecoder {
    tokenizer: Box<dyn StreamTokenizer>,
    ids: Vec<u32>,
    /// Log probabilities of the token IDs buffered since the last decoded
    /// text; `None` when one of those token IDs had no log probability.
    logprobs: Option<Vec<InferenceTokenLogprob>>,
}

impl StreamDecoder {
    pub(super) fn new(tokenizer: Box<dyn StreamTokenizer>) -> Self {
        Self {
            tokenizer,
            ids: Vec::new(),
            logprobs: Some(Vec::new()),
        }
    }

    /// Return the tokenizer, for counting reasoning tokens.
    pub(super) fn tokenizer(&self) -> &dyn StreamTokenizer {
        self.tokenizer.as_ref()
    }

    /// Buffer the token IDs of one backend output and decode them into pieces.
    ///
    /// Plain tokens are decoded together, and their text is returned once it
    /// is complete: while a multi-token character is incomplete, the token IDs
    /// stay buffered and no piece is returned for them. Each special token,
    /// as [`StreamTokenizer::is_special_token`] reports it, is decoded as its
    /// own piece: the plain tokens before it are decoded first, even when
    /// their text ends in an incomplete character, and the piece of the
    /// special token is marked. `logprobs`, when present, holds one entry per
    /// token ID.
    ///
    /// # Errors
    ///
    /// Returns [`StreamError::LogprobCountMismatch`] when the log probability
    /// count differs from the token ID count, before buffering anything, and
    /// [`StreamError::Decode`] when the tokenizer fails.
    pub(super) fn decode_incrementally(
        &mut self,
        token_ids: &[u32],
        logprobs: Option<Vec<InferenceTokenLogprob>>,
    ) -> Result<Vec<DecodedText>, StreamError> {
        if let Some(logprobs) = &logprobs
            && logprobs.len() != token_ids.len()
        {
            return Err(StreamError::LogprobCountMismatch {
                detail: format!(
                    "{} log probabilities for {} token IDs",
                    logprobs.len(),
                    token_ids.len()
                ),
            });
        }
        let mut logprobs = logprobs.map(Vec::into_iter);
        let mut pieces = Vec::new();
        for &id in token_ids {
            let logprob = logprobs.as_mut().and_then(Iterator::next);
            if self.tokenizer.is_special_token(id) {
                self.flush(true, false, &mut pieces)?;
                self.push_id(id, logprob);
                self.flush(true, true, &mut pieces)?;
            } else {
                self.push_id(id, logprob);
            }
        }
        self.flush(false, false, &mut pieces)?;
        Ok(pieces)
    }

    /// Buffer one token ID. A token ID without a log probability drops the
    /// log probabilities buffered for the current piece.
    fn push_id(&mut self, id: u32, logprob: Option<InferenceTokenLogprob>) {
        self.ids.push(id);
        match (&mut self.logprobs, logprob) {
            (Some(buffered), Some(logprob)) => buffered.push(logprob),
            _ => self.logprobs = None,
        }
    }

    /// Decode the buffered token IDs into one piece. Without `force`, the
    /// token IDs stay buffered while their text is empty or ends in an
    /// incomplete character; with `force`, only empty text stays buffered.
    fn flush(
        &mut self,
        force: bool,
        is_special_token: bool,
        pieces: &mut Vec<DecodedText>,
    ) -> Result<(), StreamError> {
        if self.ids.is_empty() {
            return Ok(());
        }
        let content = self
            .tokenizer
            .decode_ids(&self.ids, false)
            .map_err(|detail| StreamError::Decode { detail })?;
        if content.is_empty() || (!force && content.ends_with(REPLACEMENT_CHARACTER)) {
            return Ok(());
        }
        let ids = std::mem::take(&mut self.ids);
        let logprobs = self
            .logprobs
            .replace(Vec::new())
            .filter(|logprobs| !logprobs.is_empty())
            .map(|logprobs| create_logprobs(self.tokenizer.as_ref(), &ids, &logprobs));
        pieces.push(DecodedText {
            content,
            token_count: ids.len(),
            logprobs,
            is_special_token,
        });
        Ok(())
    }
}
