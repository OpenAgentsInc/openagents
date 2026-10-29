# nanochat as an inference and training target

Read-only survey, 2026-10-28, pinned to
[`92d63d4e8bb4df75c3b71618f31ddde2378b2bcd`](https://github.com/karpathy/nanochat/tree/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd).
No upstream setup, training or checkpoint-loading code was run. Psionic will
implement the relevant behavior in Coax, with Python permitted for reviewed
reference/export tooling and never required by the production runtime.

nanochat is useful here as a concrete integration target for a complete small
language-model pipeline. Its pieces fit after shared numerical operations,
a complete causal decoder and gradients. Its training program is not a
prerequisite for [early Elephant/Mammoth inference](model-targets.md).

## What the pinned code actually does

| Source | Observation | Coax consequence |
| --- | --- | --- |
| [runs/speedrun.sh](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/runs/speedrun.sh) | Trains/evaluates a tokenizer, pretrains/evaluates a base model, then runs SFT and chat evaluation. It targets an eight-GPU node. | Reproduce a tiny single-device program first. Upstream scale, timing and cost are not Psionic promises or authorized budgets. |
| [nanochat/gpt.py](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/nanochat/gpt.py) | RoPE, QK normalization/scaling, GQA, ReLU-squared MLP, untied head, window patterns, value embeddings/gates, residual/x0 scalars, smear and backout. | Pin the entire enabled architecture. A generic GPT block is not checkpoint compatibility. Causal attention remains a distinct profile from Base Labs' mask. |
| [nanochat/tokenizer.py](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/nanochat/tokenizer.py) | Trains BPE using rustbpe and runs tiktoken; a custom pretokenization pattern uses two-digit number groups. Conversation/tool tokens have a defined order. | Reuse generic byte/BPE components from the Qwen work, with separate pattern, merges and special-token identities. Implement inference and later small deterministic training in Coax. |
| [nanochat/checkpoint_manager.py](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/nanochat/checkpoint_manager.py) | Saves model/optimizer `.pt` files and JSON metadata, with compatibility defaults and rank-specific optimizer state. The tokenizer uses `tokenizer.pkl`. | Define an inspectable inert tensor/config/tokenizer export; preserve source hashes and explicit version readers. Do not execute pickle as a production parser. |
| [nanochat/optim.py](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/nanochat/optim.py) | A combined MuonAdamW optimizer treats matrix and embedding/scalar groups differently; the pinned implementation includes specific polar-iteration and variance-reduction behavior. | Teach and validate SGD, then AdamW, then the exact pinned Muon/grouping profile. State resume and independent update vectors precede a recipe-parity claim. |
| [scripts/base_train.py](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/scripts/base_train.py) and [dataloader.py](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/nanochat/dataloader.py) | The loader uses BOS-aligned best-fit cropping and describes its resume as approximate; training owns batch/accumulation, schedule and model configuration. | Record document/cropping and consumed-token semantics. Psionic's exact-resume contract is a deliberate stronger requirement, not an observed upstream guarantee. |
| [scripts/chat_sft.py](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/scripts/chat_sft.py) | Supervised conversation training is a separate stage from base pretraining. | Validate conversation rendering and assistant/tool loss masks, held-out evaluation and base-skill regression before promotion. |
| [scripts/chat_rl.py](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/scripts/chat_rl.py) | Describes a simplified on-policy, GSM8K-oriented policy-gradient recipe with mean-centered rewards and token-level normalization; it removes KL trust-region and PPO ratio/clipping. | Implement that specific recipe after SFT/evaluation. Do not label it general GRPO, PPO or RLHF support. |

The current speedrun has no separate mid-training command and does not invoke
the separate RL script. Older walkthroughs can describe different stages.
Continued/domain pretraining, preference optimization, distillation and broader
RLHF remain useful later options, each requiring its own objective/data/eval
contract. They are not inherited from a successful SFT run.

## Logical implementation sequence

1. **Shared core and urgent decisions.** Finish storage and dense operations,
   safetensors/BF16, exact tokenization, and the Elephant/Mammoth forward path.
   CPU reference and actual generated device kernels develop independently
   of generic backward support.
2. **Learning foundations.** Bounded autodiff/SGD, byte-level likelihood and
   sampling, then a complete causal decoder with checked gradients and KV
   inference. Preserve the small training example throughout.
3. **nanochat inference profile.** Implement the pinned tokenizer, all enabled
   `gpt.py` features and a safe checkpoint export. Establish independent logits,
   cached/full equality and actual multi-token generation before training it.
   Then check gradients for every extra enabled feature; forward parity alone
   does not establish that the model can be trained.
4. **State, optimizers and evaluation.** Deterministic dataset packing and
   resume; AdamW followed by the precise MuonAdamW grouping/update rules.
   Evaluate language loss/bits-per-byte and task outcomes on frozen held-out
   data. Decision evaluation also measures probabilities/calibration/abstention.
5. **Small pretraining program.** Train the named architecture end to end in
   Coax on a tiny licensed corpus with an explicit compute envelope. Then
   scale one target at a time using measured tokens, memory, throughput and
   quality. A tiny demonstration and upstream speedrun reproduction are
   separate milestones.
6. **SFT, then reward training.** Assistant/tool-token loss masks, mixture
   provenance, held-out chat evaluation and resume come before rollouts and
   policy gradients. Bound generation, reward evaluation and sample budgets.
7. **Selected scale and other post-training.** One actual homogeneous
   multi-device target, data/gradient ownership and collective recovery; later
   broader training modes. Decision-model fine-tuning can branch after its
   own hybrid backward pass and shared optimizer/eval/data foundations,
   without waiting for nanochat RL.

For compatibility, keep four claims separate: model forward execution,
checkpoint/tokenizer import, optimizer/update behavior, and whole-pipeline
reproduction. Each has a different independent oracle and acceptance gate.

The [book plan](../../book/PLAN.md) gives the reading order. The
[OpenAgents issue seed](../../../apps/openagents-web/projects/psionic/work/roadmap.json)
gives implementation dependencies and completion checks. Their ordering is
intentional: urgent original-weight inference can ship while the teaching
chapters for gradients and training are still being written.
