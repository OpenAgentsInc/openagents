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

## Retention, Compaction, and Snapshot Policy

- Policy authority: `apps/runtime/docs/KHALA_RETENTION_COMPACTION_SNAPSHOT_POLICY.md`
- Runtime applies retention per topic class (not one global horizon).
- Runtime enforces per-topic QoS tiers and replay-budget ceilings during resume/bootstrap.
- Compaction mode is tail-prune for replay journal rows in `runtime.sync_stream_events`.
- Summary topics provide snapshot bootstrap metadata (`openagents.sync.snapshot.v1`) in stale-cursor responses.
- Event-only topics remain tail-only replay (no snapshot bootstrap source).
- Live delivery fairness is enforced with per-connection queue bounds + topic-slice drain scheduling; slow consumers are throttled/disconnected deterministically.

## Publish and Frame Limits (OA-RUST-088)

Runtime enforces topic-class publish and frame-size limits in the Rust fanout hub before frames are accepted into the delivery queue.

Topic classes:

- `run_events` (`run:<run_id>:events`)
- `worker_lifecycle` (`worker:<worker_id>:lifecycle`)
- `codex_worker_events` (`runtime.codex_worker_events`)
- `fallback` (any other topic pattern)

Environment controls:

- `RUNTIME_KHALA_RUN_EVENTS_PUBLISH_RATE_PER_SECOND`
- `RUNTIME_KHALA_WORKER_LIFECYCLE_PUBLISH_RATE_PER_SECOND`
- `RUNTIME_KHALA_CODEX_WORKER_EVENTS_PUBLISH_RATE_PER_SECOND`
- `RUNTIME_KHALA_FALLBACK_PUBLISH_RATE_PER_SECOND`
- `RUNTIME_KHALA_RUN_EVENTS_MAX_PAYLOAD_BYTES`
- `RUNTIME_KHALA_WORKER_LIFECYCLE_MAX_PAYLOAD_BYTES`
- `RUNTIME_KHALA_CODEX_WORKER_EVENTS_MAX_PAYLOAD_BYTES`
- `RUNTIME_KHALA_FALLBACK_MAX_PAYLOAD_BYTES`

Deterministic violation reason codes:

- `khala_publish_rate_limited` (HTTP 429 on publish path)
- `khala_frame_payload_too_large` (HTTP 413 on publish path)

Operator visibility:

- Per-topic violation counters and last reason are exposed in Khala fanout topic windows (`/internal/v1/khala/fanout/hooks` and `/internal/v1/khala/fanout/metrics`).

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
- WS threat model + anti-replay policy: `apps/runtime/docs/KHALA_WS_THREAT_MODEL.md`

## Compatibility Gate Model

- Khala can enforce compatibility windows during socket join (`compat_enforced` in `:khala_sync_auth` config).
- Required join metadata when enforced:
  - `client_build_id`
  - `protocol_version`
  - `schema_version`
- Rejection payloads are deterministic and include:
  - failure `code` (`invalid_client_build`, `unsupported_protocol_version`, `unsupported_schema_version`, `upgrade_required`, `unsupported_client_build`)
  - active support window (`min_client_build_id`, `max_client_build_id`, schema min/max, `protocol_version`)
- Rejections emit sync auth telemetry with `surface=khala_websocket` and client/build identifiers.

## Runtime Contract Status

No `/internal/v1/*` endpoint is designated as a projection ingest API.

Projection publishing is a runtime-internal concern implemented by runtime-owned writers.

Ordering and delivery contract authority:

- `apps/runtime/docs/KHALA_ORDERING_DELIVERY_CONTRACT.md`

For rollout sequencing:

- `docs/sync/thoughts.md`
- `docs/sync/ROADMAP.md`
- `docs/sync/SURFACES.md`
