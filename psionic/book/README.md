# Building Psionic in Coax

This book builds a machine-learning library from small understandable pieces.
Every completed chapter runs the actual [Psionic library](../README.md), with
checks and independently specified results.

[Chapter 1](ch001-learning-a-number.md) starts with learning a single number.
You need basic programming and enough arithmetic to multiply and average two
numbers. It explains the small amount of calculus as it goes. For the
language, use [the Coax book](../../coax/book/README.md).

Coax listings, equations and short illustrative blocks use `jai` Markdown
fences for syntax highlighting. Command blocks use `shell`.

Commands run from the OpenAgents repository root. `psionic/check.sh` checks the
whole current book and library. Build once with `coax/build.sh` if you want to
run examples directly without the check command.

| Chapter | Status | Result |
| --- | --- | --- |
| [1. Learning a number](ch001-learning-a-number.md) | Runnable: CPU | Train `prediction = weight * x`, check every loss, predict a held-out value, and refuse malformed work. |
| [2. Shapes and storage](ch002-shapes-and-storage.md) | Runnable: CPU | Admit rank-zero through rank-four shapes, count bytes safely, locate values and stage a copy into caller storage. |
| [2a. A tensor is a view of memory](ch002a-a-tensor-is-a-view-of-memory.md) | Runnable: bounded CPU f32 views | Signed strides, slicing, transposes, read-only broadcasting, empty tensors and saved-value mutation checks. |
| [3. Numbers and matrices](ch003-numbers-and-matrices.md) | Runnable: CPU-f32-wide-v1 | Add, reduce, multiply matrices, gather rows, apply masked softmax and normalize rows with independent numerical checks. |
| [3a. A standard library of tensor operators](ch003a-a-standard-library-of-tensor-operators.md) | Runnable: CPU-f32-wide-views-v1 | 27 operators, shape/dtype inference, support manifest and analytical value VJPs over strided views. |
| [4. Reading original weights](ch004-reading-original-weights.md) | Runnable: original-weights-v1 | Inspect and bind original single/sharded BF16/F32 artifacts, decode finite windows and use imported values in shared kernels. |
| [5. Text becomes a model request](ch005-text-becomes-a-model-request.md) | Runnable: qwen35-text-v1 | Render typed decisions, normalize/tokenize original Qwen data and validate complete shared-prefix/suffix layouts. |
| [6. Attention without training](ch006-attention-without-training.md) | Runnable: bounded CPU | Execute a dense causal decoder block with grouped-query attention, rotary positions and a key/value cache. |
| [7. A hybrid sequence model](ch007-a-hybrid-sequence-model.md) | Runnable: bounded CPU | Compose Qwen-style causal linear and full-attention layers with their separate states. |
| [8. Decisions have a different mask](ch008-decisions-have-a-different-mask.md) | Runnable: bounded CPU | Compute bidirectional full-attention blocks and fork all live hybrid prefix state. |
| [9. Read the options](ch009-read-the-options.md) | Runnable: bounded CPU | Compute temperature-scaled option probabilities, vocabulary slot mass and typed decision statistics. |
| [10. Execute Elephant](ch010-execute-elephant.md) | Runnable: original weights, CPU | Authenticate Elephant and execute a typed text decision through its complete hybrid stack. |
| [11. Execute Mammoth](ch011-execute-mammoth.md) | Runnable: original weights, CPU | Plan Mammoth's storage, authenticate both shards and execute the same typed text decision through all 64 layers. |
| [12. Are the probabilities any good?](ch012-are-the-probabilities-any-good.md) | Runnable: CPU; original weights for the held-out run | Load the bundle's calibration (none: temperature 1.0), print the published answer JSON and measure accuracy, Brier score, calibration error and abstention on a frozen held-out set. |
| [13. Qualify honestly](ch013-qualify-honestly.md) | Runnable: CPU | Freeze comparison policies by digest, bind results to the exact build, make the exit status the verdict, and find chapter 10's evidence unknown, not qualified. |
| [16. Serve `/v1/systemone`](ch016-serve-systemone.md) | Runnable: loopback, toy and original Elephant, CPU | Answer NIP-DEC decision bodies over HTTP with typed refusals, deadlines, cancellation, busy refusals and the served model identity. |
| [16b. The protobuf wire](ch016b-the-protobuf-wire.md) | Runnable: toy engine and native answers | Accept `application/x-protobuf` requests by transcoding them to the NIP-DEC body chapter 16 admits, and answer in protobuf on `Accept: application/x-protobuf`. |
| [70. Learning with arrays](ch070-learning-with-arrays.md) | Runnable: cpu-f64-sgd-v1, simulated data | Predict whether a request meets its deadline with a constant, a handwritten rule, a line, logistic regression and one hidden layer: hand gradients, minibatch SGD, scaling, session splits, written costs, and a case where the larger model loses. |
| [71. Gradients as a program](ch071-gradients-as-a-program.md) | Runnable: cpu-f64-tape-v1 | A model as a list of operations, a forward pass that saves every value and a reverse pass that accumulates adjoints; checked against 60-digit central differences and by retraining chapter 70's models. |
| [41. Original MXFP4 weights](ch041-original-mxfp4-weights.md) | Runnable: bounded CPU format decoder | Decode original gpt-oss packed groups exactly, with independent bit oracles and atomic refusals. |
| [42. A few experts, then a router](ch042-a-few-experts-then-a-router.md) | Runnable: bounded CPU router and tiny affine experts | Select and normalize top-k routes, dispatch by resident expert ID and publish a checked weighted mixture. |
| [43. Attention with sinks and a window](ch043-attention-with-sinks-and-a-window.md) | Runnable: gpt-oss-attention-cpu-wide-v1 | Compute per-head sink attention with a bounded ring, split-half YaRN, biased projection and clamped expert activation. |
| [44. The harmony format](ch044-the-harmony-format.md) | Runnable: gpt-oss-harmony-text-v1 | Encode original byte ranks, frame typed messages with reasoning effort and parse strict complete replies with exact turn/handoff stops. |
| [45. Execute gpt-oss-20b](ch045-execute-gpt-oss-20b.md) | Runnable: original CPU model and seeded generation | Complete authenticated 24-layer model, dense references, replay and sampling. |
| [45a. Original weights stay on the GPU](ch045a-original-weights-stay-on-the-gpu.md) | Runnable: original 20b CUDA residency | Packed original weights, complete resident memory plan and focused owned checks. |
| [45b. A complete CUDA decoder](ch045b-a-complete-cuda-decoder.md) | Runnable: original 20b CUDA greedy generation | Every layer, cache, router and full head stay on device; complete Harmony final display. |
| [45c. Prefill together](ch045c-prefill-together.md) | Runnable: batched CUDA prefill | Coefficient reuse across tokens, grouped experts, causal chunk attention and atomic cache shadows. |
| [45f. Replay the device program](ch045f-replay-the-device-program.md) | Runnable: exact CUDA decode replay | Coax compares admitted descriptors; ordered graph nodes reuse bindings and patch changing arguments. |
| [46. Execute gpt-oss-120b](ch046-execute-gpt-oss-120b.md) | Runnable: original CPU model and residency | The same engine, 36 layers and 128 experts, original shards and measured memory. |
| [47. Let the small model guess](ch047-let-the-small-model-guess.md) | Runnable: serial CPU speculative profile | Standard acceptance/correction, block bonus, whole-cache rollback, prompt n-grams and original greedy parity. |
| [48. Call a tool](ch048-call-a-tool.md) | Runnable: finite JSON-schema enum and granted tools | Mask before sampling, typed commentary handoffs, tool-role results and final-only display. |
| [48b. Serve Responses](ch048b-serve-responses.md) | Runnable: resident original 20b CUDA, bounded Responses profile | Coax HTTP/JSON, typed reasoning/function replay, incremental SSE, cancellation and SDK checks. |
| [56. Execute a DeepSeek R1 distill](ch056-execute-a-deepseek-r1-distill.md) | Runnable: original BF16 CPU reference | Qwen2 execution, original tokenizer BOS and stripped-history recipe, greedy generation, separate reasoning/final intervals. |
| [57. DeepSeek’s front door](ch057-deepseeks-front-door.md) | Runnable: bounded recipe and stream protocol | Typed special boundaries, independent plain-run BPE, incremental UTF-8 and DSML tool events, explicit finish. |
| [58. Execute DeepSeek V2-Lite](ch058-execute-deepseek-v2-lite.md) | Runnable: complete original BF16 CPU reference | Complete dense/MoE stack, compressed/expanded latent caches, atomic token commit, independent original generation and offline replay. |
| [59. Routers that balance themselves](ch059-routers-that-balance-themselves.md) | Runnable: CPU wide routing equations | Selection-only correction, V3 grouped sigmoid and V4.1 square-root-softplus, deterministic ties and complete margin comparisons. |
| [61. Many residual streams](ch061-many-residual-streams.md) | Runnable: CPU wide hyper-connection equations | Four streams, pre/post and fixed Sinkhorn mixing, previous-mix state across sub-block/stage boundaries and final collapse. |
| [156. Fit our own calibration, and refuse a stale one](ch156-fit-our-own-calibration.md) | Runnable: cpu-f64-calibrate-v1, synthetic probabilities | Fit per-question temperature, binned and isotonic maps on one split, keep one only if it lowers calibration error and log loss on a frozen gate split, report it once on a test split, and refuse it for any other model version or question set. |
| [157. Thresholds from written costs](ch157-thresholds-from-written-costs.md) | Runnable: cpu-f64-threshold-v1, synthetic probabilities | Derive act, escalate and ask bands for an Elephant and Mammoth cascade from two written cost tables, freeze them after validation, and score them once on a confirmation set with intervals against always Elephant, always Mammoth and a fixed rule. |
| [158. Record readings, join outcomes, watch drift](ch158-record-readings-join-outcomes-watch-drift.md) | Runnable: cpu-f64-drift-v1, simulated drift | Log every decision with no content, join late or lost outcomes, flag windows whose calibration error calibrated draws rarely reach, refit maps on a schedule through chapter 156's gate, and estimate other thresholds from the log, refusing one the log never explored. |
| [164a. Learn which model to call](ch164a-learn-which-model-to-call.md) | Runnable: cpu-f64-route-v1, simulated tasks and models | Route tasks among three models by learned success minus price, then judge a new router from an old one's logs with naive, direct, inverse-propensity and doubly robust estimates, and refuse when the logs never tried a model it calls. |
| [165. A machine whose state is a trace](ch165-a-machine-whose-state-is-a-trace.md) | Runnable: bounded CPU reference interpreter | Admit and run a small integer stack machine whose four-token-per-step trace and SHA-256 digest identify the run; a refused run publishes nothing. |
| [166. Attention as exact memory](ch166-attention-as-exact-memory.md) | Runnable: bounded CPU, integer scores | Parabolic keys and hard-max reads return exactly the latest write or refuse; the machine with attention as its only memory reproduces the interpreter's digest. |
| [167. Arithmetic and branches from ReLU](ch167-arithmetic-and-branches-from-relu.md) | Runnable: bounded CPU, binary64 and binary32 | A gated ReLU layer computes integer products, steps, equality and selection exactly within ±2²⁶ in binary64; binary32 only while products stay under 2²⁴. |
| [168. Compile a gate graph into a block](ch168-compile-a-gate-graph-into-a-block.md) | Runnable: bounded CPU, binary64 | A gate language, its integer evaluator, a scheduler that reuses residual slots, and emitted weights whose forward pass reproduces the evaluator exactly. |
| [169. A harness that catches the compiler](ch169-a-harness-that-catches-the-compiler.md) | Runnable: bounded CPU | Run every program through the evaluator, a plan interpreter and the weights; catch four planted compiler bugs and place each in the schedule or the emitter. |
| [172. From hard-max to softmax](ch172-from-hard-max-to-softmax.md) | Runnable: bounded CPU, binary64/binary32/bfloat16 rounding | Prove softmax reads stay exact for any trace length once β ≥ ln(4V), and measure where binary32 and bfloat16 rounding breaks them. |

Chapters 70–71 start learning from data on chapter 3's operations, 156 fits its own calibration, 157 derives thresholds from written costs, 158 watches them drift, 164a uses chapter 70 to learn which model to call, chapters 41–48 start an independent gpt-oss branch, and 165–169 and 172 open Part XXIII (programs compiled into weights); chapters 14–40 other than 16 remain planned.

The [contents](SUMMARY.md) list completed chapters; the [chapter plan](PLAN.md)
sets the proposed full reading order: early Elephant/Mammoth and useful
retrieval, gpt-oss, Mistral and DeepSeek, then training/nanochat, vision,
motion, speech, physics, composite models, production hosting and inference
across a mesh.
Its prerequisite map lets independent branches start earlier than their
position in the contents. The [roadmap](../docs/roadmap.md) lists
future work. Planned language and
ML features are not runnable claims. Exact transcripts describe the current
CPU arithmetic/printing profile; per-value tolerances state the numerical
contract independently of formatting.
