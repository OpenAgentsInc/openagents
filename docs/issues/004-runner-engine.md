Title: Master Task – Runner Engine (execute queue unattended)
Labels: master-task, engine

Summary
Implement a background loop that pulls the next subtask, ensures a Codex session, sets autonomy settings, sends control prompt + task inputs, and advances until stop condition or pause.

Acceptance Criteria
- `task_run(id)` starts/continues execution; `task_pause(id)` pauses; `task_cancel(id)` stops and marks canceled.
- Engine executes at least one subtask end‑to‑end unattended, writing metrics and status.
- Emits lightweight UI events (e.g., `TaskUpdate { id, status, current_subtask }`).

Tech Notes
- File: `src-tauri/src/master.rs` – new module; dispatches to `new_chat_session` and `submit_chat` (src-tauri/src/lib.rs:940, 960).
- Use `session_configured` to capture `session_id` and attach to subtask (src-tauri/src/lib.rs:1315).
- Budgets: track turns/tokens using `token_count` events, and wall‑clock with Instant.

Dependencies
- 001, 002, 003

