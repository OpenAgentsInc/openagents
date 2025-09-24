Title: Master Task – Persistence + Tauri Commands
Labels: master-task, backend

Summary
Introduce `.task.json` files and Tauri commands to create/load/save master tasks and mutate their queue/status.

Acceptance Criteria
- Tauri commands:
  - `tasks_list() -> Vec<TaskMeta>`
  - `task_create(name, settings) -> Task` (writes `~/.codex/master-tasks/<id>.task.json`)
  - `task_get(id) -> Task`
  - `task_update(Task)` (full replace on save)
  - `task_delete(id)`
- File schema documented; atomic writes to avoid corruption.
- Unit tests for load/save success and corrupted file fallback.

Tech Notes
- New module `src-tauri/src/tasks.rs` for models + IO helpers.
- Use `dirs` crate to resolve `CODEX_HOME` (mirror logic in src-tauri/src/lib.rs:15).
- Keep structs small and serde‑friendly; include `version` field for migrations.

Dependencies
- None; unblocks UI and engine.

