Title: Master Task – Stream Integration (status events)
Labels: master-task, events

Summary
Add small UI events so the app can reflect master task progress without requerying the file constantly.

Acceptance Criteria
- New `UiStreamEvent::TaskUpdate { task_id, status, subtask_id?, message? }` emitted when runner starts/stops/advances.
- UI handles this event by updating the checklist and current subtask label.

Tech Notes
- File: `src-tauri/src/lib.rs` – extend UiStreamEvent enum (like existing SessionConfigured at src-tauri/src/lib.rs:1300+), emit from master.rs.
- `src/app.rs`: listen and update signals for the master task panel.

Dependencies
- 004

