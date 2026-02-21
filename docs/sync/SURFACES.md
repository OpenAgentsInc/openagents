# Khala Surface Contract (Rust-Era v1)

Date: 2026-02-21  
Status: Active  
Authority ADRs: `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`, `docs/adr/ADR-0002-proto-first-contract-governance.md`

Purpose: define authoritative Khala consumers, topic scope expectations, bootstrap paths, and resume/replay behavior.

## Global Rules

1. Khala live transport is WebSocket-only for all active consumers.
2. Commands/mutations are HTTP-only and never travel through Khala topics.
3. Sync tokens are minted by the Rust control service via `POST /api/sync/token`.
4. HTTP bootstrap is required before WS subscribe/resume.
5. Clients persist per-topic watermarks and apply frames idempotently by `(topic, seq)`.
6. Clients must discard duplicates where `seq <= last_applied`.
7. `stale_cursor` requires snapshot/bootstrap recovery before resuming live tail.

## Topic Catalog (v1)

- `runtime.run_summaries`
- `runtime.codex_worker_summaries`
- `runtime.codex_worker_events`
- `runtime.notifications`

## Authoritative Consumer Matrix

| Surface | Primary Khala Topics | Bootstrap + Recovery HTTP | Local Watermark Storage | Integration Scope |
|---|---|---|---|---|
| `apps/openagents.com/web-shell` | `runtime.codex_worker_summaries`, optional `runtime.run_summaries` | Control/runtime API hydration endpoints + `POST /api/sync/token` | browser local storage + IndexedDB cache | Default web administration lane |
| `apps/autopilot-desktop` | `runtime.codex_worker_events` (primary), optional `runtime.codex_worker_summaries` | runtime worker APIs + `POST /api/sync/token` | desktop-local persistence | Primary operator lane for Codex event administration |
| `apps/autopilot-ios` | `runtime.codex_worker_events` (primary), optional `runtime.codex_worker_summaries` | runtime worker APIs + `POST /api/sync/token` | app-local persistence | Mobile follow/monitor lane for Codex/runtime state |
| `apps/onyx` | optional `runtime.run_summaries` only (no worker-event subscription in v1) | selected control/runtime read APIs + `POST /api/sync/token` when enabled | app-local vault metadata | Explicitly limited integration scope; not a full Codex admin surface |

## Command vs Subscription Semantics

1. HTTP command path:
   - UI action -> authenticated HTTP API call -> authority write (`control.*` or `runtime.*`) -> response
2. WS subscription path:
   - topic subscribe/resume -> replay gap fill -> live tail frames
3. Prohibited:
   - RPC-style commands over Khala topics
   - authority mutation over WebSocket frames

## Out of Scope (v1)

- Per-tenant dynamic topic creation.
- Arbitrary ad hoc query subscriptions.
- Lightning service (`apps/lightning-ops`, `apps/lightning-wallet-executor`) as Khala consumers.

## Historical Notes

Removed legacy surfaces are non-canonical and not part of this matrix:
- `apps/mobile/` (removed)
- `apps/desktop/` (removed)
- `apps/inbox-autopilot/` (removed)

## Change Control

Any new surface/topic pairing must update this file and `docs/sync/ROADMAP.md` in the same change.
