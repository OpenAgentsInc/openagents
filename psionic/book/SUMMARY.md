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
- [29. A guard asks a Pylon](ch029-a-guard-asks-a-pylon.md) — partly implemented
- [30. Measure a guard decision](ch030-measure-a-guard-decision.md)
- [31. Directions on request](ch031-directions-on-request.md) — partly implemented
- [32. A guard gives directions](ch032-a-guard-gives-directions.md)

- [33. Measure the work and plan the memory](ch033-measure-the-work-and-plan-the-memory.md) — design draft
- [34. Benchmark against llama.cpp and cuBLAS](ch034-benchmark-against-llama-cpp-and-cublas.md) — design draft
- [35. Use every core](ch035-use-every-core.md) — design draft
- [36. Many questions at once](ch036-many-questions-at-once.md)
- [37. A Coax compute subset](ch037-a-coax-compute-subset.md) — first device chapter
- [38. A tensor program becomes an execution plan](ch038-a-tensor-program-becomes-an-execution-plan.md)
- [39. Compile a general tensor program](ch039-compile-a-general-tensor-program.md)
- [40. Add your own operator](ch040-add-your-own-operator.md)
- [41. Attention in tiles](ch041-attention-in-tiles.md) — design draft
- [42. DeltaNet in chunks](ch042-deltanet-in-chunks.md) — design draft
- [43. Fuse and replay](ch043-fuse-and-replay.md) — design draft
- [44. Elephant on CUDA](ch044-elephant-on-cuda.md)
- [45. Share the card](ch045-share-the-card.md)
- [46. Mammoth on CUDA](ch046-mammoth-on-cuda.md)
- [47. Elephant on Metal](ch047-elephant-on-metal.md)
- [48. Integer matrix products](ch048-integer-matrix-products.md) — design draft
- [49. Read a GGUF file](ch049-read-a-gguf-file.md)
- [50. Quantized weights and caches](ch050-quantized-weights-and-caches.md) — design draft
- [51. K-quants and lattice quants](ch051-k-quants-and-lattice-quants.md) — design draft

- [52. Tokenizers and chat templates](ch052-tokenizers-and-chat-templates.md)
- [53. Choosing the next token](ch053-choosing-the-next-token.md) — partly implemented
- [54. Search the Coax book](ch054-search-the-coax-book.md)
- [55. Search without scanning everything](ch055-search-without-scanning-everything.md) — partly implemented
- [56. Rerank the candidates](ch056-rerank-the-candidates.md) — design draft
- [57. Answer from current sources](ch057-answer-from-current-sources.md) — design draft
- [58. Continue a passage](ch058-continue-a-passage.md) — design draft
- [59. Run in the browser](ch059-run-in-the-browser.md) — design draft

- [60. Original MXFP4 weights](ch060-original-mxfp4-weights.md) — first gpt-oss branch chapter
- [61. A few experts, then a router](ch061-a-few-experts-then-a-router.md)
- [62. Attention with sinks and a window](ch062-attention-with-sinks-and-a-window.md)
- [63. The harmony format](ch063-the-harmony-format.md)
- [64. Execute gpt-oss-20b](ch064-execute-gpt-oss-20b.md)
- [65. Original weights stay on the GPU](ch065-original-weights-stay-on-the-gpu.md)
- [66. A complete CUDA decoder](ch066-a-complete-cuda-decoder.md)
- [67. Prefill together](ch067-prefill-together.md)
- [68. Coax kernels cooperate](ch068-coax-kernels-cooperate.md) — design draft
- [69. Original MXFP4 products use matrix hardware](ch069-original-mxfp4-products-use-matrix-hardware.md) — design draft
- [70. Replay the device program](ch070-replay-the-device-program.md)
- [71. Throughput is a gate](ch071-throughput-is-a-gate.md) — design draft
- [72. gpt-oss-20b on Metal](ch072-gpt-oss-on-metal.md)
- [73. Execute gpt-oss-120b](ch073-execute-gpt-oss-120b.md)
- [74. Fit 120b on the right device](ch074-fit-120b-on-the-right-device.md) — design draft
- [75. Let the small model guess](ch075-let-the-small-model-guess.md)
- [76. Verify a draft block in parallel](ch076-verify-a-draft-block-in-parallel.md) — design draft
- [77. Call a tool](ch077-call-a-tool.md)
- [78. Device generation calls a tool](ch078-device-generation-calls-a-tool.md) — design draft
- [79. Serve Responses](ch079-serve-responses.md)
- [80. Two models, one card](ch080-two-models-one-card.md)

- [81. Execute Ministral 4](ch081-execute-ministral-4.md) — design draft
- [82. Original FP8 weights](ch082-original-fp8-weights.md) — design draft
- [83. A coding agent](ch083-a-coding-agent.md) — design draft
- [84. Attention through a latent](ch084-attention-through-a-latent.md) — design draft
- [85. Execute Mistral Small 7](ch085-execute-mistral-small-7.md) — design draft
- [86. Sparse attention and shared layers](ch086-sparse-attention-and-shared-layers.md) — design draft
- [87. The model's own draft head](ch087-the-models-own-draft-head.md) — design draft

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
- [99. A decode step with no host waits](ch099-a-decode-step-with-no-host-waits.md) — design draft
- [100. Execute DeepSeek V4.1 Flash on one machine](ch100-execute-deepseek-v41-flash-on-one-machine.md) — design draft
- [101. Maya's recipe: V4.1 Flash on one card](ch101-mayas-recipe-v41-flash-on-one-card.md) — design draft
- [102. Verify drafts by expected gain](ch102-verify-drafts-by-expected-gain.md) — design draft

- [103. Learning with arrays](ch103-learning-with-arrays.md) — first learning chapter
- [104. Gradients as a program](ch104-gradients-as-a-program.md)
- [105. Make a model fail, then find out why](ch105-make-a-model-fail-then-find-out-why.md) — design draft
- [106. Differentiate any tensor program](ch106-differentiate-any-tensor-program.md)
- [107. Transform functions, not just tensors](ch107-transform-functions-not-just-tensors.md) — composable graph-function derivatives and batching
- [108. Choose the next example to learn from](ch108-choose-the-next-example-to-learn-from.md) — design draft
- [109. Learn a physical system](ch109-learn-a-physical-system.md) — own RK4 dynamics, learned surrogate and held-out rollouts
- [110. A grid world that learns, plans and recovers](ch110-a-grid-world-that-learns-plans-and-recovers.md) — design draft
- [111. Recover the physics from the motion](ch111-recover-the-physics-from-the-motion.md) — exact-solver gradients and identifiable motion
- [112. Bytes and a small language model](ch112-bytes-and-a-small-language-model.md)
- [113. Differentiate a decoder](ch113-differentiate-a-decoder.md)
- [114. Compile the backward pass](ch114-compile-the-backward-pass.md) — design draft
- [115. Train the whole tiny decoder](ch115-train-the-whole-tiny-decoder.md) — design draft
- [116. Train within a memory budget](ch116-train-within-a-memory-budget.md) — design draft
- [117. Models are compositions of modules](ch117-models-are-compositions-of-modules.md) — partly implemented
- [118. One trainer for every model](ch118-one-trainer-for-every-model.md) — design draft
- [119. Change an activation, change the answer](ch119-change-an-activation-change-the-answer.md) — design draft

- [120. Data and resumable state](ch120-data-and-resumable-state.md)
- [121. Feed the accelerator](ch121-feed-the-accelerator.md) — design draft
- [122. A checkpoint holds the whole experiment](ch122-a-checkpoint-holds-the-whole-experiment.md)
- [123. Learn a tokenizer](ch123-learn-a-tokenizer.md)
- [124. AdamW, Muon and explicit precision](ch124-adamw-and-explicit-precision.md)
- [125. Execute the pinned nanochat GPT](ch125-execute-the-pinned-nanochat-gpt.md)

- [126. Execute nanochat exactly](ch126-execute-nanochat-exactly.md) — design draft
- [127. Differentiate nanochat](ch127-differentiate-nanochat.md) — design draft
- [128. A complete pretraining run](ch128-a-complete-pretraining-run.md) — design draft
- [129. Train a whole model on one GPU](ch129-train-a-whole-model-on-one-gpu.md) — design draft
- [130. Profile a training step](ch130-profile-a-training-step.md) — design draft
- [131. Train the same model eager and compiled](ch131-train-the-same-model-eager-and-compiled.md) — design draft
- [132. Train across devices](ch132-train-across-devices.md) — design draft
- [133. A tensor spread over devices](ch133-a-tensor-spread-over-devices.md) — design draft
- [134. Data-parallel training](ch134-data-parallel-training.md) — design draft
- [135. Fully sharded training](ch135-fully-sharded-training.md) — design draft
- [136. Supervised conversations and tools](ch136-supervised-conversations-and-tools.md) — design draft
- [137. Continued pretraining and task adaptation](ch137-continued-pretraining-and-task-adaptation.md) — design draft
- [138. Preferences and reward models](ch138-preferences-and-reward-models.md) — design draft
- [139. Learn from rollouts](ch139-learn-from-rollouts.md) — design draft
- [140. Post-train a decision model](ch140-post-train-a-decision-model.md) — design draft
- [141. Distill Mammoth into Elephant](ch141-distill-mammoth-into-elephant.md) — design draft

- [142. Pixels, convolutions and patches](ch142-pixels-convolutions-and-patches.md) — partly implemented
- [143. Elephant and Mammoth see](ch143-elephant-and-mammoth-see.md) — design draft
- [144. See with C-RADIO](ch144-see-with-c-radio.md) — design draft
- [145. Compress and reconstruct an image](ch145-compress-and-reconstruct-an-image.md) — design draft
- [146. Noise, diffusion and flow](ch146-noise-diffusion-and-flow.md) — design draft

- [147. Poses have conventions](ch147-poses-have-conventions.md)
- [148. Motion tokens become poses](ch148-motion-tokens-become-poses.md) — design draft
- [149. Execute MotionBricks](ch149-execute-motionbricks.md) — design draft
- [150. Put generated motion in Verse](ch150-put-generated-motion-in-verse.md) — design draft

- [151. Waveforms and spectral features](ch151-waveforms-and-spectral-features.md) — partly implemented
- [152. A speech encoder](ch152-a-speech-encoder.md) — design draft
- [153. Transcribe with Parakeet](ch153-transcribe-with-parakeet.md) — design draft
- [154. Transcribe while someone speaks](ch154-transcribe-while-someone-speaks.md) — design draft
- [155. Speak the answer](ch155-speak-the-answer.md) — design draft

- [156. A spectral neural operator](ch156-a-spectral-neural-operator.md) — design draft
- [157. Learn Darcy flow](ch157-learn-darcy-flow.md) — design draft
- [158. A field around a shape](ch158-a-field-around-a-shape.md) — design draft
- [159. Learn a scene from its views](ch159-learn-a-scene-from-its-views.md) — design draft
- [160. Execute DoMINO](ch160-execute-domino.md) — design draft
- [161. Forecast on a sphere](ch161-forecast-on-a-sphere.md) — design draft

- [162. A recurrence you can inspect](ch162-a-recurrence-you-can-inspect.md) — design draft
- [163. Execute Nemotron Nano](ch163-execute-nemotron-nano.md) — design draft
- [164. Execute Nemotron Lightning](ch164-execute-nemotron-lightning.md) — design draft

- [165. Sequences of observations](ch165-sequences-of-observations.md) — design draft
- [166. One complete Cosmos3-Edge path](ch166-one-complete-cosmos3-edge-path.md) — design draft
- [167. Command a robot](ch167-command-a-robot.md) — runnable position/pose IK, linearized posture projection, rest-to-rest timing, PD gains, short owned Euler loop, nonlinear control measurements, bounded gain study (no qualified family) and scheduled PD (original cases and two frozen C/D poses; no-contact SDK/own parity)
- [168. Imitation before robotics scale](ch168-imitation-before-robotics-scale.md) — first robotics chapter: owned simulation and original NVIDIA data
- [169. Execute a GR00T policy](ch169-execute-a-groot-policy.md) — original learned VLA on Metal, bounded prepared-observation profile
- [170. Close the simulated control loop](ch170-close-the-simulated-control-loop.md) — genuine GR00T/Panda SDK evaluation plus a first 9-action neural prefix with owned controller samples and exact physics replay
- [171. Learn a gait in many worlds](ch171-learn-a-gait-in-many-worlds.md) — runnable GAE/Gaussian and scalar-LQR PPO; all three 512-batch continuations meet fixed goals, gait planned

- [172. Benchmark a service, not a kernel](ch172-benchmark-a-service-not-a-kernel.md) — first production-hosting chapter
- [173. Serve a model with an honest capability](ch173-serve-a-model-with-an-honest-capability.md) — design draft
- [174. Clients, streams and background jobs](ch174-clients-streams-and-background-jobs.md) — design draft
- [175. Continuous batching and a fair scheduler](ch175-continuous-batching-and-a-fair-scheduler.md) — design draft
- [176. Paged state and a tiered prefix cache](ch176-paged-state-and-a-tiered-prefix-cache.md) — design draft
- [177. Route, queue and scale replicas](ch177-route-queue-and-scale-replicas.md) — design draft
- [178. Start fast](ch178-start-fast.md) — design draft
- [179. Ship without downtime](ch179-ship-without-downtime.md) — design draft
- [180. Observe, meter and price](ch180-observe-meter-and-price.md) — design draft
- [181. Survive failures](ch181-survive-failures.md) — design draft
- [182. Disaggregate prefill and decode](ch182-disaggregate-prefill-and-decode.md) — design draft
- [183. Split a model across devices](ch183-split-a-model-across-devices.md) — design draft
- [184. A pipeline of models](ch184-a-pipeline-of-models.md) — design draft
- [185. A decision model steers voice](ch185-a-decision-model-steers-voice.md) — design draft

- [186. Split a model into stages](ch186-split-a-model-into-stages.md) — design draft
- [187. Stage packages](ch187-stage-packages.md) — design draft
- [188. Peers, transport and discovery](ch188-peers-transport-and-discovery.md) — design draft
- [189. Place the model](ch189-place-the-model.md) — design draft
- [190. Stage lifecycle and fencing](ch190-stage-lifecycle-and-fencing.md) — design draft
- [191. Experts across machines and from disk](ch191-experts-across-machines-and-from-disk.md) — design draft
- [192. Route by saved work](ch192-route-by-saved-work.md) — design draft
- [193. Compress activations exactly](ch193-compress-activations-exactly.md) — design draft
- [194. Who can read the prompt](ch194-who-can-read-the-prompt.md) — design draft
- [195. Check a peer's work](ch195-check-a-peers-work.md) — design draft
- [196. Work with Mesh-LLM](ch196-work-with-mesh-llm.md) — design draft
- [197. DeepSeek V4.1 Flash across stages](ch197-deepseek-v41-flash-across-stages.md) — design draft
- [198. Mistral Large 4 across a mesh](ch198-mistral-large-4-across-a-mesh.md) — design draft
- [199. Le Chonk across a mesh](ch199-le-chonk-across-a-mesh.md) — design draft

- [200. The simplest loop](ch200-the-simplest-loop.md) — design draft
- [201. Model-visible means logged](ch201-model-visible-means-logged.md) — design draft
- [202. Keep the prefix stable](ch202-keep-the-prefix-stable.md) — design draft
- [203. Tools as typed capabilities](ch203-tools-as-typed-capabilities.md) — design draft
- [204. A sandbox and approvals](ch204-a-sandbox-and-approvals.md) — design draft
- [205. A document tries to become the system prompt](ch205-a-document-tries-to-become-the-system-prompt.md) — design draft
- [206. Condense the history](ch206-condense-the-history.md) — design draft
- [207. Programs as tool calls](ch207-programs-as-tool-calls.md) — design draft
- [208. Forks, subagents and jobs](ch208-forks-subagents-and-jobs.md) — design draft
- [209. Plugins with owned lifecycles](ch209-plugins-with-owned-lifecycles.md) — design draft
- [210. Speak the protocols](ch210-speak-the-protocols.md) — design draft
- [211. Measure the harness](ch211-measure-the-harness.md) — design draft
- [212. Generate candidates, then check them](ch212-generate-candidates-then-check-them.md) — design draft
- [213. Consent before sharing](ch213-consent-before-sharing.md) — design draft
- [214. Remember a fact, then revise it](ch214-remember-a-fact-then-revise-it.md) — design draft

- [215. Fit our own calibration, and refuse a stale one](ch215-fit-our-own-calibration.md)
- [216. Thresholds from written costs](ch216-thresholds-from-written-costs.md)
- [217. Record readings, join outcomes, watch drift](ch217-record-readings-join-outcomes-watch-drift.md)
- [218. A Pylon checks itself before serving](ch218-a-pylon-checks-itself-before-serving.md)
- [219. How far apart may two honest answers be?](ch219-how-far-apart-may-two-honest-answers-be.md)
- [220. Read a bounded ONNX graph](ch220-read-a-bounded-onnx-graph.md)
- [221. Write a bounded ONNX graph](ch221-write-a-bounded-onnx-graph.md)
- [222. A decision in milliseconds](ch222-a-decision-in-milliseconds.md) — design draft
- [223. Distill Elephant into a small router](ch223-distill-elephant-into-a-small-router.md) — design draft
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
- [238. Pay for verified work](ch238-pay-for-verified-work.md) — design draft

- [239. A suite is three digests](ch239-a-suite-is-three-digests.md)
- [240. How far a metric moves by itself](ch240-how-far-a-metric-moves-by-itself.md)
- [241. Gates that can say unverifiable](ch241-gates-that-can-say-unverifiable.md)
- [242. Environments with checkers](ch242-environments-with-checkers.md)
- [243. Fit settings on recorded answers](ch243-fit-settings-on-recorded-answers.md)
- [244. Search a policy, cheapest first](ch244-search-a-policy-cheapest-first.md)
- [245. Reflective prompt evolution](ch245-reflective-prompt-evolution.md) — design draft
- [246. Judges are components](ch246-judges-are-components.md) — design draft
- [247. Outcomes become training rows](ch247-outcomes-become-training-rows.md)
- [248. Is the loop improving?](ch248-is-the-loop-improving.md)
- [249. An agent optimizes a Psionic kernel](ch249-an-agent-optimizes-a-psionic-kernel.md)
- [250. Can someone use Psionic instead of PyTorch?](ch250-can-someone-use-psionic-instead-of-pytorch.md) — design draft

Proposed: [full table of contents and prerequisites](PLAN.md), from early
Elephant/Mammoth through gpt-oss, Mistral and DeepSeek, nanochat
pretraining/post-training, NVIDIA retrieval, vision, motion, speech, scientific
ML, world generation, robot policies, production
hosting and inference across a mesh.
Chapters 1–32, 36–40, 44–47, 49, 52, 53, 55, 60–67, 70, 72, 73, 75, 77, 79, 80, 88–98, 103, 104, 106, 107, 109, 111, 112, 117, 120, 123, 124, 142, 147, 151, 167–172, 215–221, 224–237, 239–244 and 247–249 have runnable code; the [introduction](README.md)
table gives each one's exact status. Planned chapters without runnable code
are design drafts. The physics engine and spacecraft chapters
are in the [Aethyr book](../../aethyr/book/README.md).
