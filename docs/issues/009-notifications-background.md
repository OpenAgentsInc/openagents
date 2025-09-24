Title: Master Task – Background Runs & Notifications
Labels: master-task, polish

Summary
Allow the runner to continue in the background and notify the user upon subtask completion or errors.

Acceptance Criteria
- OS notification (or in‑app banner) when a subtask finishes or needs input.
- Optional auto‑minimize / minimized runner mode.

Tech Notes
- Use tauri‑plugin‑opener or a notification plugin; fallback to a top‑right toast panel in UI.
- Keep runner throttled when window is not focused if needed.

Dependencies
- 004

