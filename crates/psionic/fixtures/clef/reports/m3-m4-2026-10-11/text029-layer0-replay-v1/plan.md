# Layer 0 precision and Delta controls

This diagnostic tests where the first measurable native difference enters. It
does not change the frozen 40-request, 104-question requirement: all top answers
must match and the maximum probability difference must be at most 0.02. The
current result remains 103 of 104 matching top answers. No product change or
full-model run is part of this plan.

## Fixed comparison

Use the same original Q4 GGUF, exact 359 input IDs for text 029, and independent
decoder as the completed CPU reference. Reconstruct the embedding rows without
rounding. Both reference and native code read the same GGUF embedding rows into
f32 before layer 0. This equality is based on verified IDs, decoder evidence, and
source inspection: the archived native capture did not save its incoming rows.
The diagnostic must retain that limitation instead of claiming an observed
bitwise input capture.

Compare against the saved independent CPU and archived Stage 6 native layer-0
outputs. The native baseline has relative L2 difference about 0.00009055 from the
independent CPU layer. Unlike layer 35, this comparison has no preceding layer
whose errors could be amplified. Every variant reconstructs fresh original
weights; a rounded variant cannot contaminate a later f32 control.

| Variant | Linear weights | Linear inputs | Delta operator |
| --- | --- | --- | --- |
| A | Original f32 | f32 | Official chunk64 |
| W | f16 then f32 | f32 | Official chunk64 |
| X | Original f32 | f16 then f32 | Official chunk64 |
| WX | f16 then f32 | f16 then f32 | Official chunk64 |
| R | Original f32 | f32 | Official recurrent |
| WXR | f16 then f32 | f16 then f32 | Official recurrent |

A must reproduce the saved CPU output bit for bit before any other result is
interpreted. W, X, and WX form the weight/input rounding factorial; R minus A and
WXR minus WX isolate the official recurrent-versus-chunked formulation under
two fixed precision settings. This is not a complete three-factor factorial.

The eight `nn.Linear` projections cover qkv, z, alpha, beta, mixer output, and
FFN gate, up, and down. The prehooks cast only the values each projection reads.
That includes normalized mixer/FFN inputs, the gated normalized Delta output,
and the SiLU-times-up FFN output. Convolution weights, convolution output,
normalization weights, residual rows, Delta preparation, and recurrent state
stay f32. The same original stored-negative-A adapter remains in every variant.

Production references are `clef_metal.rs::encode_mixer` (lines 1663–1769) and
`encode_ffn` (1856–1893). The archived Stage 6 shader uses f32 convolution and
Delta preparation (`clef_prefill.metal`, 310–400) and a sequential state update
(409 onward). The official reference uses `torch_chunk_gated_delta_rule`, with
chunk size 64, from pinned `modeling_qwen3_5.py` line 246; the independent official
recurrent operator starts at line 327. Its output reduction still differs from
Metal's `S'·q + delta*(k·q)` expression. No native kernel code is copied into the
reference operator.

## Predictions fixed before execution

- If projection rounding is a principal source of the initial perturbation,
  WX will reduce the layer-output distance from native relative to A. W and X
  show which rounding boundary contributes more; their combination can interact.
- If recurrent-versus-chunked calculation is a principal source, R will move
  closer to native than A, or WXR will move closer than WX. The comparisons are
  labeled separately so a reduction-order effect cannot be called a half-weight
  effect.
- If neither class substantially reduces the residual, retain that negative
  result. The next diagnostic would capture native operator checkpoints or use
  an isolated native layer, rather than implement an arbitrary precision change.
- A later full independent pass with explicit half boundaries could test whether
  these small initial perturbations reproduce the observed layer trajectory and
  final top-answer change. Such a pass would be a causal control, never a new
  acceptance oracle or a waiver for the existing mismatch. It is not authorized
  by this source preparation.

No numerical threshold selects a preferred variant. Retain all distances and
all comparisons, including failures to move toward native. Even a closer match
does not prove that every native operator is correct. GEMM and normalization
reduction order, transcendental functions, fused multiply-add, and accumulation
of residuals inside Metal GEMM remain outside this precision-boundary model.

## Checkpoints and fixture screens

Each of six variants saves 31 checkpoints: input/output of eight linear
projections; q, k, v, log decay, beta, normalized/scaled q, normalized k, Delta
output, and final state; mixer normalization, mixer output/residual, FFN
normalization/output, and final layer output. Retain all shapes, raw/file
hashes, parameter hashes, exact input IDs, and full-layer/false-option-span
errors. The span is the already diagnosed option, not a parameter-selection
criterion. All full-layer coordinates are measured too.

The wrapper requests the final state for observation, then returns `None` to
the unchanged uncached forward. The operators only use `output_final_state` to
discard their already-computed state. A's bitwise check also covers this
instrumentation. Every forward must leave inputs and parameters unchanged.

Before weight loading, six small fixtures compare both official operators
against an independent f64 recurrence: lengths 1, 17, and 65; grouped heads;
nonzero incoming state; zero q/k; zero or very small decay. Output and final
state must be finite and pass absolute tolerance 1e-6 plus relative tolerance
1e-5. These tolerances screen deterministic small fixtures only. They are not
full-model accuracy thresholds and do not establish bitwise Metal equivalence.

## Proposed execution bounds

Run only after root assigns a CPU window and source review passes. Use the same
CPU-only guard as the layer-35 diagnostic: 12 GiB group cap, 24 GiB starting
available memory, 12 GiB available-memory floor, two threads, 120-second deadline,
2 GiB disk lease, and 25 GiB free-disk floor. No GPU, model server, quiet lease,
build, desktop app, or system service is involved. Stop only the guard's own
child group on failure. Preserve any failed run; do not retry or expand limits.

Layer 35 took 10.513 seconds and peaked at 3.707 GB for three variants. This
control has six variants and fresh per-variant weight decoding. Estimated
runtime is 20–60 seconds; the sequential controls and checkpoint I/O make that
an estimate, not a promise. The unchanged 120-second guard decides completion.
One decoded layer is live at a time. The static raw-checkpoint total is
1,599,806,976 bytes (1.490 GiB); a checked 2 GiB output cap allows small metadata
and file headers. Raw checkpoints stay outside Git.

Only source preparation and AST parsing have occurred. The proposed command is
`openagents lease memory --amount 12 ... -- openagents lease disk --amount 2 ...
-- <pinned-python> guard_layer0.py --run-dir <new-directory>`; no execution is
implied by this plan.
