Title: Master Task Orchestration – Overview and Roadmap
Labels: master-task, roadmap

Summary
Build a long‑running “Master Task” wrapper around Codex that plans a job into subtasks, runs them unattended across many turns/sessions, and exposes full state/controls in the UI (pause/resume, budgets, stop conditions).

Why
Large jobs (e.g., “Convert a Figma doc into screens”) take hundreds of API calls and many intermediate steps. Today Codex stops often for feedback, tethering the user. A wrapper can keep making progress while remaining transparent and steerable.

Scope
- Planning (off‑record) into a queue of subtasks
- Runner that executes subtasks in Codex sessions
- Budgets/stop conditions, retry/backoff
- UI: sidebar + detail (status, transcript, controls)
- Durable persistence to resume after restarts

Acceptance Criteria
- Master tasks appear in sidebar with status chip; can create/delete/select.
- Selecting a master task shows queue, current subtask transcript, controls (pause/resume, cancel, autonomy).
- Engine advances through at least 3 subtasks unattended; budgets and stop conditions enforce limits.
- State survives app restart.

Tech Notes
- Read docs/master-task-orchestration.md
- Hooks already present to help: off‑record channel (src-tauri/src/lib.rs:1182), session_configured handling (src-tauri/src/lib.rs:1315), submit_chat/new_chat_session.

Dependencies
- Ship issues 001 → 006 first; then error/resume polish.

