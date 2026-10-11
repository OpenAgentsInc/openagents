# Attention with 128-key tiles

The one fixed shared-machine run passed all 204 checks across 578,586,112 compared values. All 30 paired GPU-time comparisons favored the 128-key candidate. It merits full-model validation, but these synthetic attention checks do not establish model logits, answer accuracy, or whole-model speed.

| Layout | History tokens | Baseline median ms | Candidate median ms | Median paired candidate/baseline |
| --- | ---: | ---: | ---: | ---: |
| Flash, 16 query / 4 KV heads | 1,024 | 0.7153 | 0.3187 | 0.5897 |
| Flash, 16 query / 4 KV heads | 4,096 | 5.8038 | 3.0375 | 0.5234 |
| Flash, 16 query / 4 KV heads | 16,384 | 33.8210 | 18.1442 | 0.5331 |
| 27B, 24 query / 4 KV heads | 1,024 | 0.7961 | 0.4963 | 0.6234 |
| 27B, 24 query / 4 KV heads | 4,096 | 8.8559 | 5.2290 | 0.5942 |
| 27B, 24 query / 4 KV heads | 16,384 | 55.0144 | 30.6875 | 0.5322 |

Each row uses five fixed pairs, alternating baseline-first and candidate-first. The paired ratio is not a ratio of separate medians. The operation is one final chunk of at most 2,048 queries, not a complete attention layer over the entire prompt. Inputs are deterministic synthetic half values with the correct grouped-head layouts; no model weights were loaded. No task-owned model or build overlapped the GPU window. Reported one-minute host load was very high, 251.54–252.45. All samples remain available, including the first Flash 1k samples that were much slower than later ones. These data do not establish a clock, thermal, or scheduling cause.

The largest absolute difference between 32-key and 128-key outputs was `0.0001199767`, below the fixed `0.001` gate. Full small-case comparisons with independent f64 attention reached `0.0000910759`; the predetermined large-case f64 samples reached `0.0000190254`. Chunk comparisons had zero difference. Every timed output matched its own previously checked immutable expectation bit for bit. All values were finite, source hashes and output redzones stayed unchanged, and exact repeats passed.

The candidate changes only the key tile, kernel name, and explanatory comments. It retains 16 queries and 128 threads per group, half inputs and tile probabilities, and float score/output accumulation. Different key tiles still change softmax rescaling and half rounding boundaries. Full-model comparison must retain the existing raw-logit `0.001` limit and all top answers unchanged before any integration.

Compiled threadgroup scratch was exactly 19,456 bytes for the baseline and 28,672 for the candidate, both below the device's 32,768-byte limit. This checks dispatch feasibility, not actual occupancy. The largest explicit buffer total was 244,451,584 bytes; the highest recorded Metal allocation was 245,268,480 bytes. The process guard measured 371,164,072 peak accounted bytes. Build and run finished on their first attempts in 4.97 and 4.32 seconds. All owned children stopped and all leases were released. The 2 GiB process cap, 14 GiB starting margin, 12 GiB available-memory floor, and 60-second deadline remained unchanged.

## Evidence and reproduction

- `bench-1/stdout.jsonl` contains every correctness check, pipeline limit, timing sample, and the final 128-command count.
- `bench-1/summary.json` validates exact timing keys, all expected case/check counts, finite positive times, numerical passes, successful cleanup receipt, and completion. Incomplete or failed runs make `summarize.py` exit nonzero.
- `build-1/` and `bench-1/` retain logs, memory samples, source/binary hashes, limits, and cleanup receipts. No prior attempt failed.
- `PLAN.md` and `manifest.json` preserve the preparation plan; their source-only status describes preparation, not the later run. `preparation-source-check.json` similarly records an earlier static check. `summary-selftest-results.json` records six synthetic checks of summary failure handling; these are not GPU results.
- `flash-layout.json` verifies the Flash GGUF head geometry from metadata only. No tensor payload was read for that check.
- `publication-manifest.json` maps original hashes to sanitized publication copies. Only `build.py` and `guard.py` change functionally for portability: they read the repository root from `OPENAGENTS_WORKTREE`. Receipt source hashes continue to identify the original executed files.

Copy this directory to a fresh scratch location, set `OPENAGENTS_WORKTREE` to the checkout containing `crates/psionic/fixtures/clef/tools/mac_quiet_bench.py`, and follow the lease commands in `PLAN.md` with fresh output names. The original production shader hash must still match. No quiet lease, desktop control, system setting, or full model run is part of this probe.
