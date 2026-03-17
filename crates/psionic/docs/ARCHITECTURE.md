# Psionic System Spec

> Status: updated 2026-03-16 after reviewing `docs/MVP.md`,
> `docs/OWNERSHIP.md`, `crates/psionic/README.md`,
> `crates/psionic/docs/TRAIN_SYSTEM.md`,
> `docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md`,
> `crates/psionic/docs/INFERENCE_ENGINE.md`,
> `crates/psionic/psionic-array/src/lib.rs`,
> `crates/psionic/psionic-runtime/src/lib.rs`,
> `crates/psionic/psionic-cluster/src/lib.rs`,
> `crates/psionic/psionic-datastream/src/lib.rs`,
> `crates/psionic/psionic-sandbox/src/lib.rs`,
> `crates/psionic/psionic-collectives/src/lib.rs`,
> `crates/psionic/psionic-train/src/lib.rs`, and
> `crates/psionic/psionic-adapters/src/lib.rs`,
> `crates/psionic/psionic-distributed/src/lib.rs`, plus the current open and
> recently closed issue backlog through `#3860`.

## Why This Doc Exists

Psionic already has enough surface area that a short layering note is no longer
sufficient.

This document is the canonical system spec for Psionic as a whole. It answers:

- what Psionic is
- what Psionic owns and does not own
- what is implemented now
- how the subtree is layered
- what kinds of work Psionic runs
- what artifact and receipt families Psionic should emit
- how Psionic execution flows end to end
- how failures and security are handled at the substrate level

This doc should be read together with:

- `crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md`
  - framework-core completion bar for tensor, compiler, IO, replay, and local
    multi-device behavior, distinct from serving or train product acceptance,
    with a machine-readable runner artifact defined by
    `crates/psionic/docs/framework_core_acceptance_report.schema.json`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
  - deep subsystem spec for training-class execution
- `crates/psionic/docs/INFERENCE_ENGINE.md`
  - narrower completion criteria for inference-engine behavior
- `crates/psionic/docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md`
  - inference-completion plan and issue program

The Psionic Train system builds on Psionic runtime, cluster, datastream,
sandbox, and collective layers defined in this document.

## Doc Authority

- `crates/psionic/README.md` is the entrypoint and map.
- `crates/psionic/docs/ARCHITECTURE.md` is the canonical Psionic-wide system
  spec.
- `crates/psionic/docs/TRAIN_SYSTEM.md` is the canonical training subsystem
  spec.
- research audits explain why the system should move in a given direction, but
  they are not the authoritative current-state spec.

## Status Vocabulary

The status labels in Psionic docs use these meanings:

| Label | Meaning |
| --- | --- |
| `implemented` | landed and materially usable as a current substrate |
| `implemented_early` | landed, real, and usable, but still clearly early or incomplete |
| `partial` | some of the subsystem exists, but major required pieces are still missing |
| `partial_outside_psionic` | the broader OpenAgents stack has the authority or control surface, but Psionic does not yet own the native runtime or execution layer |
| `planned` | still a design target rather than a landed subsystem |

## Short Definition

Psionic is the Rust-native execution substrate for compute workloads inside
OpenAgents.

Psionic owns reusable substrate for:

- runtime execution
- backend capability and execution planning
- clustered topology and ordered state
- artifact staging and resumable transport
- runtime and environment manifest binding
- session-bound execution identity for networked lanes
- sandbox execution
- serving contracts
- training-class recovery and collective planning
- execution evidence and proof bundles

Psionic does not own:

- app UX
- wallet or payout flows
- buyer or provider product orchestration
- kernel authority or final market settlement

## What Psionic Owns

Psionic owns the machine-facing execution truth for compute lanes.

In practical terms that means:

- what artifacts were bound to execution
- what runtime or environment manifest package was actually used
- what transport or session identity claims were attached to execution
- what backend or topology ran the work
- what staged data was transferred and verified
- what proof posture or evidence was available
- what recovery or reconfiguration happened
- what receipts and execution metadata the rest of the system can consume

## What Psionic Does Not Own

Psionic is not the whole OpenAgents stack.

It must not own:

- pane-facing or desktop UX
- payout and wallet behavior
- marketplace procurement or settlement authority
- final collateral, claim, or adjudication authority
- app-owned control flows that belong in `apps/autopilot-desktop`

That boundary is intentional. Psionic explains what happened at execution time.
It does not decide what the market counts or what the product UI should do.

## Non-Goals

Psionic is also not:

- final market or settlement authority
- a home for app workflows
- a claim that every compute lane is mature today
- a hidden Python control plane behind Rust wrappers

## Tassadar Executor-Class Lane

Psionic now has an implemented-early executor-class reference lane codenamed
`Tassadar`.

The current scope is:

- owner: `crates/psionic/*`
- first target: WebAssembly-first executor semantics
- landed Phase 1 bar: CPU reference fixture plus exact parity harness
- landed Phase 2 bar: digest-bound program artifacts plus explicit
  model/program compatibility contracts
- landed Phase 3 bar: typed environment bundle plus package-driven exactness
  benchmark suite with CPU and reference-linear baselines
- landed Phase 4 bar: emitted trace artifacts, runtime-manifest lineage, and
  proof-bundle integration for replay-stable executor evidence
- landed Phase 5 bar: explicit `HullCache` fast-path decode identity, exact
  CPU/reference-linear/hull-cache equivalence checks on the validated acyclic
  subset, typed refusal for backward-branch workloads outside that subset, and
  benchmark reporting for hull-cache throughput, linear-decode speedup, and
  remaining direct-CPU gap
- landed Phase 6 bar: machine-legible runtime capability reports plus
  direct/fallback/refused decode selection diagnostics covering hull-cache,
  approximate sparse-top-k fallback, unsupported ABI/profile requests, and
  model-effective decode mismatches
- landed Phase 7A bar: explicit served `psionic.executor_trace` product
  semantics in `psionic-serve`, with typed request/response contracts,
  pull-driven trace streaming, final output extraction helpers, typed refusal
  responses, and served evidence bundles that preserve decode selection, trace
  proof, and runtime-manifest lineage
- landed Phase 7B bar: widened `core_i32_v2` Wasm profile, profile-aware
  runner construction, and article-class benchmark coverage for
  `MicroWasmKernel`, `SudokuClass`, and `HungarianMatching` with exact
  CPU/reference-linear/hull-cache parity plus published speedup and CPU-gap
  metrics
- landed trained-executor Phase 1 follow-on bar: a dedicated
  `tassadar.wasm.sudoku_v0_search.v1` profile now exists with a real 4x4
  backtracking Sudoku program representation on the CPU reference lane, while
  the validated hull/sparse fast paths still surface explicit fallback on that
  broader backward-branch search envelope
- landed trained-executor Phase 2 follow-on bar: the fake `SudokuClass`
  placeholder has been replaced by a real split-aware 4x4 Sudoku-v0 corpus
  with exact CPU-reference traces per puzzle and article-class benchmark
  reporting that stays honest about hull/sparse fallback on those search-heavy
  workloads
- landed trained-executor Phase 3 follow-on bar: the Sudoku-v0 corpus can now
  be materialized as deterministic program-plus-trace token sequences with a
  fixed executor vocabulary, reversible symbolic decode, versioned tokenized
  dataset manifests in `psionic-data`, CPU-reference dataset generation in
  `psionic-eval`, and frozen split packing plans in `psionic-train`
- landed trained-executor Phase 4 follow-on bar: `psionic-models` now carries a
  first real neural executor transformer family for the Sudoku-v0 lane, with
  explicit executor-specific descriptor/config surfaces, 2D lookup-head
  geometry claims, next-token logits over the fixed Tassadar vocabulary, and a
  claim boundary that stays honest about this being a trained sequence model
  rather than the already-exact handcrafted executor
- landed trained-executor Phase 5 follow-on bar: `psionic-train` now runs
  teacher-forced next-token optimization over the frozen Sudoku-v0 sequence
  manifest, while `psionic-eval` surfaces exact-trace, final-output, and halt
  correctness reports against the same CPU-reference sequences that generated
  the training corpus
- landed trained-executor Phase 6 follow-on bar: `psionic-eval` now benchmarks
  neural linear decode for the executor transformer against direct CPU
  reference execution on Sudoku-v0, with explicit decode-mode and KV-cache
  identity plus per-case exactness/fallback truth rather than only aggregate
  benchmark theater
- landed trained-executor Phase 7 follow-on bar: `psionic-train` now exposes a
  persisted first-run surface for the Sudoku-v0 neural executor lane, and the
  repo now carries one canonical run bundle at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_reference_run_v0` with the
  frozen training manifest, training report, linear benchmark report,
  checkpoint payload plus manifest, and trained-model artifact; the recorded
  run remains explicitly low-exactness (`0/2` validation exact-trace cases,
  `15` bps aggregate target exactness), so the claim stays at "first honest
  trained run exists" rather than "the trained executor already works"
- landed trained-executor Phase 8 follow-on bar: the same persisted run bundle
  now also carries machine-readable post-run telemetry and failure artifacts in
  `training_telemetry.json`, `exactness_curve.json`,
  `trace_divergence_report.json`, and `failure_samples.json`; those artifacts
  keep dataset/model/checkpoint identity explicit and show the current first
  run failing immediately on all 8 cases (first divergence at target token 0,
  case exactness between `9` and `16` bps), which is the correct baseline for
  later curriculum/model changes
- landed trained-executor Phase 9 follow-on bar: the same run bundle now also
  carries `postmortem.json` and `next_run_plan.json`, and the repo now has a
  human-readable first-run review in
  `docs/audits/2026-03-16-tassadar-first-run-postmortem.md`; the resulting
  plan explicitly keeps later claims tied to improved 4x4 boundary and
  short-trace exactness rather than letting scale claims outrun the evidence
- landed trained-executor Phase 10 follow-on bar: `psionic-models` now owns an
  explicit model-KV decode state plus machine-legible decode selection over
  `ReferenceLinear` and `HullCache`, `psionic-eval` now benchmarks the trained
  model’s explicit linear-scan KV path against a real hull-cache KV path and
  full direct CPU execution, and `psionic-train` now persists
  `neural_hull_benchmark_report.json` into the committed run bundle; the
  current committed run shows `8/8` hull-vs-linear prefix agreement with no
  fallback/refusal and about `1.93x` hull speedup (`42,172` vs `21,860`
  target tok/s over a `4,096`-token per-case window), while exactness remains
  `0/8`, so this phase closes the “real neural fast path exists” gap without
  pretending it closes the “trained executor works” gap
- landed trained-executor Phase 11 follow-on bar: `psionic-runtime` now owns a
  real `tassadar.wasm.sudoku_9x9_search.v1` profile plus a real split-aware
  9x9 Sudoku-class corpus, `psionic-eval` and `psionic-train` now freeze that
  workload into a tokenized sequence dataset plus training manifest,
  `psionic-models` now carries a matching 9x9 executor-transformer descriptor,
  and `psionic-train` now commits a machine-readable
  `crates/psionic/fixtures/tassadar/runs/sudoku_9x9_scale_plan_v0/scale_plan.json`
  that keeps the promotion gate explicit: the real 9x9 workload is in-tree,
  but 4x4 first-target and short-trace exactness are still blocking honest 9x9
  promotion
- landed trained-executor Phase 12 follow-on bar: `psionic-eval` now emits
  first-target / first-8 / first-32 boundary exactness plus divergence and
  first-token-confusion reports, `psionic-train` now supports an explicit
  boundary curriculum with per-epoch validation and boundary-ranked checkpoint
  selection, and the committed follow-on run bundle at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_boundary_v1` records the
  first honest post-audit boundary improvement (`10000` bps first-target
  exactness, divergence moved to target index `1`) while still failing the
  later gates (`5000` bps first-32 exactness, `0/2` exact traces)
- landed trained-executor Phase 13 follow-on bar: the lookup-family executor
  now records a stable trainable surface in descriptors, manifests,
  checkpoints, and run bundles, `psionic-train` now supports controlled output
  head / embedding / small-mixer surfaces, and `psionic-research` now commits a
  same-corpus ablation root at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_trainable_surface_ablation_v1`
  where only `output_head_embeddings_and_small_learned_mixer` materially beats
  the preserved baseline (`3750` bps first-8 exactness, `5625` bps first-32
  exactness) while still leaving `0/2` exact traces
- landed trained-executor Phase 14 follow-on bar: `psionic-train` now owns a
  canonical learned-lane promotion bundle at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1`, explicit
  `best_checkpoint_manifest.json` plus `promotion_gate_report.json` artifacts,
  and live stage/epoch/batch/validation/checkpoint progress while long runs are
  executing; the canonical promotion result remains explicitly below the bar at
  checkpoint `epoch_0006` (`10000` bps first-target, `7500` bps first-8,
  `6875` bps first-32, `0/2` exact validation traces), so this phase closes
  the “promotion tooling exists” gap without pretending the learned 4x4 gate is
  green
- landed trained-executor Phase 15 follow-on bar: `psionic-models` now carries
  a separate bounded `TassadarExecutorAttentionTransformer` family with layered
  full-prefix causal hard-max attention, fixed 2D head geometry, explicit
  per-layer semantics, and truthful hull fallback, while `psionic-eval` and
  `psionic-research` now persist a bounded same-corpus comparison root at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v1`;
  the resulting report keeps the claim boundary explicit by showing the new
  family is architecturally closer to the article but still worse than the
  preserved lookup baseline on the bounded 4x4 window (`0` bps first-target /
  first-32 exactness and `1333` target tok/s, versus `10000` / `6563` bps and
  `32000` target tok/s for the lookup baseline), so this phase lands as a
  research candidate rather than a promotion result
- landed trained-executor Phase 17 follow-on bar: `psionic-models` now carries
  a bounded typed `TassadarCompiledProgramExecutor` surface with persisted
  compile-evidence bundles, `psionic-eval` now emits exactness and
  compatibility/refusal reports for the real Sudoku-v0 corpus under
  `tassadar.wasm.sudoku_v0_search.v1.compiled_executor`, and
  `psionic-research` now materializes the canonical bundle root at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0`; the
  committed artifacts prove an exact bounded compiled/proof-backed lane on the
  matched corpus (`8/8` exact trace matches against CPU reference and `32/32`
  exact refusal matches on mismatched artifacts) while keeping the serving and
  claim boundary explicit (`eval_only`, not arbitrary-program closure, not
  learned-lane success, not article parity)
- landed trained-executor Phase 18 follow-on bar: `psionic-runtime` now
  carries a bounded real `tassadar.wasm.hungarian_v0_matching.v1` min-cost
  matching workload over 4x4 cost matrices, `psionic-models` exposes the
  matching compiled deployment fixture, `psionic-eval` now emits a real
  Hungarian-v0 benchmark package together with compiled exactness,
  compatibility/refusal, and learned-vs-compiled lane-status reports, and
  `psionic-research` now materializes the canonical bundle root at
  `crates/psionic/fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0`;
  the committed artifacts prove a bounded Hungarian-class workload contract
  plus an exact compiled/proof-backed lane on that matched corpus (`8/8`
  exact trace matches against CPU reference and `32/32` exact refusal
  matches) while keeping the serving and claim boundary explicit (`eval_only`,
  not a learned Hungarian lane, not arbitrary dimension/program closure, and
  not article parity)
- landed Phase 8A bar: typed `psionic-research` executor-variant family with
  benchmark/proof/lineage-backed bounded runs and machine-readable sweep
  records for reproducible same-contract candidate comparison
- landed Phase 8B bar: validated `SparseTopK` decode mode with explicit direct
  selection on the current subset, exact fallback on unsupported shapes, and
  benchmark reporting against CPU, reference-linear, and hull-cache baselines
- landed Phase 9A bar: planner-owned hybrid routing through
  `psionic.planner_executor_route`, with executor preflight, replay-stable
  routing decisions, typed completed/fallback/refused outcomes, and explicit
  planner-visible policy, budget, proof, selection, and refusal truth
- landed Phase 9B bar: bounded small-model executor training in
  `psionic-train`, with package-backed Tassadar supervision, fixed-budget
  training receipts, proof-aware exactness comparison against the handcrafted
  reference lane, and explicit validation-corpus-only scope claims
- landed Phase 9C bar: program-specialized compiled-weight deployments in
  `psionic-models` plus larger-2D-head family research outputs in
  `psionic-research`, with exact program-artifact binding, explicit
  runtime-contract truth, compile-time proof/runtime-manifest lineage,
  deterministic head-geometry and parameter-count declarations, and first-class
  compiled-weight suite artifacts for `program_compiled` candidates
- landed Phase 9D bar: typed learned-plus-compiled and learned-circuit
  research in `psionic-research`, with explicit research-line,
  instruction-set, execution-proxy, claim-boundary, and proof-expectation
  contracts, plus direct comparison against the handcrafted Wasm baseline and
  the bounded small-executor training lane on the validation corpus
- landed crate surfaces:
  - `psionic-runtime::tassadar`
  - `psionic-models::TassadarExecutorFixture`
  - `psionic-models::TassadarCompiledProgramExecutor`
  - `psionic-models::TassadarCompiledProgramSuiteArtifact`
  - `psionic-environments::TassadarEnvironmentBundle`
  - `psionic-eval::run_tassadar_reference_fixture_benchmark`
  - `psionic-eval::run_tassadar_article_class_benchmark`
  - `psionic-runtime::build_tassadar_execution_evidence_bundle`
  - `psionic-serve::LocalTassadarExecutorService`
  - `psionic-serve::LocalTassadarPlannerRouter`
  - `psionic-train::train_tassadar_small_executor`
  - `psionic-research::ExperimentFamily::ExecutorVariants`
  - `psionic-research::ExperimentFamily::ExecutorCircuitResearch`
- `psionic-runtime::TassadarSparseTopKRunner`
- strategic value: giving larger reasoning systems inner exact-computation
  ability

The current non-goals are:

- not current MVP compute-market product scope
- not kernel or Nexus authority work
- not app-owned UX or orchestration work
- not a claim that native CPU execution is being replaced

Phase 0 through Phase 9D are now tracked in
[#3743](https://github.com/OpenAgentsInc/openagents/issues/3743),
[#3744](https://github.com/OpenAgentsInc/openagents/issues/3744), and
[#3745](https://github.com/OpenAgentsInc/openagents/issues/3745), and
[#3746](https://github.com/OpenAgentsInc/openagents/issues/3746), and
[#3747](https://github.com/OpenAgentsInc/openagents/issues/3747), and
[#3748](https://github.com/OpenAgentsInc/openagents/issues/3748), and
[#3749](https://github.com/OpenAgentsInc/openagents/issues/3749), and
[#3760](https://github.com/OpenAgentsInc/openagents/issues/3760), and
[#3761](https://github.com/OpenAgentsInc/openagents/issues/3761), and
[#3762](https://github.com/OpenAgentsInc/openagents/issues/3762), and
[#3763](https://github.com/OpenAgentsInc/openagents/issues/3763), and
[#3764](https://github.com/OpenAgentsInc/openagents/issues/3764), and
[#3765](https://github.com/OpenAgentsInc/openagents/issues/3765), and
[#3766](https://github.com/OpenAgentsInc/openagents/issues/3766), and
[#3767](https://github.com/OpenAgentsInc/openagents/issues/3767).

## System Status At A Glance

| Area | Current Status | Current Repo Truth |
| --- | --- | --- |
| Local inference substrate | `implemented_early` | runtime, backend, model, and serve crates exist with CPU and partial Metal lanes |
| Clustered serving substrate | `implemented_early` | `psionic-cluster` owns ordered state, placement, catch-up, and sharded serving topology truth |
| Datastream and artifact staging | `implemented_early` | resumable manifests, policy-weight broadcast refs, freshness enforcement, chunk transport, and delivery receipts exist in `psionic-datastream` |
| Data contracts | `implemented_early` | `psionic-data` now owns versioned dataset manifests, tokenizer digests, split declarations, streamed iteration, and long-context packing policies |
| Sandbox execution | `implemented_early` | bounded execution, runtime detection, background jobs, file transfer, warm reusable pools, staged loop inputs, and repeated agentic iteration receipts exist in `psionic-sandbox` |
| Execution proof bundles | `implemented_early` | canonical execution-proof bundles live in `psionic-runtime` |
| Framework-core autodiff | `implemented_early` | `psionic-ir` now owns autodiff-aware graph construction, a built-in operator registry with explicit schema, implementation-family, meta-execution, and fake-execution capability contracts, an explicit `detach` op, training/evaluation plus no-grad semantics, symbolic reverse-mode backward plans, a declared gradient-support matrix, dense reference materialization, broad current primitive-family gradient coverage, first public `grad` / `value_and_grad` / `vjp` / `jvp` / bounded `vmap` / `checkpoint` transform objects above `AutodiffGraph`, graph-scoped `custom_vjp` hook registration keyed by graph digest plus reverse-mode signature, and typed refusal over current cast/backend-extension transform barriers, plus a fixed-budget trainer integration proof |
| Public lazy-array facade | `implemented_early` | `psionic-array` now owns the first public lazy-array surface above `psionic-core` and `psionic-ir`, with public device and stream handles backed by runtime-owned device truth, honest unified-memory capability flags, explicit stream-dependency policy, graph-backed arithmetic, scalar and filled-array creation helpers, reshape/permute/transpose/slice/select/concat/broadcast view families, explicit runtime determinism contracts with device-scoped seeded random-uniform and random-normal creation, logical dtype casts, `arange` / `linspace` / `eye` helpers, explicit `eval` / deferred `async_eval` semantics over replay-stable graph snapshots, explicit host-owned typed buffer export, singleton `item()` extraction, deterministic tree flatten/map/unflatten utilities, bounded runtime resource reporting with active/peak/cache counters plus cache-limit and reset controls, bounded backend-debug capture, bounded extension authoring and dispatch-resolution above the extensible operator registry, and an explicit-only implicit-materialization policy |
| Public array artifact IO | `implemented_early` | `psionic-array-io` now owns the first general array save/load surface above the lazy-array layer, with stable `ArrayArtifactReceipt` inventory, explicit dtype and quantization truth, bounded `npy` / `npz` / `safetensors` import-export, and a dense GGUF bridge that keeps export bounded to dense floating-point tensors while dequantizing GGUF block storage to logical `f32` on import instead of hiding that conversion inside model-local loaders |
| Public native function artifact IO | `implemented_early` | `psionic-function-io` now owns the first native `.psifn` function-artifact surface above `psionic-ir` and `psionic-compiler`, plus a bounded `.mlxfn` compatibility shell on top of that native substrate, with export-safe graph contracts, optional compiler artifacts, optional trace-family identity, optional deployment bundle binding, stable import/export receipts, compatibility receipts, and explicit validation that graph, compiler, trace, and deployment digests still describe the same replay-safe function boundary |
| Public distributed groups | `implemented_early` | `psionic-distributed` now owns the first public framework-distributed group layer above runtime mesh truth, with explicit mesh bootstrap from ordered member facts, reusable global-group initialization, honest singleton fallback when no reusable group exists, ordered member/rank snapshots, explicit-plan subgroup split semantics, and machine-readable backend-family capability snapshots over current topology profiles |
| Public distributed collectives | `implemented_early` | `psionic-distributed` now also owns the first bounded public collective-helper layer above `DistributedGroup`, with MLX-style singleton passthrough for `all_sum` / `all_gather` / `reduce_scatter`, explicit host-owned reference emulation for multi-rank `all_sum` / `all_gather` / `reduce_scatter` and `recv`, validation-only `send`, typed collective-support snapshots, and explicit `ring` / `mpi` / `nccl` mapping plus typed `jaccl` refusal instead of pretending backend transport execution is already public |
| Public distributed launch/config planning | `implemented_early` | `psionic-distributed` now also owns a bounded public launch/config planning shell above cluster, sandbox, and mesh truth, with hostfile parsing, honest single-rank-per-node validation, cluster membership/address/backend readiness checks, sandbox contract preflight, per-rank bootstrap payloads and sandbox job plans, distributed reserved-environment synthesis, cluster execution evidence, stable plan digests, and topology-profile-backed backend-family validation instead of a parallel compatibility-only launcher |
| Public distributed gradient helpers | `implemented_early` | `psionic-distributed` now also owns bounded tree-aware data-parallel gradient helpers above the public collective layer, with `grouped_all_sum` / `grouped_all_reduce` small-leaf packing over deterministic tree structure and floating-point `average_gradients` on top of the current reference-emulated all-reduce surface |
| Public distributed tensor-parallel helpers | `implemented_early` | `psionic-distributed` now also owns bounded MLX-style `AllToShardedLinear` and `ShardedToAllLinear` wrappers above the public distributed layer, with deterministic row/column sharding from bounded `psionic-nn::Linear`, inspectable shard-layout snapshots, local shard-input splitting, bias-slice versus bias-owner semantics, and reference-emulated multi-rank `ShardedToAllLinear` reconstruction that requires explicit rank wrappers and shard inputs instead of pretending backend transport is already public |
| Public distributed FSDP helpers | `implemented_early` | `psionic-distributed` now also owns a bounded MLX-style `fsdp_apply_gradients` helper above the public distributed and train-contract layer, with typed `zero_stage3` admission, mixed replicated/full-shard group handling, explicit remote-rank parameter-state and gradient-batch maps for reference emulation, optional global-norm clipping, shard-local optimizer updates with residency transitions, gathered full-parameter reconstruction, and stable apply receipts instead of inventing a trainer-private distributed update path |
| Collectives | `implemented_early` | elastic device-mesh observation, bandwidth-aware local/global sync planning, and benchmark-gated collective cadence receipts exist in `psionic-collectives` |
| Train recovery substrate | `implemented_early` | checkpoint, live-recovery, elastic-membership session truth, explicit checkpoint manifests or pointers, and restore receipts exist in `psionic-train` |
| Training run graph | `implemented_early` | `psionic-train` now owns typed training runs, stage-program identity, contributor-set revisions, topology revisions, participant lifecycle, and window transitions |
| Training orchestrator | `implemented_early` | `psionic-train` now owns typed window-control, assignment-posture, rollout-assignment refs, rollout-admission receipts, bounded off-policy freshness budgets, rollout-worker heartbeats, claims, upload receipts, curriculum receipts, instability verdicts, and trainer-batch assembly requests over the run graph |
| Adapter lineage | `implemented_early` | adapter identity, packaging, and hosted binding lineage exist in `psionic-adapters` |
| Eval runtime | `implemented_early` | `psionic-eval` now owns held-out eval runs, rubric-scored sample/runtime contracts, benchmark packages, repeat-run aggregation, and operator-local validator simulation, while kernel/Nexus now own canonical eval-run plus accepted-outcome authority truth |
| Environment package runtime | `implemented_early` | `psionic-environments` now owns the runtime ABI, typed workload/policy/difficulty/benchmark package shape, tool/rubric hooks, expected artifact contracts, deterministic reference sessions, digest-pinned package aliases, mixed-surface composition groups, and train/eval parity receipts, while kernel/Nexus now own environment, checkpoint-family, validator-policy, benchmark-package, and training-policy registry truth |
| Training core reference loop | `implemented_early` | `psionic-nn` now owns reusable module, parameter, buffer, deterministic state-tree/state-dict semantics, strict/non-strict keyed load behavior with explicit size-mismatch refusal, and a bounded eval-oriented quantized-module shell for supported weight families, while `psionic-train` owns the typed fixed-budget trainer-step path with parameter-group scaling semantics, scheduler bindings, optimizer state, residency transitions, checkpoint/model-IO state roundtrip, checkpoint restore lineage, and step telemetry; broader distributed trainer completion is still planned |
| Full synthetic-data or research loop | `partial_outside_psionic` | synthetic-data job and verification flows now exist in kernel/Nexus, but no Psionic-native generation runtime or research-loop crate family exists yet |
| Executor-class in-model compute lane | `implemented_early` | WebAssembly-first, CPU-reference-first `Tassadar` reference lane now exists in `psionic-runtime`, `psionic-models`, `psionic-environments`, `psionic-eval`, `psionic-serve`, `psionic-train`, and `psionic-research` with machine-legible `core_i32_v1`, widened `core_i32_v2`, and `tassadar.wasm.sudoku_v0_search.v1` Wasm profiles, an append-only trace ABI, profile-aware CPU reference and fixture runners, a real 4x4 backtracking Sudoku search-program representation on the CPU reference lane, a real split-aware 4x4 Sudoku-v0 corpus with exact CPU-reference traces per puzzle, explicit `HullCache` fast path for the validated acyclic subset, a validated `SparseTopK` decode path on its own bounded subset, exact CPU/reference-linear/hull-cache/sparse-top-k equivalence harnesses, typed refusal surfaces including backward-branch and sparse-shape fallback truth, machine-legible runtime capability reports, direct/fallback/refused decode selection diagnostics, digest-bound program artifacts, explicit model/program compatibility descriptors, typed environment bundles, package-driven exactness benchmark suites over both the validation corpus and the widened article-class corpus (`MicroWasmKernel`, `SudokuClass`, `HungarianMatching`) with CPU/reference-linear/hull-cache/sparse-top-k reporting and runtime capability/selection artifacts, emitted trace artifacts, runtime-manifest lineage, canonical proof-bundle integration, an explicit `psionic.executor_trace` served request/stream/terminal contract, a planner-owned `psionic.planner_executor_route` contract with preflight and replay-stable routing truth, a bounded small-model training lane with proof-aware exactness receipts over the validation corpus, a program-specialized compiled-weight deployment path with exact program binding and compile-time proof lineage, and typed research families that run benchmark/proof/lineage-backed executor variant sweeps plus learned-circuit research comparisons against the handcrafted and trained-small baselines while keeping claim boundaries explicit; it is still not current MVP product scope |

Recent issue closure changed one important reading of this table:

> environment packages, checkpoint-family policies, validator policies,
> benchmark packages, training policies, eval runs, training runs, accepted
> outcomes, and synthetic-data authority flows now exist in the broader
> OpenAgents stack, and Psionic now owns the first environment plus eval runtime
> crates, but broader generation loops still remain unfinished.

## Canonical Layer Model

Psionic should be understood as a layered subtree with clear dependency
direction.

### System Diagram

```text
Applications / Operators / Authority
        |
        v
  psionic-provider
        |
        v
 psionic-serve / psionic-models
        |
        v
 psionic-train / psionic-eval / psionic-data / psionic-collectives / psionic-adapters
        |
        v
 psionic-cluster / psionic-datastream / psionic-sandbox / psionic-net
        |
        v
 backend crates
        |
        v
 psionic-runtime / psionic-compiler / psionic-ir / psionic-core
```

### Layering By Crate

1. `psionic-core`
   - foundational tensor, dtype, shape, device, layout, view-semantics, and
     cross-library refusal-taxonomy types
2. `psionic-ir`
   - canonical graph, built-in plus extensible operator registry, custom-op
     schema and backend-dispatch registration contracts, transform-safety and
     functionalization contracts, dense plus non-dense meta-tensor family
     contracts, fake/meta execution and plan validation contracts,
     detach/no-grad/autodiff tracking, symbolic backward plans, and
     execution-plan representation
3. `psionic-compiler`
   - lowering, schedule-formation, fusion-policy, memory-plan, and
     plan-cache-identity boundaries over IR, plus the first public
     compile-transform surface with explicit purity, concrete-plan cache
     identity, bounded shapeless trace-family identity, trace capture, and
     plan-debug posture
   - public array-debug capture in `psionic-array` now reuses compiler
     trace/debug configuration instead of inventing a lane-local debug path
   - compiler replay fixtures now guard deterministic lowering, explicit
     schedule/fusion/memory/cache artifacts, and topology-bound program
     identity through
     `scripts/lint/psionic-compiler-replay-gate.sh`
4. `psionic-runtime`
   - runtime traits, runtime planning, execution-proof bundles, training-class
     runtime truth
   - backend-visible buffer storage identity and view-posture contracts
   - runtime-owned RNG, generator-state, checkpoint-restore, and
     deterministic-algorithm contracts
   - same-type local multi-device plan-runner contracts, explicit local
     sharding policy and refusal taxonomy, and local multi-device execution
     evidence kept distinct from clustered execution truth
5. `psionic-sandbox`
   - bounded execution profiles, runtime detection, execution receipts, and
     background-job lifecycle
6. `psionic-net`
   - peer identity, transport sessions, relay-backed rendezvous, trust and
     candidate state
7. `psionic-datastream`
   - resumable manifests, lightweight policy-weight broadcast refs, freshness
     control, chunk transfer, and delivery receipts for artifacts
8. `psionic-data`
   - versioned dataset manifests, tokenizer digests, split declarations,
     streamed iteration, and packing policy contracts
9. `psionic-eval`
   - held-out eval runs, rubric-scored runtime contracts, benchmark packages,
     repeat-run aggregation, and local validator simulation
10. `psionic-cluster`
   - ordered state, cluster admission, catch-up, scheduling, topology and
     placement truth
11. `psionic-collectives`
   - elastic device-mesh, local/global sync planning, transport-feedback
     replanning, and quantized collective policy
12. `psionic-train`
   - training-session truth for checkpointing, live recovery,
     elastic-membership posture, checkpoint pointers/manifests, restore
     receipts, and orchestrator control state
13. `psionic-adapters`
   - adapter identity, packaging, and hosted binding lineage
14. backend crates
   - backend-specific runtime implementations only
15. `psionic-models`
   - reusable model definitions and metadata
16. `psionic-serve`
   - request, response, and execution contracts for served products
17. `psionic-router`
   - reusable multi-model routing inventory, policy filters, and worker-path
     selection for served fleets
18. `psionic-provider`
   - provider-facing capability, readiness, and receipt types at the OpenAgents
     boundary

The crate list and layering are canonical for current ownership and dependency
direction, but they are not a guarantee that every planned subsystem will land
under exactly these final crate names.

### Dependency Direction

- lower crates must not depend on higher product-facing crates
- no crate in `crates/psionic/` may path-depend on `apps/*`
- reusable engine crates must not own app workflows or market authority
- `psionic-provider` is the boundary adapter, not a place to hide app logic

## Canonical Psionic Work Classes

Psionic needs two different notions of work class:

- product-level execution classes
- low-level runtime scheduling classes

### Product-Level Work Classes

| Work Class | Meaning | Current Status |
| --- | --- | --- |
| Inference | generate model outputs for served requests | `implemented_early` |
| Embeddings | generate vectors or embedding outputs | `implemented_early` |
| Clustered serving | execute inference across replicas or sharded topology | `implemented_early` |
| Sandbox execution | run bounded remote or local sandbox jobs | `implemented_early` |
| Artifact staging | move datasets, checkpoints, served artifacts, and adapter bundles | `implemented_early` |
| Training-class coordination | coordinate checkpoints, recovery, collectives, and elastic membership | `implemented_early` |
| Full training | execute trainer-step and optimizer updates | `planned` |
| Eval | run shared held-out or online evaluation | `planned` |
| Synthetic-data generation | generate or score new data under the same substrate | `planned` |
| Adapter-backed serving | serve a base artifact plus attributed adapter lineage | `implemented_early` |

### Low-Level Runtime Work Classes

These are the scheduler-facing classes already encoded in
`psionic-runtime::RuntimeWorkClass`.

| Runtime Work Class | Meaning |
| --- | --- |
| `DecodeToken` | one latency-sensitive decode step |
| `PrefillBatch` | one prefill or preparation batch |
| `DatastreamChunk` | one chunk transfer over the data plane |
| `CollectiveStep` | one collective or synchronization step |
| `CheckpointFlush` | one checkpoint or persistence flush step |

The system-wide rule is:

> product work classes explain what Psionic is doing for the platform, while
> low-level runtime work classes explain how the runtime schedules the work.

## Canonical System Objects

Psionic needs a stable object vocabulary across serving, staging, sandbox, and
training subsystems.

| Object | Owner | Purpose | Current Status |
| --- | --- | --- | --- |
| `RuntimeWorkItem` | `psionic-runtime` | one low-level schedulable unit of work | `implemented` |
| `ExecutionProofBundle` | `psionic-runtime` | canonical execution evidence for runtime work | `implemented` |
| `LocalRuntimeObservability` + `BackendRuntimeResources` + `CompilePathEvidence` | `psionic-runtime` / `psionic-serve` | machine-legible local-runtime operator truth for execution posture, queue/scheduler posture, backend health, selected-device identity, and compile/cache state | `implemented` |
| `DatastreamManifest` | `psionic-datastream` | full resumable manifest for one artifact stream | `implemented` |
| `DatastreamManifestRef` | `psionic-datastream` | compact artifact reference embedded in other contracts, including explicit distributed KV spill/restore locators | `implemented` |
| `DatastreamPolicyWeightBroadcastManifest` | `psionic-datastream` | lightweight control-plane summary for a multi-shard policy-weight artifact | `implemented_early` |
| `DatasetManifest` | `psionic-data` | versioned dataset, tokenizer, split, and shard-lineage contract | `implemented_early` |
| `DatasetIterationContract` | `psionic-data` | resume-safe split iteration over datastream-backed shards | `implemented_early` |
| `DatasetPackingPolicy` | `psionic-data` | long-context sequence packing and token-budget batch planning contract | `implemented_early` |
| `DataIngressSemanticsReport` | `psionic-data` | machine-readable bounded local data-ingress capability report over dataset source, sampler, batch-sampler, and host-device staging contracts | `implemented` |
| `DistributedDataFeedSemanticsReport` | `psionic-data` | machine-readable bounded fixed-world-size distributed data-feed report over shard partitioning, worker coordination, and replay-safe per-rank ordering contracts | `implemented` |
| `PsionicRefusal` | `psionic-core` | canonical cross-library refusal taxonomy for unsupported op, gradient, layout, capability, serialization, sandbox-policy, and topology boundaries | `implemented_early` |
| `AdvancedDTypeSemanticsReport` | `psionic-core` | machine-readable bounded promotion, cast, and backend-capability matrix for complex, float8, wider integer, and higher-precision real dtype semantics above the compact runtime-core subset | `implemented` |
| `AutocastPolicyMatrixReport` | `psionic-core` | machine-readable bounded autocast-style precision-policy matrix over backend family, preferred low-precision dtype, operator family, numerics diagnostics, and typed refusal posture | `implemented` |
| `QuantizationCapabilitySemanticsReport` | `psionic-core` | machine-readable bounded PTQ, QAT, runtime-execution, compiler-lowering, and export-aware quantization capability matrix above raw file-format decode | `implemented` |
| `OperatorParityMatrixReport` | `psionic-ir` | machine-readable seeded operator parity cases and refusal proofs against the current PyTorch-derived oracle window | `implemented` |
| `AdvancedOperatorProgramMatrixReport` | `psionic-ir` | machine-readable bounded linalg, signal, and attention-family program matrix plus explicit refusal posture for distribution and special-function families | `implemented` |
| `ProgramTransformCapabilityMatrixReport` | `psionic-ir` | machine-readable bounded capability matrix for functionalization, symbolic-rewrite readiness, export-safe graphs, bounded public `checkpoint`/`vmap`/`jvp`, and explicit remaining higher-order transform refusal | `implemented` |
| `ExportableGraphContract` | `psionic-ir` | machine-readable export-safe graph envelope with named input/output bindings for downstream packaging and deployment | `implemented` |
| `ExtensionContractSemanticsReport` | `psionic-ir` | machine-readable bounded contract surface for custom ops, kernels, autograd, backend plugins, and quantizer plugins above the extensible registry | `implemented` |
| `TensorFamilyCapabilityMatrixReport` | `psionic-ir` | machine-readable capability and refusal matrix for dense, sparse, nested, masked, and storage-aware tensor-family semantics across meta, declared-output, alias-view, and runtime-materialization surfaces | `implemented` |
| `ArrayDevice` + `ArrayStream` + `ArrayContext` + `Array` + `EvaluatedArray` + `PendingAsyncEval` + `ArrayMemoryCounters` + `ArrayRuntimeResourceReport` + `ArrayCacheLimitControl` + `ArrayCacheResetReceipt` | `psionic-array` | first public lazy-array facade above `psionic-core` and `psionic-ir`, including runtime-backed device truth, unified-memory capability flags, explicit stream-dependency policy, context-owned graph construction, graph-backed arithmetic, scalar and filled-array creation helpers, reshape/permute/transpose/flatten/expand_dims/squeeze/slice/select/concat/broadcast view families, explicit runtime determinism contracts with seeded random creation, logical dtype casts, `arange` / `linspace` / `eye` helpers, axis-aware sum reduction, explicit `eval` / deferred `async_eval` semantics, bounded runtime resource reporting with active/peak/cache counters, explicit cache-limit and reset controls, and explicit-only materialization boundaries over replay-stable graph snapshots | `implemented_early` |
| `ArrayArtifactReceipt` + `encode_*` / `decode_*` + `save_*_path` / `load_*_path` | `psionic-array-io` | public array artifact IO above `psionic-array`, including `npy`, `npz`, `safetensors`, and bounded dense GGUF import/export with explicit receipt inventory, dtype truth, and GGUF quantization-to-dense import disclosure | `implemented_early` |
| `FunctionArtifact` + `FunctionCompileBundle` + `FunctionArtifactReceipt` + `MlxfnCompatibilityReceipt` + `encode_function_artifact` / `decode_function_artifact` + `encode_mlxfn_function_artifact` / `decode_mlxfn_function_artifact` + `save_*_path` / `load_*_path` | `psionic-function-io` | public native `.psifn` function artifact IO above `psionic-ir` and `psionic-compiler`, plus a bounded `.mlxfn` compatibility shell on top of the native artifact, including export-safe graph contracts, optional compiler artifacts, optional trace-family identity, optional deployment bundle binding, stable artifact digests, import/export receipts, compatibility receipts, and explicit replay-safe validation with typed refusal outside the current `.mlxfn` subset | `implemented_early` |
| `CompilerHygieneParityMatrixReport` | `psionic-compiler` | machine-readable seeded symbolic-shape, fake-tensor, and compiler-hygiene parity cases including one bounded shapeless trace-family identity seed plus explicit symbolic-shape and reshape-formula refusal proofs for the current PyTorch-derived oracle window | `implemented` |
| `DeploymentArtifactContract` + `ExportDeploymentArtifactSemanticsReport` | `psionic-compiler` | machine-readable deployment bundle contract and bounded report for execution-plan and topology-aware graph-first artifacts | `implemented` |
| `SemanticsClaimReport` | `psionic-compat` | machine-readable claim vocabulary that separates seeded evidence from `PyTorch-credible` and `PyTorch-compatible later` posture across the current semantics program | `implemented` |
| `MlxCompatibilityScopeReport` | `psionic-compat` | machine-readable bounded upstream MLX version window and claim-language contract that keeps `MLX-class` distinct from later `MLX-compatible` facades | `implemented` |
| `MlxCpuReferenceCoverageReport` | `psionic-array` | machine-readable bounded CPU-reference coverage contract over imported MLX `array_core`, `ops_numeric`, and `device_eval_memory` families, with seeded supported cases and typed refusal posture above the public array surface | `implemented` |
| `MlxAcceptanceMatrixReport` | `psionic-compat` | machine-readable MLX-lane closure contract over array/runtime, transform/compile, `nn`/optimizer, export/tooling, distributed, and backend-closure categories | `implemented` |
| `MlxParityHarnessReport` | `psionic-compat` | machine-readable seeded upstream MLX test-family harness carrying bounded `pass`, `refusal`, and `unsupported` outcomes tied to repo-owned Psionic hooks | `implemented` |
| `MlxCompatibilityMatrixReport` | `psionic-compat` | machine-readable supported/convertible/unsupported adoption matrix that keeps current MLX claims bounded to governance support, explicit bridges, and intentionally unsupported public surfaces | `implemented` |
| `Module` | `psionic-nn` | reusable nested module tree with deterministic parameter, buffer, and submodule traversal, explicit trainable versus frozen posture, recursive freeze/unfreeze helpers, and bounded public `save_weights` / `load_weights` wrappers | `implemented` |
| `ModuleParityMatrixReport` | `psionic-nn` | machine-readable seeded module parity cases and refusal proofs for the current PyTorch-derived normalized module-tree and `state_dict` oracle window | `implemented` |
| `ModuleStateDict` | `psionic-nn` | deterministic keyed `state_dict` and saved-weights view with stable path order and persistent-vs-all-buffer selection | `implemented` |
| `ModuleStateTree` | `psionic-nn` | digest-bound flattened parameter or buffer view that downstream train, checkpoint, and compatibility code can consume | `implemented` |
| `ModuleStateLoadReport` | `psionic-nn` | explicit strict/non-strict load receipt returned by bounded public `load_weights` behavior, with loaded, missing, unexpected, and digest-transition facts | `implemented` |
| `NnTensor` | `psionic-nn` | bounded dense cpu-f32 layer input/output wrapper above `TensorSpec` plus `TensorData` | `implemented_early` |
| `Linear` + `Embedding` + `LayerNorm` + `RmsNorm` + `Activation` + `Dropout` + `Conv1d` + `Conv2d` + `Pool1d` + `Pool2d` | `psionic-nn` | bounded public CPU-reference core layer surface built above the shared module/state substrate | `implemented_early` |
| `LossReduction` + `mse_loss` + `l1_loss` + `binary_cross_entropy_loss` + `cross_entropy_loss` + `softmax_last_dim` + `log_softmax_last_dim` + `sigmoid` + `one_hot` + `InitKind` + `init_tensor` + `init_parameter` | `psionic-nn` | bounded public CPU-reference losses, initializers, and helper functions for tiny training loops above the shared module/state substrate | `implemented_early` |
| `OptimizerKind` + `OptimizerConfig` + `SchedulerKind` + `SchedulerConfig` + `SchedulerBinding` + `ParameterGroupSemantics` + `Optimizer` + `OptimizerStateSnapshot` + `OptimizerModuleStepReport` + `OptimizerGroup` + `MultiOptimizer` + `MultiOptimizerStepReport` | `psionic-nn` | bounded public optimizer-and-scheduler shell above `psionic-train` math with module-path keyed state, explicit frozen-parameter handling, parameter-group scaling, multi-optimizer composition, snapshot restore, and per-step receipts | `implemented_early` |
| `ModuleQuantizeConfig` + `Module::quantize` + `QuantizedModule` + `QuantizedLinear` + `QuantizedEmbedding` | `psionic-nn` | bounded eval-oriented quantized-module shell with explicit keep-dense versus strict posture, frozen quantized module reports, and dequantize-to-`f32` forward semantics for supported linear and embedding families | `implemented_early` |
| `OptimizerParityMatrixReport` | `psionic-train` | machine-readable seeded optimizer parity cases and refusal proofs for the current PyTorch-derived single-step optimizer oracle window | `implemented` |
| `GradientScalingSemanticsReport` | `psionic-train` | machine-readable bounded train-class mixed-precision report for fp16 dynamic loss scaling, overflow/underflow handling, bf16 no-scaling posture, and typed refusal boundaries | `implemented` |
| `ReproducibilitySemanticsReport` | `psionic-train` | machine-readable framework-wide replay seed, deterministic-mode, generator-derivation, and checkpoint-restore report across training replay and runtime determinism contracts | `implemented` |
| `BufferStorageContract` | `psionic-runtime` | backend-visible storage identity and logical view posture for one realized buffer | `implemented_early` |
| `RuntimeDeterminismContract` | `psionic-runtime` | runtime-owned RNG, generator-state, checkpoint-snapshot, and deterministic-algorithm contract for replayable execution | `implemented_early` |
| `RuntimeManifest` | `psionic-runtime` proof layer | digest-bound package for artifact, static-config, mutable-variable, and runtime lineage used at execution time | `implemented_early` |
| `DatastreamDeliveryReceipt` | `psionic-datastream` | verified proof of delivered bytes and chunk progress | `implemented` |
| `ClusterState` | `psionic-cluster` | authoritative cluster membership and ordered-state truth | `implemented` |
| `SessionClaimsBundle` | `psionic-net` / proof layer | session-scoped claims bound into the authenticated transport payload so peer identity carries runtime-manifest and proof posture in machine-legible form | `implemented_early` |
| `TrainingCheckpointReference` | `psionic-runtime` | stable identity for one training checkpoint | `implemented` |
| `TrainingRecoveryContext` | `psionic-runtime` | runtime-visible recovery posture for training-class execution | `implemented` |
| `TrainingDeviceMeshContext` | `psionic-runtime` | runtime-visible elastic device-mesh posture | `implemented` |
| `TrainingCollectiveContext` | `psionic-runtime` | runtime-visible collective posture and benchmark evidence | `implemented` |
| `ModelIoCompatibilityContract` | `psionic-train` | machine-readable boundary contract for supported and unsupported checkpoint/model portability surfaces | `implemented` |
| `AdapterArtifactIdentity` | `psionic-adapters` | stable identity for one adapter artifact | `implemented` |
| `AdapterPackageManifest` | `psionic-adapters` | package manifest for adapter bytes tied to datastream | `implemented` |
| `ProviderSandboxExecutionReceipt` | `psionic-sandbox` | receipt for one bounded sandbox run | `implemented` |
| `TrainingRun` | `psionic-train` | root identity, participant graph, and lifecycle state for one training program | `implemented_early` |
| `TrainingWindow` | `psionic-train` | one synchronized contribution or trainer interval with contributor-set and transition state | `implemented_early` |
| `TrainingSchedulerBinding` | `psionic-train` | typed scheduler config plus mutable per-group scheduler state for optimizer-step resolution | `implemented` |
| `TrainerBatchAssemblyRequest` | `psionic-train` | lightweight control-plane request for one trainer batch over rollout refs | `implemented_early` |
| `RolloutTaskClaim` | `psionic-train` | deterministic task-claim contract for one rollout assignment under one worker heartbeat | `implemented_early` |
| `RolloutAdmissionReceipt` | `psionic-train` | typed acceptance, quarantine, or discard receipt for one rollout artifact under bounded off-policy policy | `implemented_early` |
| `RolloutWorkerOutcomeReceipt` | `psionic-train` | typed claim-expiry, upload-policy, or orchestrator-wrapped outcome receipt for one rollout worker | `implemented_early` |
| `RolloutVerificationBundle` | `psionic-train` | validator-ready bundle for one rollout artifact, worker outcome, and optional benchmark evidence | `implemented_early` |
| `ValidatorVerdict` | `psionic-train` | typed validator outcome over one rollout bundle, including replay, duplicate, normalization, and benchmark checks | `implemented_early` |
| `CollectiveSyncCadenceReceipt` | `psionic-collectives` | typed cadence, transport-feedback, and replan-trace receipt for one sync step | `implemented_early` |
| `CheckpointPointer` | `psionic-train` | stable pointer to the latest accepted checkpoint for a run, stage, or window | `implemented_early` |
| `CheckpointManifest` | `psionic-train` | typed shard, digest, writer, and durability description for one checkpoint flush | `implemented_early` |
| `EnvironmentPackage` | `psionic-environments` | reusable task, rubric, tool, dataset, and artifact environment package | `implemented_early` |
| `EnvironmentBenchmarkProfile` | `psionic-environments` | validator- or operator-reusable benchmark profile bound into one environment package | `implemented_early` |
| `BenchmarkPackage` | `psionic-eval` | validator-owned packaged benchmark harness or reference evaluation profile with repeat-run aggregation | `implemented_early` |
| `EvalRun` | `psionic-eval` | one local evaluation execution over a declared environment and artifact set | `implemented_early` |

The important point is not that every object already exists. The important
point is that Psionic should converge on a typed object model rather than
passing loosely structured blobs between subsystems.

Psionic enforces capability envelopes at runtime, while higher-level compute
products define the admissible execution contract exposed to buyers, operators,
and authority layers.

## Glossary

| Term | Meaning |
| --- | --- |
| execution truth | what the Psionic runtime and cluster can honestly say happened at execution time |
| authority truth | what higher-level OpenAgents services accept as canonical outcome |
| artifact truth | what manifests, digests, package refs, and staged bytes were actually bound to execution |
| runtime identity | the verified execution origin responsible for a work item |
| session claims bundle | the signed session-scoped claim set that ties peer or session keys to runtime and artifact identity |
| training window | one bounded contributor or trainer interval with explicit control-plane state |
| checkpoint lineage | the chain of checkpoint identities, manifests, and durability transitions that define recoverable train state |
| checkpoint pointer | the stable reference to the latest accepted checkpoint for a run, stage, or window |
| checkpoint manifest | the typed shard, digest, writer, and durability description for one checkpoint flush |
| policy revision | the specific weight or policy version a worker, trainer, or eval run consumed |
| environment package | a versioned task, rubric, tool, and sandbox contract used by training or eval |
| benchmark package | a validator-owned packaged benchmark or reference evaluation profile reused for repeatable scoring |
| proof posture | the declared strength and availability of execution evidence |
| validator posture | the declared verification policy and adjudication expectations for a workload |
| manifest registry | a versioned allowlist or policy registry for manifests, proof profiles, or environment packages |
| receipt | the typed record of an accepted state transition or outcome |
| collective posture | the mesh, communication, quantization, and benchmark facts attached to one collective step |

## Artifact Model

Psionic is also an artifact system, not only an execution engine.

### Canonical Artifact Families

| Artifact | Current Carrier | Meaning |
| --- | --- | --- |
| Served artifact | `DatastreamSubjectKind::ServedArtifact` | model or sharded serving artifact used for inference |
| Checkpoint | `DatastreamSubjectKind::Checkpoint` plus `TrainingCheckpointReference` | recoverable training or optimizer state |
| Tokenized corpus | `DatastreamSubjectKind::TokenizedCorpus` | tokenized dataset shard delivered for training or eval |
| Eval bundle | `DatastreamSubjectKind::EvalBundle` | benchmark or evaluation harness artifact |
| Benchmark package | `psionic-eval` | validator-owned packaged benchmark harness or reference evaluation profile |
| Adapter package | `DatastreamSubjectKind::AdapterPackage` plus adapter manifests | adapter or LoRA artifact delivered with lineage |
| Proof artifact | execution-proof bundle or augmentation | evidence about what the runtime or cluster actually did |
| Sandbox artifact | sandbox input/output digest sets | staged inputs and outputs of bounded execution |
| Environment package | `psionic-environments` | versioned task, tool, rubric, dataset, and sandbox contract |

### Artifact Rules

- artifacts should be digest-bound
- artifacts should be referenceable through compact manifest refs where
  possible
- runtime and environment identity should distinguish digest-bound measured or
  static config from mutable runtime variables
- artifacts should carry enough lineage to explain what execution actually
  consumed
- policy-meaningful lanes should reference versioned manifest or profile
  registries rather than opaque free-form strings
- Psionic should not rely on unnamed side files for economically or
  operationally important artifacts

## Receipts And Truth Boundaries

Psionic is receipt-first, but it is not authority-first.

The tree should be understood through four truth domains.

| Truth Domain | Owned By | What It Says |
| --- | --- | --- |
| Runtime truth | `psionic-runtime` and lower execution crates | what device, work class, and proof posture actually ran |
| Artifact truth | `psionic-datastream`, `psionic-adapters`, `psionic-eval`, and `psionic-environments` | what bytes, manifests, packages, and digests were actually staged or referenced |
| Cluster and sandbox truth | `psionic-cluster`, `psionic-sandbox`, `psionic-collectives`, `psionic-train` | what topology, recovery posture, sandbox runtime, and collective decisions actually occurred |
| Authority truth | outside Psionic in kernel and control services | what the platform or market accepts as final outcome |

The key boundary is:

> Psionic determines execution truth. Higher-level OpenAgents services determine
> authority truth.

### Runtime Identity

Runtime identity means the verified execution origin responsible for a work
item, including provider node identity, sandbox instance identity, or cluster
member identity.

Runtime identity matters because it anchors:

- proof attribution
- validator checks
- receipt lineage

### Session Claims And Manifest Discipline

For proof-bearing networked execution, transport identity should carry a signed
session-claims bundle that references runtime, environment, and artifact
digests.

Psionic should also distinguish:

- digest-bound measured or static config
- mutable runtime variables
- higher-level policy profiles or manifest registries evaluated outside Psionic

That split keeps runtime truth honest without collapsing execution evidence and
policy authority into one crate.

### Canonical Receipt Families

| Receipt Family | Current Status | Producer |
| --- | --- | --- |
| runtime execution proof bundles | `implemented` | `psionic-runtime` |
| datastream delivery receipts | `implemented` | `psionic-datastream` |
| sandbox execution receipts | `implemented` | `psionic-sandbox` |
| clustered execution evidence | `implemented_early` | `psionic-cluster` |
| rollout admission receipts | `implemented_early` | `psionic-train` |
| rollout-worker outcome receipts | `implemented_early` | `psionic-train` |
| rollout validator verdicts | `implemented_early` | `psionic-train` |
| training run, trainer step, and eval receipts | `planned` | future `psionic-train` and `psionic-eval` layers |
| adapter package and hosted binding lineage | `implemented_early` | `psionic-adapters` |

## Canonical Execution Lifecycle

Every Psionic workload should fit the same high-level lifecycle even when the
details differ by lane.

1. Work is declared through typed contracts.
2. Artifact bindings and execution prerequisites are resolved.
3. Capability and topology are checked against the requested work.
4. Required artifacts are staged or resumed through datastream contracts.
5. Runtime or cluster planning produces executable work items and topology
   posture.
6. Execution occurs on the declared backend, sandbox, or cluster.
7. Evidence and receipts are emitted from the execution substrate.
8. Operator or authority surfaces consume the typed result rather than raw
   process logs.

## Time Semantics

Psionic execution participates in several time boundaries:

- artifact freshness windows
- checkpoint durability windows
- execution timeouts
- sandbox lifetime limits
- transport retry and resume windows

Training-class and clustered execution build additional timing contracts on top
of these substrate-level boundaries rather than inventing a separate execution
clock.

### Serving Variant

For serving lanes this typically means:

- served artifact resolution
- backend and capability gating
- queue admission, fairness, mixed prefill/decode work, and explicit TTFT/ITL
  plus prefill/decode handoff truth when the lane supports it
- hierarchical KV residency truth across host, device, and any explicit
  externalized tier contract the lane can actually surface
- structured outputs, tool or response-state semantics, and optional multi-model
  routing
- optional clustered placement and shard handoff
- response and proof emission

### Sandbox Variant

For sandbox lanes this typically means:

- profile realization
- bounded runtime selection
- input staging
- job execution
- output and receipt emission

### Training-Class Variant

For training-class lanes this should eventually mean:

- checkpoint and dataset staging
- participant topology formation
- mesh and collective planning
- trainer or rollout execution
- checkpoint flush and recovery handling
- train-specific receipt emission

The training variant is only partially implemented today.

## Control Plane And Observation Boundaries

Psionic exports typed state; it does not own the operator shell.

### App-Owned Control Plane

The desktop app and `autopilotctl` should consume Psionic truth for:

- capability and readiness
- runtime or cluster state
- manifest refs and session-claims posture
- artifact staging progress
- queue or admission posture, shard or cache placement, and sandbox pool health
- sandbox job state
- challenge or validator status when the lane uses one
- training and eval diagnostics, once those exist

### Authority Plane

Kernel and control services should consume Psionic truth for:

- receipts
- proof bundles
- staged artifact references
- cluster and recovery posture
- validator-facing evidence

### What Psionic Must Not Do Here

Psionic must not:

- own app workflows
- invent settlement authority
- collapse operator presentation and execution truth into one crate

## Failure Model

Psionic should handle failure explicitly and typefully.

| Failure | Expected Substrate Handling |
| --- | --- |
| backend unsupported or unavailable | fail capability checks early and expose truthful readiness posture |
| node loss during clustered execution | trigger catch-up, reconfiguration, or recovery according to cluster and train posture |
| network degradation | replan collective or transport decisions when observations degrade materially |
| datastream interruption | resume from cursor and committed bytes rather than restart whole transfer blindly |
| checkpoint flush failure | keep checkpoint non-durable and block any state transition that requires durability |
| sandbox crash | emit bounded execution failure receipt and apply retry or quarantine policy outside the sandbox engine |
| cluster membership mismatch | reject the state transition rather than silently rebasing to a different cluster |
| detached or invalid session claims | reject policy-meaningful networked execution rather than treating transport identity alone as sufficient |
| unapproved quantized collective request | reject planning rather than silently downgrade without record |
| stale artifact or policy revision | reject or quarantine the work item under explicit freshness rules |
| proof augmentation unavailable | emit explicit proof posture rather than pretending strong proof exists |

Psionic must surface failure as typed, reason-coded events rather than opaque
runtime exceptions.

Psionic should prefer:

- reason-coded failure
- replay-safe state transitions
- explicit degraded posture
- checked-in compiler replay fixtures for behavior-preserving lowering changes

It should avoid:

- silent fallback that changes truth without record
- opaque runtime-only failure behavior

## Security Model

Psionic is not the whole platform security model, but it does own several core
security surfaces.

| Threat | Mitigation Direction In Psionic |
| --- | --- |
| artifact tampering | manifest digests, chunk digests, object digests, provenance linkage |
| checkpoint tampering | checkpoint-family binding, writer identity, manifest verification, durable checkpoint posture |
| cluster spoofing or false membership | peer identity, admission policy, ordered-state truth, cluster mismatch rejection |
| detached transport identity or forged proof-bearing sessions | session-claims bundles bound to peer or session keys plus manifest refs and policy checks |
| sandbox escape or undeclared runtime behavior | bounded profiles, explicit runtime detection, execution receipts |
| proof opacity | explicit proof augmentation posture instead of hidden assumptions |
| manifest or policy-registry drift | versioned manifest registries and explicit profile identifiers carried through receipts and authority integrations |
| stale or mismatched policy artifacts | freshness windows and policy-revision binding in planned train layer |
| malicious rollout workers | planned validator sampling and train-layer admission control |
| transport degradation or relay ambiguity | explicit transport observations and candidate state in `psionic-net` and `psionic-cluster` |

The system-wide rule is:

> Psionic should always prefer explicit identity, digest binding, and typed
> degraded posture over implicit trust.

## Current And Planned Psionic Scope

Psionic already has real system scope across:

- runtime execution
- clustered serving
- sandbox execution
- artifact transport
- proof bundles
- training-class recovery substrate

Psionic is still growing into:

- full inference-engine maturity
- full Rust-native train core
- environment and eval runtime
- synthetic-data and research loops
- production-hardening around reproducibility, storage, and security

Those planned areas should still land inside the same system model described
here, not as a disconnected parallel stack.

## Companion Subsystem Specs

- `crates/psionic/docs/TRAIN_SYSTEM.md`
  - deep specification for the training subsystem
- `crates/psionic/docs/INFERENCE_ENGINE.md`
  - narrow inference completion criteria
- `crates/psionic/docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md`
  - detailed inference build-out and issue plan
- `crates/psionic/docs/COMPILER_REPLAY_REFERENCE.md`
  - compiler replay-fixture policy and validation entrypoints

## Review Checklist

- Is this logic in the lowest Psionic crate that can honestly own it?
- Does the change keep execution truth separate from app or market authority?
- Are artifacts and receipts typed and inspectable?
- Is degraded or missing proof posture stated explicitly?
- Does the change preserve the boundary between reusable Psionic substrate and
  app-owned or authority-owned control flow?

## Bottom Line

Psionic is already more than an inference experiment. It is the reusable Rust
execution substrate for OpenAgents compute lanes.

Today it already owns:

- runtime execution truth
- clustered topology truth
- artifact staging
- sandbox execution
- proof bundles
- early training-class recovery and collective truth

What it still lacks is not a new architectural direction. It lacks completion
of the same direction:

- mature inference engine behavior
- full environment and eval layers
- broader distributed training completion
- production-grade receipt, security, and operating discipline across the whole
  subtree

That is the Psionic program.
