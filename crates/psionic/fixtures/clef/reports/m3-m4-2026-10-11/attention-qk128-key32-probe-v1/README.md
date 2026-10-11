# QK128 batching with unchanged 32-key updates

The scratch candidate passed all bitwise and independent-reference checks.
It reduced median paired attention time by about 8–18% in these short tests.
This is one attention operation with synthetic inputs, not full-model
correctness or serving latency evidence. Product integration remains pending.

The previous full key128 candidate failed the model raw-logit gate and remains
rejected. This candidate instead batches four QK score blocks while preserving
each original 32-key softmax/PV update. It reduces eight TensorOps calls to
five per 128 keys without changing when exponentials round to f16.

## Correctness

All 140 commands and 216 checks passed. Every cross-variant attention output
was bitwise identical. The three intermediate-score diagnostics compared
36,557 computed scores with zero differing bits, checked untouched poisoned
cells, and passed 42 fixed independent f64 dot-product samples. Their maximum
f64 difference was 0.00000011367956176400185. Instrumented and normal kernels
produced bitwise-identical outputs.

The suite also passed 12 small random/adversarial cases, causal and absolute
position checks, repeated and nonaligned chunks, and six representative large
shapes. The maximum small-case f64 difference was 0.00009107589721679688;
large-case predetermined f64 samples differed by at most
0.00001813156995922327. Every timed output matched its own immutable,
previously finite-checked expected bytes. Inputs and redzones stayed unchanged.

## Timing

Five fixed alternating pairs ran for each shape. All samples, including
outliers and regressions, are retained in `bench-1/stdout.jsonl`.
The reported ratio is the median of five paired candidate/baseline ratios;
it is not the ratio of the two marginal medians.

| Geometry | History | Baseline median | Candidate median | Paired ratio | Faster pairs |
| --- | ---: | ---: | ---: | ---: | ---: |
| Flash, 16 query heads | 1k | 0.730 ms | 0.673 ms | 0.917 | 4/5 |
| Flash, 16 query heads | 4k | 5.776 ms | 4.990 ms | 0.857 | 4/5 |
| Flash, 16 query heads | 16k | 33.672 ms | 28.454 ms | 0.819 | 5/5 |
| 27B, 24 query heads | 1k | 0.980 ms | 0.891 ms | 0.909 | 3/5 |
| 27B, 24 query heads | 4k | 9.893 ms | 8.586 ms | 0.904 | 5/5 |
| 27B, 24 query heads | 16k | 59.025 ms | 51.713 ms | 0.887 | 5/5 |

Both geometries use four KV heads and dimension 256. A large operation covers
the final 2,048 query tokens; 1k covers 1,024. Short samples are noisy, and the
first 4k baseline in each geometry is slow. The 16k samples also vary over the
sequence. No hardware cause is inferred from these timings. This short probe
does not establish a sustained throughput gain or a full-model speed target.

## Resources and reproduction

One build passed in 3.67 seconds; one benchmark passed in 3.88 seconds,
including cleanup. No retries ran. The GPU guard kept its 2 GiB process cap,
14 GiB startup requirement, 12 GiB host floor, and 60-second deadline.
Peak sampled owned memory was 370,885,448 bytes. The explicit Metal-buffer
peak was 244,451,584 bytes. Threadgroup storage is 25,600 bytes, below the
32,768-byte device cap. Shared-machine load ranged from 9.88 to 10.22.
All owned groups and leases were released without sampling or cleanup errors.
No quiet lease, host setting, desktop, service, or unrelated process was used.

The source-only plan is preserved as `PLAN.md`. Source, build, executable, and
library hashes are in the receipts; no binaries or model weights are included.
Local roots are replaced by `<PROBE>`, `<SCRATCH>`, `<WORKTREE>`, and
`<OWNER_HOME>`. Restore those roots in a prepared scratch layout before using
the frozen build and guard scripts. Restore `PLAN.md` to the original
`README.md` name when checking the original build-source manifest.
`publication-manifest.json` records original and scrubbed hashes.

Run the source check, the guarded build, the guarded benchmark, and the summary
in that order. Do not change the bitwise gates or raise limits after a failure.
A product build must still pass the existing full-model raw-logit and top-answer
checks before this candidate can ship.
