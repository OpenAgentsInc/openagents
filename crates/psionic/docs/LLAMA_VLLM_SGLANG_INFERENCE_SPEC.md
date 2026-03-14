# Psionic Inference Spec From `llama.cpp`, `vLLM`, and `SGLang`

> Status: canonical `PSI-232` source split and completion-matrix spec, updated
> 2026-03-14 after re-verifying the live `PSI-232` through `PSI-258` GitHub
> issue state against this repo.
>
> Baseline assumption: the generic Psionic cutover through `PSI-178` and
> `OA-203` is already landed on `main`, the GPT-OSS enablement path through
> `PSI-183` is already landed on `main`, clustered serving foundations through
> `ROADMAP_CLUSTER.md` are already landed on `main`, and the current open
> backend-specific queues remain the Apple Metal GPT-OSS completion chain
> `#3270` / `#3268` / `#3269` / `#3271` / `#3272` / `#3261` / `#3262` plus the
> workload-specific NVIDIA 120B throughput queue `#3345` -> `#3360`.
>
> Scope note: this is a Psionic-owned longer-horizon inference and serving
> specification. It does not change the current MVP product authority in
> `docs/MVP.md`, and it does not move app or marketplace ownership into
> `crates/psionic/*`.

## Canonical Status And Review Rule

This document is the canonical source for:

- the `llama.cpp` / `vLLM` / `SGLang` reference split
- the implemented Psionic inference baseline through `PSI-183` and
  `ROADMAP_CLUSTER.md`
- the layer-owner matrix for the remaining inference work
- the dependency-ordered `PSI-232` through `PSI-258` issue program
- the completion criteria for claiming a full Rust-native inference and
  agent-serving stack

Future review and issue-closure decisions for `PSI-233` through `PSI-258`
should not treat the three upstream source trees as interchangeable. Each issue
must identify:

- which source layer it is primarily learning from
- which Psionic crates own that work
- which explicit non-goals still apply
- which completion bar in this document it is satisfying

If future work changes the source split, owner split, dependency order, or
definition of done for the inference stack, it must update this document
directly instead of scattering that authority across audits or issue comments.

## Objective

Define the current Psionic inference state and the dependency-ordered path to
finish Psionic into a full Rust-native inference and agent-serving stack by
taking the right lesson from each of these source projects:

- `llama.cpp` for local engine, GGUF, quantization, backend, and portability
  truth
- `vLLM` for scheduler, KV memory, batching, disaggregation, and generic
  serving-engine truth
- `SGLang` for structured generation, parser/runtime semantics, cache-aware
  routing, and agent-serving gateway truth

The target end state is not "pick one of the three and port it."

The target end state is:

- Psionic remains Rust-native and Psionic-owned
- Psionic preserves low-level engine truth instead of hiding behind external
  runtimes
- Psionic gains a real `vLLM`-class scheduler and KV subsystem
- Psionic gains a real `SGLang`-class structured serving and routing layer
- OpenAgents gets one coherent inference stack instead of a mix of narrow local
  engine fragments and ad hoc agent-serving seams

## `PSI-232` Closure Checklist

`PSI-232` is satisfied by this spec only because all of the following are
explicit here in one place:

- the authoritative three-source split:
  - `llama.cpp` for artifact, backend, local-engine, and portable-execution
    truth
  - `vLLM` for scheduler, KV, batching, PD, and generic serving-engine truth
  - `SGLang` for structured serving, parser/runtime, hierarchical KV, routing,
    and gateway truth
- the implemented baseline issue blocks that are already assumed landed
- the current open side queues that remain valid dependencies:
  - the Apple Metal GPT-OSS completion chain
  - the NVIDIA 120B throughput queue
- the owner split by crate family and serving layer
- the explicit non-goals that keep Psionic Rust-native and keep product or
  market authority out of `crates/psionic/*`
- the dependency-ordered completion matrix from `PSI-232` through `PSI-258`
- the final definition of done for the full Rust-native inference stack

The closure bar for this issue is documentation authority, not runtime code.
Later issues in this block own the real engine, scheduler, server, router, and
validation implementation work.

## Related Docs

- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/ROADMAP_CLUSTER.md`
- `crates/psionic/docs/CONFORMANCE_AND_EVIDENCE_CONTRACT.md`
- `crates/psionic/docs/INFERENCE_ENGINE.md`
- `crates/psionic/docs/METAL_GPT_OSS_LLAMA_CPP_LESSONS.md`
- `docs/audits/2026-03-13-rust-inference-engine-gap-audit.md`
- `docs/audits/2026-03-13-llama-vllm-sglang-lessons-for-psionic-audit.md`

## External Reference Inputs

This spec is grounded in the current local copies of these source repos and
their most relevant inference/runtime docs:

### `llama.cpp`

- `README.md`
- `grammars/README.md`

Why these matter here:

- they define the strongest local-engine reference for GGUF, quantization,
  backend closure, OpenAI-compatible local serving, and grammar-constrained
  generation

### `vLLM`

- `README.md`
- `docs/features/structured_outputs.md`
- `docs/features/tool_calling.md`
- `docs/features/disagg_prefill.md`
- `docs/features/automatic_prefix_caching.md`
- `docs/usage/security.md`

Why these matter here:

- they define the clearest reference for the generic serving scheduler,
  paged/block KV ownership, prefix caching, PD disaggregation, and production
  server semantics

### `SGLang`

- `README.md`
- `docs/advanced_features/hicache_design.md`
- `docs/advanced_features/pd_disaggregation.md`
- `docs/advanced_features/sgl_model_gateway.md`
- `sgl-model-gateway/README.md`

Why these matter here:

- they define the clearest reference for structured runtime behavior, parser
  seams, hierarchical KV, and the agent-serving router/gateway layer

## Non-Negotiable Constraints

The integration must preserve these boundaries:

- Psionic remains Rust-first and library-first inside `crates/psionic/*`
- Psionic must not take a required Python runtime dependency for the shipped
  engine, scheduler, or gateway path
- `llama.cpp`, `vLLM`, and `SGLang` remain reference repos and behavior oracles,
  not hidden production backends
- capability, evidence, cache, topology, and routing truth must remain explicit
  and machine-checkable
- `crates/psionic/*` may own reusable inference, routing, and structured
  serving substrate, but must not own:
  - app UX
  - wallet flows
  - compute-market settlement or procurement authority
  - final collateral, claim, or adjudication authority
- any future gateway or router crate under `crates/psionic/*` must be an
  execution and serving control plane, not a product shell and not a market
  authority

In plain terms:

- port the semantics
- do not hide behind their runtimes

## Source Hierarchy

Read the sources in this order:

### 1. Current Psionic truth

Primary authority for what is already implemented:

- `crates/psionic/README.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/ROADMAP_CLUSTER.md`
- `crates/psionic/docs/CONFORMANCE_AND_EVIDENCE_CONTRACT.md`
- current crate code under `crates/psionic/*`

### 2. `llama.cpp`

Primary semantic uses:

- GGUF ingestion and model compatibility
- quantization breadth and portable local execution
- hybrid CPU/GPU offload
- backend-kernel discipline
- grammar / JSON-schema constrained local generation

### 3. `vLLM`

Primary semantic uses:

- continuous batching
- paged/block KV management
- automatic prefix caching
- generic server shape
- disaggregated prefill/decode
- distributed serving topology

### 4. `SGLang`

Primary semantic uses:

- RadixAttention-style cache-aware runtime semantics
- HiCache-style hierarchical KV design
- structured outputs as first-class runtime behavior
- reasoning and tool parser seams
- `/v1/responses` / conversation-state serving
- cache-aware, policy-aware, multi-model routing
- MCP-aware agent-serving gateway behavior

## What Psionic Has Already Implemented

This section answers the first required question directly:

> what is implemented now, and which source project did it mostly come from?

## Current State Summary

Psionic today already has:

- a Rust-owned tensor, IR, compiler, runtime, serve, provider, cluster, and
  backend stack
- a real CUDA GPT-OSS decoder path
- a real but still open Apple-native GPT-OSS path
- real cluster execution foundations: replica routing, pipeline, layer-sharded,
  and tensor-sharded execution
- model, cache, capability, and evidence truth that is much stronger than
  "prototype inference server" status

What it does not yet have is:

- a fully generic multi-family decoder runtime
- a fully productized continuous-batching scheduler
- a fully productized paged/block KV subsystem
- a general Psionic-native server and router layer for agentic workloads

## Implemented Baseline By Roadmap Issue

The closed roadmap work below is the concrete baseline this spec assumes is
already real.

| Implemented issue block | What is already real in Psionic | Why it matters for this spec |
| --- | --- | --- |
| `PSI-110` / `PSI-111` / `PSI-115` / `PSI-116` | GGUF format loading, tokenizer reconstruction, quant block decoding, and paged model blob access | This is the `llama.cpp`-class artifact and quantization substrate that makes a Rust-native engine possible at all. |
| `PSI-112` / `PSI-113` / `PSI-114` / `PSI-118` | decoder-family metadata adapters, embeddings-family adapters, prompt-template extraction, and golden prompt/tokenizer fixtures | Psionic already knows how to understand several model families; the remaining problem is turning that wider compatibility into real executed serving paths. |
| `PSI-121` / `PSI-135` / `PSI-165` | installed-model listing, integrity verification, OCI pull, and Ollama-compatible ingest | Psionic already owns model and artifact lifecycle better than a toy demo server. |
| `PSI-126` / `PSI-156` | KV ownership semantics for text generation plus backend-specific quant kernels | The runtime already has the beginnings of real session and quantized execution truth, even if it lacks a full `vLLM`-class shared scheduler. |
| `PSI-172` / `PSI-173` / `PSI-174` / `PSI-175` | execution-profile truth, topology-plan truth, compile-path evidence, and delivery-proof inputs | The serving engine already exposes machine-checkable runtime truth instead of hand-wavy benchmark claims. |
| `PSI-176` / `PSI-177` / `PSI-178` | reusable sandbox-execution profiles, receipts, and topology-aware substitution truth | This gives Psionic reusable execution/evidence contracts that future routing and agent-serving layers can reuse instead of inventing app-local ones. |
| `PSI-179` / `PSI-180` / `PSI-181` / `PSI-182` / `PSI-183` | GPT-OSS GGUF loading, Harmony prompt/render/parse, real GPT-OSS decoder execution, CUDA kernels, and validated NVIDIA local serving | This is the first real proof that Psionic can own a full inference path without hiding behind `llama.cpp` or Ollama. |
| `ROADMAP_CLUSTER.md` through `PSI-231` | replica routing, pipeline-parallel, layer-sharded, tensor-sharded serving, sharded manifests, cluster cache compatibility, trust publication | This is the strongest current adaptation from Prime/vLLM-class distributed serving ideas, but it still needs a shared generic scheduler and richer serving semantics. |

## Current State By Area

| Area | Current Psionic state | Main source lineage already adapted | Main remaining gap |
| --- | --- | --- | --- |
| Model and artifact ingress | GGUF and Ollama-style artifact ingestion, GGUF decoder metadata for Llama/Qwen/Mistral/GPT-OSS, GGUF embeddings families, OCI ingestion, served-artifact identity, governance, invalidation | mostly `llama.cpp`-class lessons | loader coverage is broader than fully executed decoder coverage |
| Real decoder execution | real GPT-OSS GGUF execution model, Harmony prompt/render/parse, CUDA GPT-OSS serving, CPU GGUF GPT-OSS service in code, partial Metal GPT-OSS service | mostly `llama.cpp` plus Psionic-owned CUDA work | still too GPT-OSS-centric; generic real-GGUF decoder execution is not yet productized |
| Local server surface | narrow GPT-OSS OpenAI-compatible server with `/health`, `/v1/models`, `/v1/chat/completions`; generic serve library has generation and embeddings runtime concepts | mostly `llama.cpp`-style local serving shape | not yet one generic Psionic-native server for many families and APIs |
| Scheduler and queue semantics | execution-profile truth, queue-policy truth, throughput-class truth, single-request local generation, caller-static-batch embeddings, seeded sampler, warm/load/unload lifecycle | adjacent to `vLLM` concepts, but mostly Psionic-owned schema work so far | no shared scheduler batching, no continuous batching, no mixed prefill/decode runtime |
| KV and cache substrate | paged-KV policy types, prefix-cache accounting, cache invalidation policies, cluster prefix/KV compatibility truth | partial `vLLM`-style and cluster groundwork | no full `vLLM`-class block KV manager or `SGLang`-class hierarchical KV runtime |
| Distributed serving topology | selected-device truth, topology plans, replica serving, pipeline sharding, layer sharding, tensor sharding, sharded-model manifests | mostly `vLLM`-class and Prime-adjacent serving-topology lessons | local scheduler and clustered scheduler are still not one fully unified product path |
| Structured outputs and tools | no full structured-output backend, no general tool-calling contract, no reasoning-parser registry, no `/v1/responses` runtime | essentially not yet adapted from `vLLM`/`SGLang` in a shipped way | this is one of the biggest missing layers |
| Routing and gateway | cluster placement and readiness exist; no dedicated multi-model router/gateway, no cache-aware request router, no gateway-owned conversation/response state | mostly not yet adapted from `SGLang` | the full agent-serving control plane does not exist yet |
| Reliability and operator controls | backend health, runtime observability, cluster evidence, capability envelopes, plan-cache evidence, sandbox execution profiles | Psionic-owned foundation, with some lessons adjacent to `SGLang` gateway ops | missing router-level retries, rate limiting, queues, circuit breakers, and model-fleet control |
| Validation | strong GPT-OSS/NVIDIA parity and benchmark path, strong provider/evidence truth, open Apple completion queue, no non-GPT-OSS served-family pilot | mostly `llama.cpp` as behavior oracle | acceptance is still too GPT-OSS- and backend-specific |

## What Is Already Adapted From `llama.cpp`

The following is already meaningfully adapted:

- GGUF as the practical artifact boundary
- GGUF decoder-family metadata loading for Llama, Qwen, Mistral, and GPT-OSS
- quantized storage truth for GGUF tensor families including GPT-OSS `MXFP4`
  and `Q8_0`
- Harmony prompt rendering and GPT-OSS parse behavior
- a real GPT-OSS decoder model instead of routing real models through the toy
  fixture decoder
- Psionic-owned CUDA GPT-OSS kernels and a validated local server path
- local-server framing and direct comparison discipline against `llama.cpp`
- ongoing Metal lesson capture in
  `crates/psionic/docs/METAL_GPT_OSS_LLAMA_CPP_LESSONS.md`

The following `llama.cpp` lessons are not fully adapted yet:

- universal local grammar / JSON-schema constrained generation
- a more fully productized CPU and hybrid-offload serving lane
- broad family-by-family local execution parity beyond GPT-OSS
- full backend closure on Apple without proxy ambiguity

## What Is Already Adapted From `vLLM`

The following is already partially adapted:

- explicit execution profiles and queue-policy truth
- selected-device and topology-plan truth
- replica, pipeline, layer, and tensor sharding semantics
- prefix/KV cache compatibility and invalidation truth
- sharded-model manifest semantics
- compile-plan caching and warm/cold evidence

The following `vLLM` lessons are not fully adapted yet:

- real continuous batching
- real block/paged KV runtime
- automatic prefix caching under a shared scheduler
- generic high-throughput multi-family serving under one server
- disaggregated prefill/decode as a real productized runtime

## What Is Already Adapted From `SGLang`

The distinctive `SGLang` lessons are mostly not yet landed in Psionic.

What exists today is mostly only adjacent groundwork:

- cluster routing and topology substrate
- response and parse-carrying output surfaces for GPT-OSS/Harmony
- capability and observability contracts that could support a richer gateway

What is still missing from the `SGLang` side is the important part:

- RadixAttention-style cache-aware request semantics
- HiCache-class hierarchical KV runtime
- structured outputs as first-class serving behavior
- reasoning-parser and tool-parser registries
- `/v1/responses` and conversation-state serving
- cache-aware, policy-aware, multi-model routing
- MCP-aware agent-serving loops
- gateway-level retries, queues, circuit breakers, and history boundaries

## Required Adaptation Matrix From The Three Sources

This is the direct answer to "what must be implemented from those three?"

| Source | What Psionic should take | What Psionic should not cargo-cult |
| --- | --- | --- |
| `llama.cpp` | GGUF compatibility, quantization breadth, portable local execution, hybrid offload truth, grammar-constrained local generation, simple embeddable local server discipline | C/C++ code paths as runtime dependencies, `llama.cpp`-specific APIs as Psionic's public contract, backend behavior that hides fallback or proxy execution |
| `vLLM` | continuous batching, block/paged KV ownership, automatic prefix caching, disaggregated prefill/decode, high-throughput generic server semantics, topology-aware serving scheduler | Python runtime assumptions, direct adoption of `vLLM` internals, weight-centric API shapes that bypass Psionic artifact and evidence contracts |
| `SGLang` | structured outputs, reasoning/tool parser seams, response-stateful runtime, hierarchical KV, cache-aware routing, model gateway semantics, MCP-aware agent-serving boundary | tying the router to a Python gateway process, adopting OpenAI-ish agent semantics without explicit Psionic-owned truth, collapsing worker runtime and control plane into one opaque process |

## What Psionic Must Build Next

The target architecture should be treated as three stacked layers.

## Layer 1: Engine, Artifacts, and Backend Truth

Primary references:

- `llama.cpp`

Owned by:

- `psionic-models`
- `psionic-core`
- `psionic-compiler`
- `psionic-runtime`
- backend crates

This layer must own:

- GGUF and model artifact compatibility
- tokenizer and prompt-template compatibility
- quantization truth and backend execution truth
- local and hybrid execution correctness
- backend-specific kernel quality and offload posture

Exit condition for this layer:

- Psionic can run at least one additional real decoder family beyond GPT-OSS
  through a Psionic-owned path without hiding behind foreign runtimes

## Layer 2: Serving Runtime, Scheduler, and KV System

Primary references:

- `vLLM`
- selected `SGLang` runtime lessons

Owned by:

- `psionic-runtime`
- `psionic-serve`
- `psionic-cluster`

This layer must own:

- shared request scheduler
- continuous batching
- mixed prefill/decode queueing
- block/paged KV memory management
- automatic prefix caching
- disaggregated prefill/decode execution
- serving topology planning for TP/PP/DP/EP

Exit condition for this layer:

- Psionic can honestly claim a generic serving engine rather than only a strong
  model-specific runtime

## Layer 3: Structured Serving, Routing, and Agent Runtime

Primary references:

- `SGLang`
- selected `vLLM` API features

Owned by:

- `psionic-serve`
- future `psionic-router`

This layer must own:

- generic model-fleet server surfaces
- structured outputs
- tool-calling contract and parser registry
- reasoning parser seam
- `/v1/responses` and conversation-state runtime
- cache-aware and policy-aware request routing
- model-fleet reliability controls
- MCP-aware tool loops

Exit condition for this layer:

- Psionic can honestly serve OpenAgents-style multi-turn, structured, tool-using
  workloads without relying on app-local glue or external gateways

## Proposed Crate Expansion

This spec does not require an immediate crate explosion, but the intended owner
split should be explicit:

- `psionic-models`
  - widen to cover real executed decoder families beyond GPT-OSS
- `psionic-runtime`
  - own the shared batching scheduler, block/paged KV manager, automatic
    prefix caching, and hierarchical KV machinery
- `psionic-serve`
  - own generic request/response contracts, structured outputs, tool-calling,
    reasoning outputs, and generic server surfaces
- `psionic-cluster`
  - own topology-aware distributed serving execution and PD routing substrate
- `psionic-router` (new)
  - own multi-model fleet routing, cache-aware request placement, conversation
    and response state, reliability controls, and MCP-aware execution loops

This new crate must remain a reusable serving control plane. It must not become
the buyer/provider product shell or the market authority layer.

## Existing Issue Dependencies That Must Stay In Flight

These current open issues remain valid and should be treated as dependencies or
parallel side queues rather than replaced:

### Apple backend completion

- `METAL-GPT-OSS-1` / `#3270`
- `METAL-GPT-OSS-2` / `#3268`
- `METAL-GPT-OSS-3` / `#3269`
- `METAL-GPT-OSS-4` / `#3271`
- `METAL-GPT-OSS-5` / `#3272`
- `METAL-GPT-OSS-6` / `#3261`
- `METAL-GPT-OSS-7` / `#3262`

These are part of the larger local-backend closure work and should be absorbed
under the broader portable-local completion phase in this spec.

### Workload-specific NVIDIA throughput

- `GPT-OSS-120B-PERF-2` / `#3345`
- `GPT-OSS-120B-PERF-3` / `#3360`

These remain valid workload-specific performance work, but they are not the
same thing as completing the generic serving engine or agent-serving layer.

## Dependency-Ordered Path

The shortest honest path is:

### Phase 0: Keep the current backend and cluster truths green

Do not regress:

- `PSI-172` execution-profile truth
- `PSI-173` topology-plan truth
- `PSI-174` compile-path and plan-cache evidence
- `PSI-179` through `PSI-183` GPT-OSS local path
- `ROADMAP_CLUSTER.md` replicated/pipeline/layer/tensor serving truths
- open Metal GPT-OSS queue

This phase has no new broad feature work. It is a guardrail phase.

### Phase 1: Generalize the engine beyond GPT-OSS

Primary reference:

- `llama.cpp`

Goal:

- move from one highly developed decoder family to a reusable multi-family
  execution substrate

Exit condition:

- at least one additional real decoder family serves through Psionic-owned
  runtime paths

### Phase 2: Finish the serving runtime and scheduler

Primary reference:

- `vLLM`

Goal:

- move from execution-profile truth into a real shared scheduler and KV system

Exit condition:

- local and clustered serving no longer effectively depend on
  `single_request_only` execution for text generation

### Phase 3: Add structured generation and tool/runtime semantics

Primary reference:

- `SGLang`
- selected `vLLM` structured/tool features

Goal:

- move from text-generation substrate into an actual structured serving runtime

Exit condition:

- Psionic can serve structured outputs, tool calls, and multi-turn response
  state without app-local special casing

### Phase 4: Add the router/gateway layer

Primary reference:

- `SGLang`

Goal:

- separate worker execution from request routing, policies, response-state
  control, and agent-serving orchestration

Exit condition:

- OpenAgents can route multi-model, cache-aware, tool-using traffic through a
  Psionic-owned control plane

### Phase 5: Validate the three product classes separately

Goal:

- stop mixing "local engine parity", "high-throughput serving", and "agentic
  structured runtime" into one benchmark claim

Exit condition:

- Psionic has explicit acceptance matrices for:
  - local and portable execution
  - high-throughput serving
  - structured and agentic serving

## Proposed GitHub Issue Program

These issues are the concrete path from the current state to the target state.

The numbering below assumes the current roadmap block ends at `PSI-231`.

### Phase 1: Engine Generalization

Implemented now: `PSI-233` is materially landed in-tree through the generic
`CpuGgufTextGenerationService` surface plus a reusable `GgufRuntimeTokenizer`
and real CPU execution paths for representative Llama, Qwen, and Mistral GGUF
decoder families. That work keeps GPT-OSS support intact while removing the
old assumption that Psionic's non-GPT-OSS decoder-family adapters are only
metadata.

Implemented now: `PSI-234` is materially landed in-tree through the generic
`OpenAiCompatServer` plus the shipped `psionic-openai-server` binary. That path
can front GPT-OSS and representative non-GPT-OSS GGUF families through one
Psionic-owned `/v1/chat/completions` surface, exposes explicit model inventory,
and still refuses unfinished APIs such as `/v1/embeddings` instead of
pretending the broader serving plane is already complete.

| Local ID | Proposed GitHub issue title | Scope | Primary reference | Description | Depends on |
| --- | --- | --- | --- | --- | --- |
| `PSI-232` | Psionic Inference: codify the `llama.cpp` / `vLLM` / `SGLang` source split and completion matrix | docs plus runtime/serve boundary docs | mixed | Freeze the reference hierarchy for Psionic inference work so future issues are judged against the right source at the right layer instead of treating all three repos as substitutes. | current docs only |
| `PSI-233` | Psionic Models: promote GGUF decoder-family support from metadata into real executed Llama, Qwen, and Mistral runtimes | `psionic-models`, `psionic-runtime`, `psionic-serve`, backend crates | `llama.cpp` | Turn today's broader GGUF metadata support into real decoder execution support so Psionic stops being effectively GPT-OSS-first at the execution layer. | `PSI-232`, current GPT-OSS path |
| `PSI-234` | Psionic Serve: ship one generic multi-family server instead of a GPT-OSS-specific HTTP lane | `psionic-serve`, `psionic-models` | `llama.cpp` plus Psionic-owned serve contracts | Replace the current narrow GPT-OSS OpenAI server with a generic Psionic-owned server that can front multiple decoder families and embeddings without model-family-specific binaries. | `PSI-233` |
| `PSI-235` | Psionic Local: expose the real CPU GGUF decoder lane and hybrid-offload truth through the shipped server | `psionic-serve`, `psionic-runtime`, backend crates | `llama.cpp` | Close the current gap where CPU GGUF serving exists in code but is not a first-class shipped server lane, and make hybrid residency / partial offload explicit where it is truthful. | `PSI-234` |
| `PSI-236` | Psionic Local: add universal grammar and JSON-schema constrained generation fallback | `psionic-serve`, `psionic-runtime` | `llama.cpp` | Rebuild the strongest local constrained-generation lesson from `llama.cpp` so structured output survives on the smallest local serving paths instead of depending only on heavyweight runtime backends. | `PSI-234` |

### Phase 2: Scheduler And KV Completion

| Local ID | Proposed GitHub issue title | Scope | Primary reference | Description | Depends on |
| --- | --- | --- | --- | --- | --- |
| `PSI-237` | Psionic Runtime: add continuous batching and mixed prefill/decode scheduling | `psionic-runtime`, `psionic-serve` | `vLLM` | Upgrade local text generation from the current single-request posture into a real shared scheduler that can batch compatible requests and keep queue truth explicit. | `PSI-234` |
| `PSI-238` | Psionic Runtime: add a real block/paged KV manager with request-owned accounting | `psionic-runtime`, backend crates | `vLLM` | Move from policy schema into a real KV subsystem that owns page allocation, growth, reclaim, and request-level KV accounting under a shared scheduler. | `PSI-237` |
| `PSI-239` | Psionic Runtime: add automatic prefix caching with explicit tenancy and invalidation boundaries | `psionic-runtime`, `psionic-serve`, `psionic-models` | `vLLM` | Borrow the best `vLLM` APC lesson while preserving Psionic's served-artifact identity and cache invalidation truth. | `PSI-238` |
| `PSI-240` | Psionic Runtime: add disaggregated prefill/decode execution and KV-transfer seams | `psionic-runtime`, `psionic-cluster`, `psionic-serve` | `vLLM` plus `SGLang` | Rebuild the useful `vLLM` and `SGLang` PD idea so TTFT and ITL can be tuned separately and routed explicitly. | `PSI-237`, `PSI-238` |
| `PSI-241` | Psionic Runtime: add hierarchical KV residency across device, host, and distributed storage | `psionic-runtime`, `psionic-cluster`, `psionic-datastream` | `SGLang` | Turn today's cache schemas into a true `SGLang` HiCache-class hierarchical runtime with explicit spill, prefetch, and write-back policies. | `PSI-238`, `PSI-240` |
| `PSI-242` | Psionic Cluster: unify local and clustered scheduler semantics for batching, cache, and warm-route truth | `psionic-cluster`, `psionic-runtime`, `psionic-serve` | `vLLM` plus `SGLang` | Ensure clustered serving does not drift into a separate scheduling product with different cache and queue semantics than the local engine. | `PSI-237` through `PSI-241`, current cluster foundation |

### Phase 3: Structured Serving Runtime

| Local ID | Proposed GitHub issue title | Scope | Primary reference | Description | Depends on |
| --- | --- | --- | --- | --- | --- |
| `PSI-243` | Psionic Serve: add `/v1/embeddings`, `/v1/responses`, and a broader generic serving surface | `psionic-serve` | `vLLM` plus `SGLang` | Bring the server beyond `/v1/chat/completions` so the serving plane can honestly support embeddings and response-oriented flows under one generic runtime. | `PSI-234` |
| `PSI-244` | Psionic Serve: add structured outputs for choice, regex, JSON schema, grammar, and tagged structure | `psionic-serve`, `psionic-runtime` | `SGLang` plus `vLLM` plus `llama.cpp` | Treat constrained generation as a first-class capability instead of a narrow local-only fallback or ad hoc prompt convention. | `PSI-243`, `PSI-236` |
| `PSI-245` | Psionic Serve: add named, auto, required, and none tool-calling modes with parser-backed validation | `psionic-serve` | `vLLM` plus `SGLang` | Rebuild the strongest `vLLM` tool-calling contract shape while keeping the parser and runtime ownership inside Psionic. | `PSI-243`, `PSI-244` |
| `PSI-246` | Psionic Serve: add reasoning-parser seam and explicit reasoning/content separation | `psionic-serve`, `psionic-models` | `SGLang` | Rebuild the `SGLang` lesson that reasoning-bearing models need explicit parser handling and response fields instead of stringly typed conventions. | `PSI-243`, `PSI-244` |
| `PSI-247` | Psionic Serve: add response-state and conversation contracts for multi-turn agent loops | `psionic-serve` | `SGLang` | Move beyond stateless repeated chat-completions by giving Psionic explicit response-stateful and conversation-stateful runtime contracts. | `PSI-243`, `PSI-245`, `PSI-246` |
| `PSI-248` | Psionic Adapters: add LoRA and adapter import, merge/unmerge, batching, and hosted serving | `psionic-models`, `psionic-serve`, backend crates | `SGLang` plus `llama.cpp` | Replace today's explicit adapter refusal posture with a truthful Rust-native adapter runtime shaped by `SGLang` multi-LoRA and `llama.cpp` adapter ecosystem lessons. | `PSI-233`, `PSI-244`, `PSI-245` |

### Phase 4: Router And Gateway Layer

| Local ID | Proposed GitHub issue title | Scope | Primary reference | Description | Depends on |
| --- | --- | --- | --- | --- | --- |
| `PSI-249` | Psionic Router: introduce a dedicated multi-model routing and policy crate | new `psionic-router` crate plus `psionic-serve` | `SGLang` | Create a dedicated reusable serving control plane for multi-model fleets instead of stretching the worker runtime to also be the full router/gateway product. | `PSI-243`, `PSI-247` |
| `PSI-250` | Psionic Router: add cache-aware, warm-aware, and power-of-two request placement policies | `psionic-router`, `psionic-cluster` | `SGLang` | Rebuild the strongest `SGLang` gateway policy lessons so requests can be placed using warm-state, cache reuse, and model-specific routing policy instead of flat round-robin behavior. | `PSI-249`, `PSI-239`, `PSI-242` |
| `PSI-251` | Psionic Router: add retries, circuit breakers, rate limiting, request queues, and worker health control | `psionic-router` | `SGLang` | Bring the operator-facing reliability primitives that `SGLang` gateway makes explicit into a Psionic-owned router layer. | `PSI-249` |
| `PSI-252` | Psionic Router: add pluggable response and conversation storage backends | `psionic-router` | `SGLang` | Give response-stateful and conversation-stateful flows a reusable persistence boundary that can remain inside the serving control plane rather than leaking into app-local glue. | `PSI-247`, `PSI-249` |
| `PSI-253` | Psionic Router: add MCP-aware tool loop execution boundary | `psionic-router`, `psionic-serve` | `SGLang` | Rebuild the strongest `SGLang` lesson for OpenAgents-style agent serving: tools, history, and multi-model loops need a gateway boundary that is not the worker itself. | `PSI-245`, `PSI-247`, `PSI-249` |

### Phase 5: Backend Closure And Validation

| Local ID | Proposed GitHub issue title | Scope | Primary reference | Description | Depends on |
| --- | --- | --- | --- | --- | --- |
| `PSI-254` | Psionic Local: finish portable local backend closure across CPU, Metal, CUDA, and hybrid offload | backend crates, `psionic-runtime`, `psionic-serve` | `llama.cpp` | Fold the open Apple chain plus CPU/hybrid-offload truth into one local-execution closure issue so local serving remains a first-class product rather than a side lane. | `PSI-235`, `METAL-GPT-OSS-1` through `METAL-GPT-OSS-7` |
| `PSI-255` | Psionic Cluster: add acceptance matrix for TP/PP/DP/EP plus PD execution across local and clustered serving | `psionic-cluster`, `psionic-runtime`, docs/tests | `vLLM` plus `SGLang` | Convert topology truth into actual validation matrices that prove scheduler, routing, cache, and artifact behavior under the serving topologies Psionic claims. | `PSI-240`, `PSI-242`, `PSI-249`, `PSI-250` |
| `PSI-256` | Psionic Benchmarks: define separate acceptance matrices for local portability, high-throughput serving, and structured agent workloads | docs/tests/benchmarks plus serving stack | mixed | Stop collapsing every claim into one benchmark and explicitly separate local-engine, serving-engine, and agent-runtime completion targets. | `PSI-254`, `PSI-255` |
| `PSI-257` | Psionic Pilot: validate one non-GPT-OSS family end to end through the generic server | `psionic-models`, `psionic-serve`, docs/tests | `llama.cpp` plus `vLLM` | Prove that the engine is no longer GPT-OSS-only by running a real Llama, Qwen, or Mistral family through the full generic server path. | `PSI-233`, `PSI-234`, `PSI-256` |
| `PSI-258` | Psionic Pilot: validate one end-to-end agent workload with structured outputs, tool calls, response state, and cache-aware routing | `psionic-router`, `psionic-serve`, docs/tests | `SGLang` plus `vLLM` | Prove the exact workload class that makes `SGLang` relevant to OpenAgents instead of stopping at raw token generation. | `PSI-244` through `PSI-253`, `PSI-256` |

## Recommended Execution Order

The shortest honest dependency order is:

1. `PSI-232`
2. `PSI-233` -> `PSI-236`
3. `PSI-237` -> `PSI-242`
4. `PSI-243` -> `PSI-248`
5. `PSI-249` -> `PSI-253`
6. `PSI-254` -> `PSI-258`

Existing side queues that should continue in parallel:

- `METAL-GPT-OSS-1` through `METAL-GPT-OSS-7`
- `GPT-OSS-120B-PERF-2` and `GPT-OSS-120B-PERF-3`

The important dependency rule is:

- do not wait for `120B` workload closure to start the generic serving stack
- do not claim the generic serving stack is complete until at least one
  non-GPT-OSS family and one structured/tool-using workload both validate end
  to end

## Definition Of Done

Psionic should not claim "full Rust-native inference engine" until all of the
following are true:

- at least two real decoder families beyond the current GPT-OSS-first path run
  through the generic Psionic server
- local text generation no longer honestly reports only
  `single_request_only` execution posture
- the runtime owns a real block/paged KV subsystem plus automatic prefix caching
- the server supports:
  - chat completions
  - embeddings
  - structured outputs
  - tool calling
  - response-stateful flows
- a Psionic-owned router can place and control multi-model requests with
  explicit reliability and cache-aware policies
- Apple, CPU, and CUDA local lanes are all truthful about what is native,
  what is fallback, and what is unsupported
- one non-GPT-OSS family and one structured agent workload both have explicit
  validation receipts

## Bottom Line

If Psionic only keeps learning from `llama.cpp`, it will become a better local
engine but still underinvest in the serving scheduler and agent runtime layers.

If Psionic only keeps learning from `vLLM`, it will improve the serving engine
but still risk treating structured outputs, tools, response state, and router
policy as side features.

If Psionic only keeps learning from `SGLang`, it risks skipping the harder
engine-core discipline around generic scheduler and KV memory design.

The correct path is the layered one:

- `llama.cpp` for the bottom
- `vLLM` for the middle
- `SGLang` for the top

This document is the spec for getting Psionic there.
