# Core: Error Handling

File: `codex-rs/core/src/error.rs`

Defines the `CodexErr` and `SandboxErr` enums, common conversions, and helpers
for user‑facing error messages.

## Result alias

- `pub type Result<T> = std::result::Result<T, CodexErr>` used throughout core.

## SandboxErr

- `Denied { output }` — sandbox likely prevented an action.
- `Timeout { output }` — process exceeded timeout.
- `Signal(i32)` — process terminated by a signal (Unix).
- Linux‑specific: seccomp setup/backend errors; landlock restrictions.

## CodexErr

- Stream lifecycle (`Stream`, `RetryLimit`), HTTP errors (`UnexpectedStatus`),
  session state (`SessionConfiguredNotFirstEvent`), sandbox wrapper (`Sandbox`).
- Usage limits: `UsageLimitReached`, `UsageNotIncluded` with helpful blurbs.
- Auto‑convertors for `io`, `reqwest`, `serde_json`, Tokio `JoinError`.

## UI helpers

- `get_error_message_ui` maps certain error variants to nicer short messages in
  the UI (e.g., timeouts). Others default to `Display` strings.

