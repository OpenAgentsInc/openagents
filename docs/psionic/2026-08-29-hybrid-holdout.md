# Hybrid holdout — 2026-08-29

- CLI: `0.2.0-rc.20` (this landing)
- Graph: `qwen35_hybrid` when `blk.0.ffn_down.weight` exists
- Host: development Apple Silicon Mac
- Weights: not in git

## Fixture

`write_qwen35_hybrid_fixture` greedy IDs are deterministic.
`until_gen_done_on_fixture` still uses the four-tensor stub
(`embed_lmhead`).

## 27B greedy vs Ollama

Not measured in this landing. The CPU hybrid walks 64 trunk layers
and skips MTP `blk.64.nextn.*`. A 24-token stub prefill was seconds;
a real 64-layer Q8_0 prefill on this blob is expected to take minutes
per token until Metal GEMM (#358).

First mismatch position: **not run**. Do not publish a quality-parity
sentence. Re-run after install:

```text
openagents inference bench --gguf <27B blob> --prompt hello --max-tokens 1 \
  --compare-ollama qwen3.8:27b-mtp-q8_0
```

`--compare-ollama` must print JSON (or `ollama_skipped`) and must not
panic with a nested Tokio runtime.
