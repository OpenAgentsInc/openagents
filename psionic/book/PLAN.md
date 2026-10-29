# Reading order and delivery order

Only [Chapter 1](ch01-learning-a-number.md) is runnable today. The chapters
below are planned. Each will ship with shared-library code, authored checks,
independent native tests, source-linked examples and an exact supported scope.
This contents plan does not create implemented features.

The owner wants Elephant and Mammoth supported early. **Delivery priority is
foundations → native Elephant → native Mammoth**, with generated GPU compute
advancing alongside them. The training curriculum does not block that path.
The [live project](https://openagents.com/projects/psionic) carries the actual
issue dependencies; [the roadmap](../../docs/psionic/roadmap.md) explains them.

## The learning spine

| Chapters | Planned content | Cumulative result |
| --- | --- | --- |
| 1. Learning a number | Existing scalar model, loss, analytic gradient, fixed-rate updates, held-out prediction. | Runnable today. |
| 2. Shapes and storage | Bounded dimensions/counts, caller ownership, scratch, aliases and refusal. | A safe numerical API boundary on the tested target. |
| 3. Numbers and matrices | Dot/matmul, gathers, stable reductions/softmax and normalization; numerical contracts. | The shared CPU operation library. |
| 4. Learning with arrays; 5. Gradients as a program | Small classifier, then bounded reverse-mode autodiff and SGD. | Manual and automatic gradients agree. |
| 6. Bytes and predictions; 7. A small language model | A real byte vocabulary, likelihood, seeded sampling and a bigram trainer. | Complete tiny language-model training and generation. |
| 8. Attention; 9. A complete tiny decoder | Causal attention, position convention, FFN, full gradients and KV state. | A complete small transformer, not a head-only demonstration. |

## Early real-model case study

This section depends on chapters 2–3 and its own forward-architecture primers.
It can be implemented and published before chapters 4–9 are finished. Readers
interested in using open weights can follow **1 → 2 → 3 → 10–14**; they do not
need to implement training first.

| Chapters | Planned content | Completion evidence |
| --- | --- | --- |
| 10. Reading original weights | Bounded safetensors/shard reader, BF16, manifests and tokenizer identity. | Malformed artifacts refuse; independent byte/tensor vectors. |
| 11. A hybrid forward pass | Qwen3.5 dense blocks, full attention and causal Gated DeltaNet/convolution. | Every component and full layer matches independent intermediate values. |
| 12. Decisions have a different mask | Exact Base Labs state/question rendering, block-bidirectional full attention, causal recurrence. | Exact token/mask fixtures and branch-isolation checks. |
| 13. Reuse a prefix, read the options | Complete hybrid snapshots, sequential suffix forks, slot LM-head readout, typed answers. | Independent probability/result vectors and no cross-question leakage. |
| 14. Elephant, then Mammoth | The complete original-weight models, text-only first, distinct memory/numerical profiles. | Each real checkpoint passes its own full-model numerical and request gates. |

The implementation of chapter 14 is the urgent product goal. CPU is the
reference, while device work begins from a concrete kernel consumer as soon
as the shared core exists. These are independent of training chapter numbers.
Vision, quantization and hosted serving each require additional qualification.

## Execution and the nanochat pipeline

| Chapters | Planned content | Dependencies and outcome |
| --- | --- | --- |
| 15. Measure the work; 16. Coax tensor compute | Fixed numerical oracles, memory/latency observations, the missing compute subset and actual Metal dispatch. | Begins after the shared core; does not wait for the training curriculum or complete CPU throughput optimization. |
| 17. Quantized weights | One exact block format and separately identified approximate profiles. | Original-weight numerical baselines first, then memory/error/quality measurements. |
| 18. Execute nanochat | Pinned BPE/conversation rules, all enabled model features, safe checkpoint export, cached inference. | Tiny causal decoder and shared artifact/tokenizer work. A generic GPT implementation alone is insufficient. |
| 19. Data and resumable state | Provenance, splits, document packing, RNG/data counters, model/optimizer checkpoints. | Begin once learning has state; resume matches an uninterrupted run under a named profile. |
| 20. AdamW, then Muon | Independent update derivations, parameter groups and precision; exact pinned nanochat optimizer behavior. | Shared operations and gradients; no optimizer parity from plain SGD. |
| 21. A complete pretraining run | Initialization, scheduling, accumulation, token budgets and held-out loss/bits-per-byte. | nanochat forward/backward, tokenizer, data/resume and optimizer gates. Start tiny, then measure scale. |
| 22. Supervised conversations | Assistant/tool loss masks, task mixtures, validation and base-skill regression. | Complete base training and evaluation. |
| 23. Learn from rewards | Bounded rollouts, mean-centered rewards and the pinned nanochat policy-gradient recipe. | SFT and independent reward/gradient/held-out checks; broader RLHF remains separate. |
| 24. Post-train a decision model | Hybrid backward pass, selected trainable regions/adapters, cross-entropy/Brier/abstention and promotion. | Qualified decision forward path plus shared gradients/optimizer/data/eval; no dependency on nanochat RL. |
| 25. Images; 26. Distributed training | Explicit vision preprocessing/encoder profiles; one homogeneous multi-device training topology. | Actual model/target/topology evidence, not an inferred capability from checkpoints or APIs. |

The nanochat study is pinned in [docs/psionic/nanochat.md](../../docs/psionic/nanochat.md).
Its current speedrun goes from tokenizer/base training to SFT; RL is a separate
program and a distinct mid-training phase is not present in that script.
A future continued-pretraining or preference-optimization chapter needs its
own concrete workload and gates.

Chapter numbers describe the explanation. Stable issue slugs describe work.
Readiness and priority come from the signed queue and dependency graph, so an
urgent forward-only model does not wait for an unrelated book part to finish.
