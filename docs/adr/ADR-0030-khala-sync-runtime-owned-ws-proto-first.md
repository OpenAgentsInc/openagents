# ADR-0030: Khala Sync Is Runtime-Owned, WS-Only, and Proto-First

## Status

**Accepted**

## Date

2026-02-20

## Context

OpenAgents currently uses Convex as a projection/read-model sync layer while runtime + Postgres hold execution authority. We need a first-party sync path that preserves authority boundaries, replay correctness, and shared schema authority across all clients.

The migration must avoid a big-bang cutover, keep current surfaces functional, and avoid introducing a second source of truth.

## Decision

Khala is the codename for the new OpenAgents sync engine.

> Khala v1 will ship as a runtime-owned subsystem inside `apps/openagents-runtime`, using WebSockets (Phoenix Channels) only for live sync, with proto-first contracts under `proto/openagents/sync/v1`.

Normative constraints:

1. Runtime + Postgres remain authoritative for state and events.
2. Khala is projection + delivery only.
3. New Khala live transport is WS only (no new SSE endpoints for Khala).
4. Topic watermarks are allocated in Postgres via per-topic sequence rows.
5. Stream journal semantics are ordering-first; payload authority is read-model tables.
6. Convex remains non-authoritative and migration-scoped only.

### Schema / Spec Authority

- [proto/openagents/sync/v1](../../proto/openagents/sync/v1/) — canonical Khala wire contracts
- [docs/sync/thoughts.md](../sync/thoughts.md) — architecture and operational spec
- [docs/protocol/OA_SYNC_WS_MAPPING.md](../protocol/OA_SYNC_WS_MAPPING.md) — proto-to-WS mapping
- [docs/ARCHITECTURE.md](../ARCHITECTURE.md) — system topology and authority boundaries

## Scope

What this ADR covers:

- Khala ownership/deployment boundary for v1
- WS-only transport decision for Khala
- Proto package authority for sync contracts
- Watermark and replay invariants needed for resumable delivery
- Convex coexistence rules during migration

What this ADR does NOT cover:

- Full runtime implementation details (tracked in KHALA issues)
- Lightning second-wave schema/API migrations
- Specific UI rollout flags by surface

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Authority | Runtime + Postgres remain source of truth |
| Khala deployment | v1 is in-process with `apps/openagents-runtime` |
| Khala transport | WS-only for new sync lane |
| Watermark scope | Monotonic per-topic sequence in Postgres |
| Replay correctness | Resume from durable journal by topic/watermark |
| Schema authority | Proto-first contracts in `proto/openagents/sync/v1` |
| Convex role | Projection-only and migration-scoped |

Backward compatibility expectations:

- Existing SSE endpoints remain available for current runtime APIs.
- Existing Convex lanes may remain until each surface is migrated.
- Khala protocol changes follow additive protobuf evolution rules.

## Consequences

**Positive:**

- Strong correctness semantics from transactional coupling in runtime + Postgres
- Clear sync boundary that does not recreate arbitrary server-side query execution
- Shared contracts for web/mobile/desktop from one proto package

**Negative:**

- Runtime initially carries extra sync responsibilities until/if later extraction
- Temporary dual-publish complexity during migration windows

**Neutral:**

- Convex remains in use for non-migrated lanes until phased cutover completes

## Alternatives Considered

1. **Keep Convex as permanent sync layer** — rejected; does not meet first-party control and proto-first schema goals.
2. **Extract Khala as a separate service immediately** — rejected for v1; introduces cross-service transaction gaps before semantics are proven.
3. **Support SSE + WS for new Khala lane** — rejected; increases transport complexity with no correctness benefit for v1 goals.

## Migration Plan

1. Runtime/Codex dual-publish: Khala + Convex projection outputs run in parallel.
2. Surface cutovers: web/mobile/desktop move subscriptions to Khala behind flags.
3. Lightning second wave: move control-plane authority from Convex to runtime/Postgres APIs.
4. Decommission: remove remaining Convex dependencies after rollback windows.

## References

- [ADR-0028](./ADR-0028-layer0-proto-canonical-schema.md)
- [ADR-0029](./ADR-0029-convex-sync-layer-and-codex-agent-mode.md)
- [docs/sync/ROADMAP.md](../sync/ROADMAP.md)
- [docs/sync/SURFACES.md](../sync/SURFACES.md)
- [docs/GLOSSARY.md](../GLOSSARY.md)
