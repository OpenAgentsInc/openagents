# Fixed four-token dispatch swizzle

This is one scratch-only candidate. It has not been compiled or executed.

The candidate groups four adjacent token tiles for each output-column tile. It calls the unchanged production GEMM helper with a different threadgroup position. It keeps the existing weight layout, full dynamic K, four SIMD groups, strict precision, output type, and residual accumulation. The mapping follows the publicly documented source in the adjacent `mlx-m5-source-review` directory; no MLX implementation or private source was copied into the shader.

`production-planner.rs` is a snapshot of the current `WeightShapes` planner. Its shape matching, independent of token count, selects 64×128 for Flash gate/up `(m=12288,k=4096)` and down `(m=4096,k=12288)`. The probe mirrors that complete table, and uses the original 64×64 fallback or 64×32 kernel for small synthetic fixtures. `production-shapes.metal` is copied verbatim; the fixed-tile helper and SiLU are exact excerpts from the current production source. `manifest.json` records their original file hashes.

The fixed mapping is `logical_x=physical_x/4`, `logical_y=physical_y*4+physical_x%4`. Dispatch dimensions are `(4*ceil(m/output_tile), ceil(ceil(n/64)/4), 1)`. A uniform threadgroup guard returns before tensor slicing when the logical tile is outside the valid grid. There is no new reduction, tensor buffer, or dispatch in a projection.

## Correctness and timing

Ten synthetic checks cover both overwrite and residual modes with token counts 1, 3, 7, 65, and 257; output-column tails; and the fallback/narrow planners. Both variants must match bitwise over every finite output, and the baseline must also meet the existing `3e-5*max(1,abs(expected))` f64 oracle bound. Five real-weight checks use the existing independently decoded Flash layer-0 gate/up/down half assets: accumulate at 65/512/1024/2048 tokens, plus overwrite at 65. Every gate, up, half SiLU activation, and down output must match bitwise. All checks finish before timing begins.

All buffers have 256-byte guards at each end. Written buffers start poisoned; residual outputs start with a deterministic nonzero value. Immutable inputs and full weight buffers are hashed, including guards. All real half-weight files are checked against the pinned source manifest before use. No model is loaded and no new weight files are generated.

Five fixed ABBA rounds at each of 512/1024/2048 tokens give equal early/late positions to baseline and swizzle. Each slot records gate, up, down-accumulate, and complete gate/up/SiLU/down-accumulate GPU command times. The summary reports the sum of the three separately timed projections and the separately measured full-FFN command. Those scopes differ: the sum includes three command boundaries and omits SiLU, while the full command includes SiLU and buffer barriers. Do not treat either as full-model latency. CPU poisoning and residual reset occur before each timed command. Down-only timings use the identical baseline-produced activation for both variants. After every timed command, every relevant output is checked bitwise against the immutable baseline; full-FFN commands check all four outputs. These CPU comparisons are outside the recorded command times but remain inside the fixed wall-clock deadline. All expected baseline buffers are hashed again after the timing sequence.

The bench has exactly 273 command buffers and 492 dispatches, including preparation and correctness; 240 timing rows. A check-only run has 30 commands and 60 dispatches. This is a fixed trial count, with no warmup-based selection, parameter sweep, or retry on numerical failure. ABBA balances order but cannot eliminate shared-machine load, temporal drift, or cache effects. A material candidate requires at least 10% improvement in both paired full-FFN and projection-sum medians at every size; otherwise retain the result without integration.

## Bounds and execution

No build or GPU run is authorized by this file. Parent-agent scheduling and a separate grant are required. Never use a quiet lease.

- GPU: owned-process cap 2 GiB, available-memory preflight 14 GiB, ongoing floor 12 GiB, outer deadline 60 seconds, internal deadline 55 seconds.
- Explicit live Metal buffers: at most 1 GiB. `device.currentAllocatedSize` is logged at allocation phases and every timing row, checked before/after commands against 1.5 GiB. This is point-in-time accounting, not a guarantee against a transient private allocation. The external guard also caps the owned process group's physical footprint/RSS.
- Build: 4 GiB process cap, 16 GiB preflight, 12 GiB floor, 180 seconds, two Swift compiler threads. Build does not read weight payloads.
- The existing three 96 MiB files under `../gemm-tile-probe` are mapped and copied individually, then unmapped. No private transposed weights or additional persistent representation is created.

After authorization, use the repository's GPU/build and memory leases to launch `python3 guard.py build --output build-1`, then `python3 guard.py bench --output run-1`. The guard refuses missing leases, quiet leases, source changes since compilation, and changed executable/metallib hashes. It can signal only its newly created process group. Run `python3 summarize.py run-1` after completion; failed guards and partial results remain failed.
