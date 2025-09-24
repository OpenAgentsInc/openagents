# Core: Prompt and ResponseEvent

This document describes the common request/response types used across both
Responses API and Chat Completions.

File: `codex-rs/core/src/client_common.rs`

## Prompt

```rust
pub struct Prompt {
    pub input: Vec<ResponseItem>,
    pub(crate) tools: Vec<OpenAiTool>,
    pub base_instructions_override: Option<String>,
}
```

- `get_full_instructions(&ModelFamily)` — resolves base instructions and
  conditionally appends `APPLY_PATCH_TOOL_INSTRUCTIONS` when the model needs
  them and no explicit apply_patch tool is present.
- `get_formatted_input()` — returns a clone of `input` (ready to serialize).

## Reasoning and text controls

- `create_reasoning_param_for_request` — returns `Some(Reasoning)` only for
  model families that support reasoning summaries.
- `create_text_param_for_request` — converts `Verbosity` to the OpenAI
  Responses API `text.verbosity` payload for GPT‑5 family models.

## ResponseEvent

```rust
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    Completed { response_id: String, token_usage: Option<TokenUsage> },
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
    ReasoningContentDelta(String),
    ReasoningSummaryPartAdded,
    WebSearchCallBegin { call_id: String },
    RateLimits(RateLimitSnapshotEvent),
}
```

These are produced by `ModelClient` and consumed by `codex.rs` to update the UI,
run tools, and persist rollouts.

## ResponseStream

A `futures::Stream` that yields `Result<ResponseEvent>`, backed by a Tokio MPSC
channel; used to decouple networking from orchestration.

