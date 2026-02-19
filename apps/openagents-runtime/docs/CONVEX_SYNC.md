# Convex Sync Layer Integration

This document defines how `openagents-runtime` integrates with self-hosted
Convex as a reactive sync layer.

## Authority Boundary

`openagents-runtime` remains the source of truth for:

- run event logs and stream sequence ordering
- codex worker lifecycle/events
- policy and spend decisions
- replay/receipt artifacts

Convex is projection-only:

- derived read models for subscription-driven clients
- low-latency summary state for web/mobile/desktop
- non-authoritative UI sync state

Runtime correctness must never depend on Convex state.

## Write Topology

Runtime is the single writer for Convex projection documents.

Do not allow multiple writers (Laravel + runtime + clients) to mutate the same
projection docs. All projection writes are deterministic transforms from runtime
events.

Current runtime writer modules:

- `OpenAgentsRuntime.Convex.Projector`
- `OpenAgentsRuntime.Convex.Sink` (behavior contract)
- `OpenAgentsRuntime.Convex.NoopSink` (default)
- `OpenAgentsRuntime.Convex.HttpSink` (Convex `/api/mutation` sink)

Projector-owned document IDs:

- `runtime/run_summary:<run_id>`
- `runtime/codex_worker_summary:<worker_id>`

## Projection Contract

Each projection document should include:

- `runtime_source.run_id`
- `runtime_source.seq` (or `seq_range`)
- `projected_at`
- `projection_version`

This allows drift checks and deterministic replay/rebuild.

## Rebuild Posture

If projection drift is detected:

1. keep runtime event log as truth
2. clear stale projection docs
3. replay projector from runtime event history

This mirrors runtime reprojection posture for Laravel-facing read models.

## Auth Model

- Laravel remains user session authority (WorkOS/OpenAgents session).
- Laravel mints short-lived Convex auth JWTs for clients.
- Convex validates OpenAgents JWT issuer/audience.
- Convex admin keys remain operator-only and are never issued to end-user
  clients.

## Runtime Contract Status

No `/internal/v1/*` endpoint is designated as a Convex projection ingest API.
Projection publishing is an internal runtime concern implemented by
runtime-owned writer paths in `OpenAgentsRuntime.Convex.Projector`.

For active rollout sequencing, see:

- `docs/plans/active/convex-self-hosting-runtime-sync-plan.md`
- `docs/codex/unified-runtime-desktop-plan.md`
