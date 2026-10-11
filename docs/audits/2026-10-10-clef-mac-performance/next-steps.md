# Next work, in order

This round establishes correctness of an experimental submission change and
exposes a measurement problem. It does not establish an idle-Mac latency result
or a thermal explanation. Keep [#11196](https://github.com/OpenAgentsInc/openagents/issues/11196)
open until its acceptance conditions are measured.

1. **Establish an idle GPU baseline.** Use the new harness's default ten-minute
   CPU and GPU admission checks. Keep the desktop responsive. If the GPU stays
   busy, end the attempt and retain the refusal; do not suspend applications or
   stop WindowServer. Record power mode and AC status. Alternate baseline and
   candidate in an evenly counterbalanced order, using at least four rounds.
   Preserve actual token counts, fresh states, all samples, startup separately,
   and the quiet lease receipt. The serial HTTP run also retains the existing server logs for
   admission wait and execution time. Those logs round to milliseconds; expose
   the same timing split in structured responses before measuring concurrent
   scheduling. Do not label the whole HTTP duration as pure model time.

2. **Resolve sustained GEMM throughput before selecting a kernel change.** Run
   the retained repeated/distinct-weight probe with an idle GPU. Compare short
   and long command buffers, reverse their order, and test the actual projection
   dimensions used by the model. GPU timestamps describe execution under the
   current scheduler; they do not establish clock frequency or thermal
   throttling. If approved privileged telemetry is available later, correlate
   clock, power, temperature, and competing process activity with each sample.
   Useful performance work does not require guessing those values.

3. **Retest the one-submission candidate.** The exact patch is retained in
   `candidate.patch`, including stronger chunk and observer parity checks. It
   removes the wait between each chunk's layers and its head-input projections.
   The current results are confounded by desktop GPU activity and substantial
   drift. Require repeatable gains on paired inputs without a 4k or 16k
   regression before changing the default runtime.

4. **Fuse the existing TensorOps attention path.** The current code already
   uses `mpp::tensor_ops::matmul2d` for its dense projections and attention
   products. The opportunity is in `clef_attn_scores`,
   `clef_causal_softmax_to_f16`, and `clef_attn_pv`: three dispatches per query
   head, with full score and probability buffers between them. Prototype tiled
   QK, online softmax, and PV together, retaining f32 accumulators and the causal
   mask. Check absolute-position tile boundaries, wholly masked tiles, short
   inputs, and chunk equivalence against the CPU reference before timing the
   full model. Keep profiling runs separate from latency comparisons.

5. **Tune GEMM shapes against real dimensions.** The current common projection
   path uses 64 by 64 tiles and four SIMD groups. Compare candidate shapes at
   the model's actual token/output/input dimensions, including incomplete edge
   tiles. A higher rate on one synthetic matrix is insufficient: require a
   lower unprofiled end-to-end latency with numerical checks intact.

6. **Revisit the parallel DeltaNet step after the GEMM baseline is stable.**
   The earlier scalar WY experiment was correct but slower. Use matrix/tensor
   operations for block work, preserve state transitions and absolute chunk
   boundaries, and compare against the staged scan. Treat the scan's measured
   share of total latency as the upper bound on the benefit available here.

7. **Confirm configuration candidates, then tackle throughput.** Chunk-size
   and Rayon-thread results in this round are one-order screening measurements.
   Retest any promising setting against a default control in alternating rounds;
   do not choose a default from the lowest single sample. After stable latency
   and quality checks, batch independent requests with deadlines and measure
   queue delay, throughput, and tail latency together. Prefix reuse is a
   separate experiment and must not enter uncached latency claims.

The original issue targets remain 0.35 seconds at about 1k tokens and 7.5 seconds
at about 16k, with a same-session comparator. The more ambitious 4k and 16k goals
from the broader plan remain targets, not outcomes from this report. Prepared
answer accuracy, router prompt reduction, and the RTX 4080 changes are separate
workstreams; these Mac measurements neither complete nor invalidate them.
