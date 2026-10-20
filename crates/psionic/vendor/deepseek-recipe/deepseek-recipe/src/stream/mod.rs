//! Parse model output and turn it into protocol response chunks.

use std::sync::Arc;

pub use decoder::StreamTokenizer;
pub use inference::{
    CompletionUsage, FinishReason, InferenceChunk, InferenceFinishReason, PromptUsage,
};
pub use logprobs::{InferenceTokenLogprob, TokenLogprob, TopLogprob};
pub use output_parser::{ParsingOptions, ThinkingInitialStage};
pub use processor::StreamProcessor;

mod decoder;
mod inference;
mod logprobs;
mod output_parser;
mod processor;

/// An error that ends stream processing before normal completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// A `Token` or `Tokens` chunk arrived without an attached tokenizer.
    MissingTokenizer,
    /// The tokenizer failed to decode buffered token IDs.
    Decode {
        /// The tokenizer's failure message.
        detail: String,
    },
    /// A `Tokens` chunk carried a log probability count that differs from its
    /// token ID count.
    LogprobCountMismatch {
        /// Both counts.
        detail: String,
    },
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingTokenizer => {
                formatter.write_str("token chunk received without a tokenizer")
            }
            Self::Decode { detail } => write!(formatter, "failed to decode token IDs: {detail}"),
            Self::LogprobCountMismatch { detail } => {
                write!(formatter, "invalid log probabilities: {detail}")
            }
        }
    }
}

impl std::error::Error for StreamError {}

/// A parsed piece of model output, before conversion into protocol response
/// chunks.
///
/// Unless the stream ends with a [`StreamError`], [`StreamProcessor`] produces
/// `Start` first and `Finish` last, each exactly once. A tool call produces
/// `ToolCallBegin`, then `ToolCallNamed`, then zero or more
/// `ToolCallArgumentsDelta` chunks; output truncated inside a tool call ends
/// that sequence early.
///
/// `logprobs` holds the log probabilities of the tokens of the decoded text the
/// content comes from. Content split from one decoded text carries the log
/// probabilities of the whole text.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputChunk {
    /// Output begins.
    Start {
        system_fingerprint: Option<String>,
        prompt_usage: PromptUsage,
    },
    /// Answer content.
    Content {
        content: String,
        logprobs: Option<Arc<[TokenLogprob]>>,
    },
    /// Reasoning content.
    Reasoning {
        content: String,
        logprobs: Option<Arc<[TokenLogprob]>>,
    },
    /// An `invoke` tag opened a tool call; its tool name is not complete yet.
    ToolCallBegin,
    /// The tool name of the open tool call is complete. The arguments follow
    /// in [`ToolCallArgumentsDelta`](Self::ToolCallArgumentsDelta) chunks.
    ToolCallNamed { tool_name: String },
    /// A piece of the JSON arguments of the open tool call.
    ToolCallArgumentsDelta { arguments: String },
    /// Output ends.
    Finish {
        finish_reason: FinishReason,
        /// The matched stop sequence, when one ended output.
        stop_sequence: Option<String>,
        completion_usage: CompletionUsage,
    },
}

/// Convert parsed output chunks into a protocol's response chunks.
///
/// An implementation can rely on the chunk order that [`OutputChunk`]
/// describes.
pub trait ChunkGenerator: Send + 'static {
    /// The protocol's response chunk, such as an SSE event.
    type Chunk;

    /// Produce zero or more protocol chunks for one parsed output chunk.
    fn generate(&mut self, chunk: OutputChunk) -> impl Future<Output = Vec<Self::Chunk>> + Send;
}
