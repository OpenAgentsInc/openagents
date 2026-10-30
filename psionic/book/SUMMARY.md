# Contents

- [Introduction](README.md)
- [1. Learning a number](ch001-learning-a-number.md)
- [2. Shapes and storage](ch002-shapes-and-storage.md)
- [2a. A tensor is a view of memory](ch002a-a-tensor-is-a-view-of-memory.md)
- [3. Numbers and matrices](ch003-numbers-and-matrices.md)
- [3a. A standard library of tensor operators](ch003a-a-standard-library-of-tensor-operators.md)
- [3b. One tensor, several devices and dtypes](ch003b-one-tensor-several-devices-and-dtypes.md)
- [4. Reading original weights](ch004-reading-original-weights.md)
- [4a. Models travel between frameworks](ch004a-models-travel-between-frameworks.md)
- [5. Text becomes a model request](ch005-text-becomes-a-model-request.md)
- [6. Attention without training](ch006-attention-without-training.md)
- [7. A hybrid sequence model](ch007-a-hybrid-sequence-model.md)
- [8. Decisions have a different mask](ch008-decisions-have-a-different-mask.md)
- [9. Read the options](ch009-read-the-options.md)
- [10. Execute Elephant](ch010-execute-elephant.md)
- [11. Execute Mammoth](ch011-execute-mammoth.md)
- [12. Are the probabilities any good?](ch012-are-the-probabilities-any-good.md)
- [13. Qualify honestly](ch013-qualify-honestly.md)
- [16. Serve `/v1/systemone`](ch016-serve-systemone.md)
- [16b. The protobuf wire](ch016b-the-protobuf-wire.md)
- [70. Learning with arrays](ch070-learning-with-arrays.md) — first learning chapter
- [71. Gradients as a program](ch071-gradients-as-a-program.md)
- [26. A Coax compute subset](ch026-a-coax-compute-subset.md) — first device chapter
- [26a. A tensor program becomes an execution plan](ch026a-a-tensor-program-becomes-an-execution-plan.md)
- [26b. Compile a general tensor program](ch026b-compile-a-general-tensor-program.md)
- [26c. Add your own operator](ch026c-add-your-own-operator.md)
- [30. Elephant on CUDA](ch030-elephant-on-cuda.md)
- [30a. Share the card](ch030a-share-the-card.md)
- [41. Original MXFP4 weights](ch041-original-mxfp4-weights.md) — first gpt-oss branch chapter
- [42. A few experts, then a router](ch042-a-few-experts-then-a-router.md)
- [43. Attention with sinks and a window](ch043-attention-with-sinks-and-a-window.md)
- [44. The harmony format](ch044-the-harmony-format.md)

- [45. Execute gpt-oss-20b](ch045-execute-gpt-oss-20b.md)
- [45a. Original weights stay on the GPU](ch045a-original-weights-stay-on-the-gpu.md)
- [45b. A complete CUDA decoder](ch045b-a-complete-cuda-decoder.md)
- [45c. Prefill together](ch045c-prefill-together.md)
- [45f. Replay the device program](ch045f-replay-the-device-program.md)
- [46. Execute gpt-oss-120b](ch046-execute-gpt-oss-120b.md)
- [47. Let the small model guess](ch047-let-the-small-model-guess.md)
- [48. Call a tool](ch048-call-a-tool.md)
- [48b. Serve Responses](ch048b-serve-responses.md)
- [48c. Two models, one card](ch048c-two-models-one-card.md)

- [56. Execute a DeepSeek R1 distill](ch056-execute-a-deepseek-r1-distill.md)
- [57. DeepSeek’s front door](ch057-deepseeks-front-door.md)
- [58. Execute DeepSeek V2-Lite](ch058-execute-deepseek-v2-lite.md)
- [59. Routers that balance themselves](ch059-routers-that-balance-themselves.md)
- [60. Compressed and shared attention](ch060-compressed-and-shared-attention.md)
- [61. Many residual streams](ch061-many-residual-streams.md)
- [62. Memory by lookup](ch062-memory-by-lookup.md)
- [63. V4.1's expert weights](ch063-v41s-expert-weights.md)
- [66. Pack the cache exactly](ch066-pack-the-cache-exactly.md)

- [115. Benchmark a service, not a kernel](ch115-benchmark-a-service-not-a-kernel.md) — first production-hosting chapter

- [156. Fit our own calibration, and refuse a stale one](ch156-fit-our-own-calibration.md)
- [157. Thresholds from written costs](ch157-thresholds-from-written-costs.md)
- [158. Record readings, join outcomes, watch drift](ch158-record-readings-join-outcomes-watch-drift.md)
- [164a. Learn which model to call](ch164a-learn-which-model-to-call.md)

- [165. A machine whose state is a trace](ch165-a-machine-whose-state-is-a-trace.md)
- [166. Attention as exact memory](ch166-attention-as-exact-memory.md)
- [167. Arithmetic and branches from ReLU](ch167-arithmetic-and-branches-from-relu.md)
- [168. Compile a gate graph into a block](ch168-compile-a-gate-graph-into-a-block.md)
- [169. A harness that catches the compiler](ch169-a-harness-that-catches-the-compiler.md)
- [172. From hard-max to softmax](ch172-from-hard-max-to-softmax.md)

- [178. A suite is three digests](ch178-a-suite-is-three-digests.md)
- [179. How far a metric moves by itself](ch179-how-far-a-metric-moves-by-itself.md)
- [180. Gates that can say unverifiable](ch180-gates-that-can-say-unverifiable.md)
- [182. Fit settings on recorded answers](ch182-fit-settings-on-recorded-answers.md)

Proposed: [full table of contents and prerequisites](PLAN.md), from early
Elephant/Mammoth through gpt-oss, Mistral and DeepSeek, nanochat
pretraining/post-training, NVIDIA retrieval, vision, motion, speech, scientific
ML, world generation, robot policies, production hosting and inference across a
mesh.
Chapters 1–13, 16, 26, 30 (Elephant on CUDA), 41–48, 56–61, 63, 66, 70–71, 115, 156–158, 164a, 165–169, 172, 178–180 and 182 are
runnable. Chapters 14, 15, 17–25, 27–29, 31–40 and the remaining outlines
remain proposed.
