# Tinyvex Local Sync Engine — Audit (2025-10-31)

Author: Audit agent
Scope: End-to-end review of the local sync engine (Tinyvex) and the implementation status of Issue 1351 “Full two-way sync with desktop chats”. Includes bridge (Rust), app (Expo), and supporting docs/utilities.

---

## Executive Summary

Tinyvex is now a functional, in-process persistence and live-sync layer for the bridge. It persists threads, messages, ACP tool calls/plan/state, and exposes WS controls used by the app. The core MVP of Issue 1351 is effectively delivered: inbound Codex session ingestion is enabled by default; app surfaces Sync/Two-way toggles and live status; and an opt-in two-way writer emits Codex-compatible JSONL for non-Codex providers to make sessions importable/resumable.

The implementation is pragmatic and localized, with clear separation between DB writes (tinyvex crate) and WS transport (adapter in the bridge). However, several areas would benefit from tightening: DB connection management, write batching and SQLite pragmas, id/alias persistence, backfill/mutate stubs, and coverage for plan/state/tool call hydration and two-way mapping. The current approach opens a new SQLite connection per operation and relies on in-memory alias mapping, which is workable but brittle at scale or across restarts.

Overall, this is a solid foundation with clear next steps to harden, finish rough edges, and improve performance/robustness without changing the public contract.

---

## What’s Implemented

- Database and writer
  - Tinyvex crate encapsulates schema and CRUD helpers.
    - Threads/messages and ACP artifacts: `crates/tinyvex/src/lib.rs`
    - Streaming writer and ACP mirroring: `crates/tinyvex/src/writer.rs`
  - Tables: `threads`, `messages`, `acp_events`, `acp_tool_calls`, `acp_plan`, `acp_state` with basic indexes. Last-write-wins on threads by `updatedAt`; messages use `(threadId,itemId)` unique index for idempotency during streaming.
  - `list_messages` returns the last N in ascending `ts` suitable for tails; tested for ordering and limits.

- Bridge integration
  - Tinyvex DB is initialized under `~/.openagents/tinyvex/data.sqlite3` at boot: `crates/oa-bridge/src/main.rs:117`.
  - Adapter maps typed writer notifications → WS JSON shapes (`tinyvex.update`, `bridge.tinyvex_write`): `crates/oa-bridge/src/tinyvex_write.rs`.
  - WS controls implemented (no REST): subscribe/query for threads/messages/tool calls and sync controls: `crates/oa-bridge/src/ws.rs` and `crates/oa-bridge/src/controls.rs`.
  - Codex/Claude stdout translators to ACP updates then into Tinyvex via writer: `crates/oa-bridge/src/ws.rs`, `crates/acp-event-translator/src/lib.rs`.

- Inbound sessions watcher (Codex)
  - Watches `~/.codex/sessions` recursively (honors `CODEXD_HISTORY_DIR`), tails only “new-format” JSONL, translates to ACP and mirrors into Tinyvex.
  - Persists per-file offsets and learned `thread_id` across restarts in `~/.openagents/sync/state.json`. Handles truncate/rotation by resetting offset and cached id.
  - Mirrors updates to both the canonical session id and the aliased client thread doc id when known.
  - Files: `crates/oa-bridge/src/watchers/sessions_watch.rs` (spawned by default in `main.rs`).

- Two-way writer (opt-in)
  - When `sync_two_way` is enabled and the provider is not `codex`, appends Codex-compatible new-format JSONL under `<CODEXD_HISTORY_DIR or ~/.codex/sessions>/openagents/`.
  - Writes `thread.started` once per file, then `item.completed` for user/assistant/reasoning chunks.
  - Ensures watcher ignores our two-way directory to avoid re-ingestion loops.
  - Location and logic: `crates/oa-bridge/src/tinyvex_write.rs` (see `two_way_base_dir`, `append_two_way_jsonl`).

- App wiring (Expo)
  - TinyvexProvider centralizes bootstrap and live updates: subscribes to threads; bootstraps with `threadsAndTails.list`; throttles per-thread `messages.list` during streaming; debounces `threads.list` refreshes.
  - Tool calls are listed via `tvx.query name:"toolCalls.list"` with on-demand backfill when no rows are present.
  - Settings surface sync toggles and live status; preferences persisted and re-applied.
  - Files: `expo/providers/tinyvex.tsx`, `expo/app/settings/index.tsx`, thread UI consuming Tinyvex state under `expo/app/thread/*`.

- Security
  - WS requires a token; token is provisioned/persisted at startup. `/ws` rejects when missing/invalid (`bridge.sync` is behind the same WS auth): `crates/oa-bridge/src/ws.rs`.

---

## Alignment With Issue 1351

Issue: “Full two-way sync with desktop chats” (OpenAgentsInc/openagents#1351)

- Inbound Codex watcher: Implemented and enabled by default. Mirroring to Tinyvex verified with tests and logs.
- Sync controls: Implemented (`sync.status`, `sync.enable`, `sync.two_way`, `sync.full_rescan`) and surfaced in app.
- Two-way writer: Present and gated behind `sync.two_way`; currently covers user/assistant/reasoning. Tool calls/plan/state mapping is pending (tracked in comments).
- App integration: History and threads load from Tinyvex; last-50 message tails; tool calls listed inline; settings toggles and status present.

Verdict: The core acceptance criteria are met for Codex inbound; two-way writer is partially complete but functional for baseline chat transcripts.

---

## Strengths

- Clear separation of concerns
  - `tinyvex::Writer` encapsulates DB mutations and emits typed notifications; bridge adapts to WS JSON once, minimizing coupling.
- Idempotent and streaming-aware writes
  - `(threadId,itemId)` uniqueness and `seq` counters handle deltas followed by finalize; last-write-wins for threads guards against reordering.
- Pragmatic bootstrap
  - `threadsAndTails.list` aggregates threads and tail messages, including synthetic rows for unseen threads when the DB is cold after a restart. This keeps the UI responsive.
- Sensible sync controls and defaults
  - Inbound watcher ON by default; two-way OFF. Controls are integrated with app settings and WS-only as required by the repo’s rules.
- Good test coverage on the critical path
  - Unit tests for translator, Tinyvex message ordering/tails, tool-call listing, watcher id detection.

---

## Gaps, Weaknesses, and Messy Areas

1) DB connection strategy and SQLite pragmas
- Every operation opens a fresh `rusqlite::Connection` (`Connection::open(&self.db_path)` in each method). This is simple but adds overhead, reduces opportunities for batching/transactions, and limits control over WAL/journal modes.
- No explicit PRAGMA configuration (WAL, synchronous=NORMAL/EXTRA, busy_timeout) or single shared connection. Under high-frequency streaming this could introduce contention or fsync stalls.

2) Write batching and transactions
- Streaming writes are issued one-by-one; there’s no per-thread transaction or batched finalization. This is acceptable at current scale but will be the first bottleneck under heavier tails or backfills.

3) Alias persistence and resume mapping
- `sessions_by_client_doc` and `client_doc_by_session` are in-memory (`crates/oa-bridge/src/state.rs`). After a cold start, the watcher mirrors to the canonical session id only until aliasing is re-established. The app compensates with dual queries, but the mapping should be persisted (e.g., in Tinyvex) to survive restarts and edge sequences.

4) Partial two-way writer coverage
- Two-way writes currently handle user/assistant/reasoning to Codex-compatible JSONL. Tool calls, plan, and state are not yet mapped to any provider format (acknowledged in comments). This limits fidelity for resume/import beyond plain transcripts.
- Concurrency: each update spawns an async append task; multiple concurrent appends for the same thread can cause interleaving at the file level. There’s no per-thread lock.

5) Query shapes implemented in the WS adapter layer
- `threads.listSince` and `messages.since` filter results in-process after calling `list_threads`/`list_messages`. This is fine for caps of 50–500 rows, but should move into Tinyvex for efficiency on larger sets or future paging.

6) Tool call hydration is minimal
- Tinyvex stores `content_json` and `locations_json`, but the app currently lists IDs and defers to ACP live components for render. Detail hydration via `toolCalls.list` exists, but there’s no on-demand fetch of full content for a single tool call in the thread detail view yet (noted as a next step in comments).

7) Backfill and mutate stubs
- `tvx.mutate` and `tvx.backfill` controls are placeholders that emit `tinyvex.todo`; the backfill logic for general cases is unimplemented outside the ad-hoc tool-call backfill when `toolCalls.list` is empty for a thread.

8) Thread titles and metadata quality
- Writer currently sets `title: "Thread"` for synthesized thread rows. Improved heuristics (e.g., first user line prefix, project name, or provider tag) would produce better drawer UX. `message_count` is computed as a subquery and may get expensive; consider caching or approximations.

9) ACP events table is unused
- `acp_events` is appended to but not read; no retention strategy is defined. This can grow unbounded without clear value unless we plan analytics or an audit log UI.

10) Tests and failure paths
- While unit tests cover core translators and ordering semantics, there’s limited coverage for watcher edge cases (permission errors, massive file counts, deep recursion) and two-way writer idempotency/interleaving.

11) Logging and observability
- Logs are informative but can be chatty. There’s no structured error surface to the app aside from `sync.status`; exposing last errors per provider would improve diagnostics in Settings.

---

## Security and Permissions Review

- WS access requires a token (generated/persisted at startup) and is validated on every `/ws` upgrade. Good default-deny stance.
- The watcher traverses `~/.codex/sessions` recursively and ignores the `openagents/` namespace to avoid loops. Good precaution.
- File IO uses user HOME; no secrets are read or written beyond the token and sync state; no HTTP endpoints added (complies with repo rule).
- Two-way files are written only when opt-in is set; path is under the Codex sessions base.

Risks:
- File enumeration in `sync.status` does a full recursive walk each request; large trees could make it slow. Cache or rate-limit if needed.
- Two-way writer could interleave writes without per-thread serialization, leading to malformed JSONL on rare races.

---

## Performance Considerations

- Connection-per-operation and lack of WAL mode can incur fsync overhead. WAL with a shared connection would allow concurrent readers and reduce writer stalls.
- `list_threads` runs a subquery count for each row; this is O(N) extra work and can be avoided or cached.
- Filtering “since” queries in-process is fine up to the current caps but should have DB-side predicates for scalability.

---

## Recommendations (Prioritized)

P0 – Correctness and durability
- Persist alias mapping:
  - Add a `thread_aliases` table (or embed in `threads`) to store `sessionId ↔ clientThreadDocId` and ensure watcher mirrors to both ids after cold starts. Update on `thread.started` and when a client sets/changes the mapping. Wire reads into the writer/adapter.
- Two-way per-thread serialization:
  - Introduce a per-thread async mutex or a small write queue so `append_two_way_jsonl` cannot interleave writes for the same file. Consider batching lines and flushing periodically.
- Backfill and mutate behavior:
  - Implement `tvx.backfill` to scan Codex sessions and mirror missing state into Tinyvex in bounded chunks. Leave heavy backfills behind an explicit control.

P1 – Performance and robustness
- SQLite configuration:
  - Use a shared `Connection` (or a small pool) with PRAGMAs: `journal_mode=WAL`, `synchronous=NORMAL`, `busy_timeout=5000`, and `temp_store=memory`. Set once per DB open.
  - Group streaming inserts in lightweight transactions (e.g., finalize after N deltas or T milliseconds) where safe.
- Move “since” filters into Tinyvex:
  - Add `list_threads_since(updated_after, limit)` and `list_messages_since(thread_id, after_seq|after_ts, limit)` SQL to avoid post-filtering in the WS layer.
- Message count strategy:
  - Drop `messageCount` from `list_threads` or compute approximately (e.g., last-50 only) to avoid per-row subqueries. If required, consider a maintained counter updated via triggers or writer-side increments.

P2 – Completeness and UX
- Two-way writer coverage:
  - Map tool calls, plan, and state to Codex-compatible (or documented) JSONL. Only enable upon opt-in and clearly document limitations.
- Thread metadata and titles:
  - Derive `title` from first user line or project context to improve drawer readability.
- Tool-call detail hydration:
  - Add `toolCall.get` or extend `toolCalls.list` with full content for detail screens.
- ACP events retention:
  - Either make `acp_events` queriable (with a simple log viewer) or add a pruning policy.
- App diagnostics:
  - Include per-provider watcher stats and last error in Settings; throttle `sync.status` polling.

P3 – Tests and docs
- Add integration tests for:
  - Watcher offset persistence and truncate/rotate recovery.
  - Two-way writer idempotency and concurrency under rapid updates.
  - WS `sync.status` formatting and performance under large file trees.
- Expand Tinyvex docs with:
  - Backfill semantics.
  - Two-way writer format and locations.
  - Alias persistence design.

---

## Notable Code References

- Tinyvex schema and API: crates/tinyvex/src/lib.rs
- Streaming writer: crates/tinyvex/src/writer.rs
- Bridge WS: crates/oa-bridge/src/ws.rs
- Writer adapter: crates/oa-bridge/src/tinyvex_write.rs
- Sessions watcher: crates/oa-bridge/src/watchers/sessions_watch.rs
- ACP translator: crates/acp-event-translator/src/lib.rs
- App provider: expo/providers/tinyvex.tsx
- Settings toggles: expo/app/settings/index.tsx
- Tinyvex docs: docs/tinyvex/tinyvex.md, docs/tinyvex/ws-bootstrap.md

---

## What’s Unimplemented or Placeholder

- `tvx.mutate` and `tvx.backfill` controls are stubs and log `tinyvex.todo`.
- Two-way writer does not yet serialize tool calls/plan/state; only user/assistant/thought text are emitted.
- No persisted alias table; aliasing is maintained in memory and inferred in the app as a workaround.
- No DB-level “since” queries; filtering is done in the WS layer post-query.
- `acp_events` is not queried anywhere; it grows without retention.

---

## Conclusion

The Tinyvex foundation is solid and meets the day-to-day needs of the mobile app while delivering the main objectives of Issue 1351 for Codex inbound sync. The remaining work focuses on durability (alias persistence), performance (SQLite config and query shaping), and completeness (two-way coverage, backfill/mutate), all of which can be delivered incrementally without breaking the public WS contract. Addressing P0/P1 items will make the system robust for heavier local usage and future provider integrations.

---

## Suggested Next-Milestone Checklist

- [ ] Persist `sessionId ↔ clientThreadDocId` in Tinyvex and use it in watcher/writer.
- [ ] Serialize two-way writes per thread; document behavior and location.
- [ ] Implement `tvx.backfill` with bounded scanning; keep `tvx.mutate` minimal or remove if unnecessary.
- [ ] Configure SQLite (WAL, busy_timeout, synchronous) and adopt a shared connection.
- [ ] Add DB-side `list_*_since` queries; reduce per-row subqueries in `list_threads`.
- [ ] Expand two-way writer to map tool calls, plan, and state (behind toggle).
- [ ] Improve thread titles from content/project context.
- [ ] Add Settings diagnostics for last sync error and per-provider stats.
- [ ] Write integration tests for watcher offsets, two-way concurrency, and `sync.status` formatting.


## Addendum — Implemented Immediately (2025-10-31)

Following this audit, the following hardening and de‑spaghetti changes were implemented and shipped:

- Bridge bootstrap and hydration
  - `threadsAndTails.list` now falls back to Codex history when Tinyvex has fewer threads than requested, synthesizing recent threads and lightweight tails so the drawer shows a complete history after a cold start. (crates/oa-bridge/src/ws.rs)
  - `messages.list` now backfills on demand from the rollout JSONL when the DB has zero rows for a thread, mirrors ACP updates to Tinyvex, then returns the hydrated last messages. (crates/oa-bridge/src/ws.rs)
- UUID/session id extraction
  - Consolidated the filename UUID extractor into `util` and reused it across watcher and WS code; `history::resolve_session_path` accepts UUID substring matches. (crates/oa-bridge/src/util.rs, crates/oa-bridge/src/history.rs)
- SQLite defaults
  - Tinyvex initialization applies `journal_mode=WAL`, `synchronous=NORMAL`, and a `busy_timeout` for better durability/perf tradeoffs and fewer transient lock errors. (crates/tinyvex/src/lib.rs)
- App-side resiliency (already OTA’d)
  - When rendering thread timelines, the provider subscribes/queries by both client thread id and `resume_id` and mirrors snapshots to avoid stale timelines during bridge restarts. Drawer synthesis also occurs client‑side as an additional guard. (expo/providers/tinyvex.tsx)

Planned next: persist `sessionId ↔ clientThreadDocId` in Tinyvex to remove dual‑ID plumbing, DB‑side “since” queries, and a minimal `tvx.backfill` control.
