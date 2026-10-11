# Flash GEMM tile experiment

All 70 correctness checks passed: 49 small independent f64 GEMM cases and 21 weighted FFN comparisons. The 899,440,640 weighted gate/up/down/activation values matched the original kernel bit for bit. All outputs were finite and output redzones were unchanged. The largest small-GEMM difference from f64 was 0.000002187465725.

Larger tiles did not improve the FFN against the current 64-by-128, four-SIMD baseline. Keep them experimental. The table shows medians of six same-round GPU-time ratios; lower is better.

| Token rows | 128×128, four SIMD | 64×128, eight SIMD | 128×128, eight SIMD | 128×256, eight SIMD |
| --- | --- | --- | --- | --- |
| 65 | 1.9455 | 0.9662 | 1.0957 | 1.9794 |
| 1,024 | 1.1223 | 1.1778 | 1.0188 | 1.4372 |
| 2,048 | 1.1325 | 1.0791 | 1.0558 | 1.5977 |

At 1,024 rows every alternative regressed in its median paired ratio. The 2,048-row baseline ranged from 10.80 to 39.40 ms during this short shared-machine run. Comparing separate medians would misleadingly favor 128-by-128/eight-SIMD (18.12 versus 19.16 ms); its paired median ratio was 1.0558, a regression. Keep every sample. The recorded one-minute load average ranged from 26.81 to 27.58. No thermal, clock, or bandwidth cause was established.

The run used actual Flash layer-0 gate/up/down Q4_K weights, independently decoded to half, and deterministic inputs. It measures three projections and SiLU. It excludes normalization, residual addition, on-demand Q4 conversion, head execution, and serving overhead. This is not model accuracy or whole-model latency evidence. A small CPU reference pilot was allowed to overlap; no attribution of its effect was measured.

The build and bounded weight staging passed in 6.01 seconds. The GPU run finished in 5.39 seconds with a 1,017,169,216-byte peak accounted footprint under the 2 GiB guard. Its GPU lease waited 323.19 seconds and then held the resource for 5.57 seconds; that wait is excluded from kernel timing. Cleanup succeeded and both runtime leases were released. No product files changed.

## Evidence and reproduction

`summary.json` records all aggregate metrics and one-factor comparisons. `bench-1/stdout.jsonl` contains all 70 checks and 126 timing samples. `weights.json` identifies the source tensors and exact staged half bytes. `analysis.json` is the historical pre-run hypothesis, including unmeasured logical traffic scenarios. The source, build/run receipts, lease receipts, and original-to-published SHA-256 mappings are included. No model tensors, executable, compiler output, or memory timeline is published.

The portable script copies replace private locations with explicit environment variables. Set `OPENAGENTS_REPO` to the checkout containing `mac_quiet_bench.py`, `CLEF_NUMPY_PYTHON` to a Python with NumPy, `CLEF_FLASH_GGUF` to the verified artifact, and `CLEF_Q4_HELPERS` to the existing `q4k-weight-diagnostic` helper directory. That directory must contain `compare_weights.py`, the exact pinned `upstream_quants.py`, and its `source.json`; decoder identity is recorded in `weights.json`. The original source hashes remain in the receipts and manifest. Copy this bundle to a fresh scratch directory before reproduction; the scripts refuse to overwrite results or staged weights.

After a coordinated build slot:

```sh
openagents lease build --keep-target-dir --receipt build-lease-2.json -- openagents lease memory --amount 4 --receipt build-memory-lease-2.json -- python3 guard.py build --output build-2
```

After an explicit GPU handoff:

```sh
openagents lease gpu --receipt bench-gpu-lease-2.json -- openagents lease memory --amount 2 --receipt bench-memory-lease-2.json -- python3 guard.py bench --output bench-2
```

No quiet lease or desktop/system controls are used. A later winning kernel would still need accumulate, actual-model raw-logit/answer, and uncached serving checks before integration.
