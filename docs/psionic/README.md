# Psionic in the OpenAgents CLI

This directory is the owner-accepted ledger for bringing a **narrow local
inference slice** of Psionic into this repository, linking it into the
`openagents` binary, and replacing Ollama as Coder's local Qwen 3.8 runtime
once the gates in the plan pass.

| Document | Role |
| --- | --- |
| [INTENT.md](./INTENT.md) | What the owner asked for, and what this program does not include |
| [PLAN.md](./PLAN.md) | Implementation plan, crate allowlist, CLI surface, and stages |
| [CLI.md](./CLI.md) | Commands, teach mode, and the status lines from GGUF lookup through a generated token |
| [PARITY.md](./PARITY.md) | How we compare quality and tok/s to the Ollama holdout |
| [QWEN38_INFERENCE_PIPELINE.md](./QWEN38_INFERENCE_PIPELINE.md) | Weights-to-token walkthrough: tensors, quantization, load, decode |
| [OLLAMA_INFERENCE_PIPELINE.md](./OLLAMA_INFERENCE_PIPELINE.md) | How Ollama loads weights and generates a token, GGUF and MLX |
| [LLAMA_CPP_INFERENCE_PIPELINE.md](./LLAMA_CPP_INFERENCE_PIPELINE.md) | Lowest-level llama.cpp GGUF parse, mmap, Metal mapping, graph, Q8_0 |
| [PROVENANCE.md](./PROVENANCE.md) | What we studied vs copied for the in-tree GGUF crate |

OpenAgents issues (`openagents issue`; GitHub issues remain for reproducible bugs):

Closed through first generate:

- https://openagents.com/OpenAgentsInc/openagents/issues/344 — `map.done`
- https://openagents.com/OpenAgentsInc/openagents/issues/345 — Coder `/load` progress
- https://openagents.com/OpenAgentsInc/openagents/issues/346 — Unload
- https://openagents.com/OpenAgentsInc/openagents/issues/347 — Memory fields
- https://openagents.com/OpenAgentsInc/openagents/issues/352 — `ctx.done`
- https://openagents.com/OpenAgentsInc/openagents/issues/353 — Progress bar
- https://openagents.com/OpenAgentsInc/openagents/issues/354 — `prompt.done`
- https://openagents.com/OpenAgentsInc/openagents/issues/355 — `prefill.done`
- https://openagents.com/OpenAgentsInc/openagents/issues/356 — `gen.done`

Next wave (parity and speed vs Ollama): [PARITY.md](./PARITY.md)

- https://openagents.com/OpenAgentsInc/openagents/issues/357 — Hybrid qwen35 decode
- https://openagents.com/OpenAgentsInc/openagents/issues/358 — Metal GEMM / tok/s
- https://openagents.com/OpenAgentsInc/openagents/issues/359 — Tokenizer + chat template
- https://openagents.com/OpenAgentsInc/openagents/issues/360 — Explicit sampling
- https://openagents.com/OpenAgentsInc/openagents/issues/361 — `inference bench`
- https://openagents.com/OpenAgentsInc/openagents/issues/362 — Coder `--model psionic:`
- https://openagents.com/OpenAgentsInc/openagents/issues/363 — Coder stream + cancel
- https://openagents.com/OpenAgentsInc/openagents/issues/364 — Default `--local`

Related older writing stays where it is. It is evidence, not this program's
authority:

- [Coder sidecar analysis (2026-08-26)](../coder/2026-08-26-psionic-local-inference-integration-analysis.md)
- [Qwen 3.8 / Ollama audit (2026-08-28)](../2026-08-28-qwen38-and-ollama-audit.md)
- [Coder v0.2.0 scope](../coder/2026-08-28-coder-v0.2.0-scope.md) (Psionic wiring remains out of that CLI release)
- Cloud file adapter: [oa-node Psionic workers](../cloud/oa-node/PSIONIC_WORKERS.md)

The 2026-07-08 retirement of the Tassadar training-program docs does not
block this CLI inference slice. That retirement parked distributed training,
gym, and executor research. This folder is a new, bounded product program.

`crates/psionic-gguf` and `openagents inference run` walk slices 0–10 and
12: mmap, context, tokenize, prefill, generate on the **fixture** graph
(embed → RMSNorm → lm-head). A live Coder `/load` of the development
Ollama `qwen3.8:27b-mtp-q8_0` blob reaches Weights ready and Context
ready. 27B tokens are not hybrid-decoder quality. Measure with
`inference bench`. Do not flip `--local` yet.
