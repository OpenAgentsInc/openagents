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

**The first delivery path is 1–3 → 4–10 (Elephant) → 11 (Mammoth) → 12
(published decision semantics), qualified under chapter 13's rules.**
Throughput and device work in 14–21 begins as soon as it has concrete
operations and CPU oracles. Neither generic training, gpt-oss, Mistral,
DeepSeek, the mesh, a smaller imported model nor the NVIDIA branches blocks
those two decision models. [Model targets](../../docs/psionic/model-targets.md)
defines their exact architecture and acceptance gates.

The contents give a reading order; they do not require finishing every earlier
part before starting a branch. The prerequisite map below governs that choice.
Several chapters need Coax compiler features that do not exist yet; the
[Coax feature map](#coax-features-each-branch-needs) names them so Psionic can
act as their first real consumer.
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
| 12. Are the probabilities any good? | Load exactly the calibration the pinned bundle supplies (or state that none exists and keep temperature explicit); reproduce the published serialized answers; Brier score, calibration error, abstention and per-kind accuracy on a frozen held-out request corpus, kept separate from numerical conformance. |

Text support is the first scope. Image inputs, quantized variants and serving
integration each receive separate qualification. A layer fixture or imported
tensor inventory does not complete either real-model chapter. Chapter 12
measures decision quality; a matching logit is not evidence that a decision
is useful, and a good held-out score is not evidence of numerical parity.

## III. Make execution measurable, parallel and portable

These chapters accompany real consumers in II; their placement is explanatory.

| Chapter | What we build and check |
| --- | --- |
| 13. Qualify honestly | Compare with a reference whose arithmetic cannot be reproduced exactly: only like modes (decode with decode, replay with replay); distribution checks (top-1 agreement, total variation, router-margin and near-tie regret) frozen before any confirmation set is read and never loosened after; values measured on the device that ran, not a surrogate. Every result binds the exact build, libraries and arithmetic actually executed; the command's exit status is the verdict; an absent qualification reads as unknown; failures are quoted with their units and counterexamples. |
| 14. Measure the work and plan the memory | Fixed CPU numerical references, peak memory, latency samples and a reproducible workload before optimizing. Measured compute and memory bandwidth for the host, and the arithmetic intensity of prefill and decode against that roofline, so every later optimization names the bottleneck it targets. A residency plan for each model: compute directly from mapped BF16 rows without f32 expansion, separate persistent/state/scratch bytes, and refuse requests whose plan does not fit. |
| 15. Use every core | Fixed work partitioning and tree-ordered merges so every output is byte-identical at 1, 2, 4 and 8 workers; a host thread pool calling Coax kernels on disjoint ranges until Coax jobs exist, then the same replay gate on jobs. |
| 16. Many questions at once | Batched suffix forks from one prefix snapshot, proven equal to the sequential forks of chapter 8; reuse of prefix state across requests keyed by model identity and exact token prefix, with bounded eviction. |
| 17. A Coax compute subset | The compiler's missing buffer, integer index, bounded loop, work-item and binding subset, proven on one small matrix-vector kernel: generated source compiles, real device execution matches the CPU oracle, negative admission refuses. |
| 18. Attention in tiles | Online-softmax attention over key/value tiles that never materializes the full score matrix, with chunked prefill. The changed reduction order gets its own named profile and stated tolerance against the CPU reference, and the block-bidirectional mask, sinks and windows must still hold. |
| 19. Elephant and Mammoth on Metal | Generated kernels, Metal bindings and completion/lifetime rules for the full decision path; qualify Elephant, then Mammoth, against the same CPU references. Apple GPUs have no double precision and flush subnormals, so the device profile states its own accumulation (compensated float-float sums, exhaustively checked transcendentals, host-built rotary tables) rather than inheriting chapter 3's binary64 profile. |
| 20. Quantized weights and caches | One specified derived block format (chapter 26's MXFP4 decoder is a candidate), then an optional quantized KV cache; independent decode vectors, partial-row refusal, separate model profiles, and measured memory against quality: perplexity on fixed text, chapter 12's decision metrics and a product evaluation, each compared with the original weights and required to differ by less than run-to-run noise. Aggressive 2–3-bit consumer profiles carry a quality card (top-1 agreement and KL against the original, task scores) and never inherit the original's evidence. |
| 21. A second device backend | Optional CUDA branch: Linux/ABI admission, Coax-generated arithmetic, device execution and comparisons with the same CPU oracles. |

NVIDIA model compatibility does not require CUDA. Metal and CUDA are separate
backend projects; neither upstream speed claims nor a reference runtime
establishes Psionic execution. Chapter 21 needs a qualified Linux target,
which Coax does not yet claim. Chapters 15 and 16 change scheduling, never
results: each must reproduce its single-worker or sequential output exactly
under the named CPU profile before it counts.

## IV. Useful inference beyond decisions

| Chapter | What we build and check |
| --- | --- |
| 22. Choosing the next token | A named counter-based RNG with explicit seed/counter progression; temperature, top-k and top-p with stated tie rules; stop conditions and streamed tokens with cancellation; constrained decoding that masks the vocabulary to a grammar or JSON schema using chapter 5's tokenizer tables. |
| 23. Search the Coax book | Nemotron-3-Embed-1B-BF16: exact query/document formatting, bidirectional attention, padding-aware average pooling, normalization and held-out retrieval. |
| 24. Continue a passage | Llama-3.1-Minitron-4B-Width-Base: exact head dimensions/position rules, full-versus-cached logits, and deterministic and seeded continuation through chapter 22. |
| 25. Run in the browser | Psionic CPU kernels compiled to Wasm through Coaxial, then WebGPU compute once Coax emits it; serve chapter 23's search on openagents.com with natively precomputed document embeddings and a separately selected query encoder small enough to download. |

These are forward-only applications of I–II, with their own architecture
profiles. Constrained decoding is a sampling contract, not a parser of model
output after the fact: a token outside the grammar is never offered. Chapter 25
is a separate target with its own ABI, memory-growth and numerical evidence;
a Mac result does not qualify it. The optional small import lab in appendix A
can help readers with limited memory; it is not an extra dependency of
Elephant or Mammoth.

## V. An open reasoning model: gpt-oss

OpenAI's [gpt-oss](../../docs/psionic/gpt-oss.md) is the first general
assistant model in the plan: Apache-2.0 weights, a mixture-of-experts
decoder and a structured response format built for tools and reasoning. It
needs no training and no hybrid recurrence, only the forward core of I–II,
the memory plan of 14 and the sampling of 22.

| Chapter | What we build and check |
| --- | --- |
| 26. Original MXFP4 weights | The checkpoint's own MoE format: 32 FP4 (E2M1) values packed into 16 bytes per block, with one biased power-of-two scale per block in a separate `.scales` tensor. Decode every code at every admitted scale exactly into `f32` against an independent table, refuse partial blocks and out-of-range scales, and keep this original format distinct from chapter 20's derived quantization profile. |
| 27. A few experts, then a router | A tiny MoE with directly checkable choices, ties, weighting, dispatch/gather and expert-residency budgets; then gpt-oss's exact rule, softmax over the sorted top four router logits. |
| 28. Attention with sinks and a window | Learned per-head sink logits that join each softmax denominator without contributing a value; a 128-token banded window on alternate layers beside full attention, with a window-bounded cache; YaRN rotary scaling (base 150,000, factor 32 over 4,096 original positions); attention biases; and the clamped SwiGLU (alpha 1.702, limit 7, plus-one linear term). |
| 29. The harmony format | The o200k_harmony tokenizer through chapter 5's byte-BPE engine, with its own split pattern, ranks and special tokens (201,088 vocabulary entries); render system, developer, user, assistant and tool messages with reasoning effort; parse `analysis`, `commentary` and `final` channels and stop on `<\|return\|>` or `<\|call\|>`; refuse malformed streams. |
| 30. Execute gpt-oss-20b | 24 layers and 32 experts: complete original-weight generation with chapter 22's sampling; independent token, intermediate, router-choice and logit references; full versus cached logits; measured residency for its 13.8 GB of original files. |
| 31. Execute gpt-oss-120b | The same engine with 36 layers and 128 experts; a residency plan for 65.2 GB of originals, mapping experts in and out where memory requires it, measured on the machine that runs it. |
| 32. Let the small model guess | Speculative decoding: gpt-oss-20b drafts for gpt-oss-120b (one tokenizer), plus n-gram drafts from the prompt. The standard acceptance rule keeps the target's distribution: greedy output must match the target alone token for token, and sampled acceptance probabilities are computed exactly on tiny-vocabulary fixtures. Measured acceptance rate, draft length and the batch size at which the gain disappears. |
| 33. Call a tool | Function calls on the `commentary` channel with arguments constrained to their JSON schema by chapter 22, and results returned as tool messages. The host executes tools under its own authority; `analysis` text never reaches an end user. |

The torch reference upcasts MXFP4 to BF16, so a converted BF16 checkpoint is
not the original artifact; chapter 26 decodes the original blocks. The
reference torch, Triton and Metal implementations are read as untrusted
source, not executed, and their browser and Python tools are explicitly
educational upstream, so Psionic does not adopt them. The 20b model's
16 GB figure is an upstream claim until chapter 14's plan measures it.
Text input is the whole scope. This part does not block Elephant or Mammoth.

## VI. Mistral models, from Ministral to Le Chonk

[Mistral](../../docs/psionic/mistral.md) publishes a ladder of open models
whose architectures add one feature at a time on the way to Mistral Large 4
("Le Chonk"): 1T total and about 52B active parameters, 128 routed experts plus
a shared expert, latent and sparse attention, FP8 weights and a built-in draft
head. This part qualifies each feature on the smallest model that has it. Large 4's
1,060.6 GB of original files fit no single machine, so it is executed in
Part XVIII, across a mesh.

| Chapter | What we build and check |
| --- | --- |
| 34. Execute Ministral 3 | Ministral-3-3B (Apache-2.0, BF16 originals): the Tekken tokenizer through chapter 5's byte-BPE engine with its own pinned tables, Mistral's chat and reasoning format, and a dense grouped-query decoder with its long-context rotary scaling; token, intermediate and logit references. |
| 35. Original FP8 weights | E4M3 values with one scale per 128×128 block, including Large 4's power-of-two (UE8M0) scales: exact decode of every code at every admitted scale against an independent table, partial-block and scale refusals, and FP8 originals kept distinct from chapter 20's derived profiles. The reference's dynamic FP8 activations are a separate approximate profile. |
| 36. A coding agent | Devstral-Small-2-24B (Apache-2.0, FP8): Mistral's function-calling format with arguments constrained by chapter 22, and long-context requests through chapters 18 and 104; first measured against a fixed coding-task set. |
| 37. Attention through a latent | Multi-head latent attention: queries and keys split into rotary and non-rotary parts, keys and values reconstructed from a low-rank latent, and the absorbed projections that let decode attend in latent space. The compressed cache's size is accounted exactly, and the absorbed form is compared with the expanded one. |
| 38. Execute Mistral Small 4 | Mistral-Small-4-119B (Apache-2.0, FP8, 241.9 GB of originals): latent attention plus 128 routed experts with four selected and one shared expert, extending chapter 27's router. Measured residency on one large-memory machine, or the expert paging of chapter 31. |
| 39. Sparse attention and shared layers | Large 4's attention features on tiny fixtures and selected real layers: a lightning indexer that picks the top 2,048 keys per query (FP8 indexer), indexer and KV sharing across layers from the pinned source map, no rotary positions on every fourth layer, tanh soft-capping of logits at 30, and YaRN scaling (factor 2048 over 512 positions) to 1,048,576. |
| 40. The model's own draft head | Large 4's embedded multi-token-prediction draft head as a speculator under chapter 32's acceptance rule: identical greedy output, exact acceptance probabilities on tiny fixtures, and measured acceptance and draft length. |

Ministral 3, Devstral Small 2, Mistral Small 4 and Mistral Large 3 are Apache-2.0.
Large 4 uses the Mistral Ecosystem Open-Weight License. It permits internal
use and embedded product features, but offering the model as a service needs a
separate commercial licence from Mistral, and that is an owner decision. Text is the first
scope; the Pixtral-style vision encoders are separate image profiles in
Part XI. The NVFP4 variants are derived checkpoints for Blackwell GPUs, not originals.

## VII. DeepSeek, natively

DeepSeek publishes both open models and unusually detailed accounts of how it
serves them. This part builds our own version of the
[pre-reset DeepSeek V4.1 Flash work](../../docs/psionic/deepseek.md) in Coax:
the model ladder from a small distill to V4.1 Flash, V4.1's distinctive
components, and the engine techniques behind its efficiency. Each technique
is exact unless its profile says otherwise. The old Rust attempt ran V4.1 end
to end but never passed numerical parity; chapter 13's rules come from that
failure.

| Chapter | What we build and check |
| --- | --- |
| 41. Execute a DeepSeek R1 distill | R1-Distill-Qwen-1.5B through the shared dense path: the distill's own tokenizer, BOS from the tokenizer rather than the config, a template that strips earlier reasoning, and separate reasoning and final output. |
| 42. DeepSeek's front door | The recipe layer in Coax. Special tokens are inserted by boundary, never by spelling, so user or tool text that spells a marker stays text. Segments between specials encode independently. A streaming parser carries incomplete UTF-8 and multi-token delimiters across chunks in linear time and emits tool names and escaped arguments as they arrive. A stream that ends without a finish event is reported incomplete. |
| 43. Execute DeepSeek V2-Lite | 16B total, 2.4B active: DeepSeek's latent attention (sharing chapter 37's math) with 64 routed and 2 shared experts, compared in compressed and expanded form with router and combine parity and cache continuation. Fits one Mac. |
| 44. Routers that balance themselves | V3-style selection with a bias used only for choosing experts and unbiased scores for weighting, V4.1's square-root-softplus scores and route scale, explicit lowest-id tie rules, and router-margin records so near ties are visible in every comparison. |
| 45. Compressed and shared attention | V4.1's attention: a 128-token window ring, a gated pooling compressor (ratio 2) and an ungated one (ratio 1), a single shared KV head with an attention sink, KV and indexer state reused across layers from pinned source layers, and candidate blocks chosen at one layer and read by the layers after it. Chunked and whole prompts give identical state. |
| 46. Many residual streams | Hyper-connections: four parallel residual streams mixed by pre, post and Sinkhorn-normalized 4×4 coefficients (20 iterations), with the collapse that reads the previous sub-block's mix, which must cross any stage boundary. |
| 47. Memory by lookup | Engram: hashed n-gram tables (up to 4-grams over a normalized, compressed vocabulary) with prime bucket layouts and the reference's fixed per-layer multipliers reproduced exactly; tables of about 101 GB each read a few rows per token from mapped files or disk, never loaded whole. |
| 48. V4.1's expert weights | 384 routed experts and one shared expert in FP4 E2M1 with one UE8M0 scale per 32 elements along K. This is not MXFP4: the scales are separate tensors, the nibble order differs and a 0xFF scale is NaN. Clamped SwiGLU, and route weights applied before the down projection. Exact decode vectors for every code and scale. |
| 49. Hot experts in fast memory | An exact expert cache: residency seeded from a recorded routing profile, decayed-frequency heat with a swap threshold, copies between steps, misses served by a host lane or from mapped files, and next-layer gate prefetch. The cache changes only where an expert runs, never its result. |
| 50. Prefill layer by layer | Layer-major chunked prefill that reads each expert once per chunk instead of once per token, then grouped block-scaled matrix products; chunk size from a per-step time budget, and a check that no configuration silently disables chunking. |
| 51. Pack the cache exactly | Window KV in FP8 and compressed and index keys in FP4 where the reference already rounds to those grids, bit-checked against it; the memory of step buffers, which scale with chunk size, budgeted separately from the cache. |
| 52. A decode step with no host waits | The whole decode step as one recorded device command sequence: expert ids that stay on the device, sampling on the device so a stage returns a token rather than logits, and a host that schedules step N+1 while step N runs, with buffer lifetimes that allow speculative lengths to be learned a step late. |
| 53. Execute DeepSeek V4.1 Flash on one machine | The complete model, 510.3 GB of originals: one large GPU or a large-memory Mac with experts from the cache of chapter 49 and Engram from disk, qualified under chapter 13 against a reference capture and measured for time to first token, tokens per second and memory. |
| 54. Verify drafts by expected gain | The checkpoint's own draft layers, fixed-length verification first, then variable-length verification chosen from a trained confidence head and a measured cost curve, reported as committed tokens per unit time rather than projected acceptance. |

The reference implementation's exact arithmetic depended on its hardware and
parallel layout, so qualification compares distributions, not bits; chapter
13 sets the rules. The serving facts DeepSeek has published (prefill and
decode on different groups, expert parallelism only inside a fast domain, a
shared prefix cache of 128-token pages) inform chapters 104, 105, 110 and 111.
DeepSeek's own engine, kernels and libraries are references read as source,
not dependencies. V3/R1-scale checkpoints and V4.1 split into stages run in
Part XVIII.

## VIII. Learn with the operations we already own

| Chapter | What we build and check |
| --- | --- |
| 55. Learning with arrays | A small classifier using the shared operations, initially with explicit gradients and SGD. |
| 56. Gradients as a program | A bounded reverse-mode tape, saved values, accumulation and parameter identity; analytic and finite-difference checks. |
| 57. Learn a physical system | A supervised oscillator or cooling surrogate from our own deterministic solver; held-out parameters, one-step error and repeated rollout stability. |
| 58. Bytes and a small language model | A real byte vocabulary, bigram likelihood, fixed licensed splits, named RNG and complete tiny training/generation. |
| 59. Differentiate a decoder | Backward operations for the complete causal attention/FFN path; causal isolation and gradient coverage. |
| 60. Train the whole tiny decoder | A complete small transformer with training, held-out likelihood and generation; keep it as a permanent regression. |

Chapter 57 needs 55–56, not language-model training. It is our first physics
chapter, much earlier than pretrained geometry or weather models. It uses
ordinary supervised learning; higher-order physics-informed derivatives are
a separate extension in appendix C.

## IX. Data, tokenizers, state and optimization

| Chapter | What we build and check |
| --- | --- |
| 61. Data and resumable state | Provenance, disjoint splits, document packing, model/optimizer/RNG/data counters and exact resume under a named profile. |
| 62. Learn a tokenizer | Deterministic byte-BPE training in Coax with explicit tie rules and a pinned pretokenization pattern (nanochat's two-digit number groups); the trained table round-trips through chapter 5's encoder, and an independent oracle reproduces the merges. |
| 63. AdamW, then Muon | Independent update derivations, parameter groups, precision and the exact pinned nanochat optimizer behavior. |

Data admission begins with the first trainer. These chapters consolidate it
before longer runs rather than postponing provenance until this part.

## X. Rebuild nanochat, then extend the training pipeline

| Chapter | What we build and check |
| --- | --- |
| 64. Execute nanochat exactly | Pinned BPE/conversation rules, inert checkpoint export, every enabled forward feature and cached inference. |
| 65. Differentiate nanochat | Gradients for every enabled feature; tiny independent comparisons and complete parameter coverage. |
| 66. A complete pretraining run | Tokenizer training from 62, initialization, schedules, accumulation, token budgets, packing, resume and held-out loss/bits-per-byte; tiny first, measured scale later. |
| 67. Supervised conversations and tools | Assistant/tool loss masks and validation; bounded Nemotron-Agentic-v1/OpenCodeReasoning subsets with provenance and base-skill regression. |
| 68. Continued pretraining and task adaptation | A separately specified domain workload, data mixture, retention checks and promotion criteria. This is a Psionic extension, not a stage claimed from the pinned nanochat speedrun. |
| 69. Preferences and reward models | HelpSteer3 pairwise data, ranking/reward losses and an optional separately specified DPO experiment; held-out comparisons and reward-model limitations. |
| 70. Learn from rollouts | The pinned nanochat on-policy, mean-centered-reward recipe; bounded rollouts, independent gradient/reward checks and held-out task results. |
| 71. Post-train a decision model | Qualified hybrid backward/adapters, selected trainable regions, cross-entropy/Brier/abstention, resume and held-out promotion. |
| 72. Distill Mammoth into Elephant | Mammoth's slot distributions as soft targets on a recorded request corpus; KL/cross-entropy over slots, held-out agreement and chapter 12's quality metrics, with Elephant's cost measured against Mammoth's. |

The [nanochat study](../../docs/psionic/nanochat.md) is the compatibility
contract. Its current speedrun trains a tokenizer and a base model and runs
SFT; RL is a separate program. Chapter 70 depends on SFT and its own reward
contracts, not on completing preference learning or continued pretraining.
Chapters 71–72 need the decision forward path, chapter 12's evaluation and the
shared training machinery, not nanochat RL. A distilled student is a new
model profile with its own evidence; it inherits neither model's results.
No generic PPO, KL penalty or broad RLHF parity is implied.

## XI. Images become features and latent representations

| Chapter | What we build and check |
| --- | --- |
| 73. Pixels, convolutions and patches | Exact resize/channel/range conventions, convolution, sampling and patch projection with tiny independent fixtures. |
| 74. Elephant and Mammoth see | The pinned Qwen3.5 vision encoder and projector, exact image preprocessing, image token layout and multimodal rotary positions; image-bearing decision requests with their own reference captures. |
| 75. See with C-RADIO | C-RADIOv3-B feature outputs, visual-asset retrieval and a small Psionic-trained head over frozen features. |
| 76. Compress and reconstruct an image | The older Cosmos-0.1-Tokenizer-CI8x8, including wavelets, normalization and decoder; inspect latent/reconstruction numerics. |
| 77. Noise, diffusion and flow | Train a tiny generative example; distinguish objectives, schedules, conditioning and reproducible sampling. |

Chapter 74 extends the priority decision models and needs only 73 and the
Elephant/Mammoth forward path; text-only qualification does not carry over.
Chapter 75 needs the shared forward core and 73; its trained head also needs
55–56. Chapter 76 is an older teaching target, not a promise of Cosmos3 codec
compatibility. Chapter 77 introduces the methods later world/robotics models
need; it does not qualify their architectures.

## XII. Motion for Verse

| Chapter | What we build and check |
| --- | --- |
| 78. Poses have conventions | Rotation representations, skeleton hierarchy, root coordinates, units, timing and retargeting fixtures. |
| 79. Motion tokens become poses | A tiny codebook/VQ decoder and masked-token generation; known token-to-pose and pose-continuity checks. |
| 80. Execute MotionBricks | One pinned SOMA package: root timing/trajectory, masked motion-token transformer and VQ-VAE decoder, with full trajectory references. |
| 81. Put generated motion in Verse | An explicit skeleton/target interface, bounded generation and animation handoff; a reproducible destination-to-pose scene. |

This is the preferred first major Verse ML demonstration after useful vision
features. It reuses attention, common image/codebook operators where required,
and motion math; it does not depend on speech, DoMINO, weather or nanochat RL.
Unitree G1 is a separate embodiment/profile, not an interchangeable asset.

## XIII. Audio becomes words, and words become audio

| Chapter | What we build and check |
| --- | --- |
| 82. Waveforms and spectral features | Sample/rate/channel admission, windowing, tiny DFT → FFT/inverse, spectrogram/mel and exact feature normalization. |
| 83. A speech encoder | FastConformer convolution/attention, subsampling and length/mask propagation, compared against fixed intermediate tensors. |
| 84. Transcribe with Parakeet | parakeet-tdt-0.6b-v3, token-duration-transducer decoding and bounded utterances; WER/CER and language slices. |
| 85. Transcribe while someone speaks | Chunked streaming input (Voxtral-Mini-4B-Realtime, Apache-2.0, is a candidate alongside Parakeet) with carried encoder/decoder state, partial and final hypotheses, measured per-chunk latency against a stated budget, and integration with the voice engine's end-of-turn and barge-in through `verse/platform/voice.h`. |
| 86. Speak the answer | One separately selected and pinned text-to-speech checkpoint (Voxtral-4B-TTS is a candidate, but its CC BY-NC 4.0 weights exclude commercial use): text normalization, acoustic model, vocoder, streamed audio frames into the voice engine's playback path; intelligibility (ASR round trip through 84) and latency evidence. |

This branch starts from shared convolution/attention and spectral operations;
it need not wait for motion. Spectral primitives are shared with physics.
Offline transcription in 84 does not establish streaming latency; 85 does,
for its named host and chunk size. The text-to-speech target still needs a
survey entry with its weights, code and data terms before 86 is selected.
Neither chapter changes the voice engine's consent or floor rules.

## XIV. Fields, geometry and physical prediction

| Chapter | What we build and check |
| --- | --- |
| 87. A spectral neural operator | Tiny DFT/FFT references, complex arithmetic, truncation and spectral layers on a small field; reuse the spectral core from 82. |
| 88. Learn Darcy flow | Reimplement the PhysicsNeMo FNO architecture/recipe on bounded reproducibly generated data; compare against a solver on held-out fields. |
| 89. A field around a shape | Signed distances, point clouds, spatial neighborhood queries, point convolutions and local aggregation with tiny geometric oracles. |
| 90. Execute DoMINO | domino_drivaerml, a bounded DrivAerML subset, exact preprocessing, surface/volume outputs and physical error measures. |
| 91. Forecast on a sphere | Optional FourCastNet3 branch: spherical operators, variable/grid conventions, geographic weighting and multi-step forecast evaluation. |

The first physics result is already in 57. FNO adds field learning; DoMINO
adds geometry-specific operations. Chapter 91 is an alternative specialty,
not a successor that must wait for DoMINO. Its field/activation budgets and
ERA5 acquisition/terms need their own qualification. The Darcy recipe is not
a pretrained checkpoint. None of these models replaces Verse's authoritative
simulation without a separate consumer contract.

## XV. More sequence architectures

| Chapter | What we build and check |
| --- | --- |
| 92. A recurrence you can inspect | Tiny state-space transitions, causal convolution, chunked/whole-sequence versus token-step execution and independent state reset. |
| 93. Execute Nemotron Nano | NVIDIA-Nemotron-3-Nano-4B-BF16: the complete Mamba-2/Transformer hybrid and all recurrent/cache state. |
| 94. Execute Nemotron Lightning | NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16: combine the exact hybrid and MoE profiles; floating point before NVFP4 or speculation. |

This branch can follow dense inference and performance work without waiting
for motion or physics. Qwen Gated DeltaNet support does not establish Mamba-2
support. The router is chapter 27 in Part V. Active parameter count is not total checkpoint residency. Bolt's
different Gemma-4 architecture is an optional study in appendix B.

## XVI. World models and embodied policies

| Chapter | What we build and check |
| --- | --- |
| 95. Sequences of observations | Temporal/multimodal representations, a tiny conditioned generation task and codec/scheduler contracts. |
| 96. One complete Cosmos3-Edge path | Qualify one bounded conditioned generation path across its autoregressive/diffusion towers and exact codecs; numerical and physical evaluations stay distinct. |
| 97. Imitation before robotics scale | A tiny policy trained/evaluated in simulation, with action/state normalization and a bounded PhysicalAI SingleArm data exercise. |
| 98. Execute a GR00T policy | GR00T-N1.7-3B as a candidate, subject to its unresolved artifact-license inconsistency; exact VLA/flow-matching action path for one embodiment. |
| 99. Close the simulated control loop | Action chunks, observation timing, resets, held-out task success and failure recovery under the selected embodiment. |

Cosmos needs mature vision, temporal representations and generative methods.
The robotics path needs vision/language, shared training, action conventions
and flow matching; it does not require Cosmos inference first. GR00T is not a
commercial default while its card and included license disagree. Real robot
control would be a separately authorized integration after simulation.

## XVII. Host inference in production

This part turns a qualified model into a service that other people and
agents depend on. It follows the production practice described in
[Inference Engineering](../../docs/psionic/inference-engineering.md), but
builds the infrastructure in Coax: admission, scheduling, routing, scaling,
rollout and metering decide in Coax, with `#authority` files and
zero-survivor mutation where they decide work, money or policy. The host
does network, process and device effects only, as the
[Coaxial dev server](../../coax/web.md) already does for the website.

| Chapter | What we build and check |
| --- | --- |
| 100. Benchmark a service, not a kernel | Time to first token, inter-token latency, per-user and total tokens per second, and end-to-end versus inference-only time at P50, P90, P95 and P99. A load generator with a named RNG reproduces input/output length distributions, request contents, sampling parameters and jittered arrival patterns; recorded traffic can be replayed as a shadow. A fixed baseline is required, and only one variable changes per run. |
| 101. Serve a model with an honest capability | Shared System One protocol integration, versioned manifests, bounded requests, cancellation and evidence for each model/backend/profile; request and response schemas start in `proto/`. |
| 102. Clients, streams and background jobs | Streamed tokens and audio over a bidirectional connection, session reuse, cancellation that reaches the scheduler, and asynchronous jobs that are acknowledged at once and complete later under their own deadline; an OpenAI-compatible Responses edge is translated into the System One contract, not the other way round. |
| 103. Continuous batching and a fair scheduler | Token-level batching that admits and retires requests every step under a state-memory budget, chunked prefill interleaved with decode, preemption and priority classes. Every request's output must be identical to running it alone (batch invariance), which the fixed reduction order makes testable. |
| 104. Paged state and a tiered prefix cache | Fixed-size pages for KV cache and for hybrid recurrent snapshots; a prefix cache across requests that spills from device to host memory to local disk, with measured hit rate, eviction and time to first token on hits versus misses. Prefixes are found through a content-hashed chain of 128-token pages; pages never depend on a compute machine staying alive, restore overlaps layer by layer with compute, and retention is set per traffic type. Any approximate reconstruction, such as bounded sliding-window replay, is its own profile and never enters an exact cache identity. |
| 105. Route, queue and scale replicas | A cache-aware and adapter-aware router, a bounded priority queue, and a replica autoscaler (minimum and maximum replicas, window, scale-down delay, concurrency target, scale to zero). The policies are pure Coax, checked against a deterministic discrete-event simulation of recorded and synthetic traffic before any machine runs them. Prefill cost is modelled from uncached tokens and the prefix they attend to, with length buckets and engines that pull work; decode balances both request count and total cache length per replica. |
| 106. Start fast | Cold start split into machine, process, weights and kernel startup and measured separately; content-addressed weight delivery from chapter 4's identities, mapped loading, and generated-kernel caches keyed by device, compiler and source identity. |
| 107. Ship without downtime | Canary rollout with gradual traffic ramps and automatic revert, driven by both latency objectives and golden requests whose outputs must match retained evidence; draining in-flight requests before a replica stops. |
| 108. Observe, meter and price | Volume, input/output lengths, status counts, latency percentiles, replicas, utilization and queue depth; audit logs that record changes and no user content by default; per-request usage records bound to execution evidence and a pricing policy in an `#authority` file. Settlement stays in the Bitcoin wallet, outside Psionic. |
| 109. Survive failures | Health checks, cordoning a failing device or machine, idempotent retries, active-active and active-passive failover between machines or regions, data-residency routing, isolation of tool execution and protection of weights. |
| 110. Disaggregate prefill and decode | Conditional disaggregation: separate prefill and decode workers exchange a complete state snapshot (chapter 8's identity-bound claim, extended for transfer) as a versioned message; a prefill queue, a local-prefill threshold and an xPyD ratio that adjusts to traffic. Only after chapters 100 and 103 measure that prefill interference is the bottleneck. DeepSeek's published layout (small prefill groups, large decode groups, a separate vision encoder role, scaling prefill on unserved tokens and decode on requests in flight) is the reference design. |
| 111. Split a model across devices | Tensor parallelism for latency and expert parallelism for gpt-oss, DeepSeek and other MoE throughput: collectives, sharded weights and state, and results compared with the single-device reference. Expert parallelism stays inside one fast interconnect domain, with data-parallel attention, redundant experts rebalanced from a load window and balance reported as busiest over mean, measured by time rather than token count. |
| 112. A pipeline of models | A compound voice workload (voice activity, transcription, decision model, speech) with each stage scaled separately in one cluster, an end-to-end latency budget per stage and a measured cost for each network hop. |
| 113. A decision model steers voice | Elephant as the voice engine's `decisions:<model>` decider: proposals pass the existing never-awaited gate (clamping, confidence floor, epoch and deadline fences, recorded drop reasons); a slow or absent model can only fail to steer, never stall a frame. |
| 114. Train across devices | One real homogeneous topology: gradient collectives (shared with 111), sharding/resume, numerical comparisons and measured scaling. |

Serving can begin after the first qualified decision model and an actual
consumer contract. Chapter 113 is the first consumer: it needs 12's published
semantics and 100's measured latency, and the gate stays the authority over
what a decision may change. The more traffic a deployment has, the more of
this part it can justify: disaggregation, multi-device inference and global
caches each wait for a measurement that shows the need, not for a chapter
number. Hosted machines need a qualified Linux target and an owner-approved
budget; buying GPUs, choosing providers and compliance certification are
owner decisions recorded in `NEEDS_OWNER.md`, not chapters. Distributed
training can begin after a correct resumable single-device trainer and the
selected backend.

## XVIII. Inference across a mesh

A mesh pools memory and compute across machines that are loosely connected,
over ordinary networks rather than device interconnects. Chapter 111 is the
tightly coupled case in one machine or rack. The design follows the
[Mesh-LLM lessons](../../docs/psionic/mesh.md) recorded before the reset, with
the trust rules from the earlier private-inference work. Coax decides
placement, admission, routing, verification and price; the host provides
transport, files and devices.

| Chapter | What we build and check |
| --- | --- |
| 115. Split a model into stages | A stage boundary contract derived from the model: a typed list of everything that crosses it (hidden state, residual streams, shared-KV and recurrent sidebands), bound to the artifact, stage range and protocol version. Two stages must match the whole model on identical tokens: logits, greedy output, cached continuation. Dense Ministral 3 first, with raw activations. |
| 116. Stage packages | Each stage fetches and loads only its own tensors and required shared assets, verified chunk by chunk against chapter 4's identities. Fetched bytes, materialized bytes, peak memory and time to readiness are measured separately. |
| 117. Peers, transport and discovery | Encrypted peer connections with the direct or relayed path recorded and measured; private meshes joined by invitation, public meshes discovered through a Nostr event kind specified first as an OpenAgents NIP; an owner control plane separate from the inference plane. |
| 118. Place the model | Run whole on one machine if it fits, else route the whole request to a peer, else split. Splitting uses per-layer memory and measured stage and link costs, and states its objective: the sum of stage times for one request's latency, or the slowest stage for throughput. The placement planner is pure Coax, checked in simulation. |
| 119. Stage lifecycle and fencing | Load downstream stages first and publish the entry route only when every stage is ready. A topology generation fences out stale commands, and partial failures withdraw the route and release memory. Each live sequence declares whether it drains, fails or restores. |
| 120. Experts across machines and from disk | Expert parallelism over a mesh with routed tokens and gathered outputs, and expert streaming from local disk for MoE models larger than memory; measured against chapter 38's single-machine results. |
| 121. Route by saved work | Mesh-wide cache-aware routing that compares queue delay plus restore and suffix-prefill cost against cold prefill. Cache advertisements are bounded, short-lived, salted hints that never contain raw tokens; claimed hits that turn out to be missing fall back, and the realized saving is recorded. |
| 122. Compress activations exactly | Codec negotiation with a mandatory raw fallback and round-trip equality of each frame, with the full encode, decode and copy cost measured. Lossy activation formats are a separate profile with their own quality evaluation. |
| 123. Who can read the prompt | Trust levels computed from evidence: a peer serving the request reads it; a hardened process on a machine its owner controls is harder to snoop on, not private; only an attested hardware enclave running a logged release is private. Requests are routed by sensitivity and fail closed rather than falling back. |
| 124. Check a peer's work | Sampled re-execution of a peer's work on a trusted node. Results match exactly only when both sides run the same backend profile, and are otherwise compared within that profile's tolerance. Signed usage receipts per stage, a pricing policy in an `#authority` file, and settlement in Bitcoin through the wallet, outside Psionic. |
| 125. Work with Mesh-LLM | A Psionic server attached to an existing Mesh-LLM mesh through its plugin endpoint interface, serving the OpenAI-compatible and `/systemone` decision contracts so the mesh can route to it. Payments interoperate with its token-metered Lightning exchange (deposit first, then settle from reported usage) through our own wallet. Its code is read as untrusted source; joining a public mesh is an owner decision. |
| 126. DeepSeek V4.1 Flash across stages | Contiguous layer stages on several devices or machines. The boundary carries the four residual streams, the previous mix, source KV and indexer state, chosen candidate blocks and Engram history, in BF16 rather than F32. Generation fencing and stage recovery apply, and every split must match chapter 53's one-machine result on identical tokens, with per-hop latency recorded. |
| 127. Mistral Large 3 across a mesh | Mistral-Large-3-675B (Apache-2.0, 681.5 GB of originals): latent attention and 128-expert routing placed across machines, with parity against the whole-model reference on selected layers and tokens, per-hop latency and per-stage memory. |
| 128. Le Chonk across a mesh | Mistral Large 4 with all of Part VI: 1,060.6 GB of FP8 originals placed across machines, sparse attention and shared layers crossing stage boundaries, the built-in draft head, and recorded parity, latency and cost. Serving it to others is subject to its licence. |

A mesh multiplies failure modes: placement plans, advertisements and
handshakes are not evidence of correct execution. Each split earns its own
parity evidence on the machines that ran it. Joining a public mesh, paying
peers or offering Large 4 as a service each need owner approval.

## Optional laboratories

| Appendix | Scope and entry point |
| --- | --- |
| A. A small imported decoder | SmolLM2-135M after 4–6; a deliberately non-NVIDIA, smaller-memory tokenizer/checkpoint lab. |
| B. Other assistant architectures | Nemotron-Mini-4B-Instruct after 24, with an inert conversion of its original `.bin`; Bolt-E4B as a distinct Gemma-4-derived profile. Neither is automatic dense-model compatibility. |
| C. Derivatives inside derivatives | Higher-order/input derivatives and a tiny PINN after 56–57; independent derivative and PDE-residual checks. |
| D. Look inside a model | Bounded activation capture at named layers, logit lens over the residual stream, attention and recurrent-state inspection; doubles as the tool for localizing a reference mismatch in 10–11. |

## Prerequisites, not a waiting list of 128 steps

| Path | Prerequisites | First complete payoff |
| --- | --- | --- |
| Urgent decisions | 1–3, then 4–9 | Elephant 10, Mammoth 11, published semantics 12; Metal 17–19 advances alongside. |
| Decision throughput | 10 and 14; Coax jobs for the second half of 15 | Byte-identical parallel CPU 15; batched forks and prefix reuse 16. |
| Generation | Shared dense inference and 5's tokenizer tables | Sampling and constrained decoding 22 → continuation 24. |
| Mistral | 4–6, 14, 22 and the router in 27; 32 for drafting | Ministral 3 34 → FP8 35 → Devstral 36 → latent attention 37 → Small 4 38 → Large 4 components 39–40. |
| DeepSeek | 4–6, 13, 14, 22 and the router in 27; latent attention 37 | R1 distill 41 → recipe 42 → V2-Lite 43 → V4.1 components 44–48 → engine techniques 49–52 → V4.1 Flash on one machine 53 → drafts 54; across stages 126. |
| gpt-oss | 4–6, 14 and 22; the router in 27 | MXFP4 26 → 20b 30 → 120b 31 → speculative decoding 32; tool calls 33. |
| Retrieval / dense continuation | Shared storage, artifacts, tokenizer and attention; each exact model profile | 23 / 24; browser search 25 once Coax Wasm/WebGPU compute exists. |
| Training / nanochat | 55–56, 58–63 and exact nanochat forward/backward | Tiny pretraining 66 → SFT 67 → the specific rollout recipe 70. |
| Decision post-training | 10–12, 56 and hybrid backward | Adaptation 71; Mammoth-to-Elephant distillation 72. |
| Early physics | 55–56 and an owned deterministic solver | Surrogate and rollout checks 57. |
| Vision | Shared forward core and 73; gradients for a trained head | Elephant/Mammoth images 74; C-RADIO 75; learned image codec 76 is a separate branch. |
| Verse motion | Shared forward core, required codec operators and 78–79 | MotionBricks 80 → Verse 81. |
| Speech | Shared convolution/attention and 82–83 | Parakeet 84 → streaming 85; text-to-speech 86 after its target is selected. |
| Scientific ML | Shared training/spectral core for FNO; geometry operators for DoMINO | Darcy 88 or DoMINO 90; spherical weather 91 branches independently. |
| Hybrid / MoE language | Shared dense inference, state-space 92 and exact profiles; router 27 (Part V) for Lightning | Nano 93 → Lightning 94. |
| World generation | Shared vision/codec, diffusion/flow 77 and temporal 95 | One Cosmos3-Edge path 96. |
| Robot policies | Shared vision/language, training, flow 77 and imitation/embodiment 97 | GR00T 98 → simulated closed loop 99, conditional on artifact terms. |
| Production hosting | A qualified model, 14's measurements and a consumer contract | Benchmark 100 → server and clients 101–102 → batching and cache 103–104 → routing, scaling, rollout, metering 105–109. |
| Large-scale serving | Measured need from 100 and 103; two or more devices | Disaggregation 110; tensor/expert parallelism 111. |
| Voice / training scale | 12, 100 and the voice gate / a resumable trainer plus backend | Voice pipeline 112 and decision steering 113 / distributed training 114. |
| Mesh inference | 115's two-stage parity, 14's measurements and network transport | Packages and placement 116–119 → experts and routing 120–122 → trust and verification 123–124 → Mesh-LLM interoperation 125 → DeepSeek V4.1 across stages 126 → Mistral Large 3 127 → Le Chonk 128. |

## Coax features each branch needs

Psionic is a consumer of the [Coax roadmap](../../coax/ROADMAP.md), not a
place to work around it. The current compiler (see
[Coax book chapter 21](../../coax/book/ch21-road-ahead.md)) lacks these; a
chapter that needs one names the smallest language change, its real consumer
case and its verification gate, and stays planned until the feature runs.

| Missing in Coax today | Where Psionic needs it | Until then |
| --- | --- | --- |
| Buffer/index/loop/dispatch GPU compute; `coax metal` emits scalar `f32`/`bool` helpers only ([ch14](../../coax/book/ch14-gpu.md)) | 17–19, 21, 25 | CPU references; no hand-written tensor shaders. |
| WGSL emission | 25 | Wasm CPU kernels only. |
| Jobs, threads and a memory model ([ch12](../../coax/book/ch12-jobs.md), stage 5 not started) | 15, 16, 52, 101–105, 115–124 | A host thread pool calling Coax on disjoint ranges, with the replay gate. |
| `f16`/`bf16`, FP8 and FP4 scalar types | 14, 20, 26, 35, 48, every BF16 kernel | Decode from `u8`/`u16` bits, as chapter 4 does. |
| Generics over numeric types ([ch08](../../coax/book/ch08-generics.md)) | Every multi-dtype kernel, 20 | One concrete procedure per dtype. |
| SIMD lanes (`for_simd`) and vector/matrix types ([ch11](../../coax/book/ch11-data-layout.md)) | 14–15, 78–81 | Scalar loops; measure before asking. |
| Pointers, dynamic arrays and allocators ([ch06](../../coax/book/ch06-memory.md)) | 14, 16 caches, 31, 49 and 120 expert paging, 61 | Host-owned views with explicit capacities. |
| A qualified Linux target ([ch18](../../coax/book/ch18-targets.md)) and asynchronous I/O with deadlines and cancellation ([ch23](../../coax/book/ch23-asynchronous-work.md)) | 21, 101–114 on hosted machines, 115–128 on mixed peers | Apple CPU and Metal only. |

Stable issue slugs describe existing work; chapter numbers are provisional.
When a branch is selected, split it into concrete issues with the same
code/checks/book/evidence acceptance loop. The remaining outlines stay proposals
until their workloads and acceptance gates are selected.
