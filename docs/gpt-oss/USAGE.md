# GPT-OSS Usage Guide (for Future Agents)

This is a practical guide to get GPT-OSS running fast and reliably in OpenAgents, with the lessons learned from the 2026-01-04/05 sprint. Start here if you need working inference quickly.

## Fast Path (Recommended)

### 1) Start the fast llama.cpp server

```bash
scripts/gpt-oss-fast.sh
```

This script:
- Starts `llama-server` with the fast flags (including `--no-mmap`).
- Auto-picks a GGUF quant from `~/models/gpt-oss-20b/gguf`.
- Warms the server and optionally runs keepalive.

### 2) Run a quick prompt (raw mode)

```bash
scripts/local-infer.sh --backend gpt-oss --raw \
  --url http://localhost:8000 --model gpt-oss-20b \
  --max-tokens 16 --temperature 0 "1+1="
```

If you want the minimal overhead path:
```bash
scripts/gpt-oss-query.sh "1+1="
```

## Harmony / Tool-Use Path

Harmony format is required for tool calls and structured outputs. It is slower and has a larger prompt footprint.

```bash
scripts/local-infer.sh --backend gpt-oss --tools "Summarize this repo"
```

If you see a context error:
- Increase `GPT_OSS_CTX` (start with 512 or 1024).
- Or use `--raw` if you do not need Harmony/tool calls.

`local-infer` now falls back to `--raw` automatically if the Harmony prompt exceeds context size, but you should still bump `GPT_OSS_CTX` for real tool use.

## Performance Lessons (TL;DR)

- **`--no-mmap` is critical** on macOS. Without it, decode is ~10x slower.
- Keep a **persistent server** running to avoid cold-start latency.
- **Small context wins**: `GPT_OSS_CTX=384` or `512` gives best latency.
- **Keepalive** helps avoid paging spikes: `GPT_OSS_KEEPALIVE_SECS=1` is the most stable.
- **Q4_0 + f16 KV + flash-attn** is the best all-around default.
- **Q3_K_S** can be faster on decode but has more paging spikes.
- **Q2_K was slower** on this machine; skip it unless you confirm otherwise.

## Recommended Defaults

These are already baked into `scripts/gpt-oss-fast.sh`, but listed here for clarity:

- `--no-mmap`
- `-c 384` (override with `GPT_OSS_CTX`)
- `-b 256 -ub 256`
- `-np 4`
- `-ctk f16 -ctv f16`
- `--flash-attn`
- `GPT_OSS_KEEPALIVE_SECS=1`

## Troubleshooting

### Context errors (HTTP 400)

Cause: Harmony prompt is too large for `GPT_OSS_CTX`.

Fix:
- Increase `GPT_OSS_CTX` to 512/1024+
- Or run `--raw` if tool use is not required

### Slow or spiky latency

Cause: paging or cold-start effects.

Fix:
- Ensure server is started with `--no-mmap`.
- Keep it warm with `GPT_OSS_KEEPALIVE_SECS=1`.
- Use `scripts/gpt-oss-status.sh` to verify prompt/decode rates.

### mlock warnings

`llama-server --mlock` fails on macOS by default; skip it. The `gpt-oss` Metal backend was patched to make mlock opt-in, but llama.cpp still fails under default limits.

### IPv6 delays

Scripts force IPv4 (`curl -4`) to avoid intermittent localhost delays.

## Benchmarking

Quick timing:
```bash
scripts/gpt-oss-status.sh
```

Repeated latency samples:
```bash
scripts/gpt-oss-bench.sh 50
```

## Model Paths

The scripts assume GGUF models live here:
```
~/models/gpt-oss-20b/gguf/
```

Override with:
- `GPT_OSS_GGUF_MODEL_PATH` for a specific file
- `GPT_OSS_MODEL_DIR` to change the auto-pick directory

## Metal Backend (FYI)

The `gpt-oss-metal` backend builds and runs, but it is currently **too slow** (multi‑second per token) for interactive use on this machine. Use `llama.cpp` for speed unless you are explicitly profiling Metal kernels.

## Where to Find Deep Debug Notes

See `docs/logs/20260104/1509-gptoss-debug-log.md` for the full sprint log, including:
- metal kernel bottleneck analysis
- llama.cpp benchmarks
- quantization experiments
- keepalive tuning

