ACP Events: Performance & UX Optimizations
=========================================

This note outlines optimizations we can implement after the APIs and components stabilize. It focuses on end‑to‑end flow: ingestion → storage → queries → UI.

Goals
-----

- Fast, predictable reads for recent activity and session history
- Minimal recompute on hot paths (no per‑render collapsing work)
- Smooth UI for long streams (virtualized, paginated, real‑time)
- Clear privacy/retention posture and safe evolution over time

Known Bottlenecks (today)
-------------------------

- Collapsing streamed chunks (agent_message_chunk) at read time scans N events per turn.
- Queries fetch full payloads even when only summary fields are needed.
- Fixed window (take 1000) is a heuristic and can miss long turns.
- UI renders all rows without virtualization and can re‑compute on each re‑render.

Server‑Side Aggregation & Materialization
----------------------------------------

- Append‑time materialization (recommended)
  - Add `acp_messages` table with rows keyed by `{sessionId, turnSeq, role}` and fields `{text, firstTs, lastTs, tokenCount, ...}`.
  - On `agent_message_chunk`, upsert and append text; on boundary (first non‑chunk update of next turn or `prompt.result`), finalize.
  - Result: reads become O(1) for messages; no client‑side collapsing.
- Turn sequencing
  - Track `turnSeq` per session: increment at new prompt or at end of previous agent message.
  - Store `role: 'assistant' | 'user'` so we can render full dialogs quickly.
- Idempotency
  - Use `{sessionId, turnSeq}` unique index; append uses `update` not `insert` to avoid dupes.
- Denormalize helpful fields onto `acp_messages`: `modelId`, `modeId`, `toolCallIds[]`, `vendorMeta` summary.

Schema & Indexing
-----------------

- Normalize on write
  - Promote `name` (event family), `updateType`, `toolCallId`, `sessionId` to top‑level fields in `acp_events`.
  - Keep `payload: any` for lossless storage.
- Indexes
  - `by_session_time(sessionId, createdAtTs)` for timelines
  - `by_name(name)`, `by_updateType(updateType)`, `by_tool_call(toolCallId)`
  - Optional: `by_user(userId)`, `by_user_session(userId, sessionId)`
- Projection
  - Prefer queries that return only required fields (e.g., `{ts, name, updateType, toolCallId}`) to reduce bandwidth and GC.

Query/API Shape
---------------

- Paginated endpoints
  - `recentRows({ cursor, limit }): { items, nextCursor }` to avoid fixed windows.
  - `sessionRows({ sessionId, cursor, limit })` for focused views.
- Real‑time
  - Live query on `acp_messages` (and `acp_events` if needed) for reactive updates without polling.
- Delta streaming
  - Optional endpoint that returns “since ts/ordinal” to append to an existing client buffer.

UI Performance & UX
-------------------

- Virtualized table
  - Use `react-window` for thousands of rows with sticky header.
- Expand/collapse
  - Default to a compact row; expand to show full message (pre‑wrap) and metadata.
- Copy/share
  - “Copy message” and “Copy JSON” actions per row.
- Safe wrapping
  - `white-space: pre-wrap; word-break: break-word;` + ellipsis for non‑message columns.
- Loading states
  - Skeleton rows and optimistic appends for a smoother feel.

Robust Message Collapsing
-------------------------

- Content union handling
  - Collect text from single text content and arrays of content blocks; ignore non‑text or represent as `[image]`, `[audio]` markers.
- Interleaving
  - If tools interleave with chunks, treat any non‑chunk as a boundary; consider summarizing partials.
- Whitespace & blanks
  - Trim leading empty chunks; coalesce multiple spaces if desirable.

Storage, Retention, Privacy
---------------------------

- Retention
  - TTL or periodic compaction for `acp_events` (keep lossless for X days) while `acp_messages` persists longer.
- Redaction
  - Optional transform to redact secrets/PII on write; store a redacted payload alongside raw if needed.
- Multi‑tenant isolation
  - Ensure `userId`/auth constraints in queries, add per‑tenant indexes if needed.

Observability & Testing
-----------------------

- Metrics
  - Track event write QPS, message build latency, chunk sizes, row count per session.
- Backfills
  - Script to backfill `acp_messages` from historical `acp_events`.
- Property tests
  - Given a stream of mixed events, collapsed output is invariant under chunk partitioning.

Suggested Rollout Plan
----------------------

1. Normalize `acp_events` (top‑level `name`, `updateType`, `toolCallId`, indexes).
2. Add `acp_messages` and append‑time materialization (assistant + user turns).
3. Replace `recentRows` with paginated endpoints backed by `acp_messages`.
4. Add virtualization and expand/collapse UI.
5. Introduce retention/compaction and (optional) redaction.

Open Questions
--------------

- Turn boundaries: prefer spec‑driven (e.g., `prompt.result`) vs heuristic (gap or non‑chunk)?
- Do we want to persist vendor‑specific fields (e.g., `_meta.claudeCode`) verbatim in `acp_messages`?
- How much history should the “recent” view load by default (time vs count)?

—
We can implement step (1) with minimal risk; steps (2‑3) provide the biggest perf win and simplify the client.
