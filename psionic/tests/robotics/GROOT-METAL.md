# Original GR00T state encoder on Metal

This component executes the real category-2 state encoder from
`nvidia/GR00T-N1.7-LIBERO`, revision
`2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21`, subfolder `libero_10`.
Its selected original BF16 parts are authenticated against the four fixed
SHA-256 values in `groot_original_reference.py` before GPU execution and
again during the independent comparison. The test proves this selected
component; its record does not claim to authenticate the complete shards.

`groot-metal-plan.coax` supplies the eight private buffer extents and four
ordered launch descriptions: 132→1024 projection, F32 bias and ReLU,
1024→1536 projection, F32 bias. Original input-major weights are transposed
by the existing checked Coax `gl_pack`; `gl_bias` decodes biases. Projection
arithmetic runs exclusively through `dt_project_16x64`, the existing
Coax-generated Metal4 matrix kernel. The native host executes effects and
never calls a CPU projection or substitutes CPU output.

This is a distinct numerical profile: BF16 weights, F32 inputs and matrix-unit
accumulation, separate F32 bias, and ReLU. It is not the checkpoint's default
BF16-activation execution profile. The two inputs are the same authored
numerical fixtures as the CPU component: eight normalized values plus padding,
and a 132-value ramp. Neither is a captured robot observation.

The host poisons private output and numeric flags before each batch. Coax
orders successful launch receipts, requires the actual `pm_wait` receipt,
checks every numeric flag and every hidden/final raw F32 word, and only then
copies 2,560 values into the caller's disjoint publication buffer. Invalid
kernel metadata returns without writes, leaving poisoned flags invalid;
implicit device checks are also reported through `pm_wait`. The effect host
keeps both cases private until both are accepted, then creates a new output
file. This synchronous cursor assumes its owner reports genuine platform
receipts and preserves private buffers through the wait; it is not remote
attestation or a general asynchronous buffer-ownership protocol.

The independent NumPy oracle reads the original input-major BF16 files,
reduces in `longdouble`, narrows each projection to F32 before adding F32 bias,
and checks all 5,120 hidden/final values. On this arm64 NumPy build,
`longdouble` has 53 significant bits; it is not extended precision or Torch
parity. Tolerances were fixed before GPU execution at `atol=2e-5`, `rtol=1e-5`.
A failure must not widen them. The required-device gate fails if Metal is
absent or compilation fails; it never reports a skipped GPU as a pass.

```shell
psionic/tests/robotics/check-groot-metal-native.sh
REFERENCE_PYTHON=/path/to/existing/numpy/python3 \
  psionic/tests/robotics/check-groot-metal.sh ORIGINAL_PARTS NEW_OUTPUT_DIRECTORY
```

The first M5 Max run passed: case 0 hidden/final maximum absolute error
`0` / `1.4901161193847656e-7`; case 1
`7.450580596923828e-9` / `2.980232238769531e-7`. Both actual waits returned
`PM_OK`. GPU command time for the four launches was `0.479 ms` and
`0.359125 ms`, excluding compilation, packing/upload, publication and
reference evaluation. These are component timings, not end-to-end VLA
latency or throughput measurements. New-root mutation killed all 215 plan
mutants and all 37 kernel mutants. The independent native helper oracle
passed 55 cases in debug, release and ASan/UBSan builds. Unchanged passing
results are reused rather than repeated.

Vision, language, denoising/action flow, a complete original model request,
closed-loop task success and physical-robot control remain outside this
component. Chapter 190 remains unfinished until the full VLA path is qualified.
