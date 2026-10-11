# Fixed recurrence control for text029

Status: source assessment only. No implementation or workload started. This diagnostic would not change the original 40-request, 104-question all-top and 0.02 probability gate. The original same-Q4 result remains 103/104.

## Why this is informative

The completed precision control C retains the official chunk64 Delta operator. The archived native Metal path uses sequential recurrence. In the pinned Transformers source, `torch_chunk_gated_delta_rule` (lines 244–329) uses cumulative log decays, triangular elimination, and matrix products. `torch_recurrent_gated_delta_rule` (lines 332–376) decays the state, computes the correction, updates the state, and reads the updated state once per token. Both normalize Q/K with the same additive epsilon and operate in f32 for this diagnostic.

The fixed layer0 test found a small recurrence-only difference: R−A relative L2 was approximately 1.19e-7. That one layer cannot bound the effect across the model's 48 Delta layers and 16 attention layers. Each layer changes the inputs to later projections and f16 rounding boundaries. Residual and nonlinear transformations can magnify a small change. Conversely, the recurrence itself has decays and normalized keys; it is not evidence of unstable growth, and no amplification or top change should be assumed before measuring it.

One additional control is justified: D equals C in every reviewed precision boundary, except that the official recurrent Delta function replaces the official chunk64 function. This isolates the chunked versus recurrent formulation. It does not emulate the native kernel exactly. The archived Metal scan (lines 466–504) accumulates `sk` and `sq` from the decayed state, updates with FMA, and emits `sq + delta * k_dot_q`. The official recurrent operator instead dots Q with the updated state. Their reduction trees, FMA use, Q/K preparation, and output order remain different.

## Fixed implementation proposal

Use a new scratch runner and guard, preserving all executed A/B/C sources and receipts. Run only D on the same immutable text029 request, 359 exact IDs, three questions, original Q4 artifact, spans, option ordering, lexical embeddings, and f32 head.

Reuse C's one-layer-at-a-time importer, all Linear weight/input half boundaries, original key32 attention control, negative-A adapter, inference mode, and B=1/no-cache state. For each linear-attention layer, replace only the layer's `chunk_gated_delta_rule` callable with a wrapper around pinned `torch_recurrent_gated_delta_rule`. The wrapper must reject initial state, packed sequences, or unexpected keyword arguments, as the earlier layer0 wrapper does. Request the final state for hash/finite diagnostics, return `(output, None)` to the unchanged no-cache caller, and restore the original callable before releasing the layer. Retain exactly one recurrent call per Delta layer and one online-attention call per attention layer: 48 and 16 calls respectively. Do not change alpha/beta precision, conv, normalization, head weights, or any arithmetic inside the official recurrent function.

Run the already reviewed independent f64 recurrence fixtures and scalar/tensor hashing fixtures first. D layer0 must exactly equal saved `WXR_both_half_recurrent` output hash `e135fb2fa6bdb56cae5fd4c9ca4573398f8957dbae5e1c622a9310449f4d0a5f`. Layer0 has no full attention, so D and that prior WXR control have identical operations. Check the saved Delta output and final-state hashes there too, if the wrapper observes them without changing arithmetic. This validates selection and transport for the first layer; it does not validate every later layer by itself.

Before the full pass, replay the saved A and C layer63 pre-final-normalization rows through the unchanged `finish_head`. Require bitwise reproduction of their saved normalized hidden rows, lexical rows, and logits; never normalize an already normalized saved row again. This checks that the existing baseline and comparison remain intact. The recurrent operator has already passed the independent f64 fixtures and the layer0 factorial test; no new custom recurrence formula is proposed.

## Evidence and predictions

Preserve all 64 D layer outputs outside Git, their hashes, all input/parameter identities, recurrent input/output/final-state hashes, and exact call coverage. Compare D−C, D−A, and D−native at every layer for whole memory and the same question/true/false spans. Preserve final normalized hidden rows, memory, lexical rows, all three head logits and probability distributions, and both historical native modes. Keep raw arrays outside Git and publish compact summaries and hashes.

D−C must be nonzero first in a Delta layer; D layer0 must match WXR exactly. A material reduction in D−native across the trajectory would support chunked-versus-recurrent arithmetic as a contributor. Small D−C with unchanged native discrepancies would argue that this factor is insufficient. A changed single top is not sufficient evidence of a general correction. A larger difference or numerical failure is retained as a valid outcome; do not add variants or alter tolerances in response.

C's disputed P(true) is 0.4999071434150881 and native staged is 0.502091090110451. The small margin makes a top flip possible without a large trajectory improvement. This is why whole-layer and all-question comparisons are required and no prediction of the final top is made.

## Resource estimate and limits

The completed dual B/C control took 203.784 seconds, with 102.285 seconds in shared layer loading/rounding/hashing and about 27.4 seconds in each trajectory's forwards and saves. Its peak accounted memory was 5,238,472,704 bytes. Layer0 recurrent and chunked runs had comparable total times (3.635 versus 3.673 seconds for WXR and WX), including common loading and checkpoint overhead; those are not isolated operator timings.

A single D trajectory should fit an estimated 150–210 seconds, but this is an estimate, not a runtime guarantee. Retain the existing 300-second deadline, 12 GiB sampled process-group cap, 24 GiB start requirement, 12 GiB available-memory floor, two CPU threads, 2 GiB disk lease/output cap, and 25 GiB free-disk floor. No automatic retry or limit increase. One decoded f32 layer remains live at a time. Keep only hashes for recurrent states after each layer.

The 64 layer arrays require 470,548,480 raw bytes, plus one final memory/head-input export and small receipts, well below the existing 2 GiB output bound. Source/input snapshots, artifact state, post-close monitor failures, and task-owned process-group cleanup retain the completed guard's checks. No GPU, build, quiet lease, desktop app, or system service action is needed. Execution requires a separately assigned CPU window after root and peer source review.

## Pinned source evidence

- Official `modeling_qwen3_5.py`: `fd9963c32aa3d50c753a990a2670cfdbea055b6316b40027e0de976019e42551`.
- Archived Stage6 `clef_prefill.metal`: `623fba25dbe8a230d92f88e840c2c363961f926c3b908089ce8ba3688a3f2865`.
- Completed `precision_reference.py`: `7ef09fe8524fc4de689e6748b07355f324c93e5066305697854c4d7576aad694`.
- Prior `layer0_reference.py`: `dfe436ccee6c82cf5547f9b3d7a71738496cbe04e57f2a7649dda8db1b89e4de`.
- Shared `layer0_math.py`: `432a1e97996b923e4e16119d6733d9a39df604fcfcac02286eda4f62c670ee91`.
- Shared `layer35_math.py`: `bc5438f971303c6936ccd4fe61e08e8e45ff253fae56e98a434d7caaf28d8b57`.
- Completed precision publication manifest: `12a1f7fbdea0d2a32119a59e7fce0141480bc001e33f4244bed4f27352ea0811`.

A peer source assessment agreed that D isolates one remaining formulation, and that the native FMA/output-order distinction prevents treating D as exact native emulation.
