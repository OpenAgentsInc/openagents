# Psionic Train System

> Status: updated 2026-03-16 after reviewing `docs/MVP.md`,
> `docs/OWNERSHIP.md`,
> `docs/audits/2026-03-13-intellect-lessons-for-psionic-train-audit.md`,
> `docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md`,
> `crates/psionic/README.md`,
> `crates/psionic/docs/ARCHITECTURE.md`,
> `crates/psionic/docs/AUTORESEARCH_INTEGRATION_PLAN.md`,
> `docs/kernel/compute-training-authority.md`,
> `docs/headless-compute.md`,
> `crates/psionic/psionic-runtime/src/lib.rs`,
> `crates/psionic/psionic-datastream/src/lib.rs`,
> `crates/psionic/psionic-collectives/src/lib.rs`,
> `crates/psionic/psionic-train/src/lib.rs`,
> `crates/psionic/psionic-environments/src/lib.rs`,
> `crates/psionic/psionic-eval/src/lib.rs`,
> `crates/psionic/psionic-adapters/src/lib.rs`,
> `crates/psionic/psionic-data/src/lib.rs`, and
> `crates/psionic/psionic-sandbox/src/lib.rs`,
> `apps/autopilot-desktop/src/desktop_control.rs`, and
> `apps/autopilot-desktop/src/bin/autopilotctl.rs`, plus the recently closed
> train-adjacent issue backlog through `#3643` and the decentralized adapter
> training issue program starting at `#3636`.

## Why This Doc Exists

The March 13 Intellect audit correctly described the shape Psionic should grow
toward, but one part of it is now stale: Psionic no longer lacks a
`psionic-train` crate entirely.

The tree now has:

- `psionic-train`
- `psionic-collectives`
- `psionic-adapters`

That means the right question is no longer "should Psionic have any train
subtree at all?"

The right question is:

> what does the Psionic train system honestly implement today, what does it
> still not implement, and what is the full Rust-native path from the current
> substrate to a real training system?

This doc answers that question.

The train system assumes the execution substrate defined in
`ARCHITECTURE.md` and does not redefine runtime, cluster, sandbox, or artifact
transport behavior.

Apple-specific adapter work is no longer only later-family planning. The repo
now owns a canonical spec-and-fixture baseline for it in:

- `crates/psionic/docs/APPLE_ADAPTER_DATASET_SPEC.md`
- `crates/psionic/docs/APPLE_FMADAPTER_PACKAGE_SPEC.md`
- `crates/psionic/docs/APPLE_ADAPTER_LINEAGE_SPEC.md`
- `crates/psionic/fixtures/apple_adapter/`

and now also has:

- a repo-owned Apple training execution backend in `psionic-train`
- a first Rust-native Apple adapter SFT/export lane
- an optional Apple draft-model distillation lane
- an app-owned desktop-control and `autopilotctl` operator path that can
  launch, export, and accept one Apple training run into kernel authority

## Doc Authority

- `crates/psionic/docs/TRAIN_SYSTEM.md` is the canonical training subsystem
  spec.
- `crates/psionic/docs/ARCHITECTURE.md` is the canonical Psionic-wide system
  spec that defines the lower execution substrate this doc builds on.
- `crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md` is the canonical
  framework-core acceptance split; train acceptance must not be used as a
  substitute for framework-core parity claims.
- `crates/psionic/docs/ARCHITECTURE_EXPLAINER_CLUSTER_BRINGUP_RUNBOOK.md` is
  the canonical operator guide for the first truthful multi-device clustered
  attempt around the `Psionic architecture explainer` path.
- `crates/psionic/docs/APPLE_ADAPTER_DATASET_SPEC.md`,
  `crates/psionic/docs/APPLE_FMADAPTER_PACKAGE_SPEC.md`, and
  `crates/psionic/docs/APPLE_ADAPTER_LINEAGE_SPEC.md` are the canonical
  Apple-adapter reference docs for dataset shape, package inventory, and
  lineage metadata.
- `docs/audits/2026-03-13-intellect-lessons-for-psionic-train-audit.md` is
  research rationale, not the canonical current-state spec.
- `docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md`
  is a code-grounded adaptation audit for windowed training, checkpoint
  protocol discipline, validator-owned benchmark truth, and bounded research
  loops.

## Status Vocabulary

This doc uses the canonical status vocabulary defined in `ARCHITECTURE.md`:
`implemented`, `implemented_early`, `partial`, `partial_outside_psionic`, and
`planned`.

## Short Definition

The Psionic train system is not one crate.

It is the Rust-native training-class execution stack inside `crates/psionic/`
that should eventually own:

- training-session truth
- elastic membership and recovery
- collective planning
- checkpoint and weight movement
- environment-bound training and eval execution
- rollout ingestion and validation
- trainer and orchestrator control flow
- operator-inspectable receipts for the whole system

Today Psionic implements the lower half of that stack plus a first real
trainer-step core.

It already has real substrate for:

- reusable module, parameter, buffer, explicit trainable-versus-frozen
  posture, and deterministic state-tree semantics in `psionic-nn`
- deterministic `state_dict` naming plus bounded public `save_weights` /
  `load_weights` behavior with strict/non-strict keyed load posture and
  explicit size-mismatch refusal in `psionic-nn`
- a bounded reusable CPU-reference core layer surface in `psionic-nn`,
  covering linear, embedding, norms, activations, dropout, conv, and pooling
  families above the same module/state substrate
- a bounded reusable CPU-reference loss, initializer, and helper surface in
  `psionic-nn`, covering `mse_loss`, `l1_loss`, `binary_cross_entropy_loss`,
  `cross_entropy_loss`, `softmax_last_dim`, `log_softmax_last_dim`,
  `sigmoid`, `one_hot`, `init_tensor`, and `init_parameter`
- a bounded reusable public optimizer shell in `psionic-nn` that reuses
  `psionic-train` optimizer math while keeping module-path keyed state,
  explicit frozen-parameter handling, state snapshot restore, and per-step
  receipts in the framework-facing layer
- a bounded reusable public scheduler and parameter-group shell in
  `psionic-nn` that reuses `psionic-train` scheduler primitives while keeping
  scheduler bindings, group-level learning-rate and weight-decay scaling, and
  multi-optimizer composition in the framework-facing layer
- a bounded reusable eval-oriented quantized module shell in `psionic-nn`
  covering `Module::quantize(...)`, explicit keep-dense versus strict
  quantize reports, and `QuantizedLinear` plus `QuantizedEmbedding` wrappers
  over `int8_symmetric` block storage with explicit dequantize-to-`f32`
  forward semantics
- a seeded PyTorch-derived module parity matrix for normalized module-tree and
  `state_dict` semantics in `psionic-nn`, with an explicit refusal proof for
  registration-order-preserving `state_dict` parity
- a seeded PyTorch-derived optimizer parity matrix for SGD, Adam, AdamW,
  LARS, and LAMB single-step behavior in `psionic-train`, with an explicit
  refusal proof for state-kind mismatch
- training recovery posture
- checkpoint lineage
- elastic membership truth
- device-mesh and collective planning
- resumable dataset and checkpoint transport
- typed fixed-budget trainer steps
- per-group optimizer state, scaling semantics, scheduler bindings, and
  residency policy
- reusable optimizer contracts plus typed SGD, Adam, AdamW, LARS, and LAMB
  state/update semantics with explicit scheduler-driven learning-rate
  resolution
- reverse-mode autodiff, explicit detach, and training/no-grad gradient
  semantics over canonical IR primitives
- machine-legible step telemetry and checkpoint-anchored restore lineage
- checkpoint-aware policy revisions
- proof-bearing rollout artifacts and trainer-batch assembly
- versioned dataset manifests, tokenizer digests, split declarations, and
  long-context packing contracts
- environment package ABI and deterministic runtime sessions
- held-out eval runtime, benchmark packages, repeat-run aggregation, and local
  validator simulation
- bounded `Tassadar` small-executor training over the validation benchmark
  package, using the fixed-budget training core plus proof-aware exactness
  comparison against the handcrafted reference lane
- adapter lineage

It does not yet implement the full distributed trainer-orchestrator-RL runtime.

## What Psionic Train Is Not

- It is not a promise that full general model training already works in the
  repo today beyond the current narrow Apple adapter lane.
- It is not a Python trainer hidden behind Rust wrappers.
- It is not an app-owned workflow inside `apps/*`.
- It is not just "cluster execution, but for training."
- It is not just checkpoint files and background notes.

The honest description today is:

> Psionic already owns real training-class truth surfaces plus a bounded
> training-core reference loop, but it does not yet own the full distributed
> train system.

That now includes one intentionally narrow executor-training answer:

- `psionic-train::train_tassadar_small_executor(...)` can train a bounded small
  Tassadar model over package-backed validation-corpus supervision
- the learned lane uses the same fixed-budget training receipts as the rest of
  the train substrate rather than a sidecar research script
- evaluation remains proof-aware and baseline-aware: trained traces, outputs,
  and halt posture are checked against the handcrafted reference lane and keep
  the reference proof-bundle digests visible in the resulting report
- the resulting claim is intentionally scoped to the validation corpus only; it
  is not a claim that larger learned executors, broader Wasm coverage, or
  compile-to-weights work are already complete in Psionic
- the runtime side now does carry the first honest broader-executor substrate
  for later training work: `tassadar.wasm.sudoku_v0_search.v1` can represent a
  real 4x4 backtracking Sudoku program on the CPU reference lane, but that is
  still substrate for later corpus/model/training issues rather than a claim
  that the trained executor already exists
- the benchmark side now also carries a real split-aware 4x4 Sudoku-v0 corpus
  with stable train/validation/test assignments and exact CPU-reference traces
  per puzzle, which replaces the earlier placeholder `SudokuClass` proxy and
  gives later tokenization/training issues an honest package-backed source
  corpus
- the tokenized-data side now also exists for that same corpus: Psionic can
  freeze deterministic program-plus-trace token sequences with explicit
  tokenizer/vocabulary digest lineage, split-stable dataset manifests, and
  generic packing plans for train/validation/test instead of leaving later
  training work to regenerate traces ad hoc
- the model side now also has a first honest train target above that corpus:
  `psionic-models` carries a real neural executor transformer family with
  explicit next-token logits, linear decode state, and 2D lookup-head geometry
  claims, while still keeping the claim boundary truthful that this is not yet
  the exact handcrafted executor path
- the train/eval loop now also exists for that model family: `psionic-train`
  can run teacher-forced next-token optimization over the frozen sequence
  manifest, and `psionic-eval` can score the trained model with exact-trace,
  final-output, and halt correctness against the same CPU-reference sequences
  that define the corpus
- the benchmark side now also includes the trained-model comparison the audit
  asked for: neural linear decode is measured directly against CPU reference
  execution with explicit decode-mode and KV-cache identity so the remaining
  performance and exactness gap is visible instead of being hidden behind the
  handcrafted runtime lanes
- the first persisted trained-run surface now also exists for that same lane:
  `psionic-train` can execute a canonical Sudoku-v0 reference run and persist
  the frozen training manifest, training report, linear benchmark report,
  checkpoint payload plus checkpoint manifest, and trained-model artifact under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_reference_run_v0`; the
  current committed run is intentionally recorded as low exactness
  (`validation_exact_trace_case_count = 0/2`, aggregate target exactness
  `15` bps), which makes it useful as a real learning baseline rather than as
  benchmark theater
- the train-side learning loop artifacts now also exist for that same run:
  `psionic-train` can augment the persisted bundle with
  `training_telemetry.json`, `exactness_curve.json`,
  `trace_divergence_report.json`, and `failure_samples.json`, all bound to the
  same dataset/model/checkpoint identity; those artifacts currently show every
  decoded case diverging at target token 0, which is exactly the sort of
  machine-readable failure baseline the next curriculum/model iteration needs
- the post-run review loop now also exists for that same run:
  `psionic-train` can emit `postmortem.json` and `next_run_plan.json`, and the
  repo keeps the resulting plan bound to the same persisted run identity;
  `docs/audits/2026-03-16-tassadar-first-run-postmortem.md` is the
  human-readable companion review, and the current plan keeps the next run
  disciplined by prioritizing boundary curriculum and larger optimization
  budget first, and by explicitly gating 9x9 scale claims on better 4x4
  exactness evidence
- the neural fast-path benchmark loop now also exists for that same run:
  `psionic-models` now exposes explicit model-KV decode state plus
  machine-legible decode selection, `psionic-eval` can compare the trained
  model’s explicit linear-scan KV path against a real hull-cache KV path and
  direct CPU execution, and `psionic-train` can persist
  `neural_hull_benchmark_report.json` into the committed run bundle; the
  current committed Sudoku-v0 run shows `8/8` hull-vs-linear prefix agreement
  with no fallbacks or refusals and about `1.93x` hull speedup (`42,172` vs
  `21,860` target tok/s over a `4,096`-token per-case window), while exactness
  remains `0/8`, which is the right claim boundary for the lane today
- the Phase 11 scale-out substrate now also exists above that run:
  `psionic-runtime` owns a real `tassadar.wasm.sudoku_9x9_search.v1` profile
  plus a real split-aware 9x9 Sudoku-class corpus, `psionic-eval` and
  `psionic-train` can freeze that workload into a tokenized sequence dataset
  plus training manifest, `psionic-models` carries the matching 9x9
  executor-transformer descriptor, and `psionic-train` commits a machine-
  readable `scale_plan.json` fixture under
  `crates/psionic/fixtures/tassadar/runs/sudoku_9x9_scale_plan_v0`; the same
  plan keeps the promotion gate explicit, so Phase 11 now means “real 9x9
  workload and curriculum path exist” rather than “the 9x9 trained executor is
  already good”
- the Phase 12 boundary-truth run now also exists above that baseline:
  `psionic-eval` emits first-target / first-8 / first-32 exactness plus
  first-divergence and confusion reports, `psionic-train` now supports an
  explicit boundary curriculum with per-epoch validation and boundary-ranked
  checkpoint selection, and the resulting follow-on run bundle at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_boundary_v1` records that
  the selected checkpoint clears the token-0 boundary (`10000` bps
  first-target exactness, no token-0 confusions, divergence bucket moved to
  target index `1`) while still failing the later gates (`5000` bps first-32
  exactness, `0/2` exact traces); `docs/audits/2026-03-16-tassadar-phase-12-boundary-audit.md`
  is the human-readable companion note for that run
- the Phase 13 trainable-surface ablation now also exists above that baseline:
  the lookup-style executor family now records a stable trainable surface in
  descriptors, manifests, checkpoints, and run bundles; `psionic-train` can
  update the output head alone, the output head plus token embeddings, the
  output head plus token and position embeddings, or those plus one small
  learned residual mixer; and `psionic-research` now materializes a
  same-corpus ablation root at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_trainable_surface_ablation_v1`
  with a machine-readable `trainable_surface_ablation.json`; that report keeps
  `output_head_only` as the preserved baseline and recommends only
  `output_head_embeddings_and_small_learned_mixer`, which improves the
  selected checkpoint to `3750` bps first-8 exactness and `5625` bps first-32
  exactness while still leaving `0/2` exact validation traces and the first
  divergence bucket at target index `1`; the companion human-readable audit is
  `docs/audits/2026-03-16-tassadar-phase-13-trainable-surface-audit.md`
- the Phase 14 promotion-truth run now also exists above that baseline:
  `psionic-train` can execute the canonical promotion config, stream live
  stage/epoch/batch/validation/checkpoint progress while it runs, and persist
  `best_checkpoint_manifest.json` plus `promotion_gate_report.json` under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1`; the selected
  checkpoint is still `epoch_0006` from `prompt_to_first_16_tokens` with
  `10000` bps first-target exactness, `7500` bps first-8 exactness,
  `6875` bps first-32 exactness, and `0/2` exact validation traces, so the
  promotion gate remains red and the companion human-readable audit is
  `docs/audits/2026-03-16-tassadar-phase-14-blocker-audit.md`
- the Phase 14 teacher-forced continuation now also exists beside that
  baseline: `psionic-train` can execute the separate preserved config under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v2`, keeping the
  same lookup-family surface and Phase 14 gate while removing greedy-rollout
  refinement and extending teacher-forced 16-/32-token supervision; the
  resulting selected checkpoint `epoch_0008` reproduces but does not beat the
  prior best (`10000` bps first-target, `7500` bps first-8, `6875` bps
  first-32, `0/2` exact traces), and later 32-token epochs still regress, so
  that bundle closes the “maybe this was just a schedule problem” question
  without pretending the learned 4x4 gate is any closer to green; the
  companion audit is
  `docs/audits/2026-03-16-tassadar-promotion-v2-teacher-forced-audit.md`
- the Phase 15 executor-attention comparison now also exists beside that
  baseline: `psionic-models` now carries a distinct bounded
  `TassadarExecutorAttentionTransformer` family with layered causal hard-max
  attention, fixed 2D head geometry, explicit per-layer semantics, and hull
  fallback to reference-linear decode; `psionic-eval` and `psionic-research`
  now materialize a same-corpus comparison root at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v1`
  with `architecture_comparison_report.json` plus per-family run bundles
  against the preserved Phase 13 lookup baseline; the committed report keeps
  the claim boundary honest by showing the new family is closer to the article
  structurally but still much worse on the bounded validation window (`0` bps
  first-target and first-32 exactness, `1333` target tok/s, hull fallback)
  than the lookup baseline (`10000` / `6563` bps, `32000` target tok/s, direct
  hull decode), so this phase is a research-family landing rather than a
  promotion or parity result
- the post-Phase-15 trained-attention follow-on now also exists beside that
  seeded comparison: `psionic-research` now runs a bounded attention-family
  output-head training loop and persists its artifacts under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_training_v1`,
  while a trained-family comparison is now preserved under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v2`;
  the resulting artifacts show real learning progress off the seeded
  attention-family floor (`6563` bps aggregate and first-32 exactness instead
  of `0`), but they also show the same remaining blocker plainly: the trained
  attention family still gets the first target token wrong (`0` bps
  first-target), still yields `0/2` exact bounded traces, and therefore still
  does not clear the open Phase 14 gate
- the post-Phase-15 boundary-adapter follow-on now also exists beside that
  trained attention floor: `psionic-models` now carries a bounded
  relative-target output-bias adapter, `psionic-research` now preserves the
  failed output-head-only boundary attempt under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v1`,
  the improved adapter-backed run under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v2`, and
  the later hidden-state projection-adapter follow-ons under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v3` and
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v4`, the
  newer previous-token-conditioned transition-adapter follow-on under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v5`, the
  later joint transition+projection fine-tune under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v6`, the
  later trace-schema and per-position saturation runs under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v7`,
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v8`, and
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v9`, and
  the current same-corpus comparison under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v11`;
  the accepted `boundary_v2` run keeps the token-0 fix without destroying the
  bounded suffix (`10000` bps first-target, `7500` bps first-8, `6875` bps
  first-32), the later `boundary_v3` / `boundary_v4` follow-ons prove the
  remaining blocker is structural rather than vague, and the newer
  `boundary_v5` / `v7` pair proves the structural-transition surface can move
  that blocker deeper into the trace (`10000` bps first-target, `8750` bps
  first-8, `7188` bps first-32), while the later `boundary_v6` / `v8`
  joint-adapter fine-tune reproduces but does not beat that ceiling and the
  later `boundary_v7` / `boundary_v8` / `boundary_v9` saturation set proves
  the current bounded adapter family is stuck on the exact same validation
  signature; the promotion gate is still red, though, because exact validation
  traces remain `0/2` and the learned lane still first diverges at token `6`
  by predicting `<byte_00>` where the reference requires `<pc>`
- the separate Phase 17 compiled lane now also exists beside that learned
  stack: `psionic-models` now exposes a bounded typed
  `TassadarCompiledProgramExecutor` with compile-evidence bundles,
  `psionic-eval` now emits machine-readable exactness and
  compatibility/refusal reports for the real Sudoku-v0 corpus under the
  workload family id `tassadar.wasm.sudoku_v0_search.v1.compiled_executor`,
  and `psionic-research` now persists the canonical bundle root at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0`; the
  committed artifacts prove only a bounded compiled/proof-backed lane on the
  matched corpus (`8/8` exact trace matches against CPU reference and `32/32`
  exact refusal matches), with explicit `eval_only` posture, so this does not
  close the open learned-lane promotion gate and does not unblock 9x9 by
  itself
- the separate Phase 18 compiled Hungarian lane now also exists beside that
  learned stack: `psionic-runtime` now carries a real bounded
  `tassadar.wasm.hungarian_v0_matching.v1` min-cost matching workload over 4x4
  cost matrices, `psionic-eval` now emits a real Hungarian-v0 benchmark
  package plus machine-readable compiled exactness/refusal and learned-vs-
  compiled lane-status reports, and `psionic-research` now persists the
  canonical bundle root at
  `crates/psionic/fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0`;
  the committed artifacts prove only a bounded Hungarian-class workload
  contract plus an exact compiled/proof-backed lane on the matched corpus
  (`8/8` exact trace matches and `32/32` exact refusal matches), so this does
  not make the learned lane green by association and does not justify article-
  parity language
- `psionic-research` can now use that bounded trained-small receipt as an
  explicit comparator inside the learned-plus-compiled and learned-circuit
  Tassadar research family, but that does not expand the train-side claim
  boundary beyond `validation_corpus_only`

## Apple Adapter Reality

The Apple Foundation Models lane now has an honest training answer, and that
answer needs to stay precise.

Yes, the repo can now train LoRA-style Apple adapter patches and export valid
`.fmadapter` packages.

That does **not** mean:

- the Swift bridge is doing the training
- Apple exposes a repo-controlled Foundation Models training API
- the current Apple lane is already a generalized distributed trainer

The current path is:

1. `psionic-data` imports Apple adapter JSONL into the repo-owned dataset
   contract, preserving tokenizer, prompt-shaping, tool/schema augmentation,
   and long-context packing lineage. The app now derives that lineage from the
   live Apple FM bridge health profile plus dataset-aware default-instruction
   and locale posture instead of reusing hard-coded placeholder digests, and
   `psionic-data` now derives prompt/completion/tool/schema token captures from
   the full structured transcript path rather than from character counts.
2. `psionic-environments` binds that dataset to the Apple adapter
   train/eval/benchmark environment package family so the train, held-out eval,
   and runtime-smoke lanes all point at one versioned environment truth.
3. `psionic-train` now exposes the live Apple SFT executor through the
   repo-owned `AppleAdapterTrainingExecutionBackend` plus
   `run_apple_adapter_sft_export(...)`, which turns packed samples into
   token-sequence reference batches with turn-aware prompt pooling,
   tool/schema boundary encoding, completion-side token-sequence supervision,
   checkpoint emission, and staged `.fmadapter` package construction.
4. The current operator path in
   `apps/autopilot-desktop/src/apple_adapter_training_control.rs` now calls
   that Rust-native Psionic executor directly for the authoritative train and
   export path. A fresh live operator validation run
   (`rust-native-3769-validation-1773643126445`) completed the Rust train
   step, wrote repo-owned checkpoints, emitted a Rust-native Apple
   `adapter_weights.bin` blob-storage container, staged the final package, and
   then completed the bridge-backed held-out eval plus runtime-smoke path with
   `runtime_smoke_passed=true`. That means the narrow shipped Apple
   train/export lane is now Rust-native end to end, even though later issues
   still remain around eval fidelity, telemetry, UI, and cleanup.
5. `psionic-eval` runs the held-out and benchmark-style adapter harnesses, and
   the bridge-backed runtime-smoke path proves the exported package can be
   loaded and exercised against the live Apple FM runtime. That smoke path now
   checks the exported package's base-model, tokenizer, and template lineage
   against the expected runtime compatibility profile before acceptance. The
   Apple runtime parity pass now also normalizes raw guided-generation schemas
   through `AppleFmGenerationSchema::with_title_hint(...)`, uses bounded greedy
   generation options during live eval/smoke, and backs `lookup_doc` /
   `lookup_code` eval cases with real repo retrieval instead of echo tools.
   Benchmark reports now embed the full base/adapted `EvalRunState` receipts and
   a stable paired per-case receipt layer, so each weak case carries the request
   envelope, expected output, base/adapted outputs, structured-output payloads,
   observed tool-call transcripts, and copied model-request/runtime failure
   details instead of collapsing into aggregate benchmark deltas.
6. `autopilotctl training launch ...`, `autopilotctl training watch ...`,
   `autopilotctl training export ...`, and `autopilotctl training accept ...`
   provide the shipped app-owned operator flow, while
   `autopilotctl apple-fm load ...` and `autopilotctl apple-fm attach ...`
   exercise the resulting package through the retained bridge. That operator
   flow now runs the long Apple launch pipeline in the background, persists
   typed JSONL telemetry at `<run_directory>/telemetry.jsonl`, and projects the
   same phase, heartbeat, ETA, artifact-path, resource-summary, and
   failure-context fields through desktop-control so both CLI scripts and later
   WGPUI panes can inspect the run before it completes. The legacy toolkit
   compatibility wrapper in `psionic-train` is now quarantined behind the
   non-default `legacy-apple-toolkit-oracle` feature, and the packaged Apple
   release checks run
   `scripts/release/check-psionic-apple-rust-only-gate.sh`, which fails if the
   shipped Apple operator path regresses back to toolkit-root discovery,
   Python-interpreter discovery, or authoritative toolkit shell-outs.

That last bridge-backed load step is the authoritative export-validity gate.

A `.fmadapter` directory can be:

- inventory-valid
- metadata-valid
- and still not be an Apple-valid runtime asset

On 2026-03-15, GitHub issue `#3664` added the canonical parity and acceptance
program for this boundary:

- `scripts/release/check-psionic-apple-export-parity.sh`
- `crates/psionic/fixtures/apple_adapter/TOOLKIT_ORACLE_RECIPE.md`

The train system must not treat package write success as equivalent to runtime
load success.

The shipped Apple reference lane is intentionally narrow:

- base-model weights stay frozen
- only adapter parameter groups are updated
- the current live operator precision posture is `f32_reference`
- the current live operator activation-checkpoint posture is `disabled`
- tokenizer and packing truth now come from a repo-owned Apple-compatible
  transcript preprocessor, not an Apple-exact tokenizer oracle
- the current live operator export is repo-owned and bridge-accepted for the
  narrow reference lane, with `adapter_weights.bin` emitted as the same
  64-byte-aligned Core ML blob-storage family Apple accepts rather than as raw
  concatenated fp16 bytes
- the higher-level optional follow-on is draft-model distillation, not a second
  generic full-model trainer

That is why the right current claim is "the repo now ships a Rust-native
authoritative Apple train/export lane for one narrow single-host reference
path" rather than "Psionic now has complete Rust-native distributed Apple FM
training."

### Relationship To Cluster And Distributed Training Substrate

The Apple lane already reuses real Psionic train substrate outside the narrow
execution backend itself:

- fixed-budget trainer-step execution
- reusable autodiff and optimizer layers
- dataset/tokenizer/packing contracts
- environment package bindings
- held-out eval and runtime-smoke harnesses
- training summaries, receipts, and accepted-outcome authority publication
- run-graph, orchestrator, and validator vocabulary used elsewhere in
  `psionic-train`

But the Apple lane does **not** yet consume the broader distributed-training
substrate as a live execution path:

- no real `psionic-cluster` multi-node Apple training run is claimed
- no collective-backed gradient exchange or sharded optimizer execution is used
  by the shipped Apple backend
- no production multi-device training kernel or memory-sharded Apple trainer is
  claimed
- no broader cluster scheduler is yet dispatching Apple training windows across
  multiple machines

What exists today is the reusable contract layer for those future steps:
`psionic-runtime` multi-device topology truth, `psionic-collectives`
collective planning, distributed-optimizer object models, orchestrator state,
and datastream/checkpoint movement. That substrate is meant to be reused later
for broader Psionic training lanes, including any future widened Apple path,
but it is not the current execution reality for the shipped Apple adapter
operator flow.

## Decentralized Adapter Training Program

Psionic is now explicitly planning decentralized adapter training as a
first-class train workload family.

That program sits on top of the already-retained train substrate:

- `TrainingRun`
- `TrainingStage`
- `TrainingWindow`
- `PolicyRevision`
- `RolloutArtifact`
- `TrainerBatch`
- `CheckpointPointer`
- `EvalRun`
- validator receipts and accepted-outcome authority projection

The new claim this doc freezes is not "the repo already has decentralized
adapter training."

The new claim is:

> the repo now has one canonical spec for decentralized adapter training, with
> Apple adapters as the first narrow lane and open adapter backends as the next
> generalized lane under the same control-plane vocabulary.

That widening step is now no longer only planned. `psionic-train` also owns a
first bounded non-Apple execution backend in
`crates/psionic/psionic-train/src/open_adapter.rs`. The implemented reference
target is intentionally narrow and explicit:

- admissible model family: `gpt_oss.decoder_lm_head_lora`
- adapter format: `safetensors`
- first concrete backend label: `open_adapter_backend.cuda.gpt_oss_lm_head`
- supervision shape: repo-owned hidden-state plus target-token batches
- export proof: the produced artifact roundtrips through
  `psionic-adapters::LmHeadLoraAdapterArtifact`

That is enough to make the decentralized adapter architecture honestly
non-Apple-only while still staying well short of a generalized full-model or
multi-node open-backend trainer claim.

That contract layer is no longer only planned. `psionic-train` now owns a
typed adapter-window state machine in
`crates/psionic/psionic-train/src/adapter_window.rs` that can represent one
window end to end with typed receipts for:

- assignment
- local execution summary
- upload completion
- validator disposition
- aggregation eligibility
- sealed-window aggregation or promotion

Those receipts bind adapter target identity, dataset-slice identity, source
policy revision, and source checkpoint pointer without ad hoc JSON sidecars,
and the crate now carries a runnable harness that proves the window lifecycle
from planning through reconciliation. What remains open after this step is the
live cluster, artifact, validator, authority, operator, and product projection
work in the later issue set.

The next control-plane layer is now implemented too. `psionic-train` also owns
an adapter cluster coordinator in
`crates/psionic/psionic-train/src/adapter_cluster.rs` that mirrors live
`psionic-cluster` membership and telemetry into adapter contributor eligibility,
derives deterministic contributor ranking from readiness plus capability facts,
and plans typed adapter windows with inspectable contributor-set revisions and
assignment seeds. The new reference harness proves membership churn can evict
or replace contributors for later windows without collapsing the whole run.

The worker-facing side of that program is also now real in
`crates/psionic/psionic-train/src/adapter_worker_protocol.rs`. The crate now
owns typed adapter-worker sessions, heartbeats, progress snapshots, assignment
claims, assignment acknowledgements, submission receipts, claim expiry, and
claim supersession on top of one active adapter window. Those transcripts bind
worker and session identity to window id, contribution id, source policy
revision, source checkpoint pointer, and upload-manifest expectations so late,
superseded, unauthorized, or mismatched submissions can be refused with
machine-legible outcomes instead of ad hoc local strings.

The artifact-staging layer is now real as well in
`crates/psionic/psionic-train/src/adapter_artifact_storage.rs`. That module
derives adapter-package datastream manifests from contribution payloads,
enforces manifest-digest and chunk-digest replay safety across resumable upload
sessions, registers completed contribution artifacts through the generic train
artifact controller, tracks reviewable/accepted/rejected retention posture, and
promotes sealed-window adapter state into typed checkpoint manifests plus
window-scoped checkpoint pointers. The included harness proves interrupted
uploads can resume from a committed cursor, corrupt chunks are refused without
advancing state, and the latest promoted checkpoint for a window can be
restored deterministically.

The provenance-security layer is now implemented too in
`crates/psionic/psionic-train/src/adapter_submission_security.rs`. The crate
now owns signed adapter-manifest envelopes that bind assignment digest, claim
digest, worker id, session id, auth subject, trust class, target policy
revision, target checkpoint pointer, upload expectation, upload reference, and
manifest/object digests under the worker session's submission-signing key.
Accepted contributions now preserve independently verifiable provenance bundles,
while signature mismatch, reassigned worker/session identity, and stale-session
checks surface typed reject or quarantine receipts for later validator and
aggregation stages.

The validator-owned adapter review layer is now implemented in
`crates/psionic/psionic-train/src/adapter_validation.rs`. That module consumes
submission receipts, staged artifacts, signed provenance bundles, and security
receipts; samples contributions for validator replay; emits typed
`accepted`/`quarantined`/`rejected`/`replay_required` verdicts; writes the
existing adapter-window validator and aggregation-eligibility receipts; and
seals the window with one scored summary over admitted, accepted, quarantined,
rejected, and replay-required work. Candidate window scoring now also consumes
held-out eval, benchmark aggregate summaries, and runtime-smoke eval runs, and
Apple-format windows can require runtime smoke before promotion-ready status is
true.

The first real aggregation-and-promotion path is now implemented in
`crates/psionic/psionic-train/src/adapter_aggregation.rs`. The crate now owns a
deterministic first rule for accepted adapter contributions,
`weighted_manifest_digest_merge_v1`, which preserves accepted artifact,
validator, security, provenance, and aggregation-weight lineage; emits a typed
promotion receipt; and either promotes a new `PolicyRevision` plus
`CheckpointPointer` or records an explicit held outcome when accepted work or
validator posture is insufficient. `AdapterTrainingClusterCoordinator` can now
consume that receipt, update its current input revision and checkpoint pointer,
reconcile the finished window, and plan the next window directly from the
promoted revision without local manual patching.

### First Lane, Next Lane, And Non-Goals

The first live execution lane remains the narrow single-host Apple adapter path
documented above.

The first decentralized lane should still begin with adapters, not full-model
weight updates, because the existing repo truth already has:

- adapter-only update semantics
- adapter package lineage
- benchmark and runtime-smoke validation posture
- accepted-outcome authority plumbing
- app-owned operator workflows that can expose contribution and review state

The first decentralized lane should therefore mean:

- one windowed multi-party adapter-training program
- one declared adapter target identity
- one declared dataset-slice identity per contribution
- one validator-owned disposition for each submitted contribution
- one aggregation decision that can produce a new policy revision or no-op

The first implementation has explicit non-goals:

- no world-scale synchronous all-reduce
- no full-model training claims
- no app-owned runtime or bridge logic inside Psionic train
- no generalized market/product claims before authority, validation, and
  operator truth exist
- no ad hoc JSON sidecars for contribution-state truth

### Canonical Workload Vocabulary

The decentralized adapter workload family should use the following additional
typed vocabulary on top of the generic train objects:

| Object | Purpose | Planned Scope |
| --- | --- | --- |
| `AdapterTrainingProgram` | Root program identity for one decentralized adapter-training family | one adapter target family, one policy family, one validator posture |
| `AdapterTrainingWindow` | One bounded contribution interval for one adapter target | contributor set, dataset-slice plan, policy revision in, seal state |
| `AdapterContributionAssignment` | One worker assignment into one adapter window | worker id, adapter target identity, dataset slice, replay budget |
| `AdapterContributionArtifact` | One uploaded local adapter delta or equivalent contribution bundle | manifest, delta/checkpoint pointer, local execution summary, provenance |
| `AdapterContributionReceipt` | One durable control-plane receipt for assignment, execution, upload, and disposition | worker id, window id, policy revision, validator result, aggregation eligibility |
| `AdapterAggregationReceipt` | One record of how accepted contributions were combined for one sealed window | accepted set, weights, output policy revision, checkpoint pointer |
| `PolicyPromotionReceipt` | One durable record of whether a sealed window promoted or held policy state | previous revision, next revision, acceptance basis, checkpoint lineage |

Every contribution receipt must bind:

- adapter target identity
- window id and contributor-set revision
- policy revision in
- dataset slice identity
- local execution summary
- uploaded artifact identity
- validator disposition
- aggregation eligibility decision
- resulting policy revision or no-promotion decision when the window seals

### Canonical Contribution Dispositions

The validator-owned contribution disposition vocabulary is:

| Disposition | Meaning |
| --- | --- |
| `accepted` | the contribution passed required checks and may participate in aggregation |
| `quarantined` | the contribution is retained for review but is not aggregation-eligible until explicitly released |
| `rejected` | the contribution failed required checks and is permanently excluded from aggregation |
| `replay_required` | the contribution cannot be trusted yet and must be regenerated or replayed under a fresh assignment |

These states are machine-legible control-plane truth, not UI-only labels.

### Canonical Window Flow

One decentralized adapter window should follow this explicit control flow:

1. The orchestrator plans one `AdapterTrainingWindow` with a sealed contributor
   set, dataset-slice plan, adapter target identity, and input policy revision.
2. Workers receive typed `AdapterContributionAssignment` records rather than
   free-form task text.
3. Each worker produces one `AdapterContributionArtifact` plus a local
   execution summary bound to the declared dataset slice and policy revision.
4. Datastream and artifact storage record upload completion as typed receipt
   state.
5. The validator emits one machine-legible disposition of `accepted`,
   `quarantined`, `rejected`, or `replay_required`.
6. The window seals only after the acceptance or replay policy is satisfied.
7. Aggregation consumes only aggregation-eligible accepted contributions and
   emits one `AdapterAggregationReceipt`.
8. Promotion emits one `PolicyPromotionReceipt` that either advances the policy
   revision or records an explicit no-promotion outcome.

This preserves deterministic replay because assignment, upload, disposition,
seal, aggregation, and promotion are all typed receipts on one window graph.

### Decentralized Adapter Acceptance Matrix

The repo must not claim decentralized adapter training is implemented until all
of the following rows are true:

| Claim Boundary | Required Truth |
| --- | --- |
| Spec frozen | this doc names decentralized adapter training as a first-class program, defines the object vocabulary above, and preserves the first-lane/non-goal boundary |
| Window contracts live | `psionic-train` represents adapter windows and contribution receipts as typed Rust objects without ad hoc JSON sidecars |
| Cluster selection live | contributor selection, membership, assignment, heartbeat, and window seal posture are bound to live `psionic-cluster` truth rather than local-only mocks |
| Artifact staging live | contribution uploads, manifests, delta/checkpoint pointers, and provenance are persisted through typed datastream/artifact contracts |
| Validator dispositions live | accepted, quarantined, rejected, and replay-required states are emitted by validator-owned code and preserved for replay or authority projection |
| Aggregation live | sealed windows can aggregate accepted contributions into a new policy revision or explicit no-promotion result with checkpoint lineage |
| Authority projection live | kernel/Nexus persist accepted window outcomes and policy-promotion truth as durable authority projections |
| Operator flow live | desktop and `autopilotctl` expose truthful contributor/operator state for window planning, submission, review, aggregation, and acceptance |
| Productization live | provider-substrate and compute-market claims are published only after the above control, validator, and authority rows are implemented |

Until every row above is true, the honest repo claim remains:

> single-host Apple adapter training and one bounded non-Apple open adapter
> backend are real, but decentralized adapter training is still an incomplete
> program rather than a finished productized system.

On 2026-03-15, GitHub issue `#3648` added the first repo-owned QA and
reference-program layer for this workload family in
`crates/psionic/psionic-train/src/adapter_reference_program.rs`.

That layer now makes the following acceptance proof explicit:

- one canonical two-window decentralized adapter reference run with multiple
  contributors and contributor churn between windows
- coverage for both the Apple adapter lane and the first non-Apple open
  adapter backend under the same control-plane path
- typed latency envelopes for window sealing, replay completion, and policy
  promotion
- explicit chaos rejection of stale uploads, manifest corruption, and
  replay-missing submissions before promotion

The canonical regression harness for that layer is now:

- `scripts/release/check-psionic-decentralized-adapter-reference-program.sh`

This does not close the overall decentralized adapter program by itself.
Authority projection, app-owned operator surfaces, and broader productization
rows from the acceptance matrix above still remain separate closure steps.

On 2026-03-15, GitHub issue `#3661` added the first concrete operator runbook
for the clustered follow-on:

- `crates/psionic/docs/ARCHITECTURE_EXPLAINER_CLUSTER_BRINGUP_RUNBOOK.md`

That runbook is intentionally narrow and explicit about posture:

- it preserves the distinction between today's real single-host Apple operator
  path, today's cluster rehearsal truth, and later live multi-device ambition
- it names the preferred first topology as a small homogeneous Apple lab
  cluster
- it includes an explicitly experimental Apple Metal plus NVIDIA mixed-role
  path that is useful for cluster, staging, and receipt bring-up but does not
  overclaim Apple-valid mixed-backend training

On 2026-03-15, GitHub issue `#3662` tightened the first heterogeneous
mixed-backend experiment boundary:

- Apple remains the coordinator, `.fmadapter` export, and runtime-validation
  authority host for the Apple lane
- the first concrete non-Apple participant target is the CUDA-backed open
  adapter lane identified by `open_adapter_backend.cuda.gpt_oss_lm_head`
- mixed Apple plus NVIDIA participation is therefore shared at the cluster,
  artifact, validator, and replay layers first, not overclaimed as symmetric
  Apple training

## Canonical Train Objects

The full train system needs a formal object model. Today only some of these
objects have concrete repo types; the rest are planned and should become the
stable vocabulary for train-class execution.

| Object | Purpose | Current Repo Status |
| --- | --- | --- |
| `TrainingRun` | Root identity for one training program | `implemented_early` |
| `TrainingStage` | One named phase such as SFT, agentic SFT, or RL | `implemented_early` |
| `TrainingWindow` | One synchronized contribution or trainer interval with its own contributor set and transition state | `implemented_early` |
| `TrainerStep` | One optimizer update over one trainer batch | `implemented_early` |
| `PolicyRevision` | Versioned policy or weight state used by workers and trainer | `implemented_early` |
| `RolloutArtifact` | One worker-produced trajectory or completion bundle | `implemented_early` |
| `TrainerBatch` | One accepted batch of rollout or corpus inputs for a trainer step | `implemented_early` |
| `EnvironmentPackage` | One versioned environment definition used by training and eval | `implemented_early` |
| `BenchmarkPackage` | One validator-owned packaged benchmark or reference evaluation profile | `implemented_early` |
| `EvalRun` | One online or offline evaluation execution | `implemented_early` |
| `CheckpointPointer` | One stable pointer to the latest accepted checkpoint for a run, stage, or window | `implemented_early` |
| `CheckpointManifest` | One shard, digest, writer, and durability manifest for a checkpoint flush | `implemented_early` |
| `Checkpoint` | Recoverable training state and lineage anchor | `partial` |
| `ValidatorVerdict` | Verification result attached to one rollout, batch, or eval artifact | `implemented_early` |

Today the concrete object vocabulary is strongest around:

- `TrainingCheckpointReference`
- `TrainingRecoveryContext`
- `TrainingDeviceMeshContext`
- `TrainingCollectiveContext`
- `DatastreamManifest` and `DatastreamManifestRef`

Current checkpoint substrate is carried today by
`TrainingCheckpointReference`, explicit `CheckpointPointer` and
`CheckpointManifest` contracts, plus checkpoint-scoped datastream manifests.

The rest of the train object model still needs to be built explicitly.

What is still missing most clearly from the current vocabulary is:

- deeper checkpoint lineage policy such as checkpoint retention tiers,
  cross-window promotion rules, and cold-restore governance
- broader `ValidatorVerdict` families for trainer-batch and eval-class artifacts

### Current `RolloutArtifact` Shape

`RolloutArtifact` now exists in early form inside `psionic-train`. The current
shape already includes at least:

- `worker_id`
- `policy_revision`
- `environment_ref@version`
- `task_id` or task digest
- `token_ids`
- `logprobs`
- reward or rubric outputs
- termination reason
- proof or validator reference fields
- stable `artifact_digest`

## Current State At A Glance

| Subsystem | Current Status | What Is Real Today |
| --- | --- | --- |
| Runtime training truth | `implemented_early` | `TrainingRecoveryContext`, checkpoint refs, elastic-membership context, device-mesh context, collective context |
| Datastream | `implemented_early` | resumable manifests, checkpoint or dataset bindings, policy-weight control refs, freshness windows, and delivery receipts |
| Collectives | `implemented_early` | elastic mesh observation, bandwidth-aware local/global sync planning, transport-feedback replanning, and benchmark-gated quantized collective policy |
| Train session state | `implemented_early` | membership observation, async checkpoint state, durability transitions, live-recovery planning |
| Data contracts | `implemented_early` | `psionic-data` now owns versioned dataset manifests, tokenizer digests, split declarations, resumable iteration cursors, long-context packing policies, and Apple adapter JSONL import or validation with typed tool-schema augmentation plus tokenizer/prompt-shaping packing lineage |
| Adapters | `implemented_early` | adapter identity, package manifests, hosted adapter binding lineage, and first Apple `.fmadapter` reader/writer plus file-inventory validation |
| Sandbox for RL/train workloads | `implemented_early` | bounded execution, background jobs, warm reusable pools, staged loop inputs, pool acquisition receipts, and repeated agentic iteration receipts now exist in `psionic-sandbox` |
| Training core | `implemented_early` | `psionic-train` now has a typed fixed-budget trainer-step loop, `psionic-ir` now provides reusable reverse-mode autodiff plus explicit detach/training-mode gradient semantics beneath it, the repo-owned Apple adapter execution backend now turns packed Apple dataset batches into adapter-only gradient batches for that loop, the first higher-level Apple SFT lane closes the path through typed training summary plus `.fmadapter` export, and an explicitly separate optional Apple draft-model distillation lane now emits paired draft payloads plus latency or acceptance metadata; the crate also now owns a first non-Apple open adapter backend for `gpt_oss.decoder_lm_head_lora`, producing loadable LM-head LoRA `safetensors` artifacts from bounded hidden-state supervision under the same fixed-budget core; parameter-group scaling semantics, scheduler bindings, optimizer state/residency, step telemetry, model-IO roundtrip, and checkpoint restore lineage remain explicit over gradient batches |
| Training run graph | `implemented_early` | `psionic-train` now owns typed runs, contributor-set revisions, topology revisions, persistent participant ranking, heartbeats, departures, and window transitions |
| Orchestrator | `implemented_early` | `psionic-train` now owns typed window-control, assignment posture, rollout-assignment refs, rollout-admission receipts, bounded off-policy freshness budgets, rollout-worker heartbeats, claims, upload receipts, and trainer-batch assembly requests over the run graph |
| Environment ABI | `implemented_early` | `psionic-environments` now owns the package ABI, versioned key, workload/policy/difficulty/benchmark package shape, tool/rubric contracts, deterministic runtime session state machine, and a reusable Apple adapter train/eval/benchmark bundle with typed runtime refs plus train/eval parity receipts, while registry and authority truth remain in kernel/Nexus |
| Eval runtime | `implemented_early` | `psionic-eval` now owns held-out eval runs, rubric-scored sample/runtime contracts, benchmark packages, repeat-run aggregation, local validator simulation, and Apple adapter held-out plus benchmark harnesses with structured-output, tool-call, and runtime-smoke receipts, while kernel/Nexus still own canonical eval-run authority truth |
| Synthetic-data flows | `partial_outside_psionic` | synthetic-data job creation, append, finalize, and verification flows exist in kernel/Nexus, but no Psionic-native generation runtime exists yet |
| Rollout artifacts | `implemented_early` | `psionic-train` now has checkpoint-aware policy revisions, proof-bearing rollout artifacts, rollout-admission receipts, bounded stale-rollout pruning, and deterministic trainer-batch assembly with policy-lineage digests |
| Validator-aware RL verification | `implemented_early` | `psionic-train` now owns rollout-verification bundles, replay or duplicate detection, sampled benchmark checks, and typed validator verdicts; broader service productization is still later |
| Decentralized adapter window contracts | `implemented_early` | `psionic-train` now owns typed adapter-window receipts and a runnable window harness covering assignment, execution, upload, validator disposition, aggregation eligibility, seal, aggregation, and reconcile over one adapter-targeted window |
| Decentralized adapter cluster selection | `implemented_early` | `psionic-train` now owns a cluster-backed adapter coordinator that mirrors live membership and telemetry into contributor eligibility, deterministic ranking, contributor-set revisions, assignment seeds, and churn-safe window replanning |
| Decentralized adapter worker protocol | `implemented_early` | `psionic-train` now owns typed adapter-worker sessions, heartbeats, claim/ack flows, progress telemetry, superseded-claim retries, and contribution submission receipts bound to policy revision, checkpoint pointer, and upload expectations for one active adapter window |
| Decentralized adapter artifact staging | `implemented_early` | `psionic-train` now owns resumable adapter contribution uploads, manifest/chunk verification, typed contribution-artifact receipts, disposition-aware retention windows, and promoted window checkpoint manifests plus pointers for deterministic restore |
| Decentralized adapter provenance security | `implemented_early` | `psionic-train` now owns signed manifest envelopes, worker/session/auth-subject binding, independently verifiable accepted provenance bundles, and typed reject or quarantine receipts for signature mismatch, reassignment, or stale-session cases |
| Decentralized adapter validator and window scoring | `implemented_early` | `psionic-train` now owns sampled replay verification, typed validator dispositions, window sealing summaries over admitted/accepted/quarantined/rejected/replay-required work, and candidate held-out/benchmark/runtime-smoke gating for promotion readiness |
| Decentralized adapter aggregation and promotion | `implemented_early` | `psionic-train` now owns a deterministic accepted-delta aggregation rule, typed promotion receipts with artifact/validator/security/provenance lineage, hold-vs-promote gating, and coordinator-side adoption of the promoted revision plus checkpoint pointer for the next window |

## Current Crate Ownership

The current train-relevant ownership split in Psionic is:

- `psionic-runtime`
  - reusable runtime truth for training recovery, device meshes, collectives,
    and work classes such as `CollectiveStep` and `CheckpointFlush`
- `psionic-datastream`
  - resumable transport for datasets, checkpoints, served artifacts, and
    adapter packages
- `psionic-data`
  - versioned dataset manifests, tokenizer digests, split declarations,
    streamed iteration contracts, long-context packing rules, and Apple
    adapter dataset import or validation with typed schema/tool augmentation
- `psionic-collectives`
  - elastic mesh observation, local/global sync planning, transport-feedback
    replanning, and benchmark-gated collective policy
- `psionic-environments`
  - environment package ABI, execution entrypoints, tool and rubric hooks,
    artifact expectations, versioned dataset bindings, deterministic runtime
    sessions, and reusable Apple adapter train/eval/benchmark bundle helpers
- `psionic-eval`
  - held-out eval runs, rubric-scored sample/runtime contracts, benchmark
  packages, repeat-run aggregation, operator-local validator simulation, and
  Apple adapter held-out/benchmark/runtime-smoke harnesses
- `psionic-train`
  - training-session truth for checkpointing, live recovery,
    elastic-membership posture, typed run graphs, contributor-set revisions,
    window lifecycle, the fixed-budget training-core reference loop, the
    repo-owned Apple adapter reference execution backend, the higher-level
    Apple SFT/export lane, the optional Apple draft-model distillation lane,
    orchestrator state, and RL-facing rollout or batch contracts
- `psionic-adapters`
  - adapter package identity, Apple `.fmadapter` parsing or writing, file
    inventory validation, and hosted binding lineage
- `psionic-sandbox`
  - bounded sandbox execution substrate and background-job lifecycle
- `psionic-cluster`
  - durable ordered-state, cluster admission, catch-up, and topology truth

The broader OpenAgents tree now also has train-adjacent authority surfaces
outside Psionic for:

- environment package descriptors and registry behavior
- compute evaluation-run, training-run, and accepted-outcome lifecycle
- narrow Apple adapter-hosting and Apple-training provider/market projection
  surfaces
- synthetic-data job and verification lifecycle

This is already a meaningful substrate split. The missing work is higher in the
stack.

## What Is Implemented Today

### 1. Runtime-level training truth already exists

`psionic-runtime` already has typed training-class truth surfaces. The most
important ones are:

- `TrainingRecoveryPosture`
  - `SteadyState`
  - `LateJoinPending`
  - `Recovering`
  - `ElasticReconfiguration`
  - `AsyncCheckpointInFlight`
- `TrainingCheckpointAvailability`
  - `None`
  - `AsyncWriteInFlight`
  - `Durable`
- `TrainingElasticMembershipContext`
  - membership epoch
  - cluster-state digest
  - topology digest
  - active, joining, draining, and offline node sets
- `TrainingCheckpointReference`
  - checkpoint family
  - stream id
  - manifest digest
  - object digest
  - writer node
  - membership epoch and topology digests
  - optional logical step and durability timestamp
- `TrainingRecoveryContext`
  - current posture
  - checkpoint availability
  - elastic-membership facts
  - optional latest checkpoint
  - recovering and late-joiner node ids
- `TrainingDeviceMeshAxis`
  - data-parallel, tensor-parallel, pipeline-parallel, and expert-parallel axes
- `TrainingDeviceMeshContext`
  - mesh id, revision, backend, communication class, members, and axes
- `TrainingCollectiveContext`
  - collective kind
  - quantization mode
  - payload bytes
  - wire-byte estimate
  - benchmark justification

This matters because the train system does not start from nothing. The runtime
already has a typed language for recovery, checkpoints, meshes, and collectives.

### 2. Datastream already owns resumable training-class artifact movement

`psionic-datastream` is not training-specific, but it already covers several
training-critical artifact families.

Its subject model already includes:

- `TokenizedCorpus`
- `EvalBundle`
- `Checkpoint`
- `PolicyWeights`
- `ServedArtifact`
- `AdapterPackage`

Its manifests already support:

- payload digesting
- stable chunk descriptors
- dataset bindings
- checkpoint bindings
- policy-weight bindings
- control-plane-visible mirror metadata
- resumable transfer cursors
- restart-safe client progress
- final delivery receipts

That means the train system already has a real substrate for:

- dataset shard transport
- checkpoint transport
- policy-weight shard transport and lightweight control-plane refs
- eval-bundle movement
- adapter-package distribution

What is still missing is not "a data plane exists or not." The missing work is
the broader lifecycle policy over that data plane: richer retention classes,
cross-region mirror governance, and tighter integration with higher-level
orchestrator freshness rules.

### 3. Collective planning already exists

`psionic-collectives` already implements a real, inspectable collective
planning substrate.

The important current pieces are:

- `ElasticCollectivePlanner`
- `CollectiveMeshMember`
- `QuantizedCollectiveBenchmark`
- `CollectiveTransportFeedback`
- `CollectiveSyncCadencePolicy`
- `observe_mesh`
- `record_benchmark`
- `plan_collective`
- `observe_transport_feedback`
- `plan_sync`

The current planner already does several important things honestly:

- validates that declared mesh axes match member count
- ensures mesh members are actually active in the current membership set
- increments mesh revision only when mesh truth changes
- requires explicit benchmark approval before planning a quantized collective
- records transport feedback and surfaces typed replan triggers when bandwidth,
  latency, stream pressure, or mesh revision cross policy boundaries
- plans local subgroup sync separately from full-mesh sync when degraded
  transport and explicit subgroup topology justify it
- emits a `CollectiveExecutionPlan` with:
  - runtime-visible collective posture
  - explicit ring handoffs
  - a low-level `RuntimeWorkItem`
- emits a `CollectiveSyncCadenceReceipt` with:
  - cadence class
  - next global sync step
  - selected quantization
  - transport degradation posture
  - typed replan triggers

This is already enough to say Psionic has training-class collective truth.

It is not enough to say Psionic has a complete distributed optimizer or
end-to-end trainer.

### 4. `psionic-train` already implements session truth for checkpointing and recovery

`psionic-train` currently owns the most concrete part of the train system that
exists today.

Its public API centers on `TrainingSessionState`, which already supports:

- `new`
- `latest_durable_checkpoint`
- `active_checkpoint_write`
- `observe_membership`
- `begin_async_checkpoint`
- `mark_checkpoint_durable`
- `plan_live_recovery`

What that means in practice:

- Psionic can derive elastic-membership epochs from authoritative cluster truth.
- Psionic can begin an async checkpoint only from a checkpoint-scoped
  datastream manifest and only when the writer node is a known ready member.
- Psionic can surface in-flight checkpoint flush work as a typed runtime work
  item.
- Psionic can transition a checkpoint from writing to durable and update the
  durable recovery posture.
- Psionic can derive explicit live-recovery plans for recovering nodes and late
  joiners.

The current recovery action set is already meaningful:

- `ResumeFromDurableCheckpoint`
- `FenceRecoveringNodes`
- `StageCheckpointForLateJoiners`
- `RebalanceWorldSize`
- `BlockUntilDurableCheckpoint`
- `ContinueSteadyState`

That is real train-substrate behavior, not just placeholder nouns.

### 5. Adapter lineage already exists for later train outputs

`psionic-adapters` is not the core training loop, but it is relevant because a
train system eventually needs to emit attributable artifacts.

The adapter subtree already owns:

- `AdapterArtifactIdentity`
- `AdapterPackageManifest`
- target-family and residency semantics
- hosted binding lineage for adapter-backed serving

This means Psionic already has an artifact vocabulary for one class of training
outputs beyond full checkpoints.

### 6. Sandbox substrate exists, and the RL-oriented shape is now early rather than absent

`psionic-sandbox` already owns:

- runtime detection
- profile realization
- bounded job execution
- background jobs
- file transfer
- warm reusable pools
- staged loop inputs
- pool acquisition receipts
- repeated agentic iteration receipts
- execution receipts

This is enough to support bounded compiled runners plus early RL/post-training
iteration contracts.

It is not yet the mature high-throughput RL/post-training sandbox shape. The
remaining gaps are:

- productionized RL throughput and pool tuning
- broader environment-owned lifecycle and policy integration
- stronger operator and security hardening for long-running train workloads

### 7. Environment, eval, training-authority, and synthetic-data truth now spans Psionic runtime crates and authority-owned kernel surfaces

The recent issue closures matter because they changed both Psionic and the
broader system around it.

The tree now has Psionic-native execution crates for:

- environment package ABI and deterministic runtime sessions in
  `psionic-environments`
- held-out eval runs, benchmark packages, repeat-run aggregation, and local
  validator simulation in `psionic-eval`
- one repo-owned Apple adapter training execution lane in `psionic-train`

The tree also has broader OpenAgents support for:

- environment package descriptors and registry behavior
- environment refs bound into compute products and delivery proofs
- evaluation-run creation, sample ingestion, and finalize flows
- training-policy registration, training-run create/finalize flows, and
  accepted-outcome publication
- narrow Apple adapter-hosting capability publication plus matching
  compute-market truth surfaces
- synthetic-data job creation, append, finalize, and verification flows

Those capabilities currently live in kernel/proto and Nexus-control surfaces.

So the accurate reading is:

- Psionic now has native environment and eval runtime clients plus one
  repo-owned Apple training lane inside the compute substrate
- the larger platform owns the canonical authority truth for environment, eval,
  training-run, and accepted-outcome records
- provider and market surfaces now expose one narrow Apple training and
  adapter-hosting projection on top of that authority truth
- synthetic-data still remains `partial_outside_psionic` because there is no
  Psionic-native generation runtime yet

## What Psionic Can Honestly Claim Today

Today Psionic can honestly claim all of the following:

- training-class execution now has typed recovery, checkpoint, mesh, and
  collective truth in reusable crates
- clustered training recovery can be reasoned about with replay-safe session
  state rather than ad hoc logs
- checkpoint transport has a resumable data-plane substrate with delivery
  receipts
- collective planning already has benchmark-gated quantization and explicit mesh
  revisions
- fixed-budget trainer-step execution is real, with explicit optimizer-state
  ownership, residency transitions, and step telemetry
- reusable autodiff plus explicit detach or no-grad semantics now live in
  `psionic-ir` rather than trainer-private code
- reusable SGD, Adam, AdamW, LARS, and LAMB primitives plus distributed-
  optimizer contracts now live in `psionic-train`
- rollout artifacts, trainer-batch assembly, policy revisions, and
  validator-aware verification are first-class typed contracts
- environment ABI and held-out eval runtime now exist in reusable Psionic
  crates
- sandbox execution now supports warm reusable pools and repeated agentic
  iteration receipts
- training-related artifact lineage is now materially first-class data rather
  than opaque side files
- the first repo-owned Apple adapter SFT lane is real in `psionic-train`, and
  the optional Apple draft-model distillation lane is now separate typed
  behavior instead of being implied by one generic training path
- the broader OpenAgents stack now has authority-layer environment, eval,
  training-run, and accepted-outcome flows that Psionic can target as
  execution clients
- the desktop app now has a truthful Apple training operator flow on top of the
  Psionic substrate, including explicit launch, eval, export, and
  accepted-outcome publication boundaries
- the broader stack also projects one narrow Apple adapter-hosting and
  Apple-training truth path into provider and compute-market surfaces, without
  pretending that broader train procurement is complete

That is a meaningful base.

## What Psionic Cannot Honestly Claim Yet

Psionic cannot honestly claim any of the following yet:

- full production-scale Rust-native model training across real multi-device
  runtime kernels
- full production-scale Rust-native RL or post-training throughput
- broad autodiff coverage across every future backend-extension and training op
- true multi-device execution kernels and ZeRO or FSDP transport and partition
  exchange
- fully mature checkpoint retention, promotion, and cold-restore governance
- broad kernel-backed accepted-outcome authority for every train artifact and
  lifecycle beyond the current Apple adapter path
- full security hardening, chaos coverage, and operator lifecycle for the train
  stack
- a broad provider-market training family or buyer-facing procurement surface
  on top of the current Apple reference lanes
- the broader research-loop or productization program beyond the current
  reference runs

Those are still planned.

## The Gap, Precisely

The gap is no longer "there is no train subtree."

The gap is:

> Psionic now has early trainer, orchestrator, rollout, environment, eval,
> validator, and reusable framework-core gradient or update substrate, but it
> still lacks the runtime breadth, hardening, and operator or product layers
> required for a complete distributed train system.

That gap is the main planning target for the rest of this doc.

## Target Train System

The target Psionic train system should be six explicit subsystems.

### 1. Training core

Owns:

- training graph or backward substrate
- optimizer state
- optimizer-state residency, offload, and prefetch policy
- gradient update policy
- checkpoint save and restore
- trainer step loop
- step-level training telemetry such as grad, update, and parameter norms

This is the engine that does the actual learning work.

### 2. Orchestrator

Owns:

- participant roles
- training-window creation, seal, score, and reconcile transitions
- rollout scheduling
- deterministic assignment for contributor, batch, and eval slices
- batch assembly
- off-policy budgeting
- policy revision tracking
- stage transitions
- online eval interleaving

This is the control plane for the train system.

### 3. Data plane

Owns:

- dataset transport
- checkpoint transport
- policy-weight broadcast
- eval-bundle transport
- artifact freshness and replay posture

This extends the current `psionic-datastream` substrate.

### 4. Environments and eval

Owns:

- environment package ABI
- benchmark package and validator-owned reference benchmark profiles
- rollout execution contracts
- tool and multi-turn abstractions
- reward and rubric contracts
- repeat-run scoring and robust aggregation rules
- operator-local validator simulation against the same packaged benchmark
  environment
- offline and online eval over the same environment definition

This is where environment-bound training becomes honest.

### 5. Validation and adjudication

Owns:

- rollout-verification bundles
- cheap universal checks
- sampled expensive checks
- stale or malformed rollout rejection
- timer, token-accounting, and final-state verification where a benchmark or
  validator package requires them
- declared execution-strategy verification for benchmark-class workloads
- validator verdict artifacts

This is the integrity loop for untrusted or semi-trusted rollout workers.

### 6. Operator and authority integration

Owns:

- training receipts
- topology and checkpoint inspection
- validator posture inspection
- environment version visibility
- accepted-outcome export into market or kernel truth when appropriate

This is how the train system becomes operable instead of remaining a research
toy.

## Canonical Planned Role Split

The full train system should separate these roles explicitly.

### Trainer

Trusted execution responsible for:

- reading trainer batches
- applying gradient updates
- producing new checkpoints or policy revisions
- emitting step and checkpoint receipts

### Orchestrator

Trusted control plane responsible for:

- scheduling rollouts
- assigning workers
- maintaining persistent participant ranking
- selecting bounded contributor sets from a wider active population
- enforcing freshness windows
- assembling trainer batches
- coordinating evaluation
- feeding the trainer the right artifacts

### Rollout workers

Untrusted or semi-trusted execution responsible for:

- generating trajectories or outputs against a declared policy revision
- returning typed rollout artifacts
- attaching enough metadata for validator review

### Validators

Integrity checkers responsible for:

- universal schema checks
- sampling-shape checks
- termination checks
- stale-policy checks
- duplicate or copycat detection
- contribution normalization and ranking feedback
- sampled high-cost verification when economics justify it

### Environment runtime

Trusted execution substrate responsible for:

- package loading
- stateful multi-turn task execution
- tool invocation
- reward or rubric application
- sandbox-bound execution where required

### Data-plane services

Responsible for:

- checkpoint and weight transfer
- resumable corpus delivery
- manifest and digest verification
- freshness and retention policy

### Contributor Selection And Ranking

The mature train system should treat active participants and contributing
participants as different sets.

That means:

- the system may keep a wider population admitted and heartbeat-visible
- only a bounded contributor set should actually produce work in a given round,
  interval, or trainer window
- contributor selection should consider freshness, persistent ranking, topology,
  and diversity rather than only "who asked first"
- duplicate or copycat behavior should reduce effective contribution weight and
  feed back into future participant ranking

This is the cleanest way to keep elastic membership open without letting every
active participant distort batch quality or network cost.

### Control Plane Versus Heavy Artifact Plane

The train control plane should not carry the heavy payloads.

The intended split is:

- the orchestrator, validators, and operator surfaces exchange run ids,
  artifact refs, digests, policy ids, and receipts
- checkpoints, policy weights, datasets, rollout payloads, and eval bundles
  move through the heavy artifact plane in `psionic-datastream`

This keeps control messages lightweight and replayable while the actual bytes
stay in the resumable artifact substrate.

## Canonical Planned Lifecycle

The mature Psionic train lifecycle should look like this:

1. A training run is created with stable run identity, policy, environment, and
   checkpoint lineage.
2. The orchestrator forms or revises the participant topology, contributor set,
   and current `TrainingWindow`.
3. The collective planner materializes the device mesh and collective posture.
4. The heavy artifact plane stages the active checkpoint, policy weights, and
   dataset or environment artifacts while the control plane carries only refs,
   digests, and policy posture.
5. Only the selected contributor subset begins rollout or trainer work under
   explicit policy, assignment, and freshness constraints.
6. The window transitions through explicit control states such as `planned`,
   `active`, `sealed`, `scored`, and `reconciled` as work is accepted and
   judged.
7. Rollout artifacts or trainer-step inputs are validated and assembled into
   trainer batches.
8. The trainer advances one or more steps and emits step-level metrics,
   receipts, and optional checkpoints.
9. Async checkpoint flushes begin and later transition to durable state.
10. Recovery, late join, reconfiguration, or eviction events update the run
   topology and checkpoint posture.
11. Online and offline eval may run against the same environment contract or
    benchmark package contract.
12. Accepted outcomes produce durable train and eval receipts and later, when
    market-relevant, can flow into kernel truth.

The current repository implements only pieces of steps 2, 3, 4, 9, and 10.

## Planned Run State Machine

The mature train system should give operators and controllers a small explicit
run-state machine.

| `TrainingRunStatus` | Meaning |
| --- | --- |
| `planned` | run identity exists but execution has not started |
| `initializing` | artifacts, participants, and execution substrate are still being prepared |
| `active` | trainer and rollout work are progressing normally |
| `recovering` | the run is reconfiguring or resuming from checkpoint-backed state |
| `paused` | the run is intentionally halted without being terminal |
| `completed` | the run reached a successful terminal outcome |
| `failed` | the run reached a terminal failure outcome |

The runtime and operator surfaces should not infer these states indirectly from
scattered logs. They should be first-class train truth.

## Training Time Semantics

Training execution depends on explicit time boundaries.

The most important ones are:

- policy freshness windows
- rollout expiry windows
- checkpoint cadence
- contributor reselection intervals
- validator sampling or adjudication intervals
- environment timeout limits
- sandbox reuse and pool lifetime limits

These time boundaries sit above the generic execution timing defined in
`ARCHITECTURE.md` and should be recorded in train policy and receipts where
they affect acceptance or rejection.

## Canonical Train Receipts

OpenAgents is receipt-first, so the train system needs explicit receipt
families rather than vague references to "logs" or "artifacts."

Today the repo already has some lower-level receipt substrate:

- `DatastreamDeliveryReceipt`
- sandbox execution receipts
- runtime execution-proof bundles
- checkpoint and recovery contexts that can feed later receipts

The first train-specific receipt family now exists through
`RolloutAdmissionReceipt`, but the mature train system should still emit at
least these broader receipts.

| Receipt | Purpose | Minimum Contents |
| --- | --- | --- |
| `TrainingRunReceipt` | One durable summary for a full run or run stage | run id, stage id, policy ids, environment refs, checkpoint lineage, validator posture, final outcome |
| `TrainingWindowReceipt` | One durable record for one contributor or trainer window transition | run id, stage id, window id, contributor-set revision, policy revision, transition state, validator posture |
| `TrainerStepReceipt` | One accepted optimizer step | run id, stage id, step id, trainer batch digest, policy revision in and out, optimizer policy, checkpoint linkage |
| `CheckpointReceipt` | One checkpoint creation or durability event | run id, stage id, checkpoint family, manifest digest, object digest, writer identity, durability state |
| `RolloutReceipt` | One rollout artifact and its acceptance result | run id, worker id, policy revision, environment version, rollout digest, reward and termination posture, acceptance result |
| `ValidatorReceipt` | One validator verdict over a rollout, batch, or eval artifact | validator policy id, sampled or universal check class, referenced artifact digests, verdict, reason codes |
| `EvalReceipt` | One online or offline evaluation result | eval run id, environment version, rubric version, policy revision, artifact digests, score summary |

The most important design rule is simple:

> every economically or operationally important train event should have a typed
> receipt family, not only a log line or an in-memory state transition.

Train objects define the durable execution vocabulary; receipts record accepted
state transitions and outcomes over those objects.

## Policy Surfaces

The full train system should make the configurable policy surfaces explicit.
The spec should say not only what happens, but what operators and higher-level
controllers are allowed to tune.

| Policy Surface | What It Governs |
| --- | --- |
| `TrainingPolicy` | trainer step budget, training-window cadence, checkpoint cadence, optimizer posture, gradient clipping, contributor caps, stage transitions, halt policy |
| `EnvironmentPolicy` | admissible environment packages, tool access, state persistence, reward and rubric posture |
| `ValidatorPolicy` | universal checks, sampled expensive checks, stale-policy tolerances, duplicate-detection posture, contribution normalization, benchmark verification posture, rejection posture, penalty posture |
| `CollectivePolicy` | mesh layout, sync cadence, quantization mode, replan triggers, communication class |
| `SandboxPolicy` | allowed profiles, warm-pool behavior, runtime limits, filesystem or network posture, retry behavior |
| `ArtifactPolicy` | artifact freshness windows, retention classes, replay rules, archival posture, provenance requirements |

Current repo truth only covers a small piece of this policy surface directly:

- collective quantization approval, benchmark posture, sync cadence, and
  transport thresholds
- cluster admission and readiness posture
- checkpoint durability posture
- sandbox profile realization

Most train policy remains to be formalized.

### Example Policy Values

The policy surfaces above become easier to reason about when rendered with
concrete examples.

| `TrainingPolicy` Field | Example Value |
| --- | --- |
| `max_policy_drift` | `3 revisions` |
| `checkpoint_interval` | `1000 steps` |
| `gradient_clip_norm` | `1.0` |
| `halt_on_entropy_drop` | `true` |
| `max_rollout_age_ms` | `30000` |
| `max_contributing_workers` | `256` |

### Policy Revision Propagation

Policy revisions should propagate through the data plane as staged artifacts,
not as implicit mutable state.

The intended model is:

- the trainer emits a new policy revision or checkpoint-backed weight state
- the revision is published through `psionic-datastream` as a staged artifact
- the orchestrator enforces freshness and admissibility before assigning work
- rollout workers and evaluators must bind their outputs to the specific policy
  revision they consumed

This keeps policy lineage replay-safe and validator-reviewable.

Control-plane coordination should carry refs, digests, and policy ids rather
than embedding the heavy policy payloads directly in orchestration messages.

## Training Failure Semantics

The train system needs explicit failure handling, not only a list of failure
classes. The table below describes the expected control policy for the mature
system.

| Failure Type | Expected System Response |
| --- | --- |
| rollout worker crash | replay or reassign the rollout task and mark prior claim incomplete |
| stale or mismatched policy revision | reject the rollout artifact and emit a stale-policy receipt |
| duplicate or copied rollout | reject or deweight the artifact, emit duplicate-detection reason codes, and update participant ranking |
| validator rejection | discard or quarantine the referenced rollout or batch and record reason codes |
| checkpoint flush failure | block any state transition that requires durability and keep the run in non-durable posture |
| orchestrator crash | resume from durable orchestrator state and latest accepted checkpoint lineage |
| trainer crash | restart from the latest durable checkpoint and replay admissible pending control-plane state |
| environment package mismatch | reject execution before rollout start and emit environment-mismatch reason codes |
| sandbox runtime failure | terminate the affected task, record runtime and profile identity, and apply retry or quarantine policy |
| topology shock or node loss | trigger elastic reconfiguration, recovery planning, and possibly world-size rebalance |
| datastream interruption | resume from the last committed cursor rather than restart blind transfer |

The system should never collapse these into one generic "training failed"
outcome. Failure handling is part of train truth.

Orchestrator durability and trainer durability are related but distinct; loss
of one must not silently imply loss of the other.

## Security Model

The train system explicitly allows for partially trusted and untrusted roles, so
the threat model belongs in the spec and not only in later issue descriptions.

| Threat | Mitigation Direction |
| --- | --- |
| malicious rollout workers | validator sampling, schema checks, stale-policy rejection, worker admission controls |
| artifact poisoning or tampering | manifest digests, object digests, provenance requirements, signed artifacts where policy requires |
| checkpoint tampering | datastream manifest verification plus checkpoint-family and writer identity linkage |
| environment compromise | signed or pinned packages, sandbox policy, version pinning, package admissibility policy |
| policy drift | explicit policy revisions, freshness windows, off-policy budget enforcement |
| copied or replayed rollouts | duplicate detection, artifact-digest lineage, contribution normalization, and participant-ranking penalties |
| worker spam or flooding | task-claim limits, admission control, rate limiting, and orchestrator-side pruning |
| orchestrator inconsistency | durable orchestrator state and replay-safe receipts |
| validator abuse or misconfiguration | validator policy versioning, sampled check receipts, adjudication reason codes |

The current repo already helps here in a limited way through:

- manifest and chunk digests in `psionic-datastream`
- explicit checkpoint identity and writer linkage in `psionic-runtime`
- benchmark-gated collective posture in `psionic-collectives`
- bounded profile and execution receipts in `psionic-sandbox`

The broader train security model is still planned.

## Train Artifact Retention Model

Retention policy affects reproducibility, cost, and later authority linkage, so
it should be named now even before enforcement exists.

| Artifact Class | Expected Retention |
| --- | --- |
| durable checkpoints | long-term or archival, because they anchor recovery and promotion lineage |
| trainer-step receipts | long-term, because they define accepted optimization history |
| rollout artifacts | medium-term by default, with longer retention for sampled, disputed, or promoted artifacts |
| validator receipts and proof refs | long-term, because they justify acceptance or rejection outcomes |
| eval summaries | long-term, because they anchor quality and release decisions |
| raw sandbox traces and transient logs | short-term by default unless attached to an incident or dispute |

The retention table does not imply the implementation already exists. It defines
the operating model the train stack should eventually enforce.

## What The Intellect Papers Change For Psionic

The March 13 audit remains directionally correct. The useful lessons from the
Intellect papers are still these.

### From INTELLECT-1

Psionic should take:

- explicit elastic topology as first-class truth
- join and recovery modes as policy rather than ad hoc behavior
- heartbeat and explicit departure semantics
- bandwidth-aware background replanning
- quantized sync only when benchmark-justified and receipt-bearing

Psionic should not copy:

- their exact Python or PyTorch stack
- their exact transport stack
- one specific pretraining topology as permanent architecture truth

### From INTELLECT-2

Psionic should take:

- trainer, orchestrator, rollout worker, and validator as distinct roles
- policy-weight distribution as its own data plane
- untrusted rollout validation with cheap universal checks and sampled expensive
  checks
- explicit off-policy budgets
- first-class curriculum and filtering
- instability telemetry as product truth

Psionic should not copy:

- one GRPO recipe as the permanent train contract
- one relay or firewall model as the only architecture
- one economic or ledger substrate as the product base layer

### From INTELLECT-3

Psionic should take:

- environment packages as independent products
- one environment contract for training and eval
- multi-turn and tool-using environments as first-class abstractions
- RL-oriented sandbox throughput
- stage transitions from SFT to agentic SFT to RL
- orchestrator state as core product truth

Psionic should not copy:

- their exact Python environment module system
- their exact Kubernetes control plane
- their exact optimizer or MoE decisions as architecture truth

## All-Rust Implication

If OpenAgents means "no Python trainer and no Python environment system," then
the completion bar is high.

An honest all-Rust Psionic train system now exists in early form across all of
these layers inside the Rust subtree:

- training core
- optimizer ownership
- rollout artifacts
- environment ABI
- data and corpus contracts
- eval runtime
- compiled runner and crash boundary

The completion bar is still high, though.

Psionic cannot honestly claim a finished all-Rust train system until
multi-device execution kernels, broader autodiff or operator coverage, mature
environment execution at scale, hardening, and operator-grade lifecycle
management all exist inside the Rust subtree.

## Planned Crate Shape

The most likely mature crate shape is:

- `psionic-train`
  - training core, run graph, checkpoint lineage, trainer state, orchestrator
    contracts
- `psionic-collectives`
  - mesh and collective planning, quantized sync policy
- `psionic-datastream`
  - dataset, checkpoint, policy-weight, and eval-bundle transport
- `psionic-eval`
  - shared online and offline evaluation runtime
- `psionic-data` or `psionic-datasets`
  - dataset manifests, tokenizer state, splits, packing, and curriculum facts
- `psionic-environments`
  - environment ABI, runtime sessions, and package-loading contracts
- `psionic-sandbox`
  - pooled execution substrate for environment-bound agentic workloads
- `psionic-adapters`
  - later train-output lineage for adapters and promoted derived artifacts

This is the architectural direction. It is not all implemented today.

The planned crate shape is canonical for current ownership direction, but it is
not a guarantee that every future subsystem lands under exactly these final
crate names.

## Current-To-Target Matrix

| Area | Current Repo Truth | Target Repo Truth |
| --- | --- | --- |
| Checkpoint lineage | present in `psionic-train` and `psionic-runtime` | durable checkpoint families, promotion, replay, and restore across full training programs |
| Elastic membership | present in `psionic-runtime` and `psionic-train` | full participant lifecycle with heartbeats, rejoin, eviction, and topology history |
| Collective planning | present in `psionic-collectives` | full local/global sync planning with distributed optimizer integration |
| Weight broadcast | present in `psionic-datastream` | staged policy-weight broadcast with freshness cutoffs and relay policy |
| Training steps | typed fixed-budget reference loop present | broader Rust-native trainer-step engine |
| RL rollouts | typed rollout, bounded stale-rollout budgeting, and worker-protocol contracts present | validator-ready lineage and sampled adjudication |
| Environment ABI | typed runtime ABI plus typed package shape present | broader package loading, composition, and environment system |
| Eval runtime | present in `psionic-eval` | shared online/offline eval and rubric runtime, benchmark packages, and local validator simulation |
| Sandbox throughput | bounded one-shot substrate exists | RL-throughput warm pools and repeated environment loops |
| Validators for RL | rollout-verification bundles and sampled adjudication contracts present | broader service productization, batch-level adjudication, and authority integration |
| Operator surfaces | app-owned desktop-control and `autopilotctl` surfaces now exist on top of Psionic, but Psionic still does not own its own operator crate or full train operator plane | inspection, diagnostics, and receipts across all train subsystems |

## Path To Completion

The path from the current repo to a real train system is best read in four
waves.

### Wave 0: implemented substrate

Already in tree:

- runtime training truth
- datastream manifests and receipts
- collective planning substrate
- session checkpoint and recovery substrate
- adapter lineage substrate
- bounded sandbox execution substrate

### Wave 1: implemented early all-Rust train platform

Now in tree:

- fixed-budget training core with reusable autodiff and optimizer layers under
  it
- rollout artifacts, policy-lineage contracts, worker protocol, and
  validator-ready verification bundles
- environment ABI plus environment registry helpers
- data, tokenizer, split, and packing contracts
- held-out eval runtime plus Apple held-out, benchmark, and runtime-smoke
  harnesses
- run graph, checkpoint lineage, orchestrator state machine, off-policy
  budgeting, and scheduling/economics contracts
- RL-throughput sandbox primitives
- repo-owned Apple training execution backend, Apple SFT/export lane, and
  optional Apple draft-model distillation lane

### Wave 2: remaining productization and scaling

Needed next:

- true multi-device execution kernels and distributed optimizer integration
- memory-sharding, partition exchange, and broader collective or optimizer
  integration at scale
- broader model, tokenizer, and artifact-format interoperability
- stronger security, provenance, and authority integration beyond the current
  Apple accepted-outcome path
- mature artifact retention, promotion, and cold-restore governance
- broader operator lifecycle and market/product surfaces beyond the current
  app-owned Apple reference workflow

### Later scope

After the above:

- model promotion and release governance
- human preference and critique ingestion

Those later items matter, but they are not prerequisites for the core
environment-first Intellect-style train stack.

## Proposed GitHub Issue Program

The issue program below is written from the current repository state, not from
the older "there is no `psionic-train` crate" assumption.

This program first landed as issues `#3564` through `#3593` and was later
extended by the framework-core follow-ons `#3602` and `#3603`, plus the
Apple-lane closures `#3616` through `#3631`.

### Core Platform Build-Out

### 1. `Psionic Train: complete the Rust-native training core beyond recovery substrate`

Status: implemented on 2026-03-14 via GitHub issue `#3564`.

Added `psionic-train` fixed-budget training-core types and behavior for:

- typed parameter groups
- explicit optimizer-state ownership
- optimizer-state residency policy and transitions
- machine-legible step telemetry for gradient, update, and parameter norms
- visible window and cadence scheduling
- checkpoint-anchored restore via `TrainingSessionState`

Issue `#3603` extends that core with a reusable optimizer layer in
`src/optimizer.rs` so SGD, Adam, AdamW, LARS, and LAMB step semantics are no
longer trainer-private. The fixed-budget loop now composes with the reusable
optimizer surface instead of carrying its own ad hoc update implementation.

Issue `#3602` adds reusable autodiff underneath that loop in `psionic-ir`:
explicit gradient-bearing graph construction, an IR-level `detach` op,
training/evaluation plus no-grad posture, symbolic backward plans, dense
reference materialization, and a trainer-integration proof that the resulting
gradients can feed the fixed-budget training core without trainer-local
gradient logic.

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAINING_CORE_FIXED_BUDGET_REFERENCE.md`
- `scripts/release/check-psionic-training-core.sh`

The current step path is intentionally an explicit-gradient reference loop over
`f32` tensor payloads, but it no longer implies trainer-private gradient logic.
Autodiff and optimizer behavior now live in reusable lower Psionic layers,
while broader operator-family coverage and higher-order training behavior still
remain future work.

On 2026-03-15, GitHub issue `#3631` added the missing repo-owned Apple
training execution backend inside `psionic-train`:

- validation over the repo-owned Apple dataset tokenizer and prompt-shaping
  lineage plus the SFT-capable Apple environment bundle
- deterministic Apple sample batching on top of the packed dataset contract
- adapter-only parameter selection with frozen-base semantics
- repo-owned forward/loss and low-rank gradient production that feeds
  `TrainingGradientBatch` and `FixedBudgetTrainingRun`
- explicit training-posture declaration for the currently supported `f32`
  reference precision path, with graph-level checkpoint transforms available
  in `psionic-ir` but activation checkpointing still disabled in this shipped
  Apple reference lane

This lands the learning computation itself for the first Apple lane.

On 2026-03-15, GitHub issue `#3625` then added the higher-level Apple SFT lane
on top of that backend:

- fixed-budget step execution across the repo-owned Apple batches
- typed step receipts and final training summary for the Apple run
- initial/final adapter-only portable bundle snapshots plus derived adapter
  delta
- reproducibility metadata suitable for later authority publication
- valid `.fmadapter` export through `psionic-adapters`
- bounded long-run retention in the operator-facing SFT path: the optimizer step
  still consumes explicit gradient tensors, but completed-step records now
  redact those tensors after application so step history does not keep one full
  gradient snapshot per step in memory during longer Apple runs

That means the first honest Rust-native Apple adapter SFT path is now real in
repo code.

On 2026-03-15, GitHub issue `#3626` added the explicitly separate optional
Apple draft-model distillation lane on top of that SFT path:

- fixed-budget teacher/student distillation over the repo-owned Apple batches
- explicit teacher/draft runtime pairing and dual-precision posture capture
- deterministic latency and speculative-acceptance accounting in typed batch
  records and summary output
- portable draft checkpoint export plus paired `draft.mil` or
  `draft_weights.bin` payload emission inside `.fmadapter`

Authority publication and desktop workflow are now real through the later
Apple-lane issue closures. Broader provider-market training truth and generic
market claims beyond the current Apple reference path remain later work.

### 2. `Psionic RL: define rollout artifacts, trainer batches, and policy-lineage contracts`

Status: implemented on 2026-03-14 via GitHub issue `#3565`.

Added `psionic-train` RL-facing contracts for:

- checkpoint-aware `PolicyRevision`
- proof-bearing `RolloutArtifact`
- deterministic `TrainerBatch` assembly
- explicit `PolicyRevisionLineage`

The canonical runbook and harness are now:

- `crates/psionic/docs/ROLLOUT_ARTIFACT_POLICY_LINEAGE_REFERENCE.md`
- `scripts/release/check-psionic-rl-rollout-artifacts.sh`

This issue makes rollout payloads, trainer-batch assembly, and policy lineage
real and reusable. It does not yet claim freshness enforcement, worker
protocols, validator adjudication, or full orchestration.

### 3. `Environments: define a Rust-native environment ABI and runtime contract`

Status: implemented on 2026-03-14 via GitHub issue `#3566`.

Added the `psionic-environments` crate for:

- canonical `environment_ref@version` package identity
- Rust-native environment package ABI
- execution entrypoints, tool interfaces, rubric hooks, and artifact
  expectations
- deterministic runtime sessions with turn, tool, artifact, and rubric receipts

The canonical runbook and harness are now:

- `crates/psionic/docs/ENVIRONMENT_ABI_REFERENCE.md`
- `scripts/release/check-psionic-environment-abi.sh`

Kernel and Nexus still own registry and authority truth. This issue lands the
Psionic-side runtime and contract layer only.

On 2026-03-15, GitHub issue `#3622` extended the same crate with a reusable
Apple adapter environment bundle:

- a shared train/eval core package plus a benchmark-only package over the same
  typed environment ABI
- explicit Apple session/runtime, tool-bundle, rubric-binding, and
  structured-output refs carried as package metadata that now affects package
  digests
- train/eval group composition that proves the same pinned core package is
  reused across both surfaces through an explicit parity receipt

### 4. `Psionic Data: add Rust-native dataset, tokenizer, split, and packing contracts`

Status: implemented on 2026-03-14 via GitHub issue `#3567`.

Added the `psionic-data` crate for:

- canonical `dataset_ref@version` identity through `DatasetKey`
- typed dataset manifests bound to tokenizer digests and tokenized shard refs
- split declarations over `psionic-datastream` manifest refs with explicit
  shard-level sequence and token counts
- resumable streamed iteration contracts with deterministic shard ordering and
  epoch-wrap semantics
- sequence-packing and batch-packing policies for long-context workloads

The canonical runbook and harness are now:

- `crates/psionic/docs/DATASET_TOKENIZER_PACKING_REFERENCE.md`
- `scripts/release/check-psionic-data-contracts.sh`

This issue keeps byte movement in `psionic-datastream` but makes data lineage,
iteration, and packing policy first-class typed Psionic contracts. The
environment ABI now binds versioned dataset keys from this layer instead of
free-form dataset refs.

On 2026-03-15, GitHub issue `#3621` extended that same crate with the first
repo-owned Apple adapter dataset path:

- UTF-8 JSONL import into typed Apple message, tool, and guided-generation
  records
- fixture-backed validation of message ordering, tool definitions, and
  `response_format` schema requirements
- explicit tokenizer and prompt-shaping lineage metadata for later train/eval
  reuse
- deterministic Apple sample packing over explicit prompt/completion/tool/schema
  token captures, with typed refusal on tokenizer or prompt-shaping drift

On 2026-03-15, GitHub issue `#3651` added the first reviewed real-run corpus
on top of that same Apple dataset contract for the `Psionic architecture
explainer` target:

- curated `train`, `held_out`, and `benchmark` JSONL splits under
  `crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/`
- a repo-owned curation manifest that tags every split-local sample with task
  family, expected behavior, review posture, and source provenance
- machine-checkable split-leakage validation so benchmark rows remain distinct
  from train and held-out rows even when they draw from the same stable docs

This is still a first reviewed positive-path corpus, not yet the full
truthfulness or refusal slice for the first real run.

On 2026-03-15, GitHub issue `#3652` extended that same corpus with explicit
negative, correction, and retrieval-style refusal rows across `train`,
`held_out`, and `benchmark`:

- Apple-lane overclaim correction for single-host versus distributed-training
  claims
- bridge-versus-training-engine correction so the runtime sidecar is not
  confused with the Rust-owned training path
- ownership-boundary correction for pane-facing UX versus reusable Psionic or
  provider-substrate code
- retrieval-style refusal when the answer depends on current run artifacts,
  current Apple runtime state, or other stale-able evidence

That means the first reviewed real-run corpus no longer teaches only happy-path
architecture answers; it now explicitly teaches the adapter when to correct,
refuse, or avoid overclaiming.

### 5. `Psionic Eval: create the Rust-native eval and rubric runtime`

Status: implemented on 2026-03-14 via GitHub issue `#3568`.

Added the `psionic-eval` crate for:

- held-out eval-run contracts and local eval-run state machines
- rubric-scored sample construction directly from `psionic-environments`
  session summaries
- durable eval summaries with machine-legible aggregate metrics and artifacts
- explicit online/offline parity through one shared sample/runtime contract
- validator-style `BenchmarkPackage` contracts with repeat-run aggregation and
  operator-local validator simulation
- typed verification facts for timer integrity, token accounting, final-state
  capture, and declared execution strategy

The canonical runbook and harness are now:

- `crates/psionic/docs/EVAL_RUNTIME_REFERENCE.md`
- `scripts/release/check-psionic-eval-runtime.sh`

Kernel and Nexus still own canonical eval-run authority truth. This issue lands
the reusable Psionic-side runtime and benchmark-contract layer only.

On 2026-03-15, GitHub issue `#3623` extended that same crate with repo-owned
Apple adapter eval harnesses:

- held-out and benchmark scoring over imported Apple dataset fixtures plus
  observed candidate outputs
- explicit structured-output conformance and tool-call coverage checks
- bridge-backed runtime-smoke receipts that prove a `.fmadapter` package parses,
  loads, attaches, and runs against the Apple lane
- typed failure separation between dataset/config problems, package
  incompatibility, and bridge/runtime refusal

On 2026-03-15, GitHub issue `#3653` added the first real-run benchmark and
acceptance gate for the `Psionic architecture explainer` target:

- the benchmark package can now be enriched from the curated corpus so each
  benchmark case carries task-family, expected-behavior, and source-provenance
  metadata
- `psionic-eval` now compares base-model and adapted-model benchmark runs over
  the same cases and emits machine-legible per-case, per-task-family, aggregate,
  and improved-case deltas
- a reproducible acceptance policy now blocks calling a run "real" unless the
  adapted model beats the base model by the declared score, pass-rate, and
  improved-case thresholds
- the frozen architecture-explainer experiment manifest now carries one
  explicit `useful_adapter_gate` contract with:
  - the standard benchmark-improving bar for the normal reference run
  - a weaker `overfit_non_zero` benchmark gate that still requires non-zero
    movement before broader claims are allowed
  - an explicit `runtime_smoke_required` truth bit so reports can distinguish
    `accepted` from `exported_but_not_useful`

The canonical gate for that layer is now:

- `scripts/release/check-psionic-apple-architecture-explainer-benchmark.sh`

On 2026-03-15, GitHub issue `#3656` added the experiment-management layer for
that same first real-run target:

- `psionic-train` now exposes typed Apple adapter experiment manifests,
  checkpoint candidates, selection records, trend ledgers, and regression
  reason codes for the `Psionic architecture explainer` program
- the first reviewed real-run manifest now freezes dataset version, train,
  held-out, and benchmark split digests, benchmark ref, environment ref,
  runtime compatibility anchor, LoRA targets, feature widths, and acceptance
  policy in repo fixtures
- checkpoint choice is now intentional rather than log-only: accepted
  candidates sort ahead of rejected ones, then by benchmark quality and stable
  candidate id
- later iterations can now surface regression explicitly against the best prior
  accepted run instead of silently overwriting "the latest good package"

The canonical experiment-program gate for that layer is now:

- `scripts/release/check-psionic-apple-experiment-program.sh`

On 2026-03-16, GitHub issue `#3900` added the full-path acceptance harness for
that same Rust-only Apple reference lane:

- `apps/autopilot-desktop` now exposes one repo-owned acceptance entrypoint for
  the architecture-explainer lane:
  `run_architecture_explainer_acceptance_harness(...)`
- the harness runs the full path twice:
  - `overfit_non_zero`, which reuses `benchmark.jsonl` as train plus held-out
    and still requires runtime smoke, export completion, and the weak non-zero
    benchmark gate to pass
  - `standard`, which reruns the normal train/held-out/benchmark path and still
    requires runtime smoke, export completion, and the stricter benchmark bar
- the canonical operator entrypoints for that harness are now:
  - `cargo run -p autopilot-desktop --bin apple_architecture_explainer_acceptance_harness -- ...`
  - `scripts/release/check-psionic-apple-architecture-explainer-acceptance.sh`
- the current frozen manifest for that acceptance lane is now:
  - `crates/psionic/fixtures/apple_adapter/experiments/psionic_architecture_explainer_acceptance_reference_v2.json`
- the harness always writes a machine-readable acceptance receipt with:
  - top-level `acceptance_passed`
  - stage-specific reports for `overfit_non_zero` and `standard`
  - exact reason codes when a stage fails on launch, runtime smoke, export
    completion, or benchmark usefulness
- the stage-specific report tree is deterministic under the chosen report
  directory:
  - `overfit_non_zero/report.json`
  - `overfit_non_zero/report.md`
  - `overfit_non_zero/overfit_non_zero.fmadapter`
  - `standard/report.json`
  - `standard/report.md`
  - `standard/standard.fmadapter`
- the release script and harness binary both exit non-zero if either stage
  fails, so zero-improvement adapters stop at the acceptance boundary instead of
  being mistaken for complete Apple-lane success

On 2026-03-16, GitHub issue `#3901` updated the operator wording and the
canonical docs to match the exact live acceptance-harness result instead of
letting export/runtime truth imply broader Apple-lane success:

- `autopilotctl training status` and related text output now label the
  authority boundary explicitly as `authority_accept` and `authority_outcome`
  instead of the looser `accept` or `accepted_outcome`
- when Apple operator runs are present, the text output now also prints an
  explicit note that export, runtime smoke, and authority acceptance do not by
  themselves prove benchmark-useful adapter quality
- the latest live acceptance harness receipt on 2026-03-16 was:
  - top-level `acceptance_passed = false`
  - overfit stage run
    `psionic-architecture-explainer-first-real-run-overfit-non-zero-1773694818159`
    passed the weak gate with:
    - aggregate score `520` bps
    - aggregate pass rate `1428` bps
    - improved case count `1`
  - standard stage run
    `psionic-architecture-explainer-first-real-run-standard-1773694877205`
    remained rejected with:
    - aggregate score `571` bps
    - aggregate pass rate `1428` bps
    - improved case count `1`
    - reason codes:
      - `adapter_score_below_minimum`
      - `adapter_pass_rate_below_minimum`
      - `score_delta_below_minimum`
      - `pass_rate_delta_below_minimum`
      - `improved_case_count_below_minimum`
- the resulting claim boundary is now explicit:
  - the Rust-only Apple lane can train, export, load, and runtime-smoke valid
    `.fmadapter` packages
  - the same lane now clears the weak overfit non-zero gate
  - the standard benchmark-useful gate is still not met
  - operator status and authority publication are lifecycle truth, not proof
    that the adapter is already useful in the stronger benchmark sense
- the canonical human-readable companion record for that status update is now:
  - `docs/audits/2026-03-16-psionic-apple-acceptance-harness-status.md`

On 2026-03-16, GitHub issue `#3893` moved that same Apple overfit path off the
flat-zero benchmark floor without weakening the structured or tool contract:

- the Rust-only Apple training backend now adds contrastive target-bank
  alignment plus runtime-derived negative-anchor rejection on top of the older
  pooled reconstruction objective
- Apple eval samples now retain raw expected and observed text in
  machine-legible metadata so benchmark comparison can reason about behavior
  instead of only string identity
- the curated base-vs-adapter benchmark now uses behavior-aware text scoring
  for plain-text benchmark rows keyed by the reviewed corpus annotation:
  - `direct_answer` rows still zero out on policy-refusal or safety-hallucinated
    non-answers
  - `correction` rows require the right `yes` or `no` posture before they can
    score
  - `refusal` rows can now distinguish a grounded "needs current runtime
    validation" answer from a generic "`can't assist`" refusal
- structured-output rows and tool-routing rows remain strict; those still score
  from exact structured conformance and tool-call coverage rather than the new
  text rubric
- the live Apple overfit-non-zero run now clears the frozen weak gate with a
  real adapted benchmark delta:
  - aggregate score `520` bps
  - aggregate pass rate `1428` bps
  - improved case count `1`
  - the improved case is the reviewed stale-evidence refusal row
    `sample-000007`
- that is intentionally not the same thing as saying the Apple lane is now
  broadly useful:
  - the standard benchmark-improving bar is still not met
  - structured summary and tool-routing rows remain unresolved
- multiple plain-text rows still fail outright

On 2026-03-16, GitHub issue `#3894` closed the manifest-to-live parity gap for
that same first reviewed Apple lane:

- the frozen architecture-explainer manifest fixture is now pinned to the
  actual live exportable lane instead of claiming a broader geometry or target
  family:
  - symbolic targets: `decoder.attn.q_proj`
  - feature width: `2048x2048`
  - LoRA rank: `32`
- the operator no longer silently narrows unsupported manifest requests:
  - unsupported symbolic target families such as `decoder.ffn.up_proj` now
    fail before training with an explicit contract error
  - geometry or rank mismatches now fail before training with an explicit
    live-lane requirement error
- operator receipts and lineage metadata now record both the requested and the
  executed target families plus geometry, so reports show exactly what the run
  asked for and what the live lane actually executed
- the current truthful overfit report at
  `psionic-architecture-explainer-first-real-run-1773687000518` shows those
  requested and executed fields matching exactly for the frozen manifest

On 2026-03-16, GitHub issue `#3895` made the Apple training policy explicit,
inspectable, and field-sweepable instead of leaving optimizer behavior as an
implicit operator default:

- `psionic-train` experiment manifests can now carry an explicit
  `training_policy` block that freezes:
  - optimizer family plus optimizer-family fields such as `learning_rate`,
    `weight_decay`, `beta1`, `beta2`, `epsilon`, and optional
    `gradient_clip_norm`
  - optimizer residency posture
  - optional scheduler binding
  - precision policy
  - activation-checkpoint policy
  - packing policy
  - `max_steps`
  - `gradient_accumulation_steps`
- manifest validation now rejects obviously invalid policy shapes before launch,
  including:
  - `learning_rate <= 0`
  - `gradient_accumulation_steps == 0`
  - invalid packing-policy windows
- the current Rust-native Apple lane remains truthful about its live limit:
  `gradient_accumulation_steps` must still be `1`, and other values fail before
  training instead of being silently approximated
- `autopilotctl training launch ...` and
  `apple-architecture-explainer-reference-run ...` now both accept an optional
  `training_policy_override_path`, which applies field-by-field CLI overrides
  on top of the frozen manifest policy rather than replacing the whole policy
  blob
- operator-local summaries and Apple lineage metadata now persist the fully
  resolved training policy with per-field source attribution:
  - `repo_default`
  - `experiment_manifest`
  - `cli_override`
- the override-backed proof run
  `psionic-architecture-explainer-first-real-run-1773690239110` demonstrates
  the intended behavior:
  - optimizer family, residency, scheduler, precision, activation policy, and
    `gradient_accumulation_steps` remained `experiment_manifest` sourced
  - only `learning_rate`, `weight_decay`, `max_steps`, and the widened packing
    window were `cli_override` sourced
  - the run exported, loaded, and runtime-smoked successfully while keeping the
    sourced policy block visible in the resulting receipt

On 2026-03-16, GitHub issue `#3896` fixed the structured-output benchmark
contract so structured rows are judged on structured truth first instead of
being vulnerable to harness-format noise:

- Apple structured benchmark rows now canonicalize JSON semantics rather than
  requiring byte-for-byte raw text identity for the emitted JSON string
- the eval harness now accepts semantically equal structured payloads even when
  the raw JSON field order or whitespace differs
- when a structured request fails at the harness or bridge contract layer, the
  observed sample now carries explicit structured-contract metadata instead of
  forcing everything into a generic text-mismatch bucket
- benchmark failure reasons now distinguish:
  - `harness_contract_failure:structured_generation:*`
  - `runtime_failure:*`
  - true `model_output_mismatch:structured`
- this keeps the architecture-explainer structured row truthful:
  - if the model emits the wrong JSON values, it still fails as a model error
  - if the bridge or schema path is the problem, the receipt now says so

On 2026-03-16, GitHub issue `#3897` fixed the Apple repo-lookup tool contract
so tool benchmark rows are no longer suppressed by avoidable harness aborts:

- repo lookup tools now expose a tighter path contract directly in their schema
  surface:
  - repo-relative concrete file paths only
  - no directories
  - no globs
  - no absolute paths
  - no `..` traversal
- model-request mistakes on the lookup tools are now recoverable tool results
  instead of hard tool exceptions that abort the whole Apple FM turn:
  - directory requests
  - glob requests
  - wrong lookup-kind requests
  - other invalid-path proposals
- recoverable repo-lookup responses now carry retry guidance, suggested tool
  family, and suggested repo-relative paths, and the same details are persisted
  in per-sample repo-lookup metadata
- the eval harness now distinguishes the benchmark categories the issue required:
  - `model_behavior:tool_not_chosen:*`
  - `model_behavior:wrong_tool_chosen:*`
  - `model_behavior:invalid_path_proposed:*`
  - `harness_failure:*`
  - `model_behavior:true_execution_failure:*`
- this keeps the tool benchmark honest:
  - avoidable path-policy mistakes no longer look like a generic runtime crash
  - truly missing or wrong tool behavior still remains visible as model failure

On 2026-03-15, GitHub issue `#3657` tightened the Apple runtime-validation
layer around that same run:

- runtime-smoke receipts now carry the bridge-reported compatibility snapshot
  used during validation, including the current base-model family anchor,
  bridge version/platform, availability state, and adapter inventory or attach
  capability posture
- bridge-backed runtime validation now fails with explicit reasons when the
  Apple runtime is unavailable, when adapter inventory or attach support is
  missing, or when the bridge rejects the package
- `autopilotctl training accept ...` now reruns a drift check against the live
  Apple bridge before publishing authority truth, so runtime or Background
  Assets drift is surfaced explicitly instead of being hidden behind a stale
  earlier smoke receipt

The canonical runtime-validation gate for that layer is now:

- `scripts/release/check-psionic-apple-runtime-validation.sh`

### 6. `Psionic Train: define canonical run graph, topology revisions, and participant lifecycle`

Status: implemented on 2026-03-14 via GitHub issue `#3569`.

Added run-graph contracts inside `psionic-train` for:

- stable run ids, stage ids, topology revisions, contributor-set revisions, and
  `TrainingWindow` ids
- explicit participant admission, readiness, contribution, departure, and
  suspension state
- persistent participant ranking and deterministic contributor reselection
- heartbeat, departure, rejoin, and contributor-suspension lifecycle events
- replay-safe window planning with deterministic batch/eval slice assignment
- machine-legible window transitions through `planned`, `active`, `sealed`,
  `scored`, and `reconciled`

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_RUN_GRAPH_REFERENCE.md`
- `scripts/release/check-psionic-train-run-graph.sh`

This issue makes the run graph and participant lifecycle explicit typed Psionic
truth instead of a scheduler convention. It does not yet land full
orchestrator, checkpoint-pointer, or batch-propagation policy.

### 7. `Psionic Train: extend checkpoint lineage, recovery modes, and catch-up receipts`

Status: implemented on 2026-03-14 via GitHub issue `#3570`.

Added checkpoint-lineage and restore-ladder contracts inside `psionic-train`
for:

- typed `CheckpointPointer` and `CheckpointManifest` objects over explicit run,
  stage, or window scope
- explicit durability posture on checkpoint manifests, including partial-upload
  versus durable restore eligibility
- declared `TrainingRecoveryMode` choices for blocking catch-up, overlapped
  catch-up, and resume-from-last-stable-checkpoint
- pointer-first restore planning with manifest-listing fallback when the latest
  pointer is missing, stale, or references non-durable state
- deterministic shard-uploader assignment over the accepted restore manifest
- fake object-store tests covering missing pointer, stale pointer,
  partial-upload, and listing-limit failure paths

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_CHECKPOINT_RECOVERY_REFERENCE.md`
- `scripts/release/check-psionic-train-checkpoint-recovery.sh`

This issue turns checkpoint recovery from implicit latest-checkpoint heuristics
into typed restore receipts that can explain why one source was preferred over
another. It does not yet land retention policy, cold-restore classes, or
cross-window checkpoint governance.

### 8. `Psionic Collectives: add bandwidth-aware elastic sync planning and quantized policy surfaces`

Status: implemented on 2026-03-14 via GitHub issue `#3571`.

Added sync-planning and cadence-policy contracts inside `psionic-collectives`
for:

- mesh-wide `CollectiveTransportFeedback` observations with stable digests and
  explicit bandwidth, latency, and stream-pressure metrics
- `CollectiveSyncCadencePolicy` over healthy and degraded global-sync
  intervals, transport thresholds, and local/global quantization posture
- `CollectiveSyncExecutionPlan` and `CollectiveSyncStage` so local subgroup
  sync and full-mesh sync are planned as explicit ordered stages
- `CollectiveSyncCadenceReceipt` and `CollectiveReplanTrigger` so cadence,
  transport degradation, quantization fallback, and interval-elapse decisions
  stay machine-legible
- planner-owned local-group selection, interval-based deferred global sync, and
  mesh-revision replanning over the existing benchmark-gated quantized
  collective substrate

The canonical runbook and harness are now:

- `crates/psionic/docs/COLLECTIVE_SYNC_POLICY_REFERENCE.md`
- `scripts/release/check-psionic-collective-sync.sh`

This issue makes collective sync cadence explicit Psionic truth instead of a
hidden optimizer-side heuristic. It does not yet land distributed optimizer
state integration or parameter-shard accounting.

### 9. `Psionic Datastream: add sharded policy-weight broadcast and freshness control`

Status: implemented on 2026-03-14 via GitHub issue `#3572`.

Added policy-weight broadcast contracts inside `psionic-datastream` for:

- explicit `PolicyWeights` subject identity plus `DatastreamPolicyWeightBinding`
  over policy id, revision, shard identity, assembled-artifact digest, and
  freshness window
- lightweight `DatastreamPolicyWeightControlPlaneRef` and
  `DatastreamPolicyWeightBroadcastManifest` objects so orchestrators can carry
  refs, digests, and mirror metadata instead of heavy payload bytes
- mirror or relay metadata through `DatastreamMirrorLocator`
- stale-artifact rejection at control-plane-ref export time
- `InMemoryPolicyWeightBroadcast` and
  `DatastreamPolicyWeightBroadcastReceipt` for pipelined multi-shard delivery
  over the existing resumable chunk path
- tests proving the control-plane summary stays smaller than the heavy artifact
  bytes while the heavy artifact plane remains resumable and byte-accountable

The canonical runbook and harness are now:

- `crates/psionic/docs/POLICY_WEIGHT_BROADCAST_REFERENCE.md`
- `scripts/release/check-psionic-policy-weight-broadcast.sh`

This issue makes the heavy artifact plane versus lightweight control plane
split explicit for policy weights. It does not yet land orchestrator-owned
assignment or rollout freshness budgets.

### 10. `Psionic Train: build the orchestrator state machine and trainer-batch assembly contracts`

Status: implemented on 2026-03-14 via GitHub issue `#3573`.

Added the first orchestrator module inside `psionic-train` for:

- typed `TrainingOrchestratorState` over the existing run graph, target policy
  revision, and lightweight policy-weight broadcast manifest
- orchestrator ownership of contributor selection, window planning, window
  activation, sealing, scoring, and reconciliation transitions
- deterministic `TrainingWindowAssignmentPosture` carrying assignment seed,
  policy revision id, and weight-broadcast digest
- lightweight rollout and sampled-eval assignments that exchange only ids,
  digests, policy ids, and weight-broadcast refs
- lightweight `RolloutArtifactRef` and `TrainerBatchAssemblyRequest` contracts
  so trainer-batch assembly stays control-plane-safe while still composing with
  full `RolloutArtifact` and `TrainerBatch` substrate
- replay-safe tests proving admitted participants, contributing participants,
  and resulting trainer batches can all differ in one orchestrated window

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_ORCHESTRATOR_REFERENCE.md`
- `scripts/release/check-psionic-train-orchestrator.sh`

This issue makes the orchestrator a first-class Psionic control plane instead
of a loose pile of helpers around the run graph. It does not yet land
off-policy pruning or worker protocol completion.

### 11. `Psionic Train: implement off-policy budget rules and stale-rollout pruning`

Status: implemented on 2026-03-14 via GitHub issue `#3574`.

Added bounded rollout-admission contracts inside `psionic-train` for:

- explicit `TrainingOffPolicyBudget` policy over revision drift, policy age,
  rollout age, and quarantine thresholds
- typed `RolloutAdmissionReceipt` outcomes for accepted exact, accepted
  off-policy, quarantined, and discarded rollouts
- machine-readable `RolloutAdmissionSignal` reason codes so freshness and drift
  violations stay inspectable rather than log-only
- per-window `RolloutIngestionTelemetry` and retained quarantined-versus-
  discarded rollout state on the orchestrator
- replay-safe tests proving exact acceptance, bounded off-policy acceptance,
  quarantine outside direct-accept budgets, and hard discard beyond quarantine
  budgets

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_OFF_POLICY_BUDGET_REFERENCE.md`
- `scripts/release/check-psionic-train-off-policy-budget.sh`

This issue makes stale-rollout accounting first-class train control-plane truth
instead of a batch-filtering convention. Worker claim protocol completion and
validator-owned rollout adjudication now live in the follow-on records for
issues `#3575` and `#3576`.

### 12. `Psionic Train: define the inference-worker protocol for trustless rollout generation`

Status: implemented on 2026-03-14 via GitHub issue `#3575`.

Added rollout-worker protocol contracts inside `psionic-train` for:

- explicit `RolloutWorkerTrustClass` and `RolloutWorkerIdentity` so trusted
  trainer nodes are protocol-distinct from semi-trusted or untrusted rollout
  workers
- `RolloutWorkerHeartbeatReceipt` and `RolloutTaskClaim` over heartbeat
  freshness, claim TTL, deterministic sample-selection seed, and assignment
  binding
- `RolloutUploadLocator` and upload-policy enforcement for inline versus
  external artifact delivery
- `RolloutWorkerOutcomeReceipt` that wraps local claim-expiry or upload-policy
  outcomes plus orchestrator-provided rollout-admission receipts
- replay-safe tests proving fresh-heartbeat claims, bounded off-policy upload
  handling, and local receipts for expired claims or oversized uploads

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_ROLLOUT_WORKER_PROTOCOL_REFERENCE.md`
- `scripts/release/check-psionic-train-rollout-worker-protocol.sh`

This issue makes rollout-worker coordination a first-class typed protocol
inside Psionic instead of a trainer-local convention. Validator-owned rollout
verification and sampled adjudication now live in the follow-on record for
issue `#3576`.

### 13. `Validator Service: add rollout-verification bundles and sampled adjudication protocols`

Status: implemented on 2026-03-14 via GitHub issue `#3576`.

Added rollout-validation contracts inside `psionic-train` for:

- `RolloutVerificationBundle` over one rollout artifact, worker outcome, and
  optional benchmark observation or expectation
- `RolloutValidatorPolicy` with execution-proof requirements, deterministic
  sampled expensive-check posture, benchmark-check posture, and duplicate
  normalization policy
- `ValidatorVerdict` with typed replay-detected, duplicate-detected,
  stale-policy-rejected, contribution-normalized, timer-integrity,
  token-accounting, final-state, and execution-strategy reason codes
- stateful replay and duplicate detection through artifact-digest and
  response-signature history
- benchmark-gated sampled adjudication for timer, token, final-state, and
  declared-execution-strategy checks

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_ROLLOUT_VALIDATION_REFERENCE.md`
- `scripts/release/check-psionic-train-rollout-validation.sh`

This issue makes validator-ready rollout integrity first-class typed Psionic
truth. Broader external validator services, batch-level verdicts, and authority
integration are still later layers.

### 14. `Environments: define a package contract for SFT, RL, and eval`

Status: implemented on 2026-03-14 via GitHub issue `#3577`.

Added package-shape contracts inside `psionic-environments` for:

- `EnvironmentWorkloadClass` so one package can declare SFT, RL, online-eval,
  offline-eval, and validator-benchmark use explicitly
- typed `EnvironmentPolicyReference` and `EnvironmentDifficultyMetadata`
  instead of burying those semantics in free-form metadata
- `EnvironmentBenchmarkProfile` for validator-owned benchmark identity,
  runtime-profile identity, verification posture, and declared
  execution-strategy expectations
- package validation and digest coverage for workload classes, policy refs,
  difficulty metadata, and benchmark profiles
- replay-safe tests proving one package can carry both ordinary environment
  execution contracts and a reusable benchmark profile

The canonical runbook and harness are now:

- `crates/psionic/docs/ENVIRONMENT_PACKAGE_CONTRACT_REFERENCE.md`
- `scripts/release/check-psionic-environment-package-contract.sh`

This issue makes environment packages composable across training, eval, and
validator-local simulation instead of relying on raw metadata blobs or hidden
side settings. Registry install and composition flows remain the next issue.

### 15. `Environments Registry: add package install, version pinning, composition, and eval parity`

Status: implemented on 2026-03-14 via GitHub issue `#3578`.

Added the first Psionic-native registry and composition layer inside
`psionic-environments`:

- typed install requests and install receipts for versioned environment package
  materialization
- digest-bound pin aliases so train and eval code resolve immutable package
  versions instead of floating refs
- mixed-surface composition groups and group-member contracts across train,
  eval, and benchmark surfaces
- dependency-aware group resolution and benchmark-profile validation
- explicit train/eval parity receipts for shared group members

The canonical runbook and harness are now:

- `crates/psionic/docs/ENVIRONMENT_REGISTRY_REFERENCE.md`
- `scripts/release/check-psionic-environment-registry.sh`

This issue removes the need for bespoke environment-mix glue in the
orchestrator for the first train/eval/benchmark package groups. Persistent
authority sync, package publication, and richer eval-policy productization
remain later layers.

On 2026-03-15, GitHub issue `#3622` added the first repo-owned Apple adapter
specialization on top of that registry substrate: one helper now materializes
the shared Apple core package, benchmark package, mixed-surface group, and the
train/eval parity receipt together so later train and eval layers do not have
to rebuild Apple environment wiring from app-local config.

### 16. `Psionic Sandbox: add RL-throughput primitives for pooled, repeated agentic execution`

Status: implemented on 2026-03-14 via GitHub issue `#3579`.

Added the first RL-throughput sandbox control plane inside `psionic-sandbox`:

- typed warm-pool specs, snapshots, warm receipts, and acquisition receipts
- staged-input receipts for command inputs, image frames, and context artifacts
- repeated bounded loop execution on the same acquired workspace
- explicit reuse accounting so pool health and acquisition latency are visible
  to later train or operator layers

The canonical runbook and harness are now:

- `crates/psionic/docs/SANDBOX_RL_THROUGHPUT_REFERENCE.md`
- `scripts/release/check-psionic-sandbox-rl-throughput.sh`

This issue makes the sandbox layer usable for RL-style short-lived environment
actions without forcing one bespoke background-job flow per environment.
Distributed pool management and higher-level train scheduling still remain
later layers.

### 17. `Psionic Train: add SFT trace ingestion, stage transitions, and agentic pre-RL flows`

Status: implemented on 2026-03-14 via GitHub issue `#3580`.

Added the first multi-stage train-program layer inside `psionic-train`:

- typed `TrainingStageKind` identity for `general_sft`, `agentic_sft`, and `rl`
- typed SFT trace artifacts with tool-call and long-context lineage
- stage completion receipts, checkpoint-promotion receipts, and stage-transition
  receipts
- a stage-program state machine that owns `general_sft -> agentic_sft -> rl`
  sequencing

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_STAGE_PROGRAM_REFERENCE.md`
- `scripts/release/check-psionic-train-stage-program.sh`

This issue makes stage sequencing first-class Psionic truth instead of operator
glue. Curriculum, filtering, and instability policy remain the next train
issues.

### 18. `Psionic Train: implement curriculum, filtering, and non-zero-advantage gates`

Status: implemented on 2026-03-14 via GitHub issue `#3581`.

Added the first train-side curriculum controller inside `psionic-train`:

- digest-bound curriculum policy with online and offline sampling filters
- typed training candidates constructed from SFT traces and rollout artifacts
- explicit filter receipts and batch selection receipts
- difficulty-tier consumption, trivial-reward suppression, source-budget
  suppression, and non-zero-advantage gates

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_CURRICULUM_REFERENCE.md`
- `scripts/release/check-psionic-train-curriculum.sh`

This issue makes training-sample selection inspectable and reproducible.
Instability telemetry and halt policy remain the next train issue.

### 19. `Psionic Train: add instability telemetry, halt policies, and risky-optimization gating`

Status: implemented on 2026-03-14 via GitHub issue `#3582`.

Added the first train-safety controller inside `psionic-train`:

- aggregated instability telemetry over gradient norms, clipping ratios, and
  rollout-drop rate, with explicit extension points for entropy drift,
  checkpoint catch-up latency, topology churn, and failure rates
- digest-bound threshold rules that map signals to `continue`, `quarantine`, or
  `halt`
- explicit risky-optimization rules so dangerous runtime shortcuts are policy,
  not hidden flags
- final typed verdicts carrying both signal receipts and optimization receipts

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_STABILITY_REFERENCE.md`
- `scripts/release/check-psionic-train-stability.sh`

This issue makes halt/quarantine policy machine-legible. Broader operator
product surfaces beyond the current app-owned Apple workflow remain later
layers.

### 20. `Kernel and Nexus: add training and eval receipt families, policy registries, and read models`

Once train and eval become economic or productized objects, their outcomes
need authority-facing truth. This issue should add durable receipt families,
read models, and policy registries for environment packages, checkpoint
families, validator posture, and accepted train or eval outcomes. It is the
bridge from Psionic-local execution truth into higher-level OpenAgents market
or authority truth. It should also prefer typed Rust client and payload-builder
surfaces for those train, eval, and validator-facing authority contracts rather
than ad hoc JSON glue.

Status: implemented on 2026-03-14 via GitHub issue `#3583`.

The canonical authority docs are now:

- `docs/kernel/compute-evaluation-runs.md`
- `docs/kernel/compute-training-authority.md`

The generated or typed authority path now exists in `openagents-kernel-core`
and `apps/nexus-control` for:

- checkpoint-family policy registry
- validator-policy registry
- benchmark-package registry
- training-policy registry
- training-run create/finalize/list/get
- accepted eval or training outcomes

On 2026-03-15, GitHub issue `#3624` extended the same authority surface with
Apple-specific benchmark adapter kinds plus typed training-policy and
training-run metadata validation, so Apple packages and runs now have to carry
consistent benchmark, validator, and environment bindings before Nexus accepts
them.

On 2026-03-15, GitHub issue `#3627` then extended Nexus with the first
canonical Apple training-run and accepted-outcome projection path, including
held-out eval gating, optional runtime-smoke gating, and persistence of typed
Apple package lineage on both the finalized training run and accepted outcome.

### 21. `Desktop and autopilotctl: expose training operator surfaces and diagnostics`

Implemented on Sunday, March 15, 2026 via GitHub issue `#3629`, after the
earlier Apple adapter inventory/control plumbing from issue `#3620`.

The app-owned desktop-control surface and `autopilotctl` now expose a typed
training operator view. The current projection is intentionally truthful about
what is authority-backed versus what is not yet wired from a live train
controller:

- authority-backed training runs and accepted outcomes are loaded into the
  desktop-control compute-history cache alongside proof and challenge truth
- the snapshot now exposes a dedicated `training` domain with explicit
  `control_plane_state` versus `artifact_plane_state`
- operator output includes environment versions, checkpoint refs,
  contributor-set revision hints, contributor reselection timing, stale-rollout
  discard counts, duplicate quarantine or deweight counts, validator verdict
  totals, sandbox pool readiness, and visible run-level diagnostics
- the same surface now carries an app-owned Apple operator sub-view with
  explicit `launch`, `evaluation`, `export`, and `acceptance` stage state plus
  persisted run logs and authority refs
- `autopilotctl training launch`, `training export`, and `training accept`
  drive the same desktop-control mutations instead of relying on ad hoc scripts
- `autopilotctl training status` prints the same app-owned projection directly,
  while `autopilotctl status` includes a condensed training summary

This does not claim a live Psionic train orchestrator is embedded in the
desktop app yet. It does make the currently available training truth
inspectable without reconstructing it from logs or ad hoc scripts.

On 2026-03-15, GitHub issue `#3628` projected the same accepted Apple adapter
truth into `openagents-provider-substrate` as a narrow provider-hosted
adapter-family capability, and GitHub issue `#3630` updated the compute-market
docs to match that narrow truth boundary. Those surfaces sit above Psionic and
do not yet imply a broad training procurement product.

### 22. `Reference Program: run one end-to-end agentic SFT plus RL pilot on the full stack`

Implemented on Saturday, March 14, 2026.

`psionic-train` now ships a typed reference-program runner in
`src/reference_program.rs` plus the runnable harness
`scripts/release/check-psionic-agentic-sft-rl-reference-program.sh`.

The pilot intentionally crosses the currently implemented Rust-owned stack
instead of claiming completion from isolated subsystem tests:

- one versioned weather-agent environment package is reused across SFT, RL,
  online eval, and benchmark-mode eval
- dataset lineage remains explicit through environment bindings, trace source
  refs, and eval contracts
- stage-program lineage crosses `general_sft -> agentic_sft -> rl` with
  explicit checkpoint-promotion receipts
- policy weights are delivered through `psionic-datastream` broadcast receipts
- sandbox warm-pool reuse is proven through staged-input and iteration receipts
- rollout-worker heartbeat, claim, upload, and outcome receipts run against the
  real train orchestrator state
- validator-aware adjudication emits typed verdicts over rollout bundles
- benchmark aggregation and online eval both remain machine-legible
- the trainer step consumes the orchestrator-produced trainer batch rather than
  a disconnected toy batch
- the final report includes a condensed operator view without discarding the
  underlying typed receipts, lineage, and summaries

This is the current main integration gate for the early train stack. It does
not claim that replay guarantees, security hardening, artifact lifecycle, or
research-loop layers are complete, and it does not turn the landed
distributed-optimizer or model-IO contracts into proof that the full
multi-device runtime is complete.

### Production Completion And Hardening

### 23. `Psionic Train: define distributed optimizer, precision, and memory-sharding contracts`

Implemented on Saturday, March 14, 2026.

`psionic-train` now owns an explicit distributed-optimizer layer in
`src/distributed_optimizer.rs` on top of the existing fixed-budget core.

The new contract makes all of the following first-class:

- distributed optimizer family selection
- parameter sharding per group
- gradient-buffer sharding per group
- optimizer-state sharding plus residency
- master-weight residency
- precision policy across parameter, gradient, optimizer-state, master-weight,
  and reduction paths
- activation checkpointing or rematerialization policy
- long-run host/device memory budgeting and derived memory-plan receipts
- microbatch accumulation and flush discipline
- collective sync-plan attachment to the optimizer contract itself

The runtime wrapper is still intentionally bounded. It buffers microbatches,
refuses incomplete flushes, derives an explicit memory plan, and then flushes
one accumulated step through the existing fixed-budget trainer core while
preserving the higher-level distributed receipt.

This does not claim that the full multi-device runtime already exists. It does
mean the distributed optimizer, precision, and memory-sharding model is now
typed and inspectable instead of implied by future plans.

The distributed layer now composes with the reusable optimizer surface in
`src/optimizer.rs`, so local optimizer-family step semantics are inspectable
without being trapped inside one trainer implementation.

### 24. `Model IO: add Rust-native checkpoint, tokenizer, and model-format interoperability`

Implemented on Saturday, March 14, 2026.

`psionic-train` now owns a typed model-IO portability layer in
`src/model_io.rs`.

The new layer makes these train-to-serve seams explicit:

- named state-dict traversal and assignment contracts
- portable training-group reconstruction from state-dict artifacts
- machine-readable compatibility boundaries for Psionic-native state dicts,
  manifest-carrying safetensors, typed JSON state dicts, GGUF import, and
  intentionally unsupported opaque checkpoint families
- tokenizer family, digest, special-token, and version binding
- dense safetensors export and import with embedded Psionic manifest metadata
- JSON torch-style state-dict artifacts for Rust-native portability
- GGUF import with tensor inventory, tokenizer binding, and chat-template
  digest extraction
- additive adapter merge and unmerge over parameter tensors

The scope is still intentionally bounded. The current torch-compatible surface
is typed JSON rather than opaque Python checkpoint loading, and GGUF support is
currently import-focused rather than full re-export. That is still a material
shift: trained or served artifacts are now portable through one Rust-owned
contract instead of bespoke scripts or disconnected side files.

General-purpose array artifact IO now lives separately in `psionic-array-io`,
and general-purpose native function artifact IO now lives separately in
`psionic-function-io`. That split is intentional: `psionic-train::model_io`
still owns checkpoint, tokenizer, state-dict, and model-family portability,
while `psionic-array-io` owns public framework-facing `npy` / `npz` /
`safetensors` plus bounded GGUF array save/load semantics above the lazy-array
surface, and `psionic-function-io` owns public `.psifn` export/import
artifacts plus a bounded `.mlxfn` compatibility shell above export-safe graph
and compiler contracts instead of burying that boundary inside train-local
packaging code.

### 25. `Training Truth: add deterministic replay and reproducibility guarantees`

Implemented on Saturday, March 14, 2026.

`psionic-train` now owns a deterministic replay-truth layer in
`src/replay_truth.rs`.

The new contract makes these reproducibility seams explicit:

- assignment, trainer, and eval seed discipline
- deterministic sample-selection rules with stable worker and attempt identity
- replayable trainer-batch anchoring
- pinned environment package and tool contracts
- pinned tool-version labels
- reproducible eval posture with deterministic scheduler enforcement
- typed replay-verification receipts and drift signals

`PLIB-212` / `#3727` extends that foundation by publishing one
machine-readable `ReproducibilitySemanticsReport` in `src/replay_truth.rs`.
That report binds assignment, trainer, and eval seeds into runtime
determinism contracts, proves stable local-device and distributed-rank
generator derivation, proves checkpoint-stable RNG restore, and carries typed
refusal for missing strict generators or invalid distributed-rank bounds.

`PLIB-213` / `#3728` now complements that replay truth with one bounded
autocast-style `AutocastPolicyMatrixReport` in `psionic-core`. The report
keeps backend-aware low-precision policy, stability-preserving no-downcast
rules, float8 meta-only posture, and explicit unsupported mixed-precision
requests machine-legible before train-class grad scaling lands.

`PLIB-214` / `#3729` now lands that bounded train-class grad-scaling layer in
`psionic-train::mixed_precision`. The new
`GradientScalingSemanticsReport` makes fp16 dynamic loss scaling, overflow
backoff plus step-skip, underflow-driven scale growth, bf16 no-scaling
posture, and unsupported mixed-precision refusal machine-legible instead of
burying those decisions inside one trainer loop.

`PLIB-215` / `#3730` now lands one bounded
`QuantizationCapabilitySemanticsReport` in `psionic-core`. That report
separates PTQ, QAT, quantized runtime execution, compiler-lowering posture,
and export-aware quantization intent above raw decode so train- and
deployment-class quantization claims stop collapsing into "the loader can read
GGUF."

`PLIB-218` / `#3733` now lands one bounded `DataIngressSemanticsReport` in
`psionic-data`. That report makes local dataset source, iterable-streaming,
sampler, batch-sampler, and host-device staging contracts machine-legible
instead of leaving them as train-loop glue.

`PLIB-219` / `#3734` now layers one bounded
`DistributedDataFeedSemanticsReport` on top of that local ingress surface in
`psionic-data`. The new report makes fixed-world-size shard partitioning,
epoch-barrier or step-barrier worker coordination, and runtime-derived
replay-safe per-rank ordering machine-legible, while explicitly refusing
elastic membership until a higher-level distributed run-control contract lands.

This is still not the claim that the full train system can be re-executed from
one receipt without more runtime work. It is the claim that replay-compatible
inputs, pins, and verification are now explicit enough to support "same
receipt, same recomputation rules" instead of best-effort repeatability.

### 26. `Security: harden environment packages, artifact provenance, and untrusted worker admission`

Implemented on Saturday, March 14, 2026.

`psionic-train` now owns a train-security posture layer in
`src/security_posture.rs`.

The new contract makes these hardening seams explicit:

- environment package identity and digest verification
- required environment verification and safety policy references
- artifact signing contracts plus trust roots
- minimum signature counts for admitted artifacts
- untrusted-worker rate limits and burst controls
- required execution-proof posture for untrusted workers
- duplicate-artifact rejection and duplicate-response-signature quarantine
- validator-bound security receipts with typed reason codes

This does not replace the validator loop. It does connect rollout validation to
the broader train security posture instead of leaving environment trust,
artifact provenance, and untrusted-worker admission as implicit assumptions.

### 27. `Artifact Storage: define retention, garbage collection, archival, and cold-restore policy`

Status: implemented on 2026-03-14 via GitHub issue `#3590`.

`psionic-train` now owns an explicit artifact-storage lifecycle layer in
`src/artifact_storage.rs`.

The new contract makes these storage seams explicit:

- per-artifact-class retention profiles with hot and warm thresholds
- archive classes for ephemeral, restorable, and immutable artifacts
- digest-aware deduplication for rollout or other repeatable artifact classes
- typed records for checkpoint, rollout, eval, and log bundle artifacts
- explicit sweep receipts for warm migration, archival, deduplication, and
  garbage collection
- cold-restore request and completion receipts bound to restore objectives

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_ARTIFACT_STORAGE_REFERENCE.md`
- `scripts/release/check-psionic-train-artifact-storage.sh`

This issue makes train artifact retention part of typed Psionic truth instead
of operator-local scripts. Scheduler budgeting, queue preemption, and broader
economic accounting remain the next layer.

### 28. `Scheduling and Accounting: add budget, priority, preemption, and cost attribution`

Status: implemented on 2026-03-14 via GitHub issue `#3591`.

`psionic-train` now owns an explicit scheduling and accounting layer in
`src/scheduling_accounting.rs`, and `psionic-runtime` now surfaces train-owned
runtime work classes for trainer, rollout, eval, sandbox, and validator work.

The new contract makes these operator seams explicit:

- global active-work budget caps over work units, bytes, and estimated cost
- queue classes with inspectable priority and preemption policy
- role-specific cost rates for trainer, rollout, eval, sandbox, and validator
  work
- typed admission, preemption, queueing, completion, and snapshot receipts
- validator-scoped and environment-scoped cost attribution
- queue draining after completion so queued work becomes active through typed
  state transitions rather than implicit retries

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_SCHEDULING_ACCOUNTING_REFERENCE.md`
- `scripts/release/check-psionic-train-scheduling-accounting.sh`

This issue makes train-side operator economics first-class typed Psionic truth.
Chaos testing and benchmark thresholds remain the final follow-on issues in the
train program.

### 29. `Reliability: add chaos and failure-injection suites for topology, checkpoint, and validator flows`

Status: implemented on 2026-03-14 via GitHub issue `#3592`.

`psionic-train` now owns an explicit reliability suite in
`src/reliability.rs` that runs typed chaos scenarios over existing checkpoint,
collective, orchestrator, and validator contracts.

The new contract makes these reliability seams explicit:

- topology churn drills over elastic membership and checkpoint-backed recovery
- network degradation drills over collective cadence fallback
- stale-weight flood containment over rollout admission
- checkpoint corruption drills over stale-pointer fallback
- validator sampling stress over accepted, normalized, and rejected verdicts
- orchestrator restart roundtrips that resume window control after state restore

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_RELIABILITY_REFERENCE.md`
- `scripts/release/check-psionic-train-reliability.sh`

This issue makes reliability claims a machine-checkable suite instead of a
collection of unrelated unit tests. Quantitative benchmark thresholds remain
the final train-program gap.

### 30. `Benchmarking: define performance acceptance thresholds for trainer, sandbox, datastream, and validation`

Status: implemented on 2026-03-14 via GitHub issue `#3593`.

`psionic-train` now owns a typed quantitative acceptance layer in
`src/benchmarking.rs` instead of leaving train performance closure to ad hoc
notes or one-off benchmark scripts.

The new benchmark contract makes these production thresholds explicit:

- fixed-budget trainer throughput
- rollout ingestion throughput at the orchestrator boundary
- warm sandbox reuse latency and reuse ratio
- checkpoint restore latency plus resumable datastream recovery throughput
- validator verification cost and sampled benchmark-check share
- elastic scaling curves from two to four members, including degraded transport
  fallback

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_BENCHMARK_ACCEPTANCE_REFERENCE.md`
- `scripts/release/check-psionic-train-benchmark-acceptance.sh`

This issue closes the last train-system gap called out at the end of the issue
program: Psionic now has both chaos-style reliability drills and one owned
acceptance profile for deciding whether the current train substrate is fast and
stable enough to claim seriously.

## Later-Scope Issues

These are valid future issues, but they are not part of the minimum path above.

### Model promotion and release governance

Later the system will also need:

- candidate promotion gates
- release thresholds
- rollback policy
- checkpoint-to-release lineage
- human signoff hooks

### Human preference, critique, and label-ingestion pipelines

If OpenAgents expands into broader RLHF-style or critique-driven post-training,
the system will also need:

- critique and preference record schemas
- provenance and adjudication for noisy labels
- human-score and rubric blending
- reviewer-tooling integration

## Bottom Line

The current Psionic tree already contains real train-substrate work:

- runtime training truth
- datastream movement for train-relevant artifacts
- collective planning
- checkpoint and recovery session state
- early training-output lineage through adapters
- reusable environment, eval, validator, and orchestrator crates
- one real repo-owned Apple training lane with app-owned operator and accepted-
  outcome integration around it

That means the train system is no longer hypothetical.

But the current tree still stops short of a scaled, generalized, fully hardened
all-Rust training system.

The missing center of gravity is now:

- multi-device execution kernels and distributed optimizer execution at scale
- broader format interoperability and retention/promotion governance
- stronger provenance, security, and authority integration beyond the current
  Apple path
- broader operator and product surfaces beyond the current Apple reference
  workflow

That is the path Psionic still has to build from its now-real early train
system.
