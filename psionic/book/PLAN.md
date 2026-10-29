# Proposed table of contents

Proposal, updated 2026-10-29. [Chapter 1](ch01-learning-a-number.md) and
[Chapter 2](ch02-shapes-and-storage.md) and
[Chapter 3](ch03-numbers-and-matrices.md) and
[Chapter 4](ch04-reading-original-weights.md) and
[Chapter 5](ch05-text-becomes-a-model-request.md) and
[Chapter 6](ch06-attention-without-training.md) and
[Chapter 7](ch07-a-hybrid-sequence-model.md) and
[Chapter 8](ch08-decisions-have-a-different-mask.md) and
[Chapter 9](ch09-read-the-options.md) are runnable on the tested CPU host.
The remaining chapters below are outlines, not implemented capabilities. A
chapter ships with shared Coax library code, authored checks, independent
native tests, source-linked examples and an exact supported scope. Coax
listings use `jai` Markdown fences.

**The first delivery path is 1–3 → 4–10 (Elephant) → 11 (Mammoth).** Device
work in 12–15 begins as soon as it has concrete operations and CPU oracles.
Neither generic training, a smaller imported model nor the NVIDIA branches
blocks those two decision models. [Model targets](../../docs/psionic/model-targets.md)
defines their exact architecture and acceptance gates.

The contents give a reading order; they do not require finishing every earlier
part before starting a branch. The prerequisite map below governs that choice.
The [roadmap](../../docs/psionic/roadmap.md) sets delivery priorities, while the
[NVIDIA survey](../../docs/psionic/nvidia-breadth.md) pins the proposed external
targets and explains artifact, data and licensing qualifications. New breadth
is proposed here; this revision does not add or renumber live project issues.

## I. Small numbers, real contracts

| Chapter | What we build and check |
| --- | --- |
| 1. Learning a number — runnable | Learn `y = 2x`; derive the loss and gradient, verify the first update, and predict a held-out value. |
| [2. Shapes and storage](ch02-shapes-and-storage.md) — runnable | Admit bounded dimensions, caller-owned buffers, scratch and aliases; refuse malformed work before publishing output. |
| [3. Numbers and matrices](ch03-numbers-and-matrices.md) — runnable | Dot/matmul, gather, stable reductions, softmax and normalization, with independent numerical contracts. |

## II. Open weights early: Elephant and Mammoth

This part introduces inference before automatic differentiation. It depends
on the shared numerics in I and supplies the forward primitives reused later.

| Chapter | What we build and check |
| --- | --- |
| [4. Reading original weights](ch04-reading-original-weights.md) — runnable | Bounded original-weights-v1 Safetensors/shard-index parsing, exact identities, BF16/F32 window decoding and typed manifests; complete inventory and malformed-artifact refusals. |
| [5. Text becomes a model request](ch05-text-becomes-a-model-request.md) — runnable | qwen35-text-v1: frozen Base Labs rendering, Unicode-17 NFC, original Qwen byte-BPE tables, special tokens and exact shared-prefix layouts; explicit resource and structured-entry bounds. |
| [6. Attention without training](ch06-attention-without-training.md) — runnable | A bounded dense causal block, interleaved rotary tables, grouped-query attention and caches; full-sequence versus incremental results. |
| [7. A hybrid sequence model](ch07-a-hybrid-sequence-model.md) — runnable: bounded causal CPU profile | Qwen-style dense blocks, causal convolution and Gated DeltaNet state transitions alongside causal full attention; upstream and real-weight qualification remain separate. |
| [8. Decisions have a different mask](ch08-decisions-have-a-different-mask.md) — runnable: bounded CPU | Block-bidirectional full attention and complete hybrid prefix snapshots, with sequential isolated suffix forks; whole-model qualification remains separate. |
| [9. Read the options](ch09-read-the-options.md) — runnable: bounded CPU | Slot LM-head probabilities, vocabulary mass and typed statistics with explicit temperature; serialization and calibration loading remain separate. |
| 10. Execute Elephant | Complete original-weight text decisions, independent intermediate/logit/request oracles and measured resource admission. |
| 11. Execute Mammoth | Reuse the engine with its own dimensions, shards and memory profile; complete real-weight checks for this checkpoint. |

Text support is the first scope. Image inputs, quantized variants and serving
integration each receive separate qualification. A layer fixture or imported
tensor inventory does not complete either real-model chapter.

## III. Make execution measurable and portable

These chapters accompany real consumers in II; their placement is explanatory.

| Chapter | What we build and check |
| --- | --- |
| 12. Measure the work | Fixed CPU numerical references, peak memory, latency samples and a reproducible workload before optimizing. |
| 13. Coax tensor compute on Metal | The compiler's missing buffer/loop/dispatch subset, generated kernels, bindings and completion/lifetime rules; qualify Elephant, then Mammoth. |
| 14. Quantized weights | One specified block format; independent decode vectors, partial-row refusal, separate model profiles and measured quality/error/memory effects. |
| 15. A second device backend | Optional CUDA branch: Linux/ABI admission, Coax-generated arithmetic, device execution and comparisons with the same CPU oracles. |

NVIDIA model compatibility does not require CUDA. Metal and CUDA are separate
backend projects; neither upstream speed claims nor a reference runtime
establishes Psionic execution.

## IV. Useful inference beyond decisions

| Chapter | What we build and check |
| --- | --- |
| 16. Search the Coax book | Nemotron-3-Embed-1B-BF16: exact query/document formatting, bidirectional attention, padding-aware average pooling, normalization and held-out retrieval. |
| 17. Continue a passage | Llama-3.1-Minitron-4B-Width-Base: exact head dimensions/position rules, full-versus-cached logits and deterministic text continuation. |

These are forward-only applications of I–II, with their own architecture
profiles. The optional small import lab in appendix A can help readers with
limited memory; it is not an extra dependency of Elephant or Mammoth.

## V. Learn with the operations we already own

| Chapter | What we build and check |
| --- | --- |
| 18. Learning with arrays | A small classifier using the shared operations, initially with explicit gradients and SGD. |
| 19. Gradients as a program | A bounded reverse-mode tape, saved values, accumulation and parameter identity; analytic and finite-difference checks. |
| 20. Learn a physical system | A supervised oscillator or cooling surrogate from our own deterministic solver; held-out parameters, one-step error and repeated rollout stability. |
| 21. Bytes and a small language model | A real byte vocabulary, bigram likelihood, fixed licensed splits, named RNG and complete tiny training/generation. |
| 22. Differentiate a decoder | Backward operations for the complete causal attention/FFN path; causal isolation and gradient coverage. |
| 23. Train the whole tiny decoder | A complete small transformer with training, held-out likelihood and generation; keep it as a permanent regression. |

Chapter 20 needs 18–19, not language-model training. It is our first physics
chapter, much earlier than pretrained geometry or weather models. It uses
ordinary supervised learning; higher-order physics-informed derivatives are
a separate extension in appendix C.

## VI. Data, state and optimization

| Chapter | What we build and check |
| --- | --- |
| 24. Data and resumable state | Provenance, disjoint splits, document packing, model/optimizer/RNG/data counters and exact resume under a named profile. |
| 25. AdamW, then Muon | Independent update derivations, parameter groups, precision and the exact pinned nanochat optimizer behavior. |

Data admission begins with the first trainer. These chapters consolidate it
before longer runs rather than postponing provenance until this part.

## VII. Rebuild nanochat, then extend the training pipeline

| Chapter | What we build and check |
| --- | --- |
| 26. Execute nanochat exactly | Pinned BPE/conversation rules, inert checkpoint export, every enabled forward feature and cached inference. |
| 27. Differentiate nanochat | Gradients for every enabled feature; tiny independent comparisons and complete parameter coverage. |
| 28. A complete pretraining run | Initialization, schedules, accumulation, token budgets, packing, resume and held-out loss/bits-per-byte; tiny first, measured scale later. |
| 29. Supervised conversations and tools | Assistant/tool loss masks and validation; bounded Nemotron-Agentic-v1/OpenCodeReasoning subsets with provenance and base-skill regression. |
| 30. Continued pretraining and task adaptation | A separately specified domain workload, data mixture, retention checks and promotion criteria. This is a Psionic extension, not a stage claimed from the pinned nanochat speedrun. |
| 31. Preferences and reward models | HelpSteer3 pairwise data, ranking/reward losses and an optional separately specified DPO experiment; held-out comparisons and reward-model limitations. |
| 32. Learn from rollouts | The pinned nanochat on-policy, mean-centered-reward recipe; bounded rollouts, independent gradient/reward checks and held-out task results. |
| 33. Post-train a decision model | Qualified hybrid backward/adapters, selected trainable regions, cross-entropy/Brier/abstention, resume and held-out promotion. |

The [nanochat study](../../docs/psionic/nanochat.md) is the compatibility
contract. Its current speedrun trains a base model and runs SFT; RL is a
separate program. Chapter 32 depends on SFT and its own reward contracts, not
on completing preference learning or continued pretraining. Chapter 33 needs
the decision forward path and shared training machinery, not nanochat RL.
No generic PPO, KL penalty or broad RLHF parity is implied.

## VIII. Images become features and latent representations

| Chapter | What we build and check |
| --- | --- |
| 34. Pixels, convolutions and patches | Exact resize/channel/range conventions, convolution, sampling and patch projection with tiny independent fixtures. |
| 35. See with C-RADIO | C-RADIOv3-B feature outputs, visual-asset retrieval and a small Psionic-trained head over frozen features. |
| 36. Compress and reconstruct an image | The older Cosmos-0.1-Tokenizer-CI8x8, including wavelets, normalization and decoder; inspect latent/reconstruction numerics. |
| 37. Noise, diffusion and flow | Train a tiny generative example; distinguish objectives, schedules, conditioning and reproducible sampling. |

Chapter 35 needs the shared forward core and 34; its trained head also needs
18–19. Chapter 36 is an older teaching target, not a promise of Cosmos3 codec
compatibility. Chapter 37 introduces the methods later world/robotics models
need; it does not qualify their architectures.

## IX. Motion for Verse

| Chapter | What we build and check |
| --- | --- |
| 38. Poses have conventions | Rotation representations, skeleton hierarchy, root coordinates, units, timing and retargeting fixtures. |
| 39. Motion tokens become poses | A tiny codebook/VQ decoder and masked-token generation; known token-to-pose and pose-continuity checks. |
| 40. Execute MotionBricks | One pinned SOMA package: root timing/trajectory, masked motion-token transformer and VQ-VAE decoder, with full trajectory references. |
| 41. Put generated motion in Verse | An explicit skeleton/target interface, bounded generation and animation handoff; a reproducible destination-to-pose scene. |

This is the preferred first major Verse ML demonstration after useful vision
features. It reuses attention, common image/codebook operators where required,
and motion math; it does not depend on speech, DoMINO, weather or nanochat RL.
Unitree G1 is a separate embodiment/profile, not an interchangeable asset.

## X. Audio becomes words

| Chapter | What we build and check |
| --- | --- |
| 42. Waveforms and spectral features | Sample/rate/channel admission, windowing, tiny DFT → FFT/inverse, spectrogram/mel and exact feature normalization. |
| 43. A speech encoder | FastConformer convolution/attention, subsampling and length/mask propagation, compared against fixed intermediate tensors. |
| 44. Transcribe with Parakeet | parakeet-tdt-0.6b-v3, token-duration-transducer decoding and bounded utterances; WER/CER, language slices and voice-system integration. |

This branch starts from shared convolution/attention and spectral operations;
it need not wait for motion. Spectral primitives are shared with physics.
Offline transcription qualification does not establish streaming latency or
text-to-speech support.

## XI. Fields, geometry and physical prediction

| Chapter | What we build and check |
| --- | --- |
| 45. A spectral neural operator | Tiny DFT/FFT references, complex arithmetic, truncation and spectral layers on a small field; reuse the spectral core from 42. |
| 46. Learn Darcy flow | Reimplement the PhysicsNeMo FNO architecture/recipe on bounded reproducibly generated data; compare against a solver on held-out fields. |
| 47. A field around a shape | Signed distances, point clouds, spatial neighborhood queries, point convolutions and local aggregation with tiny geometric oracles. |
| 48. Execute DoMINO | domino_drivaerml, a bounded DrivAerML subset, exact preprocessing, surface/volume outputs and physical error measures. |
| 49. Forecast on a sphere | Optional FourCastNet3 branch: spherical operators, variable/grid conventions, geographic weighting and multi-step forecast evaluation. |

The first physics result is already in 20. FNO adds field learning; DoMINO
adds geometry-specific operations. Chapter 49 is an alternative specialty,
not a successor that must wait for DoMINO. Its field/activation budgets and
ERA5 acquisition/terms need their own qualification. The Darcy recipe is not
a pretrained checkpoint. None of these models replaces Verse's authoritative
simulation without a separate consumer contract.

## XII. More sequence architectures

| Chapter | What we build and check |
| --- | --- |
| 50. A recurrence you can inspect | Tiny state-space transitions, causal convolution, chunked/whole-sequence versus token-step execution and independent state reset. |
| 51. Execute Nemotron Nano | NVIDIA-Nemotron-3-Nano-4B-BF16: the complete Mamba-2/Transformer hybrid and all recurrent/cache state. |
| 52. A few experts, then a router | A tiny MoE with directly checkable choices, ties, weighting, dispatch/gather and expert-residency budgets. |
| 53. Execute Nemotron Lightning | NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16: combine the exact hybrid and MoE profiles; floating point before NVFP4 or speculation. |

This branch can follow dense inference and performance work without waiting
for motion or physics. Qwen Gated DeltaNet support does not establish Mamba-2
support. Active parameter count is not total checkpoint residency. Bolt's
different Gemma-4 architecture is an optional study in appendix B.

## XIII. World models and embodied policies

| Chapter | What we build and check |
| --- | --- |
| 54. Sequences of observations | Temporal/multimodal representations, a tiny conditioned generation task and codec/scheduler contracts. |
| 55. One complete Cosmos3-Edge path | Qualify one bounded conditioned generation path across its autoregressive/diffusion towers and exact codecs; numerical and physical evaluations stay distinct. |
| 56. Imitation before robotics scale | A tiny policy trained/evaluated in simulation, with action/state normalization and a bounded PhysicalAI SingleArm data exercise. |
| 57. Execute a GR00T policy | GR00T-N1.7-3B as a candidate, subject to its unresolved artifact-license inconsistency; exact VLA/flow-matching action path for one embodiment. |
| 58. Close the simulated control loop | Action chunks, observation timing, resets, held-out task success and failure recovery under the selected embodiment. |

Cosmos needs mature vision, temporal representations and generative methods.
The robotics path needs vision/language, shared training, action conventions
and flow matching; it does not require Cosmos inference first. GR00T is not a
commercial default while its card and included license disagree. Real robot
control would be a separately authorized integration after simulation.

## XIV. Operate and scale qualified models

| Chapter | What we build and check |
| --- | --- |
| 59. Serve a model with an honest capability | Shared System One protocol integration, versioned manifests, bounded requests, cancellation and evidence for each model/backend/profile. |
| 60. Train across devices | One real homogeneous topology: gradient collectives, sharding/resume, numerical comparisons and measured scaling. |

Serving can begin after the first qualified decision model and an actual
consumer contract. Distributed work can begin after a correct resumable
single-device trainer and the selected backend. Neither waits for robotics or
the other breadth branches merely because it is explained here.

## Optional laboratories

| Appendix | Scope and entry point |
| --- | --- |
| A. A small imported decoder | SmolLM2-135M after 4–6; a deliberately non-NVIDIA, smaller-memory tokenizer/checkpoint lab. |
| B. Other assistant architectures | Nemotron-Mini-4B-Instruct after 17, with an inert conversion of its original `.bin`; Bolt-E4B as a distinct Gemma-4-derived profile. Neither is automatic dense-model compatibility. |
| C. Derivatives inside derivatives | Higher-order/input derivatives and a tiny PINN after 19–20; independent derivative and PDE-residual checks. |

## Prerequisites, not a sixty-step waiting list

| Path | Prerequisites | First complete payoff |
| --- | --- | --- |
| Urgent decisions | 1–3, then 4–9 | Elephant 10, Mammoth 11; Metal 13 advances alongside. |
| Retrieval / dense continuation | Shared storage, artifacts, tokenizer and attention; each exact model profile | 16 / 17. |
| Training / nanochat | 18–19, 21–25 and exact nanochat forward/backward | Tiny pretraining 28 → SFT 29 → the specific rollout recipe 32. |
| Early physics | 18–19 and an owned deterministic solver | Surrogate and rollout checks 20. |
| Vision | Shared forward core and 34; gradients for a trained head | C-RADIO 35; learned image codec 36 is a separate branch. |
| Verse motion | Shared forward core, required codec operators and 38–39 | MotionBricks 40 → Verse 41. |
| Speech | Shared convolution/attention and 42–43 | Parakeet 44. |
| Scientific ML | Shared training/spectral core for FNO; geometry operators for DoMINO | Darcy 46 or DoMINO 48; spherical weather 49 branches independently. |
| Hybrid / MoE language | Shared dense inference, state-space 50 and exact profiles; router 52 for Lightning | Nano 51 → Lightning 53. |
| World generation | Shared vision/codec, diffusion/flow 37 and temporal 54 | One Cosmos3-Edge path 55. |
| Robot policies | Shared vision/language, training, flow 37 and imitation/embodiment 56 | GR00T 57 → simulated closed loop 58, conditional on artifact terms. |
| Serving / scale | A qualified model plus consumer contract / a resumable trainer plus backend | 59 / 60 can start when their own prerequisites exist. |

Stable issue slugs describe existing work; chapter numbers are provisional.
When a branch is selected, split it into concrete issues with the same
code/checks/book/evidence acceptance loop. The remaining outlines stay proposals
until their workloads and acceptance gates are selected.
