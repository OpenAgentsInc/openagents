# GPT-OSS Benchmarks

This guide describes how to benchmark GPT-OSS and FM bridge backends using the local-inference criterion bench.

## Benchmark Harness

`crates/local-inference/benches/backend_overhead.rs` measures completion latency for both backends.

## Requirements

- GPT-OSS server (llama.cpp, etc.) running
- Optional: FM bridge running (macOS)

## Run

```bash
GPT_OSS_BENCH_URL=http://localhost:8000 \
FM_BRIDGE_BENCH_URL=http://localhost:3030 \
cargo bench -p local-inference --bench backend_overhead
```

Optional model overrides:

```bash
GPT_OSS_BENCH_MODEL=gpt-oss-20b \
FM_BRIDGE_BENCH_MODEL=gpt-4o-mini-2024-07-18 \
cargo bench -p local-inference --bench backend_overhead
```

## Notes

- If a backend URL is missing or the server is unavailable, that benchmark is skipped.
- Criterion reports are written under `target/criterion/`.
