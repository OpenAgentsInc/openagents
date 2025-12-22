# Deeper Findings (Pass 2)

## Medium
- D2-M-1 Relay `max_message_size` is defined but never enforced for WebSocket frames, leaving the server open to oversized payloads and memory/CPU exhaustion. Evidence: `crates/nostr/relay/src/server.rs:33`, `crates/nostr/relay/src/server.rs:47`.
- D2-M-2 Relay database access uses synchronous rusqlite connections inside async tasks; `store_event` and `query_events` run on the Tokio executor thread and can block other work under load. Evidence: `crates/nostr/relay/src/server.rs:363`, `crates/nostr/relay/src/server.rs:446`, `crates/nostr/relay/src/db.rs:168`.
- D2-M-3 Codex SDK output schema files are created as temp files but dropped when `run_streamed` returns, so the CLI can fail to open `--output-schema` due to a race. Evidence: `crates/codex-agent-sdk/src/thread.rs:227`, `crates/codex-agent-sdk/src/thread.rs:258`.
- D2-M-4 Codex `ProcessTransport` lacks a `Drop` impl, so spawned CLI processes can outlive the caller if `kill`/`wait` aren’t called. Evidence: `crates/codex-agent-sdk/src/transport/process.rs:46`, `crates/codex-agent-sdk/src/transport/process.rs:117`.
- D2-M-5 GUI autopilot uses `cargo run` and only kills the cargo process on stop; the actual autopilot child can survive, leaving runaway runs. Evidence: `src/gui/routes/autopilot.rs:83`, `src/gui/routes/autopilot.rs:207`.

## Low
- D2-L-1 `merge_with_defaults` returns merged configs without running `validate_config`, so invalid overrides can slip through when using CLI partials. Evidence: `crates/config/src/loader.rs:185`, `crates/config/src/loader.rs:207`.
- D2-L-2 Claude usage-limit fetches use a default `reqwest::Client` without timeouts, so the status route can hang indefinitely on network stalls. Evidence: `src/gui/state.rs:350`, `src/gui/state.rs:356`.
- D2-L-3 Directive progress queries use `unwrap_or(0)` on DB errors, silently masking failures and reporting 0% progress. Evidence: `crates/issues/src/directive.rs:309`, `crates/issues/src/directive.rs:317`.
