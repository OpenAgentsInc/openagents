# Two SIMD groups for the existing Flash projection tile

Source preparation only. Do not build or run without the parent's separate execution grant. No product code changed.

This experiment changes exactly one execution configuration: `execution_simdgroups<4>` becomes `execution_simdgroups<2>`, and the host dispatches the matching 64 threads instead of 128. The tile remains 64 token rows by 128 output columns. Input half values, weight layout, descriptor orientation, full dynamic K, f32 outputs, residual accumulation, static interior slices, dynamic edge slices, and tile-grid order remain unchanged. There is no split-K work, swizzle, precision change, or extra reduction dispatch.

The installed public `MPPTensorOpsMatMul2d.h` describes `execution_simdgroups<N>` and requires the dispatched group count to match. It does not require four groups. This establishes API support, not a speed gain. Fewer participating threads might change scheduling overhead or resource use; increased per-thread state might instead make the operation slower. Neither mechanism is measured yet. A previous eight-group candidate was slower, but that does not establish the result for two groups.

## Correctness before timing

The actual Flash layer-0 gate, up, and down matrices use the unchanged production weight-shape planner, which selects 64×128 for all three. Synthetic cases force the same production 64×128 baseline helper so their comparison changes only the group count. They cover token counts 1/3/7/63/64/65/257, row and output-column tails, odd K values 1/65/127/257, real K values 4096/12288, zero values, nonsymmetric values, correlated inputs, cancellation, and mixed ranges. Every case runs in overwrite and residual modes.

All 14 synthetic checks compare every output against both the four-group baseline and an independent f64 dot product. Five real-weight checks use residual mode at 65/512/1024/2048 tokens plus overwrite at 65. Every gate, up, half activation, and down output must be finite and within the fixed absolute `0.001` bound. Bit differences are reported, not silently treated as exact equality. Each real-weight projection also checks 96 predetermined f64 dot products. Down's oracle uses each variant's own half activation, while full-output comparison additionally measures any propagation from gate/up differences. All 19 checks finish before timing.

Every buffer has 256-byte guards at both ends; outputs are poisoned or filled with the same nonzero residual before every dispatch. The host validates bounded positive shapes and exact binding lengths before encoding. Pipelines must have SIMD width 32, enough threads for their declared group count, and supported static threadgroup memory. Input, weight, and immutable expected-output hashes include their guards.

## One fixed timing plan

Run five ABBA rounds at each of 512, 1024, and 2048 tokens, comparing four groups with two. Time gate, up, residual down, and the full gate/up/SiLU/down sequence. Every timed output is checked against the immutable baseline immediately after completion. Down-only timing uses the same baseline activation in both variants. Reset, hashing, and CPU comparisons are outside GPU command timestamps and inside the wall-clock bound. Both variants use identical buffer barriers and one completed command buffer per timed scope.

The frozen bench totals are 281 command buffers, 500 dispatches, and 240 timing rows. Check-only mode has 38 commands and 68 dispatches. A useful candidate must improve both paired full-FFN and projection-sum medians by at least 10% at all three sizes. Keep every sample and paired ratio. This shared-machine microbenchmark cannot establish full-model acceptance, end-to-end latency, or a thermal, clock, or scheduling cause. There is no one-group candidate or variant sweep.

## Resource limits

Explicit live buffers peak at 889,198,592 bytes. Guard process accounting at 2 GiB, require at least 14 GiB available before launch, retain a 12 GiB available-memory floor, and stop at 60 seconds. The child also checks a 55-second internal deadline and a 1.5 GiB point-in-time `device.currentAllocatedSize` limit. Point samples cannot prove a transient allocation ceiling. Build limits are 4 GiB/start 16 GiB/floor 12 GiB/180 seconds with two Swift compilation threads.

The guard requires build or GPU leases plus the appropriate memory lease, refuses quiet leases, verifies source and executable hashes, strips injected dynamic-library environment variables, and controls only its newly created process group. No desktop app or system service is involved. Source preparation and compilation do not read weight payloads. The three existing hash-verified 96 MiB half-weight assets remain in `../gemm-tile-probe`.
