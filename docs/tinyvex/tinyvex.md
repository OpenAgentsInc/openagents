# Tinyvex: A Minimal Local Sync DB (SQLite + WS)

## Summary
- Tinyvex is a tiny, self-hosted persistence and live-sync layer designed to boot in under a second and keep the mobile app in sync with the desktop bridge.
- It replaces heavy Convex cold-starts for basic local development and on-the-go usage while preserving Codex JSONL as the source of truth.
- Scope is intentionally small: a single SQLite file, a WebSocket changefeed, and a handful of queries/mutations the app needs.

## Goals
- Instant startup: sub‑second boot on typical hardware.
- Minimal footprint: one SQLite file under `~/.openagents/tinyvex/` and an in-process WS endpoint.
- Live updates: push-only subscriptions for the app’s Threads and Messages views.
- Deterministic writes: last‑write‑wins with `updatedAt` timestamps.
- Zero external tooling: no Node CLI or external backend required.

## Non‑Goals
- No distributed sync or multi-host replication.
- No background workers beyond the single bridge process lifetime.
- No advanced features (search, vectors, complex indexes). These remain out‑of‑scope for Tinyvex and continue to be handled by Convex when enabled.

## Architecture
- Process model: Tinyvex runs inside the Rust bridge process (same binary) as a module.
- Storage: SQLite via `rusqlite` (or `sqlx` feature for SQLite), one connection pool.
- API surface:
  - WebSocket channel (reuses the bridge `/ws`) with control verbs for query/subscribe/mutate.
  - Optional HTTP health: `GET /tinyvex/health` returns `200 { ok: true }`.
- App integration: the Expo app subscribes over the existing bridge WS; no separate HTTP client.
- Source of truth: Codex JSONL rollouts remain authoritative; Tinyvex mirrors for fast querying/live UI.

## Data Model (DDL)
- SQLite file: `~/.openagents/tinyvex/data.sqlite3`
- Tables and minimal indexes:

```
-- threads
CREATE TABLE IF NOT EXISTS threads (
  id TEXT PRIMARY KEY,        -- Stable doc id (string)
  threadId TEXT,              -- Alias / external id (optional)
  title TEXT NOT NULL,
  projectId TEXT,
  resumeId TEXT,
  rolloutPath TEXT,
  source TEXT,
  createdAt INTEGER NOT NULL, -- ms
  updatedAt INTEGER NOT NULL  -- ms
);
CREATE INDEX IF NOT EXISTS idx_threads_updated ON threads(updatedAt DESC);

-- messages (append-only per thread)
CREATE TABLE IF NOT EXISTS messages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  threadId TEXT NOT NULL,
  role TEXT,                  -- 'user' | 'assistant' | 'system'
  kind TEXT NOT NULL,         -- 'message' | 'reason' | 'cmd' | ...
  text TEXT,
  data TEXT,                  -- JSON string (optional)
  itemId TEXT,                -- stable streamed item id (optional)
  partial INTEGER,            -- 0/1 for streaming partials
  seq INTEGER,                -- streaming sequence counter
  ts INTEGER NOT NULL,        -- ms
  createdAt INTEGER NOT NULL,
  updatedAt INTEGER
);
CREATE INDEX IF NOT EXISTS idx_msgs_thread_ts ON messages(threadId, ts);
CREATE INDEX IF NOT EXISTS idx_msgs_thread_item ON messages(threadId, itemId);

-- projects (minimal)
CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  workingDir TEXT NOT NULL,
  repo JSON,
  createdAt INTEGER NOT NULL,
  updatedAt INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);

-- skills (minimal)
CREATE TABLE IF NOT EXISTS skills (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  skillId TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  source TEXT NOT NULL,        -- 'user' | 'registry' | 'project'
  projectId TEXT,
  path TEXT,
  createdAt INTEGER NOT NULL,
  updatedAt INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_skills_project ON skills(projectId);
```

## WS Protocol (over the existing bridge `/ws`)
- All messages are JSON objects; requests include `id` for correlation.
- Auth: reuses the bridge token — no extra auth layer.

Requests
- `{"control": "tvx.subscribe", "stream": "threads"}` → streams full snapshot then incremental updates.
- `{"control": "tvx.subscribe", "stream": "messages", "threadId": "<id>"}` → snapshot + updates for a thread.
- `{"control": "tvx.query", "name": "threads.list", "limit": 20}` → one‑shot query; returns a single response.
- `{"control": "tvx.mutate", "name": "messages.create", "args": { ... }}` → write and ack.

Server → Client
- Snapshot: `{"type": "tinyvex.snapshot", "stream": "threads", "rows": [...] , "rev": 12}`
- Update:   `{"type": "tinyvex.update",   "stream": "threads", "ops": [{"op":"upsert","row":{...}}], "rev": 13}`
- Ack:      `{"type": "tinyvex.ack", "id": "<reqId>", "ok": true}`
- Error:    `{"type": "tinyvex.err", "id": "<reqId>", "error": "message"}`

Revisioning
- Each stream maintains an integer `rev` incremented per commit; clients discard stale updates `< currentRev`.
- Last‑write‑wins based on `updatedAt` for row conflicts.

## Queries & Mutations (MVP)
- Queries
  - `threads.list(limit?: number)` → recent threads by `updatedAt desc`.
  - `messages.list(threadId: string, limit?: number)` → recent tail by `ts asc` or `desc` + client inverts.
  - `projects.list()` and `skills.listAll()` (optional in v1; app can keep using existing stores).
- Mutations
  - `threads.upsertFromStream({ threadId?, resumeId?, title?, projectId?, rolloutPath?, source?, createdAt?, updatedAt? })`
  - `messages.upsertStreamed({ threadId, kind, role?, text, itemId, seq, ts })`
  - `messages.finalizeStreamed({ threadId, itemId, text, ts })`
  - `projects.upsertFromFs({ ... })`, `skills.upsertFromFs({ ... })` (called by bridge watchers)

## Ingestion (Bridge ↔ Tinyvex)
- On JSONL events, the bridge writes to Tinyvex using the same functions as Convex mapping (`convex_write.rs` can be adapted to a `tinyvex_write.rs`).
- Filesystem watchers (Projects/Skills) call `upsertFromFs` mutations.
- Backfill: a WS control `{ "control": "tvx.backfill" }` triggers a scan of Codex sessions and bulk inserts.

## Health & Lifecycle
- Health endpoint: `GET /tinyvex/health` returns `{ ok: true }` after DB open.
- Startup: open DB, init DDL, load last `rev` per stream into memory, ready to serve.
- Shutdown: single process lifetime; no resident daemons.

## Auth & Security
- Reuse the bridge token; all Tinyvex controls share the same WS channel.
- Bind on loopback by default; LAN exposure controlled by the bridge’s bind/interface.

## Performance Targets
- Cold-start: < 1s to open DB and init tables on a typical dev machine.
- Subscribe snapshot: < 50ms for ≤ 100 threads; messages pagination for larger threads.
- Incremental update push: < 10ms overhead per upsert on the hot path.

## Integration Plan
- Add `tinyvex` module in the bridge: SQLite pool, DDL init, revision manager, and WS handlers.
- Implement `tvx.subscribe`, `tvx.query`, `tvx.mutate`, `tvx.backfill` in the existing WS router.
- Add `tinyvex_write.rs` mirroring a subset of `convex_write.rs` to call into SQLite.
- Add a feature flag/env: `OPENAGENTS_TINYVEX=1` → use Tinyvex path; else keep Convex integration.
- Expo app: add a Tinyvex client adapter that uses the existing WS to subscribe to `tinyvex.*` events; wire Threads view first.

## Rollout & Fallback
- Default off behind `OPENAGENTS_TINYVEX=1` for early testing.
- Keep Convex code path intact for full features; allow switching in Settings.
- If Tinyvex errors, log and fall back to Convex/no‑DB behavior; the app still renders via the live JSONL feed.

## Acceptance Criteria (MVP)
- Bridge starts with `OPENAGENTS_TINYVEX=1` and logs `tinyvex ready` in < 1s.
- `/session` in the app shows live threads when subscribing to `tvx.subscribe: threads`.
- Streaming assistant/reasoning text appears in `messages` subscription with at‑least‑once delivery.
- No long-lived processes remain after the bridge exits.

## Future Enhancements (Optional)
- Add simple full‑text search using SQLite FTS5 for message text.
- Add basic vector search via a local library if needed (still optional; keep start time fast by lazy init after first request).
- Persist lightweight indices for titles and timestamps.

## Risks
- Divergence from Convex API; mitigate with a minimal adapter in the app.
- Schema evolution; keep DDL guarded with `IF NOT EXISTS` and migrations as simple `ALTER TABLE` with feature flags.
- Potential duplication of write logic; factor shared mapping utilities between Convex and Tinyvex.

