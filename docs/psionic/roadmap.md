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

## Delivery priority: Elephant, then Mammoth

The owner priority is [native Elephant and Mammoth](model-targets.md), with
shared foundations reused by the later [nanochat pipeline](nanochat.md).
[The live Psionic project](https://openagents.com/projects/psionic) contains
the dependency graph, acceptance checks, and stable issue slugs. Its source
is [work/roadmap.json](../../apps/openagents-web/projects/psionic/work/roadmap.json).
Future work is open; M0 alone is recorded complete.

| Priority | Work | Must finish first |
| --- | --- | --- |
| Now | Pin both model bundles/nanochat, reference contracts and license/provenance; admit shapes/storage. | M0 is shipped. Contract research and shapes are ready independently. |
| Next | Dense operations, safetensors/BF16, exact Qwen tokenizer/decision prompts, independent reference capture. | Each task's data/math prerequisites; no training subsystem. |
| Next | Qwen3.5 dense blocks, block-bidirectional full attention, causal DeltaNet, complete prefix forks and slot readout. | Shared operations/artifacts and exact prompt/state fixtures. |
| First real model | Complete original-weight Elephant text decisions. | All hybrid layers, exact renderer/readout, bounded state and independent full-model oracles. |
| Immediately after | Complete original-weight Mammoth using the same engine. | Elephant implementation, Mammoth-specific dimensions, shards, memory and real-weight evidence. |
| Alongside this path | Coax compute consumer, generated Metal kernels, Elephant then Mammoth device qualification. | Concrete shapes/operations and each model's CPU numerical reference; no generic training dependency. |
| After original-weight baselines | One separately qualified quantized profile and shared System One serving integration. | Full-model numerical evidence and explicit artifact/profile/contract boundaries. |

CPU reference correctness and device execution can advance together. Do not
wait for a fast full-model CPU service or finish the entire teaching spine
before starting compute support. Current scalar Metal emission is insufficient
for these kernels. Reference Python/OneJev is tooling; native model support
requires actual complete Coax execution.

Smaller synthetic fixtures and source-contract work need no paid hardware.
A large-weight reference/target run records its actual machine, memory and
compute admission when undertaken. No budget, deployment or model download is
a prerequisite for starting the ready work.

## The cumulative library and book

Milestone names remain the library's coverage map; their number is not delivery
priority. [The book plan](../../psionic/book/PLAN.md) is the current reading
order, including the early real-model case study and later nanochat chapters.

| Milestone | Dependencies | Result and completion gate |
| --- | --- | --- |
| M0: learning a number — implemented | Current CPU subset | `psionic/check.sh`: exact first update, bounded training/held-out result, native refusals/aliases, sanitizers, listings and transcripts. |
| M1: shapes and dense numerics | M0 | Bounded storage, dot/matmul/gather/reductions/normalization with independent oracles and per-operation numerics. Chapters 2–3. |
| Early decision path | M1 plus pinned artifacts/tokenizer and the hybrid forward architecture | Original-weight Elephant, then Mammoth, text first. Independent masks, prefix-state forks, slot probabilities and complete real-weight requests. Chapters 10–14 can ship before chapters 4–9. |
| M2: arrays, autodiff and SGD | M1 | A tiny classifier, analytic/finite-difference gradients, bounded tape/state and reproduction of the manual trainer. Chapters 4–5. |
| M3: byte language model | M2 | A real byte vocabulary, fixed licensed splits, likelihood, named RNG and complete small training/generation. Chapters 6–7. |
| M4: complete causal decoder | M3 | Causal attention/FFN, full gradients, full-versus-cached logits, causal isolation and a complete tiny training example. Chapters 8–9. |
| M5: artifacts and resumable state | Forward artifact work starts after M1; optimizer/data-state work after M2 | Bounded safetensors/BF16 and exact model/tokenizer identity early; deterministic packing, RNG/optimizer/data-state resume later. Weight import does not depend on completing generic training. Chapters 10 and 19. |
| M6: measured CPU behavior | Every concrete operation/workload as it appears | Fixed numerical and memory/latency observations before optimization; raw samples and honest failed budgets. Chapter 15. |
| M7: native device execution | M1 operations and the explicit Coax compute milestone | Generated kernels actually run on a named device with CPU numerical oracles, bindings, limits and completion/lifetime checks. Decision inference is the first consumer. Chapter 16. |
| M8: quantization and local inference | Original-weight model reference; M7 for device claims | Independent block vectors, partial-row refusal, separate identities and measured held-out numerical/quality/memory effects. Chapter 17. |
| nanochat inference and gradients | Shared artifacts/tokenizer plus M4 | The pinned complete architecture, inert checkpoint export, exact tokenization, cached inference and every enabled feature's gradients. Chapter 18. |
| nanochat pretraining | Its forward/backward profile, data/resume, exact AdamW/Muon groups and held-out checks | A tiny complete deterministic run first; measured one-device scale later. Upstream speedrun cost/speed is not inherited. Chapters 19–21. |
| SFT and reward training | Pretraining, masked conversation data, independent evaluation and rollout/reward contracts | SFT first, then the specific pinned nanochat policy-gradient recipe; separate promotion/regression evidence. Chapters 22–23. |
| Decision post-training | Qualified hybrid forward/backward plus shared optimizer/data/evaluation | Bounded trainable regions/adapters, cross-entropy/Brier/abstention, resume and held-out promotion; it need not wait for nanochat RL. Chapter 24. |
| M9: serving, vision and distributed work | Qualified local models and explicit consumer/target contracts | Shared System One integration; separately qualified image handling; one actual homogeneous multi-device training topology. Chapters 25–26 extend integration topics; broader APIs and scale remain explicit follow-ups. |

## What support means

Each model/backend/profile earns its own evidence. Metadata or a synthetic
layer test means partial implementation, not full real-weight support.
Elephant does not qualify Mammoth, and text support does not qualify vision.
Original BF16 file size does not establish f32-expanded residency or available
GPU memory. Quantized outputs get their own approximation/quality gates.

For nanochat, distinguish forward/checkpoint compatibility, tokenizer behavior,
all enabled gradients, optimizer updates, whole pretraining, SFT and RL. The
pinned speedrun has no standalone mid-training stage and runs SFT after base
training; RL is separate. An arbitrary tiny GPT or a Python wrapper cannot
close the native nanochat issues.

Keep M0 and the tiny complete decoder as cumulative regressions. Write each
chapter from its running library result, and keep incomplete material labelled
planned. A missing device or reference artifact does not turn an implementation
failure into a pass; record the concrete missing action and keep useful local
work moving.
