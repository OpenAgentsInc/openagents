Title: Master Task – Stop Conditions & Budgets
Labels: master-task, engine

Summary
Implement concrete stop conditions and budgets enforced by the runner: acceptance checks, token/turn/time budgets.

Acceptance Criteria
- Configurable per task: max_turns, max_tokens, max_minutes, and acceptance checks.
- Runner stops a subtask when acceptance passes or budget is exceeded and records the outcome.

Tech Notes
- Acceptance checks can be simple shells: build/test outcomes parsed from stdout; or off‑record ask: “Does the artifact pass X? answer true/false”.
- Track tokens via `token_count` events; wall clock timers around turns.

Dependencies
- 004

