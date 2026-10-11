# FFN fusion experiment

The one bounded shared-machine run passed all 66 correctness checks. Every candidate activation matched its same-tile separate baseline bit for bit. The independent small-shape f64 oracle passed; its largest final-half difference was 0.0000152587890625. All 231,542,178 checked outputs were finite, and output redzones were unchanged.

Pair fusion was slower at every tested shape by the median paired ratio (1.4% to 6.8%). Keep it experimental. Up-only fusion improved the 1,024-row pair by about 3.1% to 3.4%, but regressed the 65-row, 128-column tile and did not show a consistent large gain at 2,048 rows. This does not justify product integration.

| Token rows | Output tile | Separate median ms | Up-only paired ratio | Pair paired ratio |
| --- | --- | --- | --- | --- |
| 65 | 64 | 0.7113 | 0.9947 | 1.0541 |
| 65 | 128 | 0.6708 | 1.0236 | 1.0640 |
| 1,024 | 64 | 3.9019 | 0.9662 | 1.0140 |
| 1,024 | 128 | 3.6588 | 0.9689 | 1.0163 |
| 2,048 | 64 | 15.6246 | 0.9913 | 1.0682 |
| 2,048 | 128 | 15.4350 | 0.9859 | 1.0238 |

Each ratio is the median of six same-round candidate/baseline GPU-time ratios. Lower is better. Timing order alternates. The 2,048-row baseline ranged from about 8.3 to 21.0 ms during this short shared-machine run. The recorded one-minute load average was 20.29. These data do not establish a thermal or memory-bandwidth cause.

Weights and inputs are deterministic synthetic values. The large shape is Flash's 12,288 by 4,096 projection, but no checkpoint weights or model were loaded. Each timing sample includes two gate/up/SiLU pairs. It excludes down projection, residual addition, on-demand Q4 conversion, head execution, and serving overhead. The largest fixture allocated about 520 MB. The run took 3.96 seconds and peaked at 706,560,984 accounted bytes under the 2 GiB guard. Build and runtime cleanup succeeded; both runtime leases were released.

The pair kernel keeps both projection results in cooperative f32 registers and writes half activations. Integration would require simultaneous gate/up weight residency and an output buffer distinct from the normalized input. The up-only variant can preserve the existing Q4 conversion slot, but still needs full-FFN, raw-logit, and uncached request checks. No product source changed.

## Files and reproduction

`summary.json` contains aggregate results. `bench-1/stdout.jsonl` contains every correctness and timing record. The build and run receipts include limits, cleanup state, source hashes, and the executable hash. Lease receipts show resource ownership. `source-preparation.json` and `static-investigation.json` describe the earlier preparation phase; their historical statements that execution had not run remain unchanged.

`manifest.json` maps original bytes to sanitized copies. It preserves original SHA-256 values. Private paths become labels; no model weights, binaries, compiler output objects, or large memory timelines are included. The only functional script edit makes the repository path an explicit `OPENAGENTS_REPO` environment variable. Set it to the checkout that provides `crates/psionic/fixtures/clef/tools/mac_quiet_bench.py`; the original guard source hash is retained in the receipts. Copy this directory to a new scratch directory before reproduction, because the guard refuses to overwrite output directories.

After a coordinated build slot, use a fresh build output name:

```sh
openagents lease build --keep-target-dir --receipt build-lease-2.json -- openagents lease memory --amount 4 --receipt build-memory-lease-2.json -- python3 guard.py build --output build-2
```

After an explicit GPU handoff, use a fresh runtime output name:

```sh
openagents lease gpu --no-wait --receipt bench-gpu-lease-2.json -- openagents lease memory --amount 2 --no-wait --receipt bench-memory-lease-2.json -- python3 guard.py bench --output bench-2
```

No quiet lease or desktop/system control is part of this experiment. Timing remains a shared-machine observation.
