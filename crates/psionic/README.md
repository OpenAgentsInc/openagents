# Psionic

Psionic is the reusable Rust-native compute execution subtree for OpenAgents.

It owns the machine-facing side of the stack: tensor and graph contracts,
compiler/runtime boundaries, backend truth, artifact staging, cluster and
sandbox execution, serving interfaces, adapter packaging, evaluation, research,
and the early training substrate.

It intentionally lives under `crates/psionic/` so the engine can evolve without
bleeding product behavior into `apps/*` or authority logic into kernel and
Nexus surfaces.

## Doc Authority

- `README.md` is the Psionic entrypoint and map.
- `docs/ARCHITECTURE.md` is the canonical Psionic-wide system spec.
- `docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md` is the canonical framework-core
  completion bar for tensor, compiler, IO, replay, and local multi-device
  behavior.
- `docs/INFERENCE_ENGINE.md` is the canonical inference-engine completion doc.
- `docs/TRAIN_SYSTEM.md` is the canonical training subsystem spec.
- research audits explain direction and rationale, but they are not the
  authoritative current-state spec.

## What Psionic is

- The reusable execution substrate beneath OpenAgents provider and compute
  products.
- A Rust-native crate family for framework core, backends, transport,
  clustered execution, serving, adapters, data, eval, training, and research.
- The source of machine-legible execution truth: manifests, receipts, routing
  facts, cache facts, proof bundles, topology state, and training/eval
  lineage.
- The layer that can turn backend/runtime reality into truthful provider
  capabilities without owning desktop UX, market procurement, or settlement
  authority.

## What Psionic is not

- Not `apps/autopilot-desktop`, Mission Control, wallet/payout logic, or
  buyer/provider product orchestration.
- Not kernel/Nexus authority for compute-market truth, settlement, or accepted
  outcomes.
- Not a shortcut around `docs/OWNERSHIP.md`.
- Not a claim that every backend, model family, serving topology, or
  training-class lane is fully productized.
- Not a hidden Python control plane disguised as Rust crates.

## Tassadar Executor Lane

Psionic now has an implemented-early executor-class reference lane codenamed
`Tassadar`.

Current posture:

- it lives under `crates/psionic/*`, not in app code and not in kernel or
  Nexus authority
- it is WebAssembly-first and CPU-reference-first
- it is intended to give larger reasoning systems inner exact-computation
  ability
- its Phase 1 reference substrate now exists in `psionic-runtime` and
  `psionic-models`
- its Phase 2 artifact/compatibility contract now exists as digest-bound
  program artifacts plus explicit executor compatibility descriptors
- its Phase 3 benchmark/environment package layer now exists in
  `psionic-environments` and `psionic-eval`
- its Phase 4 proof/lineage layer now exists in `psionic-runtime`, with
  emitted trace artifacts, runtime-manifest lineage, and canonical proof-bundle
  integration
- its Phase 5 fast path now exists in `psionic-runtime` and `psionic-eval`,
  with explicit `HullCache` decode identity, exact CPU/linear/hull equivalence
  checks, typed refusal for backward-branch workloads outside the first
  validated subset, and benchmark reporting for hull-cache throughput, speedup
  over linear decode, and remaining gap vs direct CPU
- its Phase 6 runtime truth now exists in `psionic-runtime`, `psionic-models`,
  and `psionic-eval`, with a machine-legible capability report plus explicit
  direct/fallback/refused decode selection diagnostics for hull-cache,
  approximate sparse-top-k fallback, unsupported ABI/profile requests, and
  model-effective decode mismatches
- its Phase 7A served product surface now exists in `psionic-serve`, with an
  explicit `psionic.executor_trace` request/stream/terminal contract, typed
  refusal responses, trace-step streaming, final output extraction helpers, and
  served evidence bundles that preserve decode selection, trace proof, and
  runtime-manifest lineage
- its Phase 7B widened executor envelope now exists in `psionic-runtime`,
  `psionic-models`, and `psionic-eval`, with the `core_i32_v2` Wasm profile,
  profile-aware runner construction, and article-class exact benchmark
  coverage for `MicroWasmKernel`, `SudokuClass`, and `HungarianMatching`
- the first trained-executor follow-on bar now also exists in
  `psionic-runtime` and `psionic-models`: a dedicated
  `tassadar.wasm.sudoku_v0_search.v1` profile plus a real 4x4 backtracking
  Sudoku search program representation that is exact on the CPU reference lane
  and explicitly outside the current hull/sparse validated fast-path subset
- the second trained-executor follow-on bar now also exists in
  `psionic-runtime` and `psionic-eval`: the fake `SudokuClass` placeholder has
  been replaced by a real multi-case 4x4 Sudoku-v0 corpus with stable
  train/validation/test split metadata, exact CPU-reference traces per puzzle,
  and truthful article-class benchmark reporting that surfaces hull/sparse
  fallback on those backtracking workloads instead of pretending they remain
  direct fast-path cases
- the third trained-executor follow-on bar now also exists in
  `psionic-data`, `psionic-models`, `psionic-eval`, and `psionic-train`: the
  Sudoku-v0 corpus can now be materialized as deterministic program-plus-trace
  token sequences with a fixed executor vocabulary, reversible symbolic decode,
  versioned dataset manifests, split-stable lineage metadata, and frozen
  packing plans for the first honest training run
- the fourth trained-executor follow-on bar now also exists in
  `psionic-models`: a first real neural executor transformer family now runs
  next-token forward passes over the Tassadar sequence vocabulary with explicit
  2D lookup-head geometry claims, linear decode state, and a descriptor that
  marks the lane as next-token-only rather than pretending the trained model is
  already an exact executor
- the fifth trained-executor follow-on bar now also exists in
  `psionic-train` and `psionic-eval`: the executor transformer can now be
  trained with teacher-forced next-token loss over the frozen Sudoku-v0
  sequence corpus, and validation reports now expose exact-trace,
  final-output, and halt-correctness metrics against the same CPU-reference
  sequences used to build the dataset
- the sixth trained-executor follow-on bar now also exists in `psionic-eval`
  and `psionic-train`: trained-model neural linear decode can now be benchmarked
  directly against CPU reference execution on the Sudoku-v0 corpus, with
  explicit decode-mode identity, explicit no-KV-cache prefix-recompute identity,
  and per-case exactness facts instead of only aggregate scores
- the seventh trained-executor follow-on bar now also exists in
  `psionic-train` and `crates/psionic/fixtures/tassadar/runs/`: the first
  Psionic-only Sudoku-v0 reference run now persists a frozen training
  manifest, training report, linear benchmark report, checkpoint state plus
  checkpoint manifest, and a trained-model artifact bundle under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_reference_run_v0`; the
  current run is intentionally honest about still being weak
  (`validation_exact_trace_case_count = 0/2`, aggregate target exactness
  `15` bps), so this is a reproducible first-run artifact lane rather than a
  claim that Sudoku is already solved in-model
- the eighth trained-executor follow-on bar now also exists in
  `psionic-train` and that same run bundle: Phase 8 telemetry now persists
  `training_telemetry.json`, `exactness_curve.json`,
  `trace_divergence_report.json`, and `failure_samples.json`, and the current
  artifacts show that all 8 decoded cases diverge at target token 0 with case
  exactness only in the `9` to `16` bps range, which gives the next run a real
  failure-analysis baseline instead of an anecdotal “weak model” label
- the ninth trained-executor follow-on bar now also exists in
  `psionic-train`, the run bundle, and `docs/audits/`: the first run now has a
  machine-readable `postmortem.json` plus `next_run_plan.json`, and a
  human-readable review in `docs/audits/2026-03-16-tassadar-first-run-postmortem.md`;
  the resulting plan explicitly prioritizes a boundary curriculum, a larger
  optimization budget, conditional trainable-surface expansion, and truthful
  gating around what later phases do and do not prove
- the tenth trained-executor follow-on bar now also exists in
  `psionic-models`, `psionic-eval`, `psionic-train`, and that same run bundle:
  the trained executor model now exposes explicit model-KV decode selection,
  real hull-cache lookup over those KV points, and a persisted
  `neural_hull_benchmark_report.json`; on the committed Sudoku-v0 run, hull
  decode matches the explicit model-KV linear path on all `8/8` cases with no
  fallbacks or refusals and improves benchmarked decode throughput from
  `21,860` to `42,172` target tok/s over a `4,096`-token per-case window, but
  exactness remains `0/8`, so this is a real fast-path result rather than a
  claim that the model now solves Sudoku
- the eleventh trained-executor follow-on bar now also exists in
  `psionic-runtime`, `psionic-eval`, `psionic-models`, and `psionic-train`: a
  real `tassadar.wasm.sudoku_9x9_search.v1` profile, a real split-aware 9x9
  Sudoku-class corpus, a tokenized 9x9 sequence dataset plus frozen training
  manifest, a bounded 9x9 smoke-training config, and a committed
  `scale_plan.json` fixture under
  `crates/psionic/fixtures/tassadar/runs/sudoku_9x9_scale_plan_v0`; that plan
  keeps Phase 11 honest by recording the current 4x4 gate as still closed
  (`0/2` validation first-target exact cases, `0/2` exact-trace cases) while
  still making the real 9x9 workload and curriculum plan explicit
- the twelfth trained-executor follow-on bar from the post-audit issue spine
  now also exists in `psionic-eval`, `psionic-train`, `docs/audits/`, and a
  new committed follow-on run bundle at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_boundary_v1`: the learned
  4x4 lane now emits first-target / first-8 / first-32 boundary metrics,
  divergence histograms, first-token confusion, and a checkpoint leaderboard,
  and the boundary-curriculum run clears the token-0 failure at the selected
  checkpoint (`10000` bps first-target exactness, no token-0 confusions,
  divergence moved to target index `1` on both validation cases); it still has
  `0/2` exact traces and only `5000` bps first-32 exactness, so this is
  honest boundary progress rather than an exact learned-executor claim
- the thirteenth trained-executor follow-on bar from the post-audit issue
  spine now also exists in `psionic-models`, `psionic-train`,
  `psionic-research`, `docs/audits/`, and a new same-corpus ablation root at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_trainable_surface_ablation_v1`:
  the lookup-family executor now records a stable trainable surface in model
  descriptors, training manifests, checkpoints, and run bundles, and
  `psionic-research` now persists a machine-readable
  `trainable_surface_ablation.json` across four controlled surfaces; the only
  surface that beats the preserved `output_head_only` baseline is
  `output_head_embeddings_and_small_learned_mixer`, which improves boundary
  exactness to `3750` bps over the first `8` target tokens and `5625` bps over
  the first `32`, but still leaves `0/2` exact validation traces and the first
  divergence bucket at target index `1`, so this is a truthful next-surface
  recommendation rather than a promotion claim
- its Phase 8A research family now exists in `psionic-research`, with a typed
  executor-variant family, benchmark/proof/lineage-backed bounded runs, and
  machine-readable sweep records for reproducible same-contract comparisons
- its Phase 8B sparse-top-k path now exists in `psionic-runtime`,
  `psionic-models`, and `psionic-eval`, with a validated direct decode mode,
  explicit fallback on unsupported shapes, and published sparse-top-k
  throughput/speedup/CPU-gap reporting alongside CPU, linear, and hull lanes
- its Phase 9A hybrid planner route now exists in `psionic-serve`, with an
  explicit `psionic.planner_executor_route` contract, executor preflight,
  replay-stable routing decisions, typed completed/fallback/refused outcomes,
  and planner-visible policy, budget, proof, selection, and refusal truth
- its Phase 9B bounded executor-training lane now exists in `psionic-train`,
  with a small-model Tassadar trainer over package-backed supervision, a
  fixed-budget train receipt, proof-aware exactness comparison against the
  handcrafted reference lane, and explicit validation-corpus-only scope claims
- its Phase 9C compiled-weight and larger-2D exploration now exists in
  `psionic-models` and `psionic-research`, with program-specialized compiled
  executor artifacts carrying exact program binding, runtime-contract truth,
  and compile-time proof/runtime-manifest lineage, plus explicit 2D-head family
  geometry and compiled-weight suite outputs in research runs
- its Phase 9D learned-plus-compiled and learned-circuit research program now
  exists in `psionic-research`, with a typed research-only family that
  benchmarks explicit proxy surfaces against the handcrafted Wasm baseline and
  the bounded small-executor training lane while keeping proof expectations and
  claim boundaries machine-legible
- it is not current MVP compute-market product scope
- it is not a claim that Psionic is replacing native CPU execution
- its landed Phase 0/1/2/3/4/5/6/7A/7B/8A/8B/9A/9B/9C/9D issue spine is tracked in
  [#3743](https://github.com/OpenAgentsInc/openagents/issues/3743) and
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744) and
  [#3745](https://github.com/OpenAgentsInc/openagents/issues/3745) and
  [#3746](https://github.com/OpenAgentsInc/openagents/issues/3746) and
  [#3747](https://github.com/OpenAgentsInc/openagents/issues/3747) and
  [#3748](https://github.com/OpenAgentsInc/openagents/issues/3748) and
  [#3749](https://github.com/OpenAgentsInc/openagents/issues/3749) and
  [#3760](https://github.com/OpenAgentsInc/openagents/issues/3760) and
  [#3761](https://github.com/OpenAgentsInc/openagents/issues/3761) and
  [#3762](https://github.com/OpenAgentsInc/openagents/issues/3762) and
  [#3763](https://github.com/OpenAgentsInc/openagents/issues/3763) and
  [#3764](https://github.com/OpenAgentsInc/openagents/issues/3764) and
  [#3765](https://github.com/OpenAgentsInc/openagents/issues/3765) and
  [#3766](https://github.com/OpenAgentsInc/openagents/issues/3766) and
  [#3767](https://github.com/OpenAgentsInc/openagents/issues/3767)

## Crate Map

### Framework Core

- `psionic-core`: canonical tensor, shape, dtype, device, layout, and bounded
  advanced-dtype plus autocast-style precision-policy semantics contract.
- `psionic-ir`: graph, autodiff, `detach`, no-grad/training posture, and
  execution-plan types plus tensor-family capability matrices for dense,
  sparse, nested, masked, and storage-aware semantics.
- `psionic-compat`: machine-readable compatibility claim vocabulary, current
  PyTorch-facing semantics posture aggregation, the bounded MLX version-window
  or claim-language contract, the MLX acceptance-matrix report contract, and
  the seeded MLX parity-harness report plus the MLX compatibility matrix.
- `psionic-nn`: reusable module, parameter, buffer, and deterministic
  state-dict/state-tree semantics above `psionic-core`, including strict and
  non-strict keyed load behavior.
- `psionic-compiler`: lowering, scheduling, replay-stable program identity, and
  compiler diagnostics.
- `psionic-runtime`: runtime traits, allocators, compiled-plan execution,
  local-multi-device truth, and canonical execution-proof bundles.
- `psionic-catalog`: local blob, artifact, and model-catalog substrate used by
  model and serving layers.

### Backend And Platform Lanes

- `psionic-backend-cpu`: CPU backend and the current reference execution lane.
- `psionic-backend-metal`: Metal backend with the first embeddings and local
  Apple execution path.
- `psionic-backend-cuda`: CUDA backend architecture and truthful readiness
  surface.
- `psionic-backend-amd-kfd`: AMD KFD discovery/readiness substrate.
- `psionic-backend-amd-userspace`: AMD userspace discovery/readiness substrate.
- `psionic-apple-fm`: Apple Foundation Models bridge contracts and Rust client
  for the Swift sidecar.

### Network, Transport, And Execution Control

- `psionic-net`: peer identity, direct/NAT/relay sessions, rendezvous, trust
  state, and service-tunnel transport seams.
- `psionic-datastream`: resumable manifests, chunk transport, policy-weight
  broadcast refs, freshness windows, and delivery receipts.
- `psionic-cluster`: ordered-state, admission, catch-up, scheduling, and
  clustered topology truth over `psionic-net`.
- `psionic-sandbox`: bounded execution profiles, runtime detection,
  background-job lifecycle, file transfer, and repeated agentic iteration
  receipts.
- `psionic-collectives`: elastic device-mesh and benchmark-gated sync planning
  for training-class collectives.

### Serving And Adapter Surface

- `psionic-models`: reusable model families, metadata, tokenizer hooks, and
  model-loading seams.
- `psionic-serve`: served compute contracts for chat, responses, embeddings,
  scheduling, structured output, tool calling, and adapter-backed execution.
- `psionic-router`: multi-model routing, worker inventory, policy filters,
  warm/cache-aware placement, and served-fleet reliability controls.
- `psionic-provider`: provider-facing capability, readiness, and receipt types
  derived from Psionic execution truth.
- `psionic-adapters`: adapter identity, packaging, Apple `.fmadapter`
  parsing/writing, lineage, and hosted binding semantics.

### Data, Eval, Training, And Research

- `psionic-data`: versioned dataset manifests, tokenizer digests, split
  declarations, streamed iteration, and packing contracts.
- `psionic-environments`: environment package ABI, workload/difficulty/policy
  contracts, tool/rubric hooks, deterministic runtime sessions, train/eval
  parity helpers, and the `Tassadar` exact-executor environment bundle.
- `psionic-eval`: held-out eval runs, rubric-scored samples, benchmark
  packages, repeat-run aggregation, local validator simulation, Apple
  adapter eval harnesses, and the `Tassadar` package-driven exactness
  benchmark suite with CPU/reference-linear/hull-cache/sparse-top-k baselines
  and exact-equivalence reporting plus runtime capability/selection artifacts.
- `psionic-train`: checkpoint/recovery truth, elastic membership, run graphs,
  rollout-worker protocol, orchestrator control, fixed-budget training core,
  parameter-group and scheduler semantics, replay-truth and reproducibility
  semantics, Apple training execution, Apple SFT/export, model-IO
  compatibility boundaries, optional Apple draft-model distillation, and the
  bounded `Tassadar` small-executor training lane.
- `psionic-research`: typed experiment specs, bounded run manifests, result
  summaries, promotion records, and the `Tassadar` executor-variant research
  family with machine-readable sweep records for hillclimb/research loops.

### Support Tree

- `docs/`: canonical specs, acceptance matrices, runbooks, and audits.
- `fixtures/`: repo-owned fixture corpora such as Apple adapter reference
  inputs.
- `scripts/`: Psionic-specific harnesses and validation helpers.

The crate list and layering are canonical for current ownership and dependency
direction, but they are not a guarantee that every planned subsystem will land
under exactly these final crate names.

## Design Principles

- Keep machine-facing execution truth in reusable crates and keep product truth
  above Psionic.
- Keep the compiler and runtime visible and inspectable.
- Keep crate ownership narrow and documented.
- Preserve a strict boundary between reusable engine crates and OpenAgents
  provider integration.
- Prefer explicit capability/refusal surfaces over vague "supported" claims.
- Make artifacts, manifests, and receipts first-class instead of hidden side
  effects.
- Model backend families explicitly; AMD KFD and AMD userspace are separate
  backends, not one hidden toggle.
- Keep inference, embeddings, adapters, eval, and training-class substrates
  first-class in architecture from the start.

## Current Phase

Psionic is in an implemented-substrate, not-yet-complete-engine phase.

That means the repo already has a real execution tree for local serving,
adapter hosting, bounded sandbox work, early eval/train/research lanes, and a
narrow Apple adapter training path, but it still does not claim complete
backend parity or fully generalized distributed training.

### Apple Foundation Models Status

The Apple Foundation Models lane now has two distinct pieces that need to be
described separately:

- the Swift bridge plus `psionic-apple-fm` runtime surface
- the repo-owned Apple adapter training/export path in `psionic-train`

The bridge side is real and usable today for inference-time integration. The
Swift sidecar exposes health, model availability, sessions, structured output,
tool use, streaming, adapter inventory, adapter load/unload, and
session/request-level adapter binding. That is the path the desktop app and
`autopilotctl apple-fm ...` use to talk to Apple's runtime.

The training side is also real now, but it is not "the bridge trains models"
and it is not a claim that Apple exposes a repo-controlled training API. The
current repo-owned training path imports Apple adapter JSONL data through
`psionic-data`, binds it to the Apple train/eval environment package family in
`psionic-environments`, runs a fixed-budget adapter-only SFT loop in
`psionic-train`, exports a valid `.fmadapter` through `psionic-adapters`, and
can then load that package back into the bridge for local runtime smoke.

In concrete terms, yes: the repo can now train LoRA-style Apple adapter
patches today. The honest current scope is:

- frozen-base, adapter-only training over explicit low-rank parameter groups
- `f32` reference precision only
- activation checkpointing disabled in the shipped Apple reference lane
- held-out eval plus bridge-backed runtime-smoke validation before acceptance
- app-owned operator flow through `autopilotctl training launch`,
  `autopilotctl training export`, `autopilotctl training accept`, and
  `autopilotctl apple-fm load|attach`

What this does not mean is "full distributed Apple training is done." The
current Apple lane reuses the repo's data, environment, eval, optimizer,
autodiff, run-summary, and authority substrate, but it does not yet execute
through real `psionic-cluster` multi-node training, collective-backed
parameter exchange, sharded optimizer state, or production multi-device
training kernels. Those cluster/distributed-training contracts already exist as
Psionic substrate and are intended to be reused later for broader training
lanes, but the current Apple adapter path is still a narrow single-host
reference execution lane.

Implemented now:

- `psionic-catalog` local blob and artifact-catalog substrate for model and
  runtime-facing assets.
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
- Apple Foundation Models bridge contracts plus live adapter inventory,
  load/unload, attach/detach, and request-level adapter binding through
  `psionic-apple-fm` and the Swift bridge sidecar.
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
- explicit policy-weight shard manifests, lightweight control-plane refs,
  freshness windows, mirror metadata, and assembled broadcast receipts on top
  of the resumable datastream plane.
- `psionic-sandbox` runtime detection, bounded execution, background jobs,
  file-transfer lifecycle, warm reusable pools, staged loop inputs, and
  repeated agentic iteration receipts.
- canonical execution-proof bundles and embeddings-first activation-fingerprint
  proof posture.
- early train substrate: checkpoint-backed recovery, elastic membership,
  bandwidth-aware local/global sync planning, typed fixed-budget trainer
  steps, explicit checkpoint pointers and checkpoint manifests, restore
  receipts with declared recovery modes, checkpoint-anchored restore, explicit
  run graphs, contributor-set revisions, stage-program identity across
  `general_sft` / `agentic_sft` / `rl`, typed SFT trace lineage, window
  lifecycle, first orchestrator state, rollout-admission receipts, bounded
  off-policy freshness budgets, worker heartbeats, claims, upload receipts,
  and adapter lineage.
- early RL substrate: checkpoint-aware policy revisions, proof-bearing rollout
  artifacts, deterministic trainer-batch assembly, explicit policy-lineage
  digests, quarantined-versus-discarded stale-rollout pruning, typed
  rollout-validation bundles or verdicts, and a first curriculum controller
  with difficulty- and advantage-aware sample filtering plus explicit
  halt/quarantine verdicts inside `psionic-train`.
- early data substrate: versioned dataset manifests, tokenizer digests, split
  declarations, resumable streamed-iteration contracts, and long-context
  packing policies in `psionic-data`, with environment packages now binding
  versioned dataset keys instead of free-form dataset refs.
- early environment substrate: a Psionic-native package ABI, tool interfaces,
  rubric hooks, expected artifact contracts, reference runtime sessions,
  digest-pinned package aliases, mixed-surface composition groups, and
  train/eval parity receipts in `psionic-environments`, keyed to the same
  `environment_ref@version` identity used by kernel authority.
- early eval substrate: held-out eval runs, rubric-scored sample/runtime
  contracts, benchmark packages with repeat-run aggregation, and operator-local
  validator simulation in `psionic-eval`, while kernel/Nexus still own
  canonical eval-run authority truth.
- a first repo-owned Apple training lane in `psionic-train`, including the
  Apple training execution backend, Apple adapter SFT/export, and optional
  Apple draft-model distillation.
- a first integrated `agentic_sft -> rl` reference program in `psionic-train`,
  proving environment packages, dataset and checkpoint lineage, datastream
  policy-weight delivery, sandbox reuse, rollout-worker protocol, validator
  verdicts, benchmark aggregation, and one fixed-budget trainer step together
  in one typed report instead of isolated subsystem tests.
- a first explicit distributed-optimizer contract in `psionic-train`, making
  parameter sharding, gradient accumulation, optimizer-state sharding,
  precision policy, activation checkpointing, long-run memory planning, and
  collective sync attachment machine-legible on top of the fixed-budget trainer
  core.
- a first typed model-IO portability layer in `psionic-train`, making
  state-dict traversal, training-group assignment, safetensors export/import,
  torch-style JSON state artifacts, GGUF import, tokenizer version binding,
  and adapter merge/unmerge explicit instead of ad hoc.
- a first deterministic replay-truth layer in `psionic-train`, making replay
  seeds, sample-selection rules, environment and tool pins, eval posture, and
  replay drift verification machine-legible instead of scattered across
  receipts.
- a first train-security posture layer in `psionic-train`, making environment
  verification, artifact trust roots, untrusted-worker admission, poisoning
  controls, and validator-bound security receipts explicit instead of
  hand-waved around the rollout validator.
- `psionic-research` experiment specs, bounded run manifests, and promotion
  records for hillclimb-style research loops.
- broader-stack authority flows for environment packages, checkpoint-family
  policies, validator policies, benchmark packages, training policies, eval
  runs, training runs, accepted outcomes, and synthetic-data jobs now exist
  outside Psionic in kernel or Nexus surfaces.
- a narrow broader-stack Apple adapter-hosting and Apple-training projection
  now exists above Psionic in provider-substrate, desktop-control, and
  compute-market docs, without implying a generalized training market.

Still planned:

- full inference-engine maturity across model families and broader serving
  surfaces.
- richer eval-policy productization and persistent environment publication or
  authority sync.
- broader distributed training completion, freshness or validator policy, and
  orchestrator layers.
- deeper benchmark or validator policy for training-class lanes.
- policy-meaningful runtime and environment manifests plus proof-bearing
  session-claims discipline for clustered and sandboxed execution.
- AMD execution support.

For canonical current-state detail, use `docs/ARCHITECTURE.md` and
`docs/TRAIN_SYSTEM.md` rather than treating this README as the full system spec.

## Docs

- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — canonical Psionic-wide
  system spec covering layering, work classes, artifact and receipt model,
  execution lifecycle, failure, and security boundaries.
- **[docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md](docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md)** —
  canonical framework-core acceptance split for tensor, compiler, IO, replay,
  and local multi-device behavior.
- **[docs/OPERATOR_PARITY_MATRIX.md](docs/OPERATOR_PARITY_MATRIX.md)** —
  canonical seeded operator parity artifact for the current PyTorch-derived
  `OpInfo`-style coverage slice.
- **[docs/ADVANCED_OPERATOR_PROGRAM_MATRIX.md](docs/ADVANCED_OPERATOR_PROGRAM_MATRIX.md)** —
  canonical bounded advanced-operator program matrix for linalg, signal,
  attention, and explicit refusal posture for distribution and special-function
  families.
- **[docs/PROGRAM_TRANSFORM_CAPABILITY_MATRIX.md](docs/PROGRAM_TRANSFORM_CAPABILITY_MATRIX.md)** —
  canonical bounded capability matrix for functionalization, symbolic rewrites,
  export-safe graphs, and explicit future higher-order transform refusal.
- **[docs/EXPORT_DEPLOYMENT_ARTIFACT_CONTRACTS.md](docs/EXPORT_DEPLOYMENT_ARTIFACT_CONTRACTS.md)** —
  canonical bounded exportable-graph and deployment-artifact contract surface
  for graph-first packaging independent of raw checkpoints.
- **[docs/EXTENSION_CONTRACT_SEMANTICS.md](docs/EXTENSION_CONTRACT_SEMANTICS.md)** —
  canonical bounded contract surface for custom ops, kernels, autograd,
  backend plugins, and quantizer plugins.
- **[docs/DATA_INGRESS_SEMANTICS.md](docs/DATA_INGRESS_SEMANTICS.md)** —
  canonical bounded local data-ingress surface for dataset source, sampler,
  batch-sampler, and host-device staging contracts.
- **[docs/DISTRIBUTED_DATA_FEED_SEMANTICS.md](docs/DISTRIBUTED_DATA_FEED_SEMANTICS.md)** —
  canonical bounded fixed-world-size distributed data-feed surface for shard
  partitioning, worker coordination, and replay-safe input ordering.
- **[docs/TENSOR_FAMILY_CAPABILITY_MATRIX.md](docs/TENSOR_FAMILY_CAPABILITY_MATRIX.md)** —
  canonical capability and refusal matrix for dense, sparse, nested, masked,
  and storage-aware tensor-family contracts.
- **[docs/ADVANCED_DTYPE_SEMANTICS.md](docs/ADVANCED_DTYPE_SEMANTICS.md)** —
  canonical bounded promotion, cast, and backend-capability matrix for complex,
  low-precision, and wider integer dtype semantics above the compact runtime
  subset.
- **[docs/AUTOCAST_PRECISION_POLICY.md](docs/AUTOCAST_PRECISION_POLICY.md)** —
  canonical bounded autocast-style precision-policy matrix for backend-aware
  low-precision rules, numerics diagnostics, and typed refusal posture.
- **[docs/GRADIENT_SCALING_SEMANTICS.md](docs/GRADIENT_SCALING_SEMANTICS.md)** —
  canonical bounded train-class mixed-precision gradient-scaling surface for
  fp16 overflow/underflow handling and bf16 no-scaling posture.
- **[docs/QUANTIZATION_CAPABILITY_SEMANTICS.md](docs/QUANTIZATION_CAPABILITY_SEMANTICS.md)** —
  canonical bounded PTQ, QAT, quantized execution, compiler-lowering, and
  export-aware quantization capability surface above raw decode.
- **[docs/REPRODUCIBILITY_SEMANTICS.md](docs/REPRODUCIBILITY_SEMANTICS.md)** —
  canonical framework-wide replay seed, generator-derivation, and
  checkpoint-restore truth surface across runtime and training replay.
- **[docs/MODULE_PARITY_MATRIX.md](docs/MODULE_PARITY_MATRIX.md)** —
  canonical seeded module parity artifact for the current PyTorch-derived
  `module_db`-style state-tree and `state_dict` coverage slice.
- **[docs/OPTIMIZER_PARITY_MATRIX.md](docs/OPTIMIZER_PARITY_MATRIX.md)** —
  canonical seeded optimizer parity artifact for the current PyTorch-derived
  `optim_db`-style single-step optimizer coverage slice.
- **[docs/COMPILER_HYGIENE_PARITY_MATRIX.md](docs/COMPILER_HYGIENE_PARITY_MATRIX.md)** —
  canonical seeded symbolic-shape, fake-tensor, and compiler-hygiene parity
  artifact for the current PyTorch-derived compiler coverage slice.
- **[docs/SEMANTICS_CLAIM_REPORT.md](docs/SEMANTICS_CLAIM_REPORT.md)** —
  canonical machine-readable truth source for what Psionic currently treats as
  seeded evidence only versus `PyTorch-compatible later`.
- **[docs/MLX_COMPATIBILITY_SCOPE.md](docs/MLX_COMPATIBILITY_SCOPE.md)** —
  canonical bounded upstream MLX version window and claim-language contract
  for the Psionic MLX roadmap.
- **[docs/MLX_ACCEPTANCE_MATRIX.md](docs/MLX_ACCEPTANCE_MATRIX.md)** —
  canonical MLX-lane acceptance categories and machine-readable report
  contract.
- **[docs/MLX_PARITY_HARNESS.md](docs/MLX_PARITY_HARNESS.md)** —
  canonical seeded upstream MLX test families and parity-harness report
  contract.
- **[docs/MLX_COMPATIBILITY_MATRIX.md](docs/MLX_COMPATIBILITY_MATRIX.md)** —
  canonical supported/convertible/unsupported adoption matrix for the Psionic
  MLX roadmap.
- **[docs/INFERENCE_ENGINE.md](docs/INFERENCE_ENGINE.md)** — canonical
  inference-engine completion criteria and current boundaries.
- **[docs/TRAIN_SYSTEM.md](docs/TRAIN_SYSTEM.md)** — canonical training
  subsystem spec covering current substrate, planned architecture, object
  model, receipts, policy surfaces, and the issue-program path to a full
  Rust-native train stack, first tracked as GitHub issues `#3564` through
  `#3593` and later extended through `#3631`.
- **[docs/APPLE_ADAPTER_DATASET_SPEC.md](docs/APPLE_ADAPTER_DATASET_SPEC.md)** —
  canonical Apple adapter dataset contract and fixture baseline.
- **[docs/APPLE_FMADAPTER_PACKAGE_SPEC.md](docs/APPLE_FMADAPTER_PACKAGE_SPEC.md)** —
  canonical `.fmadapter` package inventory, metadata, and export contract.
- **[docs/APPLE_ADAPTER_LINEAGE_SPEC.md](docs/APPLE_ADAPTER_LINEAGE_SPEC.md)** —
  canonical Apple adapter lineage and authority-facing metadata contract.
- **[docs/TRAINING_CORE_FIXED_BUDGET_REFERENCE.md](docs/TRAINING_CORE_FIXED_BUDGET_REFERENCE.md)** —
  canonical reference loop, runbook, and acceptance criteria for the first
  real `psionic-train` fixed-budget training-core path.
- **[docs/ROLLOUT_ARTIFACT_POLICY_LINEAGE_REFERENCE.md](docs/ROLLOUT_ARTIFACT_POLICY_LINEAGE_REFERENCE.md)** —
  canonical rollout-artifact, trainer-batch, and policy-lineage runbook for
  the first reusable RL-facing contracts in `psionic-train`.
- **[docs/TRAIN_STAGE_PROGRAM_REFERENCE.md](docs/TRAIN_STAGE_PROGRAM_REFERENCE.md)** —
  canonical multi-stage `general_sft -> agentic_sft -> rl` runbook for
  `psionic-train`.
- **[docs/TRAIN_CURRICULUM_REFERENCE.md](docs/TRAIN_CURRICULUM_REFERENCE.md)** —
  canonical difficulty-aware curriculum, filtering, and non-zero-advantage
  runbook for `psionic-train`.
- **[docs/TRAIN_STABILITY_REFERENCE.md](docs/TRAIN_STABILITY_REFERENCE.md)** —
  canonical instability-telemetry, risky-optimization, and halt-policy runbook
  for `psionic-train`.
- **[docs/ENVIRONMENT_ABI_REFERENCE.md](docs/ENVIRONMENT_ABI_REFERENCE.md)** —
  canonical package ABI, runtime-session runbook, and acceptance criteria for
  the first Psionic-native environment contract.
- **[docs/ENVIRONMENT_PACKAGE_CONTRACT_REFERENCE.md](docs/ENVIRONMENT_PACKAGE_CONTRACT_REFERENCE.md)** —
  canonical package-shape runbook for workload classes, policy refs,
  difficulty metadata, and validator benchmark profiles in
  `psionic-environments`.
- **[docs/ENVIRONMENT_REGISTRY_REFERENCE.md](docs/ENVIRONMENT_REGISTRY_REFERENCE.md)** —
  canonical install, pinning, mixed-group composition, and train/eval parity
  runbook for `psionic-environments`.
- **[docs/SANDBOX_RL_THROUGHPUT_REFERENCE.md](docs/SANDBOX_RL_THROUGHPUT_REFERENCE.md)** —
  canonical warm-pool, staged-input, repeated-loop, and pool-reuse runbook for
  `psionic-sandbox`.
- **[docs/DATASET_TOKENIZER_PACKING_REFERENCE.md](docs/DATASET_TOKENIZER_PACKING_REFERENCE.md)** —
  canonical versioned-dataset, tokenizer-digest, streamed-iteration, and
  long-context packing runbook for the first Psionic-native data-contract
  layer.
- **[docs/EVAL_RUNTIME_REFERENCE.md](docs/EVAL_RUNTIME_REFERENCE.md)** —
  canonical held-out eval, benchmark-package, and local validator-simulation
  runbook for the first Psionic-native eval runtime.
- **[docs/TRAIN_RUN_GRAPH_REFERENCE.md](docs/TRAIN_RUN_GRAPH_REFERENCE.md)** —
  canonical run-graph, contributor-set, and window-lifecycle runbook for the
  first Psionic-native training run-state machine.
- **[docs/TRAIN_CHECKPOINT_RECOVERY_REFERENCE.md](docs/TRAIN_CHECKPOINT_RECOVERY_REFERENCE.md)** —
  canonical checkpoint-pointer, checkpoint-manifest, and restore-ladder
  runbook for the first explicit Psionic checkpoint-recovery receipt path.
- **[docs/COLLECTIVE_SYNC_POLICY_REFERENCE.md](docs/COLLECTIVE_SYNC_POLICY_REFERENCE.md)** —
  canonical local/global sync cadence, transport-feedback, and replanning
  runbook for the first explicit Psionic collective sync planner.
- **[docs/POLICY_WEIGHT_BROADCAST_REFERENCE.md](docs/POLICY_WEIGHT_BROADCAST_REFERENCE.md)** —
  canonical policy-weight shard, freshness, and heavy-artifact broadcast
  runbook for the first explicit Psionic datastream control-plane split.
- **[docs/TRAIN_ORCHESTRATOR_REFERENCE.md](docs/TRAIN_ORCHESTRATOR_REFERENCE.md)** —
  canonical window-control, assignment-posture, and trainer-batch assembly
  runbook for the first explicit Psionic train orchestrator.
- **[docs/AGENTIC_SFT_RL_REFERENCE_PROGRAM.md](docs/AGENTIC_SFT_RL_REFERENCE_PROGRAM.md)** —
  canonical end-to-end agentic-SFT-plus-RL pilot, including environment and
  dataset lineage, sandbox reuse, rollout-worker receipts, validator verdicts,
  online eval, benchmark aggregation, and operator-view pass criteria.
- **[docs/DISTRIBUTED_OPTIMIZER_REFERENCE.md](docs/DISTRIBUTED_OPTIMIZER_REFERENCE.md)** —
  canonical parameter-sharding, optimizer-state-sharding, precision,
  microbatch-accumulation, activation-checkpointing, and memory-budget runbook
  for the distributed optimizer layer in `psionic-train`.
- **[docs/MODEL_IO_REFERENCE.md](docs/MODEL_IO_REFERENCE.md)** —
  canonical state-dict traversal, tokenizer binding, safetensors export/import,
  GGUF import, and adapter merge/unmerge runbook for the portable model-IO
  layer in `psionic-train`.
- **[docs/TRAIN_REPLAY_TRUTH_REFERENCE.md](docs/TRAIN_REPLAY_TRUTH_REFERENCE.md)** —
  canonical replay-seed, sample-selection, environment-pin, eval-posture, and
  replay-verification runbook for `psionic-train`.
- **[docs/TRAIN_SECURITY_POSTURE_REFERENCE.md](docs/TRAIN_SECURITY_POSTURE_REFERENCE.md)** —
  canonical environment verification, artifact trust-root, untrusted-worker
  admission, and poisoning-control runbook for `psionic-train`.
- **[docs/TRAIN_ARTIFACT_STORAGE_REFERENCE.md](docs/TRAIN_ARTIFACT_STORAGE_REFERENCE.md)** —
  canonical retention-profile, deduplication, archival, garbage-collection,
  and cold-restore runbook for the train artifact-storage layer in
  `psionic-train`.
- **[docs/TRAIN_SCHEDULING_ACCOUNTING_REFERENCE.md](docs/TRAIN_SCHEDULING_ACCOUNTING_REFERENCE.md)** —
  canonical queue-class, budget-cap, preemption, and cost-attribution runbook
  for the train scheduling and accounting layer in `psionic-train`.
- **[docs/TRAIN_RELIABILITY_REFERENCE.md](docs/TRAIN_RELIABILITY_REFERENCE.md)** —
  canonical chaos-scenario, failure-injection, and recovery-suite runbook for
  the train reliability layer in `psionic-train`.
- **[docs/TRAIN_BENCHMARK_ACCEPTANCE_REFERENCE.md](docs/TRAIN_BENCHMARK_ACCEPTANCE_REFERENCE.md)** —
  canonical threshold profile, benchmark categories, and runnable acceptance
  harness for the quantitative train completion layer in `psionic-train`.
- **[docs/TRAIN_OFF_POLICY_BUDGET_REFERENCE.md](docs/TRAIN_OFF_POLICY_BUDGET_REFERENCE.md)** —
  canonical bounded stale-rollout admission, quarantine, and discard runbook
  for the first explicit Psionic off-policy control layer.
- **[docs/TRAIN_ROLLOUT_WORKER_PROTOCOL_REFERENCE.md](docs/TRAIN_ROLLOUT_WORKER_PROTOCOL_REFERENCE.md)** —
  canonical rollout-worker heartbeat, claim, upload, and worker-outcome
  runbook for the first trust-aware worker protocol in `psionic-train`.
- **[docs/TRAIN_ROLLOUT_VALIDATION_REFERENCE.md](docs/TRAIN_ROLLOUT_VALIDATION_REFERENCE.md)** —
  canonical rollout-verification bundle, sampled-adjudication, duplicate-
  detection, and validator-verdict runbook for the first validator-ready train
  integrity layer.
- **[docs/NETWORK_EXECUTION_IDENTITY_REFERENCE.md](docs/NETWORK_EXECUTION_IDENTITY_REFERENCE.md)** —
  canonical runtime-manifest, session-claims, required-vs-best-effort posture,
  and operator-surface runbook for proof-bearing networked execution identity.
- **[docs/RESEARCH_EXPERIMENT_REFERENCE.md](docs/RESEARCH_EXPERIMENT_REFERENCE.md)** —
  canonical experiment-spec, bounded result-manifest, score-contract, and
  promotion-record reference for Psionic hillclimb loops.
- **[docs/RESEARCH_RUNNER_REFERENCE.md](docs/RESEARCH_RUNNER_REFERENCE.md)** —
  canonical invocation, result-manifest, and failure-semantics reference for
  the compiled `psionic-research-runner` boundary.
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
