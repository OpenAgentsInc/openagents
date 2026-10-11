# MPS matrix multiplication probe

MPS with f16 inputs and f32 output matched the frozen MPP output bit for bit for all six tested Flash projections. Its timing improvement was small. Keep the product implementation unchanged: this probe does not establish a worthwhile improvement over the current weight-shape kernels.

The test used actual layer-0 Flash gate, up, and down weights at 1,024 and 2,048 rows. Inputs were deterministic synthetic values. Every down variant received the same f16 SiLU activation computed from the baseline gate and up results. This compares individual projections, not a complete model or a chained candidate FFN.

| Rows | Fixed64 MPP | Weight-shape MPP | MPS f16 → f32 | MPS f32 → f32 | MPS f16 → f16 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1,024 | 5.106 ms | 5.130 ms | 4.913 ms | 22.068 ms | 5.196 ms |
| 2,048 | 10.632 ms | 10.194 ms | 10.185 ms | 45.375 ms | 10.928 ms |

These values are sums of the three projection median GPU times, with four samples per projection and variant. They are **not end-to-end FFN or model latency**. Sample order alternated forward and reverse. The shared machine's one-minute load average was about 10.56. See `summary.json` for individual medians, ranges, wall time, and full error statistics.

MPS mixed f16/f32 reduced this sum by 3.8% and 4.2% against fixed64. Against the weight-shape kernels, the differences were 4.2% and 0.09%. This short run cannot establish sustained performance or a reliable benefit this small. MPS same-type f32 was about 4.3 times slower. MPS f16 output added rounding error: the down projection differed by up to 0.001940, with 97 and 195 values exceeding 0.001. That variant is a reduced-precision diagnostic, not an accepted replacement.

## Checks and bounds

- The isolated mixed-type capability run passed eight small, independent CPU-f64 oracle cases, including odd dimensions and residual addition. The weighted run repeated these cases for all five variants: 40 checks passed.
- All weighted outputs were finite. Redzones and source hashes passed. Outputs and the f16 intermediate were poisoned with NaNs before correctness calls, outside timed rounds.
- Mixed f16/f32 and weight-shape MPP outputs matched fixed64 bit for bit. Full-f32 MPS differed by at most 0.00002671 on the weighted fixtures.
- Down restores the same f32 residual before each call. MPS f16/f32 and f32/f32 use `beta=1`. The f16-output variant uses `beta=0`, widens the result, and adds the f32 residual afterward.
- Timings include activation widening for f32 MPS and residual restore/post-add where required. One-time f32 weight widening is excluded.
- Each GPU run used GPU and 2 GiB memory leases, a 2 GiB sampled process-group cap, at least 14 GiB available before launch, a 12 GiB host floor, and a 60-second deadline. No quiet lease or desktop controls were used.
- The weighted run completed in 4.77 seconds, with a sampled process footprint peak of 1.483 GiB. Its owned process group stopped cleanly. This sampling guard is not a hard allocation limit.
- No product source, defaults, or numerical gate changed. Full-model raw-logit error ≤0.001 and complete top-answer agreement remain required before integration.

Build 1 compiled successfully but was not run on the GPU. Review then added output poisoning and the weight-shape comparator. Build 2 and both GPU runs completed successfully. Both build receipts remain included.

## Reproduce

Use the same Apple SDK and an MPP-capable device. Set `OPENAGENTS_WORKTREE` to this repository and copy this directory into the directory reported by `openagents scratch`. Place `gate.f16`, `up.f16`, and `down.f16` beside `probe.swift`, using the exact sizes and hashes in `weights.json`. The weights are not included. That manifest records the public GGUF and pinned independent Q4_K decoder used to produce them.

From the copied directory, run:

```sh
openagents lease build --keep-target-dir -- openagents lease memory --amount 4 -- python3 guard.py build --output build-reproduction
openagents lease gpu -- openagents lease memory --amount 2 -- python3 guard.py mixed-check --output mixed-reproduction
```

Only if the mixed capability check completes successfully, run:

```sh
openagents lease gpu -- openagents lease memory --amount 2 -- python3 guard.py bench-mixed --output weighted-reproduction
```

If mixed types fail, retain that failure and use `bench` instead of `bench-mixed`; do not treat mixed support as established. The inspected Apple SDK documents row-major matrices, transposition, and `alpha * A * B + beta * C`, but does not explicitly guarantee mixed input/output types across devices or OS versions. The result here establishes support for these cases on this device only. Primary-source links and source hashes are in `sources.json`.

`manifest.json` lists the original binary and source hashes and the hashes of published files. Personal absolute paths were replaced with placeholders; the published guard reads `OPENAGENTS_WORKTREE`. Original scratch records remain unchanged. No weights, hidden activations, or compiled binaries are included in this publication.
