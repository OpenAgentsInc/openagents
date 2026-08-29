# Metal Q8 GEMM — 2026-08-29

```text
CLAIM
actor/session: cursor 358-metal
base: 60627927bac61c1e0f90a191b295ddea470f31d8
worktree/branch: .oa-worktrees/issue-358-metal
scope: Metal Q8_0 matvec on the shared mmap; CPU path on --backend cpu
paths: crates/psionic-gguf/src/metal_gemm.rs, crates/psionic-gguf/src/metal_wrap.rs, crates/openagents-cli/src/inference.rs, docs/psionic/
hot files: crates/openagents-cli/src/inference.rs
hot contracts: docs/psionic/CLI.md (slice 9–10 ids unchanged; added gemm.metal)
verification: cargo test -p psionic-gguf; cargo test -p openagents-cli --test inference_test; 27B hello 32-token IDs; inference bench vs Ollama
claimed_at: 2026-08-29T19:49:44Z
```

- CLI source: `0.2.0-rc.22` (this landing)
- Graph: `qwen35_hybrid` + `gemm.metal`
- Host: development Apple Silicon Mac
- Weights: not in git
- Prompt: `hello` (11 tokens, thinking-on wrap)

## Quality

`--backend cpu` and Metal both sample the holdout 32 IDs from
[2026-08-29-hybrid-holdout.md](./2026-08-29-hybrid-holdout.md). First
token is `760`.

`metal_q8_matches_cpu_when_available` compares a synthetic Q8 matrix
including a non-zero byte offset.

## Speed (warm)

`inference bench --gguf <blob> --prompt hello --max-tokens 32 --compare-ollama qwen3.8:27b-mtp-q8_0`

| Engine | tok/s | gen_ms | notes |
| --- | ---: | ---: | --- |
| OpenAgents Metal Q8 matvec | 5.05 | 6341 | one command buffer per matvec |
| Ollama `qwen3.8:27b-mtp-q8_0` | 28.29 | 1131 | warm llama.cpp Metal |

OpenAgents is faster than the CPU hybrid (~0.3 tok/s on this host) and
slower than warm Ollama.

## Remaining kernel

Need a tiled Metal `mul_mat_q8_0` (SIMD-group GEMM, reused encodings,
fused FFN pair) in the style of llama.cpp `mul_mm_q8_0`. The current
kernel is one threadgroup per output row, 32 threads reducing Q8
blocks, plus a CPU round-trip after every matvec. That is the gap
named by #358.

`--local` was not flipped.
