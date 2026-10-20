//! Model-specific conversation rendering and token encoding.
//!
//! The V4 and V4.1 prompt encodings, [`DeepseekV4Encoding`] and
//! [`DeepseekV41Encoding`], render conversations into prompts. A prompt
//! encoding with an attached tokenizer also encodes conversations into token
//! IDs.
//!
//! A rendered prompt consists of explicit special tokens and plain text.
//! Rendering inserts the template's control tokens, the placeholders of a
//! message's image sources, and each grounding-token spelling found in
//! assistant content or reasoning content as explicit special tokens. All
//! other text is plain text, including template text, message content,
//! reasoning content outside grounding tokens, tool definitions, tool call
//! names and arguments, and the response format schema. Encoding encodes a
//! special-token spelling in plain text as the token IDs of its characters.
//!
//! [`DeepseekV4Encoding`]: dsv4::DeepseekV4Encoding
//! [`DeepseekV41Encoding`]: dsv41::DeepseekV41Encoding

use std::ops::Range;

use deepseek_recipe_core::conversation::Conversation;
use deepseek_recipe_core::multimodal::ImageSource;

pub use tokenizer::{PromptTokenizer, TokenizerEncoder};
pub use v4::{dsv4, dsv41};

pub mod v4;

mod tokenizer;

/// A rendered prompt and the images its placeholders refer to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPrompt {
    /// Model input string.
    pub prompt: String,
    /// Ascending, disjoint byte ranges of `prompt` that hold explicit special
    /// tokens. Pass them to [`TokenizerEncoder::encode_ids`] when encoding
    /// `prompt`.
    pub special_ranges: Vec<Range<usize>>,
    /// Image sources in the order their placeholders appear in the prompt.
    pub image_sources: Vec<ImageSource>,
}

/// Failure to encode a conversation.
#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    /// A failure of the attached tokenizer to encode the rendered prompt.
    #[error("failed to encode conversation: {0}")]
    Encode(String),
    /// The prompt encoding has no attached tokenizer.
    #[error("no tokenizer is attached to the encoding")]
    MissingTokenizer,
}

/// Render model prompts and encode conversations into token IDs.
pub trait PromptEncoding {
    /// Encode a conversation into model token IDs using the attached tokenizer.
    ///
    /// The provided prompt encodings render the conversation with
    /// [`render_conversation`](Self::render_conversation) and pass the prompt
    /// and its special ranges to [`TokenizerEncoder::encode_ids`].
    ///
    /// The caller must attach a tokenizer before this call, such as with the
    /// `with_tokenizer` method of [`DeepseekV4Encoding`] or
    /// [`DeepseekV41Encoding`].
    ///
    /// # Errors
    ///
    /// - [`EncodingError::MissingTokenizer`] when no tokenizer is attached.
    /// - [`EncodingError::Encode`] when the attached tokenizer returns an
    ///   error, such as when [`PromptTokenizer`] fails to encode plain text or
    ///   finds an explicit special token that is not in its added vocabulary.
    ///
    /// [`DeepseekV4Encoding`]: dsv4::DeepseekV4Encoding
    /// [`DeepseekV41Encoding`]: dsv41::DeepseekV41Encoding
    fn encode(&self, conversation: &Conversation) -> Result<Vec<u32>, EncodingError>;

    /// Render the conversation followed by the start of the next assistant
    /// turn, unless the final message is a prefix continuation.
    ///
    /// The start of the next assistant turn is the assistant token followed by
    /// the thinking marker: the thinking start token, `<think>` by default, in
    /// thinking mode, and the thinking end token, `</think>` by default,
    /// otherwise. When the tool choice is [`ToolChoice::Required`] and at least
    /// one tool is defined, the prompt ends with the start of a DSML tool-call
    /// block. The result also records the byte ranges of the explicit special
    /// tokens and the image sources in placeholder order.
    ///
    /// [`ToolChoice::Required`]: deepseek_recipe_core::tools::ToolChoice::Required
    fn render_conversation(&self, conversation: &Conversation) -> RenderedPrompt;
}
