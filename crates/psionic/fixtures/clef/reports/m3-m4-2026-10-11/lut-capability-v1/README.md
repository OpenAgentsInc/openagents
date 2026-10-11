# Exact half LUT: tiny capability passed

The public `MPSNDArrayQuantizedMatrixMultiplication` path accepted half
activations, UInt4 indices, grouped half lookup values, and an explicit
float32 destination on the tested M5 Max. All five exact-decode checks passed
(11,264 half weights), as did all 20 matrix comparisons (792 output values).
The maximum error against the CPU double-precision oracle was
0.000000238418579, below the unchanged 0.001 limit.

The precision fixtures include half-product overflow with cancellation, small
terms between large terms, and a result whose half-output rounding would
introduce 0.020751953 error. Float32 residual addition was checked separately.
All outputs were finite; poisoned outputs, source immutability, and all unused
buffer padding passed. The library's internal accumulator implementation is
opaque; these are behavioral precision checks, not an ISA-level proof.

This is a tiny API capability result. It does not establish actual-weight
accuracy, throughput, full-model acceptance, or absence of private expanded
weight storage. No product code or setting changed.

## Retained attempts

1. `check-1` stopped with SIGABRT before arithmetic because MPS required a
   larger external backing buffer. Its exact source and complete error remain.
2. `check-2` accepted the corrected backing allocation, then failed exact
   grouped decoding. The descriptor's default row padding disagreed with the
   probe's contiguous UInt4 buffer. No matrix multiplication ran.
3. `check-3` set `preferPackedRows=YES` for all external buffers and passed the
   original five cases. The backing-capacity correction remained, and every
   unused padding byte stayed guarded. No numerical threshold changed.

All three builds passed. Build and run receipts identify their exact source
and executable hashes. The final binary SHA-256 is
`592022fc2d41f519bad65f4bea2077dd4521be28562b156e5787831ae2fd3e58`.
The successful run lasted 0.627 seconds and used at most 69,632 bytes of
explicit GPU buffers. The guard sampled 151,257,856 bytes of accounted
process memory; this short run has only two samples, not an exact peak.
Every owned child group stopped and leases were released.

## Reproduce

Copy this bundle into persistent scratch and set `OPENAGENTS_WORKTREE` to the
repository containing `mac_quiet_bench.py`. Use the SDK identified by
`sources.json`. No model assets are needed.

```sh
openagents lease build --keep-target-dir -- openagents lease memory --amount 4 -- python3 guard.py build --output build-reproduction
openagents lease gpu -- openagents lease memory --amount 2 -- python3 guard.py check --output check-reproduction
```

The capability process cap is 2 GiB with a 60-second deadline, 14 GiB of
available memory before launch, and a 12 GiB available-memory floor. The
build uses a 4 GiB cap and a 180-second deadline. The guard refuses quiet
leases and records complete output and cleanup. It provides no weighted
benchmark mode or precision fallback.

`summary.json` records the aggregate result; `check-3/stdout.jsonl` contains
every observed and expected output. Earlier failures and sources are retained
beside their receipts. Personal paths in the published files are placeholders;
`manifest.json` maps original and published hashes. Executables are excluded.
