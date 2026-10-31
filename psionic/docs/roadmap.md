# Starting point and roadmap

<!-- capability-claim: psionic.execution.current -->
Status reviewed 2026-10-31: the recorded CPU profiles execute original Elephant
and Mammoth text decisions in [chapters 10](../book/ch010-execute-elephant.md)
and [11](../book/ch011-execute-mammoth.md). The device records cover Elephant
on CUDA and Metal, and Mammoth on CUDA; [chapter 31](../book/ch031-elephant-on-metal.md)
has not qualified Mammoth on Metal.

Original gpt-oss 20b has complete recorded CUDA and Metal generation paths in
[chapters 45b](../book/ch045b-a-complete-cuda-decoder.md) and
[45h](../book/ch045h-gpt-oss-on-metal.md), plus the bounded resident CUDA
Responses profile in [chapter 48b](../book/ch048b-serve-responses.md).
The original 120b CPU reference does not qualify a 120b GPU path. The bounded
V4.1 component records do not qualify a complete V4.1 model.

CPU autodiff is implemented as a scalar binary64 tape in
[chapter 71](../book/ch071-gradients-as-a-program.md) and first derivatives of
32-node f32 tensor programs in
[chapter 71b](../book/ch071b-differentiate-any-tensor-program.md). General
function transforms, higher derivatives and device backward execution remain
planned. The [book status table](../book/README.md) names each delivered scope;
model quality, broader request coverage and additional targets retain their
own qualification gates.
<!-- /capability-claim -->

The [device delivery sequence](../book/PLAN.md#gpt-oss-device-delivery-order)
keeps the remaining target, scheduling and throughput work separate from those
recorded profiles. No CPU reference or component result promotes another model,
device or serving profile.

[Run the first chapter](../book/ch001-learning-a-number.md)
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

[The chapter](../book/ch002-shapes-and-storage.md) and
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

[The chapter](../book/ch003-numbers-and-matrices.md) implements
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

[The chapter](../book/ch004-reading-original-weights.md) and
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
still planned. This reader chapter does not itself qualify whole-model
execution. Chapters 10 and 11 record the separate original Elephant/Mammoth
CPU execution profiles, using the text and forward contracts below.

## Chapter 5: text becomes a model request — implemented

[The chapter](../book/ch005-text-becomes-a-model-request.md) and
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
capture and broader request qualification remain separate from this text
chapter. Complete Elephant/Mammoth execution has its own recorded CPU profiles
in chapters 10 and 11. Chapter 6 provides the first bounded causal attention
block; no training dependency has been added to the decision-model path.

## Chapter 6: attention without training — implemented

[The chapter](../book/ch006-attention-without-training.md) and
[attention library](../lib/attention.coax) execute a bounded causal
dense decoder block with learned RMS scaling, grouped-query attention,
caller-supplied interleaved rotary tables, SwiGLU and an incremental KV cache.
Independent full-sequence fixtures and native storage/refusal checks qualify
this small CPU profile. It does not implement Qwen3.5's exact hybrid block or
the decision mask. Chapter 7 supplies the separate hybrid-layer profile;
chapters 10 and 11 record whole-model execution against their stated references.

[Chapter 7](../book/ch007-a-hybrid-sequence-model.md) composes
model-specific causal linear and full-attention layers. [Its verification](../../docs/verification/psionic-ch07-2026-10-29.md)
scopes the small CPU references; original checkpoints, the decision mask and
upstream runtime comparisons remain separate gates.

## Chapter 8: decision masks and snapshots — implemented

[The chapter](../book/ch008-decisions-have-a-different-mask.md)
implements bidirectional full-attention blocks and snapshots of live recurrent,
convolution and KV state. Whole-model branch isolation and original-weight
request comparisons remain separate gates.

## Original delivery sequence: Elephant, then Mammoth

The owner priority is [native Elephant and Mammoth](targets/model-targets.md), with
shared foundations reused by the later [nanochat pipeline](targets/nanochat.md).
[The live Psionic project](https://openagents.com/projects/psionic) contains
the existing decision/nanochat dependency graph, acceptance checks and stable
issue slugs. The NVIDIA breadth and expanded contents below are proposals;
this update does not add project issues. The existing queue source
is [work/roadmap.json](../../apps/openagents-web/projects/psionic/work/roadmap.json).
The following table preserves the original delivery sequence and its gating
rationale. Its "Now" and "Next" labels describe that sequence, not current
implementation status. Use the current scoped status above and the
[book status table](../book/README.md) for delivered capabilities. This document
does not publish issue-status updates.

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
before starting compute support. The scalar `coax metal` path alone is
insufficient for these kernels; the shipped `coax metal-compute` subset supplies
buffer, loop and selected cooperative kernels. Each model still needs its own
numerical and target qualification. Reference Python/OneJev is tooling; native
model support requires actual complete Coax execution.

Smaller synthetic fixtures and source-contract work need no paid hardware.
A large-weight reference/target run records its actual machine, memory and
compute admission when undertaken. No budget, deployment or model download is
a prerequisite for starting the ready work.

## Proposed book order

The [full proposed table of contents](../book/PLAN.md) now gives
one outline per chapter, with prerequisite notes. The
[book status table](../book/README.md) lists completed chapters and their
qualified profiles; the plan also contains future outlines. Chapter numbers
and part boundaries are provisional. The
[NVIDIA breadth survey](targets/nvidia-breadth.md) records pinned candidate checkpoints,
operator requirements, data studies and source-license qualifications.

| Part | Chapters | Result the reader builds toward |
| --- | --- | --- |
| I. Small numbers, real contracts | 1–3 | Scalar learning → bounded shapes/storage → dense CPU numerics. |
| II. Open weights early | 4–13 | Weights/tokenizer → attention and Qwen hybrid → decision masks/state/readout → **Elephant, then Mammoth** → calibrated, evaluated decisions → honest qualification against references that cannot be reproduced. |
| III. A live decision API | 14–18 | Time a decision → keep the model resident → serve `/v1/systemone` → run it live → make the numbers move, one measured change at a time. |
| IV. A first mesh of Pylons | 19–21 | A Pylon answers decisions → the API fans out to Pylons serving the same model identity → receipts and sampled checks. |
| V. Measurable, parallel, portable execution | 22–35, 26a | Memory plan and CPU evidence → benchmarks against llama.cpp and cuBLAS → parallel CPU and batched prefix forks → a Coax compute subset → tiled attention and chunked DeltaNet → fusion and graph replay → CUDA on the RTX 4080 → Metal → integer matrix products → GGUF → Q8_0, K-quants and lattice quants; a small tensor IR that runs interpreted, as a static plan, and lowered to the compute subset (26a). |
| VI. Useful inference | 36–40, 38a–38c | More tokenizers and chat templates; sampling and constrained decoding; Nemotron embeddings search the Coax book; Minitron continues text; search runs in the browser; retrieval becomes a full system: BM25 and exhaustive references, an HNSW index, reranking, and grounded answers that survive corrections and deletions (38a–38c). |
| VII. An open reasoning model | 41–48 | Original MXFP4 weights → expert routing → sink/window attention and YaRN → harmony format → gpt-oss-20b, then 120b → speculative decoding → tool calls. |
| VIII. Mistral models | 49–55 | Ministral 3 → original FP8 weights → Devstral tool calls → latent attention → Mistral Small 4 → Large 4's sparse attention, shared layers and draft head. |
| IX. DeepSeek, natively | 56–69 | R1 distill → recipe layer → V2-Lite → self-balancing routers → V4.1's compressed/shared attention, hyper-connections, Engram and FP4 experts → expert cache, layer-major prefill, packed cache, host-free decode steps → V4.1 Flash on one machine → draft verification by expected gain. |
| X. Learning foundations | 70–75, 71a, 72a | Arrays/autodiff → our own physical surrogate → byte LM → complete tiny trained decoder; an early small-model path (deadline prediction), a lab that breaks training on purpose (71a), and a grid world that plans and recovers (72a). |
| XI. Data, tokenizers and optimization | 76–78 | Provenance, packing/resume, deterministic BPE training and exact AdamW/Muon updates. |
| XII. nanochat and post-training | 79–87, 75a | Exact forward/backward → pretraining → SFT/tools; separately specified continued pretraining, preferences, nanochat rollouts, decision post-training and Mammoth-to-Elephant distillation; training within a memory budget (75a) and explicit LoRA adapters in 86. |
| XIII. Vision and generative primitives | 88–92 | Image operations → Elephant/Mammoth image inputs → C-RADIO features → older Cosmos image tokenizer → tiny diffusion/flow model. |
| XIV. Motion for Verse | 93–96 | Poses/codebooks → MotionBricks SOMA → explicit Verse animation integration. |
| XV. Speech | 97–101 | Audio/spectral features → FastConformer → Parakeet TDT transcription → streaming transcription in the voice engine → text-to-speech. |
| XVI. Scientific ML | 102–106, 104a | Spectral layers/Darcy FNO; geometry/DoMINO; optional independent FourCastNet3 weather branch; a scene learned from its views, ending in a small radiance field (104a). |
| XVII. More sequence architectures | 107–109 | Tiny state-space → Nano hybrid → Lightning, reusing the router from V. |
| XVIII. World models and policies | 110–114 | One Cosmos3-Edge path; separately, tiny imitation → conditional GR00T study → simulated control loop. |
| XIX. Host inference in production | 115–129 | Service benchmarks → System One server and clients → continuous batching → paged state and a tiered prefix cache → routing, queueing and autoscaling → cold starts → canary rollout → observability and metering → failure handling → conditional disaggregation and multi-device inference → a voice pipeline and decision steering → distributed training. |
| XX. Inference across a mesh | 130–143 | Stage boundaries with whole-model parity → stage packages → peers, transport and Nostr discovery → placement → lifecycle and fencing → experts across machines and from disk → routing by saved work → exact activation compression → trust levels → re-execution checks and Bitcoin settlement → Mesh-LLM interoperation → DeepSeek V4.1 across stages → Mistral Large 3 → Le Chonk. |
| XXI. A harness for Psionic models | 144–155, 154a, 155a | The simplest agent loop → a session log that rebuilds every request → a byte-stable prefix for the server's cache → typed tools, sandbox and approvals → condensation → programs as tool calls → forks, subagents and jobs → plugins with owned lifecycles → MCP, ACP and Responses → harness benchmarks → consent before any sharing; candidates generated and checked under one budget (154a) and memory that can be corrected (155a). |
| XXII. Decisions in production | 156–164, 164a | Fitted calibration with refusal of stale maps → thresholds from written costs → readings joined to outcomes and drift alerts → Pylon self-checks → measured honest-answer distance → a bounded ONNX reader → a millisecond encoder decision model → a distilled router → shadow promotion; an Elephant/Mammoth cascade from a written consequence table (157), and learning which model to call from partial feedback with off-policy evaluation (164a). |
| XXIII. Programs compiled into weights | 165–177 | A trace-producing machine → attention as exact memory and ReLU arithmetic → a gate-graph compiler and its differential harness → interpreter or program in weights → logarithmic decode → softmax limits → exact replay as a verification class → learned and frozen-core executors → composable weight modules → a run that pays for verified work. |
| XXIV. Measure, then improve | 178–188 | Three-digest suites → noise floors and headroom → gates that can say unverifiable → environments with verified checkers → settings fitted on recorded answers → cheapest-first policy search → reflective prompt evolution → model judges as components → outcomes and verified traces as training rows → accounting for whether the loop improves; an agent that optimizes a Psionic kernel under a fixed budget (188); the book builds toward a small model that picks a cheaper inference path and is promoted only on a written quality-and-cost objective. |
| Framework track | 2a–3b, 4a, 26b–26c, 71b–71c, 74a, 75b–75c, 76a–76b, 78a, 81a–81c, 129a–129c, 161a, 189 | Tensor views and an operator library with a manifest → custom operators → general and composable autodiff over 26a's IR → modules, one trainer, data loading, reusable optimizers → GPU training, profiling, eager versus compiled → whole-experiment checkpoints and interchange (Safetensors writing, DLPack, ONNX export; pickles refused) → distributed tensors, data-parallel and fully sharded training → rebuilding three PyTorch tutorials with public APIs only. Graduation: a new network defined, trained, exported and served without PyTorch. |
| Optional laboratories | A–D | SmolLM2-135M import; Mini/Bolt assistant architectures; higher-order derivatives/PINNs; activation and state inspection. |

Several chapters need extensions beyond the shipped bounded compute subset
and generic copyable procedures and fixed-storage structs: direct WGSL
emission, language jobs and threads, narrow float scalar types, numeric
operation constraints, general SIMD and allocators, and broader target
qualification. The plan's
[Coax feature map](../book/PLAN.md#coax-features-each-branch-needs)
distinguishes the existing boundaries from those remaining language features.

The first NVIDIA application should be **semantic retrieval with Nemotron
embeddings**. The preferred first major Verse ML demonstration is
**MotionBricks-generated skeletal motion**. These follow the owner's urgent
decision-model path. Small solver-generated physical learning can start
immediately after autodiff, well before the large scientific checkpoints.

## Delivery branches and their gates

This is a dependency map, not a requirement to execute the twenty-four parts
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
| Harness | The live API, constrained decoding and the prefix cache | A simplest-loop baseline; requests rebuilt byte for byte from the session log; measured prefix-cache hit rates across long sessions; tool, sandbox and approval tests; condensation cost measured; each addition kept only with a measured gain over the simplest loop; no session content shared without consent. Chapters 144–155. |
| Decisions in production | Calibration and qualification (12–13), the live API and the Pylon mesh | Held-out calibration gates and refusal of mismatched maps; cost-derived thresholds checked by held-out cost per decision; outcome joins and drift alerts; Pylon self-checks before serving; honest-distance limits measured against substitutes; encoder decisions at millisecond latency; distilled routers and shadow promotion. Chapters 156–164. |
| Programs compiled into weights | Chapters 1–3 and 6; training (75) for the learned chapters; the wallet and mesh for paid runs | Compiled weights reproducing a reference interpreter's trace digest bit for bit; a differential harness that catches planted compiler bugs; logarithmic decode equal to the linear scan; measured softmax limits; replay verdicts used by mesh and kernel checks; learned-executor results reported by first divergence and held-out length; composition that replays exactly; payments only for replay-verified work. CPU speed always stated beside model speed. Chapters 165–177. |
| Measure, then improve | A frozen corpus (12), the harness (144), outcome joins (158) and replay verdicts (173) | Suite/question/gate digests with a receipt chain; measured noise floors and headroom; gates as zero-survivor Coax; checkers verified on empty and reference work; settings fitted with zero model calls; policy search with held-out reads after selection; GEPA with a separate reflector and drift checks; judges measured against outcomes; an exposure ledger for training rows; per-cycle cost and ablations. Chapters 178–187. |

Do not wait for a fast full-model CPU service before starting Metal compute.
Likewise, choosing NVIDIA weights does not imply CUDA support or require CUDA
for correctness. Coax owns arithmetic on each backend; native bindings provide
platform effects. A reference runtime remains development tooling.

The existing [project issue map](project.md) remains the executable work queue
for its 38 tasks. When selecting a new breadth branch, create concrete issues
from that branch's operator/data/checkpoint gates. This proposal creates no
new issues, model downloads, hardware purchases or deployment requirements.

## Measured GPT-OSS device baseline

[Chapters 45a–45c](../book/ch045a-original-weights-stay-on-the-gpu.md) now run
original 20b with resident packed weights, a complete greedy CUDA decoder and
128-token grouped prefill. The retained original vectors pass the frozen
numerical bound and exact route/top-token gates; a complete Harmony turn runs.
At 1,024 input / 128 forced output, five warm 4080 repeats give median
**3.219239 s prefill and 29.586 decode tokens/s** after qualified attention/launch tuning. The 2-second prefill goal
remains open. See the [record](../../docs/verification/psionic-gptoss-device-2026-10-29.md).

Next in the GPT-OSS delivery sequence: cooperative Coax compute primitives,
original-MXFP4 matrix products with an explicit arithmetic profile, resident
graph replay, then the complete throughput/serving gate. Larger-device 120b,
fast speculative blocks and schema-masked device tools follow that qualified
foundation. These new lettered chapters preserve all existing chapter numbers.

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
