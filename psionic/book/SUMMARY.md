# Contents

- [Introduction](README.md)
- [1. Learning a number](ch001-learning-a-number.md)
- [2. Shapes and storage](ch002-shapes-and-storage.md)
- [3. A tensor is a view of memory](ch003-a-tensor-is-a-view-of-memory.md)
- [4. Numbers and matrices](ch004-numbers-and-matrices.md)
- [5. A standard library of tensor operators](ch005-a-standard-library-of-tensor-operators.md)
- [6. One tensor, several devices and dtypes](ch006-one-tensor-several-devices-and-dtypes.md)

- [7. Reading original weights](ch007-reading-original-weights.md)
- [8. Models travel between frameworks](ch008-models-travel-between-frameworks.md)
- [9. Text becomes a model request](ch009-text-becomes-a-model-request.md)
- [10. Attention without training](ch010-attention-without-training.md)
- [11. A hybrid sequence model](ch011-a-hybrid-sequence-model.md)
- [12. Decisions have a different mask](ch012-decisions-have-a-different-mask.md)
- [13. Read the options](ch013-read-the-options.md)
- [14. Execute Elephant](ch014-execute-elephant.md)
- [15. Execute Mammoth](ch015-execute-mammoth.md)
- [16. Are the probabilities any good?](ch016-are-the-probabilities-any-good.md)
- [17. Qualify honestly](ch017-qualify-honestly.md)

- [18. Time a decision](ch018-time-a-decision.md)
- [19. Keep the model resident](ch019-keep-the-model-resident.md)
- [20. Serve `/v1/systemone`](ch020-serve-systemone.md)
- [21. The protobuf wire](ch021-the-protobuf-wire.md)
- [22. Run it live](ch022-run-it-live.md)
- [23. Make the numbers move](ch023-make-the-numbers-move.md)

- [24. A Pylon answers decisions](ch024-a-pylon-answers-decisions.md) — first mesh chapter
- [25. The API fans out](ch025-the-api-fans-out.md)
- [26. Check the mesh](ch026-check-the-mesh.md)
- [27. A public mesh](ch027-a-public-mesh.md)
- [28. A decision arrives while play continues](ch028-a-decision-arrives-while-play-continues.md)
- [29. A guard asks a Pylon](ch029-a-guard-asks-a-pylon.md)
- [30. Measure a guard decision](ch030-measure-a-guard-decision.md)
- [31. Directions on request](ch031-directions-on-request.md)
- [32. A guard gives directions](ch032-a-guard-gives-directions.md)

- [36. Many questions at once](ch036-many-questions-at-once.md)
- [37. A Coax compute subset](ch037-a-coax-compute-subset.md) — first device chapter
- [38. A tensor program becomes an execution plan](ch038-a-tensor-program-becomes-an-execution-plan.md)
- [39. Compile a general tensor program](ch039-compile-a-general-tensor-program.md)
- [40. Add your own operator](ch040-add-your-own-operator.md)
- [44. Elephant on CUDA](ch044-elephant-on-cuda.md)
- [45. Share the card](ch045-share-the-card.md)
- [46. Mammoth on CUDA](ch046-mammoth-on-cuda.md)
- [47. Elephant on Metal](ch047-elephant-on-metal.md)
- [49. Read a GGUF file](ch049-read-a-gguf-file.md)

- [52. Tokenizers and chat templates](ch052-tokenizers-and-chat-templates.md)

- [60. Original MXFP4 weights](ch060-original-mxfp4-weights.md) — first gpt-oss branch chapter
- [61. A few experts, then a router](ch061-a-few-experts-then-a-router.md)
- [62. Attention with sinks and a window](ch062-attention-with-sinks-and-a-window.md)
- [63. The harmony format](ch063-the-harmony-format.md)
- [64. Execute gpt-oss-20b](ch064-execute-gpt-oss-20b.md)
- [65. Original weights stay on the GPU](ch065-original-weights-stay-on-the-gpu.md)
- [66. A complete CUDA decoder](ch066-a-complete-cuda-decoder.md)
- [67. Prefill together](ch067-prefill-together.md)
- [70. Replay the device program](ch070-replay-the-device-program.md)
- [72. gpt-oss-20b on Metal](ch072-gpt-oss-on-metal.md)
- [73. Execute gpt-oss-120b](ch073-execute-gpt-oss-120b.md)
- [75. Let the small model guess](ch075-let-the-small-model-guess.md)
- [77. Call a tool](ch077-call-a-tool.md)
- [79. Serve Responses](ch079-serve-responses.md)
- [80. Two models, one card](ch080-two-models-one-card.md)

- [88. Execute a DeepSeek R1 distill](ch088-execute-a-deepseek-r1-distill.md)
- [89. DeepSeek’s front door](ch089-deepseeks-front-door.md)
- [90. Execute DeepSeek V2-Lite](ch090-execute-deepseek-v2-lite.md)
- [91. Routers that balance themselves](ch091-routers-that-balance-themselves.md)
- [92. Compressed and shared attention](ch092-compressed-and-shared-attention.md)
- [93. Many residual streams](ch093-many-residual-streams.md)
- [94. Memory by lookup](ch094-memory-by-lookup.md)
- [95. V4.1's expert weights](ch095-v41s-expert-weights.md)
- [96. Hot experts in fast memory](ch096-hot-experts-in-fast-memory.md)
- [97. Prefill layer by layer](ch097-prefill-layer-by-layer.md)
- [98. Pack the cache exactly](ch098-pack-the-cache-exactly.md)

- [103. Learning with arrays](ch103-learning-with-arrays.md) — first learning chapter
- [104. Gradients as a program](ch104-gradients-as-a-program.md)
- [106. Differentiate any tensor program](ch106-differentiate-any-tensor-program.md)
- [107. Transform functions, not just tensors](ch107-transform-functions-not-just-tensors.md) — composable graph-function derivatives and batching
- [109. Learn a physical system](ch109-learn-a-physical-system.md) — own RK4 dynamics, learned surrogate and held-out rollouts
- [111. Recover the physics from the motion](ch111-recover-the-physics-from-the-motion.md) — exact-solver gradients and identifiable motion
- [112. Bytes and a small language model](ch112-bytes-and-a-small-language-model.md)
- [113. Differentiate a decoder](ch113-differentiate-a-decoder.md)

- [122. A checkpoint holds the whole experiment](ch122-a-checkpoint-holds-the-whole-experiment.md)
- [123. Learn a tokenizer](ch123-learn-a-tokenizer.md)
- [124. AdamW, Muon and explicit precision](ch124-adamw-and-explicit-precision.md)


- [167. Command a robot](ch167-command-a-robot.md) — design draft
- [168. Imitation before robotics scale](ch168-imitation-before-robotics-scale.md) — first robotics chapter: owned simulation and original NVIDIA data
- [169. Execute a GR00T policy](ch169-execute-a-groot-policy.md) — original learned VLA on Metal, bounded prepared-observation profile
- [170. Close the simulated control loop](ch170-close-the-simulated-control-loop.md) — original GR00T drives a Panda through Coax control, with exact SDK physics replay
- [171. Learn a gait in many worlds](ch171-learn-a-gait-in-many-worlds.md) — design draft

- [172. Benchmark a service, not a kernel](ch172-benchmark-a-service-not-a-kernel.md) — first production-hosting chapter

- [215. Fit our own calibration, and refuse a stale one](ch215-fit-our-own-calibration.md)
- [216. Thresholds from written costs](ch216-thresholds-from-written-costs.md)
- [217. Record readings, join outcomes, watch drift](ch217-record-readings-join-outcomes-watch-drift.md)
- [218. A Pylon checks itself before serving](ch218-a-pylon-checks-itself-before-serving.md)
- [219. How far apart may two honest answers be?](ch219-how-far-apart-may-two-honest-answers-be.md)
- [220. Read a bounded ONNX graph](ch220-read-a-bounded-onnx-graph.md)
- [221. Write a bounded ONNX graph](ch221-write-a-bounded-onnx-graph.md)
- [224. Shadow a new model, then promote it](ch224-shadow-a-new-model-then-promote-it.md)
- [225. Learn which model to call](ch225-learn-which-model-to-call.md)

- [226. A machine whose state is a trace](ch226-a-machine-whose-state-is-a-trace.md)
- [227. Attention as exact memory](ch227-attention-as-exact-memory.md)
- [228. Arithmetic and branches from ReLU](ch228-arithmetic-and-branches-from-relu.md)
- [229. Compile a gate graph into a block](ch229-compile-a-gate-graph-into-a-block.md)
- [230. A harness that catches the compiler](ch230-a-harness-that-catches-the-compiler.md)
- [231. An interpreter in weights, or one program in weights](ch231-an-interpreter-in-weights-or-one-program-in-weights.md)
- [232. Decode in logarithmic time](ch232-decode-in-logarithmic-time.md)
- [233. From hard-max to softmax](ch233-from-hard-max-to-softmax.md)
- [234. Exact replay as a verification class](ch234-exact-replay-as-a-verification-class.md)
- [235. Can gradient descent learn exact execution?](ch235-can-gradient-descent-learn-exact-execution.md)
- [236. A frozen exact core with a learned interface](ch236-a-frozen-exact-core-with-a-learned-interface.md)
- [237. Composable weight modules](ch237-composable-weight-modules.md)

- [239. A suite is three digests](ch239-a-suite-is-three-digests.md)
- [240. How far a metric moves by itself](ch240-how-far-a-metric-moves-by-itself.md)
- [241. Gates that can say unverifiable](ch241-gates-that-can-say-unverifiable.md)
- [242. Environments with checkers](ch242-environments-with-checkers.md)
- [243. Fit settings on recorded answers](ch243-fit-settings-on-recorded-answers.md)
- [244. Search a policy, cheapest first](ch244-search-a-policy-cheapest-first.md)
- [247. Outcomes become training rows](ch247-outcomes-become-training-rows.md)
- [248. Is the loop improving?](ch248-is-the-loop-improving.md)
- [249. An agent optimizes a Psionic kernel](ch249-an-agent-optimizes-a-psionic-kernel.md)

Proposed: [full table of contents and prerequisites](PLAN.md), from early
Elephant/Mammoth through gpt-oss, Mistral and DeepSeek, nanochat
pretraining/post-training, NVIDIA retrieval, vision, motion, speech, scientific
ML, world generation, robot policies, production
hosting and inference across a mesh.
Chapters 1–32, 36–40, 44–47, 49, 52, 60–67, 70, 72, 73, 75, 77, 79, 80, 88–98, 103, 104, 106, 107, 109, 111, 112, 123, 124, 168–170, 172, 215–221, 224–237, 239–244 and 247–249 have runnable code; the [introduction](README.md)
table gives each one's exact status. Chapters 167 and 171 are design
drafts with no runnable code. The physics engine and spacecraft chapters
are in the [Aethyr book](../../aethyr/book/README.md). The remaining outlines remain proposed.
