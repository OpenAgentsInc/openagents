# Deployment, math defaults, and remaining M5 paths

This source-only follow-up does not identify a credible software change that is
known to close the remaining whole-model target. It adds no implementation or
workload. [Source hashes and exact line references](sources.json) distinguish
installed SDK headers, product code, and prior measured evidence.

## Deployment target: a proven operand-helper issue, not a proven GEMM slowdown

The installed SDK's `MPPTensorOpsAvailability.h:10` selects operand-aware
cooperative-tensor helpers when the minimum OS version is at least 26.2.
The earlier fixed preprocessing/export diagnostic proved that default offline
Metal 4.0 compilation selected minimum 26.0 and destination-only helpers, while
an explicit 26.2 target selected the correct input helpers. Both coordinate
exports then passed. The follow-on unchanged register-input GEMM passed its
numerical checks but ran roughly 2.4–2.6 times slower than the baseline.

This does **not** show that production's direct-device GEMMs take the old input
helper path. `matmul2d::run` forwards to `__run` at public header line 412. In the
implementation, `__run` begins at line 4145; the half×half→float, device-input and
device-output branch at lines 5160–5162 calls
`__tensorops_impl_matmul2d_op_run_dv_f16_dv_f16_dv_f32`. Its declaration at line
1150 is unconditional. All 26.2 preprocessor branches in this header affect
cooperative object sizes, initialization, element pointers, coordinates, or
loads. The direct-device type/address-space selector has no deployment branch.
There is no 26.4 or NAX selector in these installed MPP headers.

The final lowering is externally implemented. Header inspection cannot prove
identical generated instructions across OS targets, or identify the hardware
execution unit. An M5 alone does not establish the runtime compiler's minimum.
Product `ClefMetal::new_with_options` uses runtime `new_library_with_source` and
fresh default `CompileOptions` for its default, fused-attention, and shape
libraries (lines 281, 295, 320). Offline target behavior cannot be assigned to
those calls without separate runtime evidence.

If compilation provenance later needs resolution, the minimal comparison is
**two builds of the exact same production direct-device shader**: Metal 4.0 with
explicit minimum 26.0 and explicit minimum 26.2. Keep all other flags and pipeline
construction identical; record compiler/SDK/source hashes and preprocessing
macros first. Do not add 26.4 without a specific new branch. Different AIR or
library hashes alone do not prove different instructions or speed. Runtime
compilation needs its own separately reviewed branch probe before changing the
product. No such build or probe is scheduled by this note.

## Math options are separate contracts

The [Metal language specification](https://developer.apple.com/metal/Metal-Shading-Language-Specification.pdf)
4.1, section 1.6.3, documents fast as the command-line default for both FP32 math
functions and math mode. Safe mode restricts transformations; relaxed mode keeps
NaN/Inf behavior while still allowing aggressive transformations. These options
are distinct from the matmul descriptor's `relaxed_precision` flag: Table 7.4
permits truncation of **float multiplicands**, and provides no evidence that the
flag accelerates half×half→float operations or changes accumulator precision.

Installed `MTLLibrary.h:305–321` says the legacy fast-math default is enabled,
the newer math-mode default depends on language version, and the FP32 function
default is fast. Apple's [languageVersion](https://developer.apple.com/documentation/metal/mtlcompileoptions/languageversion)
documentation says runtime compilation selects the latest language by default.
Its [mathMode](https://developer.apple.com/documentation/metal/mtlcompileoptions/mathmode)
documentation describes how explicitly setting the legacy boolean maps to the
new mode and function controls. Actual runtime values have not been captured.
There is no source evidence that production accidentally requests safe math or
that enabling faster math would unlock an untested 2× path. Do not conflate a
float output with float multiplicands, or change numerical gates to justify an
option. Default-mode provenance is a measurement question, not a speed finding.

## Remaining mechanisms, ranked by evidence

1. **Locate sustained GEMM cost without per-layer CPU waits.** The older intrusive
   4k profile attributes about 3.4 of 4.4 device seconds to GEMMs. If that share
   remained representative, a 2× GEMM gain would yield only about 1.63× total;
   about 3.1× GEMM acceleration would be needed for a 2.1× total gain. Those are
   Amdahl estimates, not current profiling results. The unprofiled sustained run
   reduced submission gaps with four commands in flight, but GPU intervals still
   rose from about 11 to 26–27 ms. Simple CPU starvation is insufficient to explain
   that result. No clock, thermal, bandwidth, or scheduling cause is established.
   Existing timestamp-only counter capability can observe own command stages;
   it does not expose clock or occupancy. A separately reviewed in-command stage
   measurement could avoid the current profiler's per-layer `commit_wait`, but
   it would still perturb execution. No new measurement is authorized here.
2. **Layer-major 27B prefill could trade memory for fewer conversions.** This is
   the clearest remaining structural hypothesis for the low-memory Q4 path,
   not the resident Flash gap. Current chunk-major `linear_with_workspace`
   converts gate, up, and down into one reused half-weight slot on every chunk.
   Merely swapping the layer/chunk loops leaves those three conversions inside
   `encode_ffn` and saves nothing. To decode each weight once per layer while
   keeping chunk-sized intermediates, retain all three half matrices for that
   layer while processing its chunks. A 16k×5120 f32 hidden buffer costs 320 MiB;
   three half matrices cost 510 MiB, or 340 MiB beyond the existing 170 MiB slot.
   One in-place full-prompt hidden buffer plus the extra weights adds roughly
   660 MiB before allocator overhead. A second hidden buffer is unnecessary only
   if read/write ordering and chunk views prove safe.

   At chunk2048, eight conversion passes become one. Under the all-Q4 assumption,
   this avoids about 286 GiB of packed-weight reads plus half-weight writes across
   64 layers, while leaving the FFN GEMM count and about 561 TFLOP of arithmetic
   unchanged. [Exact arithmetic](layer-major-cost.json) is provided. There is no
   isolated conversion timing or effective bandwidth measurement that converts
   this traffic estimate into a defensible latency gain. It could approach the
   resident path with much less persistent weight storage; it cannot be called a
   1.5× or 52 GiB-qualified solution yet.

   Correctness can retain exactly the same per-layer, chronological mixer chunks,
   GEMM shapes, decoded half bits, residual order, and final normalization.
   Prefix snapshots are the major complication: the current snapshot captures
   every layer together at a token boundary. A layer-major loop must capture
   each layer's state before advancing past that boundary and publish only after
   the whole request succeeds. Cached suffixes, final memory/span output, partial
   failure, cancellation, and byte reservations need new lifecycle checks. The
   current batch scheduler advances each request one chunk through all layers;
   changing that contract affects deadlines and request fairness. Any initial
   design should be a separate serial path with prefix and batching disabled,
   rather than silently changing those features.
3. **More accurate large-key attention is primarily an accuracy experiment.**
   Splitting probability weights into half high and low parts and accumulating
   both PV products can reduce representation error; the denominator must use
   the unrounded float weights or a consistent high-plus-low sum. It adds a PV
   matrix multiplication, about 50% more matrix work for the attention pair.
   Fewer softmax updates might offset part of that cost, but there is no measured
   net gain. The older attention share was only about 7% at 4k, so even eliminating
   it would give only about 1.07× on that historical profile. A more accurate
   operator also need not pass the unchanged baseline raw-logit gate. It cannot
   be tuned to the one mismatched question.
4. **Larger chunks or remaining head waits have small demonstrated ceilings.**
   Chunk4096/8192 leaves the 1k request unchanged. It preserves the GEMM tile work
   and removes only some already-amortized chunk waits, while increasing scratch.
   Serial production already places the layer stack and tail in one command
   buffer per chunk. The latest native head medians are about 16/25/44 ms at
   1k/4k/16k, roughly 3.1/1.0/0.4% of total time. Head elimination alone cannot
   close the target. CPU/AMX splitting adds weight widening, result ownership,
   scheduling, and arithmetic risks without a measured cost advantage.

The fixed max128 pipeline experiment is now complete: all outputs matched, but
its full-FFN ratios were 0.767/0.959/1.020 at 512/1024/2048, failing the unchanged
all-size 0.90 gate. The same-source resident gate/up overlap and the explicit
register-input candidate also failed their speed gates. None is proposed for
integration. Further parameter sweeps would not resolve the current evidence
limit. Model latency targets and independent accuracy requirements remain open.

## Retention

This directory contains only this note, exact source/evidence hashes, and the
layer-major arithmetic. It includes no Apple PDF or extracted full text, model
weights, executable, private source, or local user path. The prior experiment
bundles preserve every outcome separately. No new hardware run was performed
for this assessment.
