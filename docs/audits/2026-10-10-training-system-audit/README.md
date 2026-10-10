# Training system audit

Date: October 10, 2026. Source baseline:
[`3168c986aa`](https://github.com/OpenAgentsInc/openagents/commit/3168c986aa)
(Clef M1). Scope: the Psionic engine imported at
[`crates/psionic`](../../../crates/psionic/README.md), every non-Psionic
training and evaluation surface in this repository, the Pylon compute node, the
Verse and V1 product docs, the history of Psion, Tassadar, Percepta, and Pylon,
the [kitchen-sink](../../kitchen-sink/README.md) promise ledger, and the
episode transcripts. The companion [roadmap](roadmap.md) turns these findings
into ordered work.

The working tree at the baseline also holds uncommitted Clef M2 CUDA prefill
work by another agent (`psionic-backend-cuda/src/clef_prefill.rs`,
`kernels/clef_prefill.cu`, `psionic-serve/src/qwen35/clef_cuda.rs`, and edits
to `clef/mod.rs`, `qwen35.rs`, and `build.rs`). This audit marks that work as
uncommitted wherever it is cited and does not treat it as shipped.

## Summary

OpenAgents does not have a working training system. It has a large amount of
training-shaped code, one real fine-tune, and a strong measurement plane that
nothing trains against.

- **Psionic's training code is mostly contracts, receipts, and fixture
  writers.** `psionic-train` is 417k lines. By our estimate 1–3% of it computes
  gradients. Every trainer that runs either trains a toy model, trains only an
  output head or bias, or uses finite differences. The one real GPU language
  model trainer (Parameter Golf, a 9×512 GPT on CUDA) has no accepted run in
  the repository.
- **Several "green", "promoted", or "decision-grade" artifacts are not what
  their names say.** Some are hand-typed literals. Some point at unrelated
  runs. One "Transformer-backed exactness" gate compares a tokenizer round trip
  and never runs the model.
- **The import brought far more than serving needs.** `psionic-serve` depends on
  `psionic-train`, `psionic-eval`, and `psionic-research`, so about 630k lines of
  training and research code compile into `psionic-openai-server`. Serve
  imports about 170 identifiers from them, mostly for about 25 Tassadar
  publication modules and a handful of route and adapter types. Most of the fixtures that code reads (more than 90%)
  were not imported, and no CI runs any of it.
- **The only weight training OpenAgents has done itself is one Apple
  Foundation Models LoRA for Lev** (98 records, four minutes, 2026-09-19).
  It lifted accuracy on its own suite from 0.77 to 0.90. It made calibration
  worse and did not transfer to real Coder turns.
- **Pylon cannot train.** It serves inference only, and four separate layers
  block training: the NIP-PYLON lanes, the release manifest
  (`trainingClaim: blocked`), the market rights template (`training: deny`),
  and the absence of any training job kind. The older Pylon training protocol
  survives inside `psionic-train`. Its workers write placeholder outputs, and
  its signing keys can be derived from public strings.
- **The measurement plane is the real asset.** `crates/gym`, the gates,
  `tenancy::training`, the calibration rule in `crates/gym/src/gate.rs`
  (gate `probability-v2`), and the Clef `/v1/systemone` route
  with digest receipts are rigorous and in use. What is missing is a trainer
  wired to them and a pipeline that keeps labelled, consented data.

**Verdict.** Do not build distributed pretraining. Build one narrow training
loop that serves a V1 product: small decision heads, calibration maps, and LoRA
adapters for the Clef/Kev decision doors. Train them on mechanically labelled
outcomes Coder already produces. Gate them with the Gym and serve them from
Psionic. Make Pylon a decision server and eval runner before it ever becomes a
trainer. Cut the training and research code out of the serving build, and label
every artifact as measured, authored, or sample.

## Scope and method

This is a static read of code, fixtures, docs, and transcripts. It ran no
builds, tests, or training jobs. Sixteen readers worked in parallel:

| Area | What was read |
| --- | --- |
| Psionic code | All 30 crates under [`crates/psionic/crates`](../../../crates/psionic/crates). `psionic-train` was read in two halves, plus eval, research, data, environments, optimize, adapters, datastream, serve, runtime, models, the backends, Clef, cluster, net, collectives, router, sandbox, catalog, and provider. |
| Non-Psionic training | [`training/`](../../../training/README.md), [`crates/tenancy/src/training.rs`](../../../crates/tenancy/src/training.rs), `crates/gym`, `gym-bridge`, `verse-gym`, `ext-eval`, `eval-runner`, `coderbench`, `inference/src/outcomes.rs`, `gateway/src/decision_offer.rs`, `crates/pylon`, `crates/jev`, `crates/kev`, `crates/lev` |
| Product docs | `docs/verse/*`, `docs/compute/*`, `docs/launch/1.0/*`, `docs/roadmap.md`, `docs/roadmap/*`, `docs/inference/clef-*`, `docs/decision-models/*`, `docs/lev/*`, `docs/kev/*`, `docs/gym/*`, and project 22 ("V1 Launch — Oct 9", 98 items) |
| History | [`docs/history/2026-09-28-tassadar-percepta.md`](../../history/2026-09-28-tassadar-percepta.md), [`docs/psionic-and-pylon.md`](../../psionic-and-pylon.md), [`docs/psionic/README.md`](../../psionic/README.md), the [transcript roadmap](../../history/2026-09-25-transcript-roadmap.md), the [Tassadar revival proposal](../../roadmap/2026-09-28-tassadar-revival.md), and git history for every Tassadar deletion and re-import (`git log --stat`, `git show <commit>^:<path>` for deleted files) |
| Kitchen sink | All four files in [`docs/kitchen-sink/`](../../kitchen-sink/README.md) |
| Transcripts | [`docs/transcripts/README.md`](../../transcripts/README.md) and 50 episodes read in full: 144, 145, 201–203, 206, 207, 213, 215–225, 227, 228, 230, 233–238, 240–247, 250, 251, 253, 255, 269, 270, 274, 275, and 284–289. Episodes 174 and 232 are cited from excerpts. |

The episode transcripts are machine transcriptions. Quotes from them record
what was said, not what was true.

Where this audit cites evidence that exists only in the reference-only
standalone checkout (`/Users/christopherdavid/work/psionic`, HEAD `a69c7abb`),
it says so. Line counts are committed Rust lines from `git ls-files`.

## Inventory

`crates/psionic` is its own Cargo workspace. The root `Cargo.toml` excludes it,
and no crate outside it links a Psionic crate. Everything outside reaches
Psionic over loopback HTTP: `crates/pylon/src/engine.rs`,
`crates/inference/src/upstream/psionic.rs`, and
`crates/gateway/src/inference_pylon.rs`. `crates/psionic` was in-tree until
`feb4d45007` (2026-03-16), then re-imported as 30 crates in `737f94a17c`
(2026-10-07, upstream `02e0bc85`, 2,414 files). The only change since the
import is Clef M1 (`3168c986aa`).

"Tassadar files" counts tracked `.rs` files under the crate whose names contain
`tassadar`. Status: **real** means it executes what it claims, at scale or in
production; **partial** means it executes, but only at toy scale or over a
narrow surface; **scaffold** means contracts, receipts, or descriptors with
little or no execution.

| Crate | Lines | Tassadar files | Purpose | Status |
| --- | ---: | ---: | --- | --- |
| `psionic-train` | 417,177 | 87 | Training lanes, run contracts, Pylon training protocol, receipts | Scaffold, with partial trainers (see below) |
| `psionic-eval` | 162,211 | 366 | Eval ledger, legal benchmark, Tassadar gates, Parameter Golf eval | Partial: the eval runtime and legal extraction are real; the judge is a mock |
| `psionic-serve` | 146,426 | 46 | `psionic-openai-server`: chat, responses, embeddings, Clef `/v1/systemone` | **Real**: production on coderos-4080 |
| `psionic-runtime` | 97,595 | 152 | Runtime plus the Tassadar CPU reference executor | Partial: about 75% of it is Tassadar |
| `psionic-models` | 71,847 | 48 | GGUF families, model descriptors | Real for serving families |
| `psionic-research` | 49,757 | 268 | Tassadar summary renderers, research runner | Scaffold: 7 of 9 experiment families are synthesized |
| `psionic-data` | 37,406 | 19 | Dataset contracts, BPE trainer, CS336 refinery, Psion corpus | Partial |
| `psionic-compiler` | 26,599 | 45 | Graph compilation | Partial |
| `psionic-backend-cuda` | 26,422 | 0 | 92 quantized and serving kernels, some backward kernels | **Real** |
| `psionic-ir` | 25,802 | 20 | IR plus reverse/forward-mode autodiff (`autodiff.rs`, 8.4k lines) | Real. Serve builds IR graphs only for its reference word-decoder and embedding paths (`lib.rs:10149,11376`). None of the production families (Qwen3.5 GGUF, GPT-OSS, Clef) uses it, and autodiff is used only by training lanes |
| `psionic-cluster` | 24,339 | 4 | Ordered state, sharded-serving topology over psionic-net | Partial and dormant: Pylon runs with mesh disabled |
| `psionic-provider` | 21,448 | 116 | Compute-market supply policy and receipt types | Scaffold: only tests use it |
| `psionic-router` | 16,630 | 49 | `FleetRouter`, tool loop | Partial |
| `psionic-backend-metal` | 16,177 | 0 | 39 shaders, with no gated-delta or conv1d | Partial: Qwen3.5 recurrent state runs on the host |
| `psionic-sandbox` | 15,027 | 25 | Subprocess and container runner | Partial: the subprocess path isolates nothing |
| `psionic-net` | 10,308 | 0 | Bespoke UDP mesh, relay, signed introductions | Real, but duplicates iroh |
| `psionic-environments` | 8,486 | 4 | Environment registry, rollout receipts | Scaffold: replays recorded turns and has no policy in the loop |
| `psionic-catalog` | 7,265 | 6 | Ollama blob catalog | **Real**: the live load path |
| `psionic-apple-fm` | 6,770 | 3 | HTTP client for a Swift FM bridge | Partial; duplicates `crates/lev` |
| `psionic-transformer`, `-array`, `-nn`, `-core` | 23,601 | 11 | Tensor and module stack above the IR | Partial |
| `psionic-optimize` | 4,622 | 0 | GEPA-shaped optimizer engine | Scaffold: one deterministic lineage-merge proposer and a test sequence proposer. There is no reflective (model-driven) proposer, so it is not GEPA in practice |
| `psionic-collectives` | 3,858 | 0 | Collective planning and fault semantics | Scaffold: no data plane |
| `psionic-datastream` | 3,818 | 0 | Chunk and resume manifests, weight-broadcast manifests | Scaffold: no network transport |
| `psionic-backend-cpu` | 3,227 | 0 | Graph executor | Partial: the served CPU decoders are hand-written and bypass it |
| `psionic-adapters` | 1,707 | 0 | `LmHeadLoraAdapterArtifact` | Partial: LM-head-only LoRA, CPU binding only |
| `psionic-observe`, `-backend-tests` | 998 | 0 | Tokio telemetry; backend conformance (4 ops) | Partial: no backward or optimizer cases |

Total: about 1.23M lines. About 1,270 Tassadar-named Rust files exist across
all crates, and about 1,330 tracked files mention Tassadar.

Outside Psionic, these are the training-related surfaces:

| Surface | Lines | Status |
| --- | ---: | --- |
| [`training/lev-adapter`](../../../training/lev-adapter) | Python | **Real, ran once.** Apple toolkit 26.0.0 LoRA (rank 32, 4 epochs). The artifacts are gitignored. |
| `training/baseline`, `compiled-functions`, `score-probe` | Python | Real measurement harnesses |
| [`training/tenant-demo`](../../../training/tenant-demo) | Python | Scaffold: placeholder tensors, declared metrics |
| [`crates/tenancy/src/training.rs`](../../../crates/tenancy/src/training.rs) | 1,740 | Real governance: corpus partitions, `assess_headroom`, `seal_candidate`. It has no trainer. |
| [`crates/gym`](../../../crates/gym/src/lib.rs) | 110,396 | **Real**: 10 gates, about 58 suites, 32 result files, receipt-chained store |
| [`crates/gym-bridge`](../../../crates/gym-bridge/src/sources.rs) | 3.2k | Real reader for `openagents.gym-training-summary.v1` (`sources.rs:76`). **Nothing produces that schema.** |
| `crates/verse-gym` | 4.5k | Real boards: EVALS, league, results, replay. No training view. |
| `crates/ext-eval`, `eval-runner`, `coderbench` | 33.6k | Real eval engines; the hosted runner is live on coderos-4080 |
| [`crates/inference/src/outcomes.rs`](../../../crates/inference/src/outcomes.rs) | — | Real online loop. It adjusts routing scores from paid outcomes, not weights. |
| `crates/kev` | — | Inference port of upstream Kev (candle). Training is upstream `kev/train.py`, run on Modal. |
| `crates/pylon` | 8.4k (src); 10.7k with tests and examples | Real P1 free inference on one machine; P2–P4 built on test sats. No training. |

## What training actually runs today

There are four kinds of evidence: measured in this repository, measured only
upstream, authored (typed in by hand), and sample.

### Measured, in this repository

1. **Lev adapter**
   ([`docs/lev/measurements/2026-09-19-adapter-v1.md`](../../lev/measurements/2026-09-19-adapter-v1.md)).
   On support-v2 (98 evaluation items), accuracy rose from 0.77 to 0.90 and
   Brier from 0.154 to 0.103. Log loss got worse (1.952 to 2.323), and confident
   errors rose from 6 to 8. On `coder-turns-v1` the gain did not transfer
   (lev-adapted 0.71 against lev-base 0.74). The adapters were trained on 20
   items that were later locked into `support-v2-three-way`, a leakage the
   measurement doc records.
2. **Tassadar executor promotion gates**
   ([`crates/psionic/fixtures/tassadar/runs/`](../../../crates/psionic/fixtures/tassadar)).
   Sudoku v0 promotion v2 **failed**: `first_32_token_exactness_bps 6875`, 0
   exact traces. v3 **passed** (10000 bps, 2 exact traces), but by training only
   biases (`relative_target_output_bias_plus_trace_schema_bias`).
3. **A1 minimal distributed LM** (`fixtures/psion/a1_minimal_distributed_lm`).
   Four steps of an LM-head-only update. Loss went from 5.6137 to 5.5658, and
   two contributions were aggregated on one host.
4. **Parameter Golf XTRAIN**
   ([`parameter_golf_xtrain_quick_eval_report.json`](../../../crates/psionic/fixtures/parameter_golf/reports/parameter_golf_xtrain_quick_eval_report.json)).
   16 steps. It generated `[6,8,6,8,…]` against expected `[5,6,7,8,…]`, with 0
   exact-prefix tokens and about 100 tok/s served. Served and direct outputs
   matched. Its own detail field says "explicit non-public promotion hold". This
   is the only measured train-to-serve round trip in the repository, and it is
   toy-scale.
5. **Apple adapter reference overfit**
   (`fixtures/apple_adapter/runs/`). 96 steps took the average benchmark score
   on 7 cases from 4997 to 10000 bps (pass rate 4285 to 10000). Four cases
   improved; the other three already scored 10000. The run name says
   "reference_overfit", so this measures memorisation.
6. **Clef M1 serving**
   ([`docs/inference/clef-native.md`](../../inference/clef-native.md),
   `fixtures/clef/`). The encoder showed 0 differences on 200 records. The head
   differs from torch by at most 2.3e-6 in any logit. End to end, Clef picked the
   same top answer as the f32 reference on 94.2% of questions, with max |Δp|
   0.177. **The M1 gate as written is not met** (it required 100% and |Δp| ≤
   0.02 against llama.cpp). This is serving, not training. It is listed because
   Clef is the most likely first training target.

### Measured, only upstream

These are not imported into this repository.

- **Qwen3.5-0.8B legal MLX LoRA runs** (`fixtures/qwen_legal/real_finetune/`,
  2026-05-20 to 05-22). The no-cheat suite scored a model-only pass rate of
  1851 bps on the *training* slice. The scaffold scored 370 bps worse. The run
  sets `retained_score_claim: false`, and the judge matches keywords as
  substrings. Run names say "simulated_pylons".
- **The "4080 decision-grade" and MLX executor runs**
  (`tailrun_admitted_device_matrix_20260327b`). They ran 47,104 and 93,184
  steps, both with `final_mean_loss 0.0`. These are open-adapter LM-head runs
  that run on the host CPU under a `cuda` label
  (`psionic-train/src/open_adapter.rs:1739`) and memorise a small batch.
- **Tassadar article transformer weight production.** One trainable tensor
  (`logits_projection_bias`, 303 scalars), one step, 625 bps token exactness,
  and 0 held-out exact matches, yet `weight_production_contract_green: true`.

### Authored or sample, presented beside measured data

| Artifact | Where the numbers come from |
| --- | --- |
| Psion pilot pretraining run bundle (2048 steps, 52,105 tok/s, $18.95) | Literals in `psionic-train/examples/psion_run_observability_fixtures.rs:43-60` |
| Psion actual-pretraining status `running_pretrain`, step 4096 | Hardcoded in `examples/psion_actual_pretraining_status_surface_fixtures.rs:31-39` |
| Actual-pretraining `record-checkpoint` step 16384 | String literals in `examples/psion_actual_pretraining_operator.rs:1972-1979` |
| Psion held-out loss receipt (1410 to 1120) | Typed in `examples/psion_pilot_pretraining_run_fixtures.rs:49-67` |
| Optimizer and scheduler ablation metrics | Literals at `psion_executor_optimizer_ablation.rs:439-473` and `psion_executor_scheduler_ablation.rs:315-347` |
| Google live training visualization bundle (loss 2.182, L4, 50,200 tok/s) | `sample_google_live_visualization_bundle`, `remote_training_visualization.rs:1546` |
| Research runner results for 7 of 9 families | `synthesize_scores`, `psionic-research/src/runner.rs:1586`, a closed-form formula |
| Tenant training demo metrics | Declared, with placeholder tensors (`training/tenant-demo`) |

None of these artifacts says whether it is measured or authored. They share
schemas with the measured ones.

### What each trainer actually trains

| Lane | Path | What it trains | Gradient | Device |
| --- | --- | --- | --- | --- |
| Psion reference pilot ("Pilot32m") | `psionic-train/src/psion_reference_pilot.rs:1239` | Mean of token and position embeddings, then a tied head. No attention or MLP. | Hand-derived | CPU; accelerated variant on CUDA/Metal |
| CS336 A1 reference | `cs336_a1_reference_training.rs:425` | d_model 2, one layer | Finite differences | CPU |
| CS336 A1 real-gradient | `cs336_a1_real_gradient_reference.rs` | Tiny full transformer | Analytic f64, checked against central differences | CPU |
| Parameter Golf single/8×H100 | `parameter_golf_single_h100_training.rs`, `…_distributed_8xh100_train_step.rs` | 9×512 GPT | `psionic_ir` autodiff, Muon | **CUDA**; ranks exchange over loopback TCP |
| Open adapter (Qwen legal SFT/DPO/GRPO, Gemma, GPT-OSS, swarm) | `open_adapter.rs` | LM-head LoRA over precomputed hidden states | Host mat-vec | **CPU always** |
| Qwen3.6-27B "real LoRA" | `qwen36_real_lora_sft.rs:33,602` | `lm_head.weight` LoRA; one prompt, 7 candidate tokens | Hand AdamW | CPU |
| Qwen3.8 LM-head LoRA | `qwen38_training_adapter.rs:680` | LM head; 13 targets deferred | Reference backward | CPU |
| Apple FM adapter | `apple_adapter.rs:1740` | LoRA A/B per target | Host | CPU |
| Tassadar executor | `tassadar_executor_training.rs:783` | Head, biases, embeddings, one mixer | Hand-derived | CPU |
| Tassadar article transformer | `tassadar_article_transformer_training.rs:673` | hidden 8; only `logits_projection.bias` changed | Finite differences | CPU |
| Coordinator evolution | `coordinator_evolution.rs` | ~10K-parameter head | Sep-CMA-ES (no gradient); toy fitness | CPU |

The CUDA backend holds real backward kernels (`rms_norm_*_backward`,
`rotary_embedding_backward`, `attention_causal_sequence_backward_to_f32`), but
only Parameter Golf uses them. The served decoders (`qwen35.rs`, `gguf.rs`,
`gpt_oss.rs`, `clef/`) are hand-written forward-only code. None of them refers
to `psionic_ir`, `psionic_nn`, or `psionic_transformer`. **No family we serve
can be trained by Psionic's autodiff stack** without a second implementation.

Serving has three seams where a trained artifact can be loaded:

1. The Qwen3.8 LM-head LoRA on CPU (`psionic-serve/src/qwen35.rs:36,1912`).
2. A Gemma4 CUDA `promote_exported_revision` overlay (`gguf.rs:11874`), which nothing
   calls, including tests.
3. A Parameter Golf promoted-bundle service (`lib.rs:6193`), which only
   examples call.

## Tassadar, Psion, Percepta, Pylon: history, promises, reality

### Percepta and Tassadar

Percepta's posts (2026-03-11 and 03-25) showed programs *compiled* into
transformer weights. Tassadar was the OpenAgents program built on that work:
a psionic executor lane, a Rust ALM compiler, the Bitcoin-paid run
`run.tassadar.executor.20260615`, and the surfaces around it.

What was promised:

| Episode | Promise |
| --- | --- |
| 216 | "we've more or less reproduced the Percepta paper" |
| 236 | "the largest decentralized training run we think ever in history… 200 contributors is the number to beat" |
| 237 | "an indefinite distributed training run that pays its contributors Bitcoin for verified work… because it learns from its own accepted work, it only grows sharper" |
| 238 | "it's paying people"; the whiteboard said "No gradient descent! (Optional add-on later)" |
| 240 | A walkable 3D run board: 11 pylons, 21 windows, 12 verified items, 1,020 sats |
| 243 | "Psionic… runs in every Pylon to help train the Tassadar run" |
| 274, 275 | Psionic, Pylon, and Tassadar "will all be folded into the open agents product suite" (274); Coder's CLI gets a "Training" area for "the Tassadar idea … distributed training" (275) |

What the evidence shows
([history doc](../../history/2026-09-28-tassadar-percepta.md)):

- The peak was 5 contributors paid, 1,020 real sats, and 11–12 accepted traces.
  The run re-verified one trivial program, `loop_sum_v1`, and "constructs no new
  capability".
- The first "paid" 5-sat pair was later reclassified as simulated. The first
  real settlement was 1,000 sats over Spark.
- Learned executors reached 0 of 2 exact (`attention_boundary_v5`: 7188 bps on
  the first 32 tokens). The fast route is a Rust interpreter over fixture rule
  tables, not weights.
- 237's "learns from its own accepted work" had no gradient mechanism behind
  it. 238 says so on the whiteboard.
- Tassadar was retired on 2026-07-08 "until cashflow-positive". The
  [revival proposal](../../roadmap/2026-09-28-tassadar-revival.md) keeps only
  the verified work loop: claim, run, validator replay, pay, plus honesty flags.

The W3 student sweep is the one durable scientific result. Learned exactness
failed (H1 supported). A frozen exact core with a learned interface reached
pass@1 1.0 (H2 supported). This audit treats that result as design law:
**train the routing and judging interface; keep exactness in deterministic
tools.**

What survives in the code: about 1,150 Tassadar-named Rust files compile into
the serving closure (about 1,270 across all crates; about 1,330 tracked files
mention Tassadar). No route is mounted, and the in-repo history doc's claim
that "no Tassadar code remains" is now false. Two in-repo overclaims are new
findings (see Gaps 3 and 4):

- `tassadar_rust_only_article_closeout_audit.rs:99` hardcodes "Psionic now
  reproduces the full Rust-only Percepta article claim end to end".
- The reference-linear exactness gate calls `roundtrip_from_batch`
  (`psionic-models/src/tassadar_article_transformer.rs:1333`), which tokenizes
  and decodes the CPU trace without running the model.

### Deletion and re-import history (git)

Tassadar code left this repository five times and came back once. The history
doc was written on 2026-09-28, before the re-import, so its "no Tassadar code
remains" was true then and is stale now.

| Commit | Date | What left or returned | Size | Reason |
| --- | --- | --- | --- | --- |
| `1943b0125a` | 2026-03-15 | First Tassadar work (Psionic in-model execution audit) | — | Start of the program |
| `feb4d45007` | 2026-03-16 | Psionic extracted to its own repo: 46 Tassadar `.rs` files (`psionic-runtime/src/tassadar.rs`, eval, models `tassadar_executor_{attention,transformer}.rs`, train `tassadar_executor_{training,telemetry,run}.rs`, research) | −31,766 Tassadar lines (−36.9M total, mostly checkpoint JSON) | Extraction to pinned git deps, not retirement. Work continued upstream (255 Tassadar commits, last `4f36914f` on 2026-06-23) |
| `f5919c7669` | 2026-06-09 | Desktop "Tassadar Lab" replay pane | part of the Bun rebuild | Rebuild |
| `e1fbd1c185` | 2026-07-08 | `packages/tassadar-executor`, Pylon `tassadar-capability.ts`, `tassadar-trace-client.ts`, `tassadar-cpu-transform-training.ts`, `packages/proof-replay`, replay clips, `docs/tassadar`, `docs/training`, `docs/gym` | 276 files, −337,581 | Owner mandate `5f87fd63a2`: retired "until cashflow-positive". Archived to backroom `openagents-prune-20260708-tassadar-psionic` |
| `9bf6be5191` | 2026-07-08 | `packages/tassadar-executor` restored the same day | +272,290 | The Worker had 13 runtime imports; Khala 502 incident |
| `d613b8ea22` | 2026-08-28 | Worker run, trace-factory, trace-contribution routes, `tassadar-run-settlement.ts`, Percepta receipts (with `apps/openagents.com`, `apps/pylon`) | 84 Tassadar files, −24,967 | Wave 1 (#266): Rust-only repo |
| `fae80bde79` | 2026-08-28 | `packages/tassadar-executor` for good | 30 files, −272,045 | Wave 3 (#268): Rust-only repo |
| `dabc08102f` | 2026-09-18 | Remaining Tassadar satellite docs | part of "Nuke" | Docs cleanup |
| `737f94a17c` | 2026-10-07 | Rust Tassadar re-imported as the dependency closure of `psionic-openai-server` (from psionic `02e0bc85fe`) | +1,269 `.rs` files, about 389k lines (+1.32M total) | Psionic serving import, not a revival |

The workspace guide cites `d7f53fccc` as the February DSPy/RLM prune. That
object does not exist; the prune is `17aa21b544` (2026-02-25), which predates
Tassadar and contains none of it.

What the deleted TypeScript did:

- **Executor** (`packages/tassadar-executor`): ran `TassadarAlmNumericModel`
  v1, the "Append-only Lookup Machine" from the Rust ALM compiler. Sparse wiring
  rows, hard-max parabolic attention (`keyed_read`, `cum_sum`), gated FFN rows,
  and KV writes, in f64 inside a checked 2^53 window with typed refusals. Its
  trace digest was byte-identical to Rust on `tassadar-poc-loop-sum-v1`. The
  stated claim boundary was "faithful re-execution of digest-pinned compiled
  workloads only — no softmax, no learning, no serving, no performance claim".
- **Replay** (`replay.ts`): full replay plus window spot checks that named the
  tampered step.
- **Capability envelope** (`capability-envelope.ts`): a Pylon could declare a
  capability only after a self-test receipt.
- **Kernel-optimization parity** (`kernel-optimization-parity.ts`, 340 lines,
  recoverable with `git show fae80bde79^:packages/tassadar-executor/src/kernel-optimization-parity.ts`):
  the acceptance rule for `compute.agentic_kernel_optimization_at_scale.v1`. An
  agent-written kernel was accepted only with an exact-replay verdict from an
  independent device *and* a public tok/s record beating the baseline. A wrong
  but faster kernel was rejected.

Live run evidence: the first live closeout (`7bf1f01c47`, 2026-06-10) matched
the Rust digest over 80 steps, and the Worker validator accepted a clean trace
and rejected a tampered one. The first real settlement was 1,000 sats over Spark
(`89b718fa1c`, #5232), then 5+5 sats per verified pair (`ffbcb76f62`). The
1,020-sat, 5-contributor, 11–12-trace peak above is from `15f614f46f`.

The W3 result above came from the standalone repo after extraction: next-token,
aux-state, and lookup-init students all scored 0.0 pass@1; a frozen analytic
executor with a learned interface scored 1.0 pass@1 and 1.0 replay acceptance,
with median divergence at step 512.

What is in the tree now from `737f94a17c`: the ALM compiler is intact (12
`tassadar_alm_*.rs` files in `psionic-compiler/src`, about 8.5k lines,
including `tassadar_alm_trace_replay.rs` at 348 lines), the runtime executor
(`psionic-runtime/src/tassadar.rs`, 13,233 lines), and the capability envelope
(`psionic-provider/src/lib.rs:630`). Only 5 of about 1,293 upstream Tassadar
fixtures came over, and `psionic-tassadar-student` and `TRAIN_TASSADAR` did not.
The TypeScript executor, Pylon verbs, Worker run, and settlement are gone.
Outside `crates/psionic` and `docs/`, `git grep -i tassadar` is empty. The
replay idea already lives on in `crates/plugin/src/replay.rs` (`59ab5a2816`,
#9901): receipt `openagents.plugin-invocation-receipt.v1` with verdicts
`exact_replay | failed | unverifiable`.

### Psion

Psion is psionic's learned-model family: corpus, tokenizer, pretraining
(`./TRAIN`), SFT, decentralized contribution, and an executor lane.

What was promised:

- 216: "We're calling it Psion".
- 220: small specialised "local, edge, agentic tool use" models.
- 223: "train like first a very basic language model… throw it out".
- 224: CS336 assignments as paid "homework".
- 237: Psion as the "overarching class of models".

What the code shows:

- The "Pilot32m" model is a bag of embeddings.
- "Actual pretraining" writes receipts and records step counts no code reaches.
- CS336 A1 is a finite-difference trainer at d_model 2. A2 has single-host
  DDP receipt examples.
- **No Psion model has been trained to a usable checkpoint.**
- DiLoCo, which 220, 222, and 224 said had been ported from Prime and Templar,
  appears nowhere in this repository's code, only in transcripts.

### Pylon

Pylon has meant three different things:

| Era | What "Pylon" was |
| --- | --- |
| Ep 144 (2024) | A Tauri MCP desktop server |
| Eps 203–238 | A NIP-90 compute provider. It reached "over 1,300 Pylons" by ep 224, counting duplicate instances. It was first paid for presence; 224 switched it to paying for work. It was deleted with `apps/pylon` in `d613b8ea22` (2026-08-28). |
| Now | A Rust rebuild in [`crates/pylon`](../../../crates/pylon/README.md) on NIP-PYLON (30200, 30201, 3201) with NIP-CJ free text jobs on a loopback Psionic server |

Current state, per [`verse-compute.md`](../../compute/verse-compute.md):

- P1 is live on one machine: coderos-4080 serving Qwen3.5 0.8B Q8_0, one
  allowed buyer, transient units.
- P2 (checks), P3 (paid, on TestLightning), and P4 (agent market) are built and
  tested on test sats only.
- **No Pylon trains anything.** The kitchen-sink ledger records the decision:
  X6 "Paid distributed training on home machines… Paused; not part of the
  current product. Psionic stays as the local inference engine and for
  research."

## Gaps and risks

Severity: **S1** blocks a safe or truthful training product, **S2** blocks
progress or adds material cost, **S3** is hygiene.

1. **S1 — Forgeable Pylon training receipts next to settlement code.**
   - The worker and scheduler signing keys are SHA-256 hashes of public
     identifiers:
     `deterministic_worker_signing_key`
     (`psionic-train/src/qwen_legal_pylon_training_job.rs:1840`) and
     `scheduler_signing_key` (`qwen_legal_pylon_dispatch.rs:1024`). The two
     swarm runtimes use the same pattern (`swarm_first_live_runtime.rs:1861`,
     `psion_google_two_node_swarm_runtime.rs:1370`).
   - The dispatcher has a `Production` mode (`qwen_legal_pylon_dispatch.rs:36,387`)
     and builds `Payable` payment decisions (`:791`). The Bitcoin
     settlement-proof and Treasury/Nexus handoff schemas are in
     `qwen_legal_pylon_training_job.rs:21-29,229-244`.
   - More non-test key derivers exist: `coordinator_live_buymode_dispatch.rs:727-735`,
     `adapter_reference_program.rs:948`, and
     `signed_node_identity_contract.rs:1084`.
   - The worker writes placeholder outputs (`:890`, `:1809`).
   - Anyone can forge a worker receipt or a job envelope. This is safe only
     while nothing pays on it.
2. **S1 — Authored numbers share schemas with measured ones.**
   - See the "Authored or sample" table above.
   - No field distinguishes measured, authored, or sample data, so any Verse
     board, `/promises` row, or tweet built on these files would publish
     fiction as evidence.
   - This repeats the history's failure mode: the ep 237 placeholder loss
     curve, the simulated 5-sat payment, and ep 216's "reproduced Percepta".
3. **S1 — Capability gates check that artifacts exist, not that models work.**
   - Weight production is green at 6.25% exactness.
   - Reference-linear exactness is a tokenizer round trip.
   - The trained-v1 promotion sets `primary_bundle_ref`
     (`psion_executor_trained_v1_promotion.rs:926`) to the open-adapter tailrun
     bundle `tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors`,
     which is not in this repository.
   - `*_green: true` and `public_*_claim_allowed: true` fields are unsafe to
     surface.
4. **S2 — Overclaiming text compiled into the product.**
   `tassadar_rust_only_article_closeout_audit.rs:99` and the
   `psion_executor_4080_*` "decision-grade" labels contradict the in-repo
   history doc.
5. **S2 — The serving build carries about 630k lines of training and research
   code.**
   - `psionic-serve/Cargo.toml:46,51,55` depend on eval, research, and train.
   - Serve imports about 170 identifiers from them. About 25 `src/tassadar_*`
     publication modules call about 100 `psionic_eval` report builders and
     constants. The rest are `PsionRouteKind`/`PsionRouteClass*`, the Gemma E4B
     adapter types, `load_qwen38_lm_head_adapter_safetensors`, and the
     Parameter Golf promoted types.
   - Every `psionic-openai-server` build (`scripts/pylon-psionic.sh:84`), and so
     every Pylon host setup, compiles all of it. A Pylon checkout with its build
     output is about 20 GB (`NEEDS_OWNER.md:71`); how much of that is Psionic
     training code was not measured.
6. **S2 — The imported code cannot be verified here.**
   - Of 504 fixture paths cited by eval, research, and data, 491 are missing.
     Of 468 cited by `psionic-train`, 359 are missing.
   - There is no `scripts/` or `docs/` directory under `crates/psionic`.
   - The repository has no `.github/workflows`.
   - About 480 `*_matches_committed_truth` tests exist, and many will probably
     fail because their fixtures are missing. This was not run.
   - The only way to run these lanes is `PSIONIC_TRAIN_RUNTIME_ROOT` pointed at
     the standalone checkout, which the workspace rule forbids.
7. **S2 — No served family is trainable, and adapters cannot reach the GPU.**
   - `psionic-adapters` is LM-head LoRA only, bound to the CPU decoder
     (`gguf.rs:639`).
   - MLX-trained attention LoRAs cannot be served by `psionic-openai-server`.
   - The deferred q/k/v/o/gate/up/down backward is named as "the next
     implementation step" (`qwen36_real_lora_sft.rs:442`).
8. **S2 — No consented, labelled data pipeline.**
   - The 2026-10-09 policy (#11044) permits training on user chats, but the
     gateway keeps no prompt text, Pylon logs no content, and feedback has no
     path to a corpus.
   - `trace-admit` is proposed, not built.
   - `docs/data/schema.md` has no tables for corpora, recipes, trials, or
     candidates, so locked-partition reads are coordinated only by local file
     locks.
   - Labelled corpora are tiny: support-v2 has 196 items; coder-turns has 95
     states, with a locked 65 unread.
9. **S2 — Distribution shift erases authored-suite gains.** Lev's +13 points did
   not transfer to `coder-turns-v1`, and kev-8b answered `none` on 14 of 16 real
   turns. The binding input is real-workload labels, not compute.
10. **S2 — Fine-tuning has made confidence worse.** The Lev adapter's NLL and its
    confident-error count both rose. Clef-Flash is under-confident: AUC
    0.85–0.91, but recall at threshold 0.5 is only 0.29–0.50. Any training
    objective has to include calibration, gated by the calibration rule in
    `crates/gym/src/gate.rs` (gate `probability-v2`).
11. **S2 — Training outputs cannot be sold or served.**
    `gateway/src/decision_offer.rs:15,46` hardcodes `kev-0.6b`, rejects any
    adapter identity, and accepts CPU f32 only. A sealed tenant candidate can
    never reach a Pylon decision offer.
12. **S2 — The Gym training board has no producer.**
    `openagents.gym-training-summary.v1` is read (`gym-bridge/src/sources.rs:76`)
    but never written. The Verse README's "Next" item (live Pylons, training
    windows, verified work, sats) is still open.
13. **S2 — The legal benchmark judge is a mock.** `MockLegalBenchmarkJudge` is
    the only `LegalBenchmarkJudgeAdapter`. `JudgeMode::Llm`, `MultiJudge`, and
    `Human` are enum values with no implementation. Any RL reward built on it
    optimises keyword hits.
14. **S2 — Hardware is thin and unreliable.**
    - One RTX 4080 (16 GB), flagged for machine-check errors and a 3 MB/s
      uplink, shared by Pylon, Verse, and evals.
    - One disk-starved M5 Max.
    - No cloud GPUs. About $850/month goes on orphaned GCE CPU hosts
      (`docs/cloud/2026-10-02-cloud-parallel-execution-audit.md`).
15. **S2 — The Apple base-signature treadmill.** Toolkit 26.0.0 does not support
    OS 27. Every OS base change invalidates Lev adapters and calibration maps.
16. **S3 — Two stacks for the same jobs.**
    - Two decision runtimes: `crates/kev` on candle, and Clef in `psionic-serve`.
    - Two Apple FM bridges: `crates/lev` and `psionic-apple-fm`.
    - Two P2P stacks: `psionic-net` UDP and iroh.
    - Two provider receipt vocabularies: `psionic-provider` and `nostr::pylon`.
17. **S3 — Production code asserts an error.**
    `.expect_err("bounded Metal surface should refuse flatten today")` at
    `open_adapter.rs:1845` breaks when Metal gains the capability.
18. **S3 — Repository hygiene.**
    - `crates/psionic/target/` (448 MB) is untracked and not ignored, since the
      root `.gitignore` only matches `/target`.
    - `fixtures/clef/e2e/cmp-llama-vs-reference.json` and
      `cmp-psionic-vs-llama.json` start with `skip` lines, so they are not valid
      JSON.
    - GGUF-gated Clef tests return early and pass as skipped.
    - In `auto` mode the uncommitted M2 work falls back to the CPU when a
      weight type is unsupported (Q6_K is refused at `clef_cuda.rs:268-272`).
      The only signal is a stderr line (`clef/mod.rs:339-345`) and the
      receipt's device field. Its `eprintln!` calls trip the clippy
      `print_stderr` lint, which committed serve code already violates.
    - The stale gate files `crates/gym/gates/probability-v1.json` and
      `probability-v2.json` still name `lev::calibrate::admit`, which was
      deleted.
19. **S3 — `psionic-research`'s runner reports synthesized results as
    `status=succeeded`.** Any hillclimb over those 7 families is fake.
20. **S3 — Training throttle default.** `training_cpu_budget.rs` defaults to one
    core unless `PSIONIC_TRAIN_CPU_BUDGET` is set, so any real local run is
    throttled.

## Doc and code contradictions

| Doc claim | Reality |
| --- | --- |
| [`crates/psionic/README.md`](../../../crates/psionic/README.md) and [`docs/psionic/README.md`](../../psionic/README.md): training programs and research lanes were "not imported"; `fixtures/` holds "only the 23 files" | train, eval, and research are present through serve's dependencies; 208 fixtures are tracked |
| [History doc](../../history/2026-09-28-tassadar-percepta.md): "No Tassadar code remains in this repository"; "openagents has no `crates/psionic`" | About 1,270 Tassadar-named Rust files (about 389k lines), re-imported in `737f94a17c` on 2026-10-07, after the history doc was written (it was earlier removed in `feb4d45007`); see [Deletion and re-import history](#deletion-and-re-import-history-git) |
| Workspace guide: the DSPy/RLM prune is commit `d7f53fccc` | No such object; the prune is `17aa21b544` (2026-02-25) |
| [`docs/psionic-and-pylon.md`](../../psionic-and-pylon.md): the repository "holds no Psionic or Pylon source" | `crates/psionic` and `crates/pylon` both exist |
| `docs/glossary.md`: Pylon, Psionic, and Tassadar are "Historical" | Pylon and Psionic are live; there is no entry for NIP-PYLON or in-repo serving |
| `docs/compute/compute-for-bitcoin.md` and `docs/history/2026-09-25-transcript-roadmap.md`: Pylon and Psionic are "outside this repository" | Both are in-repo |
| [`docs/roadmap.md`](../../roadmap.md): Pylon and Psionic are "Optional infrastructure/research… do not make the current coding milestone depend on them" | Clef/System One (#11194) now runs on Psionic, and Pylon P0–P4 build on it |
| `docs/compute/verse-compute.md:3`: "Nothing on this page is implemented yet" | P1–P4 are marked built on the same page |
| `docs/verse/relevance-visualizer.md:63`: the `psionic` lane "is offline" until the native door exists | Clef M1 landed the native door |
| `docs/lev/apple-fm-surface.md`: no Apple FM code in-repo; `improvement-strategy.md`: toolkit "is not on this machine" | `psionic-apple-fm` was imported; the adapter was trained 2026-09-19 |
| `docs/kev/mesh-plan.md` Phase 6: training payments "stop at payable… while Treasury owns the wallet" | Treasury is deprecated; payouts go through `pay-ledger` and `nostr::pylon` |
| `docs/os/2026-09-28-coderos-audit.md:107`: drop CUDA and inference from CoderOS | `scripts/pylon-psionic.sh` builds CUDA Psionic on coderos-4080 |
| `docs/repo/size.md`: an 18.9 MB Tassadar `checkpoint_state.json` is "In HEAD: yes" | Only `promotion_gate_report.json` is tracked in that directory |
| `docs/verse/agent-trainer-leveling.md`: "no upload… no trace endpoint" | #11109 trace upload is Done |
| Kitchen-sink README L81-85 and decision 8: "accept every payment rail" | Ledger I8/K6 and twitter decision 1: Bitcoin and Bitcoin-based stablecoins plus card |
| Six Verse/Alice calibration docs say `NEEDS_OWNER.md` lists the live calibration run | No such entries exist |
| `docs/transcripts/README.md` claims 289 episodes | The complete episode index ends at 288 (L779); 289 appears only in the coverage table (L54). There is no Psionic/training row in the historical-to-current table, and the three meanings of "Pylon" (144, 203–238, current) are not disambiguated |
| Clef route spec: `--decision-chunk` default 2048 | Committed code uses 256; the uncommitted M2 work makes it device-dependent |

## Themes the roadmap must honor

From the transcripts and the kitchen sink:

1. **Demand first.** Principle 8, ledger X6 and X12, and twitter decision 10.
   - Supply-side markets failed twice for want of buyers: GPUtopia (174, 213)
     and training payouts (224).
   - Our own products (the Coder router, Jev decisions, file relevance) are the
     first buyer of any training work.
   - Paid distributed training stays dropped until a new issue brings demand.
2. **Pay for accepted work, never uptime** (224, J2). Pay the validator as well
   as the worker (238), and pay only on receipts the recipient can dereference
   (237). Use Bitcoin rails only.
3. **No claim without proof.**
   - "No receipt means no light" (250).
   - Fixtures are "labeled illustrative and never published as measurements"
     (243).
   - "Measures replace forecasts" (twitter decision 9).
   - No "largest run" copy without a public count (twitter decision 10).
   - Promotions require held-out evaluation, as the withdrawn Microluna v7 clip
     showed (`docs/transcripts/README.md` L288-294).
4. **Improves, does not depreciate** (242, 243). Usage should produce signal
   that measurably improves the served system. Express it as a Gym time series
   of accepted outcomes per dollar (G9), and later per kilowatt-hour, the
   metric 232 and 237 named.
5. **Train the interface, not exactness.** The W3 result, the revival doc, and
   the late archive's optimizer (287): "the same labels could train a dedicated
   routing model". Decision models route and judge; tools own exactness.
6. **Consented, scrubbed, paid data.**
   - E6 (providers do not train on chats).
   - Principle 7 (delete means delete).
   - H3/H4 (sell scrubbed traces).
   - The transcripts README rule: "Access to a trace is not automatic
     permission to… train on it".
   - Engrams stay encrypted (NIP-AE); per-agent adaptation runs only on the
     owner's machine.
7. **The Verse draws only what a reader can check.** The 240 run board is the
   precedent. M4 is Missing. Decision 6: the Verse becomes a launch surface only
   when it shows your own agents' real work.
8. **One conversation, plain words.** No training mode switch. Users see
   "checked", "learned from your fixes", and "faster answers", never system
   words like "projection" or "receipt digest".
9. **Local and edge first, phones supervise.** 145; 201's goal of moving work
   local so the cloud share drops "from 100% to … 95% … 50%? 20%?"; 250's
   "fall back to using a local model"; and X15. Training and local serving live
   on desktops and connected computers.
10. **Velocity.** Lean verification (owner memory). Gates must be cheap; 270
    cut 25-minute builds to 5.
11. **Psionic's one concrete public obligation** (twitter item 11, G8, E9,
    #11131): re-runnable local-inference tokens/sec by model and GPU, within
    10%, late October.
12. **The standalone `psionic` repo is reference only.** Engine work lands in
    `crates/psionic`; docs and issues live in this repository.
