# ADR 0003 — Tinyvex as the Local Sync Engine (SQLite + WS)

 - Date: 2025-10-31
 - Status: Accepted — Updated for two‑package client and bridge‑hosted writer

## Context

Local development and on-the-go usage require a sync layer that starts instantly and keeps the app’s views coherent with desktop agent runs. Our previous local path relied on Convex, but the local Convex dev server commonly takes ~30 seconds to boot, introduces extra processes and network dependencies, and adds friction for contributors who only need fast, offline session history and streaming.

We need a simple, in‑process alternative that:
- Boots in under a second.
- Mirrors provider events into a queryable store for the app.
- Pushes live updates over the same WebSocket used for control.
- Maintains strict ACP type parity while avoiding external services.

## Decision

Adopt Tinyvex as our local sync engine:
- In‑process SQLite database + lightweight WebSocket changefeed.
- Source of truth remains at the provider boundary (ACP events, Codex JSONL rollouts). Tinyvex mirrors for fast queries and live UI.
- Typed rows are defined in Rust and exported to TypeScript via `ts-rs` into `tricoder/types`.
- The app and libraries consume typed Tinyvex snapshots/updates and make typed queries over the existing bridge WebSocket (`/ws`).

Scope is intentionally small: Threads, Messages, and Tool Calls for live session views and the drawer; optional Projects/Skills mirroring for UI chrome.

## Rationale

- Fast startup: Single process, single file. Typical cold‑start is < 1s (open DB, ensure DDL, initialize in‑memory revision counters).
- Simplicity: No separate servers, CLIs, or network bootstrap; no schema/driver setup for contributors.
- Local‑first: Works offline and over LAN; mobile connects directly to the bridge with a token.
- Typed contract: Rust structs exported to TS keep app and bridge in lock‑step (see ADR 0002). WS payloads are snake_case and ACP‑aligned.
- Observability and safety: We can deterministically upsert streaming items using stable `(thread_id, item_id, seq)` keys; revision counters allow clients to discard stale updates.

## Alternatives Considered

1) Convex local dev server
- Pros: feature‑rich; built‑in queries/subscriptions.
- Cons: cold‑start (~30s), multiple processes, external runtime dependence for simple local use.

2) App‑only persistence (AsyncStorage)
- Pros: trivial to start; no desktop dependency.
- Cons: no desktop session mirroring; cannot unify streams from multiple providers or external runs.

3) External DBs (DuckDB/Postgres/SurrealDB/etc.)
- Pros: richer features (search, vectors, replication).
- Cons: heavier operational cost and boot time; unnecessary for our MVP and mobile “command center” goals.

4) Use provider‑owned stores directly
- Pros: zero duplication.
- Cons: provider formats differ (Codex JSONL vs. Claude stream/ACP); inconsistent latency; awkward to mix into a single, typed client contract.

## Consequences

- Tinyvex is not a distributed database. No multi‑host replication or conflict resolution; it is intentionally single‑host, single‑process.
- Some advanced features (full‑text search, vector search) are out of scope for Tinyvex and can remain in Convex or arrive later as optional modules.
- The bridge owns ingestion and mapping (ACP updates and watchers), so we must keep those paths tested as providers evolve.

## Implementation

- Storage
  - SQLite file: `~/.openagents/tinyvex/data.sqlite3`.
  - DDL for `threads`, `messages`, `tool_calls` (see `docs/tinyvex/schema.md`).
- Bridge integration
  - Writer hosted in the bridge that mirrors ACP updates to Tinyvex with idempotent upserts and finalization (`crates/oa-bridge/src/tvx_writer.rs`, used by `tinyvex_write.rs`).
  - Typed envelopes and rows (`tinyvex.snapshot`, `tinyvex.update`, `tinyvex.query_result`) returned over `/ws` using `ts-rs` exported types (`tricoder/types`).
  - Inbound sessions watcher for providers tails session stores and translates new‑format JSONL → ACP → Tinyvex.
- App integration
  - Use the `tinyvex` npm package (`tinyvex/react`) for `<TinyvexProvider>` and a single hook to subscribe/query over WS and render typed rows. Drawer and thread timeline hydrate from Tinyvex snapshots/updates. The thread view is Tinyvex‑driven end‑to‑end — the app must not render raw ACP updates for the core chat/tool stream.
  - Tool calls: the provider treats Tinyvex as the single source of truth. On `tinyvex.update(stream:"toolCalls")` the provider schedules a bounded `toolCalls.list` requery and dedupes by `tool_call_id` on the client.
  - Aliasing: since the bridge may emit rows under either the canonical session id or the client thread doc id, the provider MUST alias results across both keys. When listing by canonical id, also project the same rows under the client doc id (and vice‑versa) so UI keyed by the route id hydrates immediately.
  - Optional: the provider may inject a short‑lived optimistic user message upon `run.submit` to cover the small window before Tinyvex persistence completes; this optimistic row is replaced once the Tinyvex write arrives.
  - No REST layer; all control is via WebSocket messages (e.g., `tvx.subscribe`, `tvx.query`).

## Operations

- Sync toggles: `sync.enable`, `sync.two_way`, `sync.full_rescan` (WS controls). Settings exposes simple switches and a status view. Status payload is snake_case (`SyncStatusTs`).
- Optional two‑way writing (future): when enabled, emit provider‑compatible artifacts for non‑Codex providers to allow external resume/import (e.g., Codex‑compatible JSONL under `~/.codex/sessions/openagents/`).

## Acceptance

- Bridge boots and logs Tinyvex readiness quickly (< 1s typical).
- App can subscribe and query typed rows via WS with no additional services. The thread timeline renders only Tinyvex rows (messages/tool calls/plan/state); ACP updates are consumed by the bridge and mirrored into Tinyvex, not rendered directly by the app.
- Inbound Codex sessions appear in the app within seconds; Tinyvex updates are idempotent.
- WS payloads and app/client types are generated from Rust and use snake_case consistently (see ADR 0002). Types are consumed from `tricoder/types`.

## References

- Tinyvex overview: docs/tinyvex/overview.md
- Data model: docs/tinyvex/schema.md
- WS bootstrap: docs/tinyvex/ws-bootstrap.md
- Write paths: docs/tinyvex/write-paths.md
- Threads/tails sequence: docs/tinyvex/threads-and-tails-sequence.md
- App library: `tinyvex/react` (Provider + hook)
- Bridge writer: `crates/oa-bridge/src/tvx_writer.rs` (used by `tinyvex_write.rs`)
- Sessions watcher: `crates/oa-bridge/src/watchers/*`
- ADR 0002 (Rust → TS types): docs/adr/0002-rust-to-ts-types.md
