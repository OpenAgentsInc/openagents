# Cooperative dense training prerequisite, 2026-11-02

`training-dense-f32-v1` qualifies one reusable forward/VJP component for
[#12627](https://github.com/OpenAgentsInc/openagents/issues/12627).
The issue remains open: useful 10–50M-parameter single-GPU training still
requires a separately frozen workload, quality and resource evidence.

The product is [training-dense.coax](../../../psionic/lib/device/training-dense.coax)
and its [three kernels](../../../psionic/lib/device/training-dense-kernels.coax).
The [public example](../../../psionic/examples/training-dense.coax) evaluates
a dense squared loss and obtains both operand gradients through `gd_run`.
Python and C here are verification/effects tooling, never runtime arithmetic
for a served model. No training, dataset download or old full gate ran.

## Profile and contract

`X[M,K]`, `W[K,N]`, `Y = XW`, `dX = dY Wᵀ`, `dW = Xᵀ dY`; row-major
FP32, dimensions 1–128, exact view extents. Products and sums round in FP32,
without FMA, in increasing logical reduction-index order. These are formal
bilinear VJPs, not derivatives of the discontinuous rounding operation.
Each 128-thread group owns a disjoint 32×32 output tile. Its two bounded
32×128/128×32 input tiles require exactly 32 KiB shared memory. No atomic
or cross-group accumulation occurs.

`gd_plan` checks dimensions, capacities and the caller's explicit alias fact.
`gd_run` admits all three inputs before any output write, requiring finite
values in [-1024,1024]. This proves every product and sum representable:
at most 128 products of magnitude 1024² per output. The largest profile
uses 393,216 caller-payload bytes and at most 6,291,456 products plus the
same number of additions across all three operations. This is not a
whole-device peak-memory claim.

Input storage remains valid and immutable; outputs require exclusive,
disjoint storage. The alias boolean is a caller contract, not address
introspection. Raw device kernels require the same admission and storage
conditions. A CUDA driver failure does not provide transactional publication
of partially written device outputs.

## Observations

- Nine authored check groups / 72 cases pass in debug and release.
- Focused root-only mutation kills all 88 policy mutants and all 255 kernel
  mutants, with no equivalence waivers. Only these new product units ran.
- Actual generated C passes independent scalar oracles at O0, O2 and
  ASan/UBSan: 62 observations per build, ten numerical shapes including
  128³, ragged tiles and non-binary FP32 products; exact output words,
  unchanged inputs, guarded refusals and 18 independent bilinear central
  differences per build. The public example's transcript matches its
  hand-worked loss and gradients.
- Actual RTX 4080 / sm_89 execution passes eight fixtures and 47,737 output
  words bit for bit against the independent scalar oracle. Input words and
  both output guards remain unchanged. Coax admits the reported capability
  before launch. NVRTC compilation used the existing 13.2 library, strict
  IEEE flags and the exact generated CUDA retained in the proof.
- The remote device was idle before build and before launch (145 MiB,
  0% utilization), and returned to the same observed state. The run used
  only a fresh owned `/tmp` directory; no service or other job was stopped.
  `compute-sanitizer` was unavailable on PATH and in the inspected installed
  Nix package paths; GPU memcheck is unqualified. CPU ASan/UBSan passed.

The successful CPU and GPU receipts bind source, generated C/header/CUDA,
producer hashes, raw commands and outputs. `proof-manifest.json` lists the
retained text members of `proof.tar.xz`; executables are represented by their
recorded hashes. These are retained observations, not execution attestations.

## Development failures retained

The checks were authored before product implementation; the first command
refused the missing source unit. Early check setup hit a default evaluator
budget and compile-time NaN conversion refusal; the native oracle checks
actual NaNs. CUDA emission refused thread loops inside a non-export helper,
so each exported orientation now physically contains its thread loops.

Initial mutation exposed missing exact tile-boundary cases. A separate
volume threshold had no admitted shape at its next integer and was removed:
the inclusive per-axis limit already proves bounded work. The original
padded reduction schedule admitted extra zero-only work under mutation;
the final schedule loads a bounded full reduction tile once and reduces
exactly the logical inner extent. Earlier survivor reports and successful
intermediate observations keep their original source identities. No
failure was relabeled as a final pass.

## Reproduction and remaining work

The focused CPU and generation harness takes a new output directory:

```sh
coax/bin/coax test psionic/lib/device/training-dense.coax
coax/bin/coax test psionic/lib/device/training-dense.coax --release
coax/bin/coax mutate psionic/lib/device/training-dense.coax --root-only
coax/bin/coax mutate psionic/lib/device/training-dense-kernels.coax --root-only
psionic/tests/check_training_dense.sh NEW_CPU_OUT
```

The CUDA effects harness consumes an exact generated bundle and a new output
directory, verifies its hashes, refuses a busy card, and requires a real
CUDA device/NVRTC rather than treating absence as success:

```sh
PSIONIC_NVRTC=/exact/installed/libnvrtc.so \
  psionic/tests/check_training_dense_cuda.sh GENERATED_BUNDLE NEW_CUDA_OUT
```

The retained bundle manifest names its exact producer and members. Emitting
`psionic/tests/training-dense-device.coax` supplies the capability-admission
entry; emitting `psionic/lib/device/training-dense-kernels.coax` supplies the
three CUDA kernels. The C effects adapter is the existing `cuda-host.c`.

There is no resident optimizer step, cooperative attention/normalization or
embedding backward, BF16 training/storage/overflow policy, GPU convolution
VJP or compiled GPU program in this slice. The existing chapter129 decoder
still uses one thread per row and retains its slower-than-CPU measurement.
Its public trainer/checkpoint path still admits at most 4096 parameters;
its data stream and model shape bounds also remain unchanged. The generic
IR is still limited to 32 nodes and FP32, with CPU compiled backward;
reusable AdamW currently admits at most 16,777,216 parameters. None of these
bounds was enlarged for this slice.

Before a 10–50M acceptance run, freeze exact model/initialization and source
hashes; licensed corpus and document-disjoint train/validation/test hashes;
tokenizer, packing and boundary targets; seeds/counters/order; token budget,
batch/accumulation, optimizer/schedule/clipping; FP32/BF16/master/accumulation
and overflow/loss-scale semantics; held-out quality margin and independent
baseline; throughput, memory and startup goals; and resume tolerances. The
independent PyTorch baseline is opt-in verification only. Publish every
attempt and total tuning cost. Kernel numerics alone cannot close #12627.

Publication integration registers the bounded example and both release units,
with a focused `psionic/check.sh --only training-dense` route before any model
provisioning. The [integration record](integration/record.json) retains nine
route-spy subcases, reviewed metadata bindings and the added release example.
No passing numerical/device qualification was repeated during integration.
