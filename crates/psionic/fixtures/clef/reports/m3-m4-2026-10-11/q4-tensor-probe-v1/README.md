# Q4 tensor probe: rejected candidates

Keep the existing projection kernels. Neither candidate improves this test: exact Q4_K tile decoding is about 11 times slower, and native uint4 with affine corrections is about 6 times slower and fails the fixed synthetic accuracy gate.

| Rows | Fixed64 MPP | Weight-shape MPP | Expanded f16 K256 control | Q4_K → f16 tile | Native uint4 + affine |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1,024 | 5.406 ms | 5.344 ms | 43.779 ms | 63.217 ms | 33.626 ms |
| 2,048 | 12.406 ms | 12.033 ms | 87.390 ms | 136.039 ms | 70.511 ms |

These are sums of the three projection median GPU times, not complete FFN or model latency. Each median has four samples with alternating variant order. The shared machine's one-minute load average was about 12.7. Actual layer-0 Flash gate/up/down weights were tested with deterministic activations at 1,024 and 2,048 rows. Down received the same baseline gate/up SiLU input for every variant.

## Accuracy

The scan checked all 88,080,384 weighted output values per variant. Exact tile decoding and its expanded-f16 K256 control matched fixed64 **bit for bit** in every case. Six full decoded-weight checks also matched the independent half-weight files bit for bit. This supports the intended weight arithmetic in these cases; it does not establish full-model acceptance.

Native uint4 plus affine correction changed almost every weighted output. Its largest weighted difference was 0.0006187, below the fixed 0.001 projection screen. However, it failed all four small cases with nontrivial scale/min metadata, with maximum absolute error 0.0103413. Those failures are retained and reject the candidate. Raw-uint4 capability cases passed the independent f64 oracle, confirming packing/API behavior for those cases. Raw full-K uint4 throughput was not measured, so these results do not establish whether integer multiplication executes directly in hardware or converts internally.

The small suite completed 44 comparisons: 40 passed and four affine cases failed. The weighted run completed all cases and exited 2 because those numerical failures remain failures. No thresholds changed, no cases were dropped, and no full-model run was attempted.

The K256 expanded-f16 control is itself about 7–8 times slower than fixed64. Most of this prototype's cost therefore appears before Q4 decoding is added. The experiment does not identify the hardware cause or establish that another compressed-weight kernel would be slow.

## Execution and retained failures

- Staging verified all three packed-weight and independent half-weight hashes and every repacked nibble. It completed in 0.83 seconds.
- Build 1 failed on shader syntax: an outdated unroll-pragma spelling from an SDK comment and mixed scalar/vector dispatch-position attributes. Its logs and exact source files are included. The corrections changed no algorithm or tolerance.
- Build 2 passed in 3.10 seconds. The original binary SHA-256 is `f26dc7935466aa5906c6fa6a4184d3a7522f31b40617bfb7b8d015153cf35423`.
- The small GPU run completed in 1.45 seconds and the weighted run in 7.02 seconds. Both retained numerical exit 2. All source-hash, finite-output, redzone, and owned-process cleanup checks passed.
- GPU runs used GPU and 2 GiB memory leases, a sampled 2 GiB process-group cap, at least 14 GiB available before launch, a 12 GiB host floor, and a 60-second deadline. Weighted sampled peak was 0.916 GiB; the lowest observed host-available memory was about 99.6 GiB. No bounds were raised.
- No quiet lease, desktop/service control, model run, or product source change occurred. All owned children stopped and leases were released.

`summary.json` contains full error summaries and individual timing ranges. Raw numeric rows, guard receipts, build logs, and sampled memory records remain in the run directories. `design-before-run.md` preserves the original source-only plan. `sources.json` records the installed SDK and primary Apple references. The failed candidates do not change any product default or acceptance claim.

## Reproduce

Copy this bundle into a persistent scratch directory. Use the same Apple SDK and a NumPy Python environment. Set `OPENAGENTS_WORKTREE` to the repository, `CLEF_NUMPY_PYTHON` to that Python executable, `CLEF_FLASH_GGUF` to the verified public GGUF, and `CLEF_HALF_DIRECTORY` to the previously decoded half files. `source-weights.json` pins the GGUF, tensor offsets, packed/half hashes, and independent decoder. The weights are not included.

From the copied directory:

```sh
openagents lease memory --amount 2 -- python3 guard.py stage --output stage-reproduction
openagents lease build --keep-target-dir -- openagents lease memory --amount 4 -- python3 guard.py build --output build-reproduction
openagents lease gpu -- openagents lease memory --amount 2 -- python3 guard.py check --output check-reproduction
```

Preserve the small-run failures. If the result is only the documented numerical rejection and all safety checks pass, a separate weighted diagnostic may run under the same bounds:

```sh
openagents lease gpu -- openagents lease memory --amount 2 -- python3 guard.py bench --output bench-reproduction
```

An unsupported API, memory failure, nonfinite result, corrupted guard, source mutation, or timeout must stop further work rather than raising bounds. The runner refuses unsupported K tails and validates pipeline thread count, SIMD width, and threadgroup memory before dispatch. The existing full-model raw-logit error ≤0.001 and complete top-answer agreement gates remain unchanged.

Published personal paths are replaced with placeholders; the published guard takes explicit environment paths. Original scratch files remain unchanged. `manifest.json` records original source/binary hashes and the published file hashes. No weights or executable binaries are in this bundle.
