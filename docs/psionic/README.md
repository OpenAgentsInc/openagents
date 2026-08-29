# Psionic in the OpenAgents CLI

This directory is the owner-accepted ledger for bringing a **narrow local
inference slice** of Psionic into this repository, linking it into the
`openagents` binary, and replacing Ollama as Coder's local Qwen 3.8 runtime
once the gates in the plan pass.

| Document | Role |
| --- | --- |
| [INTENT.md](./INTENT.md) | What the owner asked for, and what this program does not include |
| [PLAN.md](./PLAN.md) | First implementation plan, crate allowlist, CLI surface, and stages |
| [CLI.md](./CLI.md) | Commands, teach mode, and the status lines from GGUF lookup through a generated token |
| [QWEN38_INFERENCE_PIPELINE.md](./QWEN38_INFERENCE_PIPELINE.md) | Weights-to-token walkthrough: tensors, quantization, load, decode |
| [OLLAMA_INFERENCE_PIPELINE.md](./OLLAMA_INFERENCE_PIPELINE.md) | How Ollama loads weights and generates a token, GGUF and MLX |
| [LLAMA_CPP_INFERENCE_PIPELINE.md](./LLAMA_CPP_INFERENCE_PIPELINE.md) | Lowest-level llama.cpp GGUF parse, mmap, Metal mapping, graph, Q8_0 |
| [PROVENANCE.md](./PROVENANCE.md) | What we studied vs copied for the in-tree GGUF crate |

OpenAgents issues (`openagents issue`; GitHub issues remain for reproducible bugs):

- https://openagents.com/OpenAgentsInc/openagents/issues/344 — `inference run` through GGUF weights ready (`map.done`)
- https://openagents.com/OpenAgentsInc/openagents/issues/345 — Coder TUI shows local GGUF load progress (blocked by 344)
- https://openagents.com/OpenAgentsInc/openagents/issues/346 — Unload local inference weights (blocked by 344)
- https://openagents.com/OpenAgentsInc/openagents/issues/347 — Visualize local inference memory (blocked by 344 and 346)

Related older writing stays where it is. It is evidence, not this program's
authority:

- [Coder sidecar analysis (2026-08-26)](../coder/2026-08-26-psionic-local-inference-integration-analysis.md)
- [Qwen 3.8 / Ollama audit (2026-08-28)](../2026-08-28-qwen38-and-ollama-audit.md)
- [Coder v0.2.0 scope](../coder/2026-08-28-coder-v0.2.0-scope.md) (Psionic wiring remains out of that CLI release)
- Cloud file adapter: [oa-node Psionic workers](../cloud/oa-node/PSIONIC_WORKERS.md)

The 2026-07-08 retirement of the Tassadar training-program docs does not
block this CLI inference slice. That retirement parked distributed training,
gym, and executor research. This folder is a new, bounded product program.

`crates/psionic-gguf` and `openagents inference run` land slices 0–6
(`map.done`). Later slices (context, generate, Coder `--model psionic:`)
are not in the binary yet.
