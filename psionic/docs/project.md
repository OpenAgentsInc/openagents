# Psionic project and issue map

[Open the project](https://openagents.com/projects/psionic). Created 2026-10-28 as a public signed OpenAgents project: 38 tasks, six milestones and one initiative. The scalar-model baseline is resolved; the other 37 tasks are open. Readiness is recomputed from signed statuses, not this snapshot.

Public project identity: `30621:7fa6eb9366eead55197f55259f8702acc1656bd4663de6e474cbbb3dc9fbd3fa:psionic`.

The source of issue bodies, acceptance and dependencies is [the work seed](../../apps/openagents-web/projects/psionic/work/roadmap.json). Slugs and event IDs are stable; use the publisher for revisions and queue moves. No GitHub issues or external contributions were created.

## Start here

- [Pin the model and oracle contracts](https://openagents.com/projects/psionic#5009).
- [Implement bounded tensor shapes](https://openagents.com/projects/psionic#0bfb).

The [delivery roadmap](roadmap.md) prioritizes Elephant and Mammoth independently of generic training. [The book plan](../book/PLAN.md) distinguishes teaching order. [The model target note](targets/model-targets.md) and [nanochat survey](targets/nanochat.md) pin the sources.

## Tasks

| Issue | Stable slug | Depends on |
| --- | --- | --- |
| [Learn y = 2x with the first runnable chapter](https://openagents.com/projects/psionic#bbde) | `psionic-tiny-model` | Completed baseline |
| [Pin Elephant, Mammoth and nanochat contracts and oracle manifests](https://openagents.com/projects/psionic#5009) | `psionic-model-pins` | Ready |
| [Admit bounded tensor shapes and caller-owned storage](https://openagents.com/projects/psionic#0bfb) | `psionic-shapes` | `psionic-tiny-model` |
| [Build dense CPU operations with independent numerical oracles](https://openagents.com/projects/psionic#3047) | `psionic-dense-ops` | `psionic-shapes` |
| [Read sharded safetensors and BF16 in Coax](https://openagents.com/projects/psionic#cfeb) | `psionic-safetensors` | `psionic-shapes`, `psionic-model-pins` |
| [Implement exact Qwen3.5 tokenization for the decision models](https://openagents.com/projects/psionic#eea1) | `psionic-qwen-tokenizer` | `psionic-model-pins` |
| [Render exact Base Labs state, questions and option slots](https://openagents.com/projects/psionic#7ade) | `psionic-decision-prompts` | `psionic-qwen-tokenizer` |
| [Implement Qwen3.5 embeddings, projections, normalization and FFN](https://openagents.com/projects/psionic#007c) | `psionic-qwen-layers` | `psionic-dense-ops`, `psionic-safetensors` |
| [Implement block-bidirectional Qwen full attention](https://openagents.com/projects/psionic#d8bb) | `psionic-full-attention` | `psionic-qwen-layers` |
| [Implement causal Gated DeltaNet and convolution state](https://openagents.com/projects/psionic#19c4) | `psionic-deltanet` | `psionic-qwen-layers` |
| [Fork complete hybrid prefix state without cross-question leakage](https://openagents.com/projects/psionic#4dc8) | `psionic-prefix-forks` | `psionic-full-attention`, `psionic-deltanet`, `psionic-decision-prompts` |
| [Implement slot_lm probabilities and typed decision answers](https://openagents.com/projects/psionic#de9c) | `psionic-slot-readout` | `psionic-decision-prompts`, `psionic-dense-ops` |
| [Capture independent Elephant and Mammoth reference cases](https://openagents.com/projects/psionic#b47f) | `psionic-decision-oracles` | `psionic-model-pins` |
| [Run complete Elephant text decisions with the Coax CPU engine](https://openagents.com/projects/psionic#261e) | `psionic-elephant-cpu` | `psionic-safetensors`, `psionic-qwen-layers`, `psionic-full-attention`, `psionic-deltanet`, `psionic-prefix-forks`, `psionic-slot-readout`, `psionic-decision-oracles` |
| [Run complete Mammoth immediately after Elephant](https://openagents.com/projects/psionic#13d8) | `psionic-mammoth-cpu` | `psionic-elephant-cpu` |
| [Specify the minimum Coax tensor-compute consumer](https://openagents.com/projects/psionic#b301) | `psionic-compute-contract` | `psionic-shapes` |
| [Implement and qualify the first generated Metal tensor kernels](https://openagents.com/projects/psionic#8484) | `psionic-metal-compute` | `psionic-compute-contract`, `psionic-dense-ops` |
| [Qualify original-weight Elephant on Metal](https://openagents.com/projects/psionic#a0bb) | `psionic-elephant-metal` | `psionic-elephant-cpu`, `psionic-metal-compute` |
| [Qualify Mammoth memory and native Metal execution](https://openagents.com/projects/psionic#07a2) | `psionic-mammoth-metal` | `psionic-mammoth-cpu`, `psionic-elephant-metal` |
| [Add one measured quantized decision-model profile](https://openagents.com/projects/psionic#c6a4) | `psionic-quantization` | `psionic-elephant-cpu`, `psionic-mammoth-cpu` |
| [Add bounded reverse-mode differentiation over shared operations](https://openagents.com/projects/psionic#4a75) | `psionic-autodiff` | `psionic-dense-ops` |
| [Train a complete byte-level bigram language model](https://openagents.com/projects/psionic#3a0e) | `psionic-byte-lm` | `psionic-autodiff` |
| [Build a tiny complete causal decoder and its backward path](https://openagents.com/projects/psionic#e06a) | `psionic-causal-decoder` | `psionic-byte-lm` |
| [Implement pinned nanochat BPE and conversation tokens](https://openagents.com/projects/psionic#cc58) | `psionic-nanochat-tokenizer` | `psionic-qwen-tokenizer` |
| [Execute the pinned nanochat architecture and safe checkpoint export](https://openagents.com/projects/psionic#9b3d) | `psionic-nanochat-forward` | `psionic-causal-decoder`, `psionic-nanochat-tokenizer`, `psionic-safetensors` |
| [Check gradients for every enabled nanochat model feature](https://openagents.com/projects/psionic#191b) | `psionic-nanochat-backward` | `psionic-nanochat-forward`, `psionic-autodiff` |
| [Implement reproducible dataset packing and checkpoint resume](https://openagents.com/projects/psionic#039b) | `psionic-resume-data` | `psionic-autodiff` |
| [Implement checked AdamW and explicit precision state](https://openagents.com/projects/psionic#5d0b) | `psionic-adamw` | `psionic-autodiff` |
| [Reproduce nanochat MuonAdamW parameter groups and updates](https://openagents.com/projects/psionic#68b5) | `psionic-muon` | `psionic-adamw`, `psionic-dense-ops` |
| [Build held-out language and decision evaluation records](https://openagents.com/projects/psionic#fd99) | `psionic-evaluation` | `psionic-resume-data`, `psionic-nanochat-forward`, `psionic-elephant-cpu` |
| [Train a small nanochat profile end to end in Coax](https://openagents.com/projects/psionic#a25a) | `psionic-nanochat-pretrain` | `psionic-nanochat-forward`, `psionic-resume-data`, `psionic-muon`, `psionic-nanochat-backward` |
| [Add nanochat-style masked supervised conversation training](https://openagents.com/projects/psionic#7c86) | `psionic-nanochat-sft` | `psionic-nanochat-pretrain`, `psionic-evaluation` |
| [Implement the pinned nanochat reward-training recipe](https://openagents.com/projects/psionic#72d9) | `psionic-nanochat-rl` | `psionic-nanochat-sft`, `psionic-evaluation` |
| [Differentiate the decision backbone and masked slot loss](https://openagents.com/projects/psionic#1f17) | `psionic-decision-backward` | `psionic-elephant-cpu`, `psionic-autodiff` |
| [Fine-tune and calibrate a bounded Elephant decision profile](https://openagents.com/projects/psionic#3d9d) | `psionic-decision-posttrain` | `psionic-decision-backward`, `psionic-adamw`, `psionic-resume-data`, `psionic-evaluation` |
| [Expose qualified native decisions through the shared System One boundary](https://openagents.com/projects/psionic#c7df) | `psionic-systemone-serving` | `psionic-elephant-cpu`, `psionic-mammoth-cpu` |
| [Add separately qualified image inputs for Elephant and Mammoth](https://openagents.com/projects/psionic#04e3) | `psionic-decision-vision` | `psionic-mammoth-cpu` |
| [Add one explicit same-node distributed training topology](https://openagents.com/projects/psionic#1da2) | `psionic-distributed-training` | `psionic-nanochat-pretrain`, `psionic-resume-data`, `psionic-metal-compute` |

## Maintaining the queue

From the repository root, use the trusted publisher and snapshot tools:

```sh
bun coax/web/project/publish.ts list --project psionic
bun coax/web/project/publish.ts publish --project psionic --dry-run
bun coax/web/project/publish.ts publish --project psionic
bun coax/web/project/snapshot.ts --project psionic
```

The project signing key stays in its existing per-project secret-file convention outside the repository, with private filesystem permissions. Never print or commit it. The public identity alone is in project.json. [QUEUE.md](../../coax/web/project/QUEUE.md) describes claims, plans, completion evidence and release. Current workers commit and push main; imported historical PR instructions do not override the agent contract.

The M0 baseline links to [its verified pushed commit](https://github.com/OpenAgentsInc/openagents/commit/38eab9b9fe0326c333325ff5a05df400de1dd0e2) and [verification](../../docs/verification/psionic-m0-2026-10-28.md). No future model or training task is marked complete by publishing this plan.

The [expanded proposed contents](../book/PLAN.md) and
[NVIDIA breadth survey](targets/nvidia-breadth.md) extend the planning horizon. Their
new studies have not been added to this signed queue; the existing task slugs
and dependencies remain the issue map for the work already published.
