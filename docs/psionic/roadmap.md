# Starting point and roadmap

Status: M0 implemented on the tested Apple LP64 CPU host; M1–M9 remain
`planned`. [Run the first chapter](../../psionic/book/ch01-learning-a-number.md)
or `psionic/check.sh`; [retained verification](../verification/psionic-m0-2026-10-28.md)
records the scope. Later chapter titles are working names; gates determine
completion, not a calendar or page count.

## M0: learn one number

The first [shared library](../../psionic/lib/learn.coax) and book chapter
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

## Cumulative chapter sequence

Each row extends the same library and runnable examples. A milestone may span
several small commits; each accepted chapter includes its own evidence and
the relevant earlier regressions.

| Milestone / chapters | Depends on | Working result | Completion gate |
| --- | --- | --- | --- |
| M0 / 1. Learning a number — implemented | Current CPU subset | The one-parameter trainer above; shared library, book, and example verification. | `psionic/check.sh`: independent first update and convergence, bounded training, native refusals/aliases, sanitizers, exact listings/transcripts. |
| M1 / 2. Shapes and storage; 3. Numbers and matrices | M0 | Shared contiguous `f32` operations: indexing, dot, matrix multiply, addition, reductions, stable softmax and normalization as needed. | Overflow/capacity/alias refusals; independent tiny matrix and reduction oracles; per-op error contracts; M0 uses shared operations where applicable. |
| M2 / 4. Learning with arrays; 5. Gradients as a program | M1 | A tiny linear classifier, then bounded reverse-mode autodiff and SGD over shared ops. | Analytic and finite-difference gradients; accumulation/detach/tape-limit tests; autodiff reproduces the manual example and learns on held-out cases. |
| M3 / 6. Bytes and predictions; 7. A small language model | M2 | A 256-byte vocabulary and a trainable bigram model, with explicit token IDs and seeded sampling. | Fixed split/corpus/license; probability/loss checks, held-out loss against a declared baseline, exact RNG replay within the profile. Bytes are the actual vocabulary, not a placeholder BPE implementation. |
| M4 / 8. Attention; 9. A complete tiny decoder | M3 | Causal attention, embeddings, position convention, feed-forward block, forward/backward execution, and a bounded KV cache. | Full-sequence versus incremental-decode logits; causal isolation; checked gradients; training and generation of the entire tiny model. No head-only stand-in. |
| M5 / 10. Saving and resuming; 11. Reading a real model | M4 | Parameter/optimizer/RNG/data-state persistence; one pinned external format, tokenizer, template, and small decoder family. | Resume matches uninterrupted training within its declared profile; malformed artifacts fail; independent token IDs, intermediate tensors, logits, and multi-token generation for the complete imported model. |
| M6 / 12. Where the time and memory go | M4; M5 for imported-model claims | Measured CPU baseline, layout/view extensions justified by workloads, then focused CPU optimizations. | Fixed workload and tolerance before optimization; native debug/release and sanitizer regressions; raw cold/warm timings, allocation/scratch evidence, and honest failed budgets. |
| M7 / 13. The same computation on Metal | M6 plus compiler compute milestone | One bounded device operation, then the tiny model on actual Metal buffers. | Real device dispatch and readback, CPU numerical comparison, resource limits, command failures and completion/lifetime checks. Scalar shader compilation alone cannot pass. |
| M8 / 14. Quantized weights and useful local inference | M5, M6; M7 for GPU claims | One exact published quantization layout, a complete admitted model, and reusable local inference API. | Independent block vectors, incomplete-block refusal, memory/quality/error measurements; backend/profile identity; actual complete answers and cancellation behavior. |
| M9 / 15+. Serving, data pipelines, and distributed work | Demonstrated local use; explicit contracts | Bounded serving, richer training/data/eval, adapters and selected multi-device/distributed capabilities. | Separate framework, serving, training, device, and topology acceptance. Root protocol changes precede cross-surface integration. |

The main path is M0 → M1 → M2 → M3 → M4. CPU work continues while a concrete
M7 compute case informs compiler development. M5 checkpoint work can begin
as soon as M2 has optimizer state; the table's M5 gate also includes the later
complete-model import. M6 measurement habits start at M0 even though sustained
optimization comes later.

## Bound the first imported model

Choose the artifact when M5 starts, using the devices and budgets then
available. Prefer a small dense decoder whose full forward pass fits in local
memory and whose tokenizer and intermediate outputs can be independently
checked. Record the exact revision, license, hashes, architecture, dtype,
context, required operations, memory estimate, and oracle before implementing
its reader. An existing small-model Psionic fixture can be useful evidence.

GPT-OSS, MoE routing, large DeepSeek variants, and distributed expert offload
are later targets. The ML book and Psionic history supply worked questions
for them, but their complexity should not set the minimum first chapter.
Do not select a marketing model name as a substitute for an executable
architecture and artifact contract.

## Completion and scope decisions

After M0, assess whether a new reader can reproduce and explain every update.
After M2, assess whether this is becoming a reusable learning library rather
than a collection of model-specific loops. After M4, keep the tiny full model
as the cumulative regression when real weights or device kernels arrive.

Later work should be pulled by a complete workload: a missing tensor
operation, an observed CPU cost, a model the product needs, or a measured
device limit. Broad PyTorch/MLX API parity, a general lazy graph compiler,
multi-vendor GPU support, and public compute orchestration each require a
separate scoped decision. They are not prerequisites for this book's opening.
