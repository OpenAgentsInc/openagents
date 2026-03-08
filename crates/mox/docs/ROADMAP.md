# Mox Roadmap

> Status: updated 2026-03-08 after verifying the current GitHub issue set with
> `gh issue list --state all` / `gh issue view`, after confirming the generic
> Mox replacement track through `MOX-178` and `OA-203` is landed on `main`,
> after landing `MOX-179` / [#3239](https://github.com/OpenAgentsInc/openagents/issues/3239)
> in `780479e23`, after landing `MOX-180` /
> [#3240](https://github.com/OpenAgentsInc/openagents/issues/3240) in
> `1140c7f32`, and after confirming the remaining open GPT-OSS follow-on
> issues are `MOX-181` / [#3237](https://github.com/OpenAgentsInc/openagents/issues/3237),
> `MOX-182` / [#3238](https://github.com/OpenAgentsInc/openagents/issues/3238),
> and `MOX-183` / [#3241](https://github.com/OpenAgentsInc/openagents/issues/3241)
> for the Mox-only GPT-OSS completion track on the NVIDIA host.
>
> This is the live roadmap for `crates/mox/`. The generic phase-2/3/4 and
> desktop-cutover baseline is now merged. The remaining work below is the gap
> between "we have a generic local Rust runtime and app cutover" and "Mox can
> truthfully execute the real GPT-OSS/NVIDIA path without external
> `llama.cpp`, while remaining valid as compute-market substrate."
>
> Host execution note: on this computer, active backend execution and hardware
> validation work is NVIDIA-only. The AMD follow-on issues `MOX-151` through
> `MOX-154` were intentionally closed as not planned and are excluded from the
> active dependency queue unless a future reprioritization reopens them.
>
> GPT-OSS host note: the local
> `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf` file has been
> verified to run on this machine via `~/code/llama.cpp`, but not yet via Mox.
> The remaining open `MOX-181` through `MOX-183` track below is the work still
> needed to make that flow Mox-only.

Agent execution instruction: implement this roadmap one issue at a time in the
recommended dependency order listed here. Determine the next item from the
"Current execution queue" below, not from raw GitHub issue number ordering and
not from historical "selected open issues" batches. When multiple GitHub issues
exist for the same local roadmap ID, use the open detailed issue mapped in this
document and treat the closed duplicates as historical only. For each issue,
complete the full scoped implementation, run the relevant verification, commit
and push that issue's work immediately, comment on the GitHub issue with a
concise summary of what landed when closing it, and then move directly to the
next roadmap item. Do not stop partway through the roadmap unless blocked by a
real external dependency or an explicit user instruction to pause or
reprioritize.

Roadmap hygiene rule: after each completed roadmap issue, update this document
before moving to the next issue so it reflects the new GitHub state, landed
commit link or hash, shipped-status notes, and current execution queue. Commit
and push that roadmap update as well; do not leave the roadmap stale after an
issue lands.

Reference-first implementation rule: for any issue that touches externally
defined semantics such as GGUF/GGML parsing, quantization or block layouts,
tokenizer reconstruction, prompt rendering, sampler behavior, streaming,
catalog behavior, lifecycle semantics, or backend/runtime truth, the agent must
inspect the equivalent implementation and nearby tests in the most relevant
reference tree before coding. Do not implement those paths from memory or from
roadmap wording alone.

Mox-only execution rule: the reference repos listed below are for semantic and
behavioral truth only. They are not acceptable execution shortcuts for this
track. Do not shell out to, proxy through, sidecar against, FFI-wrap, or
otherwise delegate prompt rendering, tokenization, Harmony parsing, sampling,
or model execution to `~/code/llama.cpp` or any other external runtime when
closing roadmap issues in Epic G. The shipped path must execute through Mox
crates themselves, with external repos used only as references and validation
oracles.

Choose the primary reference intentionally:

- start with `~/code/candle` for Rust GGUF/GGML loading, quantized tensor
  storage, quantized block layout/decode rules, tokenizer reconstruction, and
  backend/runtime structure
- start with `~/code/tinygrad` for GGUF decode math cross-checks, KV-cache or
  JIT/runtime-plan behavior, and execution-evidence patterns
- start with `~/code/llama.cpp` for deployed GGUF tensor-type coverage and
  block-layout truth beyond the current Rust crates, GPT-OSS / OpenAI-MoE
  architecture metadata and tensor naming, Harmony prompt/render/parse
  semantics, tokenizer control-token behavior, and local NVIDIA execution
  behavior for the exact GGUFs carried on this host
- start with `~/code/ollama` for API-visible behavior such as prompt
  rendering, BOS/EOS defaults, truncation, streaming, catalog/lifecycle, and
  error semantics

Before coding, compare the planned Mox behavior against the chosen primary
reference and note any intentional deviations. If the reference reveals tricky
ordering, layout, shape, or fallback rules, encode those semantics in tests in
the same issue. If multiple references disagree, follow the source of truth for
the layer being implemented and say which reference won and why in the issue
comment when closing the work.

## Objective

Replace the desktop's external Ollama dependency with an in-process Rust
runtime that:

- keeps app UX and provider orchestration in `apps/autopilot-desktop`
- keeps reusable model, runtime, backend, and serving logic in `crates/mox/*`
- matches the subset of Ollama behavior OpenAgents actually depends on
- remains explicit about backend readiness, fallback, hardware support,
  lifecycle, and evidence

This is not a plan to rebuild all of Ollama.

This is also not enough, by itself, to satisfy the broader compute-market plan.
Mox must become the reusable execution substrate for truthful `inference` and
`embeddings` supply first, with later bounded `sandbox_execution` only if that
family stays explicit and machine-checkable.

## Ownership Rules

The roadmap must keep `docs/OWNERSHIP.md` intact:

- `crates/mox/*` owns reusable tensor, IR, compiler, runtime, model, serve, and
  provider-facing engine truth
- `apps/autopilot-desktop` owns the local runtime adapter, provider UX,
  inventory presentation, admission policy, and final cutover from Ollama HTTP
  calls
- `crates/mox/*` must not absorb app-specific UI or product orchestration

## Tinygrad-Style Rules

Mox should preserve the parts of Tinygrad that matter architecturally without
trying to port Tinygrad line by line:

- keep a small, inspectable primitive op surface
- keep backend crates explicit; discovery, allocation, lowering, execution, and
  health reporting belong to backends
- keep model formats and model stores separate from backend execution
- keep the serving surface library-first and in-process
- treat JIT capture, compile plans, kernel-cache behavior, batching, queueing,
  and topology as explicit runtime policy
- keep quantized tensor storage backend-backed instead of forcing eager
  CPU-only dequantization paths
- keep memory-mapped model blob access and paged tensor storage explicit so
  large local models do not require eager copies
- keep an explicit fused/custom-op escape hatch for backend-specific attention,
  quantized GEMM, and normalization kernels
- treat backend allocator pools, kernel caches, and device-memory budgets as
  explicit runtime policy rather than backend internals
- never silently run on CPU while advertising Metal, AMD, or NVIDIA readiness

The first backend-complete primitive surface still needs to be just large enough
to run the launch `inference` and `embeddings` product paths:

- matmul / batched matmul
- embedding lookup / gather
- reshape / transpose / concat / slice
- cast / dequantize hooks
- elementwise add / mul / silu / gelu
- rmsnorm / layernorm
- RoPE
- softmax
- KV-cache read / append / update

## Shipped On Main

`main` now includes the merged phase-2/3/4 baseline from PR `#3163`
(`53f31280a`) plus the roadmap follow-ups that closed `MOX-110` through
`MOX-126B` (including the `MOX-115` Candle-alignment follow-up) on `main`.
See "Delivered after the merged baseline" below for the shipped scope and the
per-issue commit anchors.

### Delivered in the merged baseline

- artifact-backed bundle ingestion in `mox-models`
- explicit quantization metadata and capability truth
- tested model-backed CPU `mox.embeddings` and `mox.text_generation` paths
- provider/receipt truth for model-backed CPU products
- Metal discovery, allocation, command submission, truthful backend selection,
  and tested Metal-backed `mox.embeddings`
- AMD topology/risk/recovery metadata plus separate `amd_kfd` and
  `amd_userspace` discovery/readiness surfaces
- provider-facing AMD context and operator runbook
- Rustygrad subtree rename to Mox

### Delivered after the merged baseline

- `MOX-115` / [#3164](https://github.com/OpenAgentsInc/openagents/issues/3164):
  initial GGML quantized tensor storage substrate for `Q4_0`, `Q4_1`, and
  `Q8_0`, plus stable storage digests and explicit block-layout metadata
- Candle-aligned `Q4_0` and `Q4_1` dequantization order plus stricter
  last-dimension block validation for GGML-shaped tensors
- explicit runtime truth for dense storage versus dequantized fallback versus
  backend-quantized storage paths
- `MOX-110` / [#3172](https://github.com/OpenAgentsInc/openagents/issues/3172):
  reusable GGUF metadata and tensor parsing in `mox-models`, `WeightFormat::Gguf`,
  explicit GGUF tensor-type metadata, truthful `F16` / `BF16` dtype support,
  a `GgufWeightBundleLoader`, and GGUF tests for metadata parsing, tensor
  loading, alignment, and unsupported-type refusal
- `MOX-111` / [#3173](https://github.com/OpenAgentsInc/openagents/issues/3173):
  reusable GGUF tokenizer metadata loading in `mox-models` for SentencePiece
  (`llama`) and GPT-style BPE (`gpt2`) families, plus stable tokenizer digests,
  preserved BOS/EOS/add-bos/add-eos and BPE pretokenizer truth, and explicit
  validation for missing tokenizer metadata and out-of-range special-token IDs
- `MOX-116` / [#3165](https://github.com/OpenAgentsInc/openagents/issues/3165):
  new `mox-catalog` blob substrate with mmap-or-buffered local reads, stable
  blob digests, and paged byte ranges, plus blob-backed GGUF paging in
  `mox-models`, storage-truth metadata on GGUF artifacts, and runtime-facing
  paged artifact/tensor planning types in `mox-runtime`
- `MOX-118` / [#3167](https://github.com/OpenAgentsInc/openagents/issues/3167):
  real golden tokenizer and prompt/template fixture corpus in `mox-models`,
  reusable GGUF chat-template extraction and digesting, prompt/template
  assertion helpers consumed from `mox-models` and `mox-serve`, and fixture
  refresh documentation in `crates/mox/docs/FIXTURE_CORPUS.md`
- `MOX-179` / [#3239](https://github.com/OpenAgentsInc/openagents/issues/3239):
  GPT-OSS / OpenAI-MoE GGUF loading and truthful mixed `MXFP4` / `Q8_0`
  storage in `mox-core` and `mox-models`, including `general.architecture =
  gpt-oss` family mapping, OpenAI-MoE metadata and tensor-layout validation,
  GGUF `MXFP4` tensor-type support with `llama.cpp`-aligned block decode
  semantics, and surfaced `quantization_modes` truth through model/provider
  metadata without pretending unsupported execution support
- `MOX-180` / [#3240](https://github.com/OpenAgentsInc/openagents/issues/3240):
  GPT-OSS / Harmony prompt rendering and channel parsing in `mox-models` and
  `mox-serve`, including Mox-owned Harmony prompt/context/message types,
  real GPT-OSS golden render fixtures pinned against the local GGUF, text/token
  Harmony parse helpers plus a streaming parser wrapper, and optional served
  Harmony structure alongside the raw token/text response lane
- `MOX-117` / [#3166](https://github.com/OpenAgentsInc/openagents/issues/3166):
  reusable Ollama-to-Mox conformance harness in `mox-serve`, a live
  `OllamaHttpSubject` over `tags` / `show` / `ps` / `generate` / `embed`,
  explicit `passed` / `failed` / `unsupported` / `intentional_difference`
  outcomes, fixture-driven prompt-render case construction from the golden
  corpus, structured report artifacts, and a documented harness runbook in
  `crates/mox/docs/CONFORMANCE_HARNESS.md`
- `MOX-119` / [#3168](https://github.com/OpenAgentsInc/openagents/issues/3168):
  shared backend parity policy in `mox-runtime` with explicit dense-versus-
  quantized drift budgets for embeddings and logits, seeded-versus-unseeded
  generation parity classes, reusable vector/logit comparison helpers, and
  policy-backed Metal embeddings parity plus conformance embed comparisons
- `MOX-160` / [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220):
  reusable local-serving isolation policy in `mox-runtime`, explicit
  in-process crash/reset truth in `mox-serve` observability and generation
  provenance, an aggregate `MoxLocalRuntime::isolation_policy()` surface, and
  cutover-contract documentation for the current in-process-versus-subprocess
  decision
- `MOX-161` / [#3171](https://github.com/OpenAgentsInc/openagents/issues/3171):
  reusable served-product fallback lattice in `mox-runtime`, with explicit
  trigger/action vocabulary, surfaced `same_backend_slow_path` / `retried` /
  `refused` selection states, provider/serve truth for realized fallback
  state, validation mapping that distinguishes explicit refusal, and a
  documented fallback boundary in the conformance/evidence contract
- `MOX-162` / [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233):
  first-class served-artifact identity and reproducibility tuples in
  `mox-runtime`, descriptor-side artifact identity metadata in `mox-models`,
  and explicit capability/receipt/provenance/session/prefix-cache truth in
  `mox-provider` and `mox-serve`, with request digests and cache ownership now
  refusing silent tokenizer/template/default drift
- `MOX-163` / [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234):
  explicit runtime cache invalidation policy in `mox-runtime` for execution-
  plan, kernel-cache, paged-tensor, prefix-cache, and KV-state families, plus
  provider/serve evidence surfacing of both the policy and realized cache
  observations so rebuild, bypass, invalidate, and restore behavior remain
  machine-checkable
- `MOX-164` / [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235):
  explicit local-artifact provenance and declared-license facts in
  `mox-catalog` and `mox-models`, plus provider-side compute-market supply
  policy and advertise/serve decisions so policy refusals stay distinct from
  integrity and unsupported-format failures
- `MOX-170` / [#3222](https://github.com/OpenAgentsInc/openagents/issues/3222):
  explicit migration-boundary truth in `mox-models`, `mox-catalog`, and
  `mox-serve`, with catalog/ingress/serving/runtime boundary facts that keep
  Ollama compatibility visible as migration substrate instead of silently
  turning it into the Mox-native architectural source of truth
- `MOX-171` / [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223):
  compute-market capability qualifiers in `mox-runtime` and `mox-provider`,
  with reusable selected-device inventory classification, explicit
  `compiled_only` versus `compiled_and_probed` backend-toolchain truth, and
  provider capability/receipt surfaces that now expose stable device,
  topology-key, memory-class, and performance-class qualifiers
- `MOX-172` / [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224):
  explicit execution-profile truth in `mox-runtime`, `mox-serve`, and
  `mox-provider`, with machine-checkable batch posture, queue policy, and
  throughput class reporting for both embeddings and text generation plus
  observability coverage that now distinguishes "no internal queue exists"
  from "the current queue just happens to be empty"
- `MOX-173` / [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225):
  explicit multi-device selection and sharding/topology truth in
  `mox-runtime`, `mox-compiler`, and `mox-provider`, with stable topology
  digests, compiler-facing topology-aware plan wrappers, provider capability
  and receipt surfacing of `selected_devices` plus `execution_topology`, and
  contract coverage for the current single-device-versus-sharded boundary
- `MOX-174` / [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226):
  explicit execution-plan cache policy/state plus warm/cold compile-path
  evidence in `mox-runtime`, backend-owned plan caching in CPU/Metal/CUDA,
  embeddings and text-generation provenance in `mox-serve`, and provider
  receipt surfacing of realized plan-cache and kernel-cache behavior
- `MOX-175` / [#3227](https://github.com/OpenAgentsInc/openagents/issues/3227):
  compute-market delivery-proof and settlement-linkage evidence in
  `mox-runtime`, `mox-serve`, and `mox-provider`, with kernel-count,
  bytes-moved, plan-cache hit/miss, and KV-growth reporting carried directly
  through runtime metrics, serve-side provenance, and provider receipts
- `OA-200` / [#3216](https://github.com/OpenAgentsInc/openagents/issues/3216):
  app-owned local-execution evidence naming cleanup in
  `apps/autopilot-desktop`, with `LocalInferenceExecutionMetrics` and
  `LocalInferenceExecutionProvenance` now replacing Ollama-specific type names
  across the Ollama worker, Apple bridge, runtime state, kernel-control
  delivery logic, and receipt evidence
- `MOX-112` / [#3177](https://github.com/OpenAgentsInc/openagents/issues/3177):
  reusable GGUF decoder-family adapters in `mox-models` for the first launch
  families, with Candle-aligned family/config extraction, Ollama-compatible
  metadata and tensor-name mapping, explicit Mistral-vs-Llama family truth,
  reusable tensor layouts, attached tokenizer/chat-template metadata, and
  explicit refusal of unsupported llama MoE artifacts
- `MOX-113` / [#3178](https://github.com/OpenAgentsInc/openagents/issues/3178):
  reusable GGUF embedding-family adapters in `mox-models` for the first launch
  encoder families, with Ollama-aligned BERT and Nomic-BERT metadata
  extraction, pooling and normalization truth, reusable tensor layouts,
  attached tokenizer metadata, completed BERT wordpiece/token-type-count GGUF
  tokenizer support, and explicit refusal of unsupported Nomic MoE artifacts
- `MOX-114` / [#3179](https://github.com/OpenAgentsInc/openagents/issues/3179):
  reusable GGUF prompt rendering in `mox-models` for the supported golden
  template families, with explicit prompt-message/rendered-prompt/error types,
  digest-gated Phi-3 / Qwen2 / Command-R compatibility, reusable
  `GgufDecoderAdapter::render_prompt(...)`, `mox-serve` re-exports, and
  conformance-harness coverage that now treats prompt rendering as parity work
  instead of an intentional gap
- `MOX-120` / [#3180](https://github.com/OpenAgentsInc/openagents/issues/3180):
  reusable local Ollama catalog discovery in `mox-catalog`, with
  Ollama-compatible default model-name normalization, parsed manifest/layer
  records, explicit layer-kind and blob-presence truth, non-mutating manifest
  scans with warnings for invalid entries, and direct model-resolution APIs on
  top of the existing shared blob substrate
- `MOX-121` / [#3181](https://github.com/OpenAgentsInc/openagents/issues/3181):
  reusable installed-model `tags` / `show` parity surfaces over the shared
  local Ollama catalog, with manifest-layer config/text/json decode helpers in
  `mox-catalog`, a local `LocalOllamaCatalogSubject` in `mox-serve`, explicit
  local GGUF model-info and capability derivation without the Ollama daemon,
  Ollama-aligned skipping of bad config blobs during listing, and fixture-backed
  list/show tests for local parity and missing-model errors
- `MOX-122` / [#3182](https://github.com/OpenAgentsInc/openagents/issues/3182):
  reusable loaded-model lifecycle truth in `mox-runtime` and `mox-serve`, with
  explicit loading/ready state, active-request counts, keepalive windows,
  `ps`-style ordering, warm/load/unload operations, and Ollama-aligned idle
  expiry semantics including zero-keepalive unload after requests go idle
- `MOX-123` / [#3183](https://github.com/OpenAgentsInc/openagents/issues/3183):
  expanded generation options in `mox-serve` for `temperature`, `top_k`,
  `top_p`, repeat/presence/frequency penalties, `seed`, and explicit
  `stop_sequences`, plus seeded sampling, penalty-adjusted logits, and
  stop-sequence truncation on the CPU reference path
- `MOX-124` / [#3184](https://github.com/OpenAgentsInc/openagents/issues/3184):
  explicit generation metrics and provenance in `mox-serve` and `mox-provider`,
  with prompt/output token counts, total/load durations, warm-versus-cold load
  state, execution-plan digests, receipt alignment to response provenance, and
  regression coverage for cold-then-warm residency and option-bearing request
  digests
- `MOX-125` / [#3185](https://github.com/OpenAgentsInc/openagents/issues/3185):
  a library-first in-process runtime surface in `mox-serve`, with
  `LocalModelCatalog`, `ManagedTextGenerationRuntime`, and `MoxLocalRuntime`
  covering `list_models`, `show_model`, `loaded_models`, `warm_model`,
  `unload_model`, `generate`, and `embed`, plus regression coverage that
  exercises the aggregate boundary end to end
- `MOX-126` / [#3186](https://github.com/OpenAgentsInc/openagents/issues/3186):
  deterministic text-generation session ownership in `mox-serve`, with
  descriptor-bound KV ownership (`model_id`, family, revision, bundle digest),
  token-sequence ownership alongside the KV cache, explicit cache-plus-token
  commit on successful generation, and regression coverage for isolation/reset
  plus descriptor-drift refusal
- `MOX-126A` / [#3169](https://github.com/OpenAgentsInc/openagents/issues/3169):
  runtime-owned paged-KV policy and accounting in `mox-runtime`, logically
  paged per-session KV state in `mox-serve` with explicit `refuse_new_pages`
  behavior instead of silent spill/evict, session metadata bound to KV
  policy/state, and generation/provider evidence carrying KV pages, bytes, and
  growth
- `MOX-126B` / [#3231](https://github.com/OpenAgentsInc/openagents/issues/3231):
  shared prompt-prefix cache truth in `mox-runtime`, `mox-serve`, and
  `mox-provider`, with explicit reusable-prefix identity inputs, shared-prefix
  reuse policy, `none` / `hit` / `miss` / `bypassed` / `rebuilt` taxonomy,
  longest-safe prefix reuse on the CPU reference path, stale-entry rebuild
  handling, and provider/receipt evidence carrying prefix-cache state,
  identity, policy, and reused-token counts
- `MOX-165` / [#3236](https://github.com/OpenAgentsInc/openagents/issues/3236):
  OCI/Docker-v2 registry pull and ingestion in `mox-catalog`, with
  Ollama-compatible manifest/blob URLs, manifest validation shared with local
  scans, digest/size-checked blob writes into the existing local store,
  explicit pull reports that distinguish reused-versus-downloaded blobs, and
  a `mox-models` GGUF loader path that can consume a resolved local Ollama
  manifest directly after pull
- `MOX-130` / [#3200](https://github.com/OpenAgentsInc/openagents/issues/3200):
  truthful Metal dense-surface coverage for the current `mox.text_generation`
  graph shape, with a distinct text-generation op contract, shared dense
  pipeline/kernel-cache accounting, explicit Metal-versus-CPU selection and
  fallback coverage for that product surface, and a direct Metal execution test
  over the current text-generation matmul/add graph
- `MOX-131` / [#3201](https://github.com/OpenAgentsInc/openagents/issues/3201):
  reusable CPU-versus-Metal parity coverage for the current
  `mox.text_generation` graph shape, with seeded exact token/text/termination
  parity, policy-backed hidden/logit drift checks, a dedicated
  `metal_text_generation_parity` integration test in `mox-serve`, and
  macOS-target import fixes in `mox-backend-metal` exposed by cross-target
  compilation
- `MOX-132` / [#3202](https://github.com/OpenAgentsInc/openagents/issues/3202):
  a tested Metal-backed `mox.text_generation` product path in `mox-serve`,
  with a real `MetalModelTextGenerationService`, shared non-streaming
  generation/session/prefix-cache/provenance flow across CPU and Metal,
  explicit Metal unavailability diagnostics instead of silent CPU fallback,
  and provider-facing capability/receipt coverage for success versus explicit
  refusal
- `MOX-140` / [#3203](https://github.com/OpenAgentsInc/openagents/issues/3203):
  explicit CUDA backend architecture truth in `mox-backend-cuda`,
  `mox-runtime`, and `mox-provider`, with a new `mox-backend-cuda` crate,
  first-class `DeviceKind::Cuda`, direct runtime/provider selection identity
  for `cuda`, and an explicit architecture-only offline state so Mox does not
  pretend NVIDIA discovery, topology, or execution are already landed
- `MOX-141` / [#3204](https://github.com/OpenAgentsInc/openagents/issues/3204):
  explicit reusable NVIDIA topology, risk, and recovery truth in
  `mox-runtime` and `mox-provider`, with first-class `nvidia_metadata` on
  runtime device descriptors, provider-visible `nvidia` capability/receipt
  context, and regression coverage so later CUDA discovery and selection work
  can build on machine-checkable NVIDIA contract surfaces
- `MOX-142` / [#3205](https://github.com/OpenAgentsInc/openagents/issues/3205):
  real `nvidia-smi`-backed CUDA discovery and health reporting in
  `mox-backend-cuda`, with runtime device descriptors populated from live
  NVIDIA query data, explicit ready versus degraded versus offline health,
  display-attached and MIG caveats preserved as degraded-state truth, and
  stable CUDA feature flags for persistence and addressing-mode posture
- `MOX-143` / [#3206](https://github.com/OpenAgentsInc/openagents/issues/3206):
  operational CUDA allocation and submission substrate in `mox-backend-cuda`,
  with dynamic `libcudart` loading, explicit device buffers plus host staging
  reads and writes, stream-based fill/copy submission with machine-checkable
  completion status, allocator/runtime-resource truth, and end-to-end buffer
  copy coverage on the selected NVIDIA device when the CUDA runtime is present
- `MOX-144` / [#3207](https://github.com/OpenAgentsInc/openagents/issues/3207):
  operational CUDA dense execution in `mox-backend-cuda`, with explicit
  `input` / `constant` / `matmul` / `add` plan validation, dense CUDA
  input/constant materialization, Candle-aligned cuBLAS row-major matmul
  lowering, cuBLAS-backed add coverage, and live end-to-end execution tests
  for the first NVIDIA primitive surface
- `MOX-145` / [#3208](https://github.com/OpenAgentsInc/openagents/issues/3208):
  truthful NVIDIA served-product selection and provider capability reporting,
  with explicit direct versus same-backend-degraded versus CPU-fallback CUDA
  selection surfaces in `mox-backend-cuda`, and provider capability/receipt
  coverage that now reports real post-`MOX-144` NVIDIA execution posture
- `MOX-146` / [#3209](https://github.com/OpenAgentsInc/openagents/issues/3209):
  explicit CPU-versus-CUDA parity evidence for the first supported NVIDIA
  served path, with a model-backed embeddings parity test in `mox-serve`
  that checks CUDA outputs against the CPU baseline under the shared
  embeddings drift budget and reports explicit CPU fallback instead of
  overclaiming CUDA parity when the backend is unavailable
- `MOX-147` / [#3210](https://github.com/OpenAgentsInc/openagents/issues/3210):
  the first tested NVIDIA-backed served product path in `mox-serve`, with a
  real `CudaModelEmbeddingsService`, CUDA-specific embeddings diagnostics,
  model-backed response/capability/receipt integration coverage, and explicit
  backend-unavailability handling instead of aspirational NVIDIA advertising
- `MOX-148` / [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232):
  a minimum shipped hardware validation profile in `mox-runtime`,
  `mox-provider`, and `crates/mox/docs/`, with explicit validation claim IDs
  for CPU reference lanes, Apple Silicon Metal lanes, NVIDIA CUDA embeddings,
  AMD KFD discovery, and refusal paths, provider-facing capability/receipt
  references back to that matrix, and a documented host-class lab runbook
- `MOX-150` / [#3211](https://github.com/OpenAgentsInc/openagents/issues/3211):
  AMD execution substrate groundwork in `mox-backend-amd-kfd`,
  `mox-backend-amd-userspace`, `mox-provider`, and `crates/mox/docs/`, with
  backend-owned staging buffers, explicit fill/copy submissions, explicit
  allocator/kernel-cache/device-budget truth for AMD substrate paths, and
  provider-visible runtime-resource coverage without advertising a shipped
  AMD served product yet
- `MOX-137` / [#3194](https://github.com/OpenAgentsInc/openagents/issues/3194):
  explicit served-product backend fallback/degraded policy in `mox-runtime`,
  `mox-provider`, and CPU/Metal backend selection, with machine-checkable
  unavailable/degraded policy enums, direct / same-backend-degraded /
  cross-backend-fallback state, and regression coverage for capability JSON
  and fallback/degraded truth
- `MOX-138` / [#3195](https://github.com/OpenAgentsInc/openagents/issues/3195):
  explicit cutover performance gates in `mox-serve` and `mox-provider`, with
  Ollama-aligned timing metrics retained on conformance observations and
  receipts, ratio-based default thresholds for generation and embeddings, and
  `ConformanceReport::cutover_ready_with_performance(...)` for machine-
  checkable semantic-plus-performance cutover decisions
- `MOX-139` / [#3196](https://github.com/OpenAgentsInc/openagents/issues/3196):
  explicit Ollama adapter-policy truth in `mox-catalog`, explicit refusal of
  adapter-bearing Ollama manifests in `mox-models` instead of silently loading
  the base GGUF alone, and `mox-serve` show-surface facts plus conformance
  handling so extra Mox evidence does not count as an Ollama semantic mismatch
- `MOX-157` / [#3197](https://github.com/OpenAgentsInc/openagents/issues/3197):
  explicit backend runtime-resource truth in `mox-runtime`, exact-spec
  allocator pooling for CPU and Metal intermediate buffers, bounded kernel-
  cache reporting, device-memory-budget reporting, and provider-visible
  serialization/tests so warm/cold and memory-admission behavior stay machine-
  checkable instead of backend-internal
- `MOX-158` / [#3198](https://github.com/OpenAgentsInc/openagents/issues/3198):
  typed backend-extension ops in the graph/plan layer for `rms_norm`,
  `layer_norm`, `rotary_embedding`, `scaled_dot_product_attention`, and
  `quantized_matmul`, explicit backend-extension capability truth on
  `BackendSelection`, CPU reference execution for those extension families,
  and a path for later Metal/CUDA/AMD specialized kernels without polluting
  the base primitive-op surface
- `MOX-159` / [#3199](https://github.com/OpenAgentsInc/openagents/issues/3199):
  explicit local-runtime observability in `mox-runtime`, `mox-serve`, and
  `mox-provider`, with bounded recent transition logs for cold-load, first-
  request warm, unload, and backend-health changes, active-session and
  active-request counts, queue-depth truth, memory-footprint snapshots, and a
  managed-runtime `observability()` surface plus provider-facing envelope
  serialization/tests
- `MOX-156` / [#3170](https://github.com/OpenAgentsInc/openagents/issues/3170):
  first-class quantized GGML/GGUF constant payloads in `mox-core` /
  `mox-ir`, Candle-aligned row-wise quantized-matmul RHS orientation,
  CPU-native `Q4_0` / `Q4_1` / `Q8_0` kernels over preserved block bytes,
  explicit `backend_quantized` + `native` CPU capability truth, provider-
  visible quantized capability reporting, and explicit Metal refusal for
  quantized constants instead of silent fallback
- `MOX-129` / [#3189](https://github.com/OpenAgentsInc/openagents/issues/3189):
  explicit local-serving memory planning, residency policy, and admission
  control in `mox-runtime` / `mox-serve` / `mox-provider`, with bounded-budget
  refusal and optional idle-oldest eviction, default decoder memory plans,
  and provider/runtime evidence carrying memory plans, residency policy, and
  current residency snapshots
- `MOX-133` / [#3190](https://github.com/OpenAgentsInc/openagents/issues/3190):
  pull-driven local streaming generation in `mox-serve` / `mox-provider`, with
  explicit backpressure, disconnect, and cancellation policy, typed chunk and
  terminal events, partial-output semantics for cancellation/disconnect/runtime
  failure after stream start, and receipt/capability truth carrying streaming
  policy
- `MOX-134` / [#3191](https://github.com/OpenAgentsInc/openagents/issues/3191):
  explicit embeddings API semantics in `mox-serve` / `mox-provider`, with
  empty-batch success, requested output-dimension handling, model-family /
  revision / normalization metadata on responses and receipts, ordered-batch
  capability truth, and explicit `supports_input_truncation = false` on the
  current byte-projection embeddings paths
- `MOX-135` / [#3192](https://github.com/OpenAgentsInc/openagents/issues/3192):
  local model-store integrity verification in `mox-catalog` / `mox-models`,
  with structured repair diagnostics for missing manifests/blobs and
  corrupt-or-size-mismatched blobs, explicit manifest-level verification over
  the shared blob substrate, and GGUF manifest loads that now refuse corrupt
  primary model blobs before parse
- `MOX-136` / [#3193](https://github.com/OpenAgentsInc/openagents/issues/3193):
  backend-neutral local runtime diagnostics taxonomy in `mox-runtime` /
  `mox-serve` / `mox-provider`, with stable error codes plus HTTP-style
  status/message/context fields, serve-layer mappings from current generation
  and embeddings failures into that taxonomy, streaming terminals carrying
  structured diagnostics, and provider receipts preserving those diagnostics
  alongside plain-text failure reasons

### GitHub issue status

Verified on 2026-03-08 via `gh issue list --state all` with duplicate spot
checks via `gh issue view`:

| Issue span | State | What landed |
| --- | --- | --- |
| [#3143](https://github.com/OpenAgentsInc/openagents/issues/3143), [#3144](https://github.com/OpenAgentsInc/openagents/issues/3144) to [#3149](https://github.com/OpenAgentsInc/openagents/issues/3149) | Closed | Phase-2 CPU baseline: artifact-backed bundles, quantization truth, CPU embeddings, CPU text generation, provider truth, and tested model-backed flows. |
| [#3150](https://github.com/OpenAgentsInc/openagents/issues/3150), [#3151](https://github.com/OpenAgentsInc/openagents/issues/3151) to [#3156](https://github.com/OpenAgentsInc/openagents/issues/3156) | Closed | Phase-3 Metal baseline: discovery, allocator/submission substrate, minimum kernel coverage, truthful backend selection, parity coverage, and Metal embeddings. |
| [#3157](https://github.com/OpenAgentsInc/openagents/issues/3157), [#3158](https://github.com/OpenAgentsInc/openagents/issues/3158) to [#3162](https://github.com/OpenAgentsInc/openagents/issues/3162) | Closed | Phase-4 AMD truth baseline: AMD metadata model, KFD/userspace discovery, provider truth, and runbook coverage. |
| [#3164](https://github.com/OpenAgentsInc/openagents/issues/3164) | Closed | `MOX-115` landed: GGML quantized tensor storage substrate, Candle-aligned `Q4_0` / `Q4_1` decode order, and stricter GGML block-shape validation. |
| [#3165](https://github.com/OpenAgentsInc/openagents/issues/3165) | Closed | `MOX-116` landed: `mox-catalog` blob access substrate, mmap-or-buffered GGUF and Ollama blob reads, paged tensor slices for GGUF tensors, storage-truth metadata on artifacts, and runtime-facing paged storage planning types. |
| [#3166](https://github.com/OpenAgentsInc/openagents/issues/3166) | Closed | `MOX-117` landed: reusable Ollama-to-Mox conformance harness in `mox-serve`, live Ollama HTTP normalization for `tags` / `show` / `ps` / `generate` / `embed`, explicit pass/fail/unsupported/intentional-difference outcomes, fixture-driven prompt-render cases, and a documented report/runbook. |
| [#3167](https://github.com/OpenAgentsInc/openagents/issues/3167) | Closed | `MOX-118` landed: real tokenizer and prompt/template fixture corpus, GGUF chat-template extraction plus digests, reusable assertion helpers, and documented fixture refresh flow. |
| [#3168](https://github.com/OpenAgentsInc/openagents/issues/3168) | Closed | `MOX-119` landed: shared backend parity policy in `mox-runtime`, explicit dense-vs-quantized drift budgets for embeddings/logits, seeded-vs-unseeded generation parity classes, reusable comparison helpers, and policy-backed parity/conformance tests plus documentation. |
| [#3170](https://github.com/OpenAgentsInc/openagents/issues/3170) | Closed | `MOX-156` landed: first-class quantized GGML/GGUF constant payloads, Candle-aligned row-wise quantized matmul RHS semantics, native CPU `Q4_0` / `Q4_1` / `Q8_0` kernels over preserved block bytes, explicit CPU quantized capability truth, and explicit Metal refusal for quantized constants. |
| [#3200](https://github.com/OpenAgentsInc/openagents/issues/3200) | Closed | `MOX-130` landed: a distinct Metal text-generation dense-surface contract in `mox-backend-metal`, shared dense pipeline/kernel-cache accounting, explicit Metal-vs-CPU selection/fallback coverage for that product surface, and direct Metal execution coverage for the current text-generation matmul/add graph. |
| [#3201](https://github.com/OpenAgentsInc/openagents/issues/3201) | Closed | `MOX-131` landed: a dedicated `metal_text_generation_parity` integration test in `mox-serve`, seeded exact CPU-vs-Metal token/text/termination parity over the current graph shape, policy-backed hidden/logit drift checks, and macOS-target Metal import fixes exposed by cross-target compilation. |
| [#3202](https://github.com/OpenAgentsInc/openagents/issues/3202) | Closed | `MOX-132` landed: a real `MetalModelTextGenerationService` in `mox-serve`, shared CPU/Metal non-streaming generation/session/prefix-cache/provenance flow, explicit Metal unavailability diagnostics instead of silent CPU fallback, and provider-facing capability/receipt tests for success versus explicit refusal. |
| [#3203](https://github.com/OpenAgentsInc/openagents/issues/3203) | Closed | `MOX-140` landed: a new `mox-backend-cuda` architecture crate, first-class `DeviceKind::Cuda`, runtime/provider backend-selection truth for `cuda`, and an explicit architecture-only offline state so NVIDIA is visible without overclaiming discovery or execution readiness. |
| [#3204](https://github.com/OpenAgentsInc/openagents/issues/3204) | Closed | `MOX-141` landed: reusable NVIDIA topology/risk/recovery metadata in `mox-runtime`, `nvidia_metadata` on runtime device descriptors, and provider-visible `nvidia` capability/receipt context so later CUDA discovery and routing work has an explicit truth contract. |
| [#3205](https://github.com/OpenAgentsInc/openagents/issues/3205) | Closed | `MOX-142` landed: real `nvidia-smi`-backed CUDA discovery in `mox-backend-cuda`, explicit ready/degraded/offline health, runtime device descriptors populated from live NVIDIA query data, and degraded-state truth for display-attached or MIG-partitioned devices. |
| [#3206](https://github.com/OpenAgentsInc/openagents/issues/3206) | Closed | `MOX-143` landed: operational `libcudart`-backed CUDA buffers and stream submission in `mox-backend-cuda`, explicit allocator/runtime-resource truth, and end-to-end staged write plus device-to-device copy coverage on the selected NVIDIA device. |
| [#3207](https://github.com/OpenAgentsInc/openagents/issues/3207) | Closed | `MOX-144` landed: the first CUDA dense execution surface in `mox-backend-cuda`, with explicit `input` / `constant` / `matmul` / `add` plan validation, dense CUDA materialization helpers, Candle-aligned cuBLAS matmul lowering, cuBLAS-backed add coverage, and live backend execution tests. |
| [#3208](https://github.com/OpenAgentsInc/openagents/issues/3208) | Closed | `MOX-145` landed: explicit direct/degraded/fallback CUDA backend-selection surfaces in `mox-backend-cuda`, plus provider capability and receipt coverage that now reflects real NVIDIA execution posture instead of the old architecture-only placeholder. |
| [#3209](https://github.com/OpenAgentsInc/openagents/issues/3209) | Closed | `MOX-146` landed: explicit CPU-vs-CUDA embeddings parity coverage in `mox-serve`, with drift-budget comparison against the CPU baseline and explicit CPU fallback truth when CUDA is unavailable. |
| [#3210](https://github.com/OpenAgentsInc/openagents/issues/3210) | Closed | `MOX-147` landed: the first tested NVIDIA-backed served product path as model-backed embeddings in `mox-serve`, with a real CUDA service, CUDA-specific diagnostics, and capability/receipt integration coverage. |
| [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232) | Closed | `MOX-148` landed: a minimum shipped hardware validation matrix in `mox-runtime`, `mox-provider`, and `crates/mox/docs`, with explicit claim IDs for CPU, Apple Silicon Metal, NVIDIA CUDA embeddings, AMD KFD discovery, and refusal paths, plus provider-facing validation references and a documented lab runbook. |
| [#3211](https://github.com/OpenAgentsInc/openagents/issues/3211) | Closed | `MOX-150` landed: AMD KFD and AMD userspace execution substrate groundwork with backend-owned staging buffers, explicit fill/copy submissions, explicit runtime-resource truth, explicit CPU fallback helpers, and provider/runtime-resource coverage while AMD served products remain unshipped. |
| [#3212](https://github.com/OpenAgentsInc/openagents/issues/3212), [#3213](https://github.com/OpenAgentsInc/openagents/issues/3213), [#3214](https://github.com/OpenAgentsInc/openagents/issues/3214), [#3215](https://github.com/OpenAgentsInc/openagents/issues/3215) | Closed (Not Planned) | `MOX-151` through `MOX-154` were closed after host reprioritization to NVIDIA-only execution and validation. Keep the landed AMD substrate from `MOX-150`, but do not treat AMD KFD lowering, served-product gating, parity, or shipped AMD execution as active roadmap dependencies on this machine. |
| [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220) | Closed | `MOX-160` landed: reusable local-serving isolation policy in `mox-runtime`, explicit `in_process` crash/reset truth in `mox-serve` observability and generation provenance, an aggregate `MoxLocalRuntime::isolation_policy()` surface, and cutover-contract documentation for the current no-subprocess decision. |
| [#3222](https://github.com/OpenAgentsInc/openagents/issues/3222) | Closed | `MOX-170` landed: explicit Ollama-compat versus Mox-native boundary metadata in `mox-models`, `mox-catalog`, and `mox-serve`, with `show`-surface facts that keep compatibility support honest as migration substrate. |
| [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223) | Closed | `MOX-171` landed: reusable device inventory qualifiers in `mox-runtime`, explicit compile-vs-probe backend toolchain truth in `mox-provider`, and provider capability/receipt surfaces that now expose selected-device inventory and backend-toolchain facts for compute-market filtering. |
| [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224) | Closed | `MOX-172` landed: runtime-owned execution profiles in `mox-runtime`, `mox-serve` defaults plus observability alignment, and provider capability reporting of batch posture, queue policy, and throughput class for embeddings and text generation. |
| [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225) | Closed | `MOX-173` landed: explicit multi-device selection truth and `ExecutionTopologyPlan` substrate in `mox-runtime`, topology-aware compiled-plan digests in `mox-compiler`, and provider capability/receipt surfacing of `selected_devices` plus `execution_topology` with multi-device regression coverage. |
| [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226) | Closed | `MOX-174` landed: explicit execution-plan cache policy/state plus compile-path evidence in `mox-runtime`, backend-owned plan caching in CPU/Metal/CUDA, `mox-serve` provenance for embeddings and generation compile paths, and provider receipt surfacing of realized plan-cache/kernel-cache behavior. |
| [#3177](https://github.com/OpenAgentsInc/openagents/issues/3177) | Closed | `MOX-112` landed: reusable GGUF decoder-family adapters, explicit Llama/Qwen/Mistral family metadata and tensor layouts, attached tokenizer/chat-template metadata, and explicit refusal of unsupported llama MoE artifacts. |
| [#3178](https://github.com/OpenAgentsInc/openagents/issues/3178) | Closed | `MOX-113` landed: reusable GGUF embedding-family adapters for BERT and Nomic-BERT, explicit pooling/normalization truth and tensor layouts, finished BERT wordpiece/token-type-count tokenizer support, and explicit refusal of unsupported Nomic MoE artifacts. |
| [#3179](https://github.com/OpenAgentsInc/openagents/issues/3179) | Closed | `MOX-114` landed: reusable GGUF prompt rendering for the supported Phi-3, Qwen2, and Command-R template digests, explicit prompt/render/error types, `GgufDecoderAdapter` render helpers, `mox-serve` re-exports, and conformance coverage that removed the old prompt-render gap. |
| [#3180](https://github.com/OpenAgentsInc/openagents/issues/3180) | Closed | `MOX-120` landed: local Ollama manifest/blob discovery and model resolution in `mox-catalog`, including default name normalization, parsed manifest/media-type/layer records, blob-presence truth, non-mutating scan warnings, and direct resolved-manifest APIs. |
| [#3181](https://github.com/OpenAgentsInc/openagents/issues/3181) | Closed | `MOX-121` landed: reusable manifest-layer config/text/json decode helpers in `mox-catalog`, plus a local `tags` / `show` subject in `mox-serve` that reads the shared Ollama catalog directly, derives GGUF model-info facts and capabilities without the Ollama daemon, skips bad config blobs during listing like Ollama does, and includes fixture-backed local list/show tests. |
| [#3182](https://github.com/OpenAgentsInc/openagents/issues/3182) | Closed | `MOX-122` landed: explicit loaded-model residency truth in `mox-runtime`, an in-memory warm/load/unload registry in `mox-serve` with `ps`-style ordering and zero-keepalive unload behavior, request lifecycle hooks that clear/reset expiry like Ollama scheduler warmups, and regression tests for keepalive ordering and idle expiry. |
| [#3183](https://github.com/OpenAgentsInc/openagents/issues/3183) | Closed | `MOX-123` landed: explicit generation-option fields in `mox-serve` for temperature, top-k, top-p, repeat/presence/frequency penalties, seed, and stop sequences, plus seeded stochastic sampling, penalty-adjusted logits, and stop-sequence truncation on the CPU reference path. |
| [#3184](https://github.com/OpenAgentsInc/openagents/issues/3184) | Closed | `MOX-124` landed: explicit generation metrics and provenance in `mox-serve` and `mox-provider`, including prompt/output token counts, total/load durations, warm/cold load state, execution-plan digests, provenance-aligned receipts, and regression coverage for cold-then-warm residency. |
| [#3185](https://github.com/OpenAgentsInc/openagents/issues/3185) | Closed | `MOX-125` landed: a library-first in-process runtime API in `mox-serve` with reusable catalog and managed-generation traits plus an aggregate `MoxLocalRuntime` wrapper over list/show/ps/warm/unload/generate/embed. |
| [#3186](https://github.com/OpenAgentsInc/openagents/issues/3186) | Closed | `MOX-126` landed: deterministic text-generation session ownership in `mox-serve`, with descriptor-bound KV ownership, token-sequence ownership alongside KV state, explicit cache-plus-token commit on successful generation, and descriptor-drift refusal coverage. |
| [#3187](https://github.com/OpenAgentsInc/openagents/issues/3187) | Closed | `MOX-127` landed: reusable context-window budgeting in `mox-models`, explicit `refuse` vs `truncate_oldest` prompt-overflow policy in `mox-serve`, Ollama-aligned over-limit error strings, and regression coverage for truncation and session-owned context pressure. |
| [#3188](https://github.com/OpenAgentsInc/openagents/issues/3188) | Closed | `MOX-128` landed: runtime-owned sampler policy and seeded replay behavior in `mox-runtime`, Ollama-aligned defaults/transform order plus bounded penalty lookback, `mox-serve` delegation to that runtime sampler, and generation-level replay coverage for the supported option surface. |
| [#3189](https://github.com/OpenAgentsInc/openagents/issues/3189) | Closed | `MOX-129` landed: reusable memory-plan and residency-policy substrate in `mox-runtime`, admission-aware loaded-model registry behavior in `mox-serve`, bounded-budget refusal plus optional idle-oldest eviction, default decoder memory planning, and capability/receipt evidence carrying memory-plan and residency-snapshot truth. |
| [#3190](https://github.com/OpenAgentsInc/openagents/issues/3190) | Closed | `MOX-133` landed: pull-driven local streaming generation in `mox-serve` with explicit backpressure/disconnect/cancellation policy, typed chunk vs terminal events, partial-output terminal semantics for cancel/disconnect/runtime failure after stream start, runtime forwarding through `MoxLocalRuntime`, and provider receipt/capability streaming truth. |
| [#3191](https://github.com/OpenAgentsInc/openagents/issues/3191) | Closed | `MOX-134` landed: explicit embeddings API semantics in `mox-serve` and `mox-provider`, including empty-batch success, requested output dimensions with re-normalization, model-family/revision/normalization metadata reporting, ordered-batch capability truth, and explicit no-input-truncation support for current byte-projection embeddings paths. |
| [#3192](https://github.com/OpenAgentsInc/openagents/issues/3192) | Closed | `MOX-135` landed: explicit local model-store integrity verification and cache-repair diagnostics in `mox-catalog`, covering missing manifests, missing blobs, digest mismatch, and declared-size mismatch, plus `mox-models` refusal of corrupt primary GGUF blobs from manifest-backed loads. |
| [#3193](https://github.com/OpenAgentsInc/openagents/issues/3193) | Closed | `MOX-136` landed: backend-neutral local runtime diagnostics taxonomy in `mox-runtime`, `mox-serve`, and `mox-provider`, including stable error codes plus HTTP-style status/message/context fields, serve-layer mappings for current request failures, streaming-terminal diagnostics, and provider receipts that preserve structured diagnostics alongside plain-text reasons. |
| [#3194](https://github.com/OpenAgentsInc/openagents/issues/3194) | Closed | `MOX-137` landed: explicit served-product backend fallback/degraded policy in `mox-runtime`, `mox-provider`, and CPU/Metal backend selection, with machine-checkable unavailable/degraded policy enums, direct / same-backend-degraded / cross-backend-fallback state, and regression coverage for capability JSON and fallback/degraded truth. |
| [#3195](https://github.com/OpenAgentsInc/openagents/issues/3195) | Closed | `MOX-138` landed: explicit cutover performance gates in `mox-serve` and `mox-provider`, with Ollama-aligned timing metrics retained on conformance observations and receipts, ratio-based generation and embeddings thresholds, and `ConformanceReport::cutover_ready_with_performance(...)` for machine-checkable semantic-plus-performance cutover decisions. |
| [#3196](https://github.com/OpenAgentsInc/openagents/issues/3196) | Closed | `MOX-139` landed: explicit Ollama adapter-policy status in `mox-catalog`, manifest-backed loader refusal in `mox-models` for adapter-bearing manifests, and `mox-serve` show-surface facts plus conformance handling so extra Mox evidence does not count as an Ollama semantic mismatch. |
| [#3197](https://github.com/OpenAgentsInc/openagents/issues/3197) | Closed | `MOX-157` landed: explicit backend runtime-resource truth in `mox-runtime`, exact-spec allocator pooling for CPU and Metal intermediate buffers, bounded kernel-cache reporting, device-memory-budget reporting, and provider serialization/tests so warm/cold and memory-admission behavior are machine-checkable instead of backend-internal. |
| [#3198](https://github.com/OpenAgentsInc/openagents/issues/3198) | Closed | `MOX-158` landed: typed backend-extension ops for normalization, RoPE, attention, and quantized matmul in the graph/plan layer, explicit backend-extension capability truth on `BackendSelection`, CPU reference execution for those families, and a later path to backend-specialized kernels without polluting the primitive-op surface. |
| [#3199](https://github.com/OpenAgentsInc/openagents/issues/3199) | Closed | `MOX-159` landed: explicit local-runtime observability surfaces in `mox-runtime`, `mox-serve`, and `mox-provider`, with bounded transition logs for cold-load/warm/unload/backend-health changes, active-session and active-request counts, queue-depth and memory-footprint snapshots, and a managed-runtime `observability()` API. |
| [#3169](https://github.com/OpenAgentsInc/openagents/issues/3169) | Closed | `MOX-126A` landed: runtime-owned paged-KV policy/accounting, a logically paged per-session KV cache in `mox-serve` with explicit `refuse_new_pages` behavior, session metadata bound to KV policy/state, and generation/provider evidence carrying KV pages, bytes, and growth. |
| [#3231](https://github.com/OpenAgentsInc/openagents/issues/3231) | Closed | `MOX-126B` landed: explicit shared prefix-cache policy/state/identity in `mox-runtime`, longest-safe prefix reuse plus stale rebuild and bypass handling in `mox-serve`, and provider/receipt truth for prefix reuse and reused-token counts. |
| [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233) | Closed | `MOX-162` landed: first-class served-artifact identity tuples in `mox-runtime`, descriptor-side artifact identity metadata in `mox-models`, provider/serve capability+receipt+provenance surfacing, and cache/session/request-digest invalidation keyed to artifact drift instead of display names alone. |
| [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234) | Closed | `MOX-163` landed: reusable runtime cache invalidation policy and cache observations, with explicit scopes/format versions/triggers for plan, kernel, paged-tensor, prefix, and KV caches plus provider/serve evidence surfaces that report realized reuse, rebuild, bypass, invalidate, and restore actions. |
| [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235) | Closed | `MOX-164` landed: explicit local-artifact provenance and declared-license facts in `mox-catalog` and `mox-models`, plus provider-side compute-market supply policy, advertise/serve decisions, and structured refusal diagnostics that stay distinct from integrity and unsupported-format failures. |
| [#3236](https://github.com/OpenAgentsInc/openagents/issues/3236) | Closed | `MOX-165` landed: OCI/Docker-v2 registry pull into the local Ollama-style manifest/blob store, shared manifest validation for remote and local paths, digest/size-checked blob ingestion with reuse reporting, and `mox-models` loading from a resolved local manifest. |
| [#3172](https://github.com/OpenAgentsInc/openagents/issues/3172) | Closed | `MOX-110` landed: reusable GGUF metadata/tensor parsing, `WeightFormat::Gguf`, `GgufWeightBundleLoader`, and truthful GGUF tensor-type coverage for currently supported dense and quantized families. |
| [#3173](https://github.com/OpenAgentsInc/openagents/issues/3173) | Closed | `MOX-111` landed: reusable GGUF tokenizer metadata loading for SentencePiece and GPT-style BPE families, stable tokenizer digests, preserved BOS/EOS/add-bos/add-eos and pretokenizer truth, and validation for missing or invalid tokenizer metadata. |
| [#3174](https://github.com/OpenAgentsInc/openagents/issues/3174), [#3175](https://github.com/OpenAgentsInc/openagents/issues/3175), [#3176](https://github.com/OpenAgentsInc/openagents/issues/3176), [#3221](https://github.com/OpenAgentsInc/openagents/issues/3221) | Closed | Historical roadmap-seeded duplicates for `MOX-117`, `MOX-118`, `MOX-119`, and `MOX-161`; use the detailed issues `#3166`, `#3167`, `#3168`, and `#3171` for landed scope. |

Current execution queue in dependency order, verified against live GitHub issue
state:

| Order | Local ID | GitHub issue | State | Why this is the current flow |
| --- | --- | --- | --- | --- |
| 1 | `MOX-110` | [#3172](https://github.com/OpenAgentsInc/openagents/issues/3172) | Closed | GGUF metadata and tensor loader substrate is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 2 | `MOX-111` | [#3173](https://github.com/OpenAgentsInc/openagents/issues/3173) | Closed | GGUF tokenizer metadata loading is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 3 | `MOX-115` | [#3164](https://github.com/OpenAgentsInc/openagents/issues/3164) | Closed | Already landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 4 | `MOX-116` | [#3165](https://github.com/OpenAgentsInc/openagents/issues/3165) | Closed | Paged GGUF and Ollama blob access is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 5 | `MOX-118` | [#3167](https://github.com/OpenAgentsInc/openagents/issues/3167) | Closed | The fixture corpus is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 6 | `MOX-117` | [#3166](https://github.com/OpenAgentsInc/openagents/issues/3166) | Closed | The conformance harness is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 7 | `MOX-119` | [#3168](https://github.com/OpenAgentsInc/openagents/issues/3168) | Closed | The backend parity policy is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 8 | `MOX-112` | [#3177](https://github.com/OpenAgentsInc/openagents/issues/3177) | Closed | GGUF-backed decoder-family adapters are now landed on `main`; keep them in sequence but skip them when choosing the next issue. |
| 9 | `MOX-113` | [#3178](https://github.com/OpenAgentsInc/openagents/issues/3178) | Closed | GGUF-backed embeddings adapters are now landed on `main`; keep them in sequence but skip them when choosing the next issue. |
| 10 | `MOX-114` | [#3179](https://github.com/OpenAgentsInc/openagents/issues/3179) | Closed | Supported GGUF prompt rendering is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 11 | `MOX-120` | [#3180](https://github.com/OpenAgentsInc/openagents/issues/3180) | Closed | Local Ollama manifest/blob discovery and model resolution are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 12 | `MOX-121` | [#3181](https://github.com/OpenAgentsInc/openagents/issues/3181) | Closed | Installed-model list/show parity is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 13 | `MOX-122` | [#3182](https://github.com/OpenAgentsInc/openagents/issues/3182) | Closed | Loaded-model lifecycle is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 14 | `MOX-123` | [#3183](https://github.com/OpenAgentsInc/openagents/issues/3183) | Closed | Generation options are now landed on `main`; keep them in sequence but skip them when choosing the next issue. |
| 15 | `MOX-124` | [#3184](https://github.com/OpenAgentsInc/openagents/issues/3184) | Closed | Metrics and provenance now describe the real option-bearing generation path, so keep this in sequence but skip it when choosing the next issue. |
| 16 | `MOX-125` | [#3185](https://github.com/OpenAgentsInc/openagents/issues/3185) | Closed | The app-facing library API boundary is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 17 | `MOX-126` | [#3186](https://github.com/OpenAgentsInc/openagents/issues/3186) | Closed | Deterministic text-generation session ownership is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 18 | `MOX-126A` | [#3169](https://github.com/OpenAgentsInc/openagents/issues/3169) | Closed | Paged KV layout, accounting, and explicit refusal policy are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 19 | `MOX-126B` | [#3231](https://github.com/OpenAgentsInc/openagents/issues/3231) | Closed | Shared prefix reuse truth is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 20 | `MOX-165` | [#3236](https://github.com/OpenAgentsInc/openagents/issues/3236) | Closed | Remote OCI ingestion is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 21 | `MOX-127` | [#3187](https://github.com/OpenAgentsInc/openagents/issues/3187) | Closed | Explicit context-window budgeting, truncation policy, and over-limit refusal semantics are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 22 | `MOX-128` | [#3188](https://github.com/OpenAgentsInc/openagents/issues/3188) | Closed | Deterministic sampling and replay semantics are now landed on `main` through the runtime-owned sampler policy; keep this in sequence but skip it when choosing the next issue. |
| 23 | `MOX-129` | [#3189](https://github.com/OpenAgentsInc/openagents/issues/3189) | Closed | Local-serving memory planning, residency policy, and admission control are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 24 | `MOX-133` | [#3190](https://github.com/OpenAgentsInc/openagents/issues/3190) | Closed | Local runtime streaming semantics are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 25 | `MOX-134` | [#3191](https://github.com/OpenAgentsInc/openagents/issues/3191) | Closed | Embeddings batch semantics, metadata reporting, and failure behavior are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 26 | `MOX-135` | [#3192](https://github.com/OpenAgentsInc/openagents/issues/3192) | Closed | Local model-store integrity verification and repair diagnostics are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 27 | `MOX-136` | [#3193](https://github.com/OpenAgentsInc/openagents/issues/3193) | Closed | Backend-neutral local runtime diagnostics are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 28 | `MOX-137` | [#3194](https://github.com/OpenAgentsInc/openagents/issues/3194) | Closed | Explicit served-product backend fallback, refusal, and degraded-state policy is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 29 | `MOX-138` | [#3195](https://github.com/OpenAgentsInc/openagents/issues/3195) | Closed | Explicit cutover performance gates are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 30 | `MOX-139` | [#3196](https://github.com/OpenAgentsInc/openagents/issues/3196) | Closed | Ollama adapter policy is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 31 | `MOX-157` | [#3197](https://github.com/OpenAgentsInc/openagents/issues/3197) | Closed | Allocator pooling, bounded kernel caches, and device-memory-budget truth are now explicit on `main`; keep this in sequence but skip it when choosing the next issue. |
| 32 | `MOX-158` | [#3198](https://github.com/OpenAgentsInc/openagents/issues/3198) | Closed | Typed backend-extension hooks are now explicit on `main`; keep this in sequence but skip it when choosing the next issue. |
| 33 | `MOX-159` | [#3199](https://github.com/OpenAgentsInc/openagents/issues/3199) | Closed | Local runtime observability is now explicit on `main`; keep this in sequence but skip it when choosing the next issue. |
| 34 | `MOX-156` | [#3170](https://github.com/OpenAgentsInc/openagents/issues/3170) | Closed | Quantized execution parity is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 35 | `MOX-130` | [#3200](https://github.com/OpenAgentsInc/openagents/issues/3200) | Closed | Metal now truthfully exposes the current text-generation dense surface on `main`; keep this in sequence but skip it when choosing the next issue. |
| 36 | `MOX-131` | [#3201](https://github.com/OpenAgentsInc/openagents/issues/3201) | Closed | CPU-vs-Metal parity coverage for the current text-generation graph is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 37 | `MOX-132` | [#3202](https://github.com/OpenAgentsInc/openagents/issues/3202) | Closed | The served Metal text-generation product path is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 38 | `MOX-140` | [#3203](https://github.com/OpenAgentsInc/openagents/issues/3203) | Closed | CUDA backend architecture and explicit pre-discovery truth are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 39 | `MOX-141` | [#3204](https://github.com/OpenAgentsInc/openagents/issues/3204) | Closed | NVIDIA topology, risk, and provider-visible evidence are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 40 | `MOX-142` | [#3205](https://github.com/OpenAgentsInc/openagents/issues/3205) | Closed | CUDA discovery and health reporting are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 41 | `MOX-143` | [#3206](https://github.com/OpenAgentsInc/openagents/issues/3206) | Closed | CUDA allocation, buffer, stream, and submission substrate are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 42 | `MOX-144` | [#3207](https://github.com/OpenAgentsInc/openagents/issues/3207) | Closed | CUDA lowering and the first NVIDIA dense primitive surface are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 43 | `MOX-145` | [#3208](https://github.com/OpenAgentsInc/openagents/issues/3208) | Closed | CUDA backend selection and provider truth are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 44 | `MOX-146` | [#3209](https://github.com/OpenAgentsInc/openagents/issues/3209) | Closed | CPU-vs-NVIDIA parity evidence for the first supported served path is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 45 | `MOX-147` | [#3210](https://github.com/OpenAgentsInc/openagents/issues/3210) | Closed | The first NVIDIA-backed served product path is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 46 | `MOX-148` | [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232) | Closed | The minimum hardware validation matrix is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 47 | `MOX-150` | [#3211](https://github.com/OpenAgentsInc/openagents/issues/3211) | Closed | AMD execution substrate groundwork is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 48 | `MOX-151` | [#3212](https://github.com/OpenAgentsInc/openagents/issues/3212) | Closed (Not Planned) | Closed after host reprioritization to NVIDIA-only execution; keep it in sequence for history but skip it when choosing the next issue. |
| 49 | `MOX-152` | [#3213](https://github.com/OpenAgentsInc/openagents/issues/3213) | Closed (Not Planned) | Closed after host reprioritization to NVIDIA-only execution; keep it in sequence for history but skip it when choosing the next issue. |
| 50 | `MOX-153` | [#3214](https://github.com/OpenAgentsInc/openagents/issues/3214) | Closed (Not Planned) | Closed after host reprioritization to NVIDIA-only execution; keep it in sequence for history but skip it when choosing the next issue. |
| 51 | `MOX-154` | [#3215](https://github.com/OpenAgentsInc/openagents/issues/3215) | Closed (Not Planned) | Closed after host reprioritization to NVIDIA-only execution; keep it in sequence for history but skip it when choosing the next issue. |
| 52 | `MOX-160` | [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220) | Closed | The process-isolation contract is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 53 | `MOX-161` | [#3171](https://github.com/OpenAgentsInc/openagents/issues/3171) | Closed | The fallback lattice is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 54 | `MOX-162` | [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233) | Closed | Served-artifact identity is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 55 | `MOX-163` | [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234) | Closed | Cache invalidation is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 56 | `MOX-164` | [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235) | Closed | Artifact provenance/license gating is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 57 | `MOX-170` | [#3222](https://github.com/OpenAgentsInc/openagents/issues/3222) | Closed | The Ollama-compat versus Mox-native boundary is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 58 | `MOX-171` | [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223) | Closed | Compute-market inventory and backend-toolchain qualifiers are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 59 | `MOX-172` | [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224) | Closed | Batch posture, queue policy, and throughput-class truth are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 60 | `MOX-173` | [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225) | Closed | Multi-device selection and explicit sharding/topology planning are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 61 | `MOX-174` | [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226) | Closed | Execution-plan cache policy/state plus compile-path evidence are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 62 | `MOX-175` | [#3227](https://github.com/OpenAgentsInc/openagents/issues/3227) | Closed | Delivery-proof and settlement-linkage evidence are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 63 | `OA-200` | [#3216](https://github.com/OpenAgentsInc/openagents/issues/3216) | Closed | The app-owned local-execution evidence rename is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 64 | `OA-201` | [#3217](https://github.com/OpenAgentsInc/openagents/issues/3217) | Closed | The app-owned local inference runtime seam is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 65 | `OA-202` | [#3218](https://github.com/OpenAgentsInc/openagents/issues/3218) | Closed | The desktop now defaults to the in-process Mox runtime on `main`; keep it in sequence but skip it when choosing the next issue. |
| 66 | `OA-203` | [#3219](https://github.com/OpenAgentsInc/openagents/issues/3219) | Closed | The production desktop no longer compiles the external Ollama worker path by default, and the remaining user-facing local-runtime wording now says Mox/local inference instead of implying external Ollama. |
| 67 | `MOX-176` | [#3228](https://github.com/OpenAgentsInc/openagents/issues/3228) | Closed | Bounded sandbox execution now has a reusable runtime-owned capability profile and provider envelope, with explicit isolation, filesystem, network, process, resource, and accelerator-access bounds instead of leaving sandbox posture implicit. |
| 68 | `MOX-177` | [#3229](https://github.com/OpenAgentsInc/openagents/issues/3229) | Closed | Sandbox execution now has reusable request-identity, evidence, and provider-receipt contracts with explicit digests, resource summaries, delivery-proof passthrough, and terminal exit reasons instead of leaving compute-market receipts to reconstruct that state later. |
| 69 | `MOX-178` | [#3230](https://github.com/OpenAgentsInc/openagents/issues/3230) | Closed | Topology-aware substitution and deliverability checks now exist in the reusable runtime/provider layer, so accelerator-sensitive offers can distinguish exact delivery, compatible substitution, and underdelivery from explicit promised-versus-delivered facts. |
| 70 | `MOX-179` | [#3239](https://github.com/OpenAgentsInc/openagents/issues/3239) | Closed | GPT-OSS / OpenAI-MoE GGUF loading and truthful mixed `MXFP4` / `Q8_0` storage are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 71 | `MOX-180` | [#3240](https://github.com/OpenAgentsInc/openagents/issues/3240) | Closed | Harmony prompt/render/parse truth is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 72 | `MOX-181` | [#3237](https://github.com/OpenAgentsInc/openagents/issues/3237) | Open | After load + prompt/parse truth exist, Mox still needs a real GGUF-backed decoder execution model instead of the current fixture decoder path. |
| 73 | `MOX-182` | [#3238](https://github.com/OpenAgentsInc/openagents/issues/3238) | Open | The real decoder then needs NVIDIA text-generation kernel coverage; the current CUDA surface is still embeddings-only for product use. |
| 74 | `MOX-183` | [#3241](https://github.com/OpenAgentsInc/openagents/issues/3241) | Open | The final step is proving the full Mox-only GPT-OSS 20B flow on this NVIDIA host and pinning it with conformance plus hardware-validation evidence. |

The next active roadmap item on this host is `MOX-181` / [#3237](https://github.com/OpenAgentsInc/openagents/issues/3237).

## Current Reality

The checked-in repo is no longer at "phase 0 bootstrap." The current truthful
baseline on `main` is:

- the generic Mox/Ollama-replacement track is landed, but the real GPT-OSS
  path is not: Mox now loads `gpt-oss` / OpenAI-MoE GGUFs and models `MXFP4`
  truthfully, and it now renders/parses GPT-OSS Harmony semantics truthfully,
  but it still does not expose a real GGUF-backed NVIDIA text-generation path
- this NVIDIA host can run the local
  `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf` file through
  external `~/code/llama.cpp`, which remains the practical reference baseline
  for the remaining `MOX-181` through `MOX-183` follow-on work
- CPU model-backed embeddings and text generation exist and are tested
- initial GGML quantized tensor storage and decode coverage now extends to
  CPU-native `Q4_0`, `Q4_1`, and `Q8_0` execution over preserved GGML block
  bytes, with explicit `backend_quantized` + `native` CPU capability truth and
  explicit Metal refusal instead of silent quantized fallback
- local GGUF and Ollama blobs can now be opened through mmap-or-buffered
  fallback paths with explicit paging and storage-truth metadata
- local Ollama manifests can now be discovered and resolved through a
  non-mutating `mox-catalog` surface with explicit scan warnings, normalized
  model names, parsed layer/media-type records, and blob-presence truth
- that same local Ollama-style store can now be integrity-verified per model,
  with structured diagnostics for missing manifests/blobs and corrupt or
  size-mismatched blobs, plus repair-action hints instead of path-exists truth
- remote OCI/Docker-v2 registries can now populate that same local
  Ollama-style manifest/blob store through explicit pull reports, shared
  manifest validation, digest/size-checked blob ingestion, and manifest-based
  GGUF loading in `mox-models`
- local installed-model `tags` / `show` parity now exists in `mox-serve`
  without the Ollama daemon, backed by the shared catalog plus explicit GGUF
  model-info and capability derivation
- loaded-model warm/load/unload and keepalive lifecycle now exists in
  `mox-serve` and `mox-runtime`, with explicit residency truth, active-request
  counts, `ps`-style ordering, and zero-keepalive idle unload semantics
- local runtime observability now exists in `mox-runtime`, `mox-serve`, and
  `mox-provider`, with bounded transition logs for cold-load, first-request
  warm, unload, and backend-health changes, plus active-session, queue-depth,
  active-request, and memory-footprint snapshots on the managed runtime seam
- `autopilot-desktop` now owns a backend-neutral `LocalInferenceRuntime` seam,
  with adapters for the current Ollama worker and the in-process Mox reference
  runtime, and the desktop now defaults that seam to the in-process Mox path
  while preserving the existing app-facing execution snapshot flow
- generation option handling now exists in `mox-serve` for temperature, top-k,
  top-p, seed, stop sequences, and repeat/presence/frequency penalties, with
  seeded sampling and explicit stop-sequence truncation on the CPU reference
  path
- sampler policy and seeded replay behavior now exist in `mox-runtime`, with
  Ollama-aligned defaults, transform order, and bounded penalty lookback, and
  `mox-serve` now delegates supported token selection to that runtime surface
- context-window budgeting now exists in `mox-models` and `mox-serve`, with
  explicit prompt-budget accounting, opt-in oldest-token truncation, and
  Ollama-aligned over-limit refusal strings instead of implicit prompt prefill
  failure
- generation metrics and provenance now exist in `mox-serve` and
  `mox-provider`, with prompt/output token counts, total/load durations,
  warm/cold load-state truth, and execution-plan digests on responses and
  receipts
- a library-first in-process runtime API now exists in `mox-serve`, with a
  reusable aggregate wrapper over local catalog inspection, loaded-model
  lifecycle, text generation, and embeddings execution
- pull-driven local streaming generation now exists in `mox-serve` and
  `mox-provider`, with explicit backpressure/disconnect/cancellation policy,
  typed chunk-vs-terminal events, partial-output terminal semantics, and
  streaming-policy truth carried in provenance, receipts, and capability
  envelopes
- local runtime failures now have a backend-neutral diagnostics taxonomy in
  `mox-runtime`, `mox-serve`, and `mox-provider`, with stable error codes plus
  status/message/context fields, serve-layer mappings for current request
  failures, and structured diagnostics preserved on streamed terminals and
  provider receipts
- served-product backend selection now has explicit unavailable/degraded
  policy plus direct, same-backend-degraded, and cross-backend-fallback state
  in `mox-runtime`, `mox-provider`, and the CPU/Metal backend seams instead of
  relying on a plain fallback string alone
- backend runtime resources now have explicit allocator-pool policy/state,
  kernel-cache policy/state, and device-memory-budget reporting in
  `mox-runtime`, with CPU and Metal backends surfacing pooled-intermediate
  reuse and bounded cache truth instead of treating those policies as hidden
  backend internals
- typed backend-extension ops now exist for normalization, RoPE, attention,
  and quantized matmul, with explicit backend-extension capability truth on
  `BackendSelection` and CPU reference execution for those semantics while
  later accelerator issues add backend-specialized kernels
- cutover performance gates now exist in the conformance harness, with
  Ollama-aligned timing metrics retained on Mox responses/receipts and
  normalized observations plus default ratio-based thresholds for generation
  and embeddings before desktop cutover
- embeddings execution now has explicit empty-batch behavior, requested
  output-dimension handling, ordered-batch truth, model-family/revision and
  normalization metadata, and provider-facing capability/receipt reporting for
  those semantics
- text-generation sessions in `mox-serve` now bind to full decoder identity
  and own their token sequence alongside KV state instead of relying on
  model-name-only cache reuse
- text-generation KV state now has an explicit logical page layout, byte/page
  growth accounting, and `refuse_new_pages` policy surfaced through
  `mox-runtime`, `mox-serve`, and `mox-provider`
- shared prompt-prefix reuse now has explicit identity, policy, state, and
  reused-token evidence in `mox-runtime`, `mox-serve`, and `mox-provider`,
  with longest-safe prefix reuse on the CPU reference path and explicit bypass
  when session-owned KV state is already populated
- GGUF tokenizer metadata loading exists for SentencePiece and GPT-style BPE
  families, and a real golden tokenizer/prompt-template fixture corpus now
  exists with GGUF chat-template extraction, stop-default references, and
  prompt/window assertion helpers
- supported GGUF prompt rendering now exists for the first truthful template
  families, with digest-gated Phi-3 / Qwen2 / Command-R compatibility and
  explicit refusal of unsupported GGUF template digests
- reusable GGUF decoder-family adapters now exist for the first launch
  Llama/Qwen/Mistral families, including explicit family metadata, reusable
  tensor layouts, tokenizer/template attachment, and explicit refusal of
  unsupported llama MoE artifacts
- reusable GGUF embedding-family adapters now exist for the first launch
  BERT/Nomic-BERT encoder families, including explicit pooling and
  normalization truth, reusable tensor layouts, attached tokenizer metadata,
  and explicit refusal of unsupported Nomic MoE artifacts
- a reusable conformance harness now exists for `tags` / `show` / `ps` /
  `generate` / `embed`, with a live Ollama HTTP adapter, fixture-driven prompt
  render cases, and explicit intentional-difference reporting
- a shared backend parity policy now exists for embeddings, logits, and
  generation-outcome classes, and current Metal embeddings parity uses that
  policy instead of a backend-local tolerance constant
- Metal now has a truthful accelerated embeddings path plus a shipped tested
  `mox.text_generation` product path for the current dense matmul/add graph,
  with seeded CPU-vs-Metal parity coverage and explicit Metal unavailability
  diagnostics instead of silent CPU fallback
- a minimum shipped hardware validation matrix now exists across CPU, Apple
  Silicon Metal, NVIDIA CUDA embeddings, AMD KFD discovery, and refusal paths,
  with provider-facing validation claim IDs tied back to that matrix
- AMD has truthful discovery/readiness surfaces, but not execution kernels
- provider-facing capability and receipt truth is ahead of the app cutover
- Mox still does not replace the desktop's Ollama dependency

## What Still Blocks Full Ollama Replacement

The remaining gaps are not "make it faster." They are mostly behavioral
contract, compatibility, lifecycle, and cutover work.

### Behavioral contract and runtime truth

- process isolation policy for in-process versus subprocess local serving
- allowed fallback lattice beyond the current direct / degraded / fallback
  selection taxonomy
- served-artifact identity and reproducibility tuple for model blob,
  tokenizer, chat template, generation defaults, quant format, and
  backend/toolchain version
- cache/state upgrade invalidation for plans, kernels, paged tensors, and
  persisted runtime state
- model provenance/license gating for what local artifacts may be advertised or
  served
- the long-term boundary between Ollama-compat support and Mox-native model /
  runtime formats

### Accelerator coverage

- AMD KFD served-product execution, capability gating, parity, and the first
  shipped AMD path while keeping AMD userspace explicitly gated
- keeping the minimum hardware validation matrix green as backend claims widen

### Desktop cutover and compute-market substrate

- app-owned local runtime seam instead of Ollama HTTP calls
- rename/remove remaining Ollama-specific app contracts and wording
- compute-market capability qualifiers, batching truth, topology truth,
  warm/cold cache truth, and delivery-proof evidence
- stable execution-plan digests, kernel counts, bytes moved, plan-cache
  hit/miss, and KV-growth evidence

## Ollama Behaviors That Must Become Explicit

The missing work is not just "run the model in Rust." Ollama currently supplies
several behavioral contracts implicitly, and Mox needs explicit semantics for
them.

From the local Ollama source reviewed for this roadmap:

- prompt-template sourcing is not trivial:
  - `convert/tokenizer.go` accepts `chat_template` from `tokenizer_config.json`
    either as a plain string or as a named list and selects the `"default"`
    template when present
  - tokenizer metadata also carries BOS/EOS and `add_bos_token` /
    `add_eos_token` behavior, while `generation_config.json` can override EOS
    token IDs
- chat truncation has defined semantics:
  - `server/prompt.go` drops old messages from the front until the rendered
    prompt fits
  - it preserves system messages and always keeps the latest message
  - image-bearing prompts count image token cost during truncation
- embeddings have their own over-limit behavior:
  - `server/routes.go` tokenizes input, adjusts context budget for BOS/EOS
    insertion, optionally truncates, errors when truncation cannot make the
    input fit, normalizes vectors, and optionally re-normalizes dimension-cut
    outputs
- streaming has wire semantics:
  - `streamResponse` uses `application/x-ndjson`
  - chunks are newline-delimited JSON objects
  - errors before the first chunk are returned as normal JSON errors
  - errors after streaming starts are emitted as streamed JSON error objects and
    terminate the stream
  - final chunks carry `done` and `done_reason`
- scheduling has admission and residency policy:
  - `server/sched.go` enforces a max pending queue, max loaded model count, and
    one active model load at a time
  - keepalive and load timeout are operational inputs, not optional extras
- memory planning already exists as structured data:
  - `ml/device.go` exposes per-device memory usage and `ErrNoMem`
- model-store integrity already matters:
  - `manifest` and `server/internal/cache/blob` validate digest format and blob
    content, handle missing files, and carry compatibility logic for manifests
    and blob-addressable storage

That is why the roadmap still needs explicit work for embeddings, integrity,
errors, fallback, and the remaining cutover/runtime truth gaps.

## Tinygrad Findings That Expand Compute Market Scope

Reviewing `~/code/tinygrad` adds scope beyond simple Ollama parity:

- `gguf_load` and `ggml_data_to_tensor` make GGUF parsing plus quant block
  decode a first-class runtime concern, not a thin metadata shim
- the Llama path makes KV-cache lifecycle, `start_pos` accounting, and the
  token-by-token JIT fast path explicit
- `apply_graph_to_jit`, `GraphRunner`, and `MultiGraphRunner` show that stable
  replayable execution plans should be a productized runtime primitive
- `GlobalCounters` and the realize path make per-request kernel count, memory
  traffic, and timing evidence straightforward to collect
- the disk device and disk-backed GGUF loading path show a credible route to
  mmap-backed model ingestion and later spill-style storage
- the AMD runtime docs make interface risk explicit, including the unsafe
  userspace driver path that may unbind `amdgpu`

That adds missing roadmap work for:

- capability-envelope qualifiers beyond simple backend selection
- batch posture, queue discipline, and throughput-class truth
- multi-device or sharded execution planning
- execution-plan caching and warm/cold compile-path evidence
- runtime evidence for compute delivery proofs
- a bounded execution-profile model for later `sandbox_execution`

## Candle Findings That Tighten Runtime And Backend Scope

Reviewing `~/code/candle` sharpens several implementation buckets that the
existing roadmap treated too loosely:

- GGUF/GGML loading in Candle is tied to backend-backed quantized tensor
  storage, not just metadata parsing; the first Mox pass of that landed in
  `MOX-115`, but full GGUF loader coverage still remains
- tokenizer reconstruction from GGUF metadata already carries BOS/EOS and
  template-processing implications, which means Mox should not treat tokenizer
  loading and prompt rendering as one issue
- seeded sampler utilities, repeat penalty, and GQA helpers show that sampler
  correctness needs to include the surrounding decode helpers, not just RNG
- explicit compile-time backend features (`metal`, `cuda`, `cudnn`, `nccl`,
  `accelerate`, `mkl`) are a strong model for compiled-vs-probed capability
  truth
- Metal buffer pooling, bounded caches, and device-memory budget work show that
  backend allocator policy is a roadmap item, not a cleanup task
- Candle's custom-op and fused-kernel escape hatches are a practical template
  for how Mox should land backend-specific attention and quantized GEMM kernels

That means the roadmap should explicitly keep tracking:

- memory-mapped model blob access and paged tensor storage
- backend allocator pooling, kernel-cache bounds, and device-memory-budget
  reporting
- a fused/custom-op surface for backend-specific kernels

## GitHub-Backed Roadmap Items

Every roadmap item below now has a GitHub issue. The GitHub column maps the
authoritative issue to use for implementation. When older closed duplicates
exist, this section lists the open detailed issue and the duplicate handling is
called out in the status section above.

### Epic A: GGUF, tokenizer, prompt, and conformance baseline

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `MOX-110` | [#3172](https://github.com/OpenAgentsInc/openagents/issues/3172) | Closed | Add `WeightFormat::Gguf` and a reusable GGUF metadata/tensor loader | `mox-models` | Required to read the format Ollama actually points at during migration. |
| `MOX-111` | [#3173](https://github.com/OpenAgentsInc/openagents/issues/3173) | Closed | Implement tokenizer loading from GGUF metadata for SentencePiece and GPT-style BPE families | `mox-models` | Fixture tokenizers are not enough for real model parity. |
| `MOX-115` | [#3164](https://github.com/OpenAgentsInc/openagents/issues/3164) | Closed | Add GGML/GGUF quant block decode coverage and backend-backed quantized tensor storage | `mox-models`, `mox-runtime`, backend crates | Candle and Tinygrad both treat quantized tensor decode/storage as a core loader/runtime boundary; the first Mox pass is now landed. |
| `MOX-116` | [#3165](https://github.com/OpenAgentsInc/openagents/issues/3165) | Closed | Add memory-mapped model blob access and paged tensor storage for local GGUF and Ollama blobs | `mox-catalog`, `mox-models`, `mox-runtime` | Large local models now load through mmap-or-buffered blob access with explicit paging and storage-truth metadata. |
| `MOX-118` | [#3167](https://github.com/OpenAgentsInc/openagents/issues/3167) | Closed | Add golden prompt-rendering and tokenizer fixtures for supported model families from real GGUF and Ollama installs | `mox-models`, `mox-serve`, test fixtures | Prompt and tokenizer behavior drifts silently without a real golden corpus, and `MOX-117` depends on these fixtures. |
| `MOX-117` | [#3166](https://github.com/OpenAgentsInc/openagents/issues/3166) | Closed | Build an Ollama-to-Mox conformance suite for `tags` / `show` / `ps` / `generate` / `embed` behavior, prompt rendering, truncation, stop handling, streaming, and error semantics | `mox-catalog`, `mox-serve`, `mox-provider`, test fixtures | Cutover should be decided by repeatable conformance evidence, not hand inspection. |
| `MOX-119` | [#3168](https://github.com/OpenAgentsInc/openagents/issues/3168) | Closed | Define numerical parity tolerances and drift budgets across CPU and accelerated backends for embeddings and text generation | `mox-serve`, backend crates, `mox-provider` | Backend parity needs explicit tolerance rules across quant modes, decode loops, and embeddings outputs. |
| `MOX-112` | [#3177](https://github.com/OpenAgentsInc/openagents/issues/3177) | Closed | Add GGUF-backed decoder model-family adapters for first launch families (`llama`, `qwen`, `mistral`) | `mox-models`, `mox-serve` | Replaces model-family construction still hidden behind Ollama. |
| `MOX-113` | [#3178](https://github.com/OpenAgentsInc/openagents/issues/3178) | Closed | Add GGUF-backed embeddings model-family adapters for the first supported embedding families | `mox-models`, `mox-serve` | Keeps embeddings real rather than demo-only. |
| `MOX-114` | [#3179](https://github.com/OpenAgentsInc/openagents/issues/3179) | Closed | Implement chat-template extraction and prompt-rendering compatibility for supported model families | `mox-models`, `mox-serve` | Landed in `af6d82a42`: reusable GGUF prompt rendering for supported Phi-3 / Qwen2 / Command-R template digests, explicit prompt/render/error types, decoder render helpers, and conformance coverage that removed the old prompt-render gap. |

### Epic B: Ollama-compatible catalog and local runtime lifecycle

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `MOX-120` | [#3180](https://github.com/OpenAgentsInc/openagents/issues/3180) | Closed | Add Ollama manifest/blob discovery and model resolution on top of `mox-catalog` | `mox-catalog` | Landed in `859dc16c5`: non-mutating local manifest discovery and model resolution, Ollama-compatible default name normalization, parsed layer/media-type records, blob-presence truth, and explicit scan warnings for invalid entries. |
| `MOX-121` | [#3181](https://github.com/OpenAgentsInc/openagents/issues/3181) | Closed | Implement installed-model listing and inspection APIs equivalent to `tags` and `show` | `mox-catalog`, `mox-serve` | Landed in `d78ac7965`: manifest-layer config/text/json decode helpers in `mox-catalog`, plus a local `tags` / `show` subject in `mox-serve` that reads the shared catalog directly, derives GGUF model-info/capability truth without the Ollama daemon, skips bad config blobs during listing, and adds fixture-backed local list/show tests. |
| `MOX-122` | [#3182](https://github.com/OpenAgentsInc/openagents/issues/3182) | Closed | Implement loaded-model registry, warm/load/unload, and keepalive semantics equivalent to `ps` and warmups | `mox-serve`, `mox-runtime` | Landed in `eb921c9e8`: explicit loaded-model residency truth, warm/load/unload registry operations, `ps`-style ordering, and Ollama-aligned idle expiry semantics. |
| `MOX-123` | [#3183](https://github.com/OpenAgentsInc/openagents/issues/3183) | Closed | Expand generation options to cover `temperature`, `top_k`, `top_p`, penalties, `seed`, and `stop` | `mox-serve` | Landed in `cf986e282`: explicit option fields, seeded sample mode, penalty-adjusted logits, and stop-sequence truncation on the CPU reference path. |
| `MOX-124` | [#3184](https://github.com/OpenAgentsInc/openagents/issues/3184) | Closed | Add generation metrics and provenance for prompt tokens, output tokens, load time, total time, warm/cold state, and plan digest | `mox-serve`, `mox-provider` | Preserves truthful receipts and UI projections after cutover. |
| `MOX-125` | [#3185](https://github.com/OpenAgentsInc/openagents/issues/3185) | Closed | Publish a library-first local runtime API for `list_models`, `show_model`, `loaded_models`, `warm_model`, `unload_model`, `generate`, and `embed` | `mox-serve`, `mox-provider` | Creates the in-process replacement boundary the app can call directly. |
| `MOX-126` | [#3186](https://github.com/OpenAgentsInc/openagents/issues/3186) | Closed | Add GGUF-backed KV-cache ownership and deterministic session lifecycle for text generation | `mox-serve` | Landed in `cd3987928`: deterministic session ownership bound to full decoder identity, token-sequence ownership alongside KV state, explicit cache-plus-token commits, and descriptor-drift refusal coverage. |
| `MOX-126A` | [#3169](https://github.com/OpenAgentsInc/openagents/issues/3169) | Closed | Add paged KV-cache layout, accounting, and spill policy for long-context text generation | `mox-serve`, `mox-runtime`, `mox-provider` | Landed in `0dfcf3f7c`: runtime-owned paged-KV policy/accounting, logically paged per-session KV state, explicit `refuse_new_pages` behavior, and generation/provider evidence for KV pages, bytes, and growth. |
| `MOX-126B` | [#3231](https://github.com/OpenAgentsInc/openagents/issues/3231) | Closed | Add shared prompt-prefix cache identity, reuse policy, accounting, and truth surfaces | `mox-serve`, `mox-runtime`, `mox-provider` | Landed in `2bd89d48f`: runtime-owned prefix-cache policy/state/identity, longest-safe shared prefix reuse with stale rebuild and bypass handling in `mox-serve`, and provider/receipt evidence for prefix reuse and reused-token counts. |
| `MOX-165` | [#3236](https://github.com/OpenAgentsInc/openagents/issues/3236) | Closed | Add OCI-distribution registry pull and model-ingestion pipeline (Ollama-compatible manifest + blobs) | `mox-catalog`, `mox-models` | Landed in `e4ffbee5b`: OCI/Docker-v2 manifest/blob pull into the local Ollama-style store, shared manifest validation for local and remote paths, digest/size-checked blob ingestion with reuse reporting, and manifest-based GGUF loading in `mox-models`. |

### Epic C: Behavioral contract and serving policy

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `MOX-127` | [#3187](https://github.com/OpenAgentsInc/openagents/issues/3187) | Closed | Add explicit context-window accounting, truncation policy, and over-limit error semantics | `mox-models`, `mox-serve` | Landed in `bf0cf75a8`: reusable context-window budgeting in `mox-models`, explicit `refuse` vs `truncate_oldest` prompt-overflow policy in `mox-serve`, Ollama-aligned over-limit error strings, and regression coverage for truncation and session-owned context pressure. |
| `MOX-128` | [#3188](https://github.com/OpenAgentsInc/openagents/issues/3188) | Closed | Add deterministic sampler implementation and replay coverage for supported generation options | `mox-serve`, `mox-runtime` | Landed in `9e283787a` and `875de50b9`: runtime-owned sampler policy and seeded replay behavior in `mox-runtime`, Ollama-aligned defaults/transform order plus bounded penalty lookback, `mox-serve` delegation to that runtime sampler, and full replay coverage for the supported option surface. |
| `MOX-129` | [#3189](https://github.com/OpenAgentsInc/openagents/issues/3189) | Closed | Add model memory planning, residency policy, and admission control for local serving | `mox-serve`, `mox-runtime`, `mox-provider` | Landed in `a12badddb`: reusable memory-plan and residency-policy substrate in `mox-runtime`, admission-aware loaded-model registry behavior in `mox-serve`, bounded-budget refusal plus optional idle-oldest eviction, default decoder memory planning, and capability/receipt evidence carrying memory-plan and residency-snapshot truth. |
| `MOX-133` | [#3190](https://github.com/OpenAgentsInc/openagents/issues/3190) | Closed | Add streaming token generation, backpressure, disconnect, and cancellation semantics for the local runtime API | `mox-serve`, `mox-provider` | Landed in `eb0f84af2`: pull-driven local streaming generation, explicit backpressure/disconnect/cancellation policy, typed chunk and terminal events, partial-output terminal semantics, and provider receipt/capability streaming truth. |
| `MOX-134` | [#3191](https://github.com/OpenAgentsInc/openagents/issues/3191) | Closed | Add embeddings API parity, batch semantics, and model metadata reporting | `mox-serve`, `mox-provider` | Landed in `1600ec4bc`: empty-batch success, requested output dimensions with re-normalization, explicit model-family/revision/normalization metadata, ordered-batch capability truth, and explicit no-input-truncation support on current byte-projection embeddings paths. |
| `MOX-135` | [#3192](https://github.com/OpenAgentsInc/openagents/issues/3192) | Closed | Add local model-store integrity verification and cache-repair diagnostics | `mox-catalog`, `mox-models` | Landed in `2516c70e3`: structured per-model integrity diagnostics and repair hints in `mox-catalog`, plus `mox-models` refusal of corrupt primary GGUF blobs from manifest-backed loads. |
| `MOX-136` | [#3193](https://github.com/OpenAgentsInc/openagents/issues/3193) | Closed | Define backend-neutral local runtime error taxonomy and desktop-facing diagnostics | `mox-serve`, `mox-provider`, `mox-runtime` | Landed in `74ebe5cf9`: runtime-owned diagnostics taxonomy with stable error codes plus HTTP-style status/message/context, serve-layer mappings from current request failures into that taxonomy, streaming-terminal diagnostics, and provider receipts preserving structured diagnostics alongside plain-text reasons. |
| `MOX-137` | [#3194](https://github.com/OpenAgentsInc/openagents/issues/3194) | Closed | Add explicit backend fallback, refusal, and degraded-state policy for served products | `mox-runtime`, `mox-provider`, backend crates | Landed in `b91fe2c4d`: explicit served-product unavailable/degraded policy enums, direct / same-backend-degraded / cross-backend-fallback state in `mox-runtime`, provider capability truth that carries those fields instead of a plain string alone, and CPU/Metal backend selection/tests that make fallback and degraded execution machine-checkable. |
| `MOX-138` | [#3195](https://github.com/OpenAgentsInc/openagents/issues/3195) | Closed | Define performance acceptance thresholds and cutover gates for Mox runtime replacement | `mox-serve`, `mox-provider`, backend crates | Landed in `f488763b0`: Ollama-aligned generation/embeddings timing metrics in Mox responses and receipts, conformance observations that retain that evidence instead of dropping it, ratio-based default performance thresholds, and `ConformanceReport::cutover_ready_with_performance(...)` for explicit semantic-plus-performance cutover gating. |
| `MOX-139` | [#3196](https://github.com/OpenAgentsInc/openagents/issues/3196) | Closed | Decide and document LoRA/adapter support policy for the Ollama replacement boundary | `mox-models`, `mox-catalog`, `mox-serve` | Landed in `cbfc30a6d`: explicit manifest adapter-policy helpers in `mox-catalog`, explicit refusal of adapter-bearing Ollama manifests in `mox-models`, and `mox-serve` show-policy facts plus conformance handling for extra Mox-only evidence. |
| `MOX-157` | [#3197](https://github.com/OpenAgentsInc/openagents/issues/3197) | Closed | Add backend allocator pooling, bounded kernel caches, and device-memory-budget reporting | `mox-runtime`, backend crates, `mox-provider` | Landed in `d54614807`: explicit runtime-resource truth in `mox-runtime`, exact-spec allocator pooling for CPU and Metal intermediate buffers, bounded kernel-cache reporting, device-memory-budget reporting, and provider-visible serialization/tests. |
| `MOX-158` | [#3198](https://github.com/OpenAgentsInc/openagents/issues/3198) | Closed | Add a fused/custom-op extension surface for backend-specific attention, quantized GEMM, RoPE, and normalization kernels | `mox-compiler`, `mox-runtime`, backend crates | Landed in `1f6c6e9fe`: typed backend-extension ops in the graph/plan layer, explicit backend-extension capability truth on `BackendSelection`, CPU reference execution for those families, and a path for later backend-specialized kernels without polluting the primitive-op surface. |
| `MOX-159` | [#3199](https://github.com/OpenAgentsInc/openagents/issues/3199) | Closed | Add local runtime observability for warm/cold transitions, active sessions, queue depth, memory footprint, and backend health changes | `mox-serve`, `mox-provider`, `mox-runtime` | Landed in `a7b73314f`: reusable runtime observability types and health tracking in `mox-runtime`, lifecycle transition tracking plus a managed-runtime `observability()` surface in `mox-serve`, and provider-facing observability envelope serialization/tests. |

### Epic D: Quantized execution and accelerated backends after the merged baseline

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `MOX-156` | [#3170](https://github.com/OpenAgentsInc/openagents/issues/3170) | Closed | Add backend-specific quantized execution kernels and parity coverage for supported GGUF quant families | `mox-compiler`, `mox-runtime`, backend crates, `mox-serve` | Landed in `d3329e658`: first-class quantized GGML/GGUF constants, Candle-aligned row-wise quantized matmul semantics, CPU-native `Q4_0` / `Q4_1` / `Q8_0` kernels over preserved blocks, explicit CPU quantized capability truth, and explicit Metal quantized-constant refusal. |
| `MOX-130` | [#3200](https://github.com/OpenAgentsInc/openagents/issues/3200) | Closed | Add Metal lowering/kernel coverage for the minimum text-generation primitive set | `mox-backend-metal`, `mox-compiler`, `mox-runtime` | Landed in `5d775fd13`: a distinct Metal text-generation dense-surface contract, shared dense pipeline/kernel-cache naming, explicit Metal-versus-CPU selection/fallback coverage, and direct Metal execution coverage for the current text-generation matmul/add graph. |
| `MOX-131` | [#3201](https://github.com/OpenAgentsInc/openagents/issues/3201) | Closed | Add CPU-vs-Metal parity coverage for the supported text-generation product path | `mox-backend-metal`, `mox-serve` | Landed in `ef1c503fa`: a dedicated `metal_text_generation_parity` integration test, seeded exact token/text/termination parity against the CPU reference path, policy-backed hidden/logit drift checks, and macOS-target Metal import fixes found by cross-target compilation. |
| `MOX-132` | [#3202](https://github.com/OpenAgentsInc/openagents/issues/3202) | Closed | Ship a tested Metal-backed `mox.text_generation` path | `mox-backend-metal`, `mox-serve`, `mox-provider` | Landed in `7466a16b1`: a real `MetalModelTextGenerationService`, shared CPU/Metal non-streaming generation flow with truthful session/prefix-cache/provenance behavior, explicit Metal diagnostics, and provider-facing success/unavailability coverage. |
| `MOX-140` | [#3203](https://github.com/OpenAgentsInc/openagents/issues/3203) | Closed | Mox phase 5: NVIDIA backend architecture and truthful capability surfaces | `mox-backend-cuda` or equivalent, `mox-runtime`, `mox-provider` | Landed in `c10e32dbf`: new `mox-backend-cuda` architecture crate, first-class `DeviceKind::Cuda`, runtime/provider backend-selection truth for `cuda`, and an explicit architecture-only offline state before NVIDIA discovery or execution. |
| `MOX-141` | [#3204](https://github.com/OpenAgentsInc/openagents/issues/3204) | Closed | Define the Mox NVIDIA capability, topology, and risk model | `mox-runtime`, `mox-provider` | Landed in `c3ff379b1`: reusable NVIDIA topology/risk/recovery metadata in `mox-runtime`, `nvidia_metadata` on runtime device descriptors, and provider-visible `nvidia` capability/receipt context for later CUDA discovery and routing work. |
| `MOX-142` | [#3205](https://github.com/OpenAgentsInc/openagents/issues/3205) | Closed | Implement NVIDIA discovery and health reporting | `mox-backend-cuda` | Landed in `c02562325`: real `nvidia-smi`-backed CUDA discovery in `mox-backend-cuda`, explicit ready/degraded/offline health, runtime device descriptors populated from live NVIDIA query data, and degraded-state truth for display-attached or MIG-partitioned devices. |
| `MOX-143` | [#3206](https://github.com/OpenAgentsInc/openagents/issues/3206) | Closed | Add CUDA allocator, buffer, stream, and command submission substrate | `mox-backend-cuda`, `mox-runtime` | Landed in `0ceef490d`: dynamic `libcudart`-backed CUDA buffers and stream submission in `mox-backend-cuda`, explicit allocator/runtime-resource truth, and end-to-end staged write plus device-to-device copy coverage on the selected NVIDIA device. |
| `MOX-144` | [#3207](https://github.com/OpenAgentsInc/openagents/issues/3207) | Closed | Add CUDA lowering and kernel coverage for the minimum served-product primitive set | `mox-backend-cuda`, `mox-compiler` | Landed in `a9a35c44b`: explicit CUDA dense-surface plan validation, `ExecutionBackend` and `compile_and_execute` support, dense CUDA input/constant materialization, Candle-aligned cuBLAS matmul lowering, cuBLAS-backed add coverage, and live end-to-end execution tests. |
| `MOX-145` | [#3208](https://github.com/OpenAgentsInc/openagents/issues/3208) | Closed | Wire NVIDIA backend selection and truthful capability reporting through Mox | `mox-runtime`, `mox-provider`, `mox-backend-cuda` | Landed in `50a9d3c63`: explicit direct/degraded/fallback CUDA backend-selection surfaces in `mox-backend-cuda`, plus provider capability/receipt coverage for direct CUDA, same-backend degraded CUDA, and explicit CPU fallback states. |
| `MOX-146` | [#3209](https://github.com/OpenAgentsInc/openagents/issues/3209) | Closed | Add CPU-vs-NVIDIA parity coverage for the first supported served product path | `mox-backend-cuda`, `mox-serve` | Landed in `a29f797b4`: a model-backed CUDA embeddings parity test in `mox-serve`, shared embedding-drift-budget comparison against the CPU baseline, and explicit CPU fallback truth when CUDA is unavailable. |
| `MOX-147` | [#3210](https://github.com/OpenAgentsInc/openagents/issues/3210) | Closed | Ship the first tested NVIDIA-backed served product path | `mox-backend-cuda`, `mox-serve`, `mox-provider` | Landed in `181d1127e`: a real `CudaModelEmbeddingsService`, CUDA-specific embeddings error/diagnostic handling, shared graph execution over the CUDA backend, and integration coverage for successful response/capability/receipt flow or explicit backend unavailability. |
| `MOX-148` | [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232) | Closed | Define and keep a minimum hardware validation matrix for CPU, Apple Silicon, NVIDIA, AMD KFD, and refusal paths | backend crates, `mox-serve`, test fixtures | Landed in `0a8e3b700`: `mox.minimum_hardware_validation.v1` in `mox-runtime`, provider-facing validation references on capability/receipt surfaces, claim coverage for CPU/Metal/CUDA/AMD KFD/refusal lanes, and a documented host-class lab runbook in `crates/mox/docs/HARDWARE_VALIDATION_MATRIX.md`. |
| `MOX-150` | [#3211](https://github.com/OpenAgentsInc/openagents/issues/3211) | Closed | Mox phase 6: AMD served-product execution path | `mox-backend-amd-kfd`, `mox-backend-amd-userspace`, `mox-runtime`, `mox-provider` | Landed in `2b46505f8`: backend-owned AMD staging allocation and explicit fill/copy submission substrate in both AMD backends, explicit allocator/kernel-cache/device-budget truth for AMD substrate paths, explicit CPU fallback helpers, and provider/runtime-resource coverage while served-product execution remains for `MOX-151` through `MOX-154`. |
| `MOX-151` | [#3212](https://github.com/OpenAgentsInc/openagents/issues/3212) | Closed (Not Planned) | Add AMD KFD lowering and kernel coverage for the first supported primitive set | `mox-backend-amd-kfd`, `mox-compiler`, `mox-runtime` | Closed after reprioritizing this host to NVIDIA-only execution and validation; keep the substrate from `MOX-150`, but do not treat KFD lowering as active queued work here. |
| `MOX-152` | [#3213](https://github.com/OpenAgentsInc/openagents/issues/3213) | Closed (Not Planned) | Wire served-product capability gating for AMD KFD separately from AMD userspace | `mox-provider`, `mox-runtime`, AMD backend crates | Closed with the same reprioritization; the KFD/userspace split remains historically relevant but is not active follow-on work on this host. |
| `MOX-153` | [#3214](https://github.com/OpenAgentsInc/openagents/issues/3214) | Closed (Not Planned) | Add CPU-vs-AMD KFD parity coverage for the first supported served product path | `mox-backend-amd-kfd`, `mox-serve` | Closed with the same reprioritization; do not advertise or test AMD served-product parity on this machine. |
| `MOX-154` | [#3215](https://github.com/OpenAgentsInc/openagents/issues/3215) | Closed (Not Planned) | Ship the first tested AMD KFD-backed served product path and keep AMD userspace explicitly gated | `mox-backend-amd-kfd`, `mox-serve`, `mox-provider` | Closed with the same reprioritization; the active accelerator execution path remains NVIDIA-only on this machine. |

### Epic E: App cutover and long-term boundary

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `OA-200` | [#3216](https://github.com/OpenAgentsInc/openagents/issues/3216) | Closed | Rename `OllamaExecutionMetrics` and `OllamaExecutionProvenance` to backend-neutral names | `apps/autopilot-desktop` | Landed in `3de54fdb4`: app-owned local execution evidence types are now backend-neutral while the current worker implementation remains explicitly Ollama-backed. |
| `OA-201` | [#3217](https://github.com/OpenAgentsInc/openagents/issues/3217) | Closed | Introduce an app-owned `LocalInferenceRuntime` trait and `MoxRuntimeAdapter` | `apps/autopilot-desktop` | Landed in `9c2d1f9d1`: added an app-owned local inference runtime seam, an Ollama-backed adapter for the current worker, an in-process Mox adapter, and desktop plumbing that now routes refresh/generate/warm/unload through the trait instead of the concrete worker. |
| `OA-202` | [#3218](https://github.com/OpenAgentsInc/openagents/issues/3218) | Closed | Switch desktop default from external Ollama HTTP calls to the in-process Mox runtime | `apps/autopilot-desktop` | Landed in `22c25d2f4`: the desktop now instantiates the app-owned local inference seam with the in-process Mox reference runtime by default, without silently falling back to the external Ollama worker, and a unit test now pins that default. |
| `OA-203` | [#3219](https://github.com/OpenAgentsInc/openagents/issues/3219) | Closed | Remove the external Ollama dependency and clean up provider/UI wording | `apps/autopilot-desktop` | Landed in `b651786e7`: the production desktop now gates the old `ollama_execution` module behind tests, backend-neutral local inference evidence/helpers live under the app-owned runtime seam, and user-facing defaults, health events, and pane wording now say Mox/local inference instead of implying external Ollama is still the default product path. |
| `MOX-160` | [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220) | Closed | Define in-process vs subprocess isolation policy for Mox local serving | `mox-serve`, `mox-runtime`, backend crates | Landed in `90224ae2e`: reusable local-serving isolation policy, explicit `in_process` crash/reset truth in observability and generation provenance, aggregate runtime isolation reporting, and documented cutover/evidence implications for the no-subprocess decision on current Mox. |
| `MOX-161` | [#3171](https://github.com/OpenAgentsInc/openagents/issues/3171) | Closed | Define allowed fallback lattice for Mox served products: refuse, degrade, replan, retry, or same-backend slow path | `mox-runtime`, `mox-provider`, `mox-serve` | Landed in `220286d8a`: reusable backend-neutral fallback lattice types, explicit trigger/action/state truth across runtime/provider/serve, refusal-aware validation mapping, and a documented fallback boundary for cutover/evidence work. |
| `MOX-162` | [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233) | Closed | Define the served-artifact identity and reproducibility tuple for model blob, tokenizer, template, defaults, quantization, and backend/toolchain version | `mox-models`, `mox-serve`, `mox-provider`, `mox-runtime` | Landed in `0dfeb6023`: added first-class served-artifact identity and backend-toolchain tuples, threaded descriptor-side artifact identity through capabilities/receipts/provenance, and keyed session/prefix/request invalidation to artifact drift instead of display names alone. |
| `MOX-163` | [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234) | Closed | Define cache and persisted-state upgrade invalidation policy for plan caches, kernel caches, paged tensors, and KV state | `mox-runtime`, `mox-serve`, `mox-models`, backend crates | Landed in `60f5831d5`: added reusable cache invalidation policy and cache observations, with explicit scopes/format versions/triggers for plan, kernel, paged-tensor, prefix, and KV caches plus provider/serve evidence surfaces. |
| `MOX-164` | [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235) | Closed | Add model provenance and license gating for locally discovered artifacts and advertised compute-market supply | `mox-catalog`, `mox-provider`, `mox-models` | Landed in `6524685a0`: added explicit local-artifact provenance and declared-license facts in `mox-catalog` and `mox-models`, plus provider-side compute-market supply policy, advertise/serve decisions, and structured refusal diagnostics distinct from integrity and unsupported-format failures. |
| `MOX-170` | [#3222](https://github.com/OpenAgentsInc/openagents/issues/3222) | Closed | Define the boundary between Ollama-compat migration support and the long-term Mox-native model/runtime format | `mox-models`, `mox-catalog`, `mox-serve` | Landed in `279c1763f`: added explicit catalog/ingress/serving/runtime boundary metadata for Ollama migration versus Mox-native execution, surfaced those facts through `show`-style observations, and documented the boundary in the cutover/evidence contract. |

### Epic F: Compute-market execution substrate beyond Ollama parity

See [CONFORMANCE_AND_EVIDENCE_CONTRACT.md](./CONFORMANCE_AND_EVIDENCE_CONTRACT.md)
for the minimum conformance harness scope and runtime evidence schema that
`MOX-117`, `MOX-171` through `MOX-175`, and `OA-201` / `OA-202` must satisfy.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `MOX-171` | [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223) | Closed | Expand Mox capability surfaces for compute-market inventory, topology, and performance qualifiers | `mox-provider`, `mox-runtime` | Landed in `2f07676d8`: added reusable selected-device inventory qualifiers in `mox-runtime`, explicit compile-vs-probe backend-toolchain truth in `mox-provider`, and capability/receipt surfacing of stable device, topology-key, memory-class, and performance-class qualifiers. |
| `MOX-172` | [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224) | Closed | Add batch execution posture, queueing policy, and throughput-class capability reporting | `mox-serve`, `mox-runtime`, `mox-provider` | Landed in `232e36c60`: added runtime-owned execution profiles, `mox-serve` defaults plus observability alignment, and provider capability reporting of batch posture, queue policy, and throughput class for embeddings and text generation. |
| `MOX-173` | [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225) | Closed | Add multi-device and sharded execution planning for supported product paths | `mox-runtime`, `mox-compiler`, backend crates, `mox-provider` | Landed in `e3fff595d`: added explicit `selected_devices` plus `ExecutionTopologyPlan` truth in `mox-runtime`, topology-aware compiled-plan digests in `mox-compiler`, provider capability/receipt surfacing of `selected_devices` and `execution_topology`, and contract coverage for the new multi-device/sharded schema. |
| `MOX-174` | [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226) | Closed | Add execution-plan caching, kernel-cache policy, and warm/cold compile-path evidence | `mox-runtime`, `mox-compiler`, backend crates, `mox-provider` | Landed in `ba3a0d1dd`: added execution-plan cache policy/state in `mox-runtime`, backend-owned plan caching plus compile-path evidence in CPU/Metal/CUDA, `mox-serve` provenance for embeddings and generation compile paths, and provider receipt surfacing of plan-cache/kernel-cache behavior. |
| `MOX-175` | [#3227](https://github.com/OpenAgentsInc/openagents/issues/3227) | Closed | Extend Mox runtime evidence with compute-market delivery-proof fields and settlement-linkage inputs | `mox-serve`, `mox-provider`, `mox-runtime` | Landed in `caff38666`: runtime metrics, serve provenance, and provider receipts now carry direct delivery-proof and settlement-linkage inputs instead of app-local reconstruction. |
| `MOX-176` | [#3228](https://github.com/OpenAgentsInc/openagents/issues/3228) | Closed | Define a reusable Mox execution-profile model for bounded `sandbox_execution` | `mox-runtime`, `mox-provider` | Landed in `75521fbef`: added runtime-owned `SandboxExecutionCapabilityProfile` with explicit isolation, filesystem, network, process, resource, and accelerator-access bounds plus stable profile digests, and a provider-facing `SandboxExecutionCapabilityEnvelope` so future sandbox supply can advertise bounded execution policy without hiding behind app-local defaults. |
| `MOX-177` | [#3229](https://github.com/OpenAgentsInc/openagents/issues/3229) | Closed | Add reusable sandbox-execution receipt and evidence contracts compatible with compute-market supply | `mox-runtime`, `mox-provider` | Landed in `4fc690916`: added runtime-owned sandbox request identity, evidence, exit, and resource-summary contracts with explicit command/environment/input/output digests plus optional execution delivery proof, and a provider-facing `SandboxExecutionReceipt` with deterministic request digests and failure/diagnostic mapping for compute-market supply. |
| `MOX-178` | [#3230](https://github.com/OpenAgentsInc/openagents/issues/3230) | Closed | Add topology-aware substitution and deliverability checks for accelerator-sensitive compute offers | `mox-provider`, `mox-runtime` | Landed in `707072ba5`: added reusable promised accelerator requirements, delivered execution contexts, and exact/compatible-substitution/underdelivered reports in `mox-runtime`, plus provider receipt support so accelerator-sensitive sandbox offers can surface machine-checkable promised-versus-delivered topology and capability differences. |

### Epic G: GPT-OSS Mox-only completion track

This epic exists because the generic Mox cutover is landed, but the concrete
`gpt-oss-20b-mxfp4.gguf` inference path on this NVIDIA host is only proven via
external `~/code/llama.cpp` today. That repo remains a reference and behavior
oracle only, not an acceptable execution dependency for closing this epic. The
follow-on work here is specifically what Mox still lacks after the generic
Ollama-replacement milestones have closed.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `MOX-179` | [#3239](https://github.com/OpenAgentsInc/openagents/issues/3239) | Closed | Add GPT-OSS / OpenAI-MoE GGUF loading and truthful MXFP4/Q8_0 storage | `mox-core`, `mox-models`, `mox-runtime` | Landed in `780479e23`: Mox now accepts `general.architecture = gpt-oss`, reconstructs the OpenAI-MoE metadata/tensor layout, recognizes GGUF `MXFP4`, and preserves mixed `MXFP4` / `Q8_0` bundle truth without faking unsupported execution support. |
| `MOX-180` | [#3240](https://github.com/OpenAgentsInc/openagents/issues/3240) | Closed | Implement Harmony prompt rendering and GPT-OSS channel parsing | `mox-models`, `mox-serve`, `mox-runtime` | Landed in `1140c7f32`: Mox now renders GPT-OSS prompts through the published Harmony Rust crate while preserving Mox-owned prompt/context/message truth, ships real GPT-OSS golden render fixtures from the local GGUF, parses Harmony output from text or token lanes (including streaming), and can carry parsed Harmony structure on served responses without dropping the raw token/text lane. |
| `MOX-181` | [#3237](https://github.com/OpenAgentsInc/openagents/issues/3237) | Open | Add a real GGUF-backed decoder execution model for GPT-OSS in Mox | `mox-models`, `mox-compiler`, `mox-runtime`, `mox-serve` | Mox text generation still runs through the toy fixture decoder path, so even a fully parsed GPT-OSS artifact cannot execute as a real model yet; acceptable closure excludes delegating execution to external runtimes. |
| `MOX-182` | [#3238](https://github.com/OpenAgentsInc/openagents/issues/3238) | Open | Add NVIDIA text-generation kernel coverage for the real GPT-OSS decoder path | `mox-backend-cuda`, `mox-compiler`, `mox-runtime`, `mox-serve` | The current CUDA product path is embeddings-only and the current CUDA primitive surface is too small for GPT-OSS/OpenAI-MoE text generation; acceptable closure excludes using `llama.cpp` CUDA as a sidecar or proxy. |
| `MOX-183` | [#3241](https://github.com/OpenAgentsInc/openagents/issues/3241) | Open | Ship and validate a Mox-only GPT-OSS 20B inference flow on the NVIDIA host | `mox-serve`, `mox-provider`, `mox-runtime`, docs/tests | After the substrate lands, we still need an end-to-end proof that Mox alone can load the local GPT-OSS 20B GGUF, serve completions with Harmony semantics, and validate behavior against local `llama.cpp` without depending on it for execution. |

## Recommended Order

The shortest honest path from today's `main` is:

1. Metal text generation is now landed via `MOX-130` through `MOX-132`; keep
   that lane green while moving to NVIDIA.
2. Land NVIDIA explicitly via `MOX-140` through `MOX-147` and keep `MOX-148`
   green as backend claims widen.
3. Keep the landed AMD substrate from `MOX-150` as historical groundwork, but
   treat `MOX-151` through `MOX-154` as closed-not-planned on this
   NVIDIA-validated host path.
4. Lock process-isolation, fallback-lattice, served-artifact identity,
   cache-invalidation, provenance, and migration-boundary decisions via
   `MOX-160`, `MOX-161`, `MOX-162`, `MOX-163`, `MOX-164`, and `MOX-170`.
5. Land the cutover contract from
   [CONFORMANCE_AND_EVIDENCE_CONTRACT.md](./CONFORMANCE_AND_EVIDENCE_CONTRACT.md)
   before hardening backend and app cutover work.
6. Add compute-market capability and evidence substrate via `MOX-171` through
   `MOX-175`.
7. Cut the app over via `OA-200` through `OA-203`.
8. Only after inference and embeddings are truthful, consider
   `MOX-176` through `MOX-178` for bounded `sandbox_execution`.
9. The generic cutover is landed, but GPT-OSS on this NVIDIA host is still
   external to Mox; `MOX-179` and `MOX-180` are now landed, so finish the remaining GPT-OSS
   completion track in strict order:
   `MOX-181` -> `MOX-182` -> `MOX-183`.

## Definition Of Done For "Replace Ollama"

The external Ollama dependency is not replaced until all of the following are
true:

- Mox can discover installed models from the local Ollama model store during
  migration
- Mox can report installed models, loaded models, and model metadata without
  calling the Ollama daemon
- Mox passes a repeatable Ollama-to-Mox conformance suite for catalog,
  generation, embeddings, prompt rendering, truncation, stop handling,
  streaming, and error semantics
- Mox has golden prompt and tokenizer fixtures for supported model families
  sourced from real GGUF or Ollama installs
- Mox can match or explicitly redefine prompt-template, BOS/EOS, and default
  stop behavior for the supported model families
- Mox has explicit context-window accounting, truncation, and over-limit refusal
  behavior for generation and embeddings
- Mox can warm, load, unload, and keep alive a local model lifecycle
- Mox has explicit paged-KV policy for long-context text generation or explicit
  refusal when that policy is unsupported
- Mox has explicit shared prompt-prefix cache policy and accounting or explicit
  refusal when shared prefix reuse is unsupported
- Mox can decide whether a model may load based on memory planning, residency
  policy, and admission control
- Mox can execute the current text-generation path with the option surface the
  desktop already uses
- Mox can stream partial output, slow-reader handling, disconnect behavior, and
  cancellation with stable final-chunk semantics
- metrics, receipts, capability surfaces, error taxonomy, and fallback states
  remain truthful
- Mox exposes a served-artifact identity tuple for model blob, tokenizer, chat
  template, generation defaults, quant format, and backend/toolchain version
- model-store integrity verification and corruption diagnostics exist for the
  local catalog path
- cache and persisted-state upgrade invalidation is explicit for execution
  plans, kernel caches, paged tensors, and KV state
- performance acceptance thresholds for cutover are defined and met
- the desktop uses an app-owned local runtime seam instead of `reqwest` calls
  to Ollama
- the app no longer advertises "Ollama" when the runtime is now an in-repo Rust
  engine
- the repo has an explicit boundary between temporary Ollama-compat migration
  support and the long-term Mox-native model/runtime format

## Additional Definition Of Done For Mox As Compute-Market Substrate

Mox is not yet a credible compute-market substrate until all of the following
are also true:

- Mox can publish truthful capability-envelope fields for backend family,
  execution kind, model family or policy, accelerator vendor/family, compiled
  backend/toolchain features, memory, topology, concurrency posture, and
  latency posture
- batchability, queueing, admission, warm/cold behavior, and compile/cache
  posture are explicit runtime policy
- runtime evidence includes stable digests and summaries that can feed
  compute-market delivery proofs without re-deriving execution truth in app code
- runtime evidence includes execution-plan digest, compile digest, kernel count,
  bytes moved, queue wait, warm/cold load state, KV growth, backend/interface
  mode, and refusal/degraded reason codes
- shared prompt-prefix reuse, cache-hit state, and warm/cold posture are
  explicit and machine-checkable
- multi-device or sharded execution is either explicitly supported for a product
  path or explicitly refused with stable diagnostics
- accelerator-sensitive offers can compare promised versus delivered topology
  and capability truth
- supported backend claims stay green against a minimum hardware validation
  matrix that includes CPU, Apple Silicon, NVIDIA, AMD KFD, and refusal paths
- model provenance and license gating is explicit for what local artifacts may
  be advertised or served into the compute market
- if `sandbox_execution` is added later, Mox exposes a bounded execution
  profile and machine-checkable evidence surface

## Non-Goals

This roadmap does not require:

- porting Ollama cloud or registry flows
- Modelfile parity as a first milestone
- OpenAI-compatible HTTP endpoints as the first milestone
- multimodal parity
- multi-runner LRU complexity before the one-model MVP lifecycle is solid
- raw accelerator trading before backend-specific compute products and
  capability envelopes are truthful

The right near-term target is smaller:

- one honest in-process local runtime
- one honest model catalog
- one honest text-generation product path
- explicit Metal, NVIDIA, and AMD backend truth
