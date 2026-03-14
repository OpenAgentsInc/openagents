# Psionic

Psionic is the Rust-native compute engine program for OpenAgents.

It is intentionally scoped as a workspace subtree under `crates/psionic/` so the
engine can evolve without bleeding product-specific behavior into shared crates.

## Doc Authority

- `README.md` is the Psionic entrypoint and map.
- `docs/ARCHITECTURE.md` is the canonical Psionic-wide system spec.
- `docs/TRAIN_SYSTEM.md` is the canonical training subsystem spec.
- research audits explain direction and rationale, but they are not the
  authoritative current-state spec.

## What Psionic is

- A tensor, IR, compiler, and runtime stack built in Rust.
- A place to land productized inference and embeddings execution.
- A backend family that can map cleanly into provider capabilities and receipts.
- A foundation for CPU first, then Metal and AMD backends.

## What Psionic is not

- Not a literal line-by-line port of Tinygrad.
- Not an app surface or provider UX layer.
- Not a shortcut around `docs/OWNERSHIP.md`.
- Not a promise that text generation, KV cache, or AMD kernels are already built.

## Crate Map

- `psionic-core`: foundational tensor, shape, dtype, and device types.
- `psionic-ir`: canonical graph and execution-plan representation.
- `psionic-compiler`: lowering and scheduling boundaries over IR.
- `psionic-runtime`: runtime traits for devices, allocation, execution, and
  canonical execution-proof bundles for local, clustered, and sandbox lanes,
  plus activation-fingerprint proof adapters for embeddings-first proof
  posture.
- `psionic-sandbox`: bounded sandbox runtime detection, profile realization, and execution receipts.
- `psionic-net`: peer identity, direct/NAT/relay session establishment, durable trust and candidate history, relay-backed rendezvous, policy-gated HTTP service tunnels, and transport observations.
- `psionic-datastream`: resumable dataset/checkpoint manifests, chunk transport, and delivery receipts.
- `psionic-cluster`: durable ordered-state, admission policy, catch-up, scheduling, and topology substrate over `psionic-net`.
- `psionic-collectives`: elastic device-mesh and benchmark-gated collective planning for training-class lanes.
- `psionic-train`: training-session truth for async checkpointing, live recovery, and elastic membership on top of ordered cluster state and datastream manifests.
- `psionic-adapters`: LoRA and adapter package identity, packaging manifests, and adapter-serving bindings for hosted products.
- `psionic-models`: reusable model definitions and metadata.
- `psionic-serve`: request/response and execution interfaces for served products.
- `psionic-router`: multi-model routing inventory, capability filters, and worker-path selection for served Psionic fleets.
- `psionic-provider`: capability, readiness, and receipt-facing types.
- `psionic-backend-cpu`: CPU reference backend.
- `psionic-backend-metal`: Metal backend with a first embeddings product path.
- `psionic-backend-amd-kfd`: AMD KFD discovery/readiness backend.
- `psionic-backend-amd-userspace`: AMD userspace discovery/readiness backend.
- `psionic-apple-fm`: Apple Foundation Models bridge contracts, HTTP client, and types for the Swift sidecar.

The crate list and layering are canonical for current ownership and dependency
direction, but they are not a guarantee that every planned subsystem will land
under exactly these final crate names.

## Design Principles

- Keep the compiler and runtime visible and inspectable.
- Keep crate ownership narrow and documented.
- Preserve a strict boundary between reusable engine crates and OpenAgents provider
  integration.
- Model backend families explicitly; AMD KFD and AMD userspace are separate
  backends, not one hidden toggle.
- Keep inference and embeddings first-class in architecture from the start.

## Current Phase

Psionic is in an implemented-substrate, not-yet-complete-engine phase.

Implemented now:

- CPU baseline plus a first Metal-backed `psionic.embeddings` lane.
- generic CPU GGUF decoder execution for GPT-OSS plus representative Llama,
  Qwen, and Mistral families through one Psionic-owned runtime surface.
- generic `psionic-openai-server` boot and model inventory for GPT-OSS plus
  non-GPT-OSS GGUF families on one `/v1/chat/completions` surface, plus
  safetensors-backed embeddings on `/v1/embeddings` and a first Psionic-owned
  `/v1/responses` surface, with per-model endpoint support reported explicitly.
- a first explicit non-GPT-OSS generic-server pilot for the Qwen family, with a
  dedicated end-to-end runbook and harness proving family inventory, scheduler
  headers, and scheduler receipts survive the same Psionic-owned runtime and
  server path as GPT-OSS.
- a first integrated structured-agent weather pilot, proving structured JSON
  output, response-state continuation, router-owned tool loops, and cache or
  route truth together in one Psionic-owned workload.
- explicit CPU-lane residency, fallback, and unsupported-control truth on that
  generic server surface instead of vague accelerator claims.
- explicit local-backend truth on the GPT-OSS server surface too, including
  native Metal, native CUDA, and explicit `llama.cpp` proxy posture with
  machine-checkable hybrid-offload labels instead of silent proxy or hybrid
  claims.
- Psionic-owned structured-output contracts on the generic server for choice,
  regex, grammar, `json_object`, `json_schema`, and tagged-structure cases via
  one shared request shape, explicit per-model capability reporting, response
  headers, and machine-readable structured values instead of hidden
  prompt-only conventions or string re-parsing.
- Psionic-owned tool-calling contracts on the generic server via `tools` plus
  `tool_choice`, with explicit `none` / `auto` / `required` / named modes,
  tagged tool envelopes, schema-backed argument validation, and
  machine-readable tool-call surfaces on both normal and streaming chat
  responses.
- a router-owned tool-loop boundary for those tool calls, with explicit
  multi-step model/tool receipts, provider descriptors, MCP-aware gateway
  seams, history-visibility controls, and refusal of hidden tool results
  instead of burying agent loops inside worker runtimes or app-local glue.
- Psionic-owned reasoning parser seams for reasoning-bearing families, starting
  with GPT-OSS / Harmony: typed parsed-response envelopes now separate final
  content, reasoning content, and side channels; `psionic_reasoning` request
  policy can explicitly separate or suppress reasoning; and both chat plus
  responses surfaces can return typed reasoning-aware response fields without
  falling back to raw-string scraping alone.
- Psionic-owned response-state and conversation contracts on `/v1/responses`,
  with router-owned pluggable in-memory or JSON-file backends, explicit
  response and conversation identifiers, truthful prompt-replay-only cache
  behavior, restart-safe local continuation on durable backends, per-model
  capability reporting, and explicit refusal for unsupported continuation
  modes instead of pushing multi-turn state emulation into callers.
- a first Psionic-owned router control plane for served fleets, with explicit
  worker/model inventory, capability filters, warm/cache-aware placement,
  bounded power-of-two least-loaded choice over warm or cache-matched pools,
  and generic-server route headers so model routing no longer lives as ad hoc
  alias logic inside `psionic-serve`.
- router-owned reliability controls for served fleets, with explicit queue
  depth, retry/refusal traces, rate-limit actions, circuit-breaker state, and
  health gating in `psionic-router` instead of app-specific failure handling.
- a first truthful adapter-serving lane for dense CPU GGUF decoder families,
  with LM-head LoRA import from safetensors, explicit attach/detach plus
  merge/unmerge residency modes, adapter compatibility/refusal surfaces, and
  real adapter-backed generation instead of metadata-only parsing or silent
  fallback to the base model.
- a first Psionic-owned continuous-batching scheduler for CPU text generation,
  with mixed prefill/decode admission, FIFO queue truth, per-request scheduling
  receipts, and generic-server execution headers instead of a hard-coded
  `single_request_only` posture on the shared local server lane.
- a real request-owned block/paged KV manager behind that scheduler, with page
  allocation, reclaim, eviction, session/request/shared-prefix owner bindings,
  and explicit KV ownership receipts across CPU and GPT-OSS execution paths.
- automatic shared prefix caching on top of that KV substrate, with explicit
  tenant/session and sampler boundaries, request-level auto/bypass/invalidate
  controls, refusal/invalidation receipts, and generic-server headers for
  prefix hit/miss/bypass truth.
- Psionic-owned prefill/decode capability contracts on top of that scheduler
  and KV substrate, with colocated and KV-transfer handoff seams, separate TTFT
  and ITL metrics, scheduler receipts, and generic-server headers that surface
  the realized prefill/decode mode instead of treating PD behavior as hidden
  runtime detail.
- hierarchical KV residency accounting across host, device, and explicit
  datastream-backed distributed tiers, with spill/prefetch/write-back movement
  truth, refusal surfaces, and cluster cache-capability reporting that only
  claims the tiers the lane can actually surface.
- one canonical serving-semantics model shared across local and clustered
  serving, with execution-profile, cache, and warm-route truth surfaced on
  whole-request, replica-routed, pipeline-sharded, layer-sharded, and
  tensor-sharded evidence paths.
- `psionic-net` direct, NAT, and relay session establishment.
- `psionic-cluster` ordered state, admission, catch-up, and clustered serving
  topology truth across replica, pipeline, layer-sharded, and tensor-sharded
  variants.
- sharded-model manifests, staged artifact residency, and clustered prefix or
  KV-cache compatibility truth.
- `psionic-datastream` resumable dataset and checkpoint delivery, now including
  explicit checkpoint-backed KV external locator contracts for distributed cache
  tiers.
- benchmark-backed quantization dispatch plus low-level batching and parking
  hooks used by serve and datastream layers.
- `psionic-sandbox` runtime detection, bounded execution, background jobs, and
  file-transfer lifecycle.
- canonical execution-proof bundles and embeddings-first activation-fingerprint
  proof posture.
- early train substrate: checkpoint-backed recovery, elastic membership,
  collective planning, typed fixed-budget trainer steps, checkpoint-anchored
  restore, and adapter lineage.
- broader-stack authority flows for environment packages, eval runs, and
  synthetic-data jobs now exist outside Psionic in kernel or Nexus surfaces.

Still planned:

- full inference-engine maturity across model families and broader serving
  surfaces.
- Psionic-native environment and eval runtime crates.
- broader distributed training completion, rollout artifacts, and orchestrator
  layers.
- training-window protocol, checkpoint pointer/manifest discipline, and
  validator-owned benchmark packages for training-class lanes.
- policy-meaningful runtime and environment manifests plus proof-bearing
  session-claims discipline for clustered and sandboxed execution.
- AMD execution support.

For canonical current-state detail, use `docs/ARCHITECTURE.md` and
`docs/TRAIN_SYSTEM.md` rather than treating this README as the full system spec.

## Docs

- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — canonical Psionic-wide
  system spec covering layering, work classes, artifact and receipt model,
  execution lifecycle, failure, and security boundaries.
- **[docs/TRAIN_SYSTEM.md](docs/TRAIN_SYSTEM.md)** — canonical training
  subsystem spec covering current substrate, planned architecture, object
  model, receipts, policy surfaces, and the issue-program path to a full
  Rust-native train stack, now tracked as GitHub issues `#3564` through
  `#3593`.
- **[docs/TRAINING_CORE_FIXED_BUDGET_REFERENCE.md](docs/TRAINING_CORE_FIXED_BUDGET_REFERENCE.md)** —
  canonical reference loop, runbook, and acceptance criteria for the first
  real `psionic-train` fixed-budget training-core path.
- **[docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md](docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md)** —
  canonical source split, owner matrix, completion matrix, and issue-program
  authority for the current `PSI-232` through `PSI-258` inference backlog.
- **[docs/TOPOLOGY_ACCEPTANCE_MATRIX.md](docs/TOPOLOGY_ACCEPTANCE_MATRIX.md)** —
  canonical support matrix and runnable validation entrypoint for local and
  clustered serving topologies, including `DP`, `PP`, `TP`, `PD`, explicit
  refusal boundaries, and current expert-parallel non-support.
- **[docs/PRODUCT_CLASS_ACCEPTANCE_MATRICES.md](docs/PRODUCT_CLASS_ACCEPTANCE_MATRICES.md)** —
  canonical split between local portability, high-throughput serving, and
  structured-agent acceptance, plus the runnable category harness that keeps
  those product claims from collapsing into one benchmark headline.
- **[docs/NON_GPT_OSS_QWEN_PILOT.md](docs/NON_GPT_OSS_QWEN_PILOT.md)** —
  canonical first non-GPT-OSS generic-server pilot, including the Qwen runbook,
  pass criteria, expected signals, and current limitations.
- **[docs/STRUCTURED_AGENT_WEATHER_PILOT.md](docs/STRUCTURED_AGENT_WEATHER_PILOT.md)** —
  canonical integrated structured-agent workload pilot, including the weather
  runbook, pass criteria, expected signals, and bounded current scope.
- **[docs/FM_BRIDGE_CONSIDERATIONS.md](docs/FM_BRIDGE_CONSIDERATIONS.md)** — Apple Foundation Models bridge: architecture, binary discovery, build, run, test, shipping, and user requirements in full detail.
- **[docs/ACTIVATION_FINGERPRINT_PROOFS.md](docs/ACTIVATION_FINGERPRINT_PROOFS.md)** — activation-fingerprint proof posture, embeddings-first artifact generation, and benchmark semantics.
- **[docs/ROADMAP_FM.md](docs/ROADMAP_FM.md)** — Apple FM lane roadmap and API coverage.
- Other planning and reference docs live under `crates/psionic/docs/`.
