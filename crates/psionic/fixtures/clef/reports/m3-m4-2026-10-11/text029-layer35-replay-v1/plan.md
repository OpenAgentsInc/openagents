# Layer 35 same-input replay

The full native capture matches the prior normal-request raw head logits within 2.9e-6. Head and pooling differences therefore do not explain the answer flip. The independent CPU and MPS references agree. Layer 35 is a full-attention layer where the false-option span relative error increases from about 0.00326 to 0.00610. This replay studies that increase without changing any acceptance input or threshold.

The three runs load only layer 35 of the unchanged original Q4 artifact:

- A uses the saved independent CPU layer 34 output and unchanged official f32 operators. Its layer 35 result must reproduce the saved independent CPU output bit for bit before any interpretation.
- B uses the saved native layer 34 output and the same official f32 operators. It separates propagated input differences from changes introduced within this layer.
- C uses exactly B's input, but rounds each decoded Linear weight and Linear input to f16 before f32 multiplication. That models native normalized projection inputs, gated attention inputs, FFN SiLU-product inputs, and all seven weight matrices. Norm parameters and residual rows remain f32. It also rounds scaled rotary Q, rotary K, and V as native attention preparation does, then models the original 32-key online updates with half-rounded unnormalized exponentials and the eight-lane denominator sum tree.

C is a diagnostic arithmetic model, not an alternate correctness oracle. CPU GEMM reductions, RMSNorm reductions, transcendental functions, fused multiply-add, and the placement of residual addition differ from Metal. In particular, it adds the residual after each output projection; native initializes the GEMM accumulator with the residual. Those limitations must stay in the results. A remaining difference cannot automatically be called a kernel bug, and a reduction in difference cannot waive the full runtime gate.

All three runs save separate mixer output/residual, FFN output, normalization and projection inputs/outputs, Q/K after rotary, values, attention probabilities, and attention output. C additionally saves its half-rounded Q/K/V and masked scores. A and B require 25 checkpoints; C requires 29. All tensor files remain outside Git; publication uses hashes, shapes, and comparison metrics. Inputs and weights are checked for mutation. Only C's in-memory diagnostic weights are deliberately rounded; no artifact file is written.

Before loading weights, fixed CPU fixtures cover causal means, grouped heads, tails of 1/17/33, and increasing/decreasing maxima across 32-key boundaries. They validate this arithmetic model's consistency, not model acceptance. The same public model operators, GGUF decoder, exact typed record, source and artifact identities remain pinned.

Proposed bounds: CPU only, two threads, 12 GiB process cap, 24 GiB starting available memory, 12 GiB floor, disk lease 2 GiB with 25 GiB free floor, 120 seconds. The earlier complete 64-layer CPU case took 75 seconds and peaked at 3.19 GB. This loads one layer, executes it three times, hashes its weights, and writes at most about 1 GB of checkpoints. Expected time is tens of seconds; the fixed 120-second deadline allows the extra hashing and writes without permitting an unbounded pass. No automatic retry or cap increase.

Source and guard review precede execution. The run stays held until the parent's current Flash/27B acceptance sequence releases its window. No builds, GPU use, quiet leases, desktop changes, or product edits are part of this diagnostic.
