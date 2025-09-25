# Master Task – QA & Dogfooding

This document outlines manual test scenarios for the Master Task flow, with an emphasis on read‑only tasks. It assumes:
- Dev desktop: `cd src-tauri && cargo tauri dev`
- Web UI served by Trunk via beforeDevCommand
- Approvals: `never`; Sandbox: read‑only or full, per test

## Quick Smoke – Read‑Only Flow

Goal: Validate plan → run (budgeted) → advance → resume on restart, with no writes.

1) Create a master task
   - In UI, click “New Master Task” (left sidebar). Rename to “Readonly – Repo Survey”.
   - Budget: defaults are OK; keep `max_turns` small (e.g., 1–2).

2) Plan subtasks
   - Click “Plan” and set goal: `List top-level files; Summarize Rust crates (no writes)`.
   - Expect: queue shows 2–4 pending items.

3) Set read‑only constraint
   - Ensure the UI shows Autonomy `sandbox` as read‑only (or assume read‑only constraints in prompt). The runner adds a “read‑only, do not modify files” constraint to its control prompt when the sandbox is set to read‑only.

4) Run
   - Click “Run”. Observe `TaskUpdate` messages. With `max_turns=1`, the runner sends a single control prompt and advances the current subtask.
   - Expect: no file writes; the streaming view shows analysis and listings only.

5) Pause / Resume
   - Click “Pause” while running; status should become Paused. Click “Run” again; it should continue the same subtask first, then advance.

6) Budget hit
   - Set very small budgets (e.g., `max_turns=1` or low `max_tokens`); run. Expect a `TaskUpdate` pause with reason “Budget hit”.

7) Error retry/backoff
   - Temporarily simulate a transient failure (e.g., unplug network). Expect retries with backoff (200/400/800ms). After max retries, task pauses and subtask `last_error` is populated.

8) Resume after restart
   - Stop the app. Restart `cargo tauri dev`. The app scans tasks and resumes any task marked Running or with a Running subtask. Expect it to pick up at the correct subtask.

## Overnight Dogfooding (Optional)

Run a longer read‑only task (e.g., repo survey + documentation indexing) with:
- `max_turns=10–20`, `max_minutes≈60`, sandbox read‑only. Leave running overnight and capture screenshots of the queue advancing and metrics increasing. Confirm no writes occurred.

## Expected Metrics & Signals

- Metrics accumulate: `turns`, `tokens_in/out`, `wall_clock_minutes`, `retries` (task and subtask-level).
- `TaskUpdate` stream includes: starting, paused (budget hit), error (retries exceeded), advanced, completed.

## Sample Config

See `docs/samples/master-task.json` for a read‑only seed idea (not a drop‑in file). Use it to craft your goal text or to manually seed a queue.

