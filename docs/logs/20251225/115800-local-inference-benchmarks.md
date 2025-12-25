# Local Inference Benchmarks - 2025-12-25

## Summary
- Added a criterion benchmark comparing GPT-OSS and FM bridge completion latency.
- Documented benchmark usage in `docs/gpt-oss/BENCHMARKS.md`.
- Updated d-019 directive + status docs to mark benchmark harness complete.

## Validation
- `cargo bench -p local-inference --bench backend_overhead --no-run`
