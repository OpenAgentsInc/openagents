# Fixed FFN-down Q6 model acceptance

**The candidate fails the unchanged BF16 text gate.** It matches 97 of 104 top
answers, compared with 93 for the original Q4 artifact. Its maximum probability
difference is 0.169436199, and 60 questions exceed the required 0.02 limit.
All 40 cases have matching token counts. The six image cases still pass all
14 top answers, exact token counts, and ten refusal checks; maximum image
probability difference is 0.002179228.

The candidate replaces a fixed 32 of 64 FFN-down matrices with Q6_K from the
original BF16 checkpoint, following the upstream mixed-precision layer-selection
rule. It does not use acceptance errors to select layers. The generation recipe,
independent encoder/decoder checks, and all failed preparation attempts are in
`../fixed-down-q6-v1/`. Weights remain outside Git.

The runner uses the immutable Stage 6 server, SHA-256
`77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991`, with the
official external BF16 head, fused attention, weight-shape GEMM, fused field
head, sequential DeltaNet, 512-token chunks, and on-demand Q4 FFN. Q6 weights
remain resident as f16. This server contains the unused block16 experiment;
its source identity is preserved in the earlier Stage 6 bundle.

The run completes in 125.03 seconds. Sampled process-group accounting peaks at
50,296,061,952 bytes, below the predeclared 52 GiB cap. Available memory stays
above 70,666,911,744 bytes. Cleanup verifies that the owned process group has
stopped. Shared GPU and memory lease receipts are included; desktop applications
and system services remain untouched. These are quality checks, not latency
acceptance measurements.

`analysis.json` counts the unmodified per-question comparisons in `1-text.json`.
`0-images.json` preserves image and refusal results. `run.json` identifies every
artifact, argument, memory sample summary, and cleanup result. The configuration,
runner, raw memory samples, and server log are retained. Reproduction requires
the external model files identified by SHA-256 in `run.json`; path placeholders
in the published copy must be replaced with local absolute paths.

The candidate is not selected for production. A further quantization recipe
must use independent calibration and development data, then pass the unchanged
BF16 acceptance suite before it can satisfy M4.
