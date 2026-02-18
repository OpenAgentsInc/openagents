# Runtime DB Baseline

This document defines baseline schema and naming conventions for runtime data.

## Namespace

All runtime tables and sequences live under the `runtime` Postgres schema.

## Extensions

- `pgcrypto` is enabled for runtime migrations that need deterministic hashing and UUID utilities.

## Baseline objects

- `runtime.runs`
  - primary key: `run_id`
  - required: `thread_id`, `status`, owner principal (`owner_user_id` or `owner_guest_scope`)
  - sequencing field: `latest_seq`
- `runtime.global_event_id_seq`
  - global sequence reserved for runtime append flows and projection watermarks.

## Naming conventions

- Table names: plural snake_case (example: `run_events`, `run_leases`)
- Primary IDs: `{entity}_id` (example: `run_id`, `frame_id`)
- Time columns: `inserted_at`, `updated_at` in UTC microseconds
- Index names: Ecto defaults + `runtime` prefix
- Constraint names: `{table}_{intent}` (example: `runs_owner_present`)

## Query/index guidance

- Append and replay paths must index by `(run_id, seq)`.
- Thread-facing reads should index `(thread_id, created_at/updated_at)`.
- Projection jobs must maintain monotonic watermarks in dedicated tables.
