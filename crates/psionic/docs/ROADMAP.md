# Psionic Roadmap

> Status: updated 2026-03-14 after re-verifying live GitHub issue state with
> `gh issue list --state all` / `gh issue view`, after confirming the generic
> Psionic replacement track through `PSI-178` and `OA-203` is landed on `main`,
> after confirming the Psionic-only GPT-OSS enablement track `PSI-179` through
> `PSI-183` remains landed on `main`, after confirming the early throughput
> issues `#3242` through `#3246` are closed, after confirming `#3249` and
> `#3247` are also closed, after closing `#3276` once the exact benchmark
> contract crossed `150 tok/s`, after closing `#3293` after the
> delayed-softmax router split moved the live prompt-cache-hit lane into the
> `171-173 tok/s` class, after closing `#3294` once the real GPT-OSS decode
> path moved onto a reusable ids-driven expert-matvec substrate without giving
> back that floor, after keeping `#3248` open as the final NVIDIA
> throughput umbrella, after closing `#3288` as superseded by the exact
> `llama.cpp` parity chain `#3293` -> `#3294` -> `#3295` -> `#3296`, and after
> closing `#3295` once the benchmark target was already met without needing a
> riskier fused gate/up rewrite, after closing `#3296` because sampled decode
> attention was still not the blocking stage on the contract-clean path, and
> after closing `#3248` once the benchmark script itself was made contract-clean
> and Psionic still measured above the local `llama.cpp` control with the same
> visible output.
>
> Benchmark truth correction: the earlier `134.62 tok/s` `prompt_cache_hit`
> reading was a transient fast sample, not a durable code baseline. The exact
> old `41a7b3568` checkpoint now reproduces at about `121.79 tok/s`, and the
> restored `main` floor before the router split on the exact
> `crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh` contract was in the
> `122-123 tok/s` class. After `#3293` landed, two consecutive end-to-end runs
> on this RTX 4080 host measured Psionic `prompt_cache_hit = 173.19 tok/s` and
> `171.29 tok/s`, both returning the exact one-sentence benchmark output. Treat
> the low `171 tok/s` class as the new truthful floor to beat on this host
> until a new kept checkpoint reproduces above it.
>
> Direction correction: the next concrete NVIDIA work is no longer generic
> "keep porting kernels until it gets faster", and it is no longer the router
> delayed-softmax split. That step is now landed and materially moved the
> benchmark, and `#3294` is also landed: the GPT-OSS decode lane now uses a
> reusable ids-driven expert-matvec backend surface and a grouped project
> kernel by default, while the benchmark floor stays in the same `170-173
> tok/s` class and short `ncu` sampling now shows about `60.4 us` in the
> gate/up kernel and about `32.7 us` in the grouped project kernel. The next
> active work should therefore stay on the remaining `llama.cpp` parity gaps
> only when they are still honest blockers. `#3296` is now closed too:
> sampled decode attention stayed small relative to the already-landed router
> and expert work, and `#3248` is now closed as well because the final
> benchmark contract was cleaned up and still showed Psionic ahead. Keep the
> dense grouped
> `routing metadata -> scatter -> grouped expert compute -> gather/accumulate`
> path as the later dense-prefill branch instead of treating it as the first
> decode bottleneck.
>
> This is the live roadmap for `crates/psionic/`. The generic phase-2/3/4 and
> desktop-cutover baseline is now merged. The remaining work below is the gap
> between "we have a generic local Rust runtime and app cutover" and "Psionic can
> truthfully execute the real GPT-OSS/NVIDIA path without external
> `llama.cpp`, while remaining valid as compute-market substrate."
>
> Host execution note: the active accelerator execution queues now split across
> three concrete host classes on this machine: the original NVIDIA GPT-OSS 20B
> parity queue is closed on this host, a new NVIDIA GPT-OSS 120B headroom queue
> is now open at `#3338`, and the Apple Silicon native-Rust Metal completion
> queue remains `#3270` -> `#3268` -> `#3269` -> `#3271` -> `#3272` -> `#3261`
> -> `#3262`. The AMD follow-on issues `PSI-151` through `PSI-154` were
> intentionally closed as not planned and are excluded from the active
> dependency queue unless a future reprioritization reopens them.
>
> GPT-OSS host note: the local
> `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf` file has now
> been verified to run on this machine both via `~/code/llama.cpp` as a
> reference oracle and via Psionic alone through the local
> `psionic-gpt-oss-server` HTTP surface on NVIDIA. The remaining open roadmap work
> on this host is no longer "make GPT-OSS run at all"; the 20B work is
> throughput parity and is already closed, while the active remaining NVIDIA
> work is 120B throughput on the same host. The truthful 20B benchmark floor
> remains in the low `171 tok/s` class for Psionic on the exact same contract,
> and the current 120B hybrid host-backed floor on the exact contract remains
> in the old `2 / 6 / 10 tok/s` shape on clean reruns: the current kept branch
> is reproducing about `2.24-2.26 tok/s` cold, `6.43-6.51 tok/s`
> warm-non-hit, and `10.41-10.57 tok/s` prompt-cache-hit on
> `/home/christopherdavid/models/gpt-oss/gpt-oss-120b-mxfp4.gguf`. The next
> 120B work should stay on `#3345`: the generation-only hidden-state
> residency checkpoint and the stateless host-KV-materialization skip are now
> both landed, but the latter did not produce a durable three-lane throughput
> shift on a fresh clean rerun, so the next honest gap is still the remaining
> host-to-device
> selected4 expert staging inside that hybrid path, not the already-ruled-out
> registered-host-buffer experiments or more cache-slot reshaping.
>
> Inference generalization note: the post-`PSI-183` inference-completion block
> `PSI-232` through `PSI-258` now has one canonical planning source in
> `crates/psionic/docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md`. `PSI-232` is the
> documentation authority issue for that block and closes when that spec is
> explicit enough to drive later implementation and review; `PSI-233` onward
> own the runtime, server, router, and validation work itself.

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

Inference-generalization rule: when working the `PSI-232` through `PSI-258`
block, use `docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md` for the source split,
owner split, dependency order, and definition of done. Do not infer those from
older throughput or host-specific sections of this roadmap.

Reference-first implementation rule: for any issue that touches externally
defined semantics such as GGUF/GGML parsing, quantization or block layouts,
tokenizer reconstruction, prompt rendering, sampler behavior, streaming,
catalog behavior, lifecycle semantics, or backend/runtime truth, the agent must
inspect the equivalent implementation and nearby tests in the most relevant
reference tree before coding. Do not implement those paths from memory or from
roadmap wording alone.

Psionic-only execution rule: the reference repos listed below are for semantic and
behavioral truth only. They are not acceptable execution shortcuts for this
track. Do not shell out to, proxy through, sidecar against, FFI-wrap, or
otherwise delegate prompt rendering, tokenization, Harmony parsing, sampling,
or model execution to `~/code/llama.cpp` or any other external runtime when
closing roadmap issues in Epic G. The shipped path must execute through Psionic
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
  architecture metadata and tensor naming, tokenizer control-token behavior,
  and local NVIDIA execution behavior for the exact GGUFs carried on this host
- start with `~/code/harmony` for the authoritative Harmony format docs,
  published Rust parser/renderer behavior, stop-token semantics, assistant
  channel rules, and GPT-OSS message/tool-call parse expectations
- start with `~/code/ollama` for API-visible behavior such as prompt
  rendering, BOS/EOS defaults, truncation, streaming, catalog/lifecycle, and
  error semantics

Before coding, compare the planned Psionic behavior against the chosen primary
reference and note any intentional deviations. If the reference reveals tricky
ordering, layout, shape, or fallback rules, encode those semantics in tests in
the same issue. If multiple references disagree, follow the source of truth for
the layer being implemented and say which reference won and why in the issue
comment when closing the work.

Performance-parity rule: when the open work is throughput parity against an
existing deployed runtime such as `llama.cpp`, do not inspect only isolated
kernels. Inspect the whole relevant reference path first: model graph builder,
scheduler/runtime, fusion decisions, kernel dispatch policy, and the concrete
CUDA kernels used on the target host/model. If the reference code is already the
proved production path for the exact workload, prefer direct ports or
line-for-line-equivalent implementations of the relevant CUDA pieces over
"clean-room but probably similar" rewrites unless there is a clear ownership or
portability reason not to.

## Objective

Replace the desktop's external Ollama dependency with an in-process Rust
runtime that:

- keeps app UX and provider orchestration in `apps/autopilot-desktop`
- keeps reusable model, runtime, backend, and serving logic in `crates/psionic/*`
- matches the subset of Ollama behavior OpenAgents actually depends on
- remains explicit about backend readiness, fallback, hardware support,
  lifecycle, and evidence

This is not a plan to rebuild all of Ollama.

This is also not enough, by itself, to satisfy the broader compute-market plan.
Psionic must become the reusable execution substrate for truthful `inference` and
`embeddings` supply first, with later bounded `sandbox_execution` only if that
family stays explicit and machine-checkable.

## Ownership Rules

The roadmap must keep `docs/OWNERSHIP.md` intact:

- `crates/psionic/*` owns reusable tensor, IR, compiler, runtime, model, serve, and
  provider-facing engine truth
- `apps/autopilot-desktop` owns the local runtime adapter, provider UX,
  inventory presentation, admission policy, and final cutover from Ollama HTTP
  calls
- `crates/psionic/*` must not absorb app-specific UI or product orchestration

## Tinygrad-Style Rules

Psionic should preserve the parts of Tinygrad that matter architecturally without
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
(`53f31280a`) plus the roadmap follow-ups that closed `PSI-110` through
`PSI-126B` (including the `PSI-115` Candle-alignment follow-up) on `main`.
See "Delivered after the merged baseline" below for the shipped scope and the
per-issue commit anchors.

### Delivered in the merged baseline

- artifact-backed bundle ingestion in `psionic-models`
- explicit quantization metadata and capability truth
- tested model-backed CPU `psionic.embeddings` and `psionic.text_generation` paths
- provider/receipt truth for model-backed CPU products
- Metal discovery, allocation, command submission, truthful backend selection,
  and tested Metal-backed `psionic.embeddings`
- AMD topology/risk/recovery metadata plus separate `amd_kfd` and
  `amd_userspace` discovery/readiness surfaces
- provider-facing AMD context and operator runbook
- Rustygrad subtree rename to Psionic

### Delivered after the merged baseline

- `PSI-115` / [#3164](https://github.com/OpenAgentsInc/openagents/issues/3164):
  initial GGML quantized tensor storage substrate for `Q4_0`, `Q4_1`, and
  `Q8_0`, plus stable storage digests and explicit block-layout metadata
- Candle-aligned `Q4_0` and `Q4_1` dequantization order plus stricter
  last-dimension block validation for GGML-shaped tensors
- explicit runtime truth for dense storage versus dequantized fallback versus
  backend-quantized storage paths
- `PSI-110` / [#3172](https://github.com/OpenAgentsInc/openagents/issues/3172):
  reusable GGUF metadata and tensor parsing in `psionic-models`, `WeightFormat::Gguf`,
  explicit GGUF tensor-type metadata, truthful `F16` / `BF16` dtype support,
  a `GgufWeightBundleLoader`, and GGUF tests for metadata parsing, tensor
  loading, alignment, and unsupported-type refusal
- `PSI-111` / [#3173](https://github.com/OpenAgentsInc/openagents/issues/3173):
  reusable GGUF tokenizer metadata loading in `psionic-models` for SentencePiece
  (`llama`) and GPT-style BPE (`gpt2`) families, plus stable tokenizer digests,
  preserved BOS/EOS/add-bos/add-eos and BPE pretokenizer truth, and explicit
  validation for missing tokenizer metadata and out-of-range special-token IDs
- `PSI-116` / [#3165](https://github.com/OpenAgentsInc/openagents/issues/3165):
  new `psionic-catalog` blob substrate with mmap-or-buffered local reads, stable
  blob digests, and paged byte ranges, plus blob-backed GGUF paging in
  `psionic-models`, storage-truth metadata on GGUF artifacts, and runtime-facing
  paged artifact/tensor planning types in `psionic-runtime`
- `PSI-118` / [#3167](https://github.com/OpenAgentsInc/openagents/issues/3167):
  real golden tokenizer and prompt/template fixture corpus in `psionic-models`,
  reusable GGUF chat-template extraction and digesting, prompt/template
  assertion helpers consumed from `psionic-models` and `psionic-serve`, and fixture
  refresh documentation in `crates/psionic/docs/FIXTURE_CORPUS.md`
- `PSI-179` / [#3239](https://github.com/OpenAgentsInc/openagents/issues/3239):
  GPT-OSS / OpenAI-MoE GGUF loading and truthful mixed `MXFP4` / `Q8_0`
  storage in `psionic-core` and `psionic-models`, including `general.architecture =
  gpt-oss` family mapping, OpenAI-MoE metadata and tensor-layout validation,
  GGUF `MXFP4` tensor-type support with `llama.cpp`-aligned block decode
  semantics, and surfaced `quantization_modes` truth through model/provider
  metadata without pretending unsupported execution support
- `PSI-180` / [#3240](https://github.com/OpenAgentsInc/openagents/issues/3240):
  GPT-OSS / Harmony prompt rendering and channel parsing in `psionic-models` and
  `psionic-serve`, including Psionic-owned Harmony prompt/context/message types,
  real GPT-OSS golden render fixtures pinned against the local GGUF, text/token
  Harmony parse helpers plus a streaming parser wrapper, and optional served
  Harmony structure alongside the raw token/text response lane
- `PSI-117` / [#3166](https://github.com/OpenAgentsInc/openagents/issues/3166):
  reusable Ollama-to-Psionic conformance harness in `psionic-serve`, a live
  `OllamaHttpSubject` over `tags` / `show` / `ps` / `generate` / `embed`,
  explicit `passed` / `failed` / `unsupported` / `intentional_difference`
  outcomes, fixture-driven prompt-render case construction from the golden
  corpus, structured report artifacts, and a documented harness runbook in
  `crates/psionic/docs/CONFORMANCE_HARNESS.md`
- `PSI-119` / [#3168](https://github.com/OpenAgentsInc/openagents/issues/3168):
  shared backend parity policy in `psionic-runtime` with explicit dense-versus-
  quantized drift budgets for embeddings and logits, seeded-versus-unseeded
  generation parity classes, reusable vector/logit comparison helpers, and
  policy-backed Metal embeddings parity plus conformance embed comparisons
- `PSI-160` / [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220):
  reusable local-serving isolation policy in `psionic-runtime`, explicit
  in-process crash/reset truth in `psionic-serve` observability and generation
  provenance, an aggregate `PsionicLocalRuntime::isolation_policy()` surface, and
  cutover-contract documentation for the current in-process-versus-subprocess
  decision
- `PSI-161` / [#3171](https://github.com/OpenAgentsInc/openagents/issues/3171):
  reusable served-product fallback lattice in `psionic-runtime`, with explicit
  trigger/action vocabulary, surfaced `same_backend_slow_path` / `retried` /
  `refused` selection states, provider/serve truth for realized fallback
  state, validation mapping that distinguishes explicit refusal, and a
  documented fallback boundary in the conformance/evidence contract
- `PSI-162` / [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233):
  first-class served-artifact identity and reproducibility tuples in
  `psionic-runtime`, descriptor-side artifact identity metadata in `psionic-models`,
  and explicit capability/receipt/provenance/session/prefix-cache truth in
  `psionic-provider` and `psionic-serve`, with request digests and cache ownership now
  refusing silent tokenizer/template/default drift
- `PSI-163` / [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234):
  explicit runtime cache invalidation policy in `psionic-runtime` for execution-
  plan, kernel-cache, paged-tensor, prefix-cache, and KV-state families, plus
  provider/serve evidence surfacing of both the policy and realized cache
  observations so rebuild, bypass, invalidate, and restore behavior remain
  machine-checkable
- `PSI-164` / [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235):
  explicit local-artifact provenance and declared-license facts in
  `psionic-catalog` and `psionic-models`, plus provider-side compute-market supply
  policy and advertise/serve decisions so policy refusals stay distinct from
  integrity and unsupported-format failures
- `PSI-170` / [#3222](https://github.com/OpenAgentsInc/openagents/issues/3222):
  explicit migration-boundary truth in `psionic-models`, `psionic-catalog`, and
  `psionic-serve`, with catalog/ingress/serving/runtime boundary facts that keep
  Ollama compatibility visible as migration substrate instead of silently
  turning it into the Psionic-native architectural source of truth
- `PSI-171` / [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223):
  compute-market capability qualifiers in `psionic-runtime` and `psionic-provider`,
  with reusable selected-device inventory classification, explicit
  `compiled_only` versus `compiled_and_probed` backend-toolchain truth, and
  provider capability/receipt surfaces that now expose stable device,
  topology-key, memory-class, and performance-class qualifiers
- `PSI-172` / [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224):
  explicit execution-profile truth in `psionic-runtime`, `psionic-serve`, and
  `psionic-provider`, with machine-checkable batch posture, queue policy, and
  throughput class reporting for both embeddings and text generation plus
  observability coverage that now distinguishes "no internal queue exists"
  from "the current queue just happens to be empty"
- `PSI-173` / [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225):
  explicit multi-device selection and sharding/topology truth in
  `psionic-runtime`, `psionic-compiler`, and `psionic-provider`, with stable topology
  digests, compiler-facing topology-aware plan wrappers, provider capability
  and receipt surfacing of `selected_devices` plus `execution_topology`, and
  contract coverage for the current single-device-versus-sharded boundary
- `PSI-174` / [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226):
  explicit execution-plan cache policy/state plus warm/cold compile-path
  evidence in `psionic-runtime`, backend-owned plan caching in CPU/Metal/CUDA,
  embeddings and text-generation provenance in `psionic-serve`, and provider
  receipt surfacing of realized plan-cache and kernel-cache behavior
- `PSI-175` / [#3227](https://github.com/OpenAgentsInc/openagents/issues/3227):
  compute-market delivery-proof and settlement-linkage evidence in
  `psionic-runtime`, `psionic-serve`, and `psionic-provider`, with kernel-count,
  bytes-moved, plan-cache hit/miss, and KV-growth reporting carried directly
  through runtime metrics, serve-side provenance, and provider receipts
- `OA-200` / [#3216](https://github.com/OpenAgentsInc/openagents/issues/3216):
  app-owned local-execution evidence naming cleanup in
  `apps/autopilot-desktop`, with `LocalInferenceExecutionMetrics` and
  `LocalInferenceExecutionProvenance` now replacing Ollama-specific type names
  across the Ollama worker, Apple bridge, runtime state, kernel-control
  delivery logic, and receipt evidence
- `PSI-112` / [#3177](https://github.com/OpenAgentsInc/openagents/issues/3177):
  reusable GGUF decoder-family adapters in `psionic-models` for the first launch
  families, with Candle-aligned family/config extraction, Ollama-compatible
  metadata and tensor-name mapping, explicit Mistral-vs-Llama family truth,
  reusable tensor layouts, attached tokenizer/chat-template metadata, and
  explicit refusal of unsupported llama MoE artifacts
- `PSI-113` / [#3178](https://github.com/OpenAgentsInc/openagents/issues/3178):
  reusable GGUF embedding-family adapters in `psionic-models` for the first launch
  encoder families, with Ollama-aligned BERT and Nomic-BERT metadata
  extraction, pooling and normalization truth, reusable tensor layouts,
  attached tokenizer metadata, completed BERT wordpiece/token-type-count GGUF
  tokenizer support, and explicit refusal of unsupported Nomic MoE artifacts
- `PSI-114` / [#3179](https://github.com/OpenAgentsInc/openagents/issues/3179):
  reusable GGUF prompt rendering in `psionic-models` for the supported golden
  template families, with explicit prompt-message/rendered-prompt/error types,
  digest-gated Phi-3 / Qwen2 / Command-R compatibility, reusable
  `GgufDecoderAdapter::render_prompt(...)`, `psionic-serve` re-exports, and
  conformance-harness coverage that now treats prompt rendering as parity work
  instead of an intentional gap
- `PSI-120` / [#3180](https://github.com/OpenAgentsInc/openagents/issues/3180):
  reusable local Ollama catalog discovery in `psionic-catalog`, with
  Ollama-compatible default model-name normalization, parsed manifest/layer
  records, explicit layer-kind and blob-presence truth, non-mutating manifest
  scans with warnings for invalid entries, and direct model-resolution APIs on
  top of the existing shared blob substrate
- `PSI-121` / [#3181](https://github.com/OpenAgentsInc/openagents/issues/3181):
  reusable installed-model `tags` / `show` parity surfaces over the shared
  local Ollama catalog, with manifest-layer config/text/json decode helpers in
  `psionic-catalog`, a local `LocalOllamaCatalogSubject` in `psionic-serve`, explicit
  local GGUF model-info and capability derivation without the Ollama daemon,
  Ollama-aligned skipping of bad config blobs during listing, and fixture-backed
  list/show tests for local parity and missing-model errors
- `PSI-122` / [#3182](https://github.com/OpenAgentsInc/openagents/issues/3182):
  reusable loaded-model lifecycle truth in `psionic-runtime` and `psionic-serve`, with
  explicit loading/ready state, active-request counts, keepalive windows,
  `ps`-style ordering, warm/load/unload operations, and Ollama-aligned idle
  expiry semantics including zero-keepalive unload after requests go idle
- `PSI-123` / [#3183](https://github.com/OpenAgentsInc/openagents/issues/3183):
  expanded generation options in `psionic-serve` for `temperature`, `top_k`,
  `top_p`, repeat/presence/frequency penalties, `seed`, and explicit
  `stop_sequences`, plus seeded sampling, penalty-adjusted logits, and
  stop-sequence truncation on the CPU reference path
- `PSI-124` / [#3184](https://github.com/OpenAgentsInc/openagents/issues/3184):
  explicit generation metrics and provenance in `psionic-serve` and `psionic-provider`,
  with prompt/output token counts, total/load durations, warm-versus-cold load
  state, execution-plan digests, receipt alignment to response provenance, and
  regression coverage for cold-then-warm residency and option-bearing request
  digests
- `PSI-125` / [#3185](https://github.com/OpenAgentsInc/openagents/issues/3185):
  a library-first in-process runtime surface in `psionic-serve`, with
  `LocalModelCatalog`, `ManagedTextGenerationRuntime`, and `PsionicLocalRuntime`
  covering `list_models`, `show_model`, `loaded_models`, `warm_model`,
  `unload_model`, `generate`, and `embed`, plus regression coverage that
  exercises the aggregate boundary end to end
- `PSI-126` / [#3186](https://github.com/OpenAgentsInc/openagents/issues/3186):
  deterministic text-generation session ownership in `psionic-serve`, with
  descriptor-bound KV ownership (`model_id`, family, revision, bundle digest),
  token-sequence ownership alongside the KV cache, explicit cache-plus-token
  commit on successful generation, and regression coverage for isolation/reset
  plus descriptor-drift refusal
- `PSI-126A` / [#3169](https://github.com/OpenAgentsInc/openagents/issues/3169):
  runtime-owned paged-KV policy and accounting in `psionic-runtime`, logically
  paged per-session KV state in `psionic-serve` with explicit `refuse_new_pages`
  behavior instead of silent spill/evict, session metadata bound to KV
  policy/state, and generation/provider evidence carrying KV pages, bytes, and
  growth
- `PSI-126B` / [#3231](https://github.com/OpenAgentsInc/openagents/issues/3231):
  shared prompt-prefix cache truth in `psionic-runtime`, `psionic-serve`, and
  `psionic-provider`, with explicit reusable-prefix identity inputs, shared-prefix
  reuse policy, `none` / `hit` / `miss` / `bypassed` / `rebuilt` taxonomy,
  longest-safe prefix reuse on the CPU reference path, stale-entry rebuild
  handling, and provider/receipt evidence carrying prefix-cache state,
  identity, policy, and reused-token counts
- `PSI-165` / [#3236](https://github.com/OpenAgentsInc/openagents/issues/3236):
  OCI/Docker-v2 registry pull and ingestion in `psionic-catalog`, with
  Ollama-compatible manifest/blob URLs, manifest validation shared with local
  scans, digest/size-checked blob writes into the existing local store,
  explicit pull reports that distinguish reused-versus-downloaded blobs, and
  a `psionic-models` GGUF loader path that can consume a resolved local Ollama
  manifest directly after pull
- `PSI-130` / [#3200](https://github.com/OpenAgentsInc/openagents/issues/3200):
  truthful Metal dense-surface coverage for the current `psionic.text_generation`
  graph shape, with a distinct text-generation op contract, shared dense
  pipeline/kernel-cache accounting, explicit Metal-versus-CPU selection and
  fallback coverage for that product surface, and a direct Metal execution test
  over the current text-generation matmul/add graph
- `PSI-131` / [#3201](https://github.com/OpenAgentsInc/openagents/issues/3201):
  reusable CPU-versus-Metal parity coverage for the current
  `psionic.text_generation` graph shape, with seeded exact token/text/termination
  parity, policy-backed hidden/logit drift checks, a dedicated
  `metal_text_generation_parity` integration test in `psionic-serve`, and
  macOS-target import fixes in `psionic-backend-metal` exposed by cross-target
  compilation
- `PSI-132` / [#3202](https://github.com/OpenAgentsInc/openagents/issues/3202):
  a tested Metal-backed `psionic.text_generation` product path in `psionic-serve`,
  with a real `MetalModelTextGenerationService`, shared non-streaming
  generation/session/prefix-cache/provenance flow across CPU and Metal,
  explicit Metal unavailability diagnostics instead of silent CPU fallback,
  and provider-facing capability/receipt coverage for success versus explicit
  refusal
- `PSI-140` / [#3203](https://github.com/OpenAgentsInc/openagents/issues/3203):
  explicit CUDA backend architecture truth in `psionic-backend-cuda`,
  `psionic-runtime`, and `psionic-provider`, with a new `psionic-backend-cuda` crate,
  first-class `DeviceKind::Cuda`, direct runtime/provider selection identity
  for `cuda`, and an explicit architecture-only offline state so Psionic does not
  pretend NVIDIA discovery, topology, or execution are already landed
- `PSI-141` / [#3204](https://github.com/OpenAgentsInc/openagents/issues/3204):
  explicit reusable NVIDIA topology, risk, and recovery truth in
  `psionic-runtime` and `psionic-provider`, with first-class `nvidia_metadata` on
  runtime device descriptors, provider-visible `nvidia` capability/receipt
  context, and regression coverage so later CUDA discovery and selection work
  can build on machine-checkable NVIDIA contract surfaces
- `PSI-142` / [#3205](https://github.com/OpenAgentsInc/openagents/issues/3205):
  real `nvidia-smi`-backed CUDA discovery and health reporting in
  `psionic-backend-cuda`, with runtime device descriptors populated from live
  NVIDIA query data, explicit ready versus degraded versus offline health,
  display-attached and MIG caveats preserved as degraded-state truth, and
  stable CUDA feature flags for persistence and addressing-mode posture
- `PSI-143` / [#3206](https://github.com/OpenAgentsInc/openagents/issues/3206):
  operational CUDA allocation and submission substrate in `psionic-backend-cuda`,
  with dynamic `libcudart` loading, explicit device buffers plus host staging
  reads and writes, stream-based fill/copy submission with machine-checkable
  completion status, allocator/runtime-resource truth, and end-to-end buffer
  copy coverage on the selected NVIDIA device when the CUDA runtime is present
- `PSI-144` / [#3207](https://github.com/OpenAgentsInc/openagents/issues/3207):
  operational CUDA dense execution in `psionic-backend-cuda`, with explicit
  `input` / `constant` / `matmul` / `add` plan validation, dense CUDA
  input/constant materialization, Candle-aligned cuBLAS row-major matmul
  lowering, cuBLAS-backed add coverage, and live end-to-end execution tests
  for the first NVIDIA primitive surface
- `PSI-145` / [#3208](https://github.com/OpenAgentsInc/openagents/issues/3208):
  truthful NVIDIA served-product selection and provider capability reporting,
  with explicit direct versus same-backend-degraded versus CPU-fallback CUDA
  selection surfaces in `psionic-backend-cuda`, and provider capability/receipt
  coverage that now reports real post-`PSI-144` NVIDIA execution posture
- `PSI-146` / [#3209](https://github.com/OpenAgentsInc/openagents/issues/3209):
  explicit CPU-versus-CUDA parity evidence for the first supported NVIDIA
  served path, with a model-backed embeddings parity test in `psionic-serve`
  that checks CUDA outputs against the CPU baseline under the shared
  embeddings drift budget and reports explicit CPU fallback instead of
  overclaiming CUDA parity when the backend is unavailable
- `PSI-147` / [#3210](https://github.com/OpenAgentsInc/openagents/issues/3210):
  the first tested NVIDIA-backed served product path in `psionic-serve`, with a
  real `CudaModelEmbeddingsService`, CUDA-specific embeddings diagnostics,
  model-backed response/capability/receipt integration coverage, and explicit
  backend-unavailability handling instead of aspirational NVIDIA advertising
- `PSI-148` / [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232):
  a minimum shipped hardware validation profile in `psionic-runtime`,
  `psionic-provider`, and `crates/psionic/docs/`, with explicit validation claim IDs
  for CPU reference lanes, Apple Silicon Metal lanes, NVIDIA CUDA embeddings,
  AMD KFD discovery, and refusal paths, provider-facing capability/receipt
  references back to that matrix, and a documented host-class lab runbook
- `PSI-150` / [#3211](https://github.com/OpenAgentsInc/openagents/issues/3211):
  AMD execution substrate groundwork in `psionic-backend-amd-kfd`,
  `psionic-backend-amd-userspace`, `psionic-provider`, and `crates/psionic/docs/`, with
  backend-owned staging buffers, explicit fill/copy submissions, explicit
  allocator/kernel-cache/device-budget truth for AMD substrate paths, and
  provider-visible runtime-resource coverage without advertising a shipped
  AMD served product yet
- `PSI-137` / [#3194](https://github.com/OpenAgentsInc/openagents/issues/3194):
  explicit served-product backend fallback/degraded policy in `psionic-runtime`,
  `psionic-provider`, and CPU/Metal backend selection, with machine-checkable
  unavailable/degraded policy enums, direct / same-backend-degraded /
  cross-backend-fallback state, and regression coverage for capability JSON
  and fallback/degraded truth
- `PSI-138` / [#3195](https://github.com/OpenAgentsInc/openagents/issues/3195):
  explicit cutover performance gates in `psionic-serve` and `psionic-provider`, with
  Ollama-aligned timing metrics retained on conformance observations and
  receipts, ratio-based default thresholds for generation and embeddings, and
  `ConformanceReport::cutover_ready_with_performance(...)` for machine-
  checkable semantic-plus-performance cutover decisions
- `PSI-139` / [#3196](https://github.com/OpenAgentsInc/openagents/issues/3196):
  explicit Ollama adapter-policy truth in `psionic-catalog`, explicit refusal of
  adapter-bearing Ollama manifests in `psionic-models` instead of silently loading
  the base GGUF alone, and `psionic-serve` show-surface facts plus conformance
  handling so extra Psionic evidence does not count as an Ollama semantic mismatch
- `PSI-157` / [#3197](https://github.com/OpenAgentsInc/openagents/issues/3197):
  explicit backend runtime-resource truth in `psionic-runtime`, exact-spec
  allocator pooling for CPU and Metal intermediate buffers, bounded kernel-
  cache reporting, device-memory-budget reporting, and provider-visible
  serialization/tests so warm/cold and memory-admission behavior stay machine-
  checkable instead of backend-internal
- `PSI-158` / [#3198](https://github.com/OpenAgentsInc/openagents/issues/3198):
  typed backend-extension ops in the graph/plan layer for `rms_norm`,
  `layer_norm`, `rotary_embedding`, `scaled_dot_product_attention`, and
  `quantized_matmul`, explicit backend-extension capability truth on
  `BackendSelection`, CPU reference execution for those extension families,
  and a path for later Metal/CUDA/AMD specialized kernels without polluting
  the base primitive-op surface
- `PSI-159` / [#3199](https://github.com/OpenAgentsInc/openagents/issues/3199):
  explicit local-runtime observability in `psionic-runtime`, `psionic-serve`, and
  `psionic-provider`, with bounded recent transition logs for cold-load, first-
  request warm, unload, and backend-health changes, active-session and
  active-request counts, queue-depth truth, memory-footprint snapshots, and a
  managed-runtime `observability()` surface plus provider-facing envelope
  serialization/tests
- `PSI-156` / [#3170](https://github.com/OpenAgentsInc/openagents/issues/3170):
  first-class quantized GGML/GGUF constant payloads in `psionic-core` /
  `psionic-ir`, Candle-aligned row-wise quantized-matmul RHS orientation,
  CPU-native `Q4_0` / `Q4_1` / `Q8_0` kernels over preserved block bytes,
  explicit `backend_quantized` + `native` CPU capability truth, provider-
  visible quantized capability reporting, and explicit Metal refusal for
  quantized constants instead of silent fallback
- `PSI-129` / [#3189](https://github.com/OpenAgentsInc/openagents/issues/3189):
  explicit local-serving memory planning, residency policy, and admission
  control in `psionic-runtime` / `psionic-serve` / `psionic-provider`, with bounded-budget
  refusal and optional idle-oldest eviction, default decoder memory plans,
  and provider/runtime evidence carrying memory plans, residency policy, and
  current residency snapshots
- `PSI-133` / [#3190](https://github.com/OpenAgentsInc/openagents/issues/3190):
  pull-driven local streaming generation in `psionic-serve` / `psionic-provider`, with
  explicit backpressure, disconnect, and cancellation policy, typed chunk and
  terminal events, partial-output semantics for cancellation/disconnect/runtime
  failure after stream start, and receipt/capability truth carrying streaming
  policy
- `PSI-134` / [#3191](https://github.com/OpenAgentsInc/openagents/issues/3191):
  explicit embeddings API semantics in `psionic-serve` / `psionic-provider`, with
  empty-batch success, requested output-dimension handling, model-family /
  revision / normalization metadata on responses and receipts, ordered-batch
  capability truth, and explicit `supports_input_truncation = false` on the
  current byte-projection embeddings paths
- `PSI-135` / [#3192](https://github.com/OpenAgentsInc/openagents/issues/3192):
  local model-store integrity verification in `psionic-catalog` / `psionic-models`,
  with structured repair diagnostics for missing manifests/blobs and
  corrupt-or-size-mismatched blobs, explicit manifest-level verification over
  the shared blob substrate, and GGUF manifest loads that now refuse corrupt
  primary model blobs before parse
- `PSI-136` / [#3193](https://github.com/OpenAgentsInc/openagents/issues/3193):
  backend-neutral local runtime diagnostics taxonomy in `psionic-runtime` /
  `psionic-serve` / `psionic-provider`, with stable error codes plus HTTP-style
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
| [#3164](https://github.com/OpenAgentsInc/openagents/issues/3164) | Closed | `PSI-115` landed: GGML quantized tensor storage substrate, Candle-aligned `Q4_0` / `Q4_1` decode order, and stricter GGML block-shape validation. |
| [#3165](https://github.com/OpenAgentsInc/openagents/issues/3165) | Closed | `PSI-116` landed: `psionic-catalog` blob access substrate, mmap-or-buffered GGUF and Ollama blob reads, paged tensor slices for GGUF tensors, storage-truth metadata on artifacts, and runtime-facing paged storage planning types. |
| [#3166](https://github.com/OpenAgentsInc/openagents/issues/3166) | Closed | `PSI-117` landed: reusable Ollama-to-Psionic conformance harness in `psionic-serve`, live Ollama HTTP normalization for `tags` / `show` / `ps` / `generate` / `embed`, explicit pass/fail/unsupported/intentional-difference outcomes, fixture-driven prompt-render cases, and a documented report/runbook. |
| [#3167](https://github.com/OpenAgentsInc/openagents/issues/3167) | Closed | `PSI-118` landed: real tokenizer and prompt/template fixture corpus, GGUF chat-template extraction plus digests, reusable assertion helpers, and documented fixture refresh flow. |
| [#3168](https://github.com/OpenAgentsInc/openagents/issues/3168) | Closed | `PSI-119` landed: shared backend parity policy in `psionic-runtime`, explicit dense-vs-quantized drift budgets for embeddings/logits, seeded-vs-unseeded generation parity classes, reusable comparison helpers, and policy-backed parity/conformance tests plus documentation. |
| [#3170](https://github.com/OpenAgentsInc/openagents/issues/3170) | Closed | `PSI-156` landed: first-class quantized GGML/GGUF constant payloads, Candle-aligned row-wise quantized matmul RHS semantics, native CPU `Q4_0` / `Q4_1` / `Q8_0` kernels over preserved block bytes, explicit CPU quantized capability truth, and explicit Metal refusal for quantized constants. |
| [#3200](https://github.com/OpenAgentsInc/openagents/issues/3200) | Closed | `PSI-130` landed: a distinct Metal text-generation dense-surface contract in `psionic-backend-metal`, shared dense pipeline/kernel-cache accounting, explicit Metal-vs-CPU selection/fallback coverage for that product surface, and direct Metal execution coverage for the current text-generation matmul/add graph. |
| [#3201](https://github.com/OpenAgentsInc/openagents/issues/3201) | Closed | `PSI-131` landed: a dedicated `metal_text_generation_parity` integration test in `psionic-serve`, seeded exact CPU-vs-Metal token/text/termination parity over the current graph shape, policy-backed hidden/logit drift checks, and macOS-target Metal import fixes exposed by cross-target compilation. |
| [#3202](https://github.com/OpenAgentsInc/openagents/issues/3202) | Closed | `PSI-132` landed: a real `MetalModelTextGenerationService` in `psionic-serve`, shared CPU/Metal non-streaming generation/session/prefix-cache/provenance flow, explicit Metal unavailability diagnostics instead of silent CPU fallback, and provider-facing capability/receipt tests for success versus explicit refusal. |
| [#3203](https://github.com/OpenAgentsInc/openagents/issues/3203) | Closed | `PSI-140` landed: a new `psionic-backend-cuda` architecture crate, first-class `DeviceKind::Cuda`, runtime/provider backend-selection truth for `cuda`, and an explicit architecture-only offline state so NVIDIA is visible without overclaiming discovery or execution readiness. |
| [#3204](https://github.com/OpenAgentsInc/openagents/issues/3204) | Closed | `PSI-141` landed: reusable NVIDIA topology/risk/recovery metadata in `psionic-runtime`, `nvidia_metadata` on runtime device descriptors, and provider-visible `nvidia` capability/receipt context so later CUDA discovery and routing work has an explicit truth contract. |
| [#3205](https://github.com/OpenAgentsInc/openagents/issues/3205) | Closed | `PSI-142` landed: real `nvidia-smi`-backed CUDA discovery in `psionic-backend-cuda`, explicit ready/degraded/offline health, runtime device descriptors populated from live NVIDIA query data, and degraded-state truth for display-attached or MIG-partitioned devices. |
| [#3206](https://github.com/OpenAgentsInc/openagents/issues/3206) | Closed | `PSI-143` landed: operational `libcudart`-backed CUDA buffers and stream submission in `psionic-backend-cuda`, explicit allocator/runtime-resource truth, and end-to-end staged write plus device-to-device copy coverage on the selected NVIDIA device. |
| [#3207](https://github.com/OpenAgentsInc/openagents/issues/3207) | Closed | `PSI-144` landed: the first CUDA dense execution surface in `psionic-backend-cuda`, with explicit `input` / `constant` / `matmul` / `add` plan validation, dense CUDA materialization helpers, Candle-aligned cuBLAS matmul lowering, cuBLAS-backed add coverage, and live backend execution tests. |
| [#3208](https://github.com/OpenAgentsInc/openagents/issues/3208) | Closed | `PSI-145` landed: explicit direct/degraded/fallback CUDA backend-selection surfaces in `psionic-backend-cuda`, plus provider capability and receipt coverage that now reflects real NVIDIA execution posture instead of the old architecture-only placeholder. |
| [#3209](https://github.com/OpenAgentsInc/openagents/issues/3209) | Closed | `PSI-146` landed: explicit CPU-vs-CUDA embeddings parity coverage in `psionic-serve`, with drift-budget comparison against the CPU baseline and explicit CPU fallback truth when CUDA is unavailable. |
| [#3210](https://github.com/OpenAgentsInc/openagents/issues/3210) | Closed | `PSI-147` landed: the first tested NVIDIA-backed served product path as model-backed embeddings in `psionic-serve`, with a real CUDA service, CUDA-specific diagnostics, and capability/receipt integration coverage. |
| [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232) | Closed | `PSI-148` landed: a minimum shipped hardware validation matrix in `psionic-runtime`, `psionic-provider`, and `crates/psionic/docs`, with explicit claim IDs for CPU, Apple Silicon Metal, NVIDIA CUDA embeddings, AMD KFD discovery, and refusal paths, plus provider-facing validation references and a documented lab runbook. |
| [#3211](https://github.com/OpenAgentsInc/openagents/issues/3211) | Closed | `PSI-150` landed: AMD KFD and AMD userspace execution substrate groundwork with backend-owned staging buffers, explicit fill/copy submissions, explicit runtime-resource truth, explicit CPU fallback helpers, and provider/runtime-resource coverage while AMD served products remain unshipped. |
| [#3212](https://github.com/OpenAgentsInc/openagents/issues/3212), [#3213](https://github.com/OpenAgentsInc/openagents/issues/3213), [#3214](https://github.com/OpenAgentsInc/openagents/issues/3214), [#3215](https://github.com/OpenAgentsInc/openagents/issues/3215) | Closed (Not Planned) | `PSI-151` through `PSI-154` were closed after host reprioritization to NVIDIA-only execution and validation. Keep the landed AMD substrate from `PSI-150`, but do not treat AMD KFD lowering, served-product gating, parity, or shipped AMD execution as active roadmap dependencies on this machine. |
| [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220) | Closed | `PSI-160` landed: reusable local-serving isolation policy in `psionic-runtime`, explicit `in_process` crash/reset truth in `psionic-serve` observability and generation provenance, an aggregate `PsionicLocalRuntime::isolation_policy()` surface, and cutover-contract documentation for the current no-subprocess decision. |
| [#3222](https://github.com/OpenAgentsInc/openagents/issues/3222) | Closed | `PSI-170` landed: explicit Ollama-compat versus Psionic-native boundary metadata in `psionic-models`, `psionic-catalog`, and `psionic-serve`, with `show`-surface facts that keep compatibility support honest as migration substrate. |
| [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223) | Closed | `PSI-171` landed: reusable device inventory qualifiers in `psionic-runtime`, explicit compile-vs-probe backend toolchain truth in `psionic-provider`, and provider capability/receipt surfaces that now expose selected-device inventory and backend-toolchain facts for compute-market filtering. |
| [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224) | Closed | `PSI-172` landed: runtime-owned execution profiles in `psionic-runtime`, `psionic-serve` defaults plus observability alignment, and provider capability reporting of batch posture, queue policy, and throughput class for embeddings and text generation. |
| [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225) | Closed | `PSI-173` landed: explicit multi-device selection truth and `ExecutionTopologyPlan` substrate in `psionic-runtime`, topology-aware compiled-plan digests in `psionic-compiler`, and provider capability/receipt surfacing of `selected_devices` plus `execution_topology` with multi-device regression coverage. |
| [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226) | Closed | `PSI-174` landed: explicit execution-plan cache policy/state plus compile-path evidence in `psionic-runtime`, backend-owned plan caching in CPU/Metal/CUDA, `psionic-serve` provenance for embeddings and generation compile paths, and provider receipt surfacing of realized plan-cache/kernel-cache behavior. |
| [#3177](https://github.com/OpenAgentsInc/openagents/issues/3177) | Closed | `PSI-112` landed: reusable GGUF decoder-family adapters, explicit Llama/Qwen/Mistral family metadata and tensor layouts, attached tokenizer/chat-template metadata, and explicit refusal of unsupported llama MoE artifacts. |
| [#3178](https://github.com/OpenAgentsInc/openagents/issues/3178) | Closed | `PSI-113` landed: reusable GGUF embedding-family adapters for BERT and Nomic-BERT, explicit pooling/normalization truth and tensor layouts, finished BERT wordpiece/token-type-count tokenizer support, and explicit refusal of unsupported Nomic MoE artifacts. |
| [#3179](https://github.com/OpenAgentsInc/openagents/issues/3179) | Closed | `PSI-114` landed: reusable GGUF prompt rendering for the supported Phi-3, Qwen2, and Command-R template digests, explicit prompt/render/error types, `GgufDecoderAdapter` render helpers, `psionic-serve` re-exports, and conformance coverage that removed the old prompt-render gap. |
| [#3180](https://github.com/OpenAgentsInc/openagents/issues/3180) | Closed | `PSI-120` landed: local Ollama manifest/blob discovery and model resolution in `psionic-catalog`, including default name normalization, parsed manifest/media-type/layer records, blob-presence truth, non-mutating scan warnings, and direct resolved-manifest APIs. |
| [#3181](https://github.com/OpenAgentsInc/openagents/issues/3181) | Closed | `PSI-121` landed: reusable manifest-layer config/text/json decode helpers in `psionic-catalog`, plus a local `tags` / `show` subject in `psionic-serve` that reads the shared Ollama catalog directly, derives GGUF model-info facts and capabilities without the Ollama daemon, skips bad config blobs during listing like Ollama does, and includes fixture-backed local list/show tests. |
| [#3182](https://github.com/OpenAgentsInc/openagents/issues/3182) | Closed | `PSI-122` landed: explicit loaded-model residency truth in `psionic-runtime`, an in-memory warm/load/unload registry in `psionic-serve` with `ps`-style ordering and zero-keepalive unload behavior, request lifecycle hooks that clear/reset expiry like Ollama scheduler warmups, and regression tests for keepalive ordering and idle expiry. |
| [#3183](https://github.com/OpenAgentsInc/openagents/issues/3183) | Closed | `PSI-123` landed: explicit generation-option fields in `psionic-serve` for temperature, top-k, top-p, repeat/presence/frequency penalties, seed, and stop sequences, plus seeded stochastic sampling, penalty-adjusted logits, and stop-sequence truncation on the CPU reference path. |
| [#3184](https://github.com/OpenAgentsInc/openagents/issues/3184) | Closed | `PSI-124` landed: explicit generation metrics and provenance in `psionic-serve` and `psionic-provider`, including prompt/output token counts, total/load durations, warm/cold load state, execution-plan digests, provenance-aligned receipts, and regression coverage for cold-then-warm residency. |
| [#3185](https://github.com/OpenAgentsInc/openagents/issues/3185) | Closed | `PSI-125` landed: a library-first in-process runtime API in `psionic-serve` with reusable catalog and managed-generation traits plus an aggregate `PsionicLocalRuntime` wrapper over list/show/ps/warm/unload/generate/embed. |
| [#3186](https://github.com/OpenAgentsInc/openagents/issues/3186) | Closed | `PSI-126` landed: deterministic text-generation session ownership in `psionic-serve`, with descriptor-bound KV ownership, token-sequence ownership alongside KV state, explicit cache-plus-token commit on successful generation, and descriptor-drift refusal coverage. |
| [#3187](https://github.com/OpenAgentsInc/openagents/issues/3187) | Closed | `PSI-127` landed: reusable context-window budgeting in `psionic-models`, explicit `refuse` vs `truncate_oldest` prompt-overflow policy in `psionic-serve`, Ollama-aligned over-limit error strings, and regression coverage for truncation and session-owned context pressure. |
| [#3188](https://github.com/OpenAgentsInc/openagents/issues/3188) | Closed | `PSI-128` landed: runtime-owned sampler policy and seeded replay behavior in `psionic-runtime`, Ollama-aligned defaults/transform order plus bounded penalty lookback, `psionic-serve` delegation to that runtime sampler, and generation-level replay coverage for the supported option surface. |
| [#3189](https://github.com/OpenAgentsInc/openagents/issues/3189) | Closed | `PSI-129` landed: reusable memory-plan and residency-policy substrate in `psionic-runtime`, admission-aware loaded-model registry behavior in `psionic-serve`, bounded-budget refusal plus optional idle-oldest eviction, default decoder memory planning, and capability/receipt evidence carrying memory-plan and residency-snapshot truth. |
| [#3190](https://github.com/OpenAgentsInc/openagents/issues/3190) | Closed | `PSI-133` landed: pull-driven local streaming generation in `psionic-serve` with explicit backpressure/disconnect/cancellation policy, typed chunk vs terminal events, partial-output terminal semantics for cancel/disconnect/runtime failure after stream start, runtime forwarding through `PsionicLocalRuntime`, and provider receipt/capability streaming truth. |
| [#3191](https://github.com/OpenAgentsInc/openagents/issues/3191) | Closed | `PSI-134` landed: explicit embeddings API semantics in `psionic-serve` and `psionic-provider`, including empty-batch success, requested output dimensions with re-normalization, model-family/revision/normalization metadata reporting, ordered-batch capability truth, and explicit no-input-truncation support for current byte-projection embeddings paths. |
| [#3192](https://github.com/OpenAgentsInc/openagents/issues/3192) | Closed | `PSI-135` landed: explicit local model-store integrity verification and cache-repair diagnostics in `psionic-catalog`, covering missing manifests, missing blobs, digest mismatch, and declared-size mismatch, plus `psionic-models` refusal of corrupt primary GGUF blobs from manifest-backed loads. |
| [#3193](https://github.com/OpenAgentsInc/openagents/issues/3193) | Closed | `PSI-136` landed: backend-neutral local runtime diagnostics taxonomy in `psionic-runtime`, `psionic-serve`, and `psionic-provider`, including stable error codes plus HTTP-style status/message/context fields, serve-layer mappings for current request failures, streaming-terminal diagnostics, and provider receipts that preserve structured diagnostics alongside plain-text reasons. |
| [#3194](https://github.com/OpenAgentsInc/openagents/issues/3194) | Closed | `PSI-137` landed: explicit served-product backend fallback/degraded policy in `psionic-runtime`, `psionic-provider`, and CPU/Metal backend selection, with machine-checkable unavailable/degraded policy enums, direct / same-backend-degraded / cross-backend-fallback state, and regression coverage for capability JSON and fallback/degraded truth. |
| [#3195](https://github.com/OpenAgentsInc/openagents/issues/3195) | Closed | `PSI-138` landed: explicit cutover performance gates in `psionic-serve` and `psionic-provider`, with Ollama-aligned timing metrics retained on conformance observations and receipts, ratio-based generation and embeddings thresholds, and `ConformanceReport::cutover_ready_with_performance(...)` for machine-checkable semantic-plus-performance cutover decisions. |
| [#3196](https://github.com/OpenAgentsInc/openagents/issues/3196) | Closed | `PSI-139` landed: explicit Ollama adapter-policy status in `psionic-catalog`, manifest-backed loader refusal in `psionic-models` for adapter-bearing manifests, and `psionic-serve` show-surface facts plus conformance handling so extra Psionic evidence does not count as an Ollama semantic mismatch. |
| [#3197](https://github.com/OpenAgentsInc/openagents/issues/3197) | Closed | `PSI-157` landed: explicit backend runtime-resource truth in `psionic-runtime`, exact-spec allocator pooling for CPU and Metal intermediate buffers, bounded kernel-cache reporting, device-memory-budget reporting, and provider serialization/tests so warm/cold and memory-admission behavior are machine-checkable instead of backend-internal. |
| [#3198](https://github.com/OpenAgentsInc/openagents/issues/3198) | Closed | `PSI-158` landed: typed backend-extension ops for normalization, RoPE, attention, and quantized matmul in the graph/plan layer, explicit backend-extension capability truth on `BackendSelection`, CPU reference execution for those families, and a later path to backend-specialized kernels without polluting the primitive-op surface. |
| [#3199](https://github.com/OpenAgentsInc/openagents/issues/3199) | Closed | `PSI-159` landed: explicit local-runtime observability surfaces in `psionic-runtime`, `psionic-serve`, and `psionic-provider`, with bounded transition logs for cold-load/warm/unload/backend-health changes, active-session and active-request counts, queue-depth and memory-footprint snapshots, and a managed-runtime `observability()` API. |
| [#3169](https://github.com/OpenAgentsInc/openagents/issues/3169) | Closed | `PSI-126A` landed: runtime-owned paged-KV policy/accounting, a logically paged per-session KV cache in `psionic-serve` with explicit `refuse_new_pages` behavior, session metadata bound to KV policy/state, and generation/provider evidence carrying KV pages, bytes, and growth. |
| [#3231](https://github.com/OpenAgentsInc/openagents/issues/3231) | Closed | `PSI-126B` landed: explicit shared prefix-cache policy/state/identity in `psionic-runtime`, longest-safe prefix reuse plus stale rebuild and bypass handling in `psionic-serve`, and provider/receipt truth for prefix reuse and reused-token counts. |
| [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233) | Closed | `PSI-162` landed: first-class served-artifact identity tuples in `psionic-runtime`, descriptor-side artifact identity metadata in `psionic-models`, provider/serve capability+receipt+provenance surfacing, and cache/session/request-digest invalidation keyed to artifact drift instead of display names alone. |
| [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234) | Closed | `PSI-163` landed: reusable runtime cache invalidation policy and cache observations, with explicit scopes/format versions/triggers for plan, kernel, paged-tensor, prefix, and KV caches plus provider/serve evidence surfaces that report realized reuse, rebuild, bypass, invalidate, and restore actions. |
| [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235) | Closed | `PSI-164` landed: explicit local-artifact provenance and declared-license facts in `psionic-catalog` and `psionic-models`, plus provider-side compute-market supply policy, advertise/serve decisions, and structured refusal diagnostics that stay distinct from integrity and unsupported-format failures. |
| [#3236](https://github.com/OpenAgentsInc/openagents/issues/3236) | Closed | `PSI-165` landed: OCI/Docker-v2 registry pull into the local Ollama-style manifest/blob store, shared manifest validation for remote and local paths, digest/size-checked blob ingestion with reuse reporting, and `psionic-models` loading from a resolved local manifest. |
| [#3172](https://github.com/OpenAgentsInc/openagents/issues/3172) | Closed | `PSI-110` landed: reusable GGUF metadata/tensor parsing, `WeightFormat::Gguf`, `GgufWeightBundleLoader`, and truthful GGUF tensor-type coverage for currently supported dense and quantized families. |
| [#3173](https://github.com/OpenAgentsInc/openagents/issues/3173) | Closed | `PSI-111` landed: reusable GGUF tokenizer metadata loading for SentencePiece and GPT-style BPE families, stable tokenizer digests, preserved BOS/EOS/add-bos/add-eos and pretokenizer truth, and validation for missing or invalid tokenizer metadata. |
| [#3174](https://github.com/OpenAgentsInc/openagents/issues/3174), [#3175](https://github.com/OpenAgentsInc/openagents/issues/3175), [#3176](https://github.com/OpenAgentsInc/openagents/issues/3176), [#3221](https://github.com/OpenAgentsInc/openagents/issues/3221) | Closed | Historical roadmap-seeded duplicates for `PSI-117`, `PSI-118`, `PSI-119`, and `PSI-161`; use the detailed issues `#3166`, `#3167`, `#3168`, and `#3171` for landed scope. |

Current execution queue in dependency order, verified against live GitHub issue
state:

| Order | Local ID | GitHub issue | State | Why this is the current flow |
| --- | --- | --- | --- | --- |
| 1 | `PSI-110` | [#3172](https://github.com/OpenAgentsInc/openagents/issues/3172) | Closed | GGUF metadata and tensor loader substrate is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 2 | `PSI-111` | [#3173](https://github.com/OpenAgentsInc/openagents/issues/3173) | Closed | GGUF tokenizer metadata loading is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 3 | `PSI-115` | [#3164](https://github.com/OpenAgentsInc/openagents/issues/3164) | Closed | Already landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 4 | `PSI-116` | [#3165](https://github.com/OpenAgentsInc/openagents/issues/3165) | Closed | Paged GGUF and Ollama blob access is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 5 | `PSI-118` | [#3167](https://github.com/OpenAgentsInc/openagents/issues/3167) | Closed | The fixture corpus is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 6 | `PSI-117` | [#3166](https://github.com/OpenAgentsInc/openagents/issues/3166) | Closed | The conformance harness is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 7 | `PSI-119` | [#3168](https://github.com/OpenAgentsInc/openagents/issues/3168) | Closed | The backend parity policy is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 8 | `PSI-112` | [#3177](https://github.com/OpenAgentsInc/openagents/issues/3177) | Closed | GGUF-backed decoder-family adapters are now landed on `main`; keep them in sequence but skip them when choosing the next issue. |
| 9 | `PSI-113` | [#3178](https://github.com/OpenAgentsInc/openagents/issues/3178) | Closed | GGUF-backed embeddings adapters are now landed on `main`; keep them in sequence but skip them when choosing the next issue. |
| 10 | `PSI-114` | [#3179](https://github.com/OpenAgentsInc/openagents/issues/3179) | Closed | Supported GGUF prompt rendering is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 11 | `PSI-120` | [#3180](https://github.com/OpenAgentsInc/openagents/issues/3180) | Closed | Local Ollama manifest/blob discovery and model resolution are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 12 | `PSI-121` | [#3181](https://github.com/OpenAgentsInc/openagents/issues/3181) | Closed | Installed-model list/show parity is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 13 | `PSI-122` | [#3182](https://github.com/OpenAgentsInc/openagents/issues/3182) | Closed | Loaded-model lifecycle is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 14 | `PSI-123` | [#3183](https://github.com/OpenAgentsInc/openagents/issues/3183) | Closed | Generation options are now landed on `main`; keep them in sequence but skip them when choosing the next issue. |
| 15 | `PSI-124` | [#3184](https://github.com/OpenAgentsInc/openagents/issues/3184) | Closed | Metrics and provenance now describe the real option-bearing generation path, so keep this in sequence but skip it when choosing the next issue. |
| 16 | `PSI-125` | [#3185](https://github.com/OpenAgentsInc/openagents/issues/3185) | Closed | The app-facing library API boundary is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 17 | `PSI-126` | [#3186](https://github.com/OpenAgentsInc/openagents/issues/3186) | Closed | Deterministic text-generation session ownership is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 18 | `PSI-126A` | [#3169](https://github.com/OpenAgentsInc/openagents/issues/3169) | Closed | Paged KV layout, accounting, and explicit refusal policy are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 19 | `PSI-126B` | [#3231](https://github.com/OpenAgentsInc/openagents/issues/3231) | Closed | Shared prefix reuse truth is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 20 | `PSI-165` | [#3236](https://github.com/OpenAgentsInc/openagents/issues/3236) | Closed | Remote OCI ingestion is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 21 | `PSI-127` | [#3187](https://github.com/OpenAgentsInc/openagents/issues/3187) | Closed | Explicit context-window budgeting, truncation policy, and over-limit refusal semantics are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 22 | `PSI-128` | [#3188](https://github.com/OpenAgentsInc/openagents/issues/3188) | Closed | Deterministic sampling and replay semantics are now landed on `main` through the runtime-owned sampler policy; keep this in sequence but skip it when choosing the next issue. |
| 23 | `PSI-129` | [#3189](https://github.com/OpenAgentsInc/openagents/issues/3189) | Closed | Local-serving memory planning, residency policy, and admission control are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 24 | `PSI-133` | [#3190](https://github.com/OpenAgentsInc/openagents/issues/3190) | Closed | Local runtime streaming semantics are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 25 | `PSI-134` | [#3191](https://github.com/OpenAgentsInc/openagents/issues/3191) | Closed | Embeddings batch semantics, metadata reporting, and failure behavior are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 26 | `PSI-135` | [#3192](https://github.com/OpenAgentsInc/openagents/issues/3192) | Closed | Local model-store integrity verification and repair diagnostics are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 27 | `PSI-136` | [#3193](https://github.com/OpenAgentsInc/openagents/issues/3193) | Closed | Backend-neutral local runtime diagnostics are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 28 | `PSI-137` | [#3194](https://github.com/OpenAgentsInc/openagents/issues/3194) | Closed | Explicit served-product backend fallback, refusal, and degraded-state policy is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 29 | `PSI-138` | [#3195](https://github.com/OpenAgentsInc/openagents/issues/3195) | Closed | Explicit cutover performance gates are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 30 | `PSI-139` | [#3196](https://github.com/OpenAgentsInc/openagents/issues/3196) | Closed | Ollama adapter policy is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 31 | `PSI-157` | [#3197](https://github.com/OpenAgentsInc/openagents/issues/3197) | Closed | Allocator pooling, bounded kernel caches, and device-memory-budget truth are now explicit on `main`; keep this in sequence but skip it when choosing the next issue. |
| 32 | `PSI-158` | [#3198](https://github.com/OpenAgentsInc/openagents/issues/3198) | Closed | Typed backend-extension hooks are now explicit on `main`; keep this in sequence but skip it when choosing the next issue. |
| 33 | `PSI-159` | [#3199](https://github.com/OpenAgentsInc/openagents/issues/3199) | Closed | Local runtime observability is now explicit on `main`; keep this in sequence but skip it when choosing the next issue. |
| 34 | `PSI-156` | [#3170](https://github.com/OpenAgentsInc/openagents/issues/3170) | Closed | Quantized execution parity is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 35 | `PSI-130` | [#3200](https://github.com/OpenAgentsInc/openagents/issues/3200) | Closed | Metal now truthfully exposes the current text-generation dense surface on `main`; keep this in sequence but skip it when choosing the next issue. |
| 36 | `PSI-131` | [#3201](https://github.com/OpenAgentsInc/openagents/issues/3201) | Closed | CPU-vs-Metal parity coverage for the current text-generation graph is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 37 | `PSI-132` | [#3202](https://github.com/OpenAgentsInc/openagents/issues/3202) | Closed | The served Metal text-generation product path is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 38 | `PSI-140` | [#3203](https://github.com/OpenAgentsInc/openagents/issues/3203) | Closed | CUDA backend architecture and explicit pre-discovery truth are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 39 | `PSI-141` | [#3204](https://github.com/OpenAgentsInc/openagents/issues/3204) | Closed | NVIDIA topology, risk, and provider-visible evidence are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 40 | `PSI-142` | [#3205](https://github.com/OpenAgentsInc/openagents/issues/3205) | Closed | CUDA discovery and health reporting are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 41 | `PSI-143` | [#3206](https://github.com/OpenAgentsInc/openagents/issues/3206) | Closed | CUDA allocation, buffer, stream, and submission substrate are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 42 | `PSI-144` | [#3207](https://github.com/OpenAgentsInc/openagents/issues/3207) | Closed | CUDA lowering and the first NVIDIA dense primitive surface are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 43 | `PSI-145` | [#3208](https://github.com/OpenAgentsInc/openagents/issues/3208) | Closed | CUDA backend selection and provider truth are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 44 | `PSI-146` | [#3209](https://github.com/OpenAgentsInc/openagents/issues/3209) | Closed | CPU-vs-NVIDIA parity evidence for the first supported served path is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 45 | `PSI-147` | [#3210](https://github.com/OpenAgentsInc/openagents/issues/3210) | Closed | The first NVIDIA-backed served product path is now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 46 | `PSI-148` | [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232) | Closed | The minimum hardware validation matrix is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 47 | `PSI-150` | [#3211](https://github.com/OpenAgentsInc/openagents/issues/3211) | Closed | AMD execution substrate groundwork is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 48 | `PSI-151` | [#3212](https://github.com/OpenAgentsInc/openagents/issues/3212) | Closed (Not Planned) | Closed after host reprioritization to NVIDIA-only execution; keep it in sequence for history but skip it when choosing the next issue. |
| 49 | `PSI-152` | [#3213](https://github.com/OpenAgentsInc/openagents/issues/3213) | Closed (Not Planned) | Closed after host reprioritization to NVIDIA-only execution; keep it in sequence for history but skip it when choosing the next issue. |
| 50 | `PSI-153` | [#3214](https://github.com/OpenAgentsInc/openagents/issues/3214) | Closed (Not Planned) | Closed after host reprioritization to NVIDIA-only execution; keep it in sequence for history but skip it when choosing the next issue. |
| 51 | `PSI-154` | [#3215](https://github.com/OpenAgentsInc/openagents/issues/3215) | Closed (Not Planned) | Closed after host reprioritization to NVIDIA-only execution; keep it in sequence for history but skip it when choosing the next issue. |
| 52 | `PSI-160` | [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220) | Closed | The process-isolation contract is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 53 | `PSI-161` | [#3171](https://github.com/OpenAgentsInc/openagents/issues/3171) | Closed | The fallback lattice is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 54 | `PSI-162` | [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233) | Closed | Served-artifact identity is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 55 | `PSI-163` | [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234) | Closed | Cache invalidation is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 56 | `PSI-164` | [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235) | Closed | Artifact provenance/license gating is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 57 | `PSI-170` | [#3222](https://github.com/OpenAgentsInc/openagents/issues/3222) | Closed | The Ollama-compat versus Psionic-native boundary is now explicit on `main`; keep it in sequence but skip it when choosing the next issue. |
| 58 | `PSI-171` | [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223) | Closed | Compute-market inventory and backend-toolchain qualifiers are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 59 | `PSI-172` | [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224) | Closed | Batch posture, queue policy, and throughput-class truth are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 60 | `PSI-173` | [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225) | Closed | Multi-device selection and explicit sharding/topology planning are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 61 | `PSI-174` | [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226) | Closed | Execution-plan cache policy/state plus compile-path evidence are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 62 | `PSI-175` | [#3227](https://github.com/OpenAgentsInc/openagents/issues/3227) | Closed | Delivery-proof and settlement-linkage evidence are now landed on `main`; keep this in sequence but skip it when choosing the next issue. |
| 63 | `OA-200` | [#3216](https://github.com/OpenAgentsInc/openagents/issues/3216) | Closed | The app-owned local-execution evidence rename is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 64 | `OA-201` | [#3217](https://github.com/OpenAgentsInc/openagents/issues/3217) | Closed | The app-owned local inference runtime seam is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 65 | `OA-202` | [#3218](https://github.com/OpenAgentsInc/openagents/issues/3218) | Closed | The desktop now defaults to the in-process Psionic runtime on `main`; keep it in sequence but skip it when choosing the next issue. |
| 66 | `OA-203` | [#3219](https://github.com/OpenAgentsInc/openagents/issues/3219) | Closed | The production desktop no longer compiles the external Ollama worker path by default, and the remaining user-facing local-runtime wording now says Psionic/local inference instead of implying external Ollama. |
| 67 | `PSI-176` | [#3228](https://github.com/OpenAgentsInc/openagents/issues/3228) | Closed | Bounded sandbox execution now has a reusable runtime-owned capability profile and provider envelope, with explicit isolation, filesystem, network, process, resource, and accelerator-access bounds instead of leaving sandbox posture implicit. |
| 68 | `PSI-177` | [#3229](https://github.com/OpenAgentsInc/openagents/issues/3229) | Closed | Sandbox execution now has reusable request-identity, evidence, and provider-receipt contracts with explicit digests, resource summaries, delivery-proof passthrough, and terminal exit reasons instead of leaving compute-market receipts to reconstruct that state later. |
| 69 | `PSI-178` | [#3230](https://github.com/OpenAgentsInc/openagents/issues/3230) | Closed | Topology-aware substitution and deliverability checks now exist in the reusable runtime/provider layer, so accelerator-sensitive offers can distinguish exact delivery, compatible substitution, and underdelivery from explicit promised-versus-delivered facts. |
| 70 | `PSI-179` | [#3239](https://github.com/OpenAgentsInc/openagents/issues/3239) | Closed | GPT-OSS / OpenAI-MoE GGUF loading and truthful mixed `MXFP4` / `Q8_0` storage are now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 71 | `PSI-180` | [#3240](https://github.com/OpenAgentsInc/openagents/issues/3240) | Closed | Harmony prompt/render/parse truth is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 72 | `PSI-181` | [#3237](https://github.com/OpenAgentsInc/openagents/issues/3237) | Closed | The real GGUF-backed GPT-OSS decoder execution model is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 73 | `PSI-182` | [#3238](https://github.com/OpenAgentsInc/openagents/issues/3238) | Closed | NVIDIA text-generation kernel coverage for the real GPT-OSS decoder path is now landed on `main`; keep it in sequence but skip it when choosing the next issue. |
| 74 | `PSI-183` | [#3241](https://github.com/OpenAgentsInc/openagents/issues/3241) | Closed | The Psionic-only GPT-OSS 20B NVIDIA-host flow is now validated on `main`; keep it in sequence but skip it when choosing the next issue. |
| 75 | `GPT-OSS-PERF-1` | [#3242](https://github.com/OpenAgentsInc/openagents/issues/3242) | Closed | Benchmark evidence, JSON summaries, and Psionic-owned GPT-OSS perf receipts are now landed; keep this in sequence but skip it when choosing the next issue. |
| 76 | `GPT-OSS-PERF-2` | [#3243](https://github.com/OpenAgentsInc/openagents/issues/3243) | Closed | Device-resident activation/scratch surfaces and removal of per-matvec round-trips are now landed; keep this in sequence but skip it when choosing the next issue. |
| 77 | `GPT-OSS-PERF-3` | [#3244](https://github.com/OpenAgentsInc/openagents/issues/3244) | Closed | CUDA KV, RMSNorm, RoPE, and decode attention are now landed; keep this in sequence but skip it when choosing the next issue. |
| 78 | `GPT-OSS-PERF-4` | [#3245](https://github.com/OpenAgentsInc/openagents/issues/3245) | Closed | CUDA router selection and MoE execution substrate are now landed; keep this in sequence but skip it when choosing the next issue. |
| 79 | `GPT-OSS-PERF-5` | [#3246](https://github.com/OpenAgentsInc/openagents/issues/3246) | Closed | The reusable GPT-OSS CUDA step-plan/runtime substrate is now landed; keep this in sequence but skip it when choosing the next issue. |
| 80 | `GPT-OSS-PERF-6A` | [#3249](https://github.com/OpenAgentsInc/openagents/issues/3249) | Closed | The graph/fusion alignment checkpoint work is done and no longer the current bottleneck; keep it in sequence for history but skip it when choosing the next issue. |
| 81 | `GPT-OSS-PERF-6` | [#3247](https://github.com/OpenAgentsInc/openagents/issues/3247) | Closed | Closed as superseded by the narrower current queue; keep it in sequence for history but skip it when choosing the next issue. |
| 82 | `GPT-OSS-PERF-6B` | [#3276](https://github.com/OpenAgentsInc/openagents/issues/3276) | Closed | Closed after `#3293` pushed the exact prompt-cache-hit lane from the old `122-123 tok/s` floor into the low `171 tok/s` class, satisfying the `150+ tok/s` umbrella target on this host. |
| 83 | `GPT-OSS-PERF-6C` | [#3288](https://github.com/OpenAgentsInc/openagents/issues/3288) | Closed | Closed as superseded by the narrower exact `llama.cpp` parity chain; keep it in sequence for history but do not treat it as the next issue. |
| 84 | `GPT-OSS-PERF-6D` | [#3293](https://github.com/OpenAgentsInc/openagents/issues/3293) | Closed | Closed after Psionic split GPT-OSS router execution into `matmul + bias + delayed-softmax top-k`, matching the intended `llama.cpp` structure closely enough to move the exact prompt-cache-hit lane into the `171-173 tok/s` class. |
| 85 | `GPT-OSS-PERF-6E` | [#3294](https://github.com/OpenAgentsInc/openagents/issues/3294) | Closed | Closed after Psionic moved the real GPT-OSS decode lane onto a reusable ids-driven expert-matvec backend surface and a grouped project kernel while keeping the exact prompt-cache-hit benchmark in the same `170-173 tok/s` class. |
| 86 | `GPT-OSS-PERF-6F` | [#3295](https://github.com/OpenAgentsInc/openagents/issues/3295) | Closed | Closed after `#3294` because the benchmark target was already honestly met on this host without needing a riskier fused gate/up rewrite. Keep the idea as future cleanup, but it is no longer an active blocker for the tracked GPT-OSS parity contract. |
| 87 | `GPT-OSS-PERF-6G` | [#3296](https://github.com/OpenAgentsInc/openagents/issues/3296) | Closed | Closed after parity was already reached on the tracked benchmark without needing an attention-dispatch rewrite. Keep `fattn.cu` alignment as future headroom work, not as the current blocker for this host contract. |
| 88 | `GPT-OSS-PERF-7` | [#3248](https://github.com/OpenAgentsInc/openagents/issues/3248) | Closed | Closed after the benchmark script itself was made contract-clean on both servers and Psionic still measured ahead of the local `llama.cpp` control with the same visible output on the exact prompt-cache-hit lane. |
| 89 | `GPT-OSS-120B-PERF-1` | [#3338](https://github.com/OpenAgentsInc/openagents/issues/3338) | Closed | Closed after direct registered-host expert execution and registered-host cache-fill copies both regressed the 120B prompt-cache-hit lane into the `6.4 tok/s` class on this host. Keep it only as a ruled-out history marker. |
| 90 | `GPT-OSS-120B-PERF-2` | [#3345](https://github.com/OpenAgentsInc/openagents/issues/3345) | Open | The active NVIDIA throughput umbrella on this host is still the hybrid 120B path. The current pushed branch still keeps the profiled selected4 cache layout (`8` slots on hot layers `23, 25, 28, 29`, `6` slots on `10, 18, 21, 22, 26, 31, 33`, and `5` elsewhere), but fresh reruns keep the truthful reproducible exact-contract floor in about the `2.25 tok/s` cold, `6.45-6.50 tok/s` warm-non-hit, and `10.50 tok/s` prompt-cache-hit class. `#3345` now points at the focused CUDA host-mapped GGUF expert-page follow-up in `#3360`. |
| 91 | `GPT-OSS-120B-PERF-3` | [#3360](https://github.com/OpenAgentsInc/openagents/issues/3360) | Open | Focused next step for 120B: add a CUDA host-registration / alias path for existing mmap-backed GGUF expert pages, use it first for the full host-backed down-expert tensor, and switch the hybrid 120B down projection onto the existing ids-driven grouped-expert kernel so one large selected4 staging leg disappears before attempting the larger gate/up packed-kernel port. |
| 92 | `METAL-GPT-OSS-1` | [#3270](https://github.com/OpenAgentsInc/openagents/issues/3270) | Open | This is the first Apple Silicon native-Rust Metal issue because the current benchmark still defaults to `llama.cpp` proxy mode on macOS, which makes any Metal throughput claim ambiguous before we even improve the native path. |
| 93 | `METAL-GPT-OSS-2` | [#3268](https://github.com/OpenAgentsInc/openagents/issues/3268) | Open | After benchmark honesty is fixed, the next native Metal blocker is structural: `psionic-backend-metal` already has device KV, shared-prefix, and reserved attention runtime substrate, but `psionic-serve` still routes the shipped Metal GPT-OSS path through host KV and `attend_impl(...)`. |
| 94 | `METAL-GPT-OSS-3` | [#3269](https://github.com/OpenAgentsInc/openagents/issues/3269) | Open | Once the serve path is using backend-owned KV and reserved attention runtime, the next gap is the remaining CPU-owned RMSNorm, RoPE, router, softmax, SwiGLU, and expert aggregation work in `GptOssMetalModelInner::forward_step(...)`. |
| 95 | `METAL-GPT-OSS-4` | [#3271](https://github.com/OpenAgentsInc/openagents/issues/3271) | Open | After the decode math is truly device-owned, the next high-value work is removing the one-op-submit / wait / readback pattern and replacing it with reusable scratch buffers plus chained command submission. |
| 96 | `METAL-GPT-OSS-5` | [#3272](https://github.com/OpenAgentsInc/openagents/issues/3272) | Open | Bounded logits output is the next cleanup after the native hot path stops round-tripping every intermediate value, because greedy and bounded-candidate requests should not read back full raw logits by default. |
| 97 | `METAL-GPT-OSS-6` | [#3261](https://github.com/OpenAgentsInc/openagents/issues/3261) | Open | Keep the validation and benchmark-evidence issue open until the native Rust Metal path, not the proxy path, has seeded parity coverage plus warm and prompt-cache-hit receipts. |
| 98 | `METAL-GPT-OSS-7` | [#3262](https://github.com/OpenAgentsInc/openagents/issues/3262) | Open | Keep the Apple throughput umbrella open until the same-host native Rust Metal path reaches the agreed llama.cpp-relative throughput band on the real benchmark contract. |

The active roadmap issues on this host now split across one NVIDIA headroom
item and the Apple Silicon native-Rust Metal queue. Do not reopen the closed
20B parity chain unless the benchmark regresses or the contract changes.

The active NVIDIA queue is now `#3345 -> #3360`, focused only on the GPT-OSS
120B hybrid path. The active Apple Silicon native-Rust Metal queue is `#3270` then
`#3268` then `#3269` then `#3271` then `#3272` then `#3261` then `#3262`. Do
not use proxy-mode benchmark results to claim closure for any of those Metal
issues.

The inference-generalization queue from
`docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md` is now active too: `PSI-233` /
[#3538](https://github.com/OpenAgentsInc/openagents/issues/3538) is now landed
in-tree through a generic GGUF CPU runtime that executes Llama, Qwen, and
Mistral families through Psionic-owned paths. `PSI-234` /
[#3539](https://github.com/OpenAgentsInc/openagents/issues/3539) is now landed
too through the generic `psionic-openai-server` path. `PSI-235` /
[#3540](https://github.com/OpenAgentsInc/openagents/issues/3540) is now landed
too through explicit CPU-lane residency, fallback, and control truth on the
generic server surface. `PSI-236` /
[#3541](https://github.com/OpenAgentsInc/openagents/issues/3541) is now landed
too through Psionic-owned GBNF and JSON-schema constrained-generation fallback
on the generic server, so the next dependency-ordered issue in that chain is
`PSI-237` / [#3542](https://github.com/OpenAgentsInc/openagents/issues/3542).

## Current Reality

The checked-in repo is no longer at "phase 0 bootstrap." The current truthful
baseline on `main` is:

- the generic Psionic/Ollama-replacement track is landed, and the real GPT-OSS
  path is now landed too: Psionic loads `gpt-oss` / OpenAI-MoE GGUFs, executes the
  real decoder path, preserves mixed `MXFP4` / `Q8_0` storage truth, and now
  exposes a real GGUF-backed NVIDIA text-generation path
- Psionic now also has a generic CPU GGUF execution entrypoint for dense
  decoder families beyond GPT-OSS, with real runtime tokenization plus
  representative executed Llama, Qwen, and Mistral paths instead of treating
  those families as metadata-only adapters
- Psionic now also has a generic OpenAI-compatible CPU server path,
  `psionic-openai-server`, that can boot multiple loaded GGUF families on one
  `/v1/chat/completions` surface while still explicitly refusing unfinished
  APIs such as `/v1/embeddings`
- that generic server now also reports CPU-only residency, unsupported hybrid
  offload, refuse-on-fallback behavior, and explicit non-implemented
  warm/unload or memory-pressure controls instead of implying a stronger local
  lane than is actually shipped today
- that generic server now also has a Psionic-owned constrained-generation
  fallback for `psionic_grammar`, `response_format.type = json_object`, and a
  useful `json_schema` subset, with explicit response headers and refusal on
  unsupported schema features instead of hiding structure behind prompt hacks
- this NVIDIA host can run the local
  `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf` file through
  both external `~/code/llama.cpp` as a reference oracle and Psionic alone through
  the local OpenAI-compatible `psionic-gpt-oss-server`
- the active gap is now measured, not speculative: the final contract-clean
  prompt-cache-hit parity run on this host now measures Psionic at
  `172.84 tok/s` versus `llama.cpp` at `160.98 tok/s`, with
  `prompt_cache_hit_visible_output_match=true` on the exact same benchmark
  script
- the kept pre-close Psionic floor on the older script form remains in the low
  `171 tok/s` class, with runs at `173.19 tok/s`, `171.29 tok/s`, `173.05
  tok/s`, and `170.05 tok/s`
- the 20B NVIDIA parity chain is closed, but the active remaining NVIDIA work
  is now the hybrid 120B path under `#3345`
  - current truthful 120B floor on the exact contract is still about
    `2.24-2.26 tok/s` cold, `6.43-6.51 tok/s` warm-non-hit, and
    `10.41-10.57 tok/s` prompt-cache-hit
  - quick cache-shape retunes around the current kept branch were already
    measured and rejected: `7` expanded slots on the last `4` layers fell to
    `9.89 tok/s`, and `6` expanded slots on the last `8` layers fell to
    `9.92 tok/s`
  - the `#3338` registered-host-buffer hypothesis is now ruled out on this
    host: direct registered-host expert execution and registered-host cache-fill
    copies both regressed into the `6.4 tok/s` class
  - the next honest 120B direction is now narrower:
    the generation-only hidden-state-residency checkpoint and stateless
    host-KV-materialization skip are both landed, but only the former has
    shown a durable throughput shift on clean reruns, so Psionic still needs
    to reduce the remaining host-to-device selected4 expert staging inside the
    hybrid host-backed MoE lane
- the Apple Silicon Metal groundwork from `#3250` and `#3252` through `#3260`
  is already landed in-tree, but the 2026-03-09 audit shows the native serve
  path still does not consume that substrate end to end: the benchmark defaults
  to proxy mode on macOS, and `GptOssMetalModelInner` still retains host-owned
  KV, attention, and MoE control flow. The remaining native-Rust Metal queue is
  `#3270` -> `#3268` -> `#3269` -> `#3271` -> `#3272` -> `#3261` -> `#3262`.
- the measured benchmark gap is now split across two visible classes of work
  instead of one vague "CUDA is slower" bucket
  - `llama.cpp` serves the timed request from a live prompt cache and only
    re-evaluates one prompt token on the second request
  - Psionic still reconstructs the CUDA KV mirror from host-owned prefix cache
    state on each HTTP request, so request-to-request prompt reuse is still not
    backend-resident the way it is in `llama.cpp`
  - on the exact current decode contract, short `ncu` sampling shows the
    sampled `router_topk_softmax_32_kernel` at about `107.7 us`, the sampled
    selected4 `moe_gate_up_swiglu_q8_1` kernel at about `72.6 us`, and the
    sampled selected4 expert-project kernel at about `33.3 us`, while sampled
    decode attention is only about `4.7 us`
  - that means the next honest work is not another generic fusion pass; it is
    the remaining `llama.cpp` execution chain now tracked under `#3294`
    through `#3296`, with `#3248` left open as the final parity umbrella over
    those decode-critical stages
- CPU model-backed embeddings and text generation exist and are tested
- initial GGML quantized tensor storage and decode coverage now extends to
  CPU-native `Q4_0`, `Q4_1`, and `Q8_0` execution over preserved GGML block
  bytes, with explicit `backend_quantized` + `native` CPU capability truth and
  explicit Metal refusal instead of silent quantized fallback
- local GGUF and Ollama blobs can now be opened through mmap-or-buffered
  fallback paths with explicit paging and storage-truth metadata
- local Ollama manifests can now be discovered and resolved through a
  non-mutating `psionic-catalog` surface with explicit scan warnings, normalized
  model names, parsed layer/media-type records, and blob-presence truth
- that same local Ollama-style store can now be integrity-verified per model,
  with structured diagnostics for missing manifests/blobs and corrupt or
  size-mismatched blobs, plus repair-action hints instead of path-exists truth
- remote OCI/Docker-v2 registries can now populate that same local
  Ollama-style manifest/blob store through explicit pull reports, shared
  manifest validation, digest/size-checked blob ingestion, and manifest-based
  GGUF loading in `psionic-models`
- local installed-model `tags` / `show` parity now exists in `psionic-serve`
  without the Ollama daemon, backed by the shared catalog plus explicit GGUF
  model-info and capability derivation
- loaded-model warm/load/unload and keepalive lifecycle now exists in
  `psionic-serve` and `psionic-runtime`, with explicit residency truth, active-request
  counts, `ps`-style ordering, and zero-keepalive idle unload semantics
- local runtime observability now exists in `psionic-runtime`, `psionic-serve`, and
  `psionic-provider`, with bounded transition logs for cold-load, first-request
  warm, unload, and backend-health changes, plus active-session, queue-depth,
  active-request, and memory-footprint snapshots on the managed runtime seam
- `autopilot-desktop` now owns a backend-neutral `LocalInferenceRuntime` seam,
  with adapters for the current Ollama worker and the in-process Psionic reference
  runtime, and the desktop now defaults that seam to the in-process Psionic path
  while preserving the existing app-facing execution snapshot flow
- generation option handling now exists in `psionic-serve` for temperature, top-k,
  top-p, seed, stop sequences, and repeat/presence/frequency penalties, with
  seeded sampling and explicit stop-sequence truncation on the CPU reference
  path
- sampler policy and seeded replay behavior now exist in `psionic-runtime`, with
  Ollama-aligned defaults, transform order, and bounded penalty lookback, and
  `psionic-serve` now delegates supported token selection to that runtime surface
- context-window budgeting now exists in `psionic-models` and `psionic-serve`, with
  explicit prompt-budget accounting, opt-in oldest-token truncation, and
  Ollama-aligned over-limit refusal strings instead of implicit prompt prefill
  failure
- generation metrics and provenance now exist in `psionic-serve` and
  `psionic-provider`, with prompt/output token counts, total/load durations,
  warm/cold load-state truth, and execution-plan digests on responses and
  receipts
- a library-first in-process runtime API now exists in `psionic-serve`, with a
  reusable aggregate wrapper over local catalog inspection, loaded-model
  lifecycle, text generation, and embeddings execution
- pull-driven local streaming generation now exists in `psionic-serve` and
  `psionic-provider`, with explicit backpressure/disconnect/cancellation policy,
  typed chunk-vs-terminal events, partial-output terminal semantics, and
  streaming-policy truth carried in provenance, receipts, and capability
  envelopes
- local runtime failures now have a backend-neutral diagnostics taxonomy in
  `psionic-runtime`, `psionic-serve`, and `psionic-provider`, with stable error codes plus
  status/message/context fields, serve-layer mappings for current request
  failures, and structured diagnostics preserved on streamed terminals and
  provider receipts
- served-product backend selection now has explicit unavailable/degraded
  policy plus direct, same-backend-degraded, and cross-backend-fallback state
  in `psionic-runtime`, `psionic-provider`, and the CPU/Metal backend seams instead of
  relying on a plain fallback string alone
- backend runtime resources now have explicit allocator-pool policy/state,
  kernel-cache policy/state, and device-memory-budget reporting in
  `psionic-runtime`, with CPU and Metal backends surfacing pooled-intermediate
  reuse and bounded cache truth instead of treating those policies as hidden
  backend internals
- typed backend-extension ops now exist for normalization, RoPE, attention,
  and quantized matmul, with explicit backend-extension capability truth on
  `BackendSelection` and CPU reference execution for those semantics while
  later accelerator issues add backend-specialized kernels
- cutover performance gates now exist in the conformance harness, with
  Ollama-aligned timing metrics retained on Psionic responses/receipts and
  normalized observations plus default ratio-based thresholds for generation
  and embeddings before desktop cutover
- embeddings execution now has explicit empty-batch behavior, requested
  output-dimension handling, ordered-batch truth, model-family/revision and
  normalization metadata, and provider-facing capability/receipt reporting for
  those semantics
- text-generation sessions in `psionic-serve` now bind to full decoder identity
  and own their token sequence alongside KV state instead of relying on
  model-name-only cache reuse
- text-generation KV state now has an explicit logical page layout, byte/page
  growth accounting, and `refuse_new_pages` policy surfaced through
  `psionic-runtime`, `psionic-serve`, and `psionic-provider`
- shared prompt-prefix reuse now has explicit identity, policy, state, and
  reused-token evidence in `psionic-runtime`, `psionic-serve`, and `psionic-provider`,
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
  `psionic.text_generation` product path for the current dense matmul/add graph,
  with seeded CPU-vs-Metal parity coverage and explicit Metal unavailability
  diagnostics instead of silent CPU fallback
- a minimum shipped hardware validation matrix now exists across CPU, Apple
  Silicon Metal, NVIDIA CUDA embeddings, AMD KFD discovery, and refusal paths,
  with provider-facing validation claim IDs tied back to that matrix
- AMD has truthful discovery/readiness surfaces, but not execution kernels
- provider-facing capability and receipt truth is ahead of the app cutover
- Psionic still does not replace the desktop's Ollama dependency

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
- the long-term boundary between Ollama-compat support and Psionic-native model /
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
several behavioral contracts implicitly, and Psionic needs explicit semantics for
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
  storage, not just metadata parsing; the first Psionic pass of that landed in
  `PSI-115`, but full GGUF loader coverage still remains
- tokenizer reconstruction from GGUF metadata already carries BOS/EOS and
  template-processing implications, which means Psionic should not treat tokenizer
  loading and prompt rendering as one issue
- seeded sampler utilities, repeat penalty, and GQA helpers show that sampler
  correctness needs to include the surrounding decode helpers, not just RNG
- explicit compile-time backend features (`metal`, `cuda`, `cudnn`, `nccl`,
  `accelerate`, `mkl`) are a strong model for compiled-vs-probed capability
  truth
- Metal buffer pooling, bounded caches, and device-memory budget work show that
  backend allocator policy is a roadmap item, not a cleanup task
- Candle's custom-op and fused-kernel escape hatches are a practical template
  for how Psionic should land backend-specific attention and quantized GEMM kernels

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
| `PSI-110` | [#3172](https://github.com/OpenAgentsInc/openagents/issues/3172) | Closed | Add `WeightFormat::Gguf` and a reusable GGUF metadata/tensor loader | `psionic-models` | Required to read the format Ollama actually points at during migration. |
| `PSI-111` | [#3173](https://github.com/OpenAgentsInc/openagents/issues/3173) | Closed | Implement tokenizer loading from GGUF metadata for SentencePiece and GPT-style BPE families | `psionic-models` | Fixture tokenizers are not enough for real model parity. |
| `PSI-115` | [#3164](https://github.com/OpenAgentsInc/openagents/issues/3164) | Closed | Add GGML/GGUF quant block decode coverage and backend-backed quantized tensor storage | `psionic-models`, `psionic-runtime`, backend crates | Candle and Tinygrad both treat quantized tensor decode/storage as a core loader/runtime boundary; the first Psionic pass is now landed. |
| `PSI-116` | [#3165](https://github.com/OpenAgentsInc/openagents/issues/3165) | Closed | Add memory-mapped model blob access and paged tensor storage for local GGUF and Ollama blobs | `psionic-catalog`, `psionic-models`, `psionic-runtime` | Large local models now load through mmap-or-buffered blob access with explicit paging and storage-truth metadata. |
| `PSI-118` | [#3167](https://github.com/OpenAgentsInc/openagents/issues/3167) | Closed | Add golden prompt-rendering and tokenizer fixtures for supported model families from real GGUF and Ollama installs | `psionic-models`, `psionic-serve`, test fixtures | Prompt and tokenizer behavior drifts silently without a real golden corpus, and `PSI-117` depends on these fixtures. |
| `PSI-117` | [#3166](https://github.com/OpenAgentsInc/openagents/issues/3166) | Closed | Build an Ollama-to-Psionic conformance suite for `tags` / `show` / `ps` / `generate` / `embed` behavior, prompt rendering, truncation, stop handling, streaming, and error semantics | `psionic-catalog`, `psionic-serve`, `psionic-provider`, test fixtures | Cutover should be decided by repeatable conformance evidence, not hand inspection. |
| `PSI-119` | [#3168](https://github.com/OpenAgentsInc/openagents/issues/3168) | Closed | Define numerical parity tolerances and drift budgets across CPU and accelerated backends for embeddings and text generation | `psionic-serve`, backend crates, `psionic-provider` | Backend parity needs explicit tolerance rules across quant modes, decode loops, and embeddings outputs. |
| `PSI-112` | [#3177](https://github.com/OpenAgentsInc/openagents/issues/3177) | Closed | Add GGUF-backed decoder model-family adapters for first launch families (`llama`, `qwen`, `mistral`) | `psionic-models`, `psionic-serve` | Replaces model-family construction still hidden behind Ollama. |
| `PSI-113` | [#3178](https://github.com/OpenAgentsInc/openagents/issues/3178) | Closed | Add GGUF-backed embeddings model-family adapters for the first supported embedding families | `psionic-models`, `psionic-serve` | Keeps embeddings real rather than demo-only. |
| `PSI-114` | [#3179](https://github.com/OpenAgentsInc/openagents/issues/3179) | Closed | Implement chat-template extraction and prompt-rendering compatibility for supported model families | `psionic-models`, `psionic-serve` | Landed in `af6d82a42`: reusable GGUF prompt rendering for supported Phi-3 / Qwen2 / Command-R template digests, explicit prompt/render/error types, decoder render helpers, and conformance coverage that removed the old prompt-render gap. |

### Epic B: Ollama-compatible catalog and local runtime lifecycle

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-120` | [#3180](https://github.com/OpenAgentsInc/openagents/issues/3180) | Closed | Add Ollama manifest/blob discovery and model resolution on top of `psionic-catalog` | `psionic-catalog` | Landed in `859dc16c5`: non-mutating local manifest discovery and model resolution, Ollama-compatible default name normalization, parsed layer/media-type records, blob-presence truth, and explicit scan warnings for invalid entries. |
| `PSI-121` | [#3181](https://github.com/OpenAgentsInc/openagents/issues/3181) | Closed | Implement installed-model listing and inspection APIs equivalent to `tags` and `show` | `psionic-catalog`, `psionic-serve` | Landed in `d78ac7965`: manifest-layer config/text/json decode helpers in `psionic-catalog`, plus a local `tags` / `show` subject in `psionic-serve` that reads the shared catalog directly, derives GGUF model-info/capability truth without the Ollama daemon, skips bad config blobs during listing, and adds fixture-backed local list/show tests. |
| `PSI-122` | [#3182](https://github.com/OpenAgentsInc/openagents/issues/3182) | Closed | Implement loaded-model registry, warm/load/unload, and keepalive semantics equivalent to `ps` and warmups | `psionic-serve`, `psionic-runtime` | Landed in `eb921c9e8`: explicit loaded-model residency truth, warm/load/unload registry operations, `ps`-style ordering, and Ollama-aligned idle expiry semantics. |
| `PSI-123` | [#3183](https://github.com/OpenAgentsInc/openagents/issues/3183) | Closed | Expand generation options to cover `temperature`, `top_k`, `top_p`, penalties, `seed`, and `stop` | `psionic-serve` | Landed in `cf986e282`: explicit option fields, seeded sample mode, penalty-adjusted logits, and stop-sequence truncation on the CPU reference path. |
| `PSI-124` | [#3184](https://github.com/OpenAgentsInc/openagents/issues/3184) | Closed | Add generation metrics and provenance for prompt tokens, output tokens, load time, total time, warm/cold state, and plan digest | `psionic-serve`, `psionic-provider` | Preserves truthful receipts and UI projections after cutover. |
| `PSI-125` | [#3185](https://github.com/OpenAgentsInc/openagents/issues/3185) | Closed | Publish a library-first local runtime API for `list_models`, `show_model`, `loaded_models`, `warm_model`, `unload_model`, `generate`, and `embed` | `psionic-serve`, `psionic-provider` | Creates the in-process replacement boundary the app can call directly. |
| `PSI-126` | [#3186](https://github.com/OpenAgentsInc/openagents/issues/3186) | Closed | Add GGUF-backed KV-cache ownership and deterministic session lifecycle for text generation | `psionic-serve` | Landed in `cd3987928`: deterministic session ownership bound to full decoder identity, token-sequence ownership alongside KV state, explicit cache-plus-token commits, and descriptor-drift refusal coverage. |
| `PSI-126A` | [#3169](https://github.com/OpenAgentsInc/openagents/issues/3169) | Closed | Add paged KV-cache layout, accounting, and spill policy for long-context text generation | `psionic-serve`, `psionic-runtime`, `psionic-provider` | Landed in `0dfcf3f7c`: runtime-owned paged-KV policy/accounting, logically paged per-session KV state, explicit `refuse_new_pages` behavior, and generation/provider evidence for KV pages, bytes, and growth. |
| `PSI-126B` | [#3231](https://github.com/OpenAgentsInc/openagents/issues/3231) | Closed | Add shared prompt-prefix cache identity, reuse policy, accounting, and truth surfaces | `psionic-serve`, `psionic-runtime`, `psionic-provider` | Landed in `2bd89d48f`: runtime-owned prefix-cache policy/state/identity, longest-safe shared prefix reuse with stale rebuild and bypass handling in `psionic-serve`, and provider/receipt evidence for prefix reuse and reused-token counts. |
| `PSI-165` | [#3236](https://github.com/OpenAgentsInc/openagents/issues/3236) | Closed | Add OCI-distribution registry pull and model-ingestion pipeline (Ollama-compatible manifest + blobs) | `psionic-catalog`, `psionic-models` | Landed in `e4ffbee5b`: OCI/Docker-v2 manifest/blob pull into the local Ollama-style store, shared manifest validation for local and remote paths, digest/size-checked blob ingestion with reuse reporting, and manifest-based GGUF loading in `psionic-models`. |

### Epic C: Behavioral contract and serving policy

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-127` | [#3187](https://github.com/OpenAgentsInc/openagents/issues/3187) | Closed | Add explicit context-window accounting, truncation policy, and over-limit error semantics | `psionic-models`, `psionic-serve` | Landed in `bf0cf75a8`: reusable context-window budgeting in `psionic-models`, explicit `refuse` vs `truncate_oldest` prompt-overflow policy in `psionic-serve`, Ollama-aligned over-limit error strings, and regression coverage for truncation and session-owned context pressure. |
| `PSI-128` | [#3188](https://github.com/OpenAgentsInc/openagents/issues/3188) | Closed | Add deterministic sampler implementation and replay coverage for supported generation options | `psionic-serve`, `psionic-runtime` | Landed in `9e283787a` and `875de50b9`: runtime-owned sampler policy and seeded replay behavior in `psionic-runtime`, Ollama-aligned defaults/transform order plus bounded penalty lookback, `psionic-serve` delegation to that runtime sampler, and full replay coverage for the supported option surface. |
| `PSI-129` | [#3189](https://github.com/OpenAgentsInc/openagents/issues/3189) | Closed | Add model memory planning, residency policy, and admission control for local serving | `psionic-serve`, `psionic-runtime`, `psionic-provider` | Landed in `a12badddb`: reusable memory-plan and residency-policy substrate in `psionic-runtime`, admission-aware loaded-model registry behavior in `psionic-serve`, bounded-budget refusal plus optional idle-oldest eviction, default decoder memory planning, and capability/receipt evidence carrying memory-plan and residency-snapshot truth. |
| `PSI-133` | [#3190](https://github.com/OpenAgentsInc/openagents/issues/3190) | Closed | Add streaming token generation, backpressure, disconnect, and cancellation semantics for the local runtime API | `psionic-serve`, `psionic-provider` | Landed in `eb0f84af2`: pull-driven local streaming generation, explicit backpressure/disconnect/cancellation policy, typed chunk and terminal events, partial-output terminal semantics, and provider receipt/capability streaming truth. |
| `PSI-134` | [#3191](https://github.com/OpenAgentsInc/openagents/issues/3191) | Closed | Add embeddings API parity, batch semantics, and model metadata reporting | `psionic-serve`, `psionic-provider` | Landed in `1600ec4bc`: empty-batch success, requested output dimensions with re-normalization, explicit model-family/revision/normalization metadata, ordered-batch capability truth, and explicit no-input-truncation support on current byte-projection embeddings paths. |
| `PSI-135` | [#3192](https://github.com/OpenAgentsInc/openagents/issues/3192) | Closed | Add local model-store integrity verification and cache-repair diagnostics | `psionic-catalog`, `psionic-models` | Landed in `2516c70e3`: structured per-model integrity diagnostics and repair hints in `psionic-catalog`, plus `psionic-models` refusal of corrupt primary GGUF blobs from manifest-backed loads. |
| `PSI-136` | [#3193](https://github.com/OpenAgentsInc/openagents/issues/3193) | Closed | Define backend-neutral local runtime error taxonomy and desktop-facing diagnostics | `psionic-serve`, `psionic-provider`, `psionic-runtime` | Landed in `74ebe5cf9`: runtime-owned diagnostics taxonomy with stable error codes plus HTTP-style status/message/context, serve-layer mappings from current request failures into that taxonomy, streaming-terminal diagnostics, and provider receipts preserving structured diagnostics alongside plain-text reasons. |
| `PSI-137` | [#3194](https://github.com/OpenAgentsInc/openagents/issues/3194) | Closed | Add explicit backend fallback, refusal, and degraded-state policy for served products | `psionic-runtime`, `psionic-provider`, backend crates | Landed in `b91fe2c4d`: explicit served-product unavailable/degraded policy enums, direct / same-backend-degraded / cross-backend-fallback state in `psionic-runtime`, provider capability truth that carries those fields instead of a plain string alone, and CPU/Metal backend selection/tests that make fallback and degraded execution machine-checkable. |
| `PSI-138` | [#3195](https://github.com/OpenAgentsInc/openagents/issues/3195) | Closed | Define performance acceptance thresholds and cutover gates for Psionic runtime replacement | `psionic-serve`, `psionic-provider`, backend crates | Landed in `f488763b0`: Ollama-aligned generation/embeddings timing metrics in Psionic responses and receipts, conformance observations that retain that evidence instead of dropping it, ratio-based default performance thresholds, and `ConformanceReport::cutover_ready_with_performance(...)` for explicit semantic-plus-performance cutover gating. |
| `PSI-139` | [#3196](https://github.com/OpenAgentsInc/openagents/issues/3196) | Closed | Decide and document LoRA/adapter support policy for the Ollama replacement boundary | `psionic-models`, `psionic-catalog`, `psionic-serve` | Landed in `cbfc30a6d`: explicit manifest adapter-policy helpers in `psionic-catalog`, explicit refusal of adapter-bearing Ollama manifests in `psionic-models`, and `psionic-serve` show-policy facts plus conformance handling for extra Psionic-only evidence. |
| `PSI-157` | [#3197](https://github.com/OpenAgentsInc/openagents/issues/3197) | Closed | Add backend allocator pooling, bounded kernel caches, and device-memory-budget reporting | `psionic-runtime`, backend crates, `psionic-provider` | Landed in `d54614807`: explicit runtime-resource truth in `psionic-runtime`, exact-spec allocator pooling for CPU and Metal intermediate buffers, bounded kernel-cache reporting, device-memory-budget reporting, and provider-visible serialization/tests. |
| `PSI-158` | [#3198](https://github.com/OpenAgentsInc/openagents/issues/3198) | Closed | Add a fused/custom-op extension surface for backend-specific attention, quantized GEMM, RoPE, and normalization kernels | `psionic-compiler`, `psionic-runtime`, backend crates | Landed in `1f6c6e9fe`: typed backend-extension ops in the graph/plan layer, explicit backend-extension capability truth on `BackendSelection`, CPU reference execution for those families, and a path for later backend-specialized kernels without polluting the primitive-op surface. |
| `PSI-159` | [#3199](https://github.com/OpenAgentsInc/openagents/issues/3199) | Closed | Add local runtime observability for warm/cold transitions, active sessions, queue depth, memory footprint, and backend health changes | `psionic-serve`, `psionic-provider`, `psionic-runtime` | Landed in `a7b73314f`: reusable runtime observability types and health tracking in `psionic-runtime`, lifecycle transition tracking plus a managed-runtime `observability()` surface in `psionic-serve`, and provider-facing observability envelope serialization/tests. |

### Epic D: Quantized execution and accelerated backends after the merged baseline

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-156` | [#3170](https://github.com/OpenAgentsInc/openagents/issues/3170) | Closed | Add backend-specific quantized execution kernels and parity coverage for supported GGUF quant families | `psionic-compiler`, `psionic-runtime`, backend crates, `psionic-serve` | Landed in `d3329e658`: first-class quantized GGML/GGUF constants, Candle-aligned row-wise quantized matmul semantics, CPU-native `Q4_0` / `Q4_1` / `Q8_0` kernels over preserved blocks, explicit CPU quantized capability truth, and explicit Metal quantized-constant refusal. |
| `PSI-130` | [#3200](https://github.com/OpenAgentsInc/openagents/issues/3200) | Closed | Add Metal lowering/kernel coverage for the minimum text-generation primitive set | `psionic-backend-metal`, `psionic-compiler`, `psionic-runtime` | Landed in `5d775fd13`: a distinct Metal text-generation dense-surface contract, shared dense pipeline/kernel-cache naming, explicit Metal-versus-CPU selection/fallback coverage, and direct Metal execution coverage for the current text-generation matmul/add graph. |
| `PSI-131` | [#3201](https://github.com/OpenAgentsInc/openagents/issues/3201) | Closed | Add CPU-vs-Metal parity coverage for the supported text-generation product path | `psionic-backend-metal`, `psionic-serve` | Landed in `ef1c503fa`: a dedicated `metal_text_generation_parity` integration test, seeded exact token/text/termination parity against the CPU reference path, policy-backed hidden/logit drift checks, and macOS-target Metal import fixes found by cross-target compilation. |
| `PSI-132` | [#3202](https://github.com/OpenAgentsInc/openagents/issues/3202) | Closed | Ship a tested Metal-backed `psionic.text_generation` path | `psionic-backend-metal`, `psionic-serve`, `psionic-provider` | Landed in `7466a16b1`: a real `MetalModelTextGenerationService`, shared CPU/Metal non-streaming generation flow with truthful session/prefix-cache/provenance behavior, explicit Metal diagnostics, and provider-facing success/unavailability coverage. |
| `PSI-140` | [#3203](https://github.com/OpenAgentsInc/openagents/issues/3203) | Closed | Psionic phase 5: NVIDIA backend architecture and truthful capability surfaces | `psionic-backend-cuda` or equivalent, `psionic-runtime`, `psionic-provider` | Landed in `c10e32dbf`: new `psionic-backend-cuda` architecture crate, first-class `DeviceKind::Cuda`, runtime/provider backend-selection truth for `cuda`, and an explicit architecture-only offline state before NVIDIA discovery or execution. |
| `PSI-141` | [#3204](https://github.com/OpenAgentsInc/openagents/issues/3204) | Closed | Define the Psionic NVIDIA capability, topology, and risk model | `psionic-runtime`, `psionic-provider` | Landed in `c3ff379b1`: reusable NVIDIA topology/risk/recovery metadata in `psionic-runtime`, `nvidia_metadata` on runtime device descriptors, and provider-visible `nvidia` capability/receipt context for later CUDA discovery and routing work. |
| `PSI-142` | [#3205](https://github.com/OpenAgentsInc/openagents/issues/3205) | Closed | Implement NVIDIA discovery and health reporting | `psionic-backend-cuda` | Landed in `c02562325`: real `nvidia-smi`-backed CUDA discovery in `psionic-backend-cuda`, explicit ready/degraded/offline health, runtime device descriptors populated from live NVIDIA query data, and degraded-state truth for display-attached or MIG-partitioned devices. |
| `PSI-143` | [#3206](https://github.com/OpenAgentsInc/openagents/issues/3206) | Closed | Add CUDA allocator, buffer, stream, and command submission substrate | `psionic-backend-cuda`, `psionic-runtime` | Landed in `0ceef490d`: dynamic `libcudart`-backed CUDA buffers and stream submission in `psionic-backend-cuda`, explicit allocator/runtime-resource truth, and end-to-end staged write plus device-to-device copy coverage on the selected NVIDIA device. |
| `PSI-144` | [#3207](https://github.com/OpenAgentsInc/openagents/issues/3207) | Closed | Add CUDA lowering and kernel coverage for the minimum served-product primitive set | `psionic-backend-cuda`, `psionic-compiler` | Landed in `a9a35c44b`: explicit CUDA dense-surface plan validation, `ExecutionBackend` and `compile_and_execute` support, dense CUDA input/constant materialization, Candle-aligned cuBLAS matmul lowering, cuBLAS-backed add coverage, and live end-to-end execution tests. |
| `PSI-145` | [#3208](https://github.com/OpenAgentsInc/openagents/issues/3208) | Closed | Wire NVIDIA backend selection and truthful capability reporting through Psionic | `psionic-runtime`, `psionic-provider`, `psionic-backend-cuda` | Landed in `50a9d3c63`: explicit direct/degraded/fallback CUDA backend-selection surfaces in `psionic-backend-cuda`, plus provider capability/receipt coverage for direct CUDA, same-backend degraded CUDA, and explicit CPU fallback states. |
| `PSI-146` | [#3209](https://github.com/OpenAgentsInc/openagents/issues/3209) | Closed | Add CPU-vs-NVIDIA parity coverage for the first supported served product path | `psionic-backend-cuda`, `psionic-serve` | Landed in `a29f797b4`: a model-backed CUDA embeddings parity test in `psionic-serve`, shared embedding-drift-budget comparison against the CPU baseline, and explicit CPU fallback truth when CUDA is unavailable. |
| `PSI-147` | [#3210](https://github.com/OpenAgentsInc/openagents/issues/3210) | Closed | Ship the first tested NVIDIA-backed served product path | `psionic-backend-cuda`, `psionic-serve`, `psionic-provider` | Landed in `181d1127e`: a real `CudaModelEmbeddingsService`, CUDA-specific embeddings error/diagnostic handling, shared graph execution over the CUDA backend, and integration coverage for successful response/capability/receipt flow or explicit backend unavailability. |
| `PSI-148` | [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232) | Closed | Define and keep a minimum hardware validation matrix for CPU, Apple Silicon, NVIDIA, AMD KFD, and refusal paths | backend crates, `psionic-serve`, test fixtures | Landed in `0a8e3b700`: `psionic.minimum_hardware_validation.v1` in `psionic-runtime`, provider-facing validation references on capability/receipt surfaces, claim coverage for CPU/Metal/CUDA/AMD KFD/refusal lanes, and a documented host-class lab runbook in `crates/psionic/docs/HARDWARE_VALIDATION_MATRIX.md`. |
| `PSI-150` | [#3211](https://github.com/OpenAgentsInc/openagents/issues/3211) | Closed | Psionic phase 6: AMD served-product execution path | `psionic-backend-amd-kfd`, `psionic-backend-amd-userspace`, `psionic-runtime`, `psionic-provider` | Landed in `2b46505f8`: backend-owned AMD staging allocation and explicit fill/copy submission substrate in both AMD backends, explicit allocator/kernel-cache/device-budget truth for AMD substrate paths, explicit CPU fallback helpers, and provider/runtime-resource coverage while served-product execution remains for `PSI-151` through `PSI-154`. |
| `PSI-151` | [#3212](https://github.com/OpenAgentsInc/openagents/issues/3212) | Closed (Not Planned) | Add AMD KFD lowering and kernel coverage for the first supported primitive set | `psionic-backend-amd-kfd`, `psionic-compiler`, `psionic-runtime` | Closed after reprioritizing this host to NVIDIA-only execution and validation; keep the substrate from `PSI-150`, but do not treat KFD lowering as active queued work here. |
| `PSI-152` | [#3213](https://github.com/OpenAgentsInc/openagents/issues/3213) | Closed (Not Planned) | Wire served-product capability gating for AMD KFD separately from AMD userspace | `psionic-provider`, `psionic-runtime`, AMD backend crates | Closed with the same reprioritization; the KFD/userspace split remains historically relevant but is not active follow-on work on this host. |
| `PSI-153` | [#3214](https://github.com/OpenAgentsInc/openagents/issues/3214) | Closed (Not Planned) | Add CPU-vs-AMD KFD parity coverage for the first supported served product path | `psionic-backend-amd-kfd`, `psionic-serve` | Closed with the same reprioritization; do not advertise or test AMD served-product parity on this machine. |
| `PSI-154` | [#3215](https://github.com/OpenAgentsInc/openagents/issues/3215) | Closed (Not Planned) | Ship the first tested AMD KFD-backed served product path and keep AMD userspace explicitly gated | `psionic-backend-amd-kfd`, `psionic-serve`, `psionic-provider` | Closed with the same reprioritization; the active accelerator execution path remains NVIDIA-only on this machine. |

### Epic E: App cutover and long-term boundary

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `OA-200` | [#3216](https://github.com/OpenAgentsInc/openagents/issues/3216) | Closed | Rename `OllamaExecutionMetrics` and `OllamaExecutionProvenance` to backend-neutral names | `apps/autopilot-desktop` | Landed in `3de54fdb4`: app-owned local execution evidence types are now backend-neutral while the current worker implementation remains explicitly Ollama-backed. |
| `OA-201` | [#3217](https://github.com/OpenAgentsInc/openagents/issues/3217) | Closed | Introduce an app-owned `LocalInferenceRuntime` trait and `PsionicRuntimeAdapter` | `apps/autopilot-desktop` | Landed in `9c2d1f9d1`: added an app-owned local inference runtime seam, an Ollama-backed adapter for the current worker, an in-process Psionic adapter, and desktop plumbing that now routes refresh/generate/warm/unload through the trait instead of the concrete worker. |
| `OA-202` | [#3218](https://github.com/OpenAgentsInc/openagents/issues/3218) | Closed | Switch desktop default from external Ollama HTTP calls to the in-process Psionic runtime | `apps/autopilot-desktop` | Landed in `22c25d2f4`: the desktop now instantiates the app-owned local inference seam with the in-process Psionic reference runtime by default, without silently falling back to the external Ollama worker, and a unit test now pins that default. |
| `OA-203` | [#3219](https://github.com/OpenAgentsInc/openagents/issues/3219) | Closed | Remove the external Ollama dependency and clean up provider/UI wording | `apps/autopilot-desktop` | Landed in `b651786e7`: the production desktop now gates the old `ollama_execution` module behind tests, backend-neutral local inference evidence/helpers live under the app-owned runtime seam, and user-facing defaults, health events, and pane wording now say Psionic/local inference instead of implying external Ollama is still the default product path. |
| `PSI-160` | [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220) | Closed | Define in-process vs subprocess isolation policy for Psionic local serving | `psionic-serve`, `psionic-runtime`, backend crates | Landed in `90224ae2e`: reusable local-serving isolation policy, explicit `in_process` crash/reset truth in observability and generation provenance, aggregate runtime isolation reporting, and documented cutover/evidence implications for the no-subprocess decision on current Psionic. |
| `PSI-161` | [#3171](https://github.com/OpenAgentsInc/openagents/issues/3171) | Closed | Define allowed fallback lattice for Psionic served products: refuse, degrade, replan, retry, or same-backend slow path | `psionic-runtime`, `psionic-provider`, `psionic-serve` | Landed in `220286d8a`: reusable backend-neutral fallback lattice types, explicit trigger/action/state truth across runtime/provider/serve, refusal-aware validation mapping, and a documented fallback boundary for cutover/evidence work. |
| `PSI-162` | [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233) | Closed | Define the served-artifact identity and reproducibility tuple for model blob, tokenizer, template, defaults, quantization, and backend/toolchain version | `psionic-models`, `psionic-serve`, `psionic-provider`, `psionic-runtime` | Landed in `0dfeb6023`: added first-class served-artifact identity and backend-toolchain tuples, threaded descriptor-side artifact identity through capabilities/receipts/provenance, and keyed session/prefix/request invalidation to artifact drift instead of display names alone. |
| `PSI-163` | [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234) | Closed | Define cache and persisted-state upgrade invalidation policy for plan caches, kernel caches, paged tensors, and KV state | `psionic-runtime`, `psionic-serve`, `psionic-models`, backend crates | Landed in `60f5831d5`: added reusable cache invalidation policy and cache observations, with explicit scopes/format versions/triggers for plan, kernel, paged-tensor, prefix, and KV caches plus provider/serve evidence surfaces. |
| `PSI-164` | [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235) | Closed | Add model provenance and license gating for locally discovered artifacts and advertised compute-market supply | `psionic-catalog`, `psionic-provider`, `psionic-models` | Landed in `6524685a0`: added explicit local-artifact provenance and declared-license facts in `psionic-catalog` and `psionic-models`, plus provider-side compute-market supply policy, advertise/serve decisions, and structured refusal diagnostics distinct from integrity and unsupported-format failures. |
| `PSI-170` | [#3222](https://github.com/OpenAgentsInc/openagents/issues/3222) | Closed | Define the boundary between Ollama-compat migration support and the long-term Psionic-native model/runtime format | `psionic-models`, `psionic-catalog`, `psionic-serve` | Landed in `279c1763f`: added explicit catalog/ingress/serving/runtime boundary metadata for Ollama migration versus Psionic-native execution, surfaced those facts through `show`-style observations, and documented the boundary in the cutover/evidence contract. |

### Epic F: Compute-market execution substrate beyond Ollama parity

See [CONFORMANCE_AND_EVIDENCE_CONTRACT.md](./CONFORMANCE_AND_EVIDENCE_CONTRACT.md)
for the minimum conformance harness scope and runtime evidence schema that
`PSI-117`, `PSI-171` through `PSI-175`, and `OA-201` / `OA-202` must satisfy.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-171` | [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223) | Closed | Expand Psionic capability surfaces for compute-market inventory, topology, and performance qualifiers | `psionic-provider`, `psionic-runtime` | Landed in `2f07676d8`: added reusable selected-device inventory qualifiers in `psionic-runtime`, explicit compile-vs-probe backend-toolchain truth in `psionic-provider`, and capability/receipt surfacing of stable device, topology-key, memory-class, and performance-class qualifiers. |
| `PSI-172` | [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224) | Closed | Add batch execution posture, queueing policy, and throughput-class capability reporting | `psionic-serve`, `psionic-runtime`, `psionic-provider` | Landed in `232e36c60`: added runtime-owned execution profiles, `psionic-serve` defaults plus observability alignment, and provider capability reporting of batch posture, queue policy, and throughput class for embeddings and text generation. |
| `PSI-173` | [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225) | Closed | Add multi-device and sharded execution planning for supported product paths | `psionic-runtime`, `psionic-compiler`, backend crates, `psionic-provider` | Landed in `e3fff595d`: added explicit `selected_devices` plus `ExecutionTopologyPlan` truth in `psionic-runtime`, topology-aware compiled-plan digests in `psionic-compiler`, provider capability/receipt surfacing of `selected_devices` and `execution_topology`, and contract coverage for the new multi-device/sharded schema. |
| `PSI-174` | [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226) | Closed | Add execution-plan caching, kernel-cache policy, and warm/cold compile-path evidence | `psionic-runtime`, `psionic-compiler`, backend crates, `psionic-provider` | Landed in `ba3a0d1dd`: added execution-plan cache policy/state in `psionic-runtime`, backend-owned plan caching plus compile-path evidence in CPU/Metal/CUDA, `psionic-serve` provenance for embeddings and generation compile paths, and provider receipt surfacing of plan-cache/kernel-cache behavior. |
| `PSI-175` | [#3227](https://github.com/OpenAgentsInc/openagents/issues/3227) | Closed | Extend Psionic runtime evidence with compute-market delivery-proof fields and settlement-linkage inputs | `psionic-serve`, `psionic-provider`, `psionic-runtime` | Landed in `caff38666`: runtime metrics, serve provenance, and provider receipts now carry direct delivery-proof and settlement-linkage inputs instead of app-local reconstruction. |
| `PSI-176` | [#3228](https://github.com/OpenAgentsInc/openagents/issues/3228) | Closed | Define a reusable Psionic execution-profile model for bounded `sandbox_execution` | `psionic-runtime`, `psionic-provider` | Landed in `75521fbef`: added runtime-owned `SandboxExecutionCapabilityProfile` with explicit isolation, filesystem, network, process, resource, and accelerator-access bounds plus stable profile digests, and a provider-facing `SandboxExecutionCapabilityEnvelope` so future sandbox supply can advertise bounded execution policy without hiding behind app-local defaults. |
| `PSI-177` | [#3229](https://github.com/OpenAgentsInc/openagents/issues/3229) | Closed | Add reusable sandbox-execution receipt and evidence contracts compatible with compute-market supply | `psionic-runtime`, `psionic-provider` | Landed in `4fc690916`: added runtime-owned sandbox request identity, evidence, exit, and resource-summary contracts with explicit command/environment/input/output digests plus optional execution delivery proof, and a provider-facing `SandboxExecutionReceipt` with deterministic request digests and failure/diagnostic mapping for compute-market supply. |
| `PSI-178` | [#3230](https://github.com/OpenAgentsInc/openagents/issues/3230) | Closed | Add topology-aware substitution and deliverability checks for accelerator-sensitive compute offers | `psionic-provider`, `psionic-runtime` | Landed in `707072ba5`: added reusable promised accelerator requirements, delivered execution contexts, and exact/compatible-substitution/underdelivered reports in `psionic-runtime`, plus provider receipt support so accelerator-sensitive sandbox offers can surface machine-checkable promised-versus-delivered topology and capability differences. |

### Epic G: GPT-OSS Psionic-only completion track

This epic existed because the generic Psionic cutover had landed, but the concrete
`gpt-oss-20b-mxfp4.gguf` inference path on this NVIDIA host had only been
proven via external `~/code/llama.cpp`. That gap is now closed on `main`:
`llama.cpp` remains a reference and behavior oracle only, while the shipped
execution path is Psionic-owned end to end.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-179` | [#3239](https://github.com/OpenAgentsInc/openagents/issues/3239) | Closed | Add GPT-OSS / OpenAI-MoE GGUF loading and truthful MXFP4/Q8_0 storage | `psionic-core`, `psionic-models`, `psionic-runtime` | Landed in `780479e23`: Psionic now accepts `general.architecture = gpt-oss`, reconstructs the OpenAI-MoE metadata/tensor layout, recognizes GGUF `MXFP4`, and preserves mixed `MXFP4` / `Q8_0` bundle truth without faking unsupported execution support. |
| `PSI-180` | [#3240](https://github.com/OpenAgentsInc/openagents/issues/3240) | Closed | Implement Harmony prompt rendering and GPT-OSS channel parsing | `psionic-models`, `psionic-serve`, `psionic-runtime` | Landed in `1140c7f32`: Psionic now renders GPT-OSS prompts through the published Harmony Rust crate while preserving Psionic-owned prompt/context/message truth, ships real GPT-OSS golden render fixtures from the local GGUF, parses Harmony output from text or token lanes (including streaming), and can carry parsed Harmony structure on served responses without dropping the raw token/text lane. |
| `PSI-181` | [#3237](https://github.com/OpenAgentsInc/openagents/issues/3237) | Closed | Add a real GGUF-backed decoder execution model for GPT-OSS in Psionic | `psionic-models`, `psionic-compiler`, `psionic-runtime`, `psionic-serve` | Landed in `4821bf07c`: Psionic now executes the real GPT-OSS/OpenAI-MoE decoder path from GGUF-backed weights, with truthful Harmony tokenization, KV-cache/session integration, grouped-KV attention, routed MoE, and explicit refusal semantics instead of routing real GPT-OSS models through the toy fixture decoder. |
| `PSI-182` | [#3238](https://github.com/OpenAgentsInc/openagents/issues/3238) | Closed | Add NVIDIA text-generation kernel coverage for the real GPT-OSS decoder path | `psionic-backend-cuda`, `psionic-compiler`, `psionic-runtime`, `psionic-serve` | Landed in `4821bf07c`: `psionic-backend-cuda` now ships Psionic-owned `Q8_0` / `MXFP4` quantized matvec kernels plus backend-owned quantized byte uploads, and `psionic-serve` now exposes a truthful CUDA GPT-OSS generation path instead of advertising embeddings-only CUDA support. |
| `PSI-183` | [#3241](https://github.com/OpenAgentsInc/openagents/issues/3241) | Closed | Ship and validate a Psionic-only GPT-OSS 20B inference flow on the NVIDIA host | `psionic-serve`, `psionic-provider`, `psionic-runtime`, docs/tests | Landed in `4821bf07c`: the local `gpt-oss-20b-mxfp4.gguf` now serves through the Psionic-owned `psionic-gpt-oss-server` OpenAI-compatible HTTP surface on NVIDIA, and the exact local `/v1/chat/completions` flow returns `323` for the `17 * 19` validation request without delegating inference to `llama.cpp`. |

### Epic H: GPT-OSS throughput parity track

This epic exists because "Psionic can run GPT-OSS correctly" and "Psionic can run
GPT-OSS in the same speed class as `llama.cpp`" are different milestones. The
functional Psionic-only NVIDIA path is landed. The remaining work is to close the
throughput gap honestly on the same OpenAI-compatible HTTP flow, without
delegating execution to `llama.cpp` and without taking prompt/render/parser
shortcuts.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `GPT-OSS-PERF-1` | [#3242](https://github.com/OpenAgentsInc/openagents/issues/3242) | Closed | Add GPT-OSS decode instrumentation and parity benchmark evidence | `psionic-serve`, `psionic-backend-cuda`, docs/scripts | Landed on `main`: benchmark JSON summaries, request-level perf receipts, and the current auditable benchmark contract now exist for the exact Psionic-vs-llama.cpp GPT-OSS flow. |
| `GPT-OSS-PERF-2` | [#3243](https://github.com/OpenAgentsInc/openagents/issues/3243) | Closed | Keep GPT-OSS CUDA activations resident and remove per-matvec round-trips | `psionic-backend-cuda`, `psionic-serve` | Landed on `main`: device-resident decode-step buffers exist, per-matvec host `Vec<f32>` round-trips are gone from the hot path, and decode now works over reusable CUDA buffers instead of isolated upload/compute/readback calls. |
| `GPT-OSS-PERF-3` | [#3244](https://github.com/OpenAgentsInc/openagents/issues/3244) | Closed | Move GPT-OSS KV, RMSNorm, RoPE, and attention onto CUDA | `psionic-backend-cuda`, `psionic-serve` | Landed on `main`: the CUDA lane now owns KV mirrors, RMSNorm, RoPE, and decode attention instead of routing those stages through the old CPU hot path. |
| `GPT-OSS-PERF-4` | [#3245](https://github.com/OpenAgentsInc/openagents/issues/3245) | Closed | Replace GPT-OSS host-side MoE routing with grouped GPU expert execution substrate | `psionic-backend-cuda`, `psionic-serve` | Landed on `main`: router selection and MoE execution now have a CUDA-backed substrate, though the current real-model MXFP4 expert path still needs kernel-quality work for full parity throughput. |
| `GPT-OSS-PERF-5` | [#3246](https://github.com/OpenAgentsInc/openagents/issues/3246) | Closed | Add graph-based GPT-OSS prefill and decode runtime on CUDA | `psionic-serve`, `psionic-runtime`, `psionic-backend-cuda` | Landed on `main`: Psionic now has a reusable GPT-OSS CUDA step-plan/runtime substrate and one-submission-per-token decode shape, but it still does not match `llama.cpp` graph/fusion architecture or CUDA kernel quality. |
| `GPT-OSS-PERF-6A` | [#3249](https://github.com/OpenAgentsInc/openagents/issues/3249) | Closed | Mirror llama.cpp GPT-OSS graph and CUDA fusion architecture | `psionic-serve`, `psionic-runtime`, `psionic-backend-cuda` | Closed after the graph/fusion alignment checkpoint work landed and the remaining bottleneck was narrowed further; keep it here as history, but do not treat it as the next issue. |
| `GPT-OSS-PERF-6` | [#3247](https://github.com/OpenAgentsInc/openagents/issues/3247) | Closed | Port llama.cpp GPT-OSS CUDA kernels and dispatch policy | `psionic-backend-cuda`, `psionic-serve` | Closed as superseded by the narrower execution queue under `#3276`; keep it here as history, but do not treat it as the next issue. |
| `GPT-OSS-PERF-6B` | [#3276](https://github.com/OpenAgentsInc/openagents/issues/3276) | Closed | Reach `150+ tok/s` on the exact GPT-OSS HTTP benchmark via MMID/MMVQ/fattn parity | `psionic-backend-cuda`, `psionic-serve`, docs/audit | Closed after `#3293` landed and pushed the exact prompt-cache-hit lane from the old `122-123 tok/s` floor to `173.19 tok/s` and `171.29 tok/s` on consecutive full-script runs, satisfying the `150+ tok/s` umbrella honestly on this host. |
| `GPT-OSS-PERF-6C` | [#3288](https://github.com/OpenAgentsInc/openagents/issues/3288) | Closed | Port the official GPT-OSS decode-relevant small-token MoE path to CUDA before the dense grouped prefill path | `psionic-backend-cuda`, `psionic-serve` | Closed as superseded by the narrower exact execution chain below. Keep it here as history because it captured the direction shift away from dense grouped-prefill-first work. |
| `GPT-OSS-PERF-6D` | [#3293](https://github.com/OpenAgentsInc/openagents/issues/3293) | Closed | Port llama.cpp delayed-softmax `topk_moe` route for GPT-OSS decode | `psionic-backend-cuda`, `psionic-serve` | Closed after Psionic split router execution into transposed-router matmul, device bias add, and delayed-softmax top-k over router logits. That moved the exact prompt-cache-hit lane into the `171-173 tok/s` class while keeping the benchmark output correct on repeated runs. |
| `GPT-OSS-PERF-6E` | [#3294](https://github.com/OpenAgentsInc/openagents/issues/3294) | Closed | Port llama.cpp `mul_mat_id`-style grouped expert matvec for GPT-OSS decode | `psionic-backend-cuda`, `psionic-serve` | Landed by introducing reusable ids-driven expert-matvec / accumulate submission surfaces in `psionic-backend-cuda`, switching the real GPT-OSS decode lane over to that substrate in `psionic-serve`, and replacing the old per-selected-slot project launch with a grouped project kernel. The exact benchmark stayed in the `170-173 tok/s` class, while short `ncu` sampling showed about `60.4 us` in the gate/up kernel and about `32.7 us` in the grouped project kernel on the kept path. |
| `GPT-OSS-PERF-6F` | [#3295](https://github.com/OpenAgentsInc/openagents/issues/3295) | Closed | Match llama.cpp fused `MUL_MAT_ID (+ADD_ID) + GLU` path for GPT-OSS experts | `psionic-backend-cuda`, `psionic-serve` | Closed as no longer required for the tracked parity contract on this host. After `#3294`, Psionic was already in the same or better throughput class than the current `llama.cpp` control on the exact benchmark, so forcing a riskier fused gate/up rewrite stopped being the next honest blocker. |
| `GPT-OSS-PERF-6G` | [#3296](https://github.com/OpenAgentsInc/openagents/issues/3296) | Closed | Align GPT-OSS decode-attention dispatch with llama.cpp `fattn.cu` | `psionic-backend-cuda`, `psionic-serve`, docs/audit | Closed as no longer required for the tracked parity contract on this host. Sampled decode attention stayed much smaller than the router and expert stages on the kept path, and the benchmark target was already reached before a `fattn.cu` rewrite became the next honest blocker. |
| `GPT-OSS-PERF-7` | [#3248](https://github.com/OpenAgentsInc/openagents/issues/3248) | Closed | Reach llama.cpp-class GPT-OSS throughput on the real Psionic HTTP path | docs/tests/benchmark path plus the serving stack | Closed after the benchmark script was updated to use the explicit system/developer/user contract, force raw-content mode on the `llama.cpp` control, normalize visible output cleanly, and print `prompt_cache_hit_visible_output_match=true`. On that final contract-clean run, Psionic measured `172.84 tok/s` versus `llama.cpp` at `160.98 tok/s` on the exact prompt-cache-hit lane. |

Recent checkpoint note:
The newer graph/fusion and hot-path fixes were real, but the benchmark floor
has now moved again in a way that reproduces. After `#3293`, the exact
prompt-cache-hit lane on this host is in the low `171 tok/s` class on repeated
full-script runs, not the old `122.42 tok/s` floor. The next step is therefore
future headroom work only, before any return to the denser grouped-prefill
path.

Live host-ceiling note:
On this host, the local `llama.cpp` control on the same benchmark script is
currently landing in the `167-169 tok/s` class, but the current script still
lets `llama.cpp` spend completion tokens in Harmony reasoning content instead
of enforcing the exact visible-output contract the way Psionic now does. Treat
that control as a throughput oracle only. Do not claim final parity closure
until both Psionic and the `llama.cpp` control are rerun above the same
contract-clean threshold on the same
benchmark contract.

### Exact `llama.cpp` parity plan for the remaining NVIDIA execution chain

The remaining gap is now concrete enough that the next work should mirror
`llama.cpp` in the same order that its GPT-OSS/OpenAI-MoE CUDA path actually
executes:

1. The `topk_moe` delayed-softmax router path is now landed in `#3293`.
   Reference:
   `~/code/llama.cpp/src/llama-graph.cpp` (`build_moe_ffn(...)`),
   `~/code/llama.cpp/ggml/src/ggml-cuda/ggml-cuda.cu`
   (`ggml_cuda_topk_moe_fusion(...)`, the special GPT-OSS delayed-softmax
   branch), and `~/code/llama.cpp/ggml/src/ggml-cuda/topk-moe.cu`.
   Psionic target:
   [gpt_oss.rs](../psionic-serve/src/gpt_oss.rs),
   [lib.rs](../psionic-backend-cuda/src/lib.rs), and
   [quantized_matvec.cu](../psionic-backend-cuda/src/kernels/quantized_matvec.cu).
   Required change:
   stop treating router32 as a bespoke helper kernel hanging off the serve path.
   Add a reusable backend op that matches `llama.cpp`'s GPT-OSS/OpenAI-MoE
   delayed-softmax contract and emits ids plus weights in one pass, including
   the same tie-breaking and post-top-k softmax behavior.
   Done when:
   the current `router_topk_softmax_32_kernel` path is no longer the primary
   GPT-OSS route, correctness matches the current route exactly, and `ncu`
   shows the router stage materially below the current `~107.7 us` sample.

2. Port ids-driven grouped expert matvec as the primary expert substrate.
   Reference:
   `~/code/llama.cpp/ggml/src/ggml-cuda/ggml-cuda.cu`
   (`ggml_cuda_mul_mat_id(...)`) plus
   `~/code/llama.cpp/ggml/src/ggml-cuda/mmvq.cu`.
   Psionic target:
   [gpt_oss.rs](../psionic-serve/src/gpt_oss.rs),
   [lib.rs](../psionic-backend-cuda/src/lib.rs), and
   [quantized_matvec.cu](../psionic-backend-cuda/src/kernels/quantized_matvec.cu).
   Required change:
   stop making the selected4 gate/up/down path rely on separate Psionic-only
   custom kernels as the primary execution model. Introduce a generic
   ids-enabled quantized expert matvec substrate, with the same small-batch
   MMVQ/MMID fast path and the same sorted-token grouped-expert fallback shape
   that `llama.cpp` uses.
   Done when:
   GPT-OSS selected experts are expressed as ids-driven expert matvecs in the
   backend/runtime surface, not as ad hoc selected4-only orchestration in
   `psionic-serve`, and `ncu` shows the sampled gate/up plus project stages
   materially below the current `~72.6 us` plus `~33.3 us` samples.

3. Match `MUL_MAT_ID (+ADD_ID) + GLU` fusion before revisiting attention.
   Reference:
   `~/code/llama.cpp/src/llama-graph.cpp` (`build_moe_ffn(...)`) and
   `~/code/llama.cpp/ggml/src/ggml-cuda/ggml-cuda.cu`
   (`ggml_cuda_can_fuse(...)`, `ggml_cuda_should_fuse_mul_mat(...)`).
   Psionic target:
   [gpt_oss.rs](../psionic-serve/src/gpt_oss.rs),
   [lib.rs](../psionic-backend-cuda/src/lib.rs), and
   [quantized_matvec.cu](../psionic-backend-cuda/src/kernels/quantized_matvec.cu).
   Required change:
   fuse the OpenAI-MoE gate/up expert loads and SwiGLU in the same structural
   place `llama.cpp` does, instead of keeping the current
   `moe_gate_up_swiglu_q8_1_selected4_quantized_kernel` as a special-case end
   state. If that means packing or re-expressing gate/up expert tensors inside
   Psionic's model/runtime surface, do that explicitly.
   Done when:
   the primary GPT-OSS gate/up path follows the same ids-driven fusion shape as
   `llama.cpp`, and the roadmap no longer needs a separate "selected4 custom
   kernel" story to explain expert execution.

4. Only then align decode-attention dispatch with `fattn.cu`.
   Reference:
   `~/code/llama.cpp/ggml/src/ggml-cuda/fattn.cu`.
   Psionic target:
   [gpt_oss.rs](../psionic-serve/src/gpt_oss.rs),
   [lib.rs](../psionic-backend-cuda/src/lib.rs), and
   [quantized_matvec.cu](../psionic-backend-cuda/src/kernels/quantized_matvec.cu)
   if new attention helpers are needed.
   Required change:
   port the relevant head-dimension, GQA-ratio, and cache-length dispatch rules
   for the GPT-OSS decode contract on this host, but only after router plus
   expert execution stop dominating the `ncu` sample.
   Done when:
   attention dispatch becomes the next measured bottleneck instead of a smaller
   background cost, and the benchmark floor is already above the current
   router/MoE-bound band.

5. Keep the benchmark contract and issue discipline strict.
   Required change:
   every kept checkpoint under `#3276`, `#3293`, `#3294`, `#3295`, `#3296`,
   and `#3248` must log the current Psionic floor, the same-run `llama.cpp`
   control, and the measured before/after kernel samples from `ncu`.
   Done when:
   the roadmap, audit, and issue comments all describe the same benchmark
   contract and the same bottleneck ordering.

### Epic I: Native Rust Metal GPT-OSS completion track

The backend groundwork for Metal GPT-OSS already exists on `main`: `#3250` and
`#3252` through `#3260` landed the quantized storage, grouped expert dispatch,
device-KV/shared-prefix substrate, decode-attention reserve path, bounded
output helpers, allocator/runtime-policy hooks, and the first
`MetalGgufGptOssTextGenerationService`. The remaining work is not "bring up
Metal from zero." It is the gap between those backend pieces and a truthful
end-to-end native Rust Metal serve path that no longer hides behind
`llama.cpp` proxy mode and no longer keeps the decode loop CPU-owned.

Source audit for this remaining queue:
[2026-03-09-metal-gpt-oss-throughput-audit.md](../../../docs/audits/2026-03-09-metal-gpt-oss-throughput-audit.md)

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `METAL-GPT-OSS-1` | [#3270](https://github.com/OpenAgentsInc/openagents/issues/3270) | Open | Make native Rust Metal versus llama.cpp proxy execution explicit | `psionic-serve`, docs/scripts | The current macOS benchmark silently enables proxy mode, so Psionic can publish Apple throughput numbers that are really `llama.cpp` unless this issue lands first. |
| `METAL-GPT-OSS-2` | [#3268](https://github.com/OpenAgentsInc/openagents/issues/3268) | Open | Wire the native Metal GPT-OSS serve path into device KV, shared-prefix, and reserved attention runtime | `psionic-serve`, `psionic-backend-metal`, `psionic-runtime` | The backend already has this substrate, but the shipped Metal GPT-OSS path still uses host `InMemoryKvCache`, host K/V vectors, and CPU `attend_impl(...)`. |
| `METAL-GPT-OSS-3` | [#3269](https://github.com/OpenAgentsInc/openagents/issues/3269) | Open | Move the remaining GPT-OSS decode math and MoE control path off the host on Metal | `psionic-serve`, `psionic-backend-metal` | Even after device-KV and attention are wired, the native Metal decode loop still leaves RMSNorm, RoPE, router selection, softmax, SwiGLU, and expert aggregation on the CPU. |
| `METAL-GPT-OSS-4` | [#3271](https://github.com/OpenAgentsInc/openagents/issues/3271) | Open | Remove per-op waits, readbacks, and transient host vectors from the native Metal decode path | `psionic-serve`, `psionic-backend-metal` | The current `run_quantized_matvec`/`run_grouped_quantized_matvec` usage pattern is still one-op-submit, wait, and readback, which kills throughput even if the kernel math is otherwise correct. |
| `METAL-GPT-OSS-5` | [#3272](https://github.com/OpenAgentsInc/openagents/issues/3272) | Open | Use bounded logits output modes in the native Metal GPT-OSS serve path | `psionic-serve`, `psionic-backend-metal` | The backend already supports bounded logits selection, but the native Metal service still materializes full logits on the host even for greedy and small-candidate requests. |
| `METAL-GPT-OSS-6` | [#3261](https://github.com/OpenAgentsInc/openagents/issues/3261) | Open | Add CPU-vs-Metal GPT-OSS parity, validation, and benchmark evidence | docs/tests/benchmark path plus the serving stack | Keep this open until the native Rust Metal path has seeded correctness tests, explicit refusal tests, and warm plus prompt-cache-hit receipts. |
| `METAL-GPT-OSS-7` | [#3262](https://github.com/OpenAgentsInc/openagents/issues/3262) | Open | Reach same-host llama.cpp-class GPT-OSS throughput on Apple Silicon | docs/tests/benchmark path plus the serving stack | Keep this open until the same-host native Rust Metal path reaches the agreed llama.cpp-relative throughput band on the real benchmark contract. |

## Recommended Order

The shortest honest path from today's `main` is:

1. Metal text generation is now landed via `PSI-130` through `PSI-132`; keep
   that lane green while moving to NVIDIA.
2. Land NVIDIA explicitly via `PSI-140` through `PSI-147` and keep `PSI-148`
   green as backend claims widen.
3. Keep the landed AMD substrate from `PSI-150` as historical groundwork, but
   treat `PSI-151` through `PSI-154` as closed-not-planned on this
   NVIDIA-validated host path.
4. Lock process-isolation, fallback-lattice, served-artifact identity,
   cache-invalidation, provenance, and migration-boundary decisions via
   `PSI-160`, `PSI-161`, `PSI-162`, `PSI-163`, `PSI-164`, and `PSI-170`.
5. Land the cutover contract from
   [CONFORMANCE_AND_EVIDENCE_CONTRACT.md](./CONFORMANCE_AND_EVIDENCE_CONTRACT.md)
   before hardening backend and app cutover work.
6. Add compute-market capability and evidence substrate via `PSI-171` through
   `PSI-175`.
7. Cut the app over via `OA-200` through `OA-203`.
8. Only after inference and embeddings are truthful, consider
   `PSI-176` through `PSI-178` for bounded `sandbox_execution`.
9. The GPT-OSS completion track is now landed on this NVIDIA host:
   `PSI-179` -> `PSI-180` -> `PSI-181` -> `PSI-182` -> `PSI-183` are closed on
   `main`; do not reopen that enablement sequence.
10. The active next work on this host is the GPT-OSS throughput track:
    `#3242` -> `#3243` -> `#3244` -> `#3245` -> `#3246` are closed, `#3249`
    and `#3247` are closed, `#3276` and `#3293` are now closed, `#3288` is
    closed as superseded history, and the current execution order is `#3294`
    then `#3295` then `#3296` then `#3248`.
11. The Apple Silicon native-Rust Metal track is now explicit too: keep the
    landed groundwork from `#3250` and `#3252` through `#3260`, but execute the
    remaining queue in order `#3270` -> `#3268` -> `#3269` -> `#3271` ->
    `#3272` -> `#3261` -> `#3262`, and do not use proxy-mode receipts to close
    any of those issues.

## Definition Of Done For "Replace Ollama"

The external Ollama dependency is not replaced until all of the following are
true:

- Psionic can discover installed models from the local Ollama model store during
  migration
- Psionic can report installed models, loaded models, and model metadata without
  calling the Ollama daemon
- Psionic passes a repeatable Ollama-to-Psionic conformance suite for catalog,
  generation, embeddings, prompt rendering, truncation, stop handling,
  streaming, and error semantics
- Psionic has golden prompt and tokenizer fixtures for supported model families
  sourced from real GGUF or Ollama installs
- Psionic can match or explicitly redefine prompt-template, BOS/EOS, and default
  stop behavior for the supported model families
- Psionic has explicit context-window accounting, truncation, and over-limit refusal
  behavior for generation and embeddings
- Psionic can warm, load, unload, and keep alive a local model lifecycle
- Psionic has explicit paged-KV policy for long-context text generation or explicit
  refusal when that policy is unsupported
- Psionic has explicit shared prompt-prefix cache policy and accounting or explicit
  refusal when shared prefix reuse is unsupported
- Psionic can decide whether a model may load based on memory planning, residency
  policy, and admission control
- Psionic can execute the current text-generation path with the option surface the
  desktop already uses
- Psionic can stream partial output, slow-reader handling, disconnect behavior, and
  cancellation with stable final-chunk semantics
- metrics, receipts, capability surfaces, error taxonomy, and fallback states
  remain truthful
- Psionic exposes a served-artifact identity tuple for model blob, tokenizer, chat
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
  support and the long-term Psionic-native model/runtime format

## Additional Definition Of Done For Psionic As Compute-Market Substrate

Psionic is not yet a credible compute-market substrate until all of the following
are also true:

- Psionic can publish truthful capability-envelope fields for backend family,
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
- if `sandbox_execution` is added later, Psionic exposes a bounded execution
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

## 2026-03-09 GPT-OSS Perf Checkpoint

- Current exact benchmark floor on this NVIDIA host:
  Psionic `prompt_cache_hit = 125.12 tok/s`
- Current same-run control:
  `llama.cpp prompt_cache_hit = 168.53 tok/s`
- This checkpoint fixed an exact-prefix CUDA reuse bug that could pair the
  third exact `HTTPS ...` request with the warm `TLS ...` device KV entry.
- This means the tracked throughput number is now both slightly better and more
  trustworthy than the previous `123.42 tok/s` baseline on `#3276`.

## 2026-03-09 GPT-OSS Exact CUDA Prefix Detachment Checkpoint

- Current truthful exact benchmark floor on this NVIDIA host:
  Psionic `prompt_cache_hit = 124.36 tok/s`
- Current same-run control:
  `llama.cpp prompt_cache_hit = 170.70 tok/s`
- The earlier exact-prefix fix was incomplete because `CudaSharedPrefixStore`
  still aliased device buffers across requests.
- A later non-exact shared-prefix reuse could mutate the stored exact prompt KV
  rows in-place, which is why the real `HTTPS -> TLS -> HTTPS` sequence could
  still return the wrong third answer even while reporting
  `prefix_tokens_reused = 158`.
- The current fix detaches non-exact CUDA shared-prefix hits into a fresh
  writable device KV allocation before prompt-tail append, so the exact stored
  prompt prefix stays truthful.
- At that checkpoint, this appeared to set the floor to beat on `#3276`, but
  the later baseline-correction reruns below superseded it. The underlying
  correctness fix still stands; only the long-term benchmark-floor claim
  changed.

## 2026-03-09 GPT-OSS Router32 CUDA Checkpoint

- Current exact benchmark floor on this NVIDIA host:
  Psionic `prompt_cache_hit = 134.62 tok/s`
- Current same-run control:
  `llama.cpp prompt_cache_hit = 168.81 tok/s`
- This checkpoint keeps a dedicated one-warp CUDA router path for the exact
  GPT-OSS `32`-expert case and keeps the split-router MoE path selected,
  because the larger fused residual+norm+router kernel still loses on this GPU.
- Two nearby branches were measured and rejected:
  `rows_per_block = 16` for the regular shared-input quantized matvec path
  regressed to `132.82 tok/s`, and a `4 rows/block` selected4 MoE down
  aggregate rewrite regressed to `131.79 tok/s`.
- This was later shown to be a transient fast sample, not a durable floor.

## 2026-03-09 GPT-OSS Baseline Correction And Grouped-MoE Direction

- The exact old `41a7b3568` checkpoint was rebuilt in a separate worktree and
  re-benchmarked on the same contract.
- Repeated current-`main` runs and the exact old checkpoint now both cluster in
  the `~123 tok/s` band, with the old checkpoint reproducing at
  `121.79 tok/s`.
- The earlier `134.62 tok/s` reading is therefore not a durable source-level
  baseline and should not be treated as the floor to beat on `#3276`.
- The next concrete direction was first captured in `#3288`, but that issue is
  now closed as superseded by the narrower exact execution chain:
  `#3293` router delayed-softmax `topk_moe`, `#3294` ids-driven grouped expert
  matvec, `#3295` fused gate/up plus GLU, then `#3296` `fattn.cu`-aligned
  attention dispatch.
- Only after those decode-critical stages improve should the work return to
  the denser grouped routing-metadata / scatter / gather path, with `#3276`
  kept as the benchmark umbrella and `#3248` still left open as the final
  throughput target.

## 2026-03-10 GPT-OSS NCU Decode Hotspot Checkpoint

- Current truthful exact benchmark floor on this NVIDIA host:
  Psionic `prompt_cache_hit = 122.42 tok/s`
- Current same-run control:
  `llama.cpp prompt_cache_hit = 166.32 tok/s`
- Short `ncu` sampling on the exact live Psionic server path now gives a more
  concrete hotspot split for the decode contract on this host:
  `router_topk_softmax_32_kernel` is about `107.7 us`,
  `moe_gate_up_swiglu_q8_1_selected4_quantized_kernel` is about `72.6 us`,
  `expert_mul_mat_vec_q8_1_project_kernel` is about `33.3 us`,
  `attention_decode_rope_cache_f16_kv_q8_1_kernel` is about `4.7 us`,
  `quantize_q8_1_rows_kernel` is about `2.3 us`, and
  `accumulate_selected4_kernel` is about `2.4 us`.
- This rules out the current generic substitution ideas as the next step on
  this GPU. The following branches were re-tested on the exact benchmark and
  reverted after regressing it:
  row-per-warp selected4 down-project, fused `f32 -> q8_1` expert-input
  quantization inside selected4 gate/up, a `4`-warp / `8`-rows-per-warp
  selected4 gate/up launch, enabling the fused
  `add_residual_rms_norm_q8_1_router_topk` path on real GPT-OSS decode, and
  forcing the exact `32 x 2880` router shape onto the generic 256-thread
  router kernel.
- Recommended next step: execute the new exact chain in the same order
  `llama.cpp` does: `#3293` delayed-softmax `topk_moe`, `#3294` ids-driven
  grouped expert matvec (`mul_mat_id` plus MMVQ/MMID), `#3295` fused gate/up
  plus GLU, then `#3296` `fattn.cu`-aligned attention dispatch. Use `ncu`
  measurements as the acceptance guide; this checkpoint remains historically
  correct, but `#3293` has since landed and `#3276` has since been closed.

## 2026-03-10 GPT-OSS Router-Split Checkpoint

- Landed `#3293` by splitting GPT-OSS router execution on CUDA into:
  transposed-router dense matmul, device bias add, and delayed-softmax top-k
  over the precomputed router logits.
- Concrete Psionic changes:
  added a per-layer router-logits scratch buffer in the CUDA decode-step plan,
  uploaded a transposed CUDA copy of each router matrix at model load, added a
  reusable CUDA delayed-softmax top-k op over logits, and rewired the real
  GPT-OSS decode path to use that split route instead of the old bespoke fused
  `router_topk_softmax_32_kernel`.
- Verification:
  a new CUDA backend test now proves the split route matches the old fused
  router helper exactly on selected ids and routing weights, and the
  `openai_http` suite still passes.
- New truthful exact benchmark floor on this NVIDIA host:
  Psionic `prompt_cache_hit = 173.19 tok/s` and `171.29 tok/s` on consecutive
  full-script runs with the exact one-sentence output.
- Same-script control status:
  `llama.cpp` still lands in the `167-169 tok/s` class on this host, but the
  current script allows it to spend completion tokens in Harmony
  `reasoning_content`, so treat that as a throughput reference and not as a
  contract-clean visible-output comparison.
- Next step:
  move directly to `#3294` and port the remaining `llama.cpp`-style
  ids-driven grouped expert matvec path now that the router split is no longer
  the limiting structural gap.

## 2026-03-10 GPT-OSS Grouped-Expert Matvec Checkpoint

- Landed `#3294` by moving the real GPT-OSS decode lane onto a reusable
  ids-driven expert-matvec backend surface instead of keeping the expert down
  projection explained only as a selected4-specific serve path.
- Concrete Psionic changes:
  `psionic-backend-cuda` now exposes ids-driven expert matvec and accumulate
  submission surfaces, and the selected4 down-project CUDA path now launches a
  grouped project kernel that computes the selected experts together instead of
  launching one project kernel per selected slot. `psionic-serve` now routes
  the real GPT-OSS decode lane through those ids-driven backend calls.
- Verification:
  backend parity tests for the ids-driven gate/up and expert-matvec paths
  passed, `openai_http` still passed, and the release benchmark script still
  returned the exact expected one-sentence output for Psionic.
- New truthful exact benchmark floor on this NVIDIA host:
  Psionic `prompt_cache_hit = 173.05 tok/s` and `170.05 tok/s` on consecutive
  full-script runs after the `#3294` landing, which keeps the path in the same
  `170-173 tok/s` class already opened by `#3293`.
- Short `ncu` sample on the kept path:
  `moe_gate_up_swiglu_q8_1_selected4_quantized_kernel` averaged about
  `60.4 us`, and
  `expert_mul_mat_vec_q8_1_project_grouped_kernel` averaged about `32.7 us`.
  Relative to the older decode-hotspot checkpoint, that keeps the project
  stage roughly flat while pulling the sampled gate/up cost down materially.
- Next step:
  `#3295` was kept open only long enough to confirm it was no longer an honest
  blocker. After `#3294`, Psionic was already in the same or better throughput
  class than the current `llama.cpp` control on this host, so the active next
  step is now only `#3296` if the final parity-contract cleanup still needs
  attention-dispatch work at all.

## 2026-03-10 Fused-Gate-Up Issue Closure

- Closed `#3295` without landing a new CUDA rewrite.
- Reason:
  after `#3294`, the exact full-script benchmark already had Psionic at
  `173.05 tok/s` and `170.05 tok/s` on the `prompt_cache_hit` lane, with the
  same-script `llama.cpp` control at `166.80 tok/s` and `167.89 tok/s`.
- Decision:
  the fused ids-driven gate/up `+ GLU` path remains a plausible future cleanup,
  but it is no longer the next honest blocker for the tracked parity contract
  on this host. Closing it avoids risky churn after the throughput target has
  already been met.

## 2026-03-10 Attention-Dispatch Issue Closure

- Closed `#3296` without landing a new attention kernel rewrite.
- Reason:
  the kept short decode-hotspot evidence still had attention far below the
  router and expert stages on this host, and the exact benchmark target was
  already met before a `fattn.cu`-style rewrite became the next honest
  blocker.
- Decision:
  keep `fattn.cu` alignment as future headroom work, but stop treating it as a
  required dependency for the current Psionic-versus-`llama.cpp` benchmark
  contract. The only remaining open issue in this NVIDIA queue is now `#3248`,
  which is about the final contract-clean parity closeout, not another forced
  kernel port.

## 2026-03-10 Contract-Clean Parity Closure

- Closed `#3248`.
- Concrete benchmark-script changes:
  `crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh` now uses the explicit
  system/developer/user request contract from the manual GPT-OSS flow, sends
  `reasoning_format: "none"` to the `llama.cpp` control, normalizes visible
  output by stripping reasoning wrappers, records `visible_output` in the JSON
  summaries, and prints `prompt_cache_hit_visible_output_match=true` when both
  servers expose the same final sentence.
- Final contract-clean run on this host:
  Psionic `prompt_cache_hit = 172.84 tok/s`
  `llama.cpp prompt_cache_hit = 160.98 tok/s`
  `prompt_cache_hit_visible_output_match=true`
- Result:
  the NVIDIA GPT-OSS parity queue is closed honestly on this host. Any future
  router/MoE/attention work should be treated as headroom or portability work,
  not as unfinished closure for the original parity track.

## 2026-03-10 GPT-OSS 120B Hybrid Decode Checkpoint

- Closed `#3338` after the registered-host-buffer experiments lost on this
  host, and opened `#3345` as the new active NVIDIA throughput issue for the
  local `/home/christopherdavid/models/gpt-oss/gpt-oss-120b-mxfp4.gguf` path.
- Current truthful 120B floor on the exact cold / warm-non-hit /
  prompt-cache-hit contract on the current kept branch:
  Psionic `2.24-2.30 tok/s`, `6.43-6.64 tok/s`, and `10.41-10.75 tok/s`.
- Current kept implementation direction:
  the hybrid 120B path already keeps more of feed-forward prep and decode
  attention on CUDA, trims single-expert cache repacking, and reuses hybrid
  selected-expert layer caches where that actually wins.
- Newly landed on the kept branch:
  generation-only decode steps can now keep the hidden state on CUDA across
  dense attention/router plus staged selected4 accumulation, instead of
  reading both the FFN residual and the MoE output back to host before the
  next CUDA-capable substep.
- Newest kept follow-up on the same branch:
  stateless no-session generation now skips host-KV materialization entirely
  when the hybrid CUDA device-argmax fast path is available, so the 120B path
  no longer reads each generated token's KV entry back to host just to serve a
  stateless request.
- Benchmark correction after fresh clean reruns:
  the earlier `~10 tok/s`-across-all-three-lanes readout was not reproducible
  on the clean branch. Repeated reruns after rebuilding the release binary
  restored the older `2 / 6 / 10 tok/s` shape, with prompt-cache-hit at about
  `10.44 tok/s`, so do not treat the earlier `10.23-10.34 tok/s` band as the
  truthful floor.
- Newest kept profiling checkpoint:
  the added hybrid selected4 cache counters proved the 120B prompt-hit lane
  was still restaging about `52 GB` of expert weights per request, and that
  the old "expand the last 15 layers" cache heuristic was misallocating scarce
  cache slots. The kept branch now uses a profiled 120B-specific expanded-slot
  layer set (`10, 12, 18, 21, 22, 23, 25, 26, 28, 29, 31, 32, 33, 34, 35`),
  which moved the truthful prompt-cache-hit floor to `10.50 tok/s` on the
  exact contract.
- Ruled-out nearby branches:
  cache-shape probes with `7` expanded slots on the last `4` layers and `6`
  expanded slots on the last `8` layers were both slower than the kept branch,
  full CUDA-visible duplicated host-weight experiments were either OOM or
  materially slower, direct selected4 execution from CUDA-registered host GGUF
  pages regressed prompt-cache-hit into the `6.4 tok/s` class, registered-host
  cache-fill copies also stayed in the `6.4 tok/s` class, and a follow-up
  branch that removed one mid-layer readback was effectively flat at
  `10.07-10.09 tok/s`.
- Next honest step:
  keep pushing on the remaining host-to-device selected4 expert staging inside
  that hybrid lane: the hidden update itself now stays device-resident and the
  stateless host-KV readback is gone, and the cache-slot profile is now guided
  by real per-layer miss data, but the hot path still spends most of its time
  restaging selected experts from host-backed MoE storage into CUDA caches.
  The next likely wins are therefore still in reducing or restructuring that
  surviving selected-expert staging traffic.
- New timing evidence on the kept branch:
  the debug-enabled prompt-cache-hit trace showed `step_wall_ns` around
  `8.76 s` for `49` generated tokens, while the timed kernel buckets only
  covered about `0.86 s` total. That means the remaining 120B wall time is
  still dominated by work outside the timed kernels, and the selected4
  cache-fill path remains the main suspect.
- Newly ruled-out follow-ups after that trace:
  an LFU/LRU mixed selected4 eviction policy regressed to about `10.32 tok/s`,
  a reprofiled sixth-slot layer set regressed to about `10.41 tok/s`, a
  memory-neutral `7/6/4` hot/mid/cold slot rebalance regressed to about
  `10.35 tok/s`, and a pinned-host async region-copy rewrite of the decode
  cache-fill path cratered the cold/warm lanes while leaving prompt-cache-hit
  effectively flat at about `10.30 tok/s`. Two newer nearby branches are now
  ruled out too: a more concentrated memory-neutral `8/6/5` hot-layer slot
  skew still stayed below the kept branch at about `10.44 tok/s`, and fully
  bypassing selected4 caches on the historically low-hit 120B layers cratered
  the exact-contract floor to about `1.64 tok/s` cold, `4.68 tok/s`
  warm-non-hit, and `7.43 tok/s` prompt-cache-hit.
- Updated direction for `#3345`:
  do not spend another cycle on small cache-shape or scratch-copy tweaks.
  The remaining honest path is to reduce or fundamentally restructure
  selected-expert staging itself.
- New exact `nsys` checkpoint on the same kept branch:
  a real cold / warm-non-hit / prompt-cache-hit capture on
  `/tmp/psionic_120b_nsys.nsys-rep` showed `190,411` host-to-device copies
  totaling about `333 GB`, with `cudaMemcpy` alone consuming about
  `25.09 s` of CUDA API time and `cudaStreamSynchronize` another `4.70 s`.
  The dominant host-to-device copy size was `4,406,400` bytes, repeated
  `71,678` times. That matches the staged selected-expert weight path, so the
  next honest 120B work should stay pinned to reducing those large repeated
  selected4 uploads rather than retuning kernel math.
- More ruled-out follow-ups after that capture:
  an async host-region rewrite of the decode-lane selected4 cache-fill path
  compiled and passed targeted tests but still lost on the real benchmark at
  about `1.65 / 5.53 / 10.46 tok/s`, a direct top-miss sixth-slot reallocation
  regressed to about `1.65 / 5.51 / 10.39 tok/s`, a narrower
  ratio-guided expanded-layer swap regressed to about
  `2.23 / 6.41 / 10.45 tok/s`, and a more aggressive `0/5/6/7` slot-skew
  layout failed on the first cold request and was reverted immediately.
- Newest kept cache-layout follow-up after those dead ends:
  a first narrow hot-layer bump kept `7` selected4 cache slots on layers
  `23, 25, 28, 29`, while the earlier profiled expanded set stayed at `6`
  slots and the remaining hybrid layers stayed at `5`. On the exact contract,
  that nudged the kept 120B measurement to about
  `2.26 / 6.43 / 10.55 tok/s`.
- Newest kept cache-layout follow-up after that:
  keeping `8` selected4 cache slots on the proven hot layers
  `23, 25, 28, 29`, keeping `6` slots only on `10, 18, 21, 22, 26, 31, 33`,
  and leaving the remaining hybrid layers at `5` produced one fast
  `2.26 / 6.51 / 10.57 tok/s` sample on the exact contract.
- Corrected reproducible 120B floor after fresh reruns on the same kept code:
  the `10.57 tok/s` hit was not a durable floor. Fresh reruns after the later
  dead-end branches keep clustering in the `10.48-10.50 tok/s` class, so the
  truthful reproducible checkpoint for the current pushed branch should still
  be treated as about `2.25 / 6.45-6.50 / 10.50 tok/s`.
- Newest kept 120B checkpoint after adding truthful per-layer cache telemetry:
  one prompt-cache-hit debug trace showed the last hybrid MoE layer was still
  running without any cache at all. Its per-layer staged bytes were about
  `10.34 GB` with zero cache hits or misses, which means that layer had fallen
  off the cache plan and was restaging every selected expert through the
  no-cache path. The kept fix rebalanced the cache map so layers
  `14, 15, 19, 20, 32` drop to `4` slots, layers `23, 25, 28, 29` stay at
  `8`, layers `10, 18, 21, 22, 26, 31, 33` stay at `6`, and layer `35`
  regains a real `5`-slot cache. On the exact contract that moved the current
  kept floor to about `2.30 / 6.64 / 10.75 tok/s`. The matching debug rerun
  dropped layer `35` staged bytes to about `2.03 GB` and restored real cache
  activity there (`43` hits / `153` misses).
- More ruled-out 120B follow-ups after that:
  an exact-prompt hybrid selected4 template-restore branch came back at about
  `2.25 / 6.40 / 10.48 tok/s`, several more memory-neutral prompt-hit-driven
  slot redistributions landed in the `10.31-10.50 tok/s` band, a
  previous-request protected-expert eviction policy landed around
  `2.26 / 6.40 / 10.42 tok/s`, and simply shrinking the benchmark context to
  `512` was effectively flat at about `2.25 / 6.48 / 10.48 tok/s`.
- Updated next honest step after the tail-layer cache restore:
  keep the next branch pinned to reducing selected-expert staging itself, not
  another blind slot shuffle. The tail-layer fix proves there was still real
  cache-allocation waste to recover, but the surviving wall is still the same
  host-backed MoE staging path tracked in `#3360`: eliminate one of the large
  selected4 upload legs, starting with the full host-backed down tensor, before
  revisiting broader gate/up changes.
