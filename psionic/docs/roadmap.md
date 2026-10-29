# Starting point and roadmap

Status, 2026-10-29: chapters 1–10 implement M0/M1, the artifact slice of M5,
text requests, bounded Qwen-style layers and authenticated original-weight
Elephant text decisions on the tested Apple LP64 CPU host. Mammoth and autodiff
remain `planned`. The independent [chapter 34 format decoder](../book/ch41-original-mxfp4-weights.md)
now implements bounded original MXFP4 bytes. [Chapter 35](../book/ch42-a-few-experts-then-a-router.md)
adds bounded top-k routing and tiny resident affine mixtures.
[Chapter 36](../book/ch43-attention-with-sinks-and-a-window.md) adds sink/window
attention, split-half YaRN, a bounded biased projection and clamped activation
under a named wide CPU profile. Original full-size projection/block execution,
checkpoint binding and complete gpt-oss execution remain planned.
[Run the first chapter](../book/ch01-learning-a-number.md)
or `psionic/check.sh`; [retained verification](../../docs/verification/psionic-m0-2026-10-28.md)
records the scope. Later chapter titles are working names; gates determine
completion, not a calendar or page count.

## M0: learn one number

The first [shared library](../lib/learn.coax) and book chapter
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

[The chapter](../book/ch02-shapes-and-storage.md) and
[shared tensor unit](../lib/tensor.coax) admit contiguous `f32`
shapes of rank 0–4, canonical inactive dimensions, bounded element/byte counts,
row-major strides and checked coordinates. A staged copy exercises exact
logical input length, writable capacities, finite data, native span observations
and unchanged storage on refusal. The 1,048,576-element cap is this teaching
profile; larger model buffers need an explicitly qualified profile.

[The verification record](../../docs/verification/psionic-ch02-2026-10-28.md) retains
Coax checks/mutation and actual native debug/release/sanitizer evidence. M1 is
complete with chapter 3 below, under its explicit teaching-profile limits.

## Chapter 3: numbers and matrices — implemented

[The chapter](../book/ch03-numbers-and-matrices.md) implements
`CPU-f32-wide-v1`: binary32 storage, explicit binary64 products and compensated
reductions, one checked narrowing per output, and staged publication. The
[operation contracts](../lib/ops-contract.coax) and
[kernels](../lib/ops.coax) cover add, row sum/dot, matmul/matvec,
row gather, binary-masked softmax, and unscaled RMSNorm/LayerNorm. Empty rows,
invalid masks/indices, non-finite data and computed range failures have explicit
refusals. Matmul work is capped at 1,048,576 multiply-add pairs.

[The retained verification](../../docs/verification/psionic-ch03-2026-10-28.md) combines
authored checks/mutation, independent Decimal(120) fixtures and real native
storage/magnitude/limit cases. Learned affine normalization, broadcasting,
strides, larger model resource profiles and new backends remain separate work.
Chapter 4's bounded original-weight reader is now implemented below; the
Elephant/Mammoth priority remains unchanged.

## Chapter 4: reading original weights — implemented

[The chapter](../book/ch04-reading-original-weights.md) and
[model-io units](../lib/model-io/) implement `original-weights-v1`:
bounded original Safetensors BF16/F32 inventory, checked exact-byte SHA-256
identities, complete single/sharded bundles, a bounded HF index subset and
caller-pinned name/dtype/shape/shard manifests. Inventory admits large original
tensors independently of the CPU execution cap; finite decoding stages windows
of at most 1,048,576 values into shared numerical storage.

[Verification](../../docs/verification/psionic-ch04-2026-10-29.md) covers owned original
files, malformed artifacts, digest/index/manifest mismatches, native aliases
and all BF16 encodings in debug/release with sanitizer and mutation gates.
The artifact slice of M5 is implemented; optimizer/RNG/data-state resume is
still planned. Real Elephant/Mammoth checkpoints are not yet qualified. Next
is the attention/forward core, using chapter 5's text contract below.

## Chapter 5: text becomes a model request — implemented

[The chapter](../book/ch05-text-becomes-a-model-request.md) and
[text library](../lib/text.coax) implement `qwen35-text-v1`: original
Qwen tokenizer indexes, Unicode-17 NFC and ranked byte-BPE, 33 added literals,
frozen tagged choice/noul/score requests, 256 one-token slot labels and exact
shared-prefix/suffix metadata. Typed entry, byte/token/work and JSON budgets
are explicit; integer structured JSON is supported without floating formatting.
The pinned schema accepts score sizes 1–10, correcting the earlier 2–10 plan.

[Verification](../../docs/verification/psionic-ch05-2026-10-29.md) retains independent
Unicode/regex/BPE fixtures, actual original data, rendered-request/token IDs,
mutation and native debug/release/sanitizer evidence. The evidence uses an
owned oracle, not an executed upstream model runtime. Upstream reference
capture, broader request qualification and complete Elephant/Mammoth execution
remain model gates. Chapter 6 now provides the first bounded causal attention
block; no training dependency has been added to the urgent decision-model path.

## Chapter 6: attention without training — implemented

[The chapter](../book/ch06-attention-without-training.md) and
[attention library](../lib/attention.coax) execute a bounded causal
dense decoder block with learned RMS scaling, grouped-query attention,
caller-supplied interleaved rotary tables, SwiGLU and an incremental KV cache.
Independent full-sequence fixtures and native storage/refusal checks qualify
this small CPU profile. It does not implement Qwen3.5's exact hybrid block or
the decision mask. Chapter 7 and real-model qualification remain next.

[Chapter 7](../book/ch07-a-hybrid-sequence-model.md) composes
model-specific causal linear and full-attention layers. [Its verification](../../docs/verification/psionic-ch07-2026-10-29.md)
scopes the small CPU references; original checkpoints, the decision mask and
upstream runtime comparisons remain separate gates.

## Chapter 8: decision masks and snapshots — implemented

[The chapter](../book/ch08-decisions-have-a-different-mask.md)
implements bidirectional full-attention blocks and snapshots of live recurrent,
convolution and KV state. Whole-model branch isolation and original-weight
request comparisons remain separate gates.

## Delivery priority: Elephant, then Mammoth

The owner priority is [native Elephant and Mammoth](targets/model-targets.md), with
shared foundations reused by the later [nanochat pipeline](targets/nanochat.md).
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
| Next: a live API | The first production deployment: a timed decision path, a resident model, `/v1/systemone` with typed refusals and deadlines, a supervised live service with golden-request promotion, and measured optimizations (chapters 14–18). | Elephant and Mammoth qualified under chapter 13, an owner-approved host and address, and published latency figures from the same harness that measured them. |
| Then: a first Pylon mesh | Pylons that answer NIP-DEC decision jobs and advertise their served model identity in NIP-PYLON beacons; the same `/v1/systemone` fans out to them, checks the answering identity and falls back or refuses; receipts and sampled re-execution checks (chapters 19–21). | The live API, `coax/lib/nostr`, and owner decisions on relays and any public participation. Payments come later. |
| Alongside this path | A memory residency plan, byte-identical multi-worker CPU execution and batched/reused prefix forks. | Each model's CPU reference; parallel and batched results must equal the single-worker sequential ones. |
| Alongside this path | Coax compute consumer, generated Metal kernels, Elephant then Mammoth device qualification. | Concrete shapes/operations and each model's CPU numerical reference; no generic training dependency. |
| After original-weight baselines | One separately qualified quantized profile, shared System One serving integration and the voice engine's decision gate as the first consumer. | Full-model numerical evidence and explicit artifact/profile/contract boundaries. |
| Production hosting | Service benchmarks, continuous batching, a tiered prefix cache, routing/queueing/autoscaling policy, fast cold starts, canary rollout, observability and metering, all deciding in Coax. | A qualified model with measured latency, a consumer contract, a qualified Linux target for hosted machines and an owner-approved budget. |
| Mesh inference | Pool memory across loosely connected machines, starting with a two-stage dense split, toward Mistral Large 3 and Le Chonk. | Whole-model parity for every split, measured links, an owner decision on public meshes and payments, and Mistral's commercial licence before offering Large 4 to others. |

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

The [full proposed table of contents](../book/PLAN.md) now gives
one outline per chapter, with prerequisite notes. Chapters 1–10 and the
independent MXFP4/router/attention chapters 41–43 are runnable;
chapter numbers and part boundaries are provisional. The new
[NVIDIA breadth survey](targets/nvidia-breadth.md) records pinned candidate checkpoints,
operator requirements, data studies and source-license qualifications.

| Part | Chapters | Result the reader builds toward |
| --- | --- | --- |
| I. Small numbers, real contracts | 1–3 | Scalar learning → bounded shapes/storage → dense CPU numerics. |
| II. Open weights early | 4–13 | Weights/tokenizer → attention and Qwen hybrid → decision masks/state/readout → **Elephant, then Mammoth** → calibrated, evaluated decisions → honest qualification against references that cannot be reproduced. |
| III. A live decision API | 14–18 | Time a decision → keep the model resident → serve `/v1/systemone` → run it live → make the numbers move, one measured change at a time. |
| IV. A first mesh of Pylons | 19–21 | A Pylon answers decisions → the API fans out to Pylons serving the same model identity → receipts and sampled checks. |
| V. Measurable, parallel, portable execution | 22–35 | Memory plan and CPU evidence → benchmarks against llama.cpp and cuBLAS → parallel CPU and batched prefix forks → a Coax compute subset → tiled attention and chunked DeltaNet → fusion and graph replay → CUDA on the RTX 4080 → Metal → integer matrix products → GGUF → Q8_0, K-quants and lattice quants. |
| VI. Useful inference | 36–40 | More tokenizers and chat templates; sampling and constrained decoding; Nemotron embeddings search the Coax book; Minitron continues text; search runs in the browser. |
| VII. An open reasoning model | 41–48 | Original MXFP4 weights → expert routing → sink/window attention and YaRN → harmony format → gpt-oss-20b, then 120b → speculative decoding → tool calls. |
| VIII. Mistral models | 49–55 | Ministral 3 → original FP8 weights → Devstral tool calls → latent attention → Mistral Small 4 → Large 4's sparse attention, shared layers and draft head. |
| IX. DeepSeek, natively | 56–69 | R1 distill → recipe layer → V2-Lite → self-balancing routers → V4.1's compressed/shared attention, hyper-connections, Engram and FP4 experts → expert cache, layer-major prefill, packed cache, host-free decode steps → V4.1 Flash on one machine → draft verification by expected gain. |
| X. Learning foundations | 70–75 | Arrays/autodiff → our own physical surrogate → byte LM → complete tiny trained decoder. |
| XI. Data, tokenizers and optimization | 76–78 | Provenance, packing/resume, deterministic BPE training and exact AdamW/Muon updates. |
| XII. nanochat and post-training | 79–87 | Exact forward/backward → pretraining → SFT/tools; separately specified continued pretraining, preferences, nanochat rollouts, decision post-training and Mammoth-to-Elephant distillation. |
| XIII. Vision and generative primitives | 88–92 | Image operations → Elephant/Mammoth image inputs → C-RADIO features → older Cosmos image tokenizer → tiny diffusion/flow model. |
| XIV. Motion for Verse | 93–96 | Poses/codebooks → MotionBricks SOMA → explicit Verse animation integration. |
| XV. Speech | 97–101 | Audio/spectral features → FastConformer → Parakeet TDT transcription → streaming transcription in the voice engine → text-to-speech. |
| XVI. Scientific ML | 102–106 | Spectral layers/Darcy FNO; geometry/DoMINO; optional independent FourCastNet3 weather branch. |
| XVII. More sequence architectures | 107–109 | Tiny state-space → Nano hybrid → Lightning, reusing the router from V. |
| XVIII. World models and policies | 110–114 | One Cosmos3-Edge path; separately, tiny imitation → conditional GR00T study → simulated control loop. |
| XIX. Host inference in production | 115–129 | Service benchmarks → System One server and clients → continuous batching → paged state and a tiered prefix cache → routing, queueing and autoscaling → cold starts → canary rollout → observability and metering → failure handling → conditional disaggregation and multi-device inference → a voice pipeline and decision steering → distributed training. |
| XX. Inference across a mesh | 130–143 | Stage boundaries with whole-model parity → stage packages → peers, transport and Nostr discovery → placement → lifecycle and fencing → experts across machines and from disk → routing by saved work → exact activation compression → trust levels → re-execution checks and Bitcoin settlement → Mesh-LLM interoperation → DeepSeek V4.1 across stages → Mistral Large 3 → Le Chonk. |
| Optional laboratories | A–D | SmolLM2-135M import; Mini/Bolt assistant architectures; higher-order derivatives/PINNs; activation and state inspection. |

Several chapters need Coax compiler features that do not exist yet: GPU
compute and WGSL emission, jobs and threads, `f16`/`bf16`, numeric generics,
SIMD, allocators and a Linux target. The plan's
[Coax feature map](../book/PLAN.md#coax-features-each-branch-needs)
names where each is needed and what Psionic does until it exists.

The first NVIDIA application should be **semantic retrieval with Nemotron
embeddings**. The preferred first major Verse ML demonstration is
**MotionBricks-generated skeletal motion**. These follow the owner's urgent
decision-model path. Small solver-generated physical learning can start
immediately after autodiff, well before the large scientific checkpoints.

## Delivery branches and their gates

This is a dependency map, not a requirement to execute the twenty parts
serially. Library milestone labels M0–M9 retain their existing coverage meaning;
the broader model studies extend that map without changing signed issue IDs.

| Branch / existing milestone coverage | What it actually needs | Completion gate |
| --- | --- | --- |
| M0: scalar learning — implemented | Current tested CPU subset | `psionic/check.sh`, retained native/mutation/sanitizer/book evidence. |
| M1: bounded CPU foundations — implemented | M0 | Chapters 2–3 ship shape/storage and add/sum/dot/matmul/gather/masked-softmax/unscaled-normalization operations. Exact small fixtures, Decimal references, resource gates and native debug/release/sanitizer evidence qualify the named CPU teaching profile. |
| Urgent decision inference | M1, bounded artifacts/tokenizer, exact Qwen hybrid and decision contract | Complete original-weight Elephant, then Mammoth; masks, prefix isolation, slot probabilities and real-weight request evidence, then the published calibration and held-out decision quality. Chapters 4–12. |
| Live API and first mesh | Qualified Elephant and Mammoth, a resident-model memory plan, the System One contract and the Nostr code in `coax/lib/nostr` | Reproducible per-phase timings and P50/P95/P99 latency from one harness; typed refusals, deadlines and identity-bearing answers; golden-request promotion; Pylon beacons and decision jobs with identity checks, fallback and receipts; sampled re-execution checks. Chapters 14–21. |
| M5: artifacts implemented; resumable state planned | Artifact admission after M1; optimizer/data state once training exists | Chapter 4 implements bounded Safetensors/BF16/F32 and exact identities; later deterministic packing and model/optimizer/RNG/data-state resume. Chapters 4 and 76. |
| M6–M8: performance, devices, quantization | Concrete operators and CPU references; original-weight evidence before quantization claims | Measured memory/latency budgets, byte-identical multi-worker CPU and batched forks, tiled attention under its own profile, real generated-kernel execution, device lifetimes and separately qualified approximate profiles judged by perplexity and decision quality against the originals. Chapters 22–35 accompany decision work. |
| NVIDIA retrieval / dense continuation | Shared forward operations/artifacts plus each target's tokenizer, masks and configuration | Token/intermediate/output reference checks plus held-out retrieval or full/cached logits; named-RNG sampling and grammar-constrained decoding; a separately qualified browser target. Chapters 36–40. |
| gpt-oss | Shared forward core, tokenizer engine, memory plan, sampling and an exact MoE router | Exact MXFP4 decode vectors, sink/window/YaRN attention references, harmony render/parse, complete 20b then 120b original-weight generation with token/router/logit references and measured residency, and schema-constrained tool calls. Chapters 41–48; no training or hybrid-recurrence prerequisite. |
| Mistral | Shared forward core, tokenizer engine, the router and speculative decoding | Ministral 3 references, exact FP8 decode vectors, schema-constrained Devstral tool calls, latent-attention absorbed-versus-expanded checks, Mistral Small 4 original-weight execution and Large 4 component references (indexer selections, shared-layer reuse, soft-capping, draft acceptance). Chapters 49–55; Large 4 licence terms recorded before any hosted use. |
| DeepSeek | Shared forward core, latent attention, the router, FP8/FP4 decoding and chapter 13's qualification rules | R1-distill and V2-Lite references; recipe fixtures with boundary-only specials and chunk-safe streaming; component references for compressed/shared attention, hyper-connections, Engram and FP4 experts; an exact expert cache, layer-major prefill, packed cache and host-free decode measured against the plain path; V4.1 Flash on one machine qualified by frozen distribution policies; variable-length drafts reported as committed tokens. Chapters 56–69, then staged execution in 141. |
| M2–M4: generic learning | M1 → arrays/autodiff → byte LM and complete causal decoder gradients | Analytic/finite-difference checks and complete tiny training/held-out generation. Chapters 70–71 and 73–75. |
| Early physical learning | Arrays/autodiff and a checked owned solver | Supervised surrogate, held-out initial conditions/parameters, one-step and rollout errors. Chapter 72; no language-model prerequisite. |
| nanochat pipeline | Exact enabled forward/backward features, deterministic tokenizer training, data/resume and AdamW/Muon | Tiny complete pretraining → SFT → pinned on-policy reward recipe; stage-specific numerical, gradient and held-out gates. Chapters 76–82 and 85. |
| Additional post-training | Shared training/evaluation; specific data/objective contracts | Continued pretraining, preference/reward/DPO, decision adaptation and Mammoth-to-Elephant distillation are separate studies. Chapters 83–84 and 86–87; decision adaptation and distillation do not wait for nanochat RL. |
| Vision | Shared forward core and exact image/patch/convolution operators; autodiff for trained heads | C-RADIO feature/retrieval checks; separately qualify the older Cosmos tokenizer and tiny diffusion/flow. Chapters 88–92. Elephant/Mammoth image inputs are chapter 89's own exact encoder/preprocessing profile. |
| Verse motion | Shared attention, required codec operators, rotations/skeleton/codebooks | Complete SOMA root/pose/decoder trajectory references before Verse integration. Chapters 93–96; speech and physics do not block it. |
| Speech | Shared convolution/attention and exact spectral/audio features | Parakeet encoder/decoder references and WER/CER; streaming transcription and a separately selected text-to-speech model get measured latency budgets in the voice engine. Chapters 97–101. |
| Scientific ML | Shared training/spectral core for Darcy; geometry operators for DoMINO; spherical operators for weather | Solver comparisons, bounded datasets, exact preprocessing and physical evaluation. Chapters 102–106; DoMINO and FourCastNet3 are independent specialties. |
| New language architectures | Dense inference and exact recurrence; explicit router/expert execution for MoE | Tiny state transitions and expert choices, then complete Nano/Lightning checkpoint evidence. Chapters 107–109; optional Mini/Bolt retain distinct profiles. |
| World generation | Mature vision, exact codecs, temporal conditioning and diffusion/flow methods | One complete bounded Cosmos3-Edge path; reference agreement and physical quality are separate tests. Chapters 110–111. |
| Robot policies | Shared vision/language, training/flow, embodiment normalization and tiny imitation | Conditional GR00T artifact selection and one simulated action/control-loop profile. Chapters 112–114; Cosmos execution is not a prerequisite. |
| M9: integration and scale | A qualified model plus protocol consumer / a correct resumable trainer plus selected backend | Reproducible service benchmarks; versioned System One serving; batch-invariant continuous batching; prefix-cache hit evidence; routing and autoscaling policies checked in deterministic traffic simulation; measured cold starts; canary rollouts with golden-request checks; usage records priced by an `#authority` policy; the voice engine's never-awaited decision gate / one actual homogeneous multi-device topology. Chapters 115–129; disaggregation and multi-device inference only after a measurement shows the need. |
| Mesh inference | A qualified model, two-stage parity and network transport | Split-versus-whole parity on the machines that ran it, verified stage packages, readiness-gated routes with generation fencing, measured placement objectives, saved-work routing, exact activation compression, sensitivity routing that fails closed, sampled re-execution checks, Mesh-LLM interoperation, then DeepSeek V4.1 across stages and Mistral Large 3 and Le Chonk across machines. Chapters 130–143; joining public meshes and paying peers are owner decisions. |

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
training recipes and usable data. The [NVIDIA survey](targets/nvidia-breadth.md)
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
