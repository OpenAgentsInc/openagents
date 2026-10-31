# Psionic chapter 8 verification — 2026-10-29

Profile `psionic-interchange-v1`: bounded named F32/BF16 Safetensors maps,
versioned LP64 C ABI, contiguous CPU f32 DLPack exchange and explicit leases.
Full mutation runs are omitted under the owner's instruction; this record
claims focused changed-code verification only.

Qualification results and final artifact identities are recorded below.

Qualified Coax 0.24.0 executable SHA-256:
`69f5cbac6cdce1b5c7c80e94bd453608eb876cb30daa31d778fbe8bb912ea4ba`.
M5 Max, macOS 26.4 arm64, Apple Clang 21.0.0 (`clang-2100.1.1.101`).
No compiler bootstrap, original full model, remote GPU or service was needed.
Numerical compilation uses `-ffp-contract=off -fno-fast-math`, native debug
`-O0` and release `-O3`. ASan/UBSan halt on errors; LeakSanitizer is disabled
on this Mac. Native live-handle counts and managed-deleter counters check
explicit lifetime completion independently of sanitizer coverage.

The focused runner is `psionic/tests/check_interchange.sh`, dispatched by
`psionic/check.sh --only interchange`. Set `COAX` to the qualified compiler
and `PSIONIC_INTEROP_PYTHON` to a Python with NumPy (tested 2.3.5).
`PSIONIC_INTEROP_OUT` optionally retains the emitted C, native binaries,
shared libraries, transcripts and independently reloaded continued artifacts.
Qualification retained these under `/tmp/psionic-interchange/final`.

The root passes 75/75 authored/imported Coax checks in debug/release, including
six new named tests and one rank-two stride property with sixteen cases.
Native tests pass in four configurations: debug/release, each plain and
ASan/UBSan. They check canonical sorted payloads, input-order-independent
files, exact BF16/F32 encodings, signed zero/subnormal preservation, every
truncation of a valid file, duplicate/unsupported metadata, output alias and
capacity refusals, unchanged public outputs, scalar/empty live tensors and
bit-preserving transport of a signaling NaN without numerical evaluation.

The DLPack harness checks pointer identity, copied shape metadata, legacy and
versioned carriers, a bad-major carrier with an unreadable later shape pointer,
GPU refusal before shape access, noncontiguous strides, byte-offset alignment
and pointer overflow, unsupported flags, required v1.2 strides and explicit
argument-error versus consumed-error behavior. Managed-deleter counters
advance exactly once for consumed refusals and on the last successful lease.
A live exported lease survives release of both original retained handles.

The shared libraries separately pass the real NumPy producer/consumer gate
in debug/release. Legacy and versioned imports consume actual NumPy capsules,
keep original array ownership pinned after the Python reference is dropped,
and free it on last release. Exports reach `numpy.from_dlpack` read-only at
the identical storage address after the original Psionic handle is released.
The last NumPy view and an unconsumed export capsule both return the lease;
all handle ledgers end at zero. Python uses `ctypes.PyDLL` to retain the GIL
while invoking NumPy's producer deleters.

An independent stdlib/NumPy producer writes a Safetensors artifact containing
`scale.weight = 0.5`, F32 shape `[1]`. The public ABI inspects it; its consumer
validates the expected name, dtype and shape. Coax's original scalar trainer
continues for sixteen updates on `x=[1,2]`, `y=[2,4]`, reaching
`weight=1.99999976`, `loss=1.42108547e-13` in both builds. The public writer
exports it, and an independent JSON/`numpy.frombuffer` reader reloads it for
inference at `[-3,0,3]`, within 0.000001 absolute error of `[-6,0,6]`.
The private `ic_continue` demonstration is outside the stable public ABI.
No installed Safetensors package or PyTorch was used, and no object is unpickled.
The format oracle is an independent implementation, not upstream-runtime parity.

The ABI header is also compiled as C++17 to check public-language compatibility.
The exact Coax example transcript passes both modes, and the book gate checks
all local links, jai/shell fences and source-linked listings. The imported
chapter 1, 3 and original-weight reader checks run in the new root; their
production files were not changed. No unrelated full suite is claimed.

Source contracts: the [Safetensors format](https://github.com/safetensors/safetensors/blob/main/README.md#format)
and [DLPack v1.2 declarations](https://github.com/dmlc/dlpack/blob/v1.2/include/dlpack/dlpack.h).
The declarations and Apache license are pinned in
`psionic/platform/vendor/README.md`; neither upstream's scripts nor an upstream
runtime was executed. Scope remains CPU contiguous f32 DLPack, bounded finite
F32/BF16 parameter maps, exclusive caller access and a known toy architecture.
GPU stream/event exchange, graph conversion (161/161a), wider layouts/dtypes
and arbitrary optimizer/checkpoint conversion are separate gates.

Final artifact SHA-256:

| Artifact | Digest |
| --- | --- |
| `interchange-debug.c` | `91b5b94cafd94c901455db170cc5463e60067fccf9af6080c572273779e51b5f` |
| `interchange-release.c` | `e87b987b5050d8817d8bb00e5b14647b547b26286b237b4b0d8530394bd5ed28` |
| `continued-debug.safetensors` | `f9a506a22682a7c07c8e47cc9da8f09a6ed479e3a20111f3b279666924d8b778` |
| `continued-release.safetensors` | `f9a506a22682a7c07c8e47cc9da8f09a6ed479e3a20111f3b279666924d8b778` |
| `interchange.h` | `64ba4ab4b596bceb4701eb57d19e5baac4d402ed5b3176987086343e78285c97` |
| `interchange.c` | `89be49a0a6dca7d7179fb430ad5a1450b5e964b7a80c4f2d37a9c6e27e92f4ca` |
| `interchange.coax` | `2918ca5eaa0acdc737f1eaabb0691fe5c569d05acab9ad46fa594479018b410f` |
