# Exact half LUT gate projection: performance rejected

The LUT path passed the unchanged 0.001 numerical limit but was much slower
than the production Metal kernel. Keep the production implementation. This
experiment changed no product code and made no full-model claim.

The probe uses actual Flash layer-0 gate weights with 1,024 deterministic
synthetic activation rows: K=4,096 and M=12,288. The public MPS NDArray API
accepts half activations, UInt4 indices, grouped half lookup values, and a
float32 destination. Every one of the 50,331,648 decoded half weights matched
the independent half artifact bit for bit, on both CPU and GPU.

All four variants completed one correctness call and five fixed, alternating
timing rounds. Every call compared all 12,582,912 output values with the
production result: 301,989,888 comparisons in total. All outputs were finite,
all guards passed, and all immutable input hashes matched. The LUT maximum
absolute difference was 0.000001311303; the other variants were bit-identical.
The independent double-precision oracle checked 96 coordinates per variant
plus the initial reference. Its maximum error was 0.000000219793.

| Variant | Median GPU command time | Range across five rounds |
| --- | ---: | ---: |
| Production MPP, fixed 64 columns | 2.376 ms | 2.145–3.436 ms |
| Production MPP, weight-shape selection | 2.532 ms | 1.843–3.035 ms |
| Dense half NDArray, float32 result | 2.490 ms | 1.884–3.117 ms |
| UInt4/half LUT NDArray, float32 result | 198.011 ms | 142.756–249.518 ms |

The LUT ratio of medians was 83.35 times the fixed-column baseline. Individual
paired ratios were 64.02–83.35. The small shared-machine sample does not support
a close ranking among the other three variants. No retry or threshold change
followed this rejection.

CPU preprocessing took 428.087 ms before upload; preprocessing and upload
together took 565.385 ms. Both were excluded from projection timings. The
reported LUT interval includes its complete quantized matrix operation; there
is no separately timed decode omitted from that interval. The exact-decode
verification ran separately and released its temporary buffer before timing.

## Memory and conditions

The owned check completed in 4.575 seconds. Explicit buffers peaked at
385,904,640 bytes after the temporary decode check. Recorded Metal allocation
endpoints were about 386.7 MB, and the process guard sampled 702,776,496 bytes
of accounted memory. The lowest sampled available host memory was about
102.9 GB. Cleanup stopped the owned child group, and all leases were released.

No persistent 96 MiB expanded weight allocation appeared at the recorded
endpoints. These observations do not exclude a transient or private expansion;
point-in-time device accounting and nine process samples are not exact peak
measurements. The API does not promise a particular internal accumulator or
fusion implementation.

This was a shared-machine run without a quiet lease. The guard recorded
available memory but no contemporaneous CPU or GPU load sample. No desktop,
service, priority, or power setting changed. These numbers do not establish
a hardware cause for throughput changes.

## Evidence and reproduction

[The summary](summary.json) contains all four timing arrays, numerical errors,
allocation observations, and executable hashes. [The complete output](check-1/stdout.jsonl)
contains every emitted check and timing record. The build and check directories
retain complete output, memory samples, and cleanup receipts. There was one
successful build and one complete check, with no compile repair or rerun.

Copy this bundle into persistent scratch. Set `OPENAGENTS_WORKTREE` to the
repository containing the shared guard utilities. Set `CLEF_GATE_ASSETS` to a
directory containing the verified `gate.q4` and `gate.f16` files described in
[source-weights.json](source-weights.json). The bundle excludes weights and
executables. Use the SDK identified by [sources.json](sources.json).

```sh
openagents lease build --keep-target-dir -- openagents lease memory --amount 4 -- python3 guard.py build --output build-reproduction
openagents lease gpu -- openagents lease memory --amount 2 -- python3 guard.py check --output check-reproduction
```

The check has a 2 GiB process cap and a 60-second deadline, requires 14 GiB
available before launch, and stops its owned group if available memory falls
below 12 GiB. The build uses a 4 GiB cap and a 180-second deadline. The guard
refuses quiet leases. Do not increase these bounds to turn a refusal into a
result. There is no half-output or full-float32-input fallback.

[The manifest](manifest.json) records original and published hashes. Personal
paths in published receipts are placeholders. Only the reproduction guard
changes to accept portable directory variables; the numerical probe and
production Metal fragment remain byte-identical to the measured sources.
