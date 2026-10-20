# Psionic implementation roadmap

Updated on 2026-10-20. Source baseline:
`818354f0e5317f40a2d6adf0b7a3228f10b8b20d` in this monorepo.
This roadmap combines the [DeepInfer review](deepinfer-lessons.md) and
[Mesh-LLM review](mesh-llm-lessons.md). Those reviews pin the upstream revisions
and describe their limitations. The Psionic findings below were checked again
after updating to this baseline.

Status: proposed implementation sequence. No runtime change or new device
qualification is completed by this document.

## Direction and priorities

First improve the native serving path that runs today: measure it accurately,
execute real device batches, and reduce structured output overhead. Then overlap
host and device work. In parallel with that work, qualify the existing Gemma
distributed path before extending stage packaging, lifecycle, and placement.
Speculative verification and cache/wire optimizations follow demonstrated
execution capability.

Keep one Rust implementation of each responsibility. Extend `psionic-runtime`,
`psionic-serve`, the existing backends, and `psionic-cluster`; keep delivery in
`psionic-datastream` and transport in `psionic-net`. DeepInfer's Python stack
and Mesh-LLM's patched llama.cpp engine are references for individual designs.
Neither is required to implement this roadmap. Pylon's existing explicit
llama.cpp fallback remains a separate serving option, as the
[overview](README.md) records.

The order below is the default priority when the same engineer or device is
needed. Dependencies determine what can proceed independently. These are
completion criteria, not calendar estimates.

| Priority | Step | Dependency | Main result |
| --- | --- | --- | --- |
| P0 | R0: Establish execution and correctness baselines | None | Comparable raw measurements and scoped output contracts. |
| P0 | R1: Execute bounded, live Qwen device batches | R0 | HTTP request grouping reaches a real device batch with bounded admission. |
| P0 | R2: Reuse compiled structured output constraints | R0 | Shared immutable compilation with request-local matching state. |
| P1 | R3: Overlap scheduling and device execution | R1; compose with R2 when enabled | Fewer measured host waits with correct buffer lifetimes and graph reuse. |
| P1 | R4: Qualify the existing Gemma stage boundary | R0 | Full-model versus staged numerical and state-continuation evidence. |
| P1 | R5: Materialize only each stage's required assets | R4 | Verified stage packages with measured transfer and memory savings. |
| P1 | R6: Control stage readiness and topology replacement | R4, R5 | Ready chains become routable; partial or stale work is cleaned up. |
| P2 | R7: Place stages using measured workload costs | R6; R1 for concurrent-throughput claims | Explicit latency/throughput objectives and comparisons with whole-request serving. |
| P2 | R8: Add cache-cost routing and exact wire compression | R0 for cache evidence; R6 for distributed use | Independently measured optimizations over a qualified execution path. |
| P2 | R9: Extend fixed-length speculative verification | R0, R1; R2 before constrained speculation | Committed throughput improves after all draft, verification, and rollback costs. |

R2 and R4 can proceed after R0 without waiting for R1. R7 and R9 are separate
follow-ups; speculation does not depend on finishing distributed serving.
No implementation step depends on a new public mesh, a storage cluster, or a
new accelerator vendor.

## DeepSeek-specific support

The [DeepSeek support and infrastructure plan](deepseek-support-plan.md) adds
a model-specific implementation sequence based on a broader source survey.
DeepSeek model names cover different numerical and protocol architectures;
support must be qualified per artifact and engine.

| Priority | DeepSeek step | Relationship to this roadmap |
| --- | --- | --- |
| P0 | DS0: Pin R1-Distill-Qwen-1.5B and its actual tokenizer/template | Extend R0 and the existing CPU/Pylon qualification paths. Qualify external-engine GPU execution separately from native support. |
| P0 | DS1: Adapt official Rust V4/V4.1 recipe components | Run protocol/token fixtures with mock inference immediately; connect strict constraints to R2. This does not require loading a full DeepSeek model. |
| P1 | DS2: Add native V2-Lite MLA/MoE reference execution | Reuse artifact and sampling contracts; implement latent KV, RoPE/scaling, and the checkpoint's shared/routed expert semantics. |
| P1 | DS3: Add single-node acceleration and V3/full-R1 variants | Reuse R1/R3 for batch/overlap claims; qualify weight/scale layouts and device support separately. |
| P2 | DS4: Add V3.2 sparse indexing, then V4.1 reference components | V4.1 additionally needs compressed/shared attention, mHC, and Engram. Qualify model-specific R4/R5 boundaries; Gemma results do not qualify these models. |
| P2 | DS5: Add expert scale-up on a qualified fast fabric | Use R4–R7 packaging, lifecycle, and placement. Current DeepEP is a Hopper-or-newer NVLink/RDMA path, not WAN Pylon transport. |
| P2 | DS6: Evaluate a matched DSpark draft | Extend R9 using an exact trained target/draft pair and calibration evidence from DeepSpec. |

DS0's prompt/token fixtures and DS1's Rust adapter are immediate additions to
the first-change queue alongside R0–R2. DS2's CPU reference can proceed without
buying datacenter hardware. The current FlashMLA release excludes Hopper and
older DeepSeek models and changes KV formats; use the support plan's pinned
kernel/model/device matrix rather than combining upstream `main` branches.
The RTX 4080 is outside the primary optimized FlashMLA/DeepGEMM targets.

## GCP rollout alongside the runtime work

The [GCP deployment proposal](deepinfer-lessons.md#gcp-deployment-proposal-2026-10-20)
uses the current project inventory, rather than assuming the local GPU or
DeepSeek's production topology. These are proposed serving milestones; the
[2026-10-20 snapshot](assets/gcp-inventory-2026-10-20.json) is read-only evidence,
not an allocation or device qualification.

| Milestone | Existing boundary | Completion evidence |
| --- | --- | --- |
| C0: One private hosted worker | Gateway `Upstream`/Chat Completions transport; model registry and current GCE health/deployment patterns | Explicit hosted identity, pinned model/runtime, actual end-to-end final answer and capability/cost/cancellation checks. Preserve opt-in `local/*` semantics. |
| C1: Efficient replicas | R0/R1/R2, bounded admission, existing accounting and fallback | Real device batches, queue-inclusive latency, overload and spot-loss behavior; compare A100 with a qualified G4/H100 candidate at the same quality floor. |
| C2: Native DeepSeek or an isolated Blackwell experiment | DS0–DS4 for native support; separately qualified external engine for an EP8 B200 experiment | Exact model/device/format results. The current DeepInfer release is SM100/V4.1-specific; no native or multi-node claim from a successful external-engine run. |
| C3: Optional P/D and persistent KV | R3–R8, measured transfer and restore contracts | A serving gain after transfer, storage, failure and idle-capacity costs. Local cache affinity stays until any worker can really access shared cache. |

A100/L4 quota and newer spot B200/H200/G4 ceilings make these experiments
plausible; they do not grant GPU-serving budget or guarantee a successful
allocation. Cloud Run GPU has separate quota, and a count of A4 nodes is not a
64-GPU NVLink domain. Start complete-request serving before disaggregation;
leave the production Clef GPUs and the owner's build priorities intact.

## What already exists

| Existing path | Source evidence | Reuse and remaining work |
| --- | --- | --- |
| HTTP work grouping | `OpenAiCompatWorker` drains queued commands and groups generation by model. Its sender is an unbounded channel. [S1] | Keep this worker as the integration point. A scheduler queue limit alone does not bound HTTP ingress. |
| Shared scheduling | `GenerationSchedulerPolicy`, `GenerationSchedulerMetrics`, and `run_continuous_batch_generation_requests` budget and interleave prefill/decode. [S2] [S3] | Extend the policy, receipts, and executor; interleaving is not proof of a fused device batch. |
| Qwen execution | CUDA, CPU, and Metal `generate_continuous_batch` methods call `generate` for each complete request. CUDA's streaming trait wraps a completed response. [S4] | Begin real device batching on CUDA. Check actual token delivery rather than assuming a streaming wrapper provides live progress. |
| Constraints and sampling | `GenerationSampler::new` compiles a request-local `StructuredOutputMatcher`; decoded token suffixes are cached. Seeded and exact bounded-candidate tests exist. [S3] [S5] | Preserve accepted constraints and sampling behavior while moving repeated work out of decode. |
| CUDA optimization | Argmax/bounded-candidate paths reduce readback. Graph capture/replay metrics and identities already exist. [S4] [S6] | Measure remaining waits, graph coverage, padding, and capture cost. |
| Real distributed execution | `DistributedGemma4TextGenerationService` uses a Metal front with a remote suffix path. Step messages carry token, position, hidden state, and forwarded K/V; HTTP worker handlers dispatch CUDA step/reset calls. [S1] [S7] | Qualify and extend this model-specific implementation. Its existence does not establish arbitrary mixed-backend stage support. |
| Cluster planning | Whole-request, pipeline, tensor, and sparse expert placement exist. The inspected pipeline planner is CUDA-only and splits layer counts evenly; tensor placement has a stricter default RDMA policy. [S8] [S9] [S10] | Connect actual execution to planned topology and retain communication eligibility checks. |
| Artifacts, transport, and recovery | Sharded manifests, digested datastream chunks, direct/relayed path observations, and ordered cluster catchup/snapshots exist. [S2] [S11] [S12] [S13] | Extend these contracts for stages and stale-work protection. Do not build another discovery or recovery system. |
| Prefix cache and MTP | Prefix compatibility includes artifact, tokenizer/template, backend, and tenant. CPU Qwen3.8 MTP supports an opt-in, one-draft, greedy path and rejects structured output. [S3] [S4] | Keep cache isolation and the MTP correctness reference; broader support needs separate qualification. |

Changes since the original reviews include public prompt-template fixtures and
additional CUDA projection/quantization support. Preserve those fixtures and
model-loading paths. They do not change the sequential Qwen batch methods or
the equal-layer pipeline split described above. [S4] [S14]

## R0: Establish execution and correctness baselines

**Scope:** extend the existing Qwen CUDA benchmark and serving metrics before
changing execution. Use a small, already supported pinned GGUF, with the
Qwen3.5 0.8B Q8_0 model in the overview as the initial CUDA candidate. Record
artifact and tokenizer/template digests, runtime commit, backend, device,
precision, cache mode, request seed, and workload. [S15]

Keep three comparisons: serial direct execution, concurrent requests through
the existing HTTP worker, and full-model versus existing Gemma staged
execution. Distinguish internal first-token computation from first token
delivered to the client. Add queue wait, effective device batch size,
host/device duration and waits, committed output tokens, and peak host/device
and KV memory. Existing aggregate timing totals cannot produce tail latency;
retain per-request observations for p50, p95, and p99.

**Complete when:** the same tokenized workload can be replayed and compared
without mixing measured output tokens with estimated speculative throughput.
Teacher-forced numerical checks and seeded-generation checks are separate.
Define tolerances per backend and execution mode before evaluating a change;
cross-backend bitwise equality is not the default requirement. Set the target
workload's latency and memory budgets before using them to accept a speedup.

## R1: Execute bounded, live Qwen device batches

**Scope:** replace the sequential CUDA Qwen batch method with per-request
execution state and a batched device step. Reuse the current worker, runtime
policy, sampler, cache ownership, and receipts. Admit compatible new requests
between steps, budget prefill alongside decode, and emit actual scheduler
metrics. Bound worker ingress as well as active requests; implement a deliberate
overload response rather than allowing the channel to grow indefinitely.

Start with one model/backend/precision and greedy decode. Add sampled and
structured requests only within their qualified execution envelope, with an
observable fallback for unsupported combinations. Carry request seeds and
cache ownership independently of batch order. Add progress delivery from the
executor to the existing API path so live streaming, cancellation, and client
backpressure work while other requests remain active.

**Complete when:** a trace shows multiple live requests in a real device batch;
adding a request does not restart or wait for every active request to finish.
Mixed prompt lengths, EOS, cancellation, queue saturation, and a disconnect
release the right resources. Batched output passes R0's numerical and sampling
checks. Compare concurrent HTTP throughput and tail latency with the serial
baseline, including single-request performance. CPU and Metal behavior remains
explicit until their batching paths are separately qualified.

## R2: Reuse compiled structured output constraints

**Scope:** split immutable compiled grammar data from mutable matcher state in
`psionic-runtime`, then reuse it in `GenerationSampler`. Key compilation by
constraint content, tokenizer identity, and compiler version. Move cold
compilation outside the decode loop, bound compilation/cache resources, and
retain the existing schema, regex, GBNF, choice, and tagged-structure contracts.

Land this in two changes: compiled-state reuse first, incremental matching and
token-mask reuse second. Compare the latter against today's candidate-text
classification. Preserve request-local state even when requests share a
compiled grammar. This work does not require XGrammar or a Python process.

**Complete when:** cold compilation, cache hits, and per-token filtering have
separate measurements; repeated tool schemas reduce work without unbounded
cache growth. Invalid input, Unicode/token boundaries, EOS, and cancellation
preserve the existing output contract. Before combining with speculation,
rejection must restore matcher state as well as model state.

## R3: Overlap scheduling and device execution

**Scope:** use R1's request states to prepare later work and process completed
results while the device runs. Make event dependencies and buffer ownership
explicit in the CUDA backend. Retain selected-token/bounded-candidate readback
and expand existing graph reuse with bounded shape buckets only where measured
workloads justify them.

**Complete when:** device timelines demonstrate the overlap and remaining host
waits. Cancellation or EOS after preparation cannot reuse live buffers or
commit an extra token. Graph keys preserve artifact, shape, decode mode,
cache format, and allocation compatibility; capture misses and eager fallback
remain visible. Report padding and capture cost alongside replay hits.
Re-run batch-invariance qualification when changing kernels or padding.

## R4: Qualify the existing Gemma stage boundary

**Scope:** start from the implemented Gemma Metal-front/remote-CUDA step/reset
path, its binary framing tests, and the existing full-model execution. Inventory
which hidden, K/V, token, and positional values the boundary actually consumes.
Bind that contract to artifact identity, layer range, state format, and schema
version. Reject incompatible boundaries before loading or dispatch.

Use a deterministic fixture for cheap contract/state tests, followed by an
isolated device run with a pinned supported artifact. Compare logits along
the same trajectory, greedy generation, cached continuation, and reset behavior.
Test actual forward-K/V dependencies rather than assuming every boundary is
one activation vector. Extend existing endpoints and transports compatibly.

**Complete when:** a retained result identifies exactly which artifact,
boundary, precision, backend pair, and runtime passed. Truncated or mismatched
frames fail before computation; state is neither shared across requests nor
left behind after cancellation. A two-stage plan alone does not close this
step. Additional model families and recurrent/shared-KV boundaries require
their own evidence.

## R5: Materialize only each stage's required assets

**Scope:** extend `ShardedModelManifest` resolution to exact tensor ownership
and required shared assets. Use `DatastreamManifest` for verified object/chunk
delivery, and the current model loaders for materialization. Account for tied
weights, shared metadata, tokenizer/template data, and model-specific assets.
Verify what existing loaders read and retain before promising stage-only memory.

**Complete when:** a worker can fetch and load its assigned stage without
fetching the whole model payload. Manifest validation catches gaps, overlaps,
wrong digests, and missing shared assets. Package materialization is atomic,
and active stage files are protected from cleanup. Report actual transferred
bytes, disk use, host RAM, VRAM, and readiness time; keep runtime workspace,
KV, and replacement headroom in the memory budget.

## R6: Control stage readiness and topology replacement

**Scope:** connect qualified execution and packages to existing ordered cluster
state. Prepare downstream stages first; publish the entry route only after
every required stage reports the expected artifact and boundary. Bind loads,
stops, and state ownership to a topology generation so delayed commands cannot
take over or stop newer work. Preserve existing authorization.

Keep control-state recovery separate from model/KV recovery. Specify the
behavior for each active request during stage loss or replacement: drain,
restore qualified state, or fail and clean up. Do not promise seamless
migration from successful route replacement alone.

**Complete when:** failed loads release partial reservations; stage loss
withdraws the route; delayed loads/stops are rejected; reconnect and restart
cannot resurrect stale ownership. Exercise duplicate commands, reordered
delivery, coordinator loss, cancellation, and cleanup. A coordinator quorum
never substitutes for readiness of every required compute stage. Resource
measurements include any period with both old and new stages resident.

## R7: Place stages using measured workload costs

**Scope:** extend `schedule_pipeline_sharded_execution` beyond even layer
counts. Use legal boundaries from R4, actual stage/package bytes from R5,
and compute/transfer observations from R0 and R6. Retain CUDA-only admission
in the generic planner until another path is explicitly integrated and
qualified; the existing Gemma mixed-backend specialization does not widen it.

Expose separate objectives: minimize total serialized stage cost for a single
request, or minimize the bottleneck for a demonstrated concurrent pipeline.
Include queueing, activation and sideband transfer, host/device copies, and
return/sampling work. Compare local full-model execution and remote whole-request
placement before adding more stages. Keep tensor collective requirements
distinct from pipeline link requirements.

**Complete when:** equal-layer, latency-oriented, and throughput-oriented plans
have measured comparisons on unequal devices. Reports show predicted versus
observed cost, selected direct/relay path, memory margins, and any missed latency
budget. A capacity gain is labeled separately from a speed gain. Missing or
stale observations retain a conservative, explainable fallback.

## R8: Add cache-cost routing and exact wire compression

These are separate changes with separate acceptance results:

- **Cache routing:** extend whole-request placement with bounded, short-lived
  evidence for queue wait, restore time, and suffix-prefill cost. Preserve
  artifact, tokenizer/template, backend, tenant, and state compatibility.
  Compare each claimed saving with cold execution and verify the realized
  hit. A cache advertisement is a hint, not proof or pinned KV.
- **Exact wire compression:** retain the R4 raw baseline and negotiate a
  precisely described encoding. Select compact formats only when complete
  round-trip reconstruction is exact. Bound decoded allocation and measure
  selection/encoding/decoding/copy time alongside bytes saved.

**Complete when:** malformed or stale evidence cannot bypass placement
eligibility or cache isolation, and an evicted hit falls back correctly.
Compression preserves qualified numerical results and improves the relevant
end-to-end cost. Lossy formats need a distinct mode, quality evaluation, and
cache identity; they are not an extension of the exact-output claim.

## R9: Extend fixed-length speculative verification

**Scope:** retain CPU Qwen3.8 MTP as the greedy correctness reference. Add
multi-token target verification using a qualified batched execution path and
compatible draft weights. Compare speculation off with fixed draft lengths
before adding adaptive verification. Count committed output tokens and all
draft, target, sampling, snapshot, rollback, and synchronization costs.

**Complete when:** the fixed-length path preserves greedy output and state
after accept-all, reject-all, and partial rejection, and improves measured
committed throughput within the declared latency/memory budget. Sampled
speculation additionally needs distribution checks; constrained speculation
needs R2's matcher rollback. Keep unsupported modes explicit.

Confidence-scheduled verification is conditional follow-up work. DeepInfer's
confidence head is model functionality that Psionic's current MTP path does
not acquire by adding a threshold. Historical acceptance is an experimental
heuristic, not an equivalent head. Bind performance curves to artifact,
backend, device, precision, kernel build, and topology before reusing them.

## First changes and validation scope

Start with R0's measurement/qualification extension, then R1's smallest real
CUDA batch and bounded admission. R2's compilation reuse and R4's existing
Gemma boundary qualification are the next independently reviewable changes.
For DeepSeek support, add DS0's distill token/template qualification and DS1's
official Rust protocol adapter, then DS2's native MLA/MoE reference. Keep their
completion claims separate from generic Qwen or Gemma qualification.
Each subsequent change should name its prerequisite result and the exact
behavior it improves; do not turn this roadmap into a single large rewrite.

Run targeted formatting and tests for edited crates from the separate
`crates/psionic` workspace, using the repository's shared-target and compiler
state setup. Reuse scheduler, sampling, binary-frame, and backend parity tests.
New behavior needs focused regression tests; device performance claims need
the corresponding isolated device run. The full release gate is unnecessary
for ordinary steps. This document itself requires link and source checks only.

Record pending device qualification explicitly. Code completion, artifact
materialization, a completed inference response, and a performance win are
different results. Do not describe one as evidence of the others.

Defer approximate bounded SWA replay, lossy activation transport, WAN expert
parallelism, persistent remote KV storage, and new accelerator vendors until
the relevant baseline and workload justify them. In particular, DeepInfer's
V4-specific replay approximation and Mesh's broad family roster do not extend
Psionic's current model or output guarantees.

## Source map

The two reviews supply upstream rationale. These links identify the existing
Psionic implementation to extend; the symbols above are present at the source
baseline, not newly proposed APIs.

[S1]: ../../crates/psionic/crates/psionic-serve/src/openai_http.rs
[S2]: ../../crates/psionic/crates/psionic-runtime/src/lib.rs
[S3]: ../../crates/psionic/crates/psionic-serve/src/lib.rs
[S4]: ../../crates/psionic/crates/psionic-serve/src/qwen35.rs
[S5]: ../../crates/psionic/crates/psionic-runtime/src/structured_output.rs
[S6]: ../../crates/psionic/crates/psionic-serve/src/psion_rvllm_cuda_graph_pool.rs
[S7]: ../../crates/psionic/crates/psionic-serve/src/gguf.rs
[S8]: ../../crates/psionic/crates/psionic-cluster/src/scheduler.rs
[S9]: ../../crates/psionic/crates/psionic-cluster/src/pipeline_sharded.rs
[S10]: ../../crates/psionic/crates/psionic-cluster/src/tensor_sharded.rs
[S11]: ../../crates/psionic/crates/psionic-datastream/src/lib.rs
[S12]: ../../crates/psionic/crates/psionic-net/src/lib.rs
[S13]: ../../crates/psionic/crates/psionic-cluster/src/ordered_state.rs
[S14]: ../../crates/psionic/crates/psionic-models/src/released_chat_templates.rs
[S15]: ../../crates/psionic/crates/psionic-serve/examples/qwen35_cuda_bench.rs
