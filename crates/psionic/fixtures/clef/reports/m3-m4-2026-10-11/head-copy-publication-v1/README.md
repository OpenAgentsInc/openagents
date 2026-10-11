# Bounded MPS copy and head diagnostics

This frozen snapshot records correctness experiments with PyTorch 2.9.1 on a shared Mac. No experiment loaded the 27B backbone. Timing values describe the probes and are not model performance claims.

The direct copy tests reproduced wrong outputs and source mutation in combined MPS-to-CPU dtype conversion. Both two-step transfer paths passed. The isolated lexical gathers passed all 44 cases. The complete head still showed changed input bytes in one experiment and different or nonfinite outputs in another. The exact cause and its connection to native 27B parity remain unproven; the original strict comparison remains failed.

`attempts.json` preserves both 1 GiB allocator refusals, subsequent executions, process cleanup, and resource bounds. `immutable-head-partial.json` contains five saved calls. The following call returned NaN, which the strict JSON serializer rejected before saving that call. No upload equality failure was established. No tensor payloads or model weights are included.

The reproducer files replace machine-specific paths with `CLEF_WORKTREE`, `CLEF_DIAGNOSTIC_SCRATCH`, and `CLEF_REFERENCE_PYTHON`. Set these to the source checkout, the retained `clef-m3-m4` scratch directory, and the Python executable with the pinned reference dependencies. Run only through the guarded GPU and 4 GiB memory leases; keep the 16 GiB startup margin, 12 GiB available-memory floor, and 120-second deadline. Existing result files deliberately refuse overwrite. These scripts require the retained head, metadata, and saved input files outside Git. `sources.json` records original and published script hashes plus primary-source links.
