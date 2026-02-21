# Khala Sync Layer Integration

This document defines how `runtime` implements Khala as a runtime-owned sync subsystem.

## Authority Boundary

`runtime` remains the source of truth for:

- run/worker event logs and sequencing,
- policy and spend decisions,
- replay/receipt artifacts.

Khala is projection-only delivery infrastructure:

- derived read models for reactive clients,
- replay journal + watermark resume,
- non-authoritative UI sync state.

Runtime correctness must never depend on client delivery success.

## Runtime-Owned Topology

Khala v1 runs inside runtime and shares runtime Postgres.

Why:

- transactional coupling between projector writes and stream append,
- simpler failure handling (DB replay as delivery authority),
- no cross-service compensator requirement in v1.

## Writer Model

Runtime is the single writer for Khala projection/read-model state.

Current modules:

- `OpenAgentsRuntime.Khala.Projector`
- `OpenAgentsRuntime.Khala.Sink` (behavior)
- `OpenAgentsRuntime.Khala.NoopSink` (default in some envs)
- `OpenAgentsRuntime.Khala.FanoutSink` (dual-path adapter)
- `OpenAgentsRuntime.Sync.ProjectorSink` (runtime sync tables)
- `OpenAgentsRuntime.Khala.Reprojection` (drop + replay rebuild)

Projector-owned document keys:

- `runtime/run_summary:<run_id>`
- `runtime/codex_worker_summary:<worker_id>`
- `runtime/codex_worker_event:<worker_id>:<seq>` (Codex event lane; streamed via `runtime.codex_worker_events`)

## Projection Contract

Each read-model payload should include:

- `runtime_source.run_id`
- `runtime_source.seq` (or `seq_range`)
- `projected_at`
- `projection_version`

Runtime records checkpoints in:

- `runtime.khala_projection_checkpoints`

Checkpoint fields:

- `projection_name`
- `entity_id`
- `document_id`
- `last_runtime_seq`
- `projection_version`
- `summary_hash`
- `last_projected_at`

Idempotent rule:

- if `(last_runtime_seq, projection_version, summary_hash)` matches, skip sink write.

## Runtime Sync Tables

Khala runtime tables include:

- `runtime.sync_topic_sequences`
- `runtime.sync_stream_events`
- `runtime.sync_run_summaries`
- `runtime.sync_codex_worker_summaries`

These tables are runtime-owned and are not Laravel authority tables.

## Rebuild Posture

If projection drift is detected:

1. keep runtime event log as truth,
2. clear stale projections,
3. replay from runtime event history.

Operational entrypoint:

- `mix runtime.khala.reproject --run-id <run_id>`
- `mix runtime.khala.reproject --worker-id <worker_id>`
- `mix runtime.khala.reproject --all`

## Auth Model

- Laravel remains auth/session authority.
- Laravel mints short-lived sync tokens (`/api/sync/token`, `/api/khala/token` compatibility).
- Runtime/Khala validates issuer, audience, expiry, and scoped entitlements.
- Operator secrets are never issued to end-user clients.

## Runtime Contract Status

No `/internal/v1/*` endpoint is designated as a projection ingest API.

Projection publishing is a runtime-internal concern implemented by runtime-owned writers.

For rollout sequencing:

- `docs/sync/thoughts.md`
- `docs/sync/ROADMAP.md`
- `docs/sync/SURFACES.md`
