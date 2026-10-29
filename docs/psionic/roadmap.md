# Starting point and roadmap

Status, 2026-10-29: chapters 1–8 implement M0/M1, the artifact slice of M5,
text requests and bounded causal Qwen-style layers on the tested Apple LP64 CPU
host. Autodiff and full model execution remain `planned`.
[Run the first chapter](../../psionic/book/ch01-learning-a-number.md)
or `psionic/check.sh`; [retained verification](../verification/psionic-m0-2026-10-28.md)
records the scope. Later chapter titles are working names; gates determine
completion, not a calendar or page count.

## M0: learn one number

The first [shared library](../../psionic/lib/learn.coax) and book chapter
implement `prediction = w * x`, mean squared error, a derivative, an update,
and a held-out prediction. They use fixed data:

| Quantity | Value |
| --- | --- |
| Training inputs | `[1, 2]` |
| Training targets | `[2, 4]` |
| Initial parameter | `w = 0` |
| Loss | `mean((w*x - y)^2)` |
| Gradient | `mean(2*x*(w*x - y))` |
| Learning rate | `1/8` |
| Held-out example | `x = 3`, target `6` |

The initial loss is exactly `10`, the initial gradient is `-10`, and the first
update gives `w = 1.25` and loss `1.40625`. These are exactly representable
binary values and make an independent first-step oracle. For later steps,
the exact-arithmetic parameter error shrinks by `3/8` per update; derive the
finite-step prediction bound before examining implementation output, with
an explicit allowance for `f32` rounding.

Write the checks before implementing the model. Include refusal of empty or
mismatched data, non-finite inputs, invalid step counts, and insufficient
output/scratch capacity. Keep training-step and memory limits in a small
authority unit. Pure checks establish finite cases; compiled native tests
exercise non-finite bit patterns, real storage, and the generated ABI.

The chapter is complete when a fresh checkout can run one documented local
entry point that:

1. Checks and mutation-tests admission with zero survivors.
2. Runs the same library in evaluator checks and native debug/optimized builds.
3. Verifies the exact first update, a fixed-step training result, and the
   held-out prediction against independently specified expectations.
4. Exercises malformed inputs and destination preservation under ASan/UBSan.
5. Checks chapter source references, stable output, and expected exit status.

Publish the real observed transcript only after running that command. Do not
add a model download, GPU prerequisite, server, or large external dataset to
this first milestone.

## Chapter 2: shapes and storage — implemented

[The chapter](../../psionic/book/ch02-shapes-and-storage.md) and
[shared tensor unit](../../psionic/lib/tensor.coax) admit contiguous `f32`
shapes of rank 0–4, canonical inactive dimensions, bounded element/byte counts,
row-major strides and checked coordinates. A staged copy exercises exact
logical input length, writable capacities, finite data, native span observations
and unchanged storage on refusal. The 1,048,576-element cap is this teaching
profile; larger model buffers need an explicitly qualified profile.

[The verification record](../verification/psionic-ch02-2026-10-28.md) retains
Coax checks/mutation and actual native debug/release/sanitizer evidence. M1 is
complete with chapter 3 below, under its explicit teaching-profile limits.

## Chapter 3: numbers and matrices — implemented

[The chapter](../../psionic/book/ch03-numbers-and-matrices.md) implements
`CPU-f32-wide-v1`: binary32 storage, explicit binary64 products and compensated
reductions, one checked narrowing per output, and staged publication. The
[operation contracts](../../psionic/lib/ops-contract.coax) and
[kernels](../../psionic/lib/ops.coax) cover add, row sum/dot, matmul/matvec,
row gather, binary-masked softmax, and unscaled RMSNorm/LayerNorm. Empty rows,
invalid masks/indices, non-finite data and computed range failures have explicit
refusals. Matmul work is capped at 1,048,576 multiply-add pairs.

[The retained verification](../verification/psionic-ch03-2026-10-28.md) combines
authored checks/mutation, independent Decimal(120) fixtures and real native
storage/magnitude/limit cases. Learned affine normalization, broadcasting,
strides, larger model resource profiles and new backends remain separate work.
Chapter 4's bounded original-weight reader is now implemented below; the
Elephant/Mammoth priority remains unchanged.

## Chapter 4: reading original weights — implemented

[The chapter](../../psionic/book/ch04-reading-original-weights.md) and
[model-io units](../../psionic/lib/model-io/) implement `original-weights-v1`:
bounded original Safetensors BF16/F32 inventory, checked exact-byte SHA-256
identities, complete single/sharded bundles, a bounded HF index subset and
caller-pinned name/dtype/shape/shard manifests. Inventory admits large original
tensors independently of the CPU execution cap; finite decoding stages windows
of at most 1,048,576 values into shared numerical storage.

[Verification](../verification/psionic-ch04-2026-10-29.md) covers owned original
files, malformed artifacts, digest/index/manifest mismatches, native aliases
and all BF16 encodings in debug/release with sanitizer and mutation gates.
The artifact slice of M5 is implemented; optimizer/RNG/data-state resume is
still planned. Real Elephant/Mammoth checkpoints are not yet qualified. Next
is the attention/forward core, using chapter 5's text contract below.

## Chapter 5: text becomes a model request — implemented

[The chapter](../../psionic/book/ch05-text-becomes-a-model-request.md) and
[text library](../../psionic/lib/text.coax) implement `qwen35-text-v1`: original
Qwen tokenizer indexes, Unicode-17 NFC and ranked byte-BPE, 33 added literals,
frozen tagged choice/noul/score requests, 256 one-token slot labels and exact
shared-prefix/suffix metadata. Typed entry, byte/token/work and JSON budgets
are explicit; integer structured JSON is supported without floating formatting.
The pinned schema accepts score sizes 1–10, correcting the earlier 2–10 plan.

[Verification](../verification/psionic-ch05-2026-10-29.md) retains independent
Unicode/regex/BPE fixtures, actual original data, rendered-request/token IDs,
mutation and native debug/release/sanitizer evidence. The evidence uses an
owned oracle, not an executed upstream model runtime. Upstream reference
capture, broader request qualification and complete Elephant/Mammoth execution
remain model gates. Chapter 6 now provides the first bounded causal attention
block; no training dependency has been added to the urgent decision-model path.

## Chapter 6: attention without training — implemented

[The chapter](../../psionic/book/ch06-attention-without-training.md) and
[attention library](../../psionic/lib/attention.coax) execute a bounded causal
dense decoder block with learned RMS scaling, grouped-query attention,
caller-supplied interleaved rotary tables, SwiGLU and an incremental KV cache.
Independent full-sequence fixtures and native storage/refusal checks qualify
this small CPU profile. It does not implement Qwen3.5's exact hybrid block or
the decision mask. Chapter 7 and real-model qualification remain next.

[Chapter 7](../../psionic/book/ch07-a-hybrid-sequence-model.md) composes
model-specific causal linear and full-attention layers. [Its verification](../verification/psionic-ch07-2026-10-29.md)
scopes the small CPU references; original checkpoints, the decision mask and
upstream runtime comparisons remain separate gates.

## Chapter 8: decision masks and snapshots — implemented

[The chapter](../../psionic/book/ch08-decisions-have-a-different-mask.md)
implements bidirectional full-attention blocks and snapshots of live recurrent,
convolution and KV state. Whole-model branch isolation and original-weight
request comparisons remain separate gates.

## Delivery priority: Elephant, then Mammoth

The owner priority is [native Elephant and Mammoth](model-targets.md), with
shared foundations reused by the later [nanochat pipeline](nanochat.md).
[The live Psionic project](https://openagents.com/projects/psionic) contains
the existing decision/nanochat dependency graph, acceptance checks and stable
issue slugs. The NVIDIA breadth and expanded contents below are proposals;
this update does not add project issues. The existing queue source
is [work/roadmap.json](../../apps/openagents-web/projects/psionic/work/roadmap.json).
The signed queue records M0 complete; chapters 2–3 implement M1 and chapter 4
implements the artifact slice of M5; chapter 5 implements the bounded text profile in source.
This change does not publish issue-status updates.

| Priority | Work | Must finish first |
| --- | --- | --- |
| Now | Pin both model bundles/nanochat, reference contracts and license/provenance; qualify pinned real artifacts with chapter 4 and extend model-specific reference capture and attention over the shared CPU core. | Chapters 1–8 are shipped in bounded CPU profiles. Each real model still needs its own larger execution/resource profile. |
| Next | Qualify original model artifacts, exact Qwen tokenizer/decision prompts, independent reference capture, and model-specific numerical extensions. | Each task's data/math prerequisites; no training subsystem. |
| Next | Qwen3.5 dense blocks, block-bidirectional full attention, causal DeltaNet, complete prefix forks and slot readout. | Shared operations/artifacts and exact prompt/state fixtures. |
| First real model | Complete original-weight Elephant text decisions. | All hybrid layers, exact renderer/readout, bounded state and independent full-model oracles. |
| Immediately after | Complete original-weight Mammoth using the same engine. | Elephant implementation, Mammoth-specific dimensions, shards, memory and real-weight evidence. |
| Then | Published decision semantics: load exactly the calibration the pinned bundle supplies, reproduce serialized answers and measure held-out decision quality. | Each model's numerical evidence; quality and conformance stay separate claims. |
| Alongside this path | A memory residency plan, byte-identical multi-worker CPU execution and batched/reused prefix forks. | Each model's CPU reference; parallel and batched results must equal the single-worker sequential ones. |
| Alongside this path | Coax compute consumer, generated Metal kernels, Elephant then Mammoth device qualification. | Concrete shapes/operations and each model's CPU numerical reference; no generic training dependency. |
| After original-weight baselines | One separately qualified quantized profile, shared System One serving integration and the voice engine's decision gate as the first consumer. | Full-model numerical evidence and explicit artifact/profile/contract boundaries. |

CPU reference correctness and device execution can advance together. Do not
wait for a fast full-model CPU service or finish the entire teaching spine
before starting compute support. Current scalar Metal emission is insufficient
for these kernels. Reference Python/OneJev is tooling; native model support
requires actual complete Coax execution.

Smaller synthetic fixtures and source-contract work need no paid hardware.
A large-weight reference/target run records its actual machine, memory and
compute admission when undertaken. No budget, deployment or model download is
a prerequisite for starting the ready work.

## Proposed book order

The [full proposed table of contents](../../psionic/book/PLAN.md) now gives
one outline per chapter, with prerequisite notes. Chapters 1–8 are runnable;
chapter numbers and part boundaries are provisional. The new
[NVIDIA breadth survey](nvidia-breadth.md) records pinned candidate checkpoints,
operator requirements, data studies and source-license qualifications.

| Part | Chapters | Result the reader builds toward |
| --- | --- | --- |
| I. Small numbers, real contracts | 1–3 | Scalar learning → bounded shapes/storage → dense CPU numerics. |
| II. Open weights early | 4–12 | Weights/tokenizer → attention and Qwen hybrid → decision masks/state/readout → **Elephant, then Mammoth** → calibrated, evaluated decisions. |
| III. Measurable, parallel, portable execution | 13–19 | Memory plan and CPU evidence → byte-identical multi-core CPU → batched/reused prefix forks → a Coax compute subset → Metal → one quantization profile → optional CUDA. |
| IV. Useful inference | 20–23 | Sampling and constrained decoding; Nemotron embeddings search the Coax book; Minitron continues text; search runs in the browser. |
| V. Learning foundations | 24–29 | Arrays/autodiff → our own physical surrogate → byte LM → complete tiny trained decoder. |
| VI. Data, tokenizers and optimization | 30–32 | Provenance, packing/resume, deterministic BPE training and exact AdamW/Muon updates. |
| VII. nanochat and post-training | 33–41 | Exact forward/backward → pretraining → SFT/tools; separately specified continued pretraining, preferences, nanochat rollouts, decision post-training and Mammoth-to-Elephant distillation. |
| VIII. Vision and generative primitives | 42–46 | Image operations → Elephant/Mammoth image inputs → C-RADIO features → older Cosmos image tokenizer → tiny diffusion/flow model. |
| IX. Motion for Verse | 47–50 | Poses/codebooks → MotionBricks SOMA → explicit Verse animation integration. |
| X. Speech | 51–55 | Audio/spectral features → FastConformer → Parakeet TDT transcription → streaming transcription in the voice engine → text-to-speech. |
| XI. Scientific ML | 56–60 | Spectral layers/Darcy FNO; geometry/DoMINO; optional independent FourCastNet3 weather branch. |
| XII. More sequence architectures | 61–64 | Tiny state-space → Nano hybrid → tiny MoE → Lightning. |
| XIII. World models and policies | 65–69 | One Cosmos3-Edge path; separately, tiny imitation → conditional GR00T study → simulated control loop. |
| XIV. Operate and scale | 70–72 | Shared System One serving, a decision model steering the voice engine, and one real distributed-training topology. |
| Optional laboratories | A–D | SmolLM2-135M import; Mini/Bolt assistant architectures; higher-order derivatives/PINNs; activation and state inspection. |

Several chapters need Coax compiler features that do not exist yet: GPU
compute and WGSL emission, jobs and threads, `f16`/`bf16`, numeric generics,
SIMD, allocators and a Linux target. The plan's
[Coax feature map](../../psionic/book/PLAN.md#coax-features-each-branch-needs)
names where each is needed and what Psionic does until it exists.

The first NVIDIA application should be **semantic retrieval with Nemotron
embeddings**. The preferred first major Verse ML demonstration is
**MotionBricks-generated skeletal motion**. These follow the owner's urgent
decision-model path. Small solver-generated physical learning can start
immediately after autodiff, well before the large scientific checkpoints.

## Delivery branches and their gates

This is a dependency map, not a requirement to execute the fourteen parts
serially. Library milestone labels M0–M9 retain their existing coverage meaning;
the broader model studies extend that map without changing signed issue IDs.

| Branch / existing milestone coverage | What it actually needs | Completion gate |
| --- | --- | --- |
| M0: scalar learning — implemented | Current tested CPU subset | `psionic/check.sh`, retained native/mutation/sanitizer/book evidence. |
| M1: bounded CPU foundations — implemented | M0 | Chapters 2–3 ship shape/storage and add/sum/dot/matmul/gather/masked-softmax/unscaled-normalization operations. Exact small fixtures, Decimal references, resource gates and native debug/release/sanitizer evidence qualify the named CPU teaching profile. |
| Urgent decision inference | M1, bounded artifacts/tokenizer, exact Qwen hybrid and decision contract | Complete original-weight Elephant, then Mammoth; masks, prefix isolation, slot probabilities and real-weight request evidence, then the published calibration and held-out decision quality. Chapters 4–12. |
| M5: artifacts implemented; resumable state planned | Artifact admission after M1; optimizer/data state once training exists | Chapter 4 implements bounded Safetensors/BF16/F32 and exact identities; later deterministic packing and model/optimizer/RNG/data-state resume. Chapters 4 and 30. |
| M6–M8: performance, devices, quantization | Concrete operators and CPU references; original-weight evidence before quantization claims | Measured memory/latency budgets, byte-identical multi-worker CPU and batched forks, real generated-kernel execution, device lifetimes and separately qualified approximate profiles. Chapters 13–19 accompany decision work. |
| NVIDIA retrieval / dense continuation | Shared forward operations/artifacts plus each target's tokenizer, masks and configuration | Token/intermediate/output reference checks plus held-out retrieval or full/cached logits; named-RNG sampling and grammar-constrained decoding; a separately qualified browser target. Chapters 20–23. |
| M2–M4: generic learning | M1 → arrays/autodiff → byte LM and complete causal decoder gradients | Analytic/finite-difference checks and complete tiny training/held-out generation. Chapters 24–25 and 27–29. |
| Early physical learning | Arrays/autodiff and a checked owned solver | Supervised surrogate, held-out initial conditions/parameters, one-step and rollout errors. Chapter 26; no language-model prerequisite. |
| nanochat pipeline | Exact enabled forward/backward features, deterministic tokenizer training, data/resume and AdamW/Muon | Tiny complete pretraining → SFT → pinned on-policy reward recipe; stage-specific numerical, gradient and held-out gates. Chapters 30–36 and 39. |
| Additional post-training | Shared training/evaluation; specific data/objective contracts | Continued pretraining, preference/reward/DPO, decision adaptation and Mammoth-to-Elephant distillation are separate studies. Chapters 37–38 and 40–41; decision adaptation and distillation do not wait for nanochat RL. |
| Vision | Shared forward core and exact image/patch/convolution operators; autodiff for trained heads | C-RADIO feature/retrieval checks; separately qualify the older Cosmos tokenizer and tiny diffusion/flow. Chapters 42–46. Elephant/Mammoth image inputs are chapter 43's own exact encoder/preprocessing profile. |
| Verse motion | Shared attention, required codec operators, rotations/skeleton/codebooks | Complete SOMA root/pose/decoder trajectory references before Verse integration. Chapters 47–50; speech and physics do not block it. |
| Speech | Shared convolution/attention and exact spectral/audio features | Parakeet encoder/decoder references and WER/CER; streaming transcription and a separately selected text-to-speech model get measured latency budgets in the voice engine. Chapters 51–55. |
| Scientific ML | Shared training/spectral core for Darcy; geometry operators for DoMINO; spherical operators for weather | Solver comparisons, bounded datasets, exact preprocessing and physical evaluation. Chapters 56–60; DoMINO and FourCastNet3 are independent specialties. |
| New language architectures | Dense inference and exact recurrence; explicit router/expert execution for MoE | Tiny state transitions and expert choices, then complete Nano/Lightning checkpoint evidence. Chapters 61–64; optional Mini/Bolt retain distinct profiles. |
| World generation | Mature vision, exact codecs, temporal conditioning and diffusion/flow methods | One complete bounded Cosmos3-Edge path; reference agreement and physical quality are separate tests. Chapters 65–66. |
| Robot policies | Shared vision/language, training/flow, embodiment normalization and tiny imitation | Conditional GR00T artifact selection and one simulated action/control-loop profile. Chapters 67–69; Cosmos execution is not a prerequisite. |
| M9: integration and scale | A qualified model plus protocol consumer / a correct resumable trainer plus selected backend | Versioned shared System One integration and the voice engine's never-awaited decision gate / one actual homogeneous multi-device topology. Chapters 70–72 can start as their own prerequisites arrive. |

Do not wait for a fast full-model CPU service before starting Metal compute.
Likewise, choosing NVIDIA weights does not imply CUDA support or require CUDA
for correctness. Coax owns arithmetic on each backend; native bindings provide
platform effects. A reference runtime remains development tooling.

The existing [project issue map](project.md) remains the executable work queue
for its 38 tasks. When selecting a new breadth branch, create concrete issues
from that branch's operator/data/checkpoint gates. This proposal creates no
new issues, model downloads, hardware purchases or deployment requirements.

## What support means

Each model/backend/profile earns its own evidence. Metadata or a synthetic
layer test means partial implementation, not full real-weight support.
Elephant does not qualify Mammoth, and text support does not qualify vision.
Original BF16 file size does not establish f32-expanded residency or available
GPU memory. Quantized outputs get their own approximation/quality gates.

For nanochat, distinguish forward/checkpoint compatibility, tokenizer behavior,
all enabled gradients, optimizer updates, whole pretraining, SFT and RL. The
pinned speedrun has no standalone mid-training stage and runs SFT after base
training; RL is separate. Our proposed continued-pretraining and preference
chapters are extensions with their own contracts. Nanochat's reward recipe
does not depend on a separately trained reward model or establish generic PPO
compatibility.

For broader models, distinguish downloadable weights, reference-code terms,
training recipes and usable data. The [NVIDIA survey](nvidia-breadth.md)
records the C-RADIO code/weight distinction, MotionBricks sample-data limits,
GR00T's unresolved card/license inconsistency, older/current Cosmos boundaries
and restricted Nemotron pretraining data. The Darcy recipe is not a qualified
pretrained artifact. A stable physical rollout, a transcription evaluation or
an exact skeleton/action interface requires evidence beyond tensor execution.

Keep M0 and the tiny complete decoder as cumulative regressions. Write each
chapter from its running library result, and keep incomplete material labelled
planned. A missing device or reference artifact does not turn an implementation
failure into a pass; record the concrete missing action and keep useful local
work moving.
