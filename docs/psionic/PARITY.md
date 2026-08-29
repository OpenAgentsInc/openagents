# Ollama parity and speed program

- Class: owner-accepted compare contract
- Status: accepted 2026-08-29; bench landed (#361). Hybrid quality
  holdout landed (#357). Metal Q8 matvec landed; tiled GEMM still
  needed to beat warm Ollama (#358). Tokenizer leftovers (#359).
- Ledger: [INTENT.md](./INTENT.md), [PLAN.md](./PLAN.md), [CLI.md](./CLI.md)
- Ollama reference: [OLLAMA_INFERENCE_PIPELINE.md](./OLLAMA_INFERENCE_PIPELINE.md)
- Decoder math: [QWEN38_INFERENCE_PIPELINE.md](./QWEN38_INFERENCE_PIPELINE.md)

This file is how we decide we are **as good as or better than** the
development Ollama path on `qwen3.8:27b-mtp-q8_0`, and faster on the
same Mac. It is not a claim that rc.18 or rc.19 already is.

Ollama is the holdout. It is not a generate fallback.

## Holdout

| Field | Value |
| --- | --- |
| Ollama tag | `qwen3.8:27b-mtp-q8_0` |
| Blob | `~/.ollama/models/blobs/sha256-2bb22714289826d7b9e0ba376c3ce47d08bce39abe598745857c44d88c09bdbf` |
| Architecture | `qwen35` |
| Quant | Q8_0 |
| Runtime `n_ctx` | 4096 (not trained 262,144) |
| First sample plan | greedy |
| Host | development Apple Silicon Mac |

Do not check the blob into git. Do not put 27B weights in CI.

## What rc.19 does and does not do

| Axis | rc.19 | Ollama today |
| --- | --- | --- |
| Load | mmap + Metal NoCopy, sub-second on a warm 27 GiB file | llama-server child maps and places layers |
| Context | F16 KV 64 KiB × `n_ctx` (256 MiB at 4096) + GDN | llama.cpp KV / GDN |
| Prefill / decode graph | embed → RMSNorm → lm-head | 64 hybrid layers |
| lm-head | parallel CPU Q8_0 matvec | Metal/CPU llama.cpp GEMM |
| Tokens | not quality-comparable | holdout |
| Measure | `inference bench` | `--compare-ollama` when the daemon is up |

`graph` is `qwen35_hybrid` when `blk.0.ffn_down.weight` exists (27B
and the hybrid fixture). The four-tensor CI file stays `embed_lmhead`.
27B greedy `hello` first 32 token IDs match Ollama
([2026-08-29-hybrid-holdout.md](./2026-08-29-hybrid-holdout.md)).
The CPU hybrid is the quality graph, not the speed path.

## Compare axes

1. **Token identity (greedy).** Same rendered prompt, same blob, first 32
   token IDs. #357 + #359.
2. **Tokenizer IDs.** Same chat messages → same ID list. #359.
3. **Throughput.** `tok_per_s` on `--max-tokens 32` after a loaded
   context, same `n_ctx`. #358 + #361.
4. **Time to first token.** Prefill wall + first decode. #357 + #358.
5. **Memory.** `mmap_resident_bytes`, process RSS, cache KV + GDN vs
   Ollama runner RSS. Already visible on `inference status`.
6. **Cancel.** Interrupt during generate. CLI path exists; Coder is #363.
7. **Tools.** Multi-round Coder turn. #362.

A receipt is a dated markdown file in this folder (or a row in this
file) with CLI version, host, prompt length, `generated`, `tok_per_s`,
Ollama `tok_per_s` when measured, and `graph`. No weights. No secrets.

## Command

```text
openagents inference bench --gguf <path> --prompt <text> --max-tokens 8
openagents inference bench --gguf <path> --prompt <text> --max-tokens 8 \
  --compare-ollama qwen3.8:27b-mtp-q8_0
```

Stdout is one JSON object. Teach lines stay on stderr.

## Issue wave

| Issue | Unlocks |
| --- | --- |
| [#357](https://openagents.com/OpenAgentsInc/openagents/issues/357) | Hybrid graph (quality). **Holdout recorded.** |
| [#358](https://openagents.com/OpenAgentsInc/openagents/issues/358) | Metal GEMM (speed). Row matvec landed; tiled kernel remains. |
| [#359](https://openagents.com/OpenAgentsInc/openagents/issues/359) | Chat template / tokenize parity |
| [#360](https://openagents.com/OpenAgentsInc/openagents/issues/360) | Explicit sampling |
| [#361](https://openagents.com/OpenAgentsInc/openagents/issues/361) | `inference bench` + Ollama compare |
| [#362](https://openagents.com/OpenAgentsInc/openagents/issues/362) | Coder `--model psionic:` |
| [#363](https://openagents.com/OpenAgentsInc/openagents/issues/363) | Coder stream + cancel |
| [#364](https://openagents.com/OpenAgentsInc/openagents/issues/364) | Default `--local` after gates |

Do not flip `--local` on a quality or speed gap.
