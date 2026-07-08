# Durable Stream integration roadmap — owned `@openagentsinc/durable-stream` on Cloudflare DO

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


**Date:** 2026-06-23
**Status:** roadmap / phased plan (derived from the audit)
**Author:** OpenAgents engineering
**Depends on:** `docs/research/2026-06-23-effect-durable-streams-on-do-audit.md`
(commit `34c9204c34`)
**Owner directive:** "build one small owned `@openagentsinc/durable-stream`
Effect primitive (DO SQLite log, conformance suite as the test oracle), Rank-1
(resumable inference) first; port ideas, don't vendor."

---

## 0. Summary

This roadmap turns the audit's recommendation into a sequenced, dependency-aware
delivery plan. We build **one owned Effect + Effect Schema durable append-only
offset-log primitive** on a Cloudflare Durable Object with SQLite storage, develop
it **conformance-driven** (the upstream ElectricSQL Durable Streams protocol + its
~320-case conformance suite as a read-only oracle), and then integrate it in
ranked order:

1. **Phase A — the owned primitive** `@openagentsinc/durable-stream`
   (offset-addressed replay, resumability + EOF, exactly-once writes, CDN-friendly
   fan-out). *This is the substrate for everything below.*
2. **Phase B — Rank-1 resumable Khala inference** (durable-proxy: persist upstream
   tokens → durable read URL → resume by offset). *Highest user-visible leverage.*
3. **Phase C — Rank-2 Verse-world bounded delta-replay buffer** (additive replay
   buffer behind the existing WebSocket transport, keyed on `WorldSequence`).
4. **Skips** — batch-job result streams (reuse Phase A only if it lands naturally)
   and nostr-relay (NIP-01 already owns that contract).

**Port ideas, do not vendor.** `projects/repos/durable-streams/*` is read-only
reference (workspace rule). We re-implement the ~4 guarantees we want in owned
Effect/Schema code; we do not pull the npm packages into the build.

---

## 1. The conformance contract (the spec we port to)

From the audit and `projects/repos/durable-streams/PROTOCOL.md` (§4–§10) +
`packages/{server,client}-conformance-tests`, the primitive must satisfy four
guarantees. These are the acceptance contract for Phase A.

1. **Offset-addressed replay.** Any read from a stored offset returns the exact
   suffix. Offsets are opaque, lexicographically sortable, monotonic, unique,
   strictly increasing (`PROTOCOL.md` §8). Catch-up and live reads must yield
   identical data (**streaming-equivalence**). Sentinels `-1` (start) and `now`
   (tail) are reserved and must never be minted as real offsets.
2. **Resumability + EOF.** Clients resume from `Stream-Next-Offset` after any
   disconnect. `Stream-Up-To-Date: true` ≠ EOF — only `Stream-Closed: true`
   (durable, monotonic, idempotent) means "no more data ever" (`PROTOCOL.md`
   §4.1, §5.6). EOF is signalled three ways by mode: catch-up `200` empty body +
   `Stream-Closed`; long-poll `204` + `Stream-Closed`; SSE `control` event with
   `streamClosed: true` then connection close.
3. **Exactly-once writes.** `(producerId, epoch, seq)` dedups retries, fences
   zombies (stale epoch → `403` + current `Producer-Epoch`), detects gaps
   (`409` + `Producer-Expected-Seq`/`Producer-Received-Seq`). Validate+append is
   serialized per `(stream, producerId)` and committed atomically with the log
   (`PROTOCOL.md` §5.2.1). DO single-threaded serialization + `transactionSync`
   give us serialization and atomicity *for free* — the audit's key reason DO is
   a good substrate.
4. **CDN-friendly fan-out.** Catch-up reads are cacheable (`ETag` that varies
   with closure status, `Cache-Control`, `Stream-Cursor` collapsing); live modes
   recycle the connection (~60s) for collapse (`PROTOCOL.md` §10). At scale,
   fan-out belongs on the CDN cache path, **not** the DO hot loop.

### Conformance suite as the oracle

- Upstream server suite (`@durable-streams/server-conformance-tests --run <url>`)
  drives raw HTTP at `{baseUrl}/v1/stream/{path}` and is the *external* oracle.
  It is a vitest harness that pulls upstream npm packages we are not allowed to
  vendor, so we run it as a **reference oracle where feasible** and otherwise
  **replicate its YAML cases as owned Bun/Effect tests** (the task explicitly
  permits "and/or replicate the YAML cases as Bun/Effect tests").
- The ~320 declarative YAML cases under
  `packages/client-conformance-tests/test-cases/{consumer,producer,lifecycle,validation}/`
  are the regression corpus. Buckets we port first (Phase A): `consumer/`
  (catch-up, offset-resumption, message-ordering, streaming-equivalence,
  cache-headers), `producer/` + `producer/idempotent/`, `lifecycle/`
  (stream-closure, lifecycle), `validation/`.
- We **do not** chase `subscriptions`, `forking`, or webhook delivery for Phase A
  (out of scope below).

---

## 2. Phase A — `@openagentsinc/durable-stream` core primitive

**Goal:** a spec-faithful, regression-protected owned Effect/Schema durable
offset-log on a DO with SQLite storage. Reusable substrate for Phases B and C.

### A.1 Package shape

- New workspace package `packages/durable-stream` →
  `@openagentsinc/durable-stream` (matches `world-contract`/`world-client`
  conventions: `"type": "module"`, `exports: { ".": "./src/index.ts" }`,
  `effect: catalog:`, `bun test`, `tsc --noEmit` typecheck).
- Pure, transport-agnostic core (Effect + Effect Schema): offset codec, request
  model, response model, the append/read/close state machine. **No Cloudflare
  import in the core** — DO bindings live behind a thin storage port so the core
  is unit-testable under Bun without `workerd`.
- A `DurableObject`-class adapter (`DurableStreamObject`) that wires the core to
  `ctx.storage.sql` (SQLite append-only log) + alarms (TTL/expiry). A minimal
  Worker entry (`/v1/stream/*` router) for the conformance harness.

### A.2 Storage model (DO SQLite)

- `stream_meta(stream_id PK, content_type, ttl_seconds, expires_at, closed,
  created_at, ...)` — one row; closure is monotonic.
- `stream_log(offset PK, seq, byte_len, body BLOB, content_kind, created_at)` —
  append-only; offset is a zero-padded lexicographically sortable token
  (`<padded_global_seq>_<padded_byte_pos>`), never `-1`/`now`.
- `producer_state(producer_id PK, epoch, last_seq, last_offset, updated_at)` —
  idempotent-producer fencing; committed atomically with the log row in one
  `transactionSync`.
- One DO per stream (`idFromName(streamPath)`) — idiomatic sharding; fan-out at
  scale leans on the CDN cache path, not one DO serving all readers.

### A.3 Conformance scope for Phase A (in vs out)

**In (the four-part contract):**

- Create (`PUT`): idempotent 200/201/409 incl. closure-status matching;
  `Content-Type`, `Stream-TTL`/`Stream-Expires-At` (reject both → 400),
  create-and-close.
- Append (`POST`): full-body; `Stream-Seq` monotonic (lexicographic, 409 on
  regression); `Stream-Closed: true` atomic append-and-close; empty-body rules.
- Idempotent producers: epoch validation, seq dedup/gap, zombie fencing,
  per-`(stream,producerId)` serialization + atomic commit, close-with-producer.
- Close (`POST Stream-Closed: true`): durable, monotonic, idempotent EOF.
- Delete (`DELETE`) + metadata (`HEAD`: tail offset, TTL, closure).
- Read catch-up (`GET ?offset=`): exact suffix, `Stream-Next-Offset`,
  `Stream-Up-To-Date`, `Stream-Closed`, `ETag` (varies w/ closure), sentinels
  `-1`/`now`.
- Read live long-poll (`?live=long-poll`): 200/204 timeout, `Stream-Cursor`,
  no-wait on closed-at-tail.
- Read live SSE (`?live=sse`): `data`/`control` events, base64 for binary,
  `streamClosed` + close.
- Streaming-equivalence (catch-up == live), offset-resumption, message-ordering,
  cache-headers, input-validation.

**Out (explicit Phase-A non-goals — track separately if ever needed):**

- **Forking** (`Stream-Forked-From`, sub-offsets, soft-delete/refcount GC) —
  audit lists it as "could branch history"; not required for Ranks 1–3.
- **Subscriptions** (`__ds/subscriptions`, webhooks, pull-wake, leases) — the
  audit's Rank-3 "wake a worker" primitive; Queues already cover our batch path.
- **Retention/compaction** (`410 Gone` before earliest retained) — TTL/expiry
  only for v1.
- **JSON-mode array flattening** — port only if Phase B/C wire format needs it;
  default content types are byte/`text/*`/SSE.

### A.4 Test oracle wiring

1. Stand up the DO server locally (`wrangler dev` / Miniflare / `workerd`) on a
   port; export `CONFORMANCE_TEST_URL`.
2. Where the upstream `server-conformance-tests --run` harness runs without
   vendoring, use it as the external oracle and record pass/fail.
3. Replicate the YAML cases (the four-part contract buckets) as owned Bun/Effect
   tests driving raw `fetch` against the local server — these ship in-package and
   gate CI. Convert any counterexample into a regression (workspace invariant
   discipline).
4. **Report pass/fail honestly per bucket.** Do not claim full conformance if
   partial.

**Effort:** M (largest single phase). **Risk:** Medium — offset monotonicity,
SSE framing, and idempotent-producer fencing are the fiddly parts; DO
serialization de-risks the concurrency requirements.

**Exit criteria:** package builds + typechecks; the owned conformance subset runs
green; honest pass/fail report for the four-part contract; **not wired into any
app** (disjoint from `workers/api`).

---

## 3. Phase B — Rank-1: resumable Khala inference (durable proxy)

**Goal:** every dropped chat stops being lost paid work. Persist the upstream
provider token stream into a per-request durable stream, hand the client a
resumable read URL, resume by offset on reconnect.

**Target:**
`apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`
(`buildIncrementalSseResponse`, the SSE pass-through; EPIC #6017 / closed #6027
lineage).

**Approach (port the durable-proxy idea, build small):**

- Key a `@openagentsinc/durable-stream` by a stable `requestId`.
- Tee the upstream provider SSE into the durable stream (idempotent producer =
  retry-safe upstream→stream write); frame as today's `chat.completion.chunk`.
- Return a durable read URL; client reconnects `?offset=<last>` (or an
  `autoResume` fetch wrapper) and resumes exactly. `Stream-Closed` = the
  completion's clean EOF.

**Hard requirement — metering exactly-once (the audit's risk):** settlement
**must** fire once on the single real EOF, **never** on a resumed/replayed/
catch-up read. Replays and CDN cache hits are free. Prompts/credentials must stay
out of any cacheable surface (public-projection / `INVARIANTS.md`). Storage
bounded by TTL/expiry.

**⚠️ Collision / gating:** the inference route is owned by the **in-flight
`workers/api` (M4) lane**. **Phase B is deferred to its own issue and must not be
started until that lane is clear** to avoid stepping on concurrent work. Phase A
ships fully independently of it.

**Effort:** M. **Risk:** Medium (metering double-bill, credential leakage into
cache) — both are explicit acceptance gates on the Phase-B issue.
**Depends on:** Phase A.

---

## 4. Phase C — Rank-2: Verse-world bounded delta-replay buffer

**Goal:** close the real correctness gap where world state can silently diverge
between snapshot points. Today the Region DO persists only the transport **clock**
(`min_replay_seq`/`current_seq`), not the delta payloads, so an in-window
reconnect sends a heartbeat + relies on the next snapshot — never gap-free replay.

**Targets:** `apps/openagents-world/src/protocol.ts` (Region DO transport clock,
cursor/reconnect), `packages/world-contract/src/index.ts` (cursors/deltas/
snapshots/sequences), `packages/world-client/src/index.ts` (delta application).

**Approach (additive replay buffer, NOT a transport swap):**

- Add a bounded per-region **delta replay buffer** in the Region DO SQLite,
  offset = the existing `WorldSequence` (reuse Phase A's offset-log idea, not its
  HTTP wire format).
- On in-window reconnect (`minReplaySeq ≤ seq ≤ currentSeq`), replay buffered
  deltas instead of heartbeat-only.
- **Do NOT reshape `world-contract`.** Keep the WebSocket JSON envelopes, WoC
  interest tiers, moderation, command receipts, sight-policy pruning. The Durable
  Streams offset-resumption / streaming-equivalence cases are an **oracle** to
  test the buffer against, not a mandate to conform the wire format.

**Effort:** M (low-medium as a scoped port; high if mistakenly done as a
re-platforming — do not). **Risk:** Low-medium. **Depends on:** Phase A (offset-log
idea); independent of Phase B.

---

## 5. Skips (explicit)

- **Batch-job result streams** — Queues already give durability/retry/ack. Only
  reuse the Phase-A primitive for a job's tailable progress/output stream *if it
  lands naturally* once Phase A exists. No standalone work.
- **nostr-relay** — NIP-01 (`REQ`/`EOSE`/`since`) already *is* a resumable event
  protocol. Replacing it with Electric's wire shape breaks compatibility for zero
  benefit. **Leave it alone.**
- **Forking, subscriptions, retention/compaction** — out of the owned primitive's
  v1 scope (§A.3). Track separately only if a future consumer needs them.

---

## 6. Sequence, dependencies, effort/risk

| Phase | What | Depends on | Effort | Risk | Collision |
|------|------|-----------|--------|------|-----------|
| A | `@openagentsinc/durable-stream` core (4-part contract, conformance oracle) | — | M | Med | none (new package) |
| B | Rank-1 resumable Khala inference (durable proxy) | A | M | Med | **`workers/api` M4 lane — gated** |
| C | Rank-2 Verse-world delta-replay buffer (additive) | A | M | Low-Med | none (additive, no contract reshape) |
| — | Skips: batch streams (opportunistic), nostr (none) | A | S | Low | — |

**Critical path:** A → (B and C in parallel once A lands and the M4 lane is clear
for B). **This roadmap's build deliverable is Phase A only**; B and C are tracked
as sub-issues and built later.

---

## 7. References

**Read-only upstream reference (port, do not vendor):**

- Protocol: `projects/repos/durable-streams/PROTOCOL.md`
- Conformance: `projects/repos/durable-streams/packages/{server,client}-conformance-tests/`
- Durable Proxy (resumable AI): `projects/repos/durable-streams/docs/durable-proxy.md`

**Our stack:**

- Inference SSE: `apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`
- World transport: `apps/openagents-world/src/protocol.ts`, `apps/openagents-world/src/index.ts`
- World contract/client: `packages/world-contract/src/index.ts`, `packages/world-client/src/index.ts`
- DO SQLite + alarms pattern reference: `apps/openagents-world/src/index.ts`

**Source audit:** `docs/research/2026-06-23-effect-durable-streams-on-do-audit.md`
(commit `34c9204c34`).
