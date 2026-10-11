# Flash GEMM K and layout experiment

All 24 correctness checks passed: 15 small f64 GEMM checks at the actual K dimensions and nine weighted FFN comparisons. The 385,474,560 weighted gate/up/down/activation values matched the baseline bit for bit. All outputs were finite and output redzones were unchanged.

Neither candidate produced a material FFN speedup. Keep both experimental. All three modes use 64-token by 128-output tiles and four SIMD groups. The first candidate specializes the whole K dimension to 4,096 or 12,288; it does not introduce a manual K loop. The second changes only weight storage from output-major to input-major and uses dynamic-K NN multiplication. Every transposed half bit matches its original value.

| Token rows | Baseline median ms | Static-K paired ratio | Pretransposed-NN paired ratio |
| --- | --- | --- | --- |
| 65 | 1.0845 | 0.9991 | 0.9788 |
| 1,024 | 5.2519 | 1.0020 | 1.0259 |
| 2,048 | 10.7947 | 1.0054 | 1.0124 |

Each ratio is the median of six same-round candidate/baseline GPU-time ratios. Lower is better. Ordering alternates each round. Static K was effectively unchanged. Pretransposition gained about 2.1% at 65 rows and regressed the two larger cases. Its staging took 1.30 seconds for 301,989,888 bytes. The probe retains both layouts; production could replace a layout but would need a compatible loader, Q4 conversion, and fallback design. No evidence here justifies that change.

The input weights are the same actual Flash layer-0 gate/up/down half values as the earlier tile experiment. Inputs are deterministic. Timed work includes three projections and SiLU, excluding residual addition, normalization, Q4 conversion, model head, and serving overhead. This is not a model accuracy or sustained-latency result. The recorded one-minute load average was 10.98. No clock, thermal, or memory-bandwidth cause was measured.

Offline Metal compilation, the Swift build, and transposition staging finished in 4.14 seconds under build and 4 GiB memory leases. The one GPU run completed in 2.28 seconds with a 1,067,042,040-byte peak accounted footprint under its 2 GiB guard. Cleanup succeeded and runtime leases were released. The data-allocation bound was 1.5 GiB. No product source changed.

## Evidence and reproduction

`summary.json` contains aggregates and every paired ratio. `bench-1/stdout.jsonl` contains all checks and 54 timing samples. `weights.json` includes original and transposed half hashes and one-time staging costs. The manifest maps original to sanitized source bytes. Model tensors, symlinks, compiler objects, executable, and large memory timelines are omitted.

Portable scripts take explicit locations. Set `OPENAGENTS_REPO` to the checkout containing `mac_quiet_bench.py`, `CLEF_NUMPY_PYTHON` to a Python with NumPy, and `CLEF_HALF_SOURCE` to the previous tile probe's staged `gate.f16`, `up.f16`, `down.f16`, and `weights.json`. Every source half file is checked against its recorded SHA-256. Copy this bundle to a new scratch directory before reproduction; scripts refuse to overwrite staged outputs or results.

After a coordinated build slot:

```sh
openagents lease build --keep-target-dir --receipt build-lease-2.json -- openagents lease memory --amount 4 --receipt build-memory-lease-2.json -- python3 guard.py build --output build-2
```

After an explicit GPU handoff:

```sh
openagents lease gpu --no-wait --receipt bench-gpu-lease-2.json -- openagents lease memory --amount 2 --no-wait --receipt bench-memory-lease-2.json -- python3 guard.py bench --output bench-2
```

No quiet lease or desktop/system controls are used. Keep these experiments separate from measured serving improvements.
