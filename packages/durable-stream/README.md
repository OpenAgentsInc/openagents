# @openagentsinc/durable-stream

An Effect + Effect Schema **durable append-only offset-log** primitive on
Cloudflare Durable Objects with SQLite storage. It implements the four-part
[Durable Streams](https://github.com/durable-streams/durable-streams) conformance
contract — **ported, not vendored** (the upstream ElectricSQL repo is a read-only
reference under `projects/repos/durable-streams/`).

This is the owned substrate for:

- **Rank-1** resumable Khala inference (durable proxy: persist upstream tokens →
  durable read URL → resume by offset). *Deferred — issue #6058, gated behind the
  in-flight `workers/api` lane.*
- **Rank-2** Verse-world bounded delta-replay buffer. *Deferred — issue #6059.*

See the roadmap: `docs/research/2026-06-23-durable-stream-integration-roadmap.md`
and the audit `docs/research/2026-06-23-effect-durable-streams-on-do-audit.md`.
EPIC: #6056. Core primitive: #6057.

## The four-part conformance contract

1. **Offset-addressed replay** — reads from a stored offset return the exact
   suffix; offsets are opaque, lexicographically-sortable, monotonic, strictly
   increasing, URL-safe, and never the reserved sentinels `-1`/`now`.
2. **Resumability + EOF** — resume from `Stream-Next-Offset`; `Stream-Up-To-Date`
   ≠ EOF; only `Stream-Closed` (durable, monotonic, idempotent) is EOF, signalled
   per read mode.
3. **Exactly-once writes** — `(producerId, epoch, seq)` dedup, zombie fencing,
   gap detection; validate+append serialized per `(stream, producerId)` and
   committed atomically (the DO's single-threaded transactional execution gives
   this for free).
4. **CDN-friendly fan-out** — cacheable catch-up reads (`ETag` varying with
   closure, `Cache-Control`, `Stream-Cursor`).

## Layout

| File | Role |
|------|------|
| `src/offset.ts` | offset codec (branded Effect Schema; lexicographic) |
| `src/protocol.ts` | wire header/param constants + helpers |
| `src/core.ts` | transport-agnostic state machine (pure; Bun-testable) |
| `src/store.ts` | `StreamStore` port + in-memory impl |
| `src/http.ts` | Web `Request`/`Response` adapter (+ SSE) |
| `src/durable-object.ts` | Cloudflare DO + SQLite adapter |
| `src/test-server.ts` | Bun-hostable test server (same core/http paths) |

The core has **no Cloudflare import** — the DO adapter is isolated, so the whole
protocol state machine is unit-testable under Bun with `MemoryStreamStore`. The
DO adapter (`SqliteStreamStore` / `handleDurableStreamFetch`) wires the same core
to `ctx.storage.sql` and DO alarms (TTL/expiry).

Stream URL scheme: `{base}/v1/stream/{path}` (matches the upstream conformance
harness).

## Tests / conformance oracle

```
bun test          # 54 owned conformance + unit tests, all green
bun run typecheck # tsc --noEmit, clean
```

The owned tests in `src/*.test.ts` **replicate the upstream conformance YAML
cases** (`projects/repos/durable-streams/packages/client-conformance-tests/test-cases/`)
as Bun tests driving raw `fetch` against the local test server. This is the
"conformance suite as oracle" loop; the task explicitly permits replicating the
YAML cases as Bun/Effect tests.

### Honest conformance coverage

Upstream declares **269 YAML cases** (the "~320" figure also counts the server
suite's inline vitest cases). We implemented a **representative subset (54 owned
tests)** covering the four-part contract. Status by bucket:

**Covered (green):**

- `producer/create-stream` — 201/200/409 idempotency, content-type mismatch,
  create-and-close, TTL/expiry conflict + malformed TTL.
- `producer/append-data` + `sequence-ordering` — concatenation, 404, empty-body
  400, content-type 409, `Stream-Seq` monotonic regression 409.
- `producer/idempotent/{sequence-validation,epoch-management,multi-producer,error-handling}`
  + idempotent close — dedup 204, gap 409 (expected/received), new-epoch accept,
  bad-new-epoch 400, zombie fence 403 (current epoch), independent producers,
  partial/non-integer header 400, duplicate closing-append 204.
- `consumer/read-catchup` + `offset-handling` + `offset-resumption` +
  `message-ordering` — empty read, single/multi chunk, exact-suffix resume from
  offset, ordering, unicode, `offset=now`, `offset=-1`, malformed-offset 400.
- `consumer/streaming-equivalence` — catch-up == SSE stored data.
- `consumer/cache-headers` — `ETag` present, cacheable `Cache-Control`, ETag
  varies with closure (`:c`).
- `consumer/read-longpoll` — 200-with-data + cursor, 204 timeout + up-to-date +
  cursor, immediate-204-on-closed-at-tail (no cursor).
- `consumer/read-sse` + `read-sse-base64` — text data/control events,
  `streamClosed` control, binary base64 + `stream-sse-data-encoding` header.
- `lifecycle/stream-closure` + `stream-lifecycle` — close-only EOF-at-tail,
  idempotent close, append-to-closed 409 + `Stream-Closed` + final offset,
  atomic append-and-close, HEAD metadata, DELETE + isolated recreate.
- `validation/input-validation` — partial coverage via the 400/409 cases above.

**Not implemented (out of v1 scope — tracked separately, NOT claimed green):**

- **Forking** (`Stream-Forked-From`, `Stream-Fork-Offset`, sub-offsets,
  soft-delete / refcount GC). No upstream-fork YAML bucket is satisfied.
- **Subscriptions** (`__ds/subscriptions`, webhooks, pull-wake, leases).
- **JSON mode** array flattening / message-boundary semantics
  (`producer/batching`, `idempotent-json-batching`, `json-parsing-errors`).
- **Held-connection live waiting** — the core is synchronous, so long-poll/SSE
  return the currently-available suffix immediately rather than blocking for new
  data. The required 204/closed shapes and SSE framing are covered; the *waiting*
  semantics (`fault-injection`, `retry-resilience`, `read-auto` reconnection,
  full `read-sse` 26-case streaming) are an adapter concern not exercised here.
- **Retention/compaction** (`410 Gone` before earliest retained), `304
  Not-Modified` via `If-None-Match`, full `error-context`/`dynamic-headers`
  buckets.

We do **not** claim full conformance. The external upstream
`@durable-streams/server-conformance-tests --run` harness was **not executed**,
because running it requires `pnpm install` inside the read-only reference clone
(pulling `@durable-streams/client` + vitest), which the workspace read-only-
reference rule forbids. The owned Bun tests are the in-package oracle and gate CI.

## Not wired into any app

Per the EPIC plan, this package ships standalone. Rank-1 inference wiring
(`apps/openagents.com/workers/api`) is deferred to #6058 and gated behind the
in-flight `workers/api` lane to avoid collision. Rank-2 world buffer is #6059.
