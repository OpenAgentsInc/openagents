# Inference-Engineering Book — Implementation Log

The overnight loop's audit log for turning the inference-engineering book's
investigation notes (`khala-investigation-notes.md`) into shipped Khala product
behavior. Each entry records what was done, where, the verification bar, and the
honest scope (what is real vs still `not_measured` / inert).

Conventions:

- Entries are append-only and dated newest-last within a priority lane.
- A priority tag (`P0-1`, `P0-2`, …) maps to the section in
  `khala-investigation-notes.md`.
- "DONE" means merged to `main` and (where noted) deployed. Branch-only work is
  "in progress", never DONE.

---

## P0-1 — Make the Khala scorecard production-complete — DONE, deployed `9b0c9b56`

- **Notes ref:** `khala-investigation-notes.md` §P0 item 1.
- **What shipped:** the canonical, public-safe Khala request-lifecycle telemetry
  schema (`openagents.khala.telemetry.v1`) and its production wiring — token
  counts, the latency split surface, request class, route/provider/served-model,
  verification class + executed verdict + scalar reward, and the cost/margin
  disclosure — recorded on the immediate `openagents` block (small summary) with
  the full record dereferenceable behind the public inference receipt.
- **Honesty discipline:** every numeric is either a real measured number or the
  explicit `not_measured` sentinel; a measured `0` and `not_measured` are
  distinct products. Nothing is fabricated.
- **Where:** `apps/openagents.com/workers/api/src/inference/khala-telemetry.ts`
  (schema + builders) + the `chat-completions-routes.ts` build sites; doc
  cross-reference `docs/inference/2026-06-23-khala-telemetry-scorecard-book-p0-1.md`
  and `docs/inference/khala.md` §3.
- **Merge / deploy:** PR #6085 (merge `f350c3bec5`), deployed `9b0c9b56`.
- **Left honestly `not_measured` for follow-on:** the provider/gateway/verifier/
  settlement time split, queue/batch wait, region, and (at P0-1) the
  cache-affinity hash + cached-input dimension — each recorded as a sentinel with
  a `blockerRef` rather than a fake number. P0-2 closes the cache-affinity hash +
  cached-input gaps.

---

## P0-2 — Treat prefix caching as a product feature — DONE (PR open, #6084)

- **Notes ref:** `khala-investigation-notes.md` §P0 item 2 ("Treat Prefix
  Caching As A Product Feature"); book §5.3 (caching) — prompt order controls
  whether a long shared prefix is reusable.
- **What shipped (the six deliverables):**
  1. **Stable prompt layout** — `assembleStablePromptLayout` orders the outgoing
     messages stable-first (acceptance contract → identity → tool schemas →
     stable policy), volatile/user content last. Gateway-injected blocks are
     tagged with a `StableBlockKind` (structural classification of our own
     blocks, not user-intent string matching). The `khala-identity.ts` injection
     stays in the stable prefix.
  2. **Deterministic ordering + hashing** — `canonicalJson` /
     `serializeToolSchemas` (sorted keys, stable tool order) → byte-identical
     prefix text → stable `stablePrefixHash` for the same logical inputs.
  3. **Cache-affinity keys** — `deriveCacheAffinityKey({account, session?,
     codebase?})`; recorded only as the one-way `hashCacheAffinityKey` digest
     (`cacheAffinityKeyHash`). Raw key never leaves the gateway.
  4. **Provider session affinity** — `sessionAffinityParams` sets Fireworks
     `x-session-affinity` and OpenAI-style `user` to the same opaque hash value,
     pinning a session to one cache-warm replica.
  5. **Cached input tokens + total reconciliation** — `cachedInputTokens` flows
     from provider usage into the telemetry block + record; the live discrepancy
     (`total` 679 ≠ prompt 347 + completion 20) is reconciled in the record
     builder: provider total is recorded receipt-first (authoritative), the gap
     is disclosed as `unaccountedTokens` (312 = billed reasoning/thinking/tool-use),
     never recomputed or dropped.
  6. **Cache-aware routing** — `decideCacheAwareRouting` reorders (never widens)
     the viable lane plan toward the cache-warm lane via a typed
     `CacheWarmthOracle` keyed by the affinity hash, gated by health + privacy/
     region pin policy. Inert by default (no oracle wired → plan unchanged).
- **Where:** `apps/openagents.com/workers/api/src/inference/prompt-prefix-cache.ts`
  (+ `.test.ts`), `cache-aware-routing.ts` (+ `.test.ts`), the
  `chat-completions-routes.ts` integration (+ `.test.ts`), and
  `khala-telemetry.ts` (new `cachedInputTokens` on the block + `unaccountedTokens`
  reconciliation field on the record). Docs: `docs/inference/khala.md` §3
  prefix-caching subsection.
- **Verification bar (green):** the inference test suites (698 tests),
  `typecheck`, `check:architecture`, `check:effect-topology`, and
  `check:public-projection-freshness`. Tests cover deterministic prefix ordering
  (same stable inputs → identical prefix + cache-key hash), volatile content
  never in the prefix, the one-way public-safe affinity hash, session-affinity
  headers set when supported, cached-token telemetry populating from a fixture
  provider-usage payload with correct totals reconciliation, and cache-aware
  routing picking the warm lane (and refusing an unhealthy / pin-forbidden one).
- **Honest scope — still `not_measured` / inert:** cached input tokens remain
  `not_measured` for providers/lanes that do not report a cached dimension; the
  cache-aware-routing seam is inert until the Worker wires a real
  `CacheWarmthOracle` / health / pin-policy (the warm-lane KV/DO record is a
  follow-on); region and the provider/gateway/verifier/settlement time split stay
  `not_measured` (P0-1 follow-on, unchanged here).
- **Status:** PR open against `main` (#6084); orchestrator reviews / merges /
  deploys / smokes. NOT deployed by this entry.

---

## P0-3 — Finish the streaming/async split — DONE (PR open, #6086)

- **Notes ref:** `khala-investigation-notes.md` §P0 item 3 ("Finish The
  Streaming/Async Split"); book Ch.7 (Production — streaming vs async / client
  code); the local 524 postmortem
  `docs/inference/2026-06-22-long-running-inference-response-strategies.md`.
- **Already partial before this entry (NOT re-done):** the durable-stream EPIC
  #6056 (single-client reconnect-resumable SSE proxy) merged; the
  `openagents-inference-batch-jobs` queue exists in `wrangler.jsonc`; the submit
  route + `inference_batch_jobs` D1 table (migration `0217`) + the public receipt
  route + the `executeBatchJob` Queue consumer + the Worker `queue` handler
  routing batch-job messages to it (all flag-gated by
  `INFERENCE_BATCH_JOBS_ENABLED`) were already wired; and the **interactive
  stream already attaches the terminal `openagents` receipt at stream close**
  (`buildTerminalFrame` in `chat-completions-routes.ts`) with
  `requestClass: interactive_stream`. So deliverable 2 (terminal receipt at
  stream close) and the interactive-vs-async request-class distinction on the
  chat path were already in place.
- **What this entry FINISHED (the remaining gaps):**
  1. **Batch closeout receipt is now an auditable terminal receipt.** The
     dereferenceable batch closeout receipt
     (`/api/public/inference/batch-job-receipts/<ref>`) now carries the canonical
     `openagents.khala.telemetry.v1` **record** as its `openagents` block — the
     async-lane counterpart of the interactive stream's stream-close receipt —
     with `requestClass: batch` (distinguishing detached work from an interactive
     stream), a measured `queueWaitMs: 0` (a batch job never blocks the edge), and
     the real `batchWaitMs`. (`batch-job-closeout-receipts.ts`
     `buildBatchJobTelemetryRecord` + `batch-job-routes.ts`.)
  2. **Batch/queue wait is now measured.** Migration `0223` adds
     `enqueued_at`/`started_at` to `inference_batch_jobs`; the submit route stamps
     `enqueued_at` and the queue message carries `enqueuedAtIso` (the START of the
     batch wait), the consumer stamps `started_at` from an injected clock (the
     END), and `computeBatchWaitMs` derives `batchWaitMs = started_at -
     enqueued_at`. Honest `not_measured` + a `batch_wait_not_measured` `blockerRef`
     when timing is unavailable (a pre-`0223` job, a token-only job that was never
     enqueued). Surfaced on the closeout receipt AND `GET
     /v1/inference/batches/:jobId` (job state + wait, auditable from the poll).
  3. **Durable-connection scope documented + confirmed (deliverable 4).** The
     chat gateway uses a DO only for the #6056 single-client reconnect proxy (not
     multi-subscriber); DO + hibernatable-WebSocket multi-subscriber transport is
     reserved for the live Verse world (`apps/openagents-world`). Plain
     request/response inference spins up no DO/WebSocket. Documented in
     `docs/inference/khala.md` §4 "Streaming / async split".
- **Where:** `apps/openagents.com/workers/api/migrations/0223_inference_batch_jobs_wait_timing.sql`,
  `src/inference/batch-job-store.ts` (timing columns), `batch-job-consumer.ts`
  (`enqueuedAtIso` on the message, `nowIso` clock, `started_at` stamp),
  `batch-job-closeout-receipts.ts` (terminal telemetry record + `computeBatchWaitMs`),
  `batch-job-routes.ts` (submit stamps enqueue time; receipt + status read populate
  the telemetry/wait), minimal `index.ts` consumer-deps clock wiring. Docs:
  `docs/inference/khala.md` (§3 telemetry table + §4 streaming/async split).
- **Verification bar (green):** the inference test suites (703 tests),
  `typecheck`, `check:architecture`, `check:effect-topology`,
  `check:public-projection-freshness`, and the exact-route manifest test
  (`worker-exact-routes.test.ts` — no route changes, the three batch routes were
  already registered). New tests: a detached job enqueues → consumer runs it →
  the closeout receipt dereferences with `requestClass: batch` + `queueWaitMs: 0`
  + a real `batchWaitMs` (end-to-end `batch-job-flow.test.ts`); the consumer
  stamps `started_at` from the injected clock; `computeBatchWaitMs` derives the
  delta or rejects unmeasurable/negative inputs; an unmeasured wait records a
  `batch_wait_not_measured` blocker rather than a fake `0`; the status poll exposes
  the wait.
- **Honest scope — still `not_measured` / inert:** the async consumer stays
  flag-gated off (`INFERENCE_BATCH_JOBS_ENABLED`) until the orchestrator arms it;
  the batch closeout telemetry record leaves per-item token counts `not_measured`
  at the closeout summary (they are metered per item, not re-aggregated) with a
  `batch_token_usage_per_item` blocker; the interactive-stream `queueWaitMs` stays
  `not_measured` (no edge queue instrumentation); region + the
  provider/gateway/verifier/settlement time split stay `not_measured` (P0-1
  follow-on, unchanged here).
- **Status:** PR open against `main` (#6086); orchestrator reviews / merges /
  deploys / smokes. NOT deployed by this entry.
