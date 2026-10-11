# Rejected block16 register-column solve

The candidate preserved every checked output but was slower in every paired
timing comparison. Keep the existing block16 implementation. No product source
changed, and no full model ran. The cause of the slowdown is not established.

## Results

All 24 correctness checks passed: 16,081,920 output and final-state values were
bitwise identical to the frozen block16 shader. The 22 numerical fixture
schedules also passed the existing independent f64 bound; the largest absolute
error across those comparisons was 8.394025870217536e-7. The two larger checks
used the real Flash dimensions with deterministic correlated inputs.

| Tokens | Scope | Baseline median | Candidate median | Median paired ratio |
| --- | --- | ---: | ---: | ---: |
| 1,024 | Preparation and scan | 0.975 ms | 1.491 ms | 1.530 |
| 1,024 | Scan only | 0.877 ms | 1.405 ms | 1.600 |
| 2,048 | Preparation and scan | 1.989 ms | 3.066 ms | 1.539 |
| 2,048 | Scan only | 1.742 ms | 2.811 ms | 1.615 |

Each group has six paired rounds with alternating order. The candidate lost
all 24 pairs. `run-1/stdout.jsonl` preserves every check and all 48 individual
timings in their original order, including GPU start/end times. The one-minute
load average was 14.65–14.71. These short shared-machine measurements do not
establish sustained model throughput, clock behavior, thermal limits, or
memory-bandwidth limits. `run-1/summary.json` reports successful execution and
correctness; that `passed` field is not a speedup verdict. The separate
`recommendation.json` records rejection of the optimization.

The shader changes only the triangular solve. One SIMD lane holds a value
column's 16 corrections and applies the same ascending-pivot `fma` operations.
All threads join the publication barrier before the unchanged TensorOps calls.
This reduces the scan's threadgroup barriers from 23 to 8 per full block.
Both scan pipelines reported the same 20,480 bytes of static threadgroup memory.
That reduction did not improve the measured running time.

## Coverage and resource bounds

The fixtures cover tails around 16 and 32, both value-head orders, zero decay
on either side of a block boundary, underflowing products, one-token chunks,
correlated keys, and 2,032 tokens carried through nonaligned 257-token chunks.
The independent f64 recurrence uses the existing
`2e-5 + 2e-4 * abs(expected)` bound. The actual-shape checks use 16 key heads,
32 value heads, dimension 128, convolution width 8,192, and value offset 4,096.
All input values remain unchanged, output buffers start with sentinel values,
and surrounding buffer guards remain intact.

The complete fixture data estimate is below 512 MiB. The owned process group's
sampled peak was 621,233,160 bytes, including runtime overhead, under the 2 GiB
cap. Execution took 11.129 seconds under a 60-second deadline. The guard required
14 GiB available before launch and maintained a 12 GiB host-memory floor.
The build took 2.072 seconds under a 4 GiB cap, with two Swift compiler threads.
Both build and GPU runs used memory leases. Every lease was released, the owned
child group stopped, and no sampling or cleanup errors occurred. No quiet lease,
desktop-app control, or system-service control was used.

## Evidence map

- `baseline.metal`: exact frozen product shader.
- `kernels.metal`: block16 definition, unchanged baseline, and renamed candidate.
- `source.json`: original shader and checked-in fixture-source hashes.
- `compiled.json`: original executable, AIR, source, and tool hashes. The
  executable and AIR are excluded from this source-and-evidence bundle.
- `probe.swift`, `build.py`, `guard.py`, and `summarize.py`: reproduction tools.
- `build-1/` and `run-1/`: complete stdout, stderr, and guarded receipts.
- `*-lease-1.json`: admission, release, and exit receipts.
- `manifest.json`: hashes and byte lengths of original and published files,
  including every path substitution. Existing run evidence was not overwritten.

No weights, model artifacts, large memory timelines, or unrelated process
commands are included. Private filesystem prefixes are replaced with artifact
labels. Original hashes embedded in receipts remain unchanged. The published
`guard.py` obtains the repository path from `OPENAGENTS_REPO`; its original and
published hashes therefore differ as recorded in the manifest. The two guard
dependencies remain in the repository and their exact run hashes are in the
receipts. All Swift and Metal source bytes are unchanged.

## Reproduction

Use a Mac with the required Metal TensorOps SDK. Set `OPENAGENTS_REPO` to the
matching repository checkout. Within this bundle directory, after coordinating
resource availability, use a build lease and a 4 GiB memory lease to invoke
`python3 guard.py build --output build-reproduction`. This compiles the shader
offline and builds Swift; it does not execute GPU work.

After obtaining a separate GPU window, use a GPU lease and a 2 GiB memory lease
to invoke `python3 guard.py bench --output run-reproduction`. Then invoke
`python3 summarize.py run-reproduction`. The output directory must be new.
The guards refuse quiet leases and never overwrite an existing run directory.
Reproduction timings are new evidence, not replacements for the original run.
