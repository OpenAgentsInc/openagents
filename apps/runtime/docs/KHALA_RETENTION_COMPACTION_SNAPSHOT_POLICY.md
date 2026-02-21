# Khala Retention, Compaction, and Snapshot Policy (OA-RUST-085)

Status: Active  
Owner: Runtime/Khala  
Last updated: 2026-02-21

## Purpose

Define and enforce bounded replay behavior for Khala topics so storage and replay cost remain controlled while deterministic recovery stays available.

## Invariants

1. Runtime remains authority; Khala replay journal is delivery-only.
2. Retention is applied per topic class, not as one global horizon.
3. Compaction is tail-prune only for `runtime.sync_stream_events`; authority and read-model tables are not compacted by this job.
4. Stale cursor responses are deterministic and include snapshot bootstrap metadata when snapshot-capable topics are affected.

## Topic Classes and Retention Windows

Source of truth in code: `apps/runtime/lib/openagents_runtime/sync/topic_policy.ex` and `apps/runtime/config/config.exs`.

| Topic | Class | Retention (seconds) | Retention (human) | Compaction Mode | Snapshot |
|---|---|---:|---|---|---|
| `runtime.run_summaries` | `durable_summary` | `604800` | 7 days | `tail_prune_with_snapshot_rehydrate` | Enabled (`runtime.sync_run_summaries`) |
| `runtime.codex_worker_summaries` | `durable_summary` | `259200` | 3 days | `tail_prune_with_snapshot_rehydrate` | Enabled (`runtime.sync_codex_worker_summaries`) |
| `runtime.codex_worker_events` | `high_churn_events` | `86400` | 1 day | `tail_prune_without_snapshot` | Disabled |
| `runtime.notifications` | `ephemeral_notifications` | `43200` | 12 hours | `tail_prune_without_snapshot` | Disabled |

Fallback for unknown topics: `86400` seconds.

## Snapshot Contract (`openagents.sync.snapshot.v1`)

Snapshot metadata is attached to stale-cursor responses under `snapshot_plan.topics[].snapshot`.

Required fields:

- `topic`
- `format` (`openagents.sync.snapshot.v1`)
- `schema_version` (currently `1`)
- `cadence_seconds`
- `source_table`

Example:

```json
{
  "topic": "runtime.run_summaries",
  "format": "openagents.sync.snapshot.v1",
  "schema_version": 1,
  "cadence_seconds": 300,
  "source_table": "runtime.sync_run_summaries"
}
```

## Runtime Enforcement

Retention implementation:

- `OpenAgentsRuntime.Sync.RetentionJob` now computes per-topic cutoffs from topic policy.
- Pruning is executed by topic in bounded batches.
- `run_once/1` returns `topic_stats` including deletion counts, class, compaction mode, retention seconds, head/oldest watermarks, and stale-risk measurements.

Stale cursor behavior:

- `OpenAgentsRuntimeWeb.SyncChannel` returns:
  - `code=stale_cursor`
  - `full_resync_required=true`
  - `stale_topics[]`
  - `snapshot_plan` (format + per-topic snapshot metadata for snapshot-capable topics)

## Observability

New telemetry metric families in `OpenAgentsRuntime.Telemetry.Metrics`:

- `openagents_runtime.sync.retention.cycle.*`
- `openagents_runtime.sync.retention.topic.*`

Topic-level telemetry tags:

- `event_type`
- `status`
- `topic_class`
- `snapshot`

## Verification

Primary tests:

- `apps/runtime/test/openagents_runtime/sync/retention_job_test.exs`
- `apps/runtime/test/openagents_runtime/sync/topic_policy_test.exs`
- `apps/runtime/test/openagents_runtime_web/channels/sync_channel_test.exs` (stale cursor snapshot plan)

Run:

```bash
cd apps/runtime
mix test test/openagents_runtime/sync/retention_job_test.exs \
  test/openagents_runtime/sync/topic_policy_test.exs \
  test/openagents_runtime_web/channels/sync_channel_test.exs \
  test/openagents_runtime/telemetry/metrics_test.exs
```
