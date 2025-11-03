# Tinyvex “ephemeral_*” threads

Ephemeral threads are internal, short‑lived documents created by the bridge/watchers to mirror provider activity while a canonical session/thread mapping is not yet available. They are not user‑facing chats and should be hidden in normal UIs.

## Why they exist

- During startup or for ad‑hoc runs, the watcher may process provider output (Codex or Claude Code) before it can associate that activity with a user thread “alias”.
- To avoid dropping data, the bridge mirrors events into a temporary Tinyvex thread whose `id` starts with `ephemeral_…`.
- Once an alias mapping is known (resume/session id ↔ client thread id), subsequent data is written under the canonical id. In logs you may see:
  - `NO ALIAS MAPPING FOUND — mirrored to session only …` (temporary condition)

## Naming conventions

- Claude Code live runs:
  - `ephemeral_claude_<id>` — transient holder for a live run
  - `ephemeral_claude_out_<id>` — stdout stream mirror
  - `ephemeral_claude_err_<id>` — stderr stream mirror
- Other providers may add similar `ephemeral_*` documents for bootstrap phases or tool/process output.

## Consumer guidance (apps, clients)

- Hide ephemeral threads by default:
  - Filter out any thread whose `id` starts with `"ephemeral_"` when listing recent chats or auto‑selecting a thread.
  - Do not subscribe/query messages for ephemeral ids in end‑user views.
- Alias handling:
  - Real, user‑facing rows include stable ids (e.g., UUID‑like) and often a `resume_id` that the bridge uses to link provider sessions.
  - If your client receives events under a canonical `thread_id` that differs from the selected UI thread, treat a match on `resume_id` as equivalent.
- Debugging mode (optional):
  - If needed, provide a developer toggle to show ephemeral threads and compact raw events; keep this off by default.

## Where they come from (bridge internals)

- Codex watcher: `crates/oa-bridge/src/watchers/sessions_watch.rs`
- Claude watcher: `crates/oa-bridge/src/watchers/claude_watch.rs`
- Writer/adapter: `crates/oa-bridge/src/tinyvex_write.rs`

These components emit `tinyvex.update` and populate Tinyvex rows even before the alias map is established. When the alias is known, future rows are written under the canonical id.

## UI policy

- Expo app and Tauri desktop both filter ephemeral threads from the history list and avoid auto‑picking them.
- Live streaming chunks (thoughts/partial messages) remain ephemeral UI state — only finalized assistant/user messages are persisted to Tinyvex.

## FAQ

- “Why do I see no messages after clicking an ephemeral row?”
  - Ephemeral docs often contain process output or nothing relevant to chat bubbles. Hide them, and select a canonical thread instead.
- “Can I safely delete ephemeral docs?”
  - They are transient and safe to ignore. Deletion is optional; filtering is sufficient for UX.

