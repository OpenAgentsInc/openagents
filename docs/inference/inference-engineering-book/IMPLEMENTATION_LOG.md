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
  - a real `batchWaitMs` (end-to-end `batch-job-flow.test.ts`); the consumer
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

---

## P1-5 — Build a provider/engine benchmark matrix — DONE (PR open, #6088)

- **Notes ref:** `khala-investigation-notes.md` §P1 item 5 ("Build A Provider
  And Engine Benchmark Matrix") + Open Question #5 (minimum lane decision suite);
  book Ch.4 §4.5 (software benchmarking — "faster at _what_", shadow real
  traffic, match production sequence shapes/contents/concurrency, change one
  variable at a time) and Ch.1 §1.4.1 (latency percentiles over a right-skewed
  distribution). Consumes the P0-1 telemetry schema (#6085) and the P0-2 prefix
  cache (#6084).
- **What shipped (the four deliverables):**
  1. **Typed matrix** (`benchmark/matrix.ts`) varying lane (Vertex-Anthropic /
     Vertex-Gemini / Fireworks / partner-passthrough real; Pylon whole-small /
     Psionic shard-WAN labeled `not_yet_available`), engine (provider-native /
     vLLM / SGLang / TensorRT-LLM, paired to lanes), workload (chat /
     khala-code-artifact-gen / verifier-run / long-context-codebase-question),
     sequence shape (ISL / OSL / cacheable-prefix / concurrency, tagged
     realistic|synthetic), streaming-vs-batch, temperature/reasoning, and the
     verification expectation _derived from the workload_. `expandMatrix` is
     deterministic with a stable axis-encoded `cellId`.
  2. **Runner + pluggable seam** (`benchmark/runner.ts`, `lane-seam.ts`): records
     a canonical `openagents.khala.telemetry.v1` record per sample (reuses the
     schema, never forks). Default = a deterministic, network-free, spend-free
     FIXTURE lane; a real-lane adapter (`makeRealLaneSeam`) is flag/owner-gated —
     unarmed it throws `RealLaneNotArmedError` and `canSpend:false`, so no test or
     un-armed env can ever issue a billable request. Not-yet-available lanes are
     never executed (skipped run, null record, honest reason).
  3. **Dereferenceable report** (`benchmark/report.ts`): per-(lane × workload)
     latency percentiles (P50/P90/P99 + mean over _measured_ samples only),
     perceived TPS, cost-per-accepted-outcome (null when zero accepted — never a
     fake 0), verification rate, cache hit rate. Public-safe (only counts /
     durations / neutral classifiers / coarse region) with a structural
     `checkReportPublicSafety` tripwire.
  4. **Realistic-traffic honesty:** a fixture/synthetic report is
     `decisionGrade:false` + carries an `illustrativeNotice`; decision-grade
     requires an owner-armed REAL seam over REALISTIC traffic with no
     synthetic-only group. Each group independently flagged `syntheticOnly`.
- **Where:** `apps/openagents.com/workers/api/src/inference/benchmark/`
  (`matrix.ts`, `lane-seam.ts`, `runner.ts`, `report.ts`, `fixtures.ts`,
  `index.ts` + `*.test.ts`). `fixtures.ts` ships `SAMPLE_DECISION_SUITE_CONFIG`
  (the Q5 minimum decision suite: Fireworks vs Vertex on chat/code/verifier/
  long-context + the two future lanes). Doc:
  `docs/inference/2026-06-23-khala-benchmark-harness-book-p1-5.md`.
- **Verification bar (green):** the inference test suites (742 tests, 39 new),
  `typecheck`, `check:architecture`, `check:effect-topology`,
  `check:public-projection-freshness`. Tests cover: the matrix expands to the
  expected cells (192 for the decision suite); the fixture runner is deterministic
  and produces telemetry records with the P0-1 fields; the report aggregation
  (percentiles, cost-per-accepted-outcome, verification rate, cache hit rate) is
  correct on a hand-checkable fixture set; the real-lane seam is flag-gated OFF by
  default (no network); and the report is public-safe (the tripwire trips on a
  forbidden key).
- **Honest scope — fixture vs owner-gated:** every number is ILLUSTRATIVE today —
  produced by the deterministic fixture lane over SYNTHETIC shapes. A real,
  decision-grade sweep requires the owner to (1) source realistic Khala traffic
  shapes, (2) provide a live `RealLaneExecutor`, (3) arm `makeRealLaneSeam` with
  explicit confirmation and run it (the only path that can spend). NOT run here.
- **2026-06-23 continuation:** added `benchmark/real-sweep-plan.ts`, a pure
  no-provider-call preflight for the owner-armed real sweep. It requires explicit
  owner confirmation, a public-safe approval ref, a positive msat budget cap, and
  a maximum billable sample cap before the real seam can be armed. It counts only
  currently available lane cells toward the billable sample upper bound, warns
  when future lanes are skipped, requires public-safe observed Khala traffic
  evidence for every `realistic` shape, and keeps synthetic or unevidenced
  traffic out of `decisionGrade` even when it is owner-approved for a smoke.
- **Status:** PR open against `main` (#6088); orchestrator reviews / merges /
  deploys / smokes. NOT deployed, NO spend by this entry.

---

## P1-7 — Quantization needs a Khala eval gate — in progress (PR open, #6090)

- **Notes ref:** `khala-investigation-notes.md` §P1 item 7 ("Quantization Needs
  A Khala Eval Gate") + Open Question #6 (which quant modes may share a public
  alias); book Ch.5 (Quantization). Consumes the P0-1 telemetry schema (#6085),
  the P1-5 benchmark cost-per-accepted-outcome math (#6088), and the P0-4 executed
  Khala-code verifier `AcceptanceVerdict` (#6087) as the eval gate.
- **The principle encoded:** a model served at a reduced precision (FP8 / MXFP8 /
  NVFP4 / INT4 / …) is NOT the same product as the unqualified model id, and a
  throughput win that lowers the accepted-outcome rate is a LOSS unless
  cost-per-accepted-outcome improves.
- **What shipped (the four deliverables):**
  1. **Quantization metadata** (`khala-quantization.ts`): typed
     `KhalaQuantizationMetadata` (precision / backend / backendVersion / scope /
     evalGatePassed / evalGateRef) + `KhalaServedModelDescriptor`. Added as a
     first-class `quantization` field on `KhalaTelemetryRecord`
     (`khala-telemetry.ts`). HONEST sentinels: `unquantized` (known full),
     `not_measured` (honestly unknown), or a concrete reduced-precision value —
     an unmeasured request records `UNKNOWN_QUANTIZATION`, never a fabricated
     full precision.
  2. **Same-model-claim guard** (`khala-quantization-guard.ts`): a typed,
     fail-closed check over the descriptor (`evaluateSameModelClaim` /
     `assertSameModelClaim` → `KhalaSameModelClaimError`). REJECTS an undisclosed
     quantized lane, a `not_measured` lane, and a disclosed-but-ungated quantized
     lane under an UNQUALIFIED public alias; PASSES an unquantized lane, a
     qualified alias (precision named in the id), and a disclosed +
     eval-gate-qualified quantized lane. A bounded enum/field check over the
     structured descriptor — the receipt-disclosure sibling of the identity guard.
  3. **Quantization eval gate** (`khala-quantization-eval-gate.ts`): scores a
     quantized lane vs the original precision on EXECUTED `AcceptanceVerdict`s
     (reuses P0-4, never a regex) and the cost-per-accepted-outcome delta (reuses
     the P1-5 report math). PASSES only when accepted-outcome quality HOLDS, or a
     small drop within the agreed bound is bought back by a sufficient
     cost-per-accepted-outcome improvement; FAILS a drop beyond bound, a drop
     without a cost win, an aggressive (KV-cache/attention) scope without owner
     ack, no baseline accepted outcomes, or no samples.
  4. **Policy doc** (`docs/inference/2026-06-23-khala-quantization-eval-gate-book-p1-7.md`):
     weights-only / FP8 before aggressive KV/attention quant; disclosure required;
     Open Question Q6 alias-sharing policy resolved.
- **Where:** `apps/openagents.com/workers/api/src/inference/`
  (`khala-quantization.ts`, `khala-quantization-guard.ts`,
  `khala-quantization-eval-gate.ts` + `*.test.ts`; `quantization` field added to
  `khala-telemetry.ts`).
- **Verification bar (green):** inference suites (779 tests, 37 new),
  `typecheck`, `check:architecture`, `check:effect-topology`,
  `check:public-projection-freshness`. Tests: quant metadata populates from a
  fixture descriptor (honest sentinel when unknown); the guard rejects an
  undisclosed/unknown/ungated quantized alias and passes a disclosed-gated one +
  a qualified alias; the eval gate passes when quality holds (or a small drop is
  offset by a cost win) and fails when accepted-outcome drops without a cost win;
  the real sweep is flag-gated OFF (no real serving in tests).
- **Honest scope — fixture vs owner/compute-gated:** the metadata, guard, and the
  eval-gate LOGIC run on deterministic fixture comparison sets — no real
  quantized serving, no spend, no compute. The real quantized-vs-original sweep
  (`collectRealQuantSweepSamples`) throws `RealQuantSweepNotArmedError` unless
  owner-armed with an executor; a decision-grade gate decision promoting a real
  lane requires that armed sweep over realistic traffic. NOT run here.
- **Status:** PR open against `main` (#6090); orchestrator reviews / merges /
  deploys / smokes. NOT deployed, NO spend by this entry.

---

## P1-8 — Test speculation where Khala is actually low-batch — in progress (PR open, #6091)

- **Notes ref:** `khala-investigation-notes.md` §P1 item 8 ("Test Speculation
  Where Khala Is Actually Low-Batch"); book Ch.5 (Speculative Decoding);
  `docs/inference/speculative-decoding-article.md`;
  `2026-06-19-decentralized-serving-shard-wan.md` (receipt-mode disclosure).
- **The principle (book, our words):** speculative decoding speeds up DECODE by
  letting a cheap drafter guess tokens and the target model verify them in one
  parallel pass. It profits ONLY when there is spare compute to spend on
  verification — at LOW batch. At HIGH batch / under compute pressure the
  verification work competes with throughput and it is a LOSS. So it must be
  MEASURED (acceptance rate), DYNAMICALLY DISABLED, and DISCLOSED in the receipt.
  Code is a fit (syntax + prompt-context repetition) for draft-free n-gram /
  lookahead modes; EAGLE is a later learned Psionic lane (needs target
  hidden-state data + training).
- **What shipped (the four deliverables):**
  1. **Acceptance-rate telemetry** (`khala-speculation.ts` + the `speculation`
     field on `khala-telemetry.ts`): typed mode (n-gram / lookahead / EAGLE /
     none / not_measured), `active`, the acceptance rate, and the draft-token
     count pair behind it. Honest `not_measured` when no draft/verify pass
     produced counts; `none` (known no speculation) distinct from `not_measured`
     (unknown managed lane); rate clamped to `[0,1]`, never fabricated.
  2. **Dynamic-disablement policy** (`decideSpeculation`): a bounded, typed
     decision over `{batchSize, computePressure}` with documented thresholds
     (default batch ≤ 4, pressure ≤ 0.6). Enables a draft-free mode at low
     batch/pressure; disables on high batch / high pressure / unknown signal /
     learned-or-unavailable mode / not-requested — each with a typed reason.
  3. **Receipt-mode disclosure:** the speculation mode is a first-class field on
     the canonical `openagents.khala.telemetry.v1` record (ties to the shard-WAN
     speculative/direct-return/async receipt-mode idea).
  4. **Policy doc** (`docs/inference/2026-06-23-khala-speculation-telemetry-book-p1-8.md`):
     n-gram/lookahead fit for code repetition; EAGLE flagged as a later
     learned/Psionic lane needing target hidden-state data + training.
- **Fixture decode trace:** `benchmark/speculation-lane.ts` derives a
  deterministic per-cell speculation outcome — code workloads request a draft-free
  mode, the policy decides on/off from the cell's batch (concurrency) + derived
  pressure, and honest draft counts populate only when ENABLED. Threaded through
  the fixture seam + runner so a benchmark sample discloses speculation the same
  way production will; the report adds a draft-acceptance aggregate per
  (workload × model × temperature × route), with a null rate (honest absence)
  where speculation did not run.
- **Where:** `apps/openagents.com/workers/api/src/inference/`
  (`khala-speculation.ts` + `khala-speculation.test.ts`,
  `khala-speculation-telemetry.test.ts`; `speculation` field on
  `khala-telemetry.ts`; `benchmark/speculation-lane.ts`, and the
  `lane-seam.ts` / `runner.ts` / `report.ts` / `index.ts` threads).
- **Verification bar (green):** inference suites (807 tests, 28 new),
  `typecheck`, `check:architecture`, `check:effect-topology`,
  `check:public-projection-freshness`. Tests: acceptance-rate telemetry populates
  from a fixture decode trace (honest sentinel when no speculation ran);
  `decideSpeculation` enables at low batch + disables at high batch / pressure /
  unknown / learned-mode / not-requested; speculation mode disclosed in the
  record + the per-axis report aggregate; the real speculative-decoding engine is
  flag-gated OFF (fixture seam never spends; un-armed real seam refuses to run).
- **Honest scope — fixture vs owner/compute-gated:** the telemetry, the policy,
  and the fixture decode trace run deterministically with no real draft model and
  no spend. The REAL speculative decode (an actual n-gram/lookahead drafter or a
  learned EAGLE head) needs a real serving engine that does not exist in the
  Worker, so the live hot path discloses `not_measured`/`none`; a future
  compute/owner-gated serving engine threads real counts into the same fields.
  EAGLE is a named-but-unbuilt Psionic learned-serving lane. NOT run/armed here.
- **Status:** PR open against `main` (#6091); orchestrator reviews / merges /
  deploys / smokes. NOT deployed, NO spend by this entry.

---

## P2-9 — Disaggregation And Dynamo Patterns — DONE (study, #6092)

- **Notes ref:** `khala-investigation-notes.md` §P2 item 9 ("Disaggregation And
  Dynamo Patterns") + Open Question Q7; book Ch.5 (parallelism /
  disaggregation) and Ch.7 (serving-system pressure).
- **What shipped:** the short design-spike doc
  `docs/inference/2026-06-23-khala-disaggregation-dynamo-study.md`. It records
  the measured traffic/context trigger for reopening prefill/decode
  disaggregation, the prefill-queue and KV-pressure metrics that must exist
  first, and the Dynamo decision.
- **Recommendation:** NOT YET for MVP. Keep Khala on the simpler monolithic
  serving path until production receipts show high-volume, large-model,
  long-context traffic where prefill dominates after prefix caching. Use
  NVIDIA Dynamo as an architecture reference for KV-block management,
  KV-aware routing, and prefill/decode scheduling, not as a dependency today.
- **Honest scope:** docs-only study. No serving engine, gateway route,
  dependency, live traffic, cost, or deploy behavior changed.

---

## P2-11 — Modality-Specific Cloud Primitives — DONE (study, #6094)

- **Notes ref:** `khala-investigation-notes.md` §P2 item 11
  ("Modality-Specific Cloud Primitives"); book Ch.6 (modalities);
  `docs/inference/khala.md` (Khala as one Agent Cloud primitive);
  `docs/inference/2026-06-19-agent-cloud-revshare-everywhere.md`.
- **What shipped:** the per-modality contract doc
  `docs/inference/2026-06-23-khala-modality-cloud-primitive-contracts.md`.
  It defines request shape, receipt/metric fields, scaling lane, and
  product-promise gating for embeddings/bulk documents, live voice, and
  image/video primitives.
- **Principle recorded:** shared account, balance, receipt, referral, and
  settlement rails do not imply shared chat metrics. Embeddings are
  async/batch-first, live voice is bidirectional/session-first, and image/video
  lanes are compute-bound artifact jobs scaled independently from LLM
  decode-heavy traffic.
- **Honest scope:** docs-only study. No production primitive, WebSocket route,
  batch worker, image/video worker, telemetry schema, product-promise state,
  traffic, spend, or deploy behavior changed.
