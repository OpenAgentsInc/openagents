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
| [3. Numbers and matrices](ch003-numbers-and-matrices.md) | Runnable: CPU-f32-wide-v1 | Add, reduce, multiply matrices, gather rows, apply masked softmax and normalize rows with independent numerical checks. |
| [4. Reading original weights](ch004-reading-original-weights.md) | Runnable: original-weights-v1 | Inspect and bind original single/sharded BF16/F32 artifacts, decode finite windows and use imported values in shared kernels. |
| [5. Text becomes a model request](ch005-text-becomes-a-model-request.md) | Runnable: qwen35-text-v1 | Render typed decisions, normalize/tokenize original Qwen data and validate complete shared-prefix/suffix layouts. |
| [6. Attention without training](ch006-attention-without-training.md) | Runnable: bounded CPU | Execute a dense causal decoder block with grouped-query attention, rotary positions and a key/value cache. |
| [7. A hybrid sequence model](ch007-a-hybrid-sequence-model.md) | Runnable: bounded CPU | Compose Qwen-style causal linear and full-attention layers with their separate states. |
| [8. Decisions have a different mask](ch008-decisions-have-a-different-mask.md) | Runnable: bounded CPU | Compute bidirectional full-attention blocks and fork all live hybrid prefix state. |
| [9. Read the options](ch009-read-the-options.md) | Runnable: bounded CPU | Compute temperature-scaled option probabilities, vocabulary slot mass and typed decision statistics. |
| [10. Execute Elephant](ch010-execute-elephant.md) | Runnable: original weights, CPU | Authenticate Elephant and execute a typed text decision through its complete hybrid stack. |
| [12. Are the probabilities any good?](ch012-are-the-probabilities-any-good.md) | Runnable: CPU; original weights for the held-out run | Load the bundle's calibration (none: temperature 1.0), print the published answer JSON and measure accuracy, Brier score, calibration error and abstention on a frozen held-out set. |
| [41. Original MXFP4 weights](ch041-original-mxfp4-weights.md) | Runnable: bounded CPU format decoder | Decode original gpt-oss packed groups exactly, with independent bit oracles and atomic refusals. |
| [42. A few experts, then a router](ch042-a-few-experts-then-a-router.md) | Runnable: bounded CPU router and tiny affine experts | Select and normalize top-k routes, dispatch by resident expert ID and publish a checked weighted mixture. |
| [43. Attention with sinks and a window](ch043-attention-with-sinks-and-a-window.md) | Runnable: gpt-oss-attention-cpu-wide-v1 | Compute per-head sink attention with a bounded ring, split-half YaRN, biased projection and clamped expert activation. |
| [44. The harmony format](ch044-the-harmony-format.md) | Runnable: gpt-oss-harmony-text-v1 | Encode original byte ranks, frame typed messages with reasoning effort and parse strict complete replies with exact turn/handoff stops. |
| [45. Execute gpt-oss-20b](ch045-execute-gpt-oss-20b.md) | Runnable: original CPU model and seeded generation | Complete authenticated 24-layer model, dense references, replay and sampling. |
| [45a. Original weights stay on the GPU](ch045a-original-weights-stay-on-the-gpu.md) | Runnable: original 20b CUDA residency | Packed original weights, complete resident memory plan and focused owned checks. |
| [45b. A complete CUDA decoder](ch045b-a-complete-cuda-decoder.md) | Runnable: original 20b CUDA greedy generation | Every layer, cache, router and full head stay on device; complete Harmony final display. |
| [45c. Prefill together](ch045c-prefill-together.md) | Runnable: batched CUDA prefill | Coefficient reuse across tokens, grouped experts, causal chunk attention and atomic cache shadows. |
| [46. Execute gpt-oss-120b](ch046-execute-gpt-oss-120b.md) | Runnable: original CPU model and residency | The same engine, 36 layers and 128 experts, original shards and measured memory. |
| [47. Let the small model guess](ch047-let-the-small-model-guess.md) | Runnable: serial CPU speculative profile | Standard acceptance/correction, block bonus, whole-cache rollback, prompt n-grams and original greedy parity. |
| [48. Call a tool](ch048-call-a-tool.md) | Runnable: finite JSON-schema enum and granted tools | Mask before sampling, typed commentary handoffs, tool-role results and final-only display. |
| [56. Execute a DeepSeek R1 distill](ch056-execute-a-deepseek-r1-distill.md) | Runnable: original BF16 CPU reference | Qwen2 execution, original tokenizer BOS and stripped-history recipe, greedy generation, separate reasoning/final intervals. |
| [165. A machine whose state is a trace](ch165-a-machine-whose-state-is-a-trace.md) | Runnable: bounded CPU reference interpreter | Admit and run a small integer stack machine whose four-token-per-step trace and SHA-256 digest identify the run; a refused run publishes nothing. |
| [166. Attention as exact memory](ch166-attention-as-exact-memory.md) | Runnable: bounded CPU, integer scores | Parabolic keys and hard-max reads return exactly the latest write or refuse; the machine with attention as its only memory reproduces the interpreter's digest. |
| [172. From hard-max to softmax](ch172-from-hard-max-to-softmax.md) | Runnable: bounded CPU, binary64/binary32/bfloat16 rounding | Prove softmax reads stay exact for any trace length once β ≥ ln(4V), and measure where binary32 and bfloat16 rounding breaks them. |

Chapters 41–48 start an independent gpt-oss branch, and 165, 166 and 172 open Part XXIII (programs compiled into weights); chapter 11 and chapters 13–40 remain planned.

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
