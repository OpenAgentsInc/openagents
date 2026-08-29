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
| OpenAgents CPU hybrid (rc.20) | 0.31 | 412131 / 128 tok | Metal wrap only; Q8 on CPU |
| OpenAgents Metal, one wait/matvec | 5.05 | 6341 | first Metal land |
| OpenAgents Metal, batched + fused FFN | 6.52 | 4911 | this landing |
| Ollama `qwen3.8:27b-mtp-q8_0` (cold) | 4.95 | 6462 | first compare in this session |
| Ollama `qwen3.8:27b-mtp-q8_0` (warm) | 29.36 | 1090 | second compare, same process |

OpenAgents is faster than the CPU hybrid and slower than warm Ollama.

A published `0.2.0-rc.20` binary prints `Selected backend is metal` and
never emits `gemm.metal`. That run is the 0.31 tok/s line.

## Remaining kernel

GPU hybrid GDN + FFN later landed at ~9.6 tok/s
([2026-08-29-gpu-hybrid.md](./2026-08-29-gpu-hybrid.md)). The remaining
#358 gap is GPU full-attention and a faster Q8 kernel.

`--local` was not flipped.
