Title: Master Task – UI Sidebar + Detail Panel Skeleton
Labels: master-task, ui

Summary
Add a new sidebar section for Master Tasks and a right‑hand detail view that shows title, controls (pause/resume/cancel/autonomy), and a placeholder for the subtask checklist and transcript.

Acceptance Criteria
- Sidebar lists master tasks with status chip; clicking selects.
- Detail panel shows task name and controls; content area has 2 columns: checklist (left), transcript (right placeholder).
- No engine logic yet; reads data via `tasks_list()` / `task_get()`.

Tech Notes
- File: `src/app.rs` – add a new section under OpenAgents; reuse styling patterns (buttons with underline, zinc borders).
- Wire commands via `tauri_invoke` similarly to existing status and chat calls.
- Maintain a `selected_task_id` signal; default none.

Dependencies
- Issue 001 (commands) provides data.

