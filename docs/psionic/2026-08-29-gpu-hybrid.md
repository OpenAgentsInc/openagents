# GPU hybrid GDN + FFN — 2026-08-29

```text
CLAIM
actor/session: cursor 358-gpu-layer
base: 610bf8e248628a61095b000e1abbf449408a0228
worktree/branch: .oa-worktrees/issue-358-gpu-layer
scope: Metal hybrid layer (GDN + FFN); GPU-resident hidden between hybrid layers
paths: crates/psionic-gguf/src/metal_gemm.rs, crates/psionic-gguf/src/qwen35.rs, crates/psionic-gguf/src/metal_graph.rs, docs/psionic/
hot files: crates/psionic-gguf/src/metal_gemm.rs
hot contracts: none
verification: cargo test -p psionic-gguf; cargo test -p openagents-cli --test inference_test; 27B hello 32-token IDs; inference bench vs Ollama
claimed_at: 2026-08-29T20:14:00Z
```

- CLI source: `0.2.0-rc.22` (unpublished)
- Graph: `qwen35_hybrid` + Metal hybrid layers
- Host: development Apple Silicon Mac
- Weights: not in git
- Prompt: `hello` (11 tokens, thinking-on wrap)

## Quality

27B greedy first 32 IDs still match
[2026-08-29-hybrid-holdout.md](./2026-08-29-hybrid-holdout.md).
First token is `760`. `--backend cpu` is unchanged. `OPENAGENTS_HYBRID_CPU`
forces the CPU hybrid graph.

A Metal shader compile that used a field named `kernel` failed the
whole library (`kernel` is reserved). Decode then used CPU Q8 at
~0.53 tok/s. The field is `ksize`. `metal_q8_matches_cpu_when_available`
now fails if wrap succeeds and the shader does not compile.

## Speed (warm)

`inference bench --gguf <blob> --prompt hello --max-tokens 32 --compare-ollama qwen3.8:27b-mtp-q8_0`

Second compare in the session (warm Ollama):

| Engine | tok/s | gen_ms | notes |
| --- | ---: | ---: | --- |
| OpenAgents batched Q8 + fused FFN | 6.52 | 4911 | prior #358 land |
| OpenAgents GPU hybrid + hidden stream | 9.56 | 3346 | this landing |
| Ollama `qwen3.8:27b-mtp-q8_0` (warm) | 27.52 | 1163 | same second compare |

An 8-row Q8 tile was slower (6.7 tok/s) and was not kept.

## Remaining gap

Full-attention layers still run on CPU and flush hidden every fourth
layer. One command buffer per token (GPU attention + KV) and a
faster Q8 kernel are still required to beat warm Ollama.

`--local` was not flipped. #358 stays open.
