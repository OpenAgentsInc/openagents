# Architecture and considerations

Status: chapters 1–2 implement bounded scalar learning and contiguous CPU
shape/storage admission in [psionic/](../../psionic/README.md). Dense numerical
operations, autodiff, artifact and accelerator contracts below remain proposed. The [source survey](research.md) supplies
their motivation; each increment needs fresh checks and native evidence.

## One library with a small beginning

Psionic should own numerical execution, tensor and model semantics, gradients,
optimizers, model/data admission, and execution records in Coax. The book's
examples use those modules. A small native host supplies storage and platform
effects through generated C interfaces.

Current layout, with future modules added as chapters require them:

```text
psionic/
  lib/          authored Coax; admission and scalar model, dense operations later
  book/         chapters, contents, source references, example manifest
  examples/     small drivers that load the actual library
  platform/     C ABI host: allocation, files, clocks; device bindings later
  tests/        native boundary and independent numerical comparisons
  fixtures/     small deterministic inputs, expected values, provenance
```

Start with files, not a translation of the old crate hierarchy. Extract
`tensor`, `autodiff`, `nn`, `optim`, `model-io`, and backend boundaries when
the next executable chapter needs them. Keep Psionic out of the compiler's
standard library: it is a consumer that can motivate language improvements.

## Storage before tensor convenience

Use caller-owned `[] f32` views and small fixed metadata records. The host
allocates the capacities requested and admitted by Coax; Coax decides whether
dimensions, counts, offsets, and requested work fit. Initial execution is
synchronous, so buffers stay alive for the full call. Do not retain a borrowed
view after its owner's lifetime or return a view into local scratch.

Chapter 2 implements the shape/storage portion of these contracts in
[lib/tensor.coax](../../psionic/lib/tensor.coax): rank 0–4, at most 1,048,576
elements, canonical zero inactive dimensions and checked row-major offsets.
Its staged copy requires an exact logical input view and sufficient separate
destination/scratch capacity; it accepts every finite binary32 value. Later
arithmetic kernels must state their own numeric domains. The first dense
contract should retain:

- `f32` values, row-major contiguous storage, rank at most four, rank-zero
  scalars containing one element, and positive dimensions for non-scalars.
  Zero-length dimensions, strided views, broadcasting, and other dtypes return
  an explicit unsupported result until a chapter defines them.
- Dimension products and byte sizes are checked against explicit limits before
  multiplication. Release integer wrapping must never turn an oversized
  tensor into a smaller admitted allocation.
- Inputs, outputs, and scratch have separate capacity contracts. An output
  that overlaps an input is refused initially. Read-only inputs may share
  storage; writable scratch is disjoint from all live inputs and outputs.
  In-place operations require their own later contracts.
- All shape, size, finite-input, and overlap checks complete before writes.
  An admission failure leaves the destination unchanged. If arithmetic fails
  after work begins, scratch may be dirty, but no successful output/model state
  is published; define that boundary explicitly.
- Large model arrays live in resident host allocations, not copied by-value
  fixed-array structs or large stack frames. Track peak scratch and persistent
  bytes separately.

The native adapter observes real addresses for overlap, while Coax owns the
decision over those observations. Pure evaluator tests cannot prove native
pointer validity or lifetime. Exercise actual aliases and short buffers under
ASan/UBSan. The current local-borrow diagnostic is only a bounded check, as
[the invariants](../../INVARIANTS.md) explain.

## Numerical contracts

Write a contract per operation: shape, accumulation type/order, finite-input
policy, output behavior, aliasing, and a justified error bound. Keep a simple
scalar CPU implementation available when optimized versions arrive. Sensitive
reductions may use an explicitly named `f64` reference path; promotion must be
visible rather than silently changing the `f32` execution profile.

Start with exact tiny fixtures, then use `abs(actual - expected) <= atol +
rtol * abs(expected)` for comparisons that require tolerance. Name tolerances
per operation and workload. Preserve them when optimizing. Test cancellation,
large/small magnitudes, degenerate shapes, and non-finite representations in
the compiled program as well as ordinary finite examples in Coax checks.

Stable softmax needs a policy for empty rows, all-masked rows, infinities, and
NaNs. An invalid computation must produce an explicit failure; substituting a
uniform distribution could conceal a broken model. Attention masks and their
gradients must obey the same defined semantics. Sampling gets a named RNG
algorithm, seed, counter progression, tie rule, and policy version.

Claim determinism by execution profile. Exact integer/index/state transitions,
fixed-profile CPU replay, floating-point comparisons across devices, and model
quality are separate contracts. Equal generated text is weak numerical
evidence; teacher-forced intermediate tensors and logits localize errors.

## Learning is part of the foundation

The first trainer uses an analytic gradient that the reader can derive. Then
add a bounded reverse-mode tape over the same primitives used for inference.
Make tape capacity, saved activations, parameter identity, gradient lifetime,
zeroing/accumulation, detach, repeated backward calls, and unsupported gradient
operations explicit. Refuse tape exhaustion before publishing an optimizer
update. Finite-difference checks on tiny smooth problems supplement hand
derivations and full training examples.

Begin with SGD and a fixed tiny dataset. Add optimizer state and checkpoint
resume before claiming reproducible training beyond an uninterrupted run.
A checkpoint must bind parameter/optimizer layouts, model version, tokenizer,
RNG state, step, and data position. Loss reduction alone does not demonstrate
held-out quality or correct gradient behavior.

## Artifacts and authority

Admission belongs in narrow `#authority;` files with checks written first and
zero surviving mutants. This includes storage/resource bounds, model-file
admission, supported execution-profile selection, and later consent or work
budget rules. Keep pure arithmetic separately testable; a directive is not a
proof of numerical accuracy, memory safety, or authorization.

Begin with tiny owned fixture data. Before importing a real checkpoint, choose
one format and one architecture, pin its origin/license and bytes, and define
bounded parsing plus required tensor names, shapes, dtypes, and tokenizer
identity. File I/O is native; parsing and acceptance are Coax. Reject truncated,
duplicate, inconsistent, oversized, and unsupported data before publishing a
model. Preserve originals and version readers; never persist raw ABI structs.

Use an internal versioned fixture format only when it helps the early chapters,
and label it as internal. Interchange changes start in root `proto/` and the
protocol spec when a real cross-surface consumer exists. Preserve published
contracts and signatures. Do not create a separate Psionic copy of the protocol.

Execution records eventually bind source/compiler version, model/data/tokenizer
hashes, numerical profile, backend/device, seed, workload, resource use, result,
and refusal reason. A hash or receipt identifies evidence; it does not confer
permission, prove correct computation, or authorize payment. Wallets and final
settlement remain outside Psionic.

## Acceleration and current compiler limits

The current `coax metal` profile emits scalar `f32`/`bool` helpers with no
loops, buffer access, or tensor compute entry points. Coaxial's WebGPU drawing
path likewise does not establish general ML kernel support. A CPU tensor loop
cannot simply be emitted as a Metal matrix kernel today.

Before acceleration, make a separate Coax compiler milestone for the smallest
needed compute subset: buffer/index types, bounded loops, work-item identity,
entry/binding layout, and defined arithmetic. Reductions later need explicit
workgroup memory and synchronization semantics. Checks must cover negative
admission, generated-source compilation, and real device execution. Preserve
the CPU oracle and keep new syntax marked planned until it runs.

Native Metal bindings may create buffers, encode commands, and report
completion. Tensor arithmetic, tiling choices, and dispatch/admission decisions
belong in Coax. A hidden C/Swift tensor implementation, a BLAS call carrying
the core math, or a handwritten shader library would change the from-scratch
scope and require an explicit architectural decision. Rust and Python runtimes
are not required by this proposal; optional Python tooling may independently
generate or compare test fixtures.

General generics, dynamic arrays, jobs, and complete ownership should not block
the first chapters. Use concrete types, explicit capacities, tagged operations,
and synchronous calls. When these become inadequate, retain a real consumer
case, a minimum language change, and its verification gate.

Apple CPU is the first local target; Metal is the proposed first accelerator.
Linux CPU, CUDA, browser/Wasm, and multi-device execution each need their own
ABI, numerical, resource, and lifecycle evidence. A successful Mac build does
not qualify those targets.

## Breadth grows from concrete operator families

The [expanded book plan](../../psionic/book/PLAN.md) and
[NVIDIA survey](nvidia-breadth.md) propose more than language models. Each
branch extends the same library with explicit operators and representations:
convolution/resampling and patch layout for vision; spectral/complex and
spherical arithmetic for audio/fields; point neighborhoods and signed
distances for geometry; codebooks/rotations/skeletons for motion; recurrent
state and expert routing for new sequence architectures; diffusion/flow
schedules and embodiment normalization for world/action models.

Add each family through tiny independent examples before importing a complete
checkpoint. Preprocessing, codecs, physical units, coordinate frames, timing
and output interpretation belong in the model profile alongside tensor names.
The host supplies decoding/device effects as needed; model decisions and
numerical transforms remain Coax. Avoid forcing every modality into a text
tokenizer or treating every persistent state as a KV cache.

The early physical surrogate uses ordinary first-order supervised learning.
Input/higher-order derivatives for PINNs require an explicit autodiff extension;
codebooks, geometry queries and stochastic sampling likewise need stated
differentiability rules. Frozen-feature training must identify the trainable
head instead of implying gradients through the entire imported encoder.

Model compatibility, backend qualification, training reproduction and consumer
behavior are separate claims. A checkpoint can be imported without its full
original corpus; a training recipe still needs permitted data and exact resume
to become a reproducible chapter. Preserve weight, code, sample-data and
training-data terms independently. The survey records concrete source
inconsistencies to resolve when those proposed targets are selected.
