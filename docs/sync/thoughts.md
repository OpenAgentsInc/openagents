You’re thinking about this exactly the right way: **don’t clone Convex**, don’t chase protocol compatibility, and don’t big-bang cutover Lightning. Build **your own sync plane** that matches your authority doctrine and your product shape.

Below is a BEAM-centric design that can replace Convex for the **runtime/Codex projection lane first**, while keeping the door open to migrate Lightning later.

---

# OpenAgents Sync Engine (OA Sync) — BEAM-first design

## 0) First principle: what “sync” means in OpenAgents

Convex bundled multiple things. You want to unbundle:

1. **Authority (truth):** runtime event log in Postgres
2. **Projection (read models):** derived documents/tables from events
3. **Subscription delivery:** push updates to clients with resume semantics
4. **Query execution:** read models served efficiently (not arbitrary serverless functions)

Your replacement engine is basically:

> **Projection tables + a subscription bus + a resumable client protocol**, all owned by you.

---

## 1) Target topology

### Runtime (already exists)

* Event log append-only
* Checkpoints & reproject tooling
* Deterministic projector

### New component: OA Sync Service (BEAM)

Runs near runtime (could be inside runtime app or separate app):

* Consumes projection updates from runtime projector
* Writes to Postgres read tables (or receives that they were written)
* Broadcasts updates to subscribers over WebSockets (Phoenix Channels)
* Maintains per-subscription cursor / watermark for resume

**Important:** OA Sync is not a second source of truth. It is **delivery + query** over read models.

---

## 2) Protocol design (proto-first, minimal semantics)

### 2.1 Core idea: “documents + versions + watermarks”

Convex gives reactive queries. For your current needs, you can get 80% of value with:

* A set of named read models (documents) like:

  * `runtime.run_summary:<run_id>`
  * `runtime.codex_worker_summary:<worker_id>`
* A **monotonic version** per document (or per collection)
* A **watermark** for stream resume

### 2.2 Envelope types (proto)

You already listed these; I’d lock a small v1:

* `Subscribe { topics[], resume_after? }`
* `Subscribed { subscription_id, current_watermark }`
* `Update { topic, doc_key, doc_version, payload_bytes, watermark }`
* `Heartbeat { watermark }`
* `Error { code, message, retry_after_ms }`

Where:

* `topic` is a logical stream (e.g. `runtime/run_summaries`, `runtime/worker_summaries`)
* `doc_key` is stable (`run_summary:<id>`)
* `doc_version` increments on change
* `watermark` is a monotonic “stream position” (more below)

Payload format:

* Start with **JSON** for speed, but shape must be proto-defined.
* Upgrade to binary proto later without changing semantics.

---

## 3) Watermarks and resume semantics (the hard part)

You need one thing Convex nails: reconnect and “don’t miss updates.”

### 3.1 Choose a watermark source

Options:

1. **Global stream seq** (best): runtime already has monotonic `seq` per run; you want something similar for each “topic stream.”
2. **DB transaction LSN** (tempting, don’t): ties you to PG internals and makes portability ugly.
3. **Updated_at timestamps** (no): not safe.

**Recommendation:** create a **per-topic monotonic sequence** maintained by your sync service or by DB.

### 3.2 Minimal schema to support it

A durable table:

* `sync_stream_events`

  * `topic` (text)
  * `watermark` (bigint, monotonic per topic)
  * `doc_key` (text)
  * `doc_version` (bigint)
  * `payload` (jsonb or bytea) *optional*
  * `inserted_at`

Two ways to use it:

**Mode A (simple, durable):**

* projector writes doc to read table
* projector (or sync service) appends a `sync_stream_events` row with the new watermark
* websocket service streams from this table
* resume = “give me events after watermark”

**Mode B (lower storage, more ephemeral):**

* don’t persist event rows, only persist `doc_version`
* on reconnect, client calls “since watermark” => server computes diffs from doc versions
* this is harder and eventually you reinvent persisted stream events

**Start with Mode A.** It gives you correct resume with low complexity.

### 3.3 Retention and stale cursor

Keep N hours/days of `sync_stream_events` per topic.
If client resumes before earliest retained watermark → return:

* `410 stale_cursor` equivalent → client must full resync.

You already have the semantics in runtime streams; mirror them.

---

## 4) BEAM implementation choices

### 4.1 Phoenix Channels over raw WS

Use Phoenix Channels:

* solid reconnect/backoff patterns
* multiplex topics
* fits your stack

### 4.2 Broadcasting

Avoid relying purely on `Phoenix.PubSub` for durability; use PubSub for “live push,” but keep DB stream for resume.

Pattern:

* On update: write `sync_stream_events` row, then broadcast `{topic, watermark}` via PubSub.
* Subscribers wake, read rows > last sent watermark, push them.

This avoids message loss issues if the WS node restarts.

### 4.3 Scaling

* Stateless WS nodes are fine because resume is DB-backed.
* Use Postgres indexes `(topic, watermark)`.

---

## 5) Query model (replace Convex queries)

Convex is basically “reactive queries.” You can implement a constrained alternative:

### v1: “document fetch + list endpoints”

* `GET /sync/v1/doc/:doc_key`
* `GET /sync/v1/list/:collection?cursor=...` (optional)
* Subscriptions push doc updates

This matches your actual usage today (run/worker summaries).

### Later: “parameterized views”

If you need “list workers for org,” build explicit read tables + indexed queries:

* `codex_worker_summaries` keyed by `org_id`
* `run_summaries` keyed by `org_id`

Keep query shapes **explicit** and proto-governed. Don’t build arbitrary query execution; that’s how you become a database product.

---

## 6) Auth model (Laravel still front door)

You can keep your current pattern:

* openagents.com mints short-lived JWTs

But you should stop using “Convex-specific claims” and use “OA Sync claims”:

JWT includes:

* `sub` user id
* `org_id`
* `scopes` (read topics)
* `exp` short TTL
* `session_id` for revocation
* maybe `device_id`

OA Sync verifies JWT (HS256 for MVP ok; move to JWKS/RS256 later).

Channel join checks:

* topic allowlist derived from org entitlements

---

## 7) Schema authority: proto-first, generated clients

### Clients you need (v1)

* TypeScript (web + desktop)
* Swift (iOS + macOS)
* Rust (desktop codex)
* Elixir (sync service)
* PHP (optional, control plane helpers)

Generate:

* message types
* enums (error codes, topics)
* validators where possible

Key: **no hand-authored TS schemas** in product logic.

---

## 8) Migration plan (matches your audit’s hybrid recommendation)

### Phase 1 (runtime/Codex lane): dual publish

* Keep runtime projector as-is, but add OA Sync sink alongside Convex sink.
* Dual-write read models:

  * to Convex (as now, if it’s actually enabled)
  * to OA Sync tables + stream events

Run shadow clients:

* Web/mobile subscribes to OA Sync in a feature flag
* Compare parity: counts, lag, drift

### Phase 2: cut clients over

* Remove Convex client dependencies from web/mobile/desktop
* Remove `/api/convex/token` and replace with `/api/sync/token` (or reuse endpoint name but change semantics)

### Phase 3: Lightning ops

* Later, migrate lightning control-plane state into Postgres tables + OA Sync topics
* lightning-ops becomes an HTTP client to openagents APIs or directly to OA Sync where appropriate

---

## 9) Concrete “v1 topics” for OA Sync

Start tiny:

* `runtime.run_summaries` (doc updates for run summary keys)
* `runtime.codex_worker_summaries`
* `runtime.notifications` (optional small feed)

If you later do Enclave:

* `enclave.thread_summaries`
* `enclave.approvals`
* `enclave.spend_ledger`

---

## 10) Critical design constraints (to avoid rebuilding Convex)

* No arbitrary server-side functions in the sync layer.
* No generic reactive query language.
* Only explicit read models and explicit subscription topics.
* DB-backed resume semantics, always.

That’s how you get 90% of Convex value for your use case without becoming a database company.

---

# A BEAM-shaped “MVP spec” for OA Sync (what to build first)

### Server (Elixir)

1. `sync_stream_events` table + indexes
2. `sync_docs` read tables for run_summary / worker_summary (or reuse existing projection tables)
3. `OA.Sync.ProjectorSink` that appends stream events
4. Phoenix Channel `sync:v1`

   * `subscribe(topics, resume_after)`
   * push updates
   * heartbeat
   * stale cursor handling
5. `GET /sync/v1/doc/:key` for initial fetch

### Client (TS)

1. WS client with:

   * connect + auth
   * subscribe
   * resume with watermark
2. cache docs by `doc_key`
3. hook to React to update views

### Verification

* reconnect/resume tests (drop WS mid-stream; ensure no gaps)
* stale cursor test (purge events; ensure client full resync)
* throughput test (burst 10k updates; ensure bounded memory)

---

If you want, next I can turn this into two concrete artifacts:

1. A `proto/openagents/sync/v1/*.proto` set (topics, envelopes, errors).
2. An Elixir implementation outline with the actual tables, indices, channel join checks, and the broadcast/replay loop pattern.

And we can explicitly decide: **SSE vs WebSocket**. If you want BEAM-first and multi-topic multiplexing, WS/Channels is the most natural.
