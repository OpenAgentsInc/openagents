# Reprojection and Reconciliation

`OpenAgentsRuntime.Runs.Reprojection` provides operator-safe rebuild and drift
repair for Laravel-facing read models (`public.runs`, `public.messages`,
`public.run_events`) sourced from runtime event log state.

## Command

- Full reproject (scoped):
  - `mix runtime.reproject --run-id <run_id>`
  - `mix runtime.reproject --thread-id <thread_id>`
- Dry run (no mutation):
  - `mix runtime.reproject --run-id <run_id> --dry-run`
- Drift reconcile mode:
  - `mix runtime.reproject --reconcile`
  - `mix runtime.reproject --reconcile --run-id <run_id> --no-repair`

Optional filters:

- `--since <ISO-8601 UTC>`
- `--until <ISO-8601 UTC>`
- `--limit <n>`

## Guarantees

- Per-run monotonic watermarks in `runtime.projection_watermarks`.
- Idempotent apply markers in `runtime.projection_applied_events`.
- Duplicate and out-of-order event application safety.
- Deterministic repair path: clear run-scoped projection rows + rebuild from
  runtime event log.

## Operational Notes

- Use `--dry-run` first in production incident response.
- Prefer run-scoped repair before broad thread/time-window rebuild.
- If drift repeats, inspect writer lag and event ingestion health before
  repeatedly forcing rebuilds.
