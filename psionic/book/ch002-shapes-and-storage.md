# 2. Shapes and storage

Chapter 1 learned one number. A matrix needs more than a list of numbers: it
needs dimensions, an order for storing them, and enough memory for the work.
By the end of this chapter we can describe a small tensor, locate one of its
values, and copy it into caller-owned storage while checking those boundaries.

The implementation is [lib/tensor.coax](../lib/tensor.coax). The example in
[examples/shape-storage.coax](../examples/shape-storage.coax) calls that same
library. We add no allocator, matrix arithmetic or automatic differentiation
in this chapter. [Chapter 1](ch001-learning-a-number.md) remains a regression.

## Run it

From the repository root:

```shell
coax/build.sh
coax/bin/coax run psionic/examples/shape-storage.coax
```

The current output is:

```jai
scalar: elements = 1, bytes = 4
matrix [2, 3]: elements = 6, bytes = 24
strides = [3, 1], index [1, 2] = 5
copied 6 values: first = 10.0, last = 60.0, spare output = -7.0
refused: five output slots cannot hold six values; storage unchanged
refused: an empty axis and an oversized element count
```

Check both chapters, including native debug/release and sanitizers, with:

```shell
psionic/check.sh
```

No model files or external dataset are needed. Python is verification tooling;
the library and example require Coax and the host C compiler.

## Give a flat list a shape

A **tensor** here is contiguous `f32` storage with a shape. A scalar has rank
zero, a vector has rank one, and a matrix has rank two. Rank counts axes;
the extent of each axis says how many positions that axis has.

Our example is a matrix with two rows and three columns:

```jai
row 0: [10, 20, 30]
row 1: [40, 50, 60]

shape = [2, 3]
elements = 2 * 3 = 6
bytes = 6 * 4 = 24
```

`f32` is four bytes on this tested binary32 target. Shape metadata does not
allocate those bytes or prove that a pointer owns them. It describes a request
that a caller must back with real storage.

This chapter's CPU profile makes a small, explicit choice:

| Item | Contract |
| --- | --- |
| Rank | Zero through four, inclusive. |
| Active dimensions | Positive integer extents. An empty axis is refused. |
| Inactive dimensions | Zero; metadata has exactly four dimension slots. |
| Scalar | Rank zero, all dimensions zero, one element and four data bytes. |
| Size | At most 1,048,576 logical elements, hence at most 4,194,304 data bytes per tensor. |
| Layout | Contiguous, row-major `f32`; unused stride slots are zero. |
| Coordinates | Four slots; active coordinates are zero-based and below their extents, inactive coordinates are zero. |

A rank-zero scalar and a rank-one vector of length one both store one value.
They have different shapes. A rank-one vector of length zero is refused;
it is not another spelling of a scalar.

The element limit is a bound for this teaching profile. Larger real-model
buffers will need an explicitly qualified larger profile. Broadcasting,
strided views, transposes, additional dtypes and empty tensors remain future
contracts. Nothing silently reshapes or converts an unsupported request.

These are the actual small metadata/result types:

<!-- source: psionic/lib/tensor.coax metadata -->
```jai
PsionicShape :: struct { rank: int; dims: [4] int; }
PsionicLayout :: struct { status: PsionicTensorStatus; elements: int; bytes: int; strides: [4] int; }
PsionicIndex :: struct { status: PsionicTensorStatus; offset: int; }
PsionicCopy :: struct { status: PsionicTensorStatus; written: int; }
```

The caller keeps the shape and the backing allocation separately. No result
retains a borrowed data view. A failed layout returns its status with zero
elements, bytes and strides; the default zeros do not describe an admitted
empty tensor. Always inspect `status` before using result fields.

## Count before multiplying

Multiplying arbitrary dimension metadata first is unsafe. For example,
`INT64_MAX * INT64_MAX` cannot be represented by the target's Coax `int`.
A release build must refuse that request before integer wrapping can make it
look small.

For positive integers, the admission test can be rearranged:

```jai
current_elements * next_dimension <= limit
next_dimension <= limit / current_elements
```

We start with one element, so the divisor is never zero. Each admitted
multiplication leaves the count between one and the limit. Only then do we
multiply by four for the byte count, whose maximum is already known to fit.
All active/inactive dimensions are checked before product admission, giving
shape errors a deterministic precedence over a size error.

The same backward walk gives the **strides**. A stride counts how many float
elements to advance when a coordinate increases by one; it is not a byte count.

<!-- source: psionic/lib/tensor.coax layout -->
```jai
psionic_tensor_layout :: (shape: PsionicShape) -> PsionicLayout #export_c {
    if shape.rank < 0 || shape.rank > 4 { return PsionicLayout.{status = PsionicTensorStatus.rank}; }
    for axis: 0..<4 {
        if axis < shape.rank {
            if shape.dims[axis] <= 0 { return PsionicLayout.{status = PsionicTensorStatus.dimension}; }
        } else {
            if shape.dims[axis] != 0 { return PsionicLayout.{status = PsionicTensorStatus.dimension}; }
        }
    }
    elements := 1; axis := shape.rank; strides: [4] int;
    while axis > 0 {
        axis -= 1;
        dim := shape.dims[axis];
        if dim > 1048576 / elements { return PsionicLayout.{status = PsionicTensorStatus.limit}; }
        strides[axis] = elements;
        elements *= dim;
    }
    return PsionicLayout.{status = PsionicTensorStatus.ok, elements = elements, bytes = elements * 4, strides = strides};
}
```

For `[2, 3]`, the last axis has stride one and the row axis has stride three.
For `[2, 3, 4, 5]`, the strides are `[60, 20, 5, 1]` and there are 120 values.
These are small independent check fixtures, not observations copied from the
implementation. The scalar has no active axes; its strides are all zero.

## Find one value

Rows are stored consecutively:

```jai
storage = [10, 20, 30, 40, 50, 60]
offset(row, column) = row * 3 + column
offset(1, 2) = 1 * 3 + 2 = 5
```

The general implementation uses the equivalent repeated multiply-and-add
form. It revalidates the shape and every coordinate before computing an
offset, so a caller cannot forge a layout result and use it as admission.

<!-- source: psionic/lib/tensor.coax index -->
```jai
psionic_tensor_index :: (shape: PsionicShape, coordinates: [4] int) -> PsionicIndex #export_c {
    layout := psionic_tensor_layout(shape);
    if layout.status != PsionicTensorStatus.ok { return PsionicIndex.{status = layout.status}; }
    for axis: 0..<4 {
        if axis < shape.rank {
            if coordinates[axis] < 0 || coordinates[axis] >= shape.dims[axis] {
                return PsionicIndex.{status = PsionicTensorStatus.index};
            }
        } else {
            if coordinates[axis] != 0 { return PsionicIndex.{status = PsionicTensorStatus.index}; }
        }
    }
    offset := 0;
    for axis: 0..<shape.rank { offset = offset * shape.dims[axis] + coordinates[axis]; }
    return PsionicIndex.{status = PsionicTensorStatus.ok, offset = offset};
}
```

At each active axis, the new offset remains below the product of dimensions
visited so far. That product is at most the admitted total count. The final
offset is therefore between zero and `elements - 1`, with no large intermediate
integer. A scalar's only coordinate tuple is `[0, 0, 0, 0]`, at offset zero.
An invalid coordinate returns `index` and offset zero; offset zero alone is
not a success signal.

The native oracle starts with each flat position and derives its coordinates
using independent suffix products and division. This checks 1,555 positions
across 121 small shapes, rather than implementing the same indexing loop twice.

## Storage belongs to the caller

Read [Coax's memory chapter](../../coax/book/ch06-memory.md) for borrowed views.
Our copy takes three views that remain alive for the entire synchronous call:

| View | Meaning |
| --- | --- |
| `source` | Exactly the shaped tensor's logical element count. Slice a larger allocation to that count before calling. |
| `destination` | Writable capacity of at least that count; successful copy leaves unused tail values alone. |
| `scratch` | Separate writable capacity of at least that count, used for staging; its unused tail also stays unchanged. |

The copy accepts every finite binary32 value, including values larger than
16, signed zero and subnormals. It refuses infinities and NaNs. It preserves
the admitted values' bits on the tested CPU profile; it performs no float
arithmetic. Chapter 1's `[-16, 16]` restriction belongs to its trainer, not to
tensor storage. Each later arithmetic kernel will need its own numeric
contract and error bounds.

The three declared spans must be disjoint, including unused writable capacity.
Exact aliases and partial overlaps are both refused. Adjacent half-open spans
are disjoint. Input memory beyond the logical source view is outside the call
and is neither inspected nor copied.

An ordinary native caller uses [platform/tensor-memory.h](../platform/tensor-memory.h).
The C adapter observes addresses through [spans.h](../platform/spans.h), shared
with chapter 1. It reports whether disjoint storage could be established;
Coax owns the decision to refuse. Negative counts, nonempty null spans and
unrepresentable address ranges cannot establish disjoint storage. Coax checks
shape/capacity first, so those earlier failures still have their stated status.

This observation does not establish allocation ownership, alignment or lifetime.
Native callers must provide valid float storage with truthful extents, keep it
alive, and avoid concurrent reads/writes during the call. A fabricated pointer
with a plausible range cannot be made safe by comparing its address.

## Refuse first, stage next, publish last

The checked Coax entry point receives the native overlap observation. Our
example can supply `false` because its three local arrays are distinct.
That shortcut is justified by the example's storage, not a general solution
for unknown native pointers.

<!-- source: psionic/lib/tensor.coax copy -->
```jai
psionic_tensor_copy_checked :: (shape: PsionicShape, source: [] f32, destination: [] f32,
        scratch: [] f32, overlap: bool) -> PsionicCopy #export_c {
    layout := psionic_tensor_layout(shape);
    if layout.status != PsionicTensorStatus.ok { return PsionicCopy.{status = layout.status}; }
    if source.count != layout.elements || destination.count < layout.elements || scratch.count < layout.elements {
        return PsionicCopy.{status = PsionicTensorStatus.capacity};
    }
    if overlap { return PsionicCopy.{status = PsionicTensorStatus.alias}; }
    for value: source {
        if !psionic_tensor_finite_bits(f32_bits(value)) { return PsionicCopy.{status = PsionicTensorStatus.number}; }
    }
    for i: 0..<layout.elements { scratch[i] = source[i]; }
    for i: 0..<layout.elements { destination[i] = scratch[i]; }
    return PsionicCopy.{status = PsionicTensorStatus.ok, written = layout.elements};
}
```

All validation finishes before either writable span changes. Scratch receives
the complete logical tensor, then the destination receives that staged result.
A copy alone could use less scratch; this simple operation establishes the
staging boundary that later fallible numerical kernels will build on.

| Failure | Condition, in precedence order |
| --- | --- |
| `rank` | Rank is outside zero through four. |
| `dimension` | An active extent is nonpositive or an inactive extent is nonzero. |
| `limit` | The product would exceed the profile's element limit. |
| `capacity` | Source length differs from the logical count, or destination/scratch is short. |
| `alias` | Native observations cannot establish disjoint declared spans. |
| `number` | A logical source value is non-finite. |

Every refusal returns `written = 0` and leaves source, destination and scratch
unchanged. Success returns `ok` and the exact logical count. These are call-level
publication rules, not atomic CPU writes or a concurrent-reader guarantee.
The caller must not install a failed result as a successful tensor operation.

## The complete example

<!-- source: psionic/examples/shape-storage.coax example -->
```jai
main :: () {
    scalar := psionic_tensor_layout(PsionicShape.{});
    #assert scalar.status == PsionicTensorStatus.ok;
    shape := PsionicShape.{rank = 2, dims = [4] int.[2, 3, 0, 0]};
    matrix := psionic_tensor_layout(shape);
    last := psionic_tensor_index(shape, [4] int.[1, 2, 0, 0]);
    #assert matrix.status == PsionicTensorStatus.ok && last.status == PsionicTensorStatus.ok;
    print("scalar: elements = %, bytes = %\n", scalar.elements, scalar.bytes);
    print("matrix [2, 3]: elements = %, bytes = %\n", matrix.elements, matrix.bytes);
    print("strides = [%, %], index [1, 2] = %\n", matrix.strides[0], matrix.strides[1], last.offset);

    source := [6] f32.[10, 20, 30, 40, 50, 60];
    destination := [8] f32.[-7, -7, -7, -7, -7, -7, -7, -7];
    scratch := [8] f32.[-9, -9, -9, -9, -9, -9, -9, -9];
    // Local arrays are disjoint. Native callers use the address-observing adapter.
    copied := psionic_tensor_copy_checked(shape, source[..], destination[..], scratch[..], false);
    #assert copied.status == PsionicTensorStatus.ok && copied.written == 6;
    for i: 0..<6 { #assert destination[i] == source[i] && scratch[i] == source[i]; }
    #assert destination[6] == -7 && destination[7] == -7 && scratch[6] == -9 && scratch[7] == -9;
    print("copied % values: first = %, last = %, spare output = %\n",
        copied.written, destination[0], destination[last.offset], destination[6]);

    refused := psionic_tensor_copy_checked(shape, source[..], destination[0..<5], scratch[..], false);
    #assert refused.status == PsionicTensorStatus.capacity && refused.written == 0;
    for i: 0..<6 { #assert destination[i] == source[i] && scratch[i] == source[i]; }
    #assert destination[6] == -7 && destination[7] == -7 && scratch[6] == -9 && scratch[7] == -9;
    print("refused: five output slots cannot hold six values; storage unchanged\n");

    empty_axis := psionic_tensor_layout(PsionicShape.{rank = 2, dims = [4] int.[2, 0, 0, 0]});
    oversized := psionic_tensor_layout(PsionicShape.{rank = 2, dims = [4] int.[1024, 1025, 0, 0]});
    #assert empty_axis.status == PsionicTensorStatus.dimension && oversized.status == PsionicTensorStatus.limit;
    print("refused: an empty axis and an oversized element count\n");
}
```

The first copy accepts values from 10 through 60, writes six floats and leaves
two spare slots in each writable array unchanged. The next call supplies only
five output slots. The program asserts that it gets `capacity`, reports zero
writes, and preserves both arrays' earlier contents. The final two requests
exercise an empty axis and a product just above the limit.

## Checks and their limits

The library begins with authored `#test` and finite `#forall` checks. They pin
scalar/rank behavior, canonical dimensions, product boundaries, strides, index
bounds, capacity combinations, failure precedence, finite bit classification,
copying and untouched tails. The authority mutation command requires zero
survivors, with no equivalent exemptions:

```shell
coax/bin/coax test psionic/lib/tensor.coax
coax/bin/coax test psionic/lib/tensor.coax --release
coax/bin/coax mutate psionic/lib/tensor.coax
```

The evaluator refuses constructing actual non-finite floats at compile time.
The pure checks therefore exercise their integer bit encodings; the
[native harness](../tests/tensor.c) supplies real infinities/NaNs at every
source position, including the first and last. It also checks the generated
ABI, true short allocations, negative/extreme span counts, actual partial
aliases, adjacent storage, and source/scratch/destination preservation.

The native runs include a 1,048,576-element copy backed by heap allocations.
The data alone use about 12 MiB across input, output and scratch, plus two
spare floats. This is an exercised size boundary, not a speed or peak-process-
memory benchmark. All native tests run in debug and optimized modes, plain
and under ASan/UBSan. [The verification record](../../docs/verification/psionic-ch02-2026-10-28.md)
records the actual run and its scope.

The [example manifest](examples.json) covers all three book programs. The
checker compares these listings directly with their source markers, validates
links, and runs every example against an exact transcript and exit status in
both build modes. Small fixtures, finite checks, mutations and sanitizer runs
do not prove all possible inputs safe. Native pointer obligations remain real.

## Try a change

Change the example's shape to `[3, 2]` without changing the six source values.
Before running it, derive the strides and the offset of `[2, 1]`. Update the
coordinate and expected output; the final value should still be 60. Then
try a nonzero inactive dimension and check that both writable arrays stay
unchanged on refusal.

[Next we build dense CPU operations](ch003-numbers-and-matrices.md) over this
admitted storage. Shapes,
counts and publication boundaries stay explicit while the numerical work grows.
