# 2a. A tensor is a view of memory

A matrix does not need its own allocation. The same six numbers can describe
a two-by-three matrix, its transpose, one row, or a reversed row. The difference
is the rule that maps a coordinate to a storage address.

[Chapter 2](ch002-shapes-and-storage.md) deliberately admitted only contiguous,
nonempty tensors. We keep that contract intact and introduce
[`tensor-view.coax`](../lib/tensor-view.coax), a separate bounded CPU view layer.
It is the storage foundation for chapter 3a's future general operators.

## Run it

The [example](../examples/tensor-views.coax) needs no model or dataset:

```shell
coax/build.sh
coax/bin/coax run psionic/examples/tensor-views.coax
psionic/check.sh --only tensor-view
```

Its [checked transcript](../fixtures/tensor-views.out) is:

```jai
transpose [3, 2], strides [1, 3]: 1.0, 4.0, 2.0, 5.0, 3.0, 6.0
write through transpose: 10.0, 30.0, 50.0, 20.0, 40.0, 60.0
storage version: 1; saved value changed before backward
broadcast [2, 3], strides [0, 1]: read-only; write refused
empty slice: 0 elements, no address is read
retired storage: existing views refused
```

## Separate storage from a view

The caller owns the actual `f32` allocation and one authoritative storage
record. Views carry its identity and allocation epoch, a shape, signed element
strides, an absolute element offset, and write permission. They contain no raw
pointer and allocate no memory.

<!-- source: psionic/lib/tensor-view.coax types -->
```jai
TvShape :: struct {rank:int;dims:[4]int;}
TvStorage :: struct {identity:int;epoch:int;elements:int;version:int;live:bool;}
TvView :: struct {identity:int;epoch:int;shape:TvShape;strides:[4]int;offset:int;writable:bool;}
TvLayout :: struct {status:TvStatus;elements:int;low:int;high:int;contiguous:bool;injective:bool;}
TvSaved :: struct {status:TvStatus;identity:int;epoch:int;version:int;}
```

A view's shape can contain zero through four axes. A scalar has rank zero and
one element. An active zero extent makes an empty tensor with zero elements.
Inactive dimensions and strides are zero. Each active extent and absolute
stride is at most 1,048,576; a nonempty logical tensor and its backing storage
also have at most 1,048,576 elements. These bounds make the integer proofs
small and explicit. All products are checked or bounded before evaluation.

For coordinate `i`, the address is:

```jai
address = offset + sum(i[axis] * stride[axis])
```

For `[2, 3]` with strides `[3, 1]`, coordinate `[1, 2]` addresses element five.
Transposing swaps shape and strides: `[3, 2]` with `[1, 3]` visits storage in
logical order `0, 3, 1, 4, 2, 5`. No coefficient moves. A dimension of length
one can have any admitted stride without affecting contiguity.

We re-admit metadata at every public data boundary. Public structs are not
trusted cached proofs. For each axis, the minimum contribution is the smaller
of zero and `(extent-1)*stride`; the maximum is the larger. Summing these with
the origin bounds every reachable address. A nonempty view must remain inside
the backing storage. An empty view dereferences nothing and may have an origin
at one past the storage end; its reported low/high fields both equal that
origin and are not reachable addresses. Its canonical contiguous strides are
zero, and it is considered contiguous and injective.

## Slice, transpose and broadcast

`tv_slice(storage, view, axis, start, count, step)` uses an explicit count.
Axes and starts are nonnegative; a step is nonzero and can be negative. We
check the first and last selected coordinates before constructing the view.
Slicing a three-column row with `start=2, count=3, step=-1` reverses it.
An empty slice permits `start=extent`, keeps the previous origin and uses a
zero stride on that axis. There is no ambiguous negative endpoint convention.

`tv_transpose` exchanges two axes and their strides. `tv_broadcast` aligns
axes from the right and permits equal extents or expansion of a source extent
of one. Inserted or expanded axes have zero stride. An extent one can expand
to zero; zero cannot expand to one. Every broadcast result is read-only,
including a broadcast that happens to preserve its original shape.

For a three-element row broadcast to `[2, 3]`, strides `[0, 1]` make both rows
refer to the same three values. Reads are well defined. An ordinary elementwise
write would have multiple logical destinations for the same cell, so this
chapter refuses that write.

Writable views need a sufficient injectivity proof. Sort non-singleton axes by
absolute stride, start with a covered span of one, and require each next stride
to step beyond the span covered by the earlier axes. Then extend the covered
span. This admits contiguous, transposed, reversed and many gapped layouts.
It can refuse unusual injective layouts; we do not claim to solve every integer
alias problem. Read-only views may overlap themselves.

`tv_alias` compares two admitted views of the same owner. Empty views do not
alias; separated address intervals prove disjointness. Intersecting intervals
report `may_alias`, even when even/odd interleaving happens to use disjoint
cells. The answer is conservative rather than a promise of exact overlap.
Always inspect status first: an error clears the payload, so a false
`may_alias` field on an error is not a disjointness proof.

## Materialize and write through a view

`tv_gather` materializes logical row-major order into a contiguous output.
`tv_scatter` writes a contiguous source through a writable view. All coordinate
arithmetic and copying remain in Coax. The small
[native adapter](../platform/tensor-view-memory.c) observes the actual declared
address ranges, including the storage record, and passes overlap to Coax.

Both boundaries require exactly one owner record and an exact backing-data
extent. Gather accepts output capacity; scatter requires an exact source
count. Scratch has capacity for all logical elements. Writable spans, including
their declared tails, must be disjoint from read spans and each other. Gather
checks only selected values; scatter checks all source values. Nonfinite
selected/source values refuse. Signed zero and other finite representations
are copied unchanged.

All validation finishes before staging or publication. Every refusal reports
zero written and preserves data, owner, output and scratch. Success preserves
unselected storage and spare writable capacity. Empty reads/writes do no data
work, and an empty write does not change the storage version.

## A saved value remembers the storage version

Backward often needs values from the forward pass. A saved view must notice a
write through any alias of its storage. Saving records identity, epoch and the
current version, rather than copying all the values:

<!-- source: psionic/lib/tensor-view.coax versions -->
```jai
tv_save :: (s:TvStorage,v:TvView)->TvSaved #export_c {
    r:=tv_layout(s,v);if r.status!=TvStatus.ok {return TvSaved.{status=r.status};}
    return TvSaved.{identity=s.identity,epoch=s.epoch,version=s.version};
}
tv_saved_status :: (s:TvStorage,saved:TvSaved)->TvStatus #export_c {
    if saved.status!=TvStatus.ok || !tv_storage_valid(s) || saved.identity!=s.identity || saved.epoch!=s.epoch {return TvStatus.state;}
    if saved.version!=s.version {return TvStatus.modified;}return TvStatus.ok;
}
```

A successful nonempty scatter increments the authoritative version exactly
once, even if the new bits equal the old ones. A version at the signed integer
maximum refuses a nonempty write before touching anything. Failed writes do
not invalidate saved values. The check is deliberately storage-wide: even a
write to a disjoint slice invalidates every saved token of that owner.

`tv_retire` marks the record dead before the caller releases the allocation.
Every old view then refuses. Reusing an allocation requires a fresh identity or
epoch; a matching pointer does not resurrect an old view. A replacement storage
record must not reset a live owner's version or reuse its identity/epoch.

This is explicit borrowed-storage ownership. The caller maintains one
authoritative record per allocation, binds it to the truthful data span,
preserves identities/epochs, routes every mutation through scatter, and keeps
exclusive access during each call. Native structs can be forged and arbitrary
external pointer writes cannot be detected. This chapter adds neither automatic
allocation/reference counting nor a concurrent memory transaction. Device
placement and allocator ownership remain chapter 3b; integrating saved tokens
with the general backward graph remains chapter 71b.

## Evidence and the next step

The [native check](../tests/tensor_view.c) independently enumerates addresses
and uniqueness for 28,350 small layouts, rather than asking another view helper
for the expected answer. It checks every supported rank and transposition,
actual aliases, short buffers, signed zero, infinity and quiet/signaling NaN
representations, version overflow, retired/reused epochs, null empty spans and
a maximum-size transposed materialization. Debug/release and native
ASan/UBSan supplement the authored Coax checks and exact example transcript.

The [verification record](../../docs/verification/psionic-ch002a-2026-10-29.md)
names the tested build and its limits. Chapter 3a can now implement operators
against shared layout and mutation rules. Existing model engines retain their
qualified contiguous interfaces until each migration has its own checks.
