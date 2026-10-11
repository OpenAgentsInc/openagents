# Independent SIMD-group projection plan

This organization has not been tested in the retained projection experiments. No implementation, compilation, weight read, or GPU execution is authorized by this plan.

Apple's March 2026 [MPP guide](https://developer.apple.com/download/files/Metal-Performance-Primitives-Programming-Guide.pdf) describes independent SIMD-group output tiles, a 2×2 arrangement of 32×32 tiles as an initial half-input configuration, and periodic synchronization of cooperative accumulation at K intervals. These are starting points, not measured gains for Clef. The guide's code and text are not copied here.

The earlier serial-K controls in `q4-tensor-prototype/candidates.metal` instead use one four-SIMD 64×64 operation, K=256, and a shared weight tile. The affine uint4 path is also a four-SIMD operation. The other examined projection probes use threadgroup-scoped operations. [The source audit](source-audit.json) records their hashes.

Implement only one candidate: four independent 32×32 products forming a 64×64 threadgroup tile, direct device half inputs, K=128 steps, f32 cooperative accumulators and outputs. Preserve the existing NT interpretation and disable relaxed precision. Initialize overwrite accumulators to zero; load the existing residual once before the first step in accumulate mode. Store once afterward. Do not add a final residual or create device partial matrices.

Each SIMD group owns a disjoint tile determined by its group index. Every thread must reach every common K-step barrier, including SIMD groups whose output tile is entirely outside a tail. Such groups skip tensor operations inside the loop, never return before a barrier. Static slices are limited to complete subtiles; partial views have explicit remaining K/row/column extents and parent row strides. No shared staging or walk-order change is included.

Compare against the current production planner on the existing real Flash gate/up/down assets. Initial checks must scan every output with fixed absolute error at most `0.001`, test overwrite/nonzero-residual modes, row/column/K tails, and cancellation. Small fixtures must check every output against f64 at both real K values (4096 and 12288), in addition to odd K tails; real-weight fixtures use fixed independent f64 samples. Freeze both variants' finite outputs after those scans; every timed repeat must match its own immutable expectation bitwise. Hash expectations and inputs afterward.

Keep five ABBA rounds at 512/1024/2048 tokens, separate projection and full-FFN timings, and the existing 10% paired improvement gate. Retain 2 GiB/start14/floor12/60-second bounds; no retries, variants, or increased limits. A pass still needs weighted model-logit validation. The different K reduction can fail numerical gates, and extra barriers or smaller output tiles can outweigh any benefit. This experiment cannot establish a cache or scheduling cause by itself.
