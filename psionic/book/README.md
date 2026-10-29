# Building Psionic in Coax

This book builds a machine-learning library from small understandable pieces.
Every completed chapter runs the actual [Psionic library](../README.md), with
checks and independently specified results.

[Chapter 1](ch01-learning-a-number.md) starts with learning a single number.
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
| [1. Learning a number](ch01-learning-a-number.md) | Runnable: CPU | Train `prediction = weight * x`, check every loss, predict a held-out value, and refuse malformed work. |
| [2. Shapes and storage](ch02-shapes-and-storage.md) | Runnable: CPU | Admit rank-zero through rank-four shapes, count bytes safely, locate values and stage a copy into caller storage. |
| [3. Numbers and matrices](ch03-numbers-and-matrices.md) | Runnable: CPU-f32-wide-v1 | Add, reduce, multiply matrices, gather rows, apply masked softmax and normalize rows with independent numerical checks. |
| [4. Reading original weights](ch04-reading-original-weights.md) | Runnable: original-weights-v1 | Inspect and bind original single/sharded BF16/F32 artifacts, decode finite windows and use imported values in shared kernels. |
| [5. Text becomes a model request](ch05-text-becomes-a-model-request.md) | Runnable: qwen35-text-v1 | Render typed decisions, normalize/tokenize original Qwen data and validate complete shared-prefix/suffix layouts. |
| [6. Attention without training](ch06-attention-without-training.md) | Runnable: bounded CPU | Execute a dense causal decoder block with grouped-query attention, rotary positions and a key/value cache. |
| [7. A hybrid sequence model](ch07-a-hybrid-sequence-model.md) | Runnable: bounded CPU | Compose Qwen-style causal linear and full-attention layers with their separate states. |
| [8. Decisions have a different mask](ch08-decisions-have-a-different-mask.md) | Runnable: bounded CPU | Compute bidirectional full-attention blocks and fork all live hybrid prefix state. |
| [9. Read the options](ch09-read-the-options.md) | Runnable: bounded CPU | Compute temperature-scaled option probabilities, vocabulary slot mass and typed decision statistics. |
| [10. Execute Elephant](ch10-execute-elephant.md) | Runnable: original weights, CPU | Authenticate Elephant and execute a typed text decision through its complete hybrid stack. |

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
