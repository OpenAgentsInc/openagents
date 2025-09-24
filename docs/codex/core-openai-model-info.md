# Core: OpenAI Model Info

Location: `codex-rs/core/src/openai_model_info.rs`

Provides token window and output limits for known slugs, and the auto‑compact
threshold used by history compaction.

## ModelInfo

```rust
struct ModelInfo {
    context_window: u64,
    max_output_tokens: u64,
    auto_compact_token_limit: Option<i64>,
}
```

## get_model_info(ModelFamily)

- Returns hardcoded values for popular models (o3, o4‑mini, gpt‑4.1, gpt‑4o,
  gpt‑3.5, GPT‑OSS) and a general rule for `gpt-5*`/`codex-*`.
- These values are used to:
  - Annotate UI with context limits.
  - Trigger automatic compaction when token usage crosses
    `auto_compact_token_limit`.
  - Fill `Config.model_context_window` and `model_max_output_tokens` when not
    overridden in config.

