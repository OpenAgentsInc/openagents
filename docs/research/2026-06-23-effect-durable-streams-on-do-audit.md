# Effect-native Durable Streams on Cloudflare Durable Objects — grounded audit

**Date:** 2026-06-23
**Status:** research / decision-support (no code change)
**Author:** OpenAgents research
**Scope:** Should we build/adopt an Effect-native port of ElectricSQL's
Durable Streams on Cloudflare Durable Objects, and where in *our* stack would
it actually help?

---

## 0. TL;DR

- **What it is.** Durable Streams (ElectricSQL, beta Dec 2025) is a minimal
  **HTTP-based protocol** for *append-only, offset-addressed, resumable* byte
  streams, with catch-up reads, live tailing (long-poll + SSE), explicit EOF
  (close), Kafka-style **idempotent producers** (exactly-once writes), forking,
  and durable subscriptions. Its real spec is a **~1,560-line `PROTOCOL.md`**
  plus a **language-agnostic conformance test suite (~320 cases)**. The tweet
  describes pointing an agent at the Cloudflare DO/Workers docs + the
  durable-streams codebase + those conformance tests and iterating to "make the
  tests pass" — a clean conformance-driven port.
- **The methodology is sound and reusable.** A frozen protocol doc + an
  executable, language-agnostic conformance suite is exactly the kind of
  "narrow contract, bounded state space, run the checker, convert
  counterexamples to regressions" loop our workspace already favors. The
  conformance suite *is* the spec; an agent can drive a green-to-spec port
  against it. We should keep this pattern in our toolbox.
- **Where it helps US (ranked):**
  1. **Resumable Khala inference streaming** — strongest fit. Today our SSE
     pass-through has **no persistence and no resume**; a disconnect loses
     in-flight tokens and the work is gone. This is the protocol's flagship use
     case. **Port the Durable-Proxy idea** (persist upstream → durable read URL
     → resume by offset). *Adopt-the-idea, build small.*
  2. **Verse world transport / interest fan-out** — partial fit. We already
     have a cursor/delta/snapshot/sequence model that *is* a resumable
     delta-stream — but our DO persists only the transport **clock**
     (`minReplaySeq`/`currentSeq`), **not the delta payloads**, so "resume"
     today means heartbeat + fresh snapshot, never true gap replay. Durable
     Streams' offset-addressed log is the missing replay buffer. **Port the
     offset-log idea behind our existing WebSocket transport; do not swap
     transports.** *Port, scoped.*
  3. **Batch-job + nostr-relay event feeds** — weak/medium fit. Mostly already
     covered by Queues / NIP-01 `REQ`+EOSE. *Mostly skip; one narrow port.*
- **Don't vendor.** Per the workspace read-only-reference rule
  (`projects/repos/*` is study-only), we **port the protocol ideas into our
  owned Effect/Schema code**, we do not vendor the TypeScript packages wholesale.

---

## 1. What Durable Streams is (the spec, with repo pointers)

Reference clone (read-only): `projects/repos/durable-streams/`.

### 1.1 Core model (`PROTOCOL.md`)

A **stream is a URL.** It is an append-only, immutable-by-position byte
sequence. The protocol is defined purely by HTTP methods + query params +
custom headers on that URL (`PROTOCOL.md` §3–§4):

- **Create** — `PUT {stream-url}` (idempotent; `200` if matching config, `409`
  if config differs); optional `Content-Type`, `Stream-TTL`,
  `Stream-Expires-At`, atomic `Stream-Closed: true`.
  (`PROTOCOL.md` §5.1, lines ~222–289)
- **Append** — `POST {stream-url}`, full-body or chunked; optional
  `Stream-Seq` (monotonic lexicographic writer sequence) and atomic
  `Stream-Closed: true`. (`PROTOCOL.md` §5.2, lines ~290–360)
- **Close** — `POST … Stream-Closed: true`, empty body → terminal, durable,
  monotonic, idempotent EOF. (`PROTOCOL.md` §5.3, §4.1)
- **Delete / Head / Metadata** — `DELETE`, `HEAD` for tail offset + TTL +
  closure status. (`PROTOCOL.md` §5.4–§5.5)
- **Read — three modes** (`PROTOCOL.md` §5.6–§5.8):
  - **Catch-up:** `GET ?offset=<offset>` → bytes from offset, with
    `Stream-Next-Offset`, `Stream-Up-To-Date: true` when caught up, `ETag`,
    CDN `Cache-Control`.
  - **Long-poll:** `GET ?offset=…&live=long-poll&cursor=…` → `200` with new
    data, or `204` on timeout (with `Stream-Up-To-Date`, `Stream-Cursor`).
  - **SSE:** `GET ?offset=…&live=sse` → `data` events + a `control` event
    carrying `streamNextOffset` / `streamCursor` / `upToDate` / `streamClosed`.
    Binary streams are base64-encoded with `stream-sse-data-encoding: base64`.

### 1.2 Idempotent producers — exactly-once writes (`PROTOCOL.md` §5.2.1)

A two-layer sequence design: transport `Producer-Id` + `Producer-Epoch` +
`Producer-Seq` (retry/dedup safety, zombie fencing) and application `Stream-Seq`
(cross-restart ordering). Servers **MUST** serialize validate+append per
`(stream, producerId)` and **SHOULD** commit producer-state + log atomically.
This is the "exactly-once-ish replay" guarantee.

### 1.3 Forking (`PROTOCOL.md` §4.2, `docs/fork.md`)

`PUT` with `Stream-Forked-From` + `Stream-Fork-Offset` creates a stream that
inherits a source's data up to an offset without copying; reads transparently
stitch source+fork. Soft-delete / reference-count GC handles shared history.
Useful for "branch a conversation/session from history."

### 1.4 Subscriptions (`PROTOCOL.md` §6–§7)

Durable cursors under a reserved `__ds/subscriptions/:id` namespace, delivered
by signed webhook **or** pull-wake (claim/ack/release with generation fencing +
leases). Glob/explicit stream membership. This is the "wake a worker when N
streams have pending events" primitive.

### 1.5 Higher-level layers (docs)

- **Durable State** (`docs/durable-state.md`, `packages/state`): typed
  `insert`/`update`/`delete` change events + `snapshot`/`reset` control
  messages over a JSON stream → materialize DB-style state.
- **Durable Proxy** (`docs/durable-proxy.md`, `@durable-streams/proxy`):
  forwards to an upstream AI provider, **persists the streaming response** into
  a durable stream, hands the client a **resumable read URL** + a
  `createDurableFetch({ autoResume: true })` client. **This is the resumable-
  inference pattern, off the shelf as a design.**
- StreamDB, StreamFS, TanStack AI / Vercel AI SDK transports.

### 1.6 The conformance contract (the real spec)

`packages/server-conformance-tests` (run via
`npx @durable-streams/server-conformance-tests --run <url>`) and
`packages/client-conformance-tests/test-cases/*.yaml` (~320 declarative cases).
Buckets:

- `consumer/` — catch-up, long-poll, SSE (+ base64), offset resumption &
  monotonicity, message ordering, cache headers, retry-resilience,
  fault-injection, streaming-equivalence (catch-up vs live must agree).
- `producer/` — create, append, batching, sequence ordering, and
  `producer/idempotent/` (epoch management, sequence validation, autoclaim,
  multi-producer, concurrent-requests, zombie fencing).
- `lifecycle/` — stream closure (EOF), lifecycle, dynamic headers.
- `validation/` — input validation.

**Conformance contract in four bullets:**

1. **Offset-addressed replay.** Any read from a stored offset returns the exact
   suffix; offsets are opaque, lexicographically sortable, monotonic. Catch-up
   and live reads must yield identical data (streaming-equivalence).
2. **Resumability + EOF.** Clients resume from `Stream-Next-Offset` after any
   disconnect; `Stream-Up-To-Date` ≠ EOF — only `Stream-Closed` (durable,
   monotonic, idempotent) means "no more data ever."
3. **Exactly-once writes.** `(producerId, epoch, seq)` dedups retries, fences
   zombies, and the validate+append is serialized per `(stream, producerId)`
   and atomic.
4. **CDN-friendly fan-out.** Catch-up reads are cacheable (`ETag`,
   `Cache-Control`, `Stream-Cursor` collapsing) so one origin can fan a stream
   out to many readers; live modes recycle the connection (~60s) for collapse.

Official servers are **Caddy (Go)** and a **Node dev server**. **There is no
Cloudflare DO server in-repo** — which is exactly the gap the tweet's
"Effect fork on DO" fills.

---

## 2. The "Effect fork via conformance tests" methodology

The tweet's recipe: aim an agent at (a) Cloudflare DO/Workers docs,
(b) the durable-streams codebase, (c) the conformance suite, and iterate
`make it pass the conformance tests`.

**Why this works.** The protocol is small, frozen, and — critically —
**executable as a black-box HTTP contract**. The conformance runner just needs a
base URL (`CONFORMANCE_TEST_URL`). So the loop is: stand up a DO-backed server
that speaks the HTTP surface, point `--run http://localhost:port` at it, and
grind reds to green. This is precisely the workspace's preferred shape:
**narrow the production contract, model the bounded state space, run the
checker, convert counterexamples into regressions** (`CLAUDE.md` Invariant
Discipline). The 320 YAML cases are a ready-made regression corpus.

**Is Cloudflare DO a good substrate?** Yes — better than most:

- DO storage is **strongly consistent and serialized per object**, each method
  **implicitly wrapped in a transaction** (CF Storage API docs). That directly
  satisfies the protocol's "serialize validate+append per `(stream,producerId)`"
  and "commit producer-state + log atomically" requirements *for free*, where a
  multi-node Caddy/Postgres server has to work for it.
- **SQLite storage backend** (SQL API + `transactionSync`) is a natural
  append-only log: `INSERT` rows keyed by offset, range-scan from an offset.
- **Alarms** cover `Stream-TTL` / `Stream-Expires-At` sliding-window and
  hard-deadline expiry, and subscription lease timeouts — the same alarm
  pattern our world DO already uses for hot-row TTL.
- **Hibernatable WebSockets** (`acceptWebSocket`, `serializeAttachment`, 16 KB
  attachment cap) keep idle readers connected with the DO evicted — though note
  the protocol's *live* modes are HTTP long-poll/SSE, not WebSocket, so a strict
  port leans on HTTP, with WS as an optional transport.
- **Caveats:** per-object throughput/storage limits make "one DO per stream"
  the natural sharding (each stream = `idFromName(streamPath)`), which is
  idiomatic but means *fan-out across millions of viewers wants the CDN cache
  path*, not the DO itself, in the hot loop. The protocol is explicitly designed
  for that (cacheable catch-up reads), so it composes — but a naive DO-only impl
  would bottleneck on the origin.

**Should WE use this methodology?** Yes, as a **tool**, with judgment:

- For a **net-new owned primitive** (e.g. a `@openagentsinc/durable-stream`
  Effect package), conformance-driven porting is the right way to get a
  spec-faithful, regression-protected core fast — and we already work in
  Effect + Effect Schema, so the typed-Stream/Layer/Schema port is natural.
- For **existing bespoke surfaces** (world transport, inference SSE), do **not**
  rip-and-replace to chase a foreign conformance suite. Instead **extract the
  specific guarantee we're missing** (offset replay buffer; resumable proxy) and
  test it against *our* invariants. The conformance suite is a reference oracle,
  not a mandate to conform our product wire format to Electric's HTTP shape.

---

## 3. Where it helps US (the core — ranked, grounded)

We are Effect + Foldkit + Effect Schema throughout. Below, each item gives:
**(a)** current bespoke approach, **(b)** what durable-streams gives,
**(c)** effort/risk, **(d)** adopt-lib / port-idea / skip.

### Rank 1 — Resumable Khala inference streaming  ⭐ strongest fit

**File:** `apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`
(SSE pass-through; EPIC #6027).

**(a) Current.** `buildIncrementalSseResponse` (`chat-completions-routes.ts`
~lines 506–575) builds a `ReadableStream` that **pumps the upstream provider
stream straight to the client**, frames it as `chat.completion.chunk` SSE, and
settles metering on the **final** chunk. On an upstream fault it closes the SSE
cleanly with `[DONE]` so the socket isn't left hanging. **Critically: nothing is
persisted.** If the *client* disconnects mid-stream (tab suspend, network flap,
laptop sleep) the in-flight tokens are gone and there is **no resume** — the
only recovery is to re-run the (paid, slow) completion. This is exactly the
failure Durable Streams was built to kill ("when the stream fails, the product
fails — even if the model did the right thing").

**(b) What durable-streams gives.** The **Durable Proxy** pattern
(`docs/durable-proxy.md`): persist the upstream token stream into a durable
stream keyed by a stable `requestId`, return a **resumable read URL**; client
reconnects with `?offset=<last>` (or `createDurableFetch({ autoResume: true })`)
and picks up exactly where it left off. `Stream-Closed` is the clean EOF for the
completion; idempotent producers make the *upstream→stream* write retry-safe;
catch-up reads are CDN-cacheable so the same completion can be shared/replayed
to multiple viewers (multi-tab, share-a-run links) without re-billing.

**(c) Effort/risk.** Medium. We already terminate provider SSE in a Worker, so
inserting "tee into a per-request DO log, hand back a durable read URL" is a
contained change. Risk: must keep **metering settlement** authoritative (settle
once on real EOF, never on a resumed/replayed read — replays must not re-bill),
and keep prompts/credentials out of any cacheable surface (our
`INVARIANTS.md`/public-projection rules). Storage cost is bounded by TTL/expiry
(the protocol's native sliding window).

**(d) Verdict: PORT THE IDEA, build small.** Stand up a tiny Effect/DO
durable-stream keyed by `requestId` behind the existing inference route. Do
**not** adopt the npm proxy package wholesale (read-only-reference rule; also it
carries its own auth/allowlist server we don't need). Worth doing — this is the
single highest-leverage, most user-visible win (every dropped chat today is lost
spend).

### Rank 2 — Verse world transport / interest fan-out  ⭐ partial fit, real gap

**Files:** `apps/openagents-world/src/protocol.ts` (619 LoC, Region DO transport
clock + cursor/reconnect), `packages/world-contract/src/index.ts` (719 LoC,
Effect Schema cursors/deltas/snapshots/sequences), `packages/world-client/src/index.ts`
(985 LoC, mirrors snapshots+deltas into a read-only `WorldReadModel`).

**(a) Current.** We already have a **resumable delta-stream-with-cursors**, just
bespoke and WebSocket-bound:

- Cursors are `cursor.<region>.<sequence>`; `cursorForSequence` /
  `sequenceFromCursor` parse them (`protocol.ts` ~246–268).
- Reconnect (`?cursor=…`) resolves to **resume** (heartbeat at current cursor)
  when `minReplaySeq ≤ seq ≤ currentSeq`, else a typed **`cursor` diagnostic +
  fresh snapshot** (`protocol.ts` ~442–469).
- Deltas are sparse (absent fields unchanged), applied by
  `applyDeltaToReadModel` / `applyDeltaToState` in the client
  (`world-client/src/index.ts` ~756–802).
- **The gap:** the Region DO persists only the **transport clock**
  (`region_transport_clock`: `min_replay_seq`, `currentSeq` — `protocol.ts`
  ~138–144, 258), **not the delta payloads**. So even when a reconnect lands
  *inside* the replay window, we do **not** replay the missed deltas — we send a
  heartbeat and rely on the next snapshot. "Resumable" today is really
  "reconnect-safe re-snapshot," not gap-free replay. Snapshots rehydrate from
  the D1 projection cache.

**(b) What durable-streams gives.** The exact missing piece: an
**offset-addressed append-only log of deltas** in DO SQLite, so a reconnect at
offset N replays N→tail precisely (the `consumer/offset-resumption` +
`streaming-equivalence` guarantee), instead of re-snapshotting. Idempotent
producers would make our **service bridge ingest** (`bridge.ts`, replays public
source refs) exactly-once and de-dupe replays natively (we currently de-dupe by
`row.kind + worldRowKey(row)` key overwrite). `Stream-TTL` maps onto our hot-row
alarm expiry. Forking could branch a region's history for replay/debug.

**(c) Effort/risk.** Medium-high if done as a transport swap; **low-medium if
done as a scoped port.** Our wire format is WebSocket JSON envelopes
(`snapshot`/`delta`/`diagnostic`) with WoC interest tiers, moderation, command
receipts, sight-policy pruning — a rich, working contract we do **not** want to
rewrite to Electric's HTTP/SSE byte shape. Interest-scoped fan-out (per-session
subscription plans, near/far tiers, first-sight-full vs continued-lite) is
*more* than a flat stream and has no clean durable-streams analogue.

**(d) Verdict: PORT THE OFFSET-LOG IDEA behind the existing transport; do NOT
adopt the lib or swap transports.** Specifically: add a bounded per-region
**delta replay buffer** in the Region DO SQLite (offset = our existing
`WorldSequence`), and on in-window reconnect replay buffered deltas instead of
heartbeat-only. This closes the real correctness gap (silent state divergence
between snapshot points) while keeping our WoC interest model, Schema contract,
and command-receipt semantics intact. The Durable Streams conformance cases for
offset-resumption / streaming-equivalence are a useful **oracle** to test our
replay buffer against. *This is worth doing for the Verse world transport — but
as an additive replay buffer, not a re-platforming.*

### Rank 3 — Batch-job consumer & nostr-relay feeds  — weak/medium fit

**Files:** `apps/openagents.com/workers/api/src/inference/batch-job-routes.ts`
(#6028, Queue/DO consumer), `apps/nostr-relay/src/*`.

**(a) Current.** Batch jobs ride **Cloudflare Queues** + a DO consumer — Queues
already give durability, retry, and ack semantics. The nostr-relay speaks
**NIP-01**: `REQ` with filters, stored events, `EOSE` (end-of-stored-events),
then live. NIP-01 *is* a domain-specific durable/resumable event protocol with
its own conformance expectations (NIPs); since-cursors give resume.

**(b) What durable-streams gives.** For batch jobs: marginal — a durable result
stream per job (resume reading job output/progress) is the only real add, and
overlaps Rank 1 if jobs are LLM completions. For nostr: nothing we should adopt
— replacing NIP-01 semantics with Electric's wire shape would break protocol
compatibility for zero benefit; the *idea* of an offset replay buffer is already
expressed natively as `since`/`EOSE`.

**(c) Effort/risk.** Low value, low-medium effort.

**(d) Verdict: MOSTLY SKIP.** One narrow, optional port: if batch jobs stream
long-running **progress/output** that a client tails and can disconnect from,
reuse the Rank-1 durable-stream primitive for the job's output stream. Do **not**
touch nostr-relay — NIP-01 already owns that contract.

---

## 4. Don't-bother / mismatch notes (honest)

- **Don't swap the world WebSocket transport for HTTP long-poll/SSE.** Our
  transport carries interest tiers, command receipts, moderation, sight-policy
  pruning, and presence — a flat offset byte-stream is a downgrade. We want the
  *replay buffer*, not the wire format.
- **Don't conform our public wire formats to Electric's HTTP shape.** The
  conformance suite is an oracle for a *new internal primitive*, not a mandate
  to reshape `world-contract` or the OpenAI-compatible SSE our clients expect.
- **Don't adopt the npm packages into the build.** `projects/repos/*` is
  read-only reference; the proxy package additionally bundles its own
  auth/allowlist server we don't need and a JWT model that isn't ours. Port the
  ~3 ideas we want (offset replay, resumable proxy, idempotent producer) into
  owned Effect/Schema code.
- **DO-only fan-out is a trap at scale.** The protocol's cheap fan-out comes
  from the **CDN cache path**, not the DO. A naive "one DO serves all readers"
  port loses that. For the world, our interest-scoped per-region DO model is the
  right sharding; for inference, lean on cacheable catch-up reads for
  share/replay.
- **Metering must not double-bill on replay.** Any resumable-inference work
  must keep settlement on the single real EOF; replays/catch-up reads are free.
- **Queues already cover batch durability.** No need to reinvent it as a stream.

---

## 5. Recommendation

1. **Build a small owned Effect primitive** — call it
   `@openagentsinc/durable-stream` (Effect `Stream`/`Layer`/`Schema`, DO SQLite
   log, alarm-driven TTL, idempotent-producer headers). Develop it
   **conformance-driven**: stand up the DO server, point the upstream
   `server-conformance-tests --run` at it as a reference oracle, and additionally
   pin *our* invariants. This is the reusable substrate for Rank 1 and the
   buffer in Rank 2.
2. **Rank 1 first** (resumable inference) — highest user-visible leverage; every
   dropped chat today is lost paid work.
3. **Rank 2 as an additive replay buffer** in the Region DO — closes a real
   correctness gap (state divergence between snapshots) without re-platforming
   the world transport. *Yes, worth doing for the Verse world transport.*
4. **Skip Rank 3** except reusing the Rank-1 primitive for batch-job output
   streams; leave nostr-relay (NIP-01) alone.
5. **Keep the conformance-driven-port methodology** in the toolbox for future
   net-new protocol primitives.

### Repo pointers (read-only reference)

- Protocol: `projects/repos/durable-streams/PROTOCOL.md`
- Conformance: `projects/repos/durable-streams/packages/{server,client}-conformance-tests/`
- AI resume pattern: `projects/repos/durable-streams/docs/durable-proxy.md`
- Typed state layer: `projects/repos/durable-streams/docs/durable-state.md`,
  `packages/state/`
- CF Agents SDK (DO state-sync/broadcast family, higher-level than an offset
  log): `projects/repos/agents/`

### Our stack pointers

- Inference SSE pass-through: `apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`
- Batch jobs: `apps/openagents.com/workers/api/src/inference/batch-job-routes.ts`
- World transport / cursor / reconnect: `apps/openagents-world/src/protocol.ts`
- World contract (cursors/deltas/snapshots): `packages/world-contract/src/index.ts`
- World client (delta application): `packages/world-client/src/index.ts`
- World bridge ingest (dedup by row key): `apps/openagents-world/src/bridge.ts`
- Nostr relay: `apps/nostr-relay/src/`
