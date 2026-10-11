# Three-part parallel K reduction for Flash down

Source implementation only. Parent source review and an execution grant are required before a build or GPU run. No product code changed.

This candidate replaces only the Flash down projection (`K=12288`) with three independent K partitions of 4096 elements and one f32 reduction. Gate, up, and SiLU remain the production kernels. The original weight-shape planner selects 64×128 for all actual Flash gate/up/down shapes. Synthetic small cases retain the production fallback baseline, while the candidate uses its fixed 64×128 partial tile.

The source rationale and pinned public MLX references are in the adjacent `mlx-m5-source-review/split-k-plan.md`. The partition count follows MLX's 4096-element partition rule for Flash down. This is parallel K work across threadgroups, unlike the rejected serial K loops. It adds partial writes/reads and one dispatch, so a gain is uncertain. The separate five-part 27B route is outside this experiment.

## Exact memory views and reduction

`candidate.metal` constructs each input view at the selected K offset with explicit extents `(4096, rows)` and element strides `(1,12288)`. The installed public `metal_tensor` header provides this pointer/extents/strides constructor. Even a dynamic token-tail slice retains K extent 4096 and cannot read the remainder of the parent K dimension. The largest reachable offset is `(rows-1)*12288 + partition*4096 +4095`, within the original allocation.

The grid is one-dimensional, partition-major, with the original row/output tile order inside each partition. Three distinct f32 buffers each have 256-byte guards at both ends. Each threadgroup owns one output tile in exactly one partial plane. A buffer barrier precedes a separate reduction kernel. The reduction computes `(p0+p1)+p2`, then adds the residual once when enabled; it has no atomics or half conversion. Only `reduce.metal` is compiled with `-fno-fast-math`; production and partial matrix sources keep the original compilation options. It is linked into the same offline metallib after separate compilation.

Host shape checks refuse K other than 12288, zero dimensions, more than 2048 tokens, or more than 12288 columns before allocating partial buffers. Binding lengths are checked before encoding. The shader also returns uniformly on invalid dimensions or partition IDs. Tail views retain the finite row/column parent bounds rather than extending static tiles.

## Frozen correctness checks

Fourteen full synthetic checks cover zero, nonsymmetric, correlated, cancellation, mixed-range, and partition-distinguishing inputs in both overwrite and residual modes. Token counts are 1/3/7/63/64/65/257, with small output-column tails. Every output and every partial is checked against an independent f64 reference. Every full candidate output is also compared with production. All these numerical comparisons retain fixed absolute error ≤`0.001`; there is no relative fallback or threshold fitting.

Five real-weight checks use the existing independently decoded Flash layer-0 gate/up/down assets: residual mode at 65/512/1024/2048 tokens, plus overwrite at 65. Every gate, up, and half activation output must be bitwise identical. Every final down output must be finite and within the same absolute `0.001` bound. Ninety-six fixed whole-dot and per-partial f64 samples provide an independent oracle. The three partials are guarded separately and poisoned before each candidate call.

All 19 checks finish before timing. Inputs, weights, and immutable expected outputs are hashed including guards. Failures retain their coordinates, maxima, or nonfinite bit patterns where available. A microbenchmark pass does not establish full-model weighted parity.

## Fixed timing and limits

Five ABBA rounds at each of 512/1024/2048 tokens compare one baseline and one candidate. The full-FFN command includes the candidate partial/reduction dispatch and barrier. Separately timed down also includes both candidate dispatches in one command buffer. Gate/up timings use unchanged production code in both slots. After every timed command, all relevant outputs are checked; CPU comparisons and reset are outside GPU command timestamps but remain inside the deadline. Immutable baseline buffers are hashed again after each size.

Expected bench totals: 281 command buffers, 579 dispatches, 240 timing rows. Check-only totals: 38 commands and 87 dispatches. The summary requires exact check/timing-key coverage. A material candidate requires at least 10% improvement in both paired full-FFN and projection-sum medians at every size. Shared-machine load, cache effects, and temporal drift remain limitations. There is no partition-count sweep or automatic retry.

Explicit worst-case live buffers are 989,863,424 bytes, including all three guarded partials, below 1 GiB. Keep process 2 GiB/start 14 GiB/floor 12 GiB/wall 60 s, internal 55 s deadline, and 1.5 GiB point-in-time `device.currentAllocatedSize` checks. The build cap remains 4 GiB/start 16 GiB/floor 12 GiB/wall 180 s with two Swift threads. The owned-process guard also checks physical footprint/RSS. Point-in-time Metal allocation measurements cannot prove a transient private-allocation ceiling.

After a separate grant, run the existing guarded build and one bench under build/GPU plus memory leases. Never use a quiet lease. The guard checks source and executable/metallib hashes, confines outputs to scratch, preserves partial failures, and can signal only its newly created process group. No weight payload is read during source preparation or compilation. The existing three hash-verified 96 MiB half-weight files remain in `../gemm-tile-probe`.
