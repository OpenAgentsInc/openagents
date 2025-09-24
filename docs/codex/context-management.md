# Context Management and Truncation

How Codex keeps prompts within model limits and stays readable while streaming.

## Token Usage and Remaining Context

- Token accounting: `TokenUsage` (protocol) carries input, cached input, output, and reasoning tokens.
  - File: `codex-rs/protocol/src/protocol.rs`
  - Helper: `TokenUsage::tokens_in_context_window()` treats prior‑turn reasoning output as dropped from the live window.
  - Remaining percent: `percent_of_context_window_remaining(context_window)` subtracts a baseline to reflect the user‑controllable portion.
    - Baseline constant (`BASELINE_TOKENS`): `codex-rs/protocol/src/protocol.rs:609`
    - Computation: `codex-rs/protocol/src/protocol.rs:640`–`codex-rs/protocol/src/protocol.rs:668`

## Auto‑Compaction (Summarization)

- Trigger: when token usage for the turn reaches `get_auto_compact_token_limit()` (from config or model info), Codex runs an inline compaction task before continuing the turn.
  - Decision and trigger: `codex-rs/core/src/codex.rs:1720`–`codex-rs/core/src/codex.rs:1860`, `codex-rs/core/src/codex.rs:1816`–`codex-rs/core/src/codex.rs:1900`
  - Limit source: `ModelClient::get_auto_compact_token_limit()`: `codex-rs/core/src/client.rs:96`–`codex-rs/core/src/client.rs:104`
  - If still above the limit after compaction, Codex emits a user‑facing error suggesting a new session or trimming input: `codex-rs/core/src/codex.rs:1849`–`codex-rs/core/src/codex.rs:1876`

- Compaction workflow: `codex-rs/core/src/codex/compact.rs`
  - Instructions: `SUMMARIZATION_PROMPT` = `core/templates/compact/prompt.md`
  - Runs a one‑off prompt using the current history, waits for completion, then replaces history with a bridged form that includes:
    - Initial session context (user instructions and environment context)
    - A compact “bridge” message containing the prior user messages and the generated summary
      - Template renderer: `history_bridge.md` via Askama; `HistoryBridgeTemplate` at `codex-rs/core/src/codex/compact.rs:15`–`codex-rs/core/src/codex/compact.rs:23`
  - Prior user messages collection filters out session prefix entries (`<user_instructions>`, `<environment_context>`):
    - Collect/filter: `collect_user_messages()` and `is_session_prefix_message()` in `codex-rs/core/src/codex/compact.rs`

- Size control inside compaction:
  - Aggregated prior‑user text is truncated with a middle‑elision to stay well under the window: `COMPACT_USER_MESSAGE_MAX_TOKENS * 4` bytes.
    - Constant and usage: `codex-rs/core/src/codex/compact.rs:13`, `codex-rs/core/src/codex/compact.rs:118`–`codex-rs/core/src/codex/compact.rs:128`
  - Truncation utility: `truncate_middle(s, max_bytes)` prefers newline boundaries and preserves UTF‑8 integrity.
    - File: `codex-rs/core/src/truncate.rs`

## Exec Output Summarization (Head+Tail)

- Large command outputs are summarized for the model to keep tool‑call payloads compact. Clients still receive full streaming output; only the formatted summary that goes back to the model is capped.
  - Formatter: `format_exec_output_str()` in `codex-rs/core/src/codex.rs:3080`–`codex-rs/core/src/codex.rs:3180`
  - Limits:
    - `MODEL_FORMAT_MAX_BYTES = 10 KiB`, `MODEL_FORMAT_MAX_LINES = 256`
    - Split into head/tail with an elision marker: `MODEL_FORMAT_HEAD/Tail_*` constants
    - Byte‑accurate clipping at UTF‑8 boundaries (`take_bytes_at_char_boundary`, `take_last_bytes_at_char_boundary`)
  - Tests validate line and byte caps: `codex-rs/core/src/codex.rs:3461`–`codex-rs/core/src/codex.rs:3513`

## Project Docs Size Limits

- Project documentation (`AGENTS.md`) embedded into user instructions is capped by `project_doc_max_bytes` to protect the context window.
  - Discovery/merge and cap: `codex-rs/core/src/project_doc.rs`
  - Config key: `project_doc_max_bytes` in `~/.codex/config.toml` (see `codex-rs/core/src/config.rs`)

## Prompt Cache

- The Responses API requests include `prompt_cache_key: conversation_id` so servers can cache the stable prefix (system instructions, tool schemas, session prefix messages), reducing repeated input usage across turns.
  - Field set at: `codex-rs/core/src/client.rs:219`
  - Tests cover cache/prefix stability: `codex-rs/core/tests/suite/prompt_caching.rs`

## UI Feedback

- Codex reports percentage of context remaining (normalized by baseline) and emits rate‑limit snapshots. This helps users gauge when compaction will occur or when to start a fresh session.
  - Percentage calculation: `TokenUsage::percent_of_context_window_remaining()` in `codex-rs/protocol/src/protocol.rs`

