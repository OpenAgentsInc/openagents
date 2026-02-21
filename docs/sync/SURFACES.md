# Khala Surface Contract (v1)

Date: 2026-02-20  
Status: Active

Purpose: define exactly how each app consumes Khala, which topics it subscribes to, and what HTTP paths are used for bootstrap and recovery.

## Global Rules

1. Topics are coarse per model class.
2. Auth tokens are minted by Laravel (`/api/sync/token`, with `/api/khala/token` retained for compatibility).
3. Initial hydration is HTTP; Khala only carries incremental updates.
4. Each client persists per-topic watermark and maintains a `doc_key` cache.
5. Clients must apply updates idempotently with doc-version monotonic checks.

## Topic Catalog (v1)

- `runtime.run_summaries`
- `runtime.codex_worker_summaries`
- `runtime.codex_worker_events`
- `runtime.notifications`

## Surface Matrix

| Surface | Khala Subscription Topics | Bootstrap + Recovery HTTP | Local Watermark Storage | Current Status |
|---|---|---|---|---|
| Web Codex admin (`apps/openagents.com`) | `runtime.codex_worker_summaries`, optional `runtime.run_summaries` | `/api/runtime/codex/workers*` + sync hydration endpoints as they land | localStorage/IndexedDB | Feature-gated via `VITE_KHALA_SYNC_ENABLED` |
| Mobile Codex workers (`apps/mobile`) | `runtime.codex_worker_summaries` | `/api/runtime/codex/workers*` + stale-cursor hydration endpoints | AsyncStorage/SQLite/MMKV-backed persistence | Feature-gated via `EXPO_PUBLIC_KHALA_SYNC_ENABLED` |
| Desktop status surfaces (`apps/desktop`) | `runtime.codex_worker_summaries`, optional `runtime.run_summaries` | Laravel/runtime status APIs + sync hydration endpoints | SQLite | Feature-gated via `OA_DESKTOP_KHALA_SYNC_ENABLED` |
| Autopilot iOS (`apps/autopilot-ios`) | `runtime.codex_worker_events` (live Codex event lane), optional `runtime.codex_worker_summaries` | `/api/runtime/codex/workers*`, `/api/runtime/codex/workers/{id}/events`, `/api/sync/token` | app-local (UserDefaults in v1) | Khala WS is primary live lane |
| Autopilot desktop inbox panes (`apps/autopilot-desktop`) | none in v1 primary lane (inbox is local projected domain inside desktop shell) | desktop command lane + local inbox projection state | desktop-local state | Consolidated into desktop shell (standalone inbox app removed) |
| Lightning ops (`apps/lightning-ops`) | none for control-plane lane | `/api/internal/lightning-ops/control-plane/query|mutation` | N/A | API/mock transport; no Khala authority dependency |
| Lightning wallet executor (`apps/lightning-wallet-executor`) | none | service-local + Lightning infra | N/A | Not a Khala consumer |

## Out of Scope for v1

- Per-tenant topics.
- Arbitrary query subscriptions.
- Authority writes through sync transport.

## Change Control

Any new surface/topic pairing must update this file and `docs/sync/ROADMAP.md` in the same change.
