Title: Master Task – Error Handling, Retry, and Metrics
Labels: master-task, engine

Summary
Handle transient tool errors and no‑op turns gracefully with retry/backoff. Track metrics per task/subtask for visibility.

Acceptance Criteria
- Automatic retry (up to N) on transient errors (timeouts, rate limits); exponential backoff with jitter.
- Metrics: total turns, tokens in/out, retries, wall clock per subtask and per master task.
- Errors surface to UI with a helpful last_error field; runner pauses on repeated failure.

Tech Notes
- Store metrics in `.task.json`; surface in UI detail.
- Consider a simple enum for failure classification.

Dependencies
- 004, 006

