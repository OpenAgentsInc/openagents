# Chapters 26 and 30: warp kernels and tensor cores, 2026-10-29

Same host as the [chapter 26 record](psionic-ch26-2026-10-29.md): RTX 4080,
driver 595.71.05, NVRTC 13.2.51, now coax0 0.24.0. Another agent's gpt-oss
process shared the GPU during part of the session; the numbers below are from
runs while it was idle (GPU utilization at or below a few percent before
each run).

## What was built

- **Warp kernels** in the CUDA compute subset (coax0 0.24.0). A kernel with
  `for lane: 0..<32` loops runs one 32-lane warp per work item. Arrays of
  `32*k` values indexed `lane*k + offset` inside lane loops, where the offset
  is built from constants and constant-loop counters, are per-lane registers.
  Code outside lane loops is the warp's shared control flow. Admission is
  Coax policy (`coax/lib/compiler/compute.coax`, 201 mutants, zero survivors);
  `coax/tests/cuda.py` refuses each construct outside the profile (lane loops
  with other bounds or outside kernels, malformed per-lane indices, per-lane
  arrays used elsewhere, lane loops writing shared values, `break` or
  `continue` out of a lane loop, primitives with wrong fragment arrays or
  inside lane loops) and checks the emitted registers, entry and `mma.sync`.
- **The MMA primitive** `cu_mma_bf16_16x8x16` (`coax/lib/compute/mma.coax`,
  175 mutants, zero survivors): its Coax body is the CPU meaning in the PTX
  m16n8k16 fragment layout; `coax cuda` emits one inline `mma.sync` instead.
- **Tensor-core projection** (`psionic/lib/device/warp.coax`, 428 mutants,
  zero survivors): `dw_split` writes activations as two BF16 halves,
  `dw_project` multiplies 16 rows by 88 tokens per warp with both halves over
  a contiguous slice of columns, and chapter 26's `dv_reduce` adds slices.
- **Plan** (`psionic/lib/device/plan.coax`, zero survivors): projections of
  16 or more tokens use the tensor-core path; slices double until a launch has
  4,096 warps, each slice keeps at least 8 steps, at most 32.

Checks: `psionic/tests/run_device_warp.sh`, `psionic/tests/check_device.sh`,
`psionic/tests/run_elephant_device.sh`.

## Tolerances, frozen before measuring

- MMA primitive against its Coax meaning: `|device - CPU| <= 2^-20 * (|C| + sum |A B|)`.
- Tensor-core projection against the CPU reference profile (`qm_project`):
  `|device - reference| <= 2^-14 * sum |w x|`.
- Decision: chapter 30's 0.0005 absolute bound on each statistic, against the
  independent reference and the recorded CPU result.

## Results

MMA: 4,096 warps of random finite BF16 fragments and f32 accumulators; the
largest difference was 3.8e-7 of `|C| + sum |A B|` (bound 9.5e-7), and 227,554
of 524,288 outputs were bit-identical to the Coax meaning.

Tensor-core projections on random BF16 weights at Elephant's shapes, 86
tokens, mean of 20 launches (split + project + reduce), against cuBLAS 13.3
`GemmEx` with BF16 weights and activations and f32 accumulation, measured in
the same session with `psionic/tests/bench_cublas.sh`:

| Shape | Slices | Psionic | TFLOP/s | cuBLAS BF16 | Fraction of cuBLAS | Reference error |
| --- | --- | --- | --- | --- | --- | --- |
| 8,192 x 4,096 | 8 | 0.472 ms | 12.2 | 0.104 ms | 22% | 6.2e-7 |
| 12,288 x 4,096 | 8 | 0.710 ms | 12.2 | 0.204 ms | 29% | 6.3e-7 |
| 4,096 x 12,288 | 16 | 0.785 ms | 11.0 | 0.187 ms | 24% | 4.2e-7 |
| 4,096 x 4,096 | 16 | 0.217 ms | 13.3 | 0.052 ms | 24% | 5.0e-7 |
| 1,024 x 4,096 | 16 | 0.063 ms | 11.5 | 0.017 ms | 27% | 4.4e-7 |

The work-item kernel of chapter 26 ran the same shapes at 5.4-6.9 TFLOP/s.
cuBLAS's BF16 path rounds the activations to BF16, which costs up to 1.7e-4
of `sum |w x|`; Psionic's halves keep it near 6e-7.

Full decision (chapter 30's sample, plan with tensor-core projections, default
budget of 14.2 GiB, 0.94 GiB streamed): `yes`, p(yes) 0.757202773, p(no)
0.242797227, slot mass 0.228238784, confidence 0.514405545. Against the
recorded CPU result (0.757196285, 0.242803715, 0.228232722, 0.514392571) the
largest difference is 1.3e-5 (confidence), inside the frozen 0.0005; the
independent reference check passes too. Five warm runs took 145.8-148.8 ms
from request to probabilities (mean 147.3 ms), against 236.6-238.5 ms with
chapter 26's work-item projections. The tensor-core projections took 94.7 ms
of a 152.5 ms profiled run (248 launches, 62%); the slice reductions 10.8 ms
and every other kernel together 47 ms.

With `PSIONIC_DEVICE_QUALIFY=1`, 54 of the 70 launches of layers 0 and 3 and
the readout were identical to the same kernels on the CPU. The other 16 are
the tensor-core projections, whose f32 partial sums round differently from the
Coax meaning of the MMA primitive; the per-word relative measure the harness
prints is not meaningful for them because many partial sums are near zero, so
they are qualified by the projection comparison above instead.

## Recorded launch replay

`PSIONIC_DEVICE_REPLAY=1` records the decision's 1,122 copies and launches
once (CUDA stream capture through `pc_capture_begin`/`pc_capture_end`) and
replays the recording for every run. It returned the same probabilities to
every printed digit. Warm replays took 151.2-155.9 ms (first run, including
the recording, 177.0 ms), measured while a compiler bootstrap loaded the CPU;
direct launches took 145.8-148.8 ms on a quiet machine. The GPU is busy for
nearly the whole decision, so launch overhead is not what limits it; replay is
kept opt-in.

## Limits

No shared memory or barriers yet: each warp reads its weight fragments from
L2 or DRAM directly, which is why the products reach about a quarter of
cuBLAS. One machine, one driver, one request.
