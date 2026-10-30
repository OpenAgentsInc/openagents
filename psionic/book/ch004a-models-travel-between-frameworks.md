# 4a. Models travel between frameworks

A model can leave its training framework without carrying executable objects.
For our first exchange, its architecture is the known equation `y = weight*x`.
The artifact supplies one named parameter, `scale.weight`. We import a weight
of 0.5 from an independent NumPy producer, continue training in Coax, write a
Safetensors file, and reload it independently for inference. There is no Python
or original framework in the Psionic runtime.

This chapter adds [Coax interchange policy](../lib/interchange.coax), a
[versioned public C interface](../platform/interchange.h), and its
[allocation/transport binding](../platform/interchange.c). The
[machine-readable profile](../fixtures/interchange-profile.json) states the
first limits. [Chapter 4](ch004-reading-original-weights.md) remains the reader
for larger original model bundles; this chapter adds a smaller bidirectional
exchange profile without changing that original contract.

## Run the example and the external round trip

```shell
coax/bin/coax run psionic/examples/model-interchange.coax
PSIONIC_INTEROP_PYTHON=/path/to/python-with-numpy \
  COAX=/path/to/qualified/coax psionic/check.sh --only interchange
```

The external gate needs NumPy for verification, tested with version 2.3.5.
It builds debug and release native binaries, sanitized binaries and shared
libraries. Python loads the shared library with ctypes; the numerical update
is generated from Coax. An installed Safetensors package or PyTorch is not
required. The [Coax-only example](../examples/model-interchange.coax) and its
[exact transcript](../fixtures/model-interchange.out) are also checked:

```jai
Portable parameter: scale.weight, F32 [1], 4 bytes
Continued in Coax from 0.5: weight = 1.9999998
Reloaded prediction at x=3: 5.999999
No object deserialization or original framework executes.
```

## Separate architecture, parameters and training state

A parameter map binds a unique name to a dtype, shape and dense byte interval.
Names are identifiers, not instructions. Loading `scale.weight` does not
construct an arbitrary network: the receiving application already knows the
scale architecture and validates its expected parameter inventory.

| Object | What crosses this boundary |
| --- | --- |
| Architecture | Agreed by the two applications: multiply by one scalar weight |
| Inference parameters | Named F32/BF16 tensors in an inert Safetensors file |
| A live tensor | An explicit leased contiguous CPU f32 DLPack carrier |
| Optimizer, step, RNG and data cursor | Not included in this inference artifact; resumable checkpoints belong to 76b |
| A portable graph | A bounded ONNX graph: read in [161](ch161-read-a-bounded-onnx-graph.md), written in [161a](ch161a-write-a-bounded-onnx-graph.md) |

Further training here starts from the imported parameter with the original
[chapter 1 trainer](../lib/learn.coax). It does not pretend that a weights file
restores an optimizer or the exact trajectory of another trainer. Pickled `.pt`
objects are refused as unsupported bytes; neither the C binding nor the Coax
reader invokes object deserialization.

## Write a complete named map

Safetensors stores a little-endian eight-byte header length, a JSON header and
row-major tensor bytes. Each header entry names its dtype, shape and half-open
data offsets relative to the payload. The format requires a complete indexed
payload. These rules come from the
[upstream format specification](https://github.com/safetensors/safetensors/blob/main/README.md#format).

Our writer accepts 1–16 parameters, ranks 0–4, at most 1,048,576 elements per
tensor and at most 4 MiB of aggregate payload. Scalars contain one element;
empty weights are refused in this first profile. Names are 1–127 printable
non-space ASCII bytes, with quote, backslash and `__metadata__` excluded.
They are owned fixed-size metadata after inspection.

`ic_plan` validates every descriptor, checks unique names and computes all
source/payload intervals. It sorts names lexicographically, writes matching
payload order and pads the bounded JSON header with spaces to an eight-byte
boundary. Reordering the incoming map produces the same file. No source
strides, implicit transposes or implicit dtype conversions exist at this
boundary. `psionic_weights_size_v1` reports the exact required file size from
metadata; it does not read the parameter values.

`ic_write` validates every source encoding before touching its staging buffer,
then writes the prefix, header and data and publishes the complete file.
The host rejects observed output aliases with the parameter table, names and
payloads. Every refusal reports zero written and leaves public output intact;
a success preserves unused output capacity. Inspection likewise publishes
its whole owned metadata inventory only after validation. Its offsets are
absolute file offsets, and the caller retains the borrowed file bytes.

F32/BF16 payloads keep their exact little-endian encodings, including signed
zero and finite subnormals. Both export and inspection reject NaN and infinity.
The general Safetensors format allows more dtypes, empty tensors and nonfinite
values; this bounded profile does not claim them. BF16 widening, f32 narrowing,
layout materialization and name mapping must be explicit prior conversion
steps, with independent checks and an artifact identity recorded. A renamed
weight alone does not establish architectural compatibility.

## Keep a stable language boundary

The public header has versioned `*_v1` entry points and fixed-width PODs.
It requires an LP64 little-endian IEEE binary32 host. Public records have
compile-time layout assertions: `PsTensorV1` is 64 bytes, parameter information
192 bytes, and results 16 bytes. No compiler-generated Coax layout appears in
that interface; the native adapter copies metadata into the generated private
records. Future incompatible contracts require a new ABI version.

| Interface | Ownership/result |
| --- | --- |
| `psionic_weights_size_v1`, `write_v1`, `inspect_v1` | Borrow inputs during the call; return status and file bytes or parameter count |
| `psionic_tensor_copy_v1` | Copy an admitted CPU f32 descriptor into owned storage and return one handle lease |
| `psionic_tensor_view_v1` | Borrow a read-only descriptor while the handle remains live |
| `psionic_tensor_retain_v1`, `release_v1` | Acquire another explicit lease; release clears the caller's handle |
| `psionic_dlpack_import_v1` | Consume one producer carrier, borrow its storage and own its eventual deleter |
| `psionic_tensor_dlpack_v1` | Produce a read-only versioned carrier that owns an additional handle lease |

Opaque handles are capabilities, not arbitrary addresses. Callers supply valid
pointers, truthful extents and exclusive access. Handle copying without a
retain does not create a second lease. A pointer returned by `tensor_view`
does not survive the last lease. The API is synchronous; callers coordinate
producer mutation and calls themselves. Coax decides reference transitions;
C allocates/frees storage and invokes producer callbacks at those transitions.

## Exchange a live tensor through DLPack

DLPack standardizes a tensor descriptor and a managed deleter. It does not
serialize an architecture or copy the data. We vendor only the
[Apache-licensed declarations](../platform/vendor/dlpack.h) from
[DLPack v1.2](https://github.com/dmlc/dlpack/blob/v1.2/include/dlpack/dlpack.h),
with its [license](../platform/vendor/DLPACK-LICENSE).

The import path first admits the carrier kind and version. A versioned major
mismatch invokes the producer's stable-prefix deleter without reading the
later tensor fields. It then checks CPU device 0, float code/32 bits/one lane,
rank and flags before reading any shape or strides. Coax validates the copied
shape, element count, contiguous element strides and byte offset against the
observed pointer alignment and address-range arithmetic.

We accept legacy and versioned 1.0–1.2 imports. Version 1.2 requires explicit
strides for rank greater than zero; older carriers may omit them to describe
contiguous data. Nontrivial dimensions must have canonical contiguous strides;
singleton strides are immaterial. Scalars and empty live tensors are supported.
Empty imports normalize the data view to null, independent of the producer's
unused pointer. The offset is at most 4 MiB; nonempty f32 data must be aligned,
with an offset divisible by four and no address overflow. These are observed
range checks, not a proof that an arbitrary external pointer names real memory.

Exports are versioned 1.0, CPU 0, read-only, with explicit element strides and
zero offset. Read-only is a consumer contract: another producer alias can still
write the shared bytes between coordinated calls. Transport preserves f32 bits,
including nonfinite values; weight export and numerical operations impose their
own finite-value admission. GPU exchange needs device-specific stream/event
ownership and is a separate extension. There is no implicit device fallback.

## Consume once, then release once

The transfer rule is precise: a nonnull carrier, recognized kind and empty
nonnull output transfer ownership into `psionic_dlpack_import_v1`. Validation
failure then calls the producer deleter immediately. Success holds it until
the last Psionic lease disappears. An argument error before transfer leaves
ownership with the caller. A null producer deleter is permitted by DLPack;
the producer then remains responsible for the data's lifetime.

Export first acquires another Coax lease. Releasing the original handle leaves
the carrier valid. When the receiving framework calls the carrier deleter, the
lease is returned; only the last reference frees storage or calls the imported
producer's deleter. The C harness counts these transitions, including refused
imports and a bad-version carrier whose later shape pointer is deliberately
unreadable.

<!-- source: psionic/lib/interchange.coax leases -->
```jai
ic_access :: (s:IcLease)->IcStatus #export_c {
    if !s.live || s.refs<1 || s.refs>1024 {return IcStatus.ownership;}
    return IcStatus.ok;
}
ic_retain :: (s:IcLease)->IcLeaseChange #export_c {
    if ic_access(s)!=IcStatus.ok || s.refs>=1024 {return IcLeaseChange.{status=IcStatus.ownership};}
    n:=s;n.refs+=1;return IcLeaseChange.{next=n};
}
ic_release :: (s:IcLease)->IcLeaseChange #export_c {
    if ic_access(s)!=IcStatus.ok {return IcLeaseChange.{status=IcStatus.ownership};}
    n:=s;n.refs-=1;if n.refs==0 {n.live=false;}
    return IcLeaseChange.{next=n,dispose=n.refs==0};
}
```

The [NumPy tooling](../tests/interchange_numpy.py) uses actual producer capsules,
renames consumed capsules to prevent a second cleanup, and checks pointer
equality rather than inferring zero-copy from equal values. Deleting the
original array leaves its data alive through the imported lease. In the other
direction, `numpy.from_dlpack` owns the exported lease after the original
Psionic handle is released. The last NumPy view and an unconsumed capsule each
exercise their real cleanup paths. Python retains the GIL when calling producer
deleters through the shared library.

## Continue learning, export, reload

The external producer writes `scale.weight = 0.5` with NumPy and an independent
stdlib JSON/byte encoder. The public ABI inspects that artifact, the private
example invokes Coax's trainer for sixteen updates on `[1,2] -> [2,4]`, and the
public writer exports the result. An independent JSON reader and
`numpy.frombuffer` reload the payload and predict at `[-3,0,3]`, within 0.000001
absolute error of `[-6,0,6]`. The resulting weight is `1.99999976`, with MSE
`1.42108547e-13` in the qualified builds.

<!-- source: psionic/examples/model-interchange.coax example -->
```jai
main :: () {
    // The external artifact supplies a parameter; the architecture stays y=w*x.
    trained:=ic_continue(0.5,16);#assert trained.status==IcStatus.ok;
    tensor:=IcTensor.{name_count=12,shape=TvShape.{rank=1,dims=[4]int.[1,0,0,0]},bytes=4};
    name:="scale.weight";for i:0..<name.count {tensor.name[i]=name[i];}
    parameters:=[1]IcTensor.[tensor];payload:[4]u8;bits:=f32_bits(trained.weight);
    for i:0..<4 {payload[i]=cast,trunc(u8)(bits>>(8*i));}
    file:[512]u8;stage:[512]u8;written:=ic_write(parameters[..],payload[..],file[..],stage[..],false);
    #assert written.status==IcStatus.ok;
    names:[16]IcTensor;read:=ic_inspect(file[0..<written.written],names[..],false);
    #assert read.status==IcStatus.ok && read.written==1 && names[0].name_count==12;
    reloaded:=f32_from_bits(pw_word(file[..],names[0].source,4).bits);
    #assert abs(reloaded*3-6)<0.000003f32;
    print("Portable parameter: scale.weight, F32 [1], % bytes\n",names[0].bytes);
    print("Continued in Coax from 0.5: weight = %\n",reloaded);
    print("Reloaded prediction at x=3: %\n",reloaded*3);
    print("No object deserialization or original framework executes.\n");
}
```

The [native checks](../tests/interchange.c) also check canonical name order,
BF16/F32 bytes, every truncation of a valid file, duplicate/unsupported
metadata, nonfinite values, output aliases, scalar and empty live tensors,
noncontiguous carriers, offset alignment/overflow and exact deleter counts.
The [verification record](../../docs/verification/psionic-ch4a-2026-10-29.md)
binds the producer, artifacts, commands and results. This is an inference
artifact and a tensor ownership boundary; it does not qualify arbitrary model
conversion, GPU DLPack or resumable cross-framework optimizer state.

Next, [chapter 5](ch005-text-becomes-a-model-request.md) turns text into the
request consumed by original models. The broader graph exchange and training
checkpoint work remain in the [plan](PLAN.md).
