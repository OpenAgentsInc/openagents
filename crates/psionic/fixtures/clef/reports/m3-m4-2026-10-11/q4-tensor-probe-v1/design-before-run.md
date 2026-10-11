# Q4 tensor projection candidates

Status: sources prepared only. No compilation, weight staging, GPU execution, model run, or product edit has occurred for these candidates.

The purpose is to test a different mechanism from the earlier tile/layout/MPS probes: keep Q4_K weights compressed until each matrix tile consumes them. A result is useful only if the full error scan and unchanged full-model gate pass. No speedup is claimed.

## Two candidates and controls

1. `q4_tile_f16` decodes the original packed Q4_K block into a 64-output × 256-K threadgroup half tile. The expression is the current `half(fma(d * scale, q, -(dmin * min)))`. It uses the current four-SIMD TensorOps scope, a persistent f32 result, and a K=256 loop. The tile costs 32 KiB per threadgroup; pipeline creation must refuse if it exceeds the actual device limit. It eliminates the full expanded-weight reads and the on-demand global conversion slot. A register-only input is deferred because the installed API requires a single SIMD group, static K, and no input transpose, unlike the existing kernel.
2. `native_uint4_affine` repacks Q4_K nibbles without changing them, with f32 scale and negative-min planes for each group of 32 K values. Each native half×uint4 K=32 product receives `scale * partial + offset * sum(X)`. Activation sums are a separate timed GPU dispatch. This algebra omits the current per-weight half rounding and changes reduction order. It is deliberately an approximate candidate, not a lossless implementation.

`expanded_tile_f16` uses the same K=256 loop and threadgroup tile with the already decoded half weights. It separates reduction-order changes from decoding errors. Frozen fixed64 and weight-shape MPP kernels are the references. `native_uint4_raw` checks packing and API semantics against scale=1/min=0 cases; its output is not a valid model projection for ordinary Q4_K weights.

Every full weighted decode is compared bit for bit with the independently decoded half file before projection tests. A decode mismatch stops the run. Down uses the same baseline gate/up SiLU input for all variants. The MPP reference retains its current f32 residual accumulation kernel; candidate residual additions occur after their K loop, so that difference remains included in the error measurement.

## Why int4 or BF16 is not a free replacement

The installed macOS 26.5 headers explicitly support half×int4/uint4→float. They route to distinct external implementations but do not expose the machine instructions or guarantee a speed advantage over half. Apple describes automatic use of available hardware; this does not establish integer arithmetic throughput on this Mac. A capability test and measurement are required. [Apple TensorOps presentation](https://developer.apple.com/videos/play/wwdc2026/330/)

Q4_K contains per-group scale and min values. Feeding its nibble bytes directly into a raw integer multiply loses those values. Moving scale/min outside a reduction also removes the per-weight f16 rounding used by this engine. BF16 remains 16 bits and drops mantissa precision relative to these existing f16 values; the inspected sources provide no speed guarantee over f16. No BF16 conversion or relaxed-precision setting is proposed.

The newer block-scaled tensor planes in Apple's presentation require APIs absent from the installed 26.5 `MTLTensorDescriptor`. The current prototype needs no SDK or system changes. Header hashes and the distinction between documented support and unproved implementation details are in `sources.json`.

## Fixed validation plan

- Small cases: `(n,m,k)` of `(1,1,256)`, `(17,65,256)`, `(3,7,256)`, and `(17,65,512)`, each with and without residual. The first two use exact raw-uint4 weights; the others include scales/minima. An independent CPU-f64 dot product evaluates the actual rounded-half weights. Baseline and lossless-weight cases use the predeclared scaled 3e-5 small-kernel bound; affine cases additionally need absolute error ≤0.001.
- Real weights: all layer-0 Flash gate/up/down outputs, at n=1,024 and 2,048. Weight shapes are 12,288×4,096 and 4,096×12,288. Fixed64, current weight-shapes, expanded-half K256, decoded-half K256, and native-uint4 affine are compared without choosing or omitting outputs.
- Record maximum absolute/scaled error, RMS/relative RMS, bit differences, and every count above 0.001. Outputs must be finite. The full-output absolute 0.001 projection screen is fixed in advance. Numerical failures do not stop later cases from being recorded; the process exits 2 after all cases if any fail.
- Poison output/intermediate data before correctness calls. Check front/back redzones and hashes of all weights, inputs, and residual sources. No aliases are intentional. K must be a multiple of 256; unsupported partial-K shapes are refused rather than silently padded. N/M tails are included in the small cases.
- Four timing samples per variant/projection, alternating forward/reverse order. Include activation sums and residual operations. Record GPU and wall time and host load. A sum of projection times is not model latency.
- Only after a candidate wins the bounded projection test and passes its numerical screen: run the same frozen model corpus with raw-logit error ≤0.001 and 100% top-answer agreement. No tolerance changes, new checkpoint, or quantizer fitting are permitted to make this pass.

The prepared GPU guard uses a 2 GiB process-group cap, start with at least 14 GiB available, keep a 12 GiB host floor, and stop at 60 seconds. Compiles use a build lease and 4 GiB cap. All runs require an explicit handoff from the coordinating agent. No quiet lease, desktop control, model load, or system setting is part of this probe. Do not raise the bounds if the candidate is too slow or large.

`stage.py` is prepared but not run. It reads only the three verified packed projections from the original public GGUF, checks their hashes, checks the existing independent half files, and writes raw Q4, native nibble, and scale/min files; the half weights are symlinked. New weight files total about 189 MiB, and no complete model is read. It requires NumPy and a separate bounded I/O window before the run. `build.py` and `guard.py` compile/run the scratch sources only. The runner expects staged files in `artifacts/` beside its sources.

## Independent scheduling opportunity

The serving path already uses a concurrent encoder, but `ClefMetalBatch::before_dispatch` emits a buffer-wide barrier by default. Apple documents that this waits for prior dispatches in the encoder to finish. Three known-independent projection groups currently remain serialized:

- Hybrid qkv/alpha/beta at `clef_metal.rs:1685`.
- Attention query/key/value at `clef_metal.rs:1780`.
- FFN gate/up at `clef_metal.rs:1864`, **only when the two weights are resident f16**. The on-demand Q4 path reuses one conversion slot, so gate/up cannot overlap there.

The smallest scheduling test keeps the initial norm dependency and final consumer barrier, and skips only barriers between these independent projections using the existing explicit helper. No allocation increase is needed. The safe test uses the same inputs, disjoint outputs, poisoned buffers, all-output exact-bit checks, and alternating serialized/concurrent timings under the same bounded guard. This may remove avoidable stalls, but the source does not show that it explains the reported sustained throughput drop. Concurrent large GEMMs can also contend for the same compute resources; no large win is assumed.
