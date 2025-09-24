Title: Master Task – Persistence & Resume
Labels: master-task, backend

Summary
Persist engine state and resume execution after app restart or system crash.

Acceptance Criteria
- On start, engine discovers in‑progress tasks and resumes safely at the correct subtask.
- All timers/budgets recompute from persisted metrics; runner won’t duplicate completed subtasks.

Tech Notes
- Keep `.task.json` authoritative; avoid storing engine state in memory only.
- Tie rollouts to subtasks via `session_id`/`rollout_path` captured from `session_configured`.

Dependencies
- 001, 004

