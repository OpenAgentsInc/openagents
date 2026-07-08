# What To Look Into For Khala

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Status: opinionated follow-up notes after reading `Inference Engineering.pdf`
and the local `docs/inference/` corpus on 2026-06-23.

## My Read

This is relevant to Khala, but mostly at the production-systems level. The book
does not change the Khala thesis. It sharpens what we should measure and which
serving decisions should remain evidence-backed.

Khala is already pointed in the right direction: one compatible endpoint, many
workers underneath, receipt-first metering, executed verification, Pylon supply,
and Bitcoin settlement. The next risk is that we add more routing, engines, or
benchmark claims before the request lifecycle is measurable enough to compare
them.

## Khala on Blueprint + Tassadar plugin extensibility

A companion architectural-direction note,
[`../2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`](../2026-06-23-khala-blueprint-program-and-plugin-extensibility.md),
extends the "inference platform + control plane" read here in two ways. First,
Khala's inference should run as **typed, optimizable Blueprint/DSPy programs**
(signature lookup + GEPA-optimized prompts/policies + the learned coordinator),
so quality/cost improve by optimization against the executed evals and
acceptance receipts this telemetry list is built to capture — the scorecard
fields above are precisely the reward inputs that learned-program optimization
needs. Second, that Blueprint program layer should be **extensible via
independently authored capability units** (Tassadar-style plugins/modules)
discovered, composed into Khala programs, metered per use, and (FUTURE) paid in
Bitcoin with a revenue split — so Khala grows capabilities without core
changes. That note keeps an honest current-vs-future split and honors the
Tassadar no-public-marketplace boundary; nothing in it is a product promise.

## P0 - Do First

### 1. Make The Khala Scorecard Production-Complete

Current M8 docs are honest that token, cost, Verse, settlement, and energy
telemetry are still missing or partial. The book's metric split suggests the
receipt/manifest should preserve at least:

- prompt tokens, completion tokens, total tokens;
- cached input tokens where the provider exposes them;
- TTFT;
- inter-token latency or perceived TPS;
- total wall-clock time;
- provider time, gateway overhead, verifier time, and settlement time when
  available;
- queue wait time and batch-job wait time;
- request class: interactive stream, detached async job, verifier run, or batch;
- route, provider, served model, region, cache-affinity key hash, and fallback
  reason;
- verification class, executed verifier result, and scalar reward;
- cost basis, price, margin bucket, settlement state, and blocker refs.

The immediate use is M8: stop treating tokens and cost as afterthoughts in the
recorded manifests. The strategic use is router training: Khala's learned
coordinator needs reward inputs that reflect accepted outcome per sat and per
second, not just "did the model answer."

### 2. Treat Prefix Caching As A Product Feature

The Fireworks doc already says prompt caching is on by default and benefits from
`x-session-affinity`. The book makes this more important: prompt order controls
whether a long shared prefix is reusable.

Look into:

- stable prompt layout for Khala-code and Autopilot: system text, tool schemas,
  acceptance contract, and stable policy blocks first; user-specific request and
  volatile context last;
- deterministic ordering and hashing of tool schemas and verifier contracts;
- cache-affinity keys by account/session/codebase, recorded as public-safe
  hashes in receipts;
- provider telemetry for cache hits and cached-token billing;
- cache-aware routing in the ModelRouter/Coordinator, constrained by provider
  health, privacy, region, and spend policy.

This is probably the highest-leverage near-term performance/cost item because
Khala coding traffic repeats long system prompts, tool schemas, and codebase
context.

### 3. Finish The Streaming/Async Split

The local 524 postmortem already discovered the same production lesson as the
book: long synchronous inference through the edge is the wrong shape.

Look into:

- keep streaming as the interactive default for Autopilot and head-to-head
  runner paths;
- finish the batch-job consumer for detached/minutes-long inference jobs;
- attach the terminal `openagents` receipt event at stream close and to the
  batch receipt endpoint;
- expose queue wait and job state in receipts so long-running work is auditable;
- use Durable Objects/WebSockets only where a live multi-subscriber UI or Verse
  projection needs them.

### 4. Fix The Executed-Verifier Contract Gap

The current status doc says a contract-augmented crossy-road artifact passes the
real headless runner, while the bare prompt fails and the gateway pre-screen
rejects CDN-loaded three.js. That is directly related to the book's "evals
before optimization" lesson.

Look into:

- injecting or otherwise supplying the verifier state contract to `khala-code`
  runs so the bare product path can produce execution-verifiable artifacts;
- changing the cheap pre-screen so a single HTML file may use a pinned
  allowable CDN dependency for three.js;
- recording pre-screen verdict and executed verdict separately;
- making `verified:true` impossible without an executed verifier receipt.

Until this is clean, faster model serving risks optimizing for artifacts that
still fail the actual acceptance suite.

## P1 - Next Layer

### 5. Build A Provider And Engine Benchmark Matrix

Before Pylon or self-hosted lanes become product claims, benchmark them against
real Khala workloads.

The matrix should vary:

- lane: Vertex, Fireworks, partner passthrough, Pylon whole-small, later
  Psionic shard-WAN;
- engine: provider-native, vLLM, SGLang, TensorRT-LLM where applicable;
- workload: chat, Khala-code artifact generation, verifier run, long-context
  codebase question, embedding/batch future lane;
- sequence shape: input length, output length, cacheable prefix length;
- streaming versus batch;
- temperature/reasoning settings;
- verification outcome, not just raw token speed.

Use realistic traffic. Synthetic benchmarks are useful only if they match the
input/output lengths, prompt contents, cache behavior, and concurrency Khala
actually sees.

### 6. Start Pylon Serving With Proven Engines

For near-term whole-small-model Pylon serving, avoid writing a bespoke engine.
Use a proven runtime and make Pylon capability evidence precise.

Look into:

- vLLM for broad support and fast experiments;
- SGLang for Kimi/Qwen/DeepSeek/MoE or code-heavy experiments;
- TensorRT-LLM only when the model/GPU path is stable enough to justify the
  configuration work;
- worker self-benchmarking before registration;
- warm-model residency and cold-start telemetry;
- canary and replay challenges before payout;
- receipt fields for engine, version, quantization, GPU, warm/cold state, and
  verifier/parity result.

This fits the existing split: Psionic owns execution evidence; product surfaces
own pricing, routing, payout, and marketplace authority.

### 7. Quantization Needs A Khala Eval Gate

Quantization is likely useful for Pylon and managed open-model lanes, but it can
change output quality.

Look into:

- weights-only or FP8/MXFP8 first, before aggressive KV or attention
  quantization;
- custom eval comparison against original precision using Khala-code executed
  checks and any future product-specific evals;
- explicit quantization metadata in receipts;
- rejecting "same model" claims unless the precision/backend is disclosed.

For Khala, a 40 percent throughput win that drops accepted outcome rate is a
loss unless the cost-per-accepted-outcome improves.

### 8. Test Speculation Where Khala Is Actually Low-Batch

Speculative decoding is relevant, but it is not always profitable. It helps most
when decode is the bottleneck and batches are low enough that spare compute can
verify drafts.

Look into:

- n-gram/lookahead speculation for code generation and code revision;
- EAGLE only as a later Psionic/learned-serving experiment because it requires
  target-model hidden-state data and training;
- acceptance-rate telemetry by workload, model, temperature, and route;
- dynamic disablement when batch size or compute pressure makes speculation
  counterproductive;
- receipt-mode disclosure for shard-WAN speculative/direct-return/async modes.

## P2 - Study, But Do Not Put On The MVP Critical Path

### 9. Disaggregation And Dynamo Patterns

Prefill/decode disaggregation is relevant to future high-volume, long-context
Khala coding traffic, but it is probably premature for the current stage.

Look into later:

- conditional disaggregation keyed by post-cache input length;
- prefill queue size as a first-class metric;
- decode KV cache pressure and offload strategy;
- dynamic prefill/decode engine ratios;
- Dynamo's KVBM and routing ideas as architecture references.

Trigger condition: high-volume large-model traffic with enough long context that
prefill work dominates, not a desire to add infrastructure complexity early.

### 10. Multi-Cloud And Geo-Aware Routing

Khala's supply is already multi-provider: Vertex, Fireworks, passthrough, and
eventually Pylon. The book's multi-cloud chapter is a useful control-plane
reference.

Look into:

- global capacity view across providers and Pylons;
- active-active provider failover for the compatible API;
- region and data-residency controls for customer accounts;
- provider-health scoring in routing receipts;
- workload planes that keep serving if a central control plane is degraded.

### 11. Modality-Specific Cloud Primitives

Do not reuse chat metrics for everything. Embeddings, ASR, TTS, image, and video
will each need different serving, batching, and receipt fields.

Look into:

- embeddings as async/batch-first;
- live voice as bidirectional streaming;
- image/video as compute-bound lanes scaled independently;
- per-modality product promises that require paid receipts before green claims.

## How This Maps To Existing Khala Docs

- [`../khala.md`](../khala.md): keep the public API, coordinator, verification,
  and settlement shape. Add richer telemetry to the `openagents` block as
  non-breaking fields or public receipt detail.
- [`../khala-buildout-roadmap.md`](../khala-buildout-roadmap.md): M0/M1/M8
  should prioritize telemetry completeness, streaming, and executed verification
  before new composition claims.
- [`../2026-06-19-fireworks-provider.md`](../2026-06-19-fireworks-provider.md):
  turn session affinity and cached-token telemetry into router inputs.
- [`../2026-06-19-decentralized-serving-shard-wan.md`](../2026-06-19-decentralized-serving-shard-wan.md):
  keep shard-WAN receipt-gated. The book supports starting with whole-small
  serving and treating large-model fabric work as hardware/evidence gated.
- [`../2026-06-19-leyten-compute-shard-audit.md`](../2026-06-19-leyten-compute-shard-audit.md):
  the harvest list is still right. Add production engine benchmarking and cache
  telemetry before marketplace claims.
- [`../2026-06-22-long-running-inference-response-strategies.md`](../2026-06-22-long-running-inference-response-strategies.md):
  make this operational policy, not just a postmortem.
- [`../2026-06-23-khala-head-to-head-m8-status.md`](../2026-06-23-khala-head-to-head-m8-status.md):
  use the next run to close telemetry gaps, not to chase another anecdotal
  artifact.

## Open Questions Worth Turning Into Issues

1. What is the canonical public-safe Khala request telemetry schema?
2. Which fields belong in the immediate `openagents` response block versus a
   dereferenceable receipt?
3. What cache-affinity policy is allowed for each provider and account privacy
   posture?
4. How should Khala inject or expose acceptance-runner contracts for arbitrary
   artifact tasks?
5. What is the minimum benchmark suite for deciding between Fireworks, Vertex,
   Pylon whole-small, and later shard-WAN lanes?
6. What quantization modes are allowed to share a public model alias?
7. At what traffic threshold does disaggregation become worth a design spike?
8. Which metrics feed the Verse HUD, and which remain private operational
   telemetry?
