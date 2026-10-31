# Original Qwen3-VL vision on Metal

This is the complete original visual encoder from pinned
`nvidia/GR00T-N1.7-LIBERO@2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21`,
`libero_10`: original patch convolution, learned 48×48 positions, all 24
vision blocks, ordinary merger, and three DeepStack mergers after blocks
5, 11 and 17. Two separate still cameras each contribute 256 packed patches;
the ordinary and three DeepStack outputs each have shape `[128,2048]`.

Coax owns the 315 exact learned tensor specifications and range binding,
336 buffer extents, 29 ordered stages and every one of the 359 micro-op
launches. `groot_gpu_dispatch.h` only translates scalar/view arguments and
calls platform effects. No C branch chooses a transformer's operator order.
Original BF16 matrix payloads already have `[output,input]` order, so they
feed `dt_project_16x64` directly. Coax decodes BF16 norm/bias vectors to F32.

The current mixed precision profile uses Metal4 F32 matrix accumulation,
wide layer norm statistics/centering/division through the qualified Metal
float-float f64 profile, F32 affine stages and attention, tanh-GELU in vision
block MLPs, and a separate
bounded-error erf approximation for the original mergers' default GELU.
The merger activation is not replaced with the tanh formula. The independent
native oracle checks the erf approximation over 49,153 points from −12 to
12, finite extremes, and nonfinite flag handling, with fixed
`2e-6 + 2e-6*abs(reference)` limits.

The ordinary merger normalizes 1024 channels before joining each group of
four patches; DeepStack normalizes the joined 4096 channels. Every stage
assigns separate poisoned flag regions to its kernels. Only an actual
successful wait and every flag zero advance its Coax cursor. Separate raw
F32 finite checks precede copying a capture into a disjoint output buffer.
The cursor assumes genuine platform receipts and private buffers retained
through the wait. It does not provide remote attestation or an asynchronous
GPU ownership protocol.

The independent original Transformers 4.57.3 reference ran on MPS/F32
before this runner and is reused without rerunning the full model. Its
capsule supplies the original processor's packed pixels from two owned
patterned images and records original output hashes. This test does not
qualify our image decoding/preprocessing path or a captured robot episode.
Whole-shard authentication reuses the earlier completed full SHA-256 record;
the runner rechecks exact file sizes and pinned header hashes. It neither
redownloads weights nor treats a declared digest as new payload authentication.

The full vision comparison limits were frozen before execution at
`2e-3 + 2e-4*abs(reference)` for every captured value. The initial trial
passed all four endpoint outputs but exceeded this limit at block 23 and
the same merger input, reaching `1.1801831534776992` times the error budget.
Inspection found a real source mismatch: `torch.linspace(0,47,16)` uses one
rounded F32 step and symmetric endpoint construction. Direct `i*47/15`
differs by one ULP at six coordinates. Coax now follows the original
construction; no tolerance was widened.

After that correction in the original F32-statistics profile, all 34 original boundaries and 16,777,216 values
passed. The last block's maximum absolute error was `0.1156005859375`
against large learned activations, and its largest relative-plus-absolute
budget fraction was `0.677938552473845`. The ordinary output's maximum
absolute error was `0.00019550323486328125`; the three DeepStack maxima were
`0.00001239776611328125`, `0.00002002716064453125`, and
`0.00022983551025390625`.

That F32-statistics run's summed M5 Max GPU command time was `0.194422041648 s`. This excludes
compilation, original-weight loading/decoding/upload, input preparation,
CPU waits/flag scans, capture copies and comparison. It is a vision
component measurement, not full VLA throughput or end-to-end action latency.
All 351 new device-root mutants and 910 original F32-plan-root mutants were killed.
The independent native oracle passed 64,656 cases in debug, release and
ASan/UBSan builds. Previously passing tile and generic device gates were
reused unchanged.

The later mixed precision norm profile retained all frozen comparison limits:
all 34 boundaries and 16,777,216 values passed again on the changed graph.
Its ordinary output maximum absolute error was `0.0002923011779785156`,
and the three DeepStack maxima were `0.00001239776611328125`,
`0.0000171661376953125`, and `0.000247955322265625`. Summed GPU command
time was `0.214742041659 s`, with the same exclusions. All 913 changed
plan-root mutants were killed. The 351-mutant vision helper proof,
64,656 native cases in three modes, and the already qualified wide norm
primitive were reused unchanged; only the norm kernel name/library mapping
and four-module effect loading changed. Both earlier records are retained.

```shell
psionic/tests/robotics/check-groot-vision-metal-native.sh
REFERENCE_PYTHON=/path/to/existing/numpy/python3 \
  psionic/tests/robotics/check-groot-vision-metal.sh \
  ORIGINAL_SHARD_DIRECTORY ORIGINAL_SDK_CAPSULE NEW_OUTPUT_DIRECTORY
```

The required-device runner fails without compatible Metal and never
substitutes CPU inference. Production request admission, language inference,
action flow, closed-loop simulation and physical robot control remain separate.
This component alone does not finish chapter 190.
