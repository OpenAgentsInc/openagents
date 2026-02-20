# Khala Surface Contract (v1)

Date: 2026-02-20
Status: Proposed

Purpose: lock which product surfaces consume Khala, which topics they subscribe to, and which HTTP endpoints are used for initial hydration.

## Global v1 rules

1. Topics are coarse per model class.
2. Surface auth comes from Laravel-minted sync token (`/api/sync/token`).
3. Initial hydration is HTTP; Khala WS provides incremental updates.
4. Clients persist per-topic watermark and maintain local doc cache by `doc_key`.

## Topic catalog (v1)

- `runtime.run_summaries`
- `runtime.codex_worker_summaries`

## Surface mapping

| Surface | Topic subscriptions | Initial hydration endpoints | Watermark storage | Status |
|---|---|---|---|---|
| Web Codex admin (`apps/openagents.com`) | `runtime.codex_worker_summaries`, optional `runtime.run_summaries` | Existing runtime-proxied endpoints in Laravel (`/api/runtime/codex/workers*`) + planned sync hydration endpoints (`/api/sync/v1/doc/:doc_key`, optional `/api/sync/v1/list/:collection`) | localStorage/IndexedDB | Implemented behind `VITE_KHALA_SYNC_ENABLED`; summary lane runs on Khala WS when enabled and retains legacy polling when disabled |
| Mobile Codex workers (`apps/mobile`) | `runtime.codex_worker_summaries` | Existing runtime APIs used by screen today (`/api/runtime/codex/workers*`) + planned sync hydration endpoints for stale cursor recovery | AsyncStorage/SQLite | Planned; high-priority early migration |
| Desktop status surfaces (`apps/desktop`) | `runtime.codex_worker_summaries`, optional `runtime.run_summaries` | Existing Laravel APIs for task and status retrieval + planned sync hydration endpoints | SQLite | Planned |
| Lightning ops dashboards (`apps/lightning-ops`) | Not in Khala wave 1 | Convex-backed control-plane flows today; new Postgres-backed APIs in wave 2 | N/A in wave 1 | Deferred to Khala phase C |

## Out of scope for v1

- Per-tenant topics.
- Arbitrary query subscriptions.
- Sync-based authority writes.

## Change control

Any new surface/topic pairing must update this file and `docs/sync/ROADMAP.md` in the same change.
