use super::logprobs::InferenceTokenLogprob;

/// Prompt-side token counts supplied by the inference backend. Unspecified
/// counts default to zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PromptUsage {
    /// Total input tokens, including tokens read from the prompt cache.
    pub prompt_tokens: usize,
    /// Input tokens read from the prompt cache.
    pub prompt_cache_hit_tokens: usize,
}

/// Completion-side token counts accumulated by the stream processor from
/// `Text.token_count` and token IDs that decode into text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompletionUsage {
    /// Tokens accounted for by processed chunks, including discarded markup
    /// and the entire chunk containing a stop sequence that the processor
    /// matches. Undecoded trailing token IDs and chunks after the stop are
    /// excluded.
    pub completion_tokens: usize,
    /// Number of tokens the emitted reasoning content encodes to. Present in
    /// thinking mode when the processor has a tokenizer that counts tokens.
    pub reasoning_tokens: Option<usize>,
}

/// Text and metadata received from an inference backend.
#[derive(Debug)]
pub enum InferenceChunk {
    /// Initial metadata reported before the model produces output.
    Ready {
        system_fingerprint: Option<String>,
        prompt_usage: PromptUsage,
    },
    /// Text produced by the model, and the number of tokens that text
    /// accounts for.
    Text { content: String, token_count: usize },
    /// The generated token IDs of one backend output, decoded together by the
    /// processor's tokenizer.
    ///
    /// `logprobs`, when present, holds one entry per token ID. Supply it when
    /// [`InferenceOptions::logprobs`](crate::request::InferenceOptions::logprobs)
    /// is set; the decoded text then carries the log probabilities of its
    /// tokens. Text decoded from token IDs pushed without log probabilities
    /// carries none.
    Tokens {
        token_ids: Vec<u32>,
        logprobs: Option<Vec<InferenceTokenLogprob>>,
    },
    /// Terminal metadata reported after the model stops.
    Finish {
        finish_reason: InferenceFinishReason,
    },
}

/// The finish reason reported by the inference backend.
///
/// When the backend's final output carries token IDs, the caller must forward
/// them in a `Tokens` chunk only for `Length`, and must drop them
/// for the other reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceFinishReason {
    /// The model stopped. The caller also reports a prompt that exceeds the
    /// context length and a structured output runtime error as `Stop`.
    Stop,
    /// The backend reached its output token limit.
    Length,
    /// The backend stopped output for content filtering.
    ContentFilter,
}

/// The finish reason that protocol chunk generators report, which
/// [`StreamProcessor`](super::StreamProcessor) derives from the backend's
/// finish reason, parsed tool calls, and matched stop sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    /// The backend stopped without a parsed tool call.
    Stop,
    /// The backend reached its output token limit.
    Length,
    /// The backend stopped after the output contained a tool call.
    ToolCalls,
    /// The processor matched a stop sequence.
    StopSequence,
    /// The backend stopped output for content filtering.
    ContentFilter,
    /// The inference stream ended without a finish chunk.
    EndOfStream,
}
