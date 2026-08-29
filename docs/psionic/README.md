# Psionic in the OpenAgents CLI

This directory is the owner-accepted ledger for bringing a **narrow local
inference slice** of Psionic into this repository, linking it into the
`openagents` binary, and replacing Ollama as Coder's local Qwen 3.8 runtime
once the gates in the plan pass.

| Document | Role |
| --- | --- |
| [INTENT.md](./INTENT.md) | What the owner asked for, and what this program does not include |
| [PLAN.md](./PLAN.md) | First implementation plan, crate allowlist, CLI surface, and stages |
| [QWEN38_INFERENCE_PIPELINE.md](./QWEN38_INFERENCE_PIPELINE.md) | Weights-to-token walkthrough: tensors, quantization, load, decode |

Related older writing stays where it is. It is evidence, not this program's
authority:

- [Coder sidecar analysis (2026-08-26)](../coder/2026-08-26-psionic-local-inference-integration-analysis.md)
- [Qwen 3.8 / Ollama audit (2026-08-28)](../2026-08-28-qwen38-and-ollama-audit.md)
- [Coder v0.2.0 scope](../coder/2026-08-28-coder-v0.2.0-scope.md) (Psionic wiring remains out of that CLI release)
- Cloud file adapter: [oa-node Psionic workers](../cloud/oa-node/PSIONIC_WORKERS.md)

The 2026-07-08 retirement of the Tassadar training-program docs does not
block this CLI inference slice. That retirement parked distributed training,
gym, and executor research. This folder is a new, bounded product program.

Implementation code has not landed. Do not treat this folder as proof that
the binary already contains Psionic.
