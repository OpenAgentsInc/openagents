# 3b. One tensor, several devices and dtypes

A tensor's shape does not tell us where its bytes live, how they represent a
number, or whether a device has finished writing them. We need those answers
before building a training framework on chapter 3a's operators.

This chapter implements **tensor-device-v1**: owned CPU, CUDA and Metal
allocations, explicit copies, f32/f16/bf16 storage, promotion, immutable shared
views, completion and cancellation, seeded uniform tensors, and one shared
program: `dropout(relu(a+b))`. The [support profile](../fixtures/tensor-device/profile.json)
is deliberately small. Chapter 3a's full operator registry remains CPU f32.

The [placement policy](../lib/tensor-placement.coax) and
[arithmetic kernels](../lib/tensor-device-kernels.coax) are Coax. The
[native adapter](../platform/tensor-device-runtime.c) allocates memory and
performs driver effects. It executes the same generated kernels on each GPU;
it contains no independent implementation of casting, random generation or
model arithmetic.

## Run the chapter

The [Coax example](../examples/tensor-devices.coax) runs the shared kernel on
tiny local arrays and walks the allocation policy. The native check below
also runs actual opaque tensor handles through transfers and completion.
Use a qualified Coax compiler with `cuda` and `metal-compute` support for the
respective device check; no compiler rebuild is part of this chapter's gate.

```shell
coax/bin/coax run psionic/examples/tensor-devices.coax
COAX=/path/to/qualified/coax psionic/check.sh --only tensor-device
COAX=/path/to/qualified/coax psionic/tests/check_tensor_device.sh --metal
# On the qualified NVIDIA host (set the NVRTC library path for your install):
LD_LIBRARY_PATH="$HOME/cuda-13.2/lib:${LD_LIBRARY_PATH:-}" \
  COAX=/path/to/qualified/coax psionic/tests/check_tensor_device.sh --cuda
```

The [frozen example output](../fixtures/tensor-devices.out) is:

```jai
dropout(relu(a+b)), seed 123 stream 7: 0.0, 0.0, 0.0, 12.0
promotion: f16 + f16 stays f16; f16 + bf16 becomes f32
counter reservation: first 0, count 4, next 4
released: 0 bytes, 0 pending operations
```

The GPU switches require the requested device (CUDA device 0 or the default Metal device in this profile).
An unavailable backend reports
`absent`; a compile or execution failure reports an error. The adapter never
substitutes CPU execution for a GPU operation. `ptd_export` is an explicit
host readback, and cross-device copies disclose host staging in their plan.

## A handle owns a reference, a view describes addresses

Chapter 2a's `TvView` describes rank, dimensions, strides and offset. A
`PtdTensor` adds an owned reference to one allocation and its session. Ready
storage is immutable in this profile. Creating a view retains its allocation;
dropping the original handle leaves the view valid. Copying a raw C pointer
does not retain anything: use `ptd_view` for an alias and `ptd_to` for a new
allocation. Even a same-device, same-dtype `ptd_to` produces independent storage.

A view may reverse a dimension, transpose axes or broadcast a scalar with
zero strides. The host admits it against the original allocation, and the
shared kernel maps each logical work item through its signed strides. A
copy/export gathers the logical order into contiguous packed output.
Writable views are refused. In-place mutation and chapter 2a's saved-value
version protocol remain separate; ready device tensors have no mutation API.

The policy keeps these records:

<!-- source: psionic/lib/tensor-placement.coax records -->
```jai
TdDevice :: enum u8 {cpu;cuda;metal;}
TdDtype :: enum u8 {f32;f16;bf16;}
TdStatus :: enum u8 {ok;device;dtype;shape;capacity;budget;state;busy;number;arithmetic;counter;alias;driver;absent;cancelled;memory;}
TdIds :: struct {session:int=1;storage:int=1;}
TdSessionState :: struct {identity:int;device:TdDevice;budget:int;bytes:int;allocations:int;pending:int;live:bool;poisoned:bool;}
TdState :: struct {owner:TvStorage;session:int;device:TdDevice;dtype:TdDtype;resident:int;temporary:int;refs:int;readers:int;pending:bool;error:TdStatus;}
TdDropout :: struct {seed:u32;stream:u32;first:int;keep:int=32768;training:bool=true;}
TdRng :: struct {seed:u32;stream:u32;next:int;}
```

The native ID ledger issues monotonically increasing, never-reused session
and storage identities through Coax. Public info is a snapshot of metadata;
the opaque allocation holds the authoritative reference and reader counts.
All host access is exclusive: this first runtime has no thread-safe global
registry or concurrent host API. Multiple device readers may be pending at once.

## Placement is an explicit operation

The [C interface](../platform/tensor-device-runtime.h) exposes these effects:

| Call | Contract |
| --- | --- |
| `ptd_open` | Open the requested device and compile its trusted Coax-generated module; create an empty bounded session. |
| `ptd_from_f32`, `ptd_from_bytes` | Validate shape and exact input length, allocate private storage, cast on the requested device, wait, then publish an owned handle. |
| `ptd_view` | Validate a read-only view and retain the existing allocation. |
| `ptd_to` | Allocate independent target storage, gather/cast on the target device, wait and publish it. |
| `ptd_random` | Validate the counter range and create a completed seeded tensor on the requested device. |
| `ptd_program` | Admit co-placed inputs and return a pending private output. |
| `ptd_wait`, `ptd_cancel` | Drain work, collect numerical flags and release input reader claims; cancellation stores `cancelled`. |
| `ptd_export` | Require ready finite data and enough disjoint caller storage; stage a complete logical host copy before publication. |
| `ptd_release`, `ptd_close` | Release an idle reference, or close a session with no remaining allocations/work. Busy calls leave their handles intact. |

Inputs to the program must belong to its session. A foreign device/session
returns `device` until the caller copies it explicitly. Same-session device
copies read the original buffer directly. CPU-to-GPU, GPU-to-CPU and copies
between GPU sessions stage bytes through the host. Cross-device peer copies,
pinned transfer pools and automatic placement are future work.

A session admits at most 1 GiB of tensor allocations; rank and logical/storage
element counts retain chapter 2a's bounds of four and 1,048,576. Each root
contains packed payload, aligned per-element numerical flags and 128 bytes of
kernel metadata. Coax admits resident bytes plus any temporary target transfer
buffer before allocation. Temporary device storage is freed after completion.
Host transport/readback scratch is separately bounded by those element limits;
driver contexts, compiled modules and allocator overhead are outside this
payload budget. An empty tensor still owns its metadata allocation.

## Storage format is a contract

Storage bytes are little-endian. f32 uses four bytes, f16 and bf16 two. We
classify non-finite encodings as integers before making a floating-point
value. A cast refusing NaN or infinity publishes no tensor. A finite value
that rounds to infinity reports `arithmetic`. Signed zero and finite
subnormal bits survive casts.

Casting uses integer round-to-nearest-even, independent of a device's native
half instructions. For half precision, normal fractions retain ten bits;
subnormal conversion shifts the implicit leading bit into the fraction. The
half decoder makes that leading bit explicit:

<!-- source: psionic/lib/tensor-device-kernels.coax decode -->
```jai
tdk_decode_bits :: (word:u32,dtype:int)->u32 {
    if dtype==0 {return word;}if dtype==2 {return word<<16;}
    sign:=(word&0x8000)<<16;exponent:=int((word>>10)&31);fraction:=word&1023;
    if exponent==31 {return sign|0x7f800000|(fraction<<13);}
    if exponent==0 {if fraction==0 {return sign;}exponent=-14;for shift:0..<10 {if fraction<1024 {fraction<<=1;exponent-=1;}}return sign|(u32(exponent+127)<<23)|((fraction&1023)<<13);}
    return sign|(u32(exponent+112)<<23)|(fraction<<13);
}
```

Each work item writes its private output and a flag: zero means success,
one a non-finite source, two a non-finite arithmetic/narrowing result. The
host waits before reading flags. One failed item prevents publication of the
whole result, even if other work items wrote their private output successfully.

Promotion is explicit and symmetric:

| left / right | f32 | f16 | bf16 |
| --- | --- | --- | --- |
| f32 | f32 | f32 | f32 |
| f16 | f32 | f16 | f32 |
| bf16 | f32 | f32 | bf16 |

The program decodes both operands to f32, adds, applies ReLU and dropout in
f32, then narrows once to the promoted output dtype. This is a specific
mixed-precision rule, rather than an implicit autocast system. There is no
low-precision accumulation or automatic loss scaling here.

## Randomness belongs to a logical counter

A mutable device-global random stream makes reproducibility depend on launch
order. Instead each request specifies a seed, stream and first logical
counter. `td_rng_reserve` returns a range and an updated value record. The
caller carries that record forward; allocating or releasing a tensor consumes
no counters. A range never wraps beyond the 32-bit counter space.

The teaching generator is Counter32-v1:

<!-- source: psionic/lib/tensor-device-kernels.coax rng -->
```jai
tdk_word :: (seed:u32,stream:u32,counter:u32)->u32 {
    x:=add_wrap(seed,mul_wrap(0x9e3779b9,add_wrap(counter,1)));x^=mul_wrap(stream,0x85ebca6b);
    x^=x>>16;x=mul_wrap(x,0x7feb352d);x^=x>>15;x=mul_wrap(x,0x846ca68b);x^=x>>16;return x;
}
```

Its odd multiplications and xor shifts give a deterministic 32-bit
permutation for a fixed key. This is a reproducibility primitive; we have not
qualified its statistical quality for large training runs or cryptography.

Uniform values use a dtype-specific exact grid: the top 24 bits divided by
2^24 for f32, top 11 divided by 2048 for f16, top 8 divided by 256 for bf16.
Every stored value lies in `[0,1)`; narrowing cannot round it up to one.
Generating 257 values in one request or chunks of 52 and 205 with consecutive
counters produces identical bytes. Changing dtype deliberately changes that
grid. The same dtype, seed, stream and logical range give identical CPU,
CUDA and Metal bytes, independent of allocation identity.

Dropout keeps an item when its counter word's top 16 bits are below `keep`,
a Q16 threshold in `[0,65536]`, and scales it by `65536/keep`. A zero threshold
drops everything; evaluation mode bypasses dropout. Shape/keep/counter
admission still applies in evaluation mode. Counter assignment follows
logical output indices, including strided/broadcast inputs.

## Completion protects lifetimes

`ptd_program` borrows each input allocation until its output completes.
Passing two views of the same root takes two reader claims on that root.
Release is refused while any claim remains. A pending output also refuses
export, copy, view creation and release. Call `ptd_wait` or `ptd_cancel`
explicitly, then release it.

CPU computation currently happens during the call but retains this pending
state until the explicit wait. CUDA queues immediately on the session's
stream. Metal encodes a private batch and commits it at wait. These expose
the same lifetime contract; they do not promise identical scheduling or
host/device overlap. Waiting for one of two pending readers releases only
its own claims. Repeated waits do not release them twice.

Cancellation drains device work before releasing its inputs and marks its
private result unusable; it is not preemption. Numerical errors invalidate
that result while leaving the session usable. Driver failures poison the
session, refusing further execution, but completed/failed allocations can
still be released. Session close refuses live allocations and pending work.
The host must never call through a freed handle.

## One source, measured numerical behavior

The shared program is:

<!-- source: psionic/lib/tensor-device-kernels.coax program -->
```jai
tdk_program :: (item:int,ad:int,bd:int,od:int,seed:u32,stream:u32,first:int,keep:int,training:int,meta:[]int,a:[]u8,b:[]u8,out:[]u8,flags:[]u8) #export_c {
    ab:=tdk_decode_bits(tdk_get(a,tdk_address(item,meta,false),ad),ad);bb:=tdk_decode_bits(tdk_get(b,tdk_address(item,meta,true),bd),bd);flags[item]=0;
    if !tdk_finite(ab) || !tdk_finite(bb) {flags[item]=1;return;}
    sum:=f32_from_bits(ab)+f32_from_bits(bb);if !tdk_finite(f32_bits(sum)) {flags[item]=2;return;}
    value:f32=0;if sum>0 {value=sum;}
    if training!=0 {if keep==0 || int(tdk_word(seed,stream,cast(u32)(first+item))>>16)>=keep {value=0;}else {scale:=65536.0f32/f32(keep);value*=scale;}}
    if !tdk_finite(f32_bits(value)) {flags[item]=2;return;}word:=tdk_encode_bits(f32_bits(value),od);if !tdk_finite(tdk_decode_bits(word,od)) {flags[item]=2;return;}tdk_put(out,item,od,word);
}
```

[Native checks](../tests/tensor_device.c) run it on all three backends and
all nine dtype pairs. Normal fixtures with a half-probability dropout mask
match independent expected values bit for bit. Packed casts and seeded
uniform bytes are exact. Arithmetic uses frozen scaled error bounds
`abs(actual-reference)/max(1,abs(reference))`: 2e-6 for f32, 2^-9 for f16,
and 2^-6 for bf16. These permit rounding in the general non-power-of-two
scale, not changes to counter masks or placement.

The [verification record](../../docs/verification/psionic-ch03b-2026-10-29.md)
reports actual errors and subnormal arithmetic behavior on the tested CPUs,
RTX 4080 and Apple M5 Max. In the four-value diagnostic fixture, CPU/CUDA
preserve all four results; Metal flushes the three subnormal inputs. Integer casts preserve every finite f16/bf16
encoding, even on a device whose f32 arithmetic flushes subnormals. General
arithmetic must use the reported device profile rather than assume universal
bit equality.

The focused checks cover every finite 16-bit encoding, independent mathematical
rounding-boundary vectors of both signs, split random requests, promotions,
reversed/transposed/broadcast views, owned transfers, overlapping exports,
concurrent reader claims, pending operations, cancellation, numerical overflow,
malformed inputs, budgets, empty tensors and cleanup. They run debug/release
native builds with ASan/UBSan. Full mutation runs are omitted under the owner's
focused-check instruction; no mutation-completeness claim is made.

This gives the framework track a real storage/device boundary for later
operators and autodiff. Extending the 3a registry to GPUs, an allocator pool,
general graph scheduling, automatic differentiation across device copies and
high-throughput fused training kernels remain separate work.
