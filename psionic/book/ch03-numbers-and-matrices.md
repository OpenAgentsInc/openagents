# 3. Numbers and matrices

We can now give arrays [checked shapes and storage](ch02-shapes-and-storage.md).
This chapter puts numbers through that storage: add two arrays, reduce rows,
multiply matrices, gather rows, form probabilities and normalize a row.
These are small CPU operations that later models will share.

The actual library is [lib/ops.coax](../lib/ops.coax), with request/resource
admission in [lib/ops-contract.coax](../lib/ops-contract.coax). The [runnable program](../examples/numbers-matrices.coax)
uses those files. This chapter is forward numerical execution; gradients,
learned layers and imported checkpoints follow later.

## Run the complete example

From the repository root:

```shell
coax/build.sh
coax/bin/coax run psionic/examples/numbers-matrices.coax
```

The current transcript is:

```jai
matmul [2,3] x [3,2]: [58.0, 64.0, 139.0, 154.0]
row sums: [6.0, 15.0]
row dots with themselves: [14.0, 77.0]
add: first = 2.0, last = 12.0
gather [1,0,1]: first = 4.0, middle = 1.0, last = 6.0
softmax [1000,1000]: [0.5, 0.5]
RMSNorm [3,4], epsilon 3.5: [0.75, 1.0]
LayerNorm [1,3], epsilon 3: [-0.5, 0.5]
refused: all-masked row; destination and scratch unchanged
```

Check the entire current book and library:

```shell
psionic/check.sh
```

This adds no model download, BLAS dependency or Python runtime. Python is
verification tooling; all request decisions and tensor arithmetic are Coax.
The language's scalar `exp`/`sqrt` intrinsics use the target's math library.
We implement the array algorithms around them.

## Specify the numbers before calculating

A numeric type alone does not specify an algorithm. Addition order, product
precision, conversion and exceptional values can change a model's results.
This chapter names its profile **CPU-f32-wide-v1**:

| Choice | Contract |
| --- | --- |
| Storage | Contiguous row-major binary32 (`f32`), with chapter 2's element limit. |
| Intermediates | Binary64 (`f64`), explicitly promoted before products or differences. |
| Reduction | Increasing inner index, using compensated binary64 addition. |
| Output | One narrowing to `f32` per output. Add/sum/dot/matmul refuse when the computed wide result lies outside the finite `f32` interval. |
| Rounding | Round to nearest, ties to even; no contraction into FMA, fast-math or reassociation. |
| Input values | Every logical float must be finite, including masked logits and unselected table rows. No inherited `[-16, 16]` cap. |
| Underflow | A tiny result may round to a subnormal or signed zero; this is allowed. |
| Backends | The tested Apple LP64 CPU. Different accumulation, devices and scalar math implementations need separate qualification. |

This is an explicit wider-accumulation profile. Chapter 1 keeps its original
`f32` training arithmetic. A future optimized `f32` kernel cannot silently
replace this profile and inherit its results or tolerances.

Finite `f32` products fit in `f64`: their significands need at most 48 bits,
while binary64 provides 53. Even a million maximum-magnitude products total
less than `1.3e83`, far below binary64 overflow. We can therefore reject
an out-of-range final result before attempting its `f32` conversion. Cancellation
between large products can still produce a valid small answer.

## One request, explicit operand shapes

The checked execution boundary uses a small operation tag and dimensions.
It provides one place to validate a request and publish a result. The kernels
remain separate Coax procedures.

<!-- source: psionic/lib/ops-contract.coax request -->
```jai
PsionicOp :: enum u8 { add; sum; dot; matmul; gather; softmax; rmsnorm; layernorm; }
PsionicOpStatus :: enum u8 { ok; operation; shape; limit; parameter; capacity; alias; number; index; mask; arithmetic; }
PsionicRequest :: struct { op: PsionicOp; rows: int; width: int; columns: int; epsilon: f32; }
PsionicPlan :: struct { status: PsionicOpStatus; a: int; b: int; indices: int; mask: int; output: int; }
PsionicResult :: struct { status: PsionicOpStatus; written: int; }
```

`rows` and `width` describe input A as `[rows, width]`. `columns` has a
meaning only for matrix multiply and gather; it must be zero otherwise.
The plan determines exact read-only view counts and required writable capacity:

| Operation | Input B / auxiliary input | Result |
| --- | --- | --- |
| `add` | B has the same shape as A. | `[rows, width]`, elementwise `A + B`. |
| `sum` | B is empty. | `rows` values, one sum per row. |
| `dot` | B has the same shape as A. | `rows` values, one dot product per corresponding pair of rows. |
| `matmul` | B is `[width, columns]`. | `[rows, columns]`. Set `columns = 1` for matrix-vector multiplication. |
| `gather` | Exactly `columns` integer row indices; B is empty. | `[columns, width]`, preserving request order and repetitions. |
| `softmax` | A byte mask with the same logical count as A; B is empty. | One probability row per input row. |
| `rmsnorm`, `layernorm` | Positive finite `epsilon`; B is empty. | One normalized row per input row, without learned scale or bias. |

All read-only view counts are exact. An unused B, index or mask view must be
empty. Destination and scratch may have spare capacity, which stays unchanged
on success. Axes are positive; an empty reduction is refused instead of being
assigned an implicit identity. These operations handle rows/matrices; they do
not establish arbitrary-axis reductions, broadcasting, batched matmul or
strided layouts.

Each logical input/output fits at most 1,048,576 elements. Matrix multiply also
admits at most 1,048,576 scalar multiply-add pairs. The plan checks that budget
by division before multiplication. Other kernels make a fixed number of
passes over admitted spans. Larger model profiles must qualify larger bounds.

## Add values, then add rows

Elementwise addition pairs values at the same flat position. A row sum has
one output for several input values; a dot product first multiplies matching
values and then adds those products.

```jai
sum([1, 2, 3]) = 6
dot([1, 2, 3], [1, 2, 3]) = 1*1 + 2*2 + 3*3 = 14
```

A wider accumulator helps, but it does not make addition exact. For example,
adding `1` to a binary64 value near `1e20` loses that small term. A later
subtraction of the large value does not recover it. Our check requires the
sum of the stored `f32` sequence `[1e20, 1, -1e20]` to be `1`.

The accumulator retains a rounded main sum and adds its residual to a separate
correction. This uses the TwoSum error transformation and cascaded compensation
studied by Ogita, Rump and Oishi in [Accurate Sum and Dot Product, algorithms
3.1 and 4.1](https://www.tuhh.de/ti3/paper/rump/OgRuOi05.pdf).

<!-- source: psionic/lib/ops.coax accumulate -->
```jai
psionic_ops_accumulate :: (s: PsionicAccumulator, value: f64) -> PsionicAccumulator {
    next := s.sum + value;
    virtual_value := next - s.sum;
    error := (s.sum - (next - virtual_value)) + (value - virtual_value);
    return PsionicAccumulator.{sum = next, correction = s.correction + error};
}
psionic_ops_total :: (s: PsionicAccumulator) -> f64 { return s.sum + s.correction; }
```

For sums and exact widened `f32` products, the error-free residual identity
and a standard bound on summing residuals give this conservative combined
bound after narrowing, under the profile's IEEE assumptions:

```jai
u32 = 2^-24
u64 = 2^-53
S = exact sum of n terms
A = sum of absolute term values
abs(result - S) <= (u32 + 2*u64)*abs(S) + 4*n*n*u64*u64*A + 2^-149
```

Here `n <= 1048576`; binary64 overflow/underflow does not occur in these
sum/product transforms. The last term allows binary32 subnormal rounding.
This bound includes conditioning: near cancellation, `A` can be enormous
compared with `abs(S)`. It does not promise a small relative error for every
possible input, nor prove that a compiler implements the intended arithmetic.
Exact cancellation fixtures and independent native comparisons still matter.

## Multiply a rectangular matrix

A matrix product combines a row from A with a column from B. The inner
sizes must match; the output has A's row count and B's column count:

```jai
A = [[1, 2, 3],       B = [[ 7,  8],
     [4, 5, 6]]           [ 9, 10],
                           [11, 12]]

C[0,0] = 1*7 + 2*9 + 3*11 = 58
C[0,1] = 1*8 + 2*10 + 3*12 = 64
C[1,0] = 4*7 + 5*9 + 6*11 = 139
C[1,1] = 4*8 + 5*10 + 6*12 = 154
```

For an output at `(row, column)`, the flat offsets are `row*width + k` in A
and `k*columns + column` in B. The plan has already bounded these products
and the total work. Products are promoted before multiplication, not after a
possibly overflowing `f32` product.

<!-- source: psionic/lib/ops.coax reduce -->
```jai
psionic_ops_row_sum :: (a: [] f32, base: int, width: int) -> f64 {
    acc: PsionicAccumulator;
    for k: 0..<width { acc = psionic_ops_accumulate(acc, f64(a[base + k])); }
    return psionic_ops_total(acc);
}
psionic_ops_reduce :: (r: PsionicRequest, a: [] f32, b: [] f32, row: int, column: int) -> f64 {
    if r.op == PsionicOp.sum { return psionic_ops_row_sum(a, row * r.width, r.width); }
    acc: PsionicAccumulator;
    for k: 0..<r.width {
        value := f64(a[row * r.width + k]);
        if r.op == PsionicOp.dot { value *= f64(b[row * r.width + k]); }
        else { value *= f64(b[k * r.columns + column]); }
        acc = psionic_ops_accumulate(acc, value);
    }
    return psionic_ops_total(acc);
}
```

Sum, row-dot and matrix multiplication share the compensated accumulator.
Matrix-vector multiplication is the same matrix kernel with one output column.
The straightforward scalar loop remains our CPU reference when later kernels
change tiling or dispatch.

## Gather rows without changing their order

Gather picks complete rows from a table. It is useful for embedding lookup:
a token ID selects one row of learned numbers. This chapter demonstrates the
storage operation before introducing a tokenizer or a learned embedding.

```jai
table = [[1, 2, 3], [4, 5, 6]]
indices = [1, 0, 1]
result = [[4, 5, 6], [1, 2, 3], [4, 5, 6]]
```

Every index must be nonnegative and strictly below `rows`. All indices are
checked before staging starts. The operation preserves selected finite values'
bits, including signed zero and subnormals. Repeated indices are allowed;
invalid ones are refused rather than clamped or wrapped.

## Turn scores into probabilities

Softmax exponentiates scores and divides by their sum. Directly exponentiating
large scores such as `1000` overflows ordinary binary64. Subtracting a row's
largest allowed score first leaves the probabilities mathematically unchanged:

```jai
maximum = largest allowed score
weight[i] = exp(score[i] - maximum) if allowed, otherwise 0
probability[i] = weight[i] / sum(weight)
```

A mask byte is exactly zero or one. Each row needs at least one allowed entry.
The maximum selector keeps the first allowed index on ties; the numerical
probabilities do not depend on which equal maximum is selected. Because one
exponential is exactly `exp(0) = 1`, the denominator cannot be zero. The
subtraction happens in `f64`, so even opposite maximum finite `f32` values
have a finite difference. Exponential underflow to zero is allowed.

<!-- source: psionic/lib/ops.coax softmax -->
```jai
psionic_ops_softmax_row :: (r: PsionicRequest, a: [] f32, mask: [] u8, scratch: [] f32, base: int) {
    maximum := f64(a[base + psionic_ops_argmax(a, mask, base, r.width)]);
    acc: PsionicAccumulator;
    for col: 0..<r.width {
        if mask[base + col] == 1 { acc = psionic_ops_accumulate(acc, exp(f64(a[base + col]) - maximum)); }
    }
    denominator := psionic_ops_total(acc);
    for col: 0..<r.width {
        probability: f64 = 0;
        if mask[base + col] == 1 { probability = exp(f64(a[base + col]) - maximum) / denominator; }
        scratch[base + col] = f32(probability);
    }
}
```

Masked entries are exact positive zero. Allowed probabilities are rounded to
`f32`, so their sum need only be close to one. This version recalculates
exponentials instead of allocating another temporary array. A fused or cached
version must preserve its declared numerical contract.

A zero-width request, invalid mask byte, all-masked row, infinity or NaN is a
failure, including a NaN at a masked position. Validation covers every row
before any staging write. No failure becomes a uniform distribution.

## Normalize a row

RMSNorm scales by the root mean square. LayerNorm first subtracts the row
mean and uses the population variance. Both put epsilon inside the square root:

```jai
RMSNorm(x) = x / sqrt(mean(x*x) + epsilon)
LayerNorm(x) = (x - mean(x)) / sqrt(mean((x - mean(x))^2) + epsilon)
```

For `[3,4]` and epsilon `3.5`, the RMS denominator is
`sqrt((9+16)/2 + 3.5) = 4`, so the result is `[0.75,1]`.
For `[1,3]` and epsilon `3`, the mean is two and the denominator is two,
so LayerNorm returns `[-0.5,0.5]`.

<!-- source: psionic/lib/ops.coax norm -->
```jai
psionic_ops_norm_row :: (r: PsionicRequest, a: [] f32, scratch: [] f32, base: int) {
    mean: f64 = 0;
    if r.op == PsionicOp.layernorm { mean = psionic_ops_row_sum(a, base, r.width) / f64(r.width); }
    squares: PsionicAccumulator;
    for col: 0..<r.width {
        centered := f64(a[base + col]) - mean;
        squares = psionic_ops_accumulate(squares, centered * centered);
    }
    denominator := sqrt(psionic_ops_total(squares) / f64(r.width) + f64(r.epsilon));
    for col: 0..<r.width { scratch[base + col] = f32((f64(a[base + col]) - mean) / denominator); }
}
```

The variance uses a second pass over centered values rather than subtracting
two close wide moments. Epsilon must be finite and strictly positive, including
for a constant or one-element row. Each normalized component is bounded by
about `sqrt(width)` in this profile; the native checks also exercise maximum
finite input magnitudes and the smallest positive epsilon.

These are unscaled normalization cores. Learned gain/bias and any row
broadcasting need explicit layer contracts. Their absence is not a claim
of complete checkpoint-compatible normalization.

## Validate, stage and publish

The native entry point in [platform/ops-memory.h](../platform/ops-memory.h)
observes writable overlaps against both float operands, integer indices and
byte masks. Read-only operands may share storage. Writable spans, including
unused capacity, must be disjoint from every other span. Adjacent spans are
allowed. The shared [spans.h](../platform/spans.h) accounts for each element's
actual byte width; an index view is not a float-sized range.

The checked Coax entry point receives that observation, re-derives the plan
from the request, validates data, and invokes the kernels:

<!-- source: psionic/lib/ops.coax checked -->
```jai
psionic_ops_checked :: (r: PsionicRequest, a: [] f32, b: [] f32, indices: [] int, mask: [] u8,
        destination: [] f32, scratch: [] f32, overlap: bool) -> PsionicResult #export_c {
    p := psionic_ops_admit(r, a.count, b.count, indices.count, mask.count, destination.count, scratch.count, overlap);
    if p.status != PsionicOpStatus.ok { return PsionicResult.{status = p.status}; }
    for value: a { if !psionic_tensor_finite_bits(f32_bits(value)) { return PsionicResult.{status = PsionicOpStatus.number}; } }
    for value: b { if !psionic_tensor_finite_bits(f32_bits(value)) { return PsionicResult.{status = PsionicOpStatus.number}; } }
    for index: indices {
        if index < 0 || index >= r.rows { return PsionicResult.{status = PsionicOpStatus.index}; }
    }
    if r.op == PsionicOp.softmax {
        for row: 0..<r.rows {
            allowed := false;
            for col: 0..<r.width {
                value := mask[row * r.width + col];
                if value > 1 { return PsionicResult.{status = PsionicOpStatus.mask}; }
                allowed = allowed || value == 1;
            }
            if !allowed { return PsionicResult.{status = PsionicOpStatus.mask}; }
        }
    }
    status := psionic_ops_compute(r, a, b, indices, mask, scratch);
    if status != PsionicOpStatus.ok { return PsionicResult.{status = status}; }
    for i: 0..<p.output { destination[i] = scratch[i]; }
    return PsionicResult.{status = PsionicOpStatus.ok, written = p.output};
}
```

The failure order is operation tag → shape/size/work → canonical parameters
→ view counts/capacities → aliases → finite data → indices/masks → arithmetic.
Every failure reports zero `written`; successful calls publish the exact
planned output count. Failed plans have zero counts; the execution wrapper always inspects status
before using them.

Pre-execution refusals leave inputs, destination and scratch unchanged.
A computed result outside the finite binary32 interval returns `arithmetic`:
the destination is unchanged, but scratch may contain an earlier staged prefix.
For example, adding `[1,max_f32]` to `[2,max_f32]` stages `3` and then refuses.
No partial output is installed as a successful result. Source data and unused
tails stay unchanged. These are synchronous call-level guarantees; ordinary
writes do not become concurrent atomic transactions.

Native callers still own truthful allocations, alignment, extents, lifetime
and exclusive writable access. An overlap observation cannot validate arbitrary
pointer ownership. The example supplies `false` only because its local arrays
are independently owned and disjoint.

## The complete program

<!-- source: psionic/examples/numbers-matrices.coax example -->
```jai
main :: () {
    a := [6] f32.[1, 2, 3, 4, 5, 6]; b := [6] f32.[7, 8, 9, 10, 11, 12];
    ids := [3] int.[1, 0, 1]; mask := [2] u8.[1, 1];
    out: [10] f32; scratch: [10] f32;
    // Every writable local array is disjoint from the read-only operands.
    matrix := psionic_ops_checked(PsionicRequest.{op = PsionicOp.matmul, rows = 2, width = 3, columns = 2},
        a[..], b[..], ids[0..<0], mask[0..<0], out[..], scratch[..], false);
    #assert matrix.status == PsionicOpStatus.ok && matrix.written == 4;
    #assert out[0] == 58 && out[1] == 64 && out[2] == 139 && out[3] == 154;
    print("matmul [2,3] x [3,2]: [%, %, %, %]\n", out[0], out[1], out[2], out[3]);

    sum := psionic_ops_checked(PsionicRequest.{op = PsionicOp.sum, rows = 2, width = 3},
        a[..], b[0..<0], ids[0..<0], mask[0..<0], out[..], scratch[..], false);
    #assert sum.status == PsionicOpStatus.ok && out[0] == 6 && out[1] == 15;
    print("row sums: [%, %]\n", out[0], out[1]);
    dot := psionic_ops_checked(PsionicRequest.{op = PsionicOp.dot, rows = 2, width = 3},
        a[..], a[..], ids[0..<0], mask[0..<0], out[..], scratch[..], false);
    #assert dot.status == PsionicOpStatus.ok && out[0] == 14 && out[1] == 77;
    print("row dots with themselves: [%, %]\n", out[0], out[1]);
    add := psionic_ops_checked(PsionicRequest.{rows = 2, width = 3},
        a[..], a[..], ids[0..<0], mask[0..<0], out[..], scratch[..], false);
    #assert add.status == PsionicOpStatus.ok && out[0] == 2 && out[5] == 12;
    print("add: first = %, last = %\n", out[0], out[5]);

    gather := psionic_ops_checked(PsionicRequest.{op = PsionicOp.gather, rows = 2, width = 3, columns = 3},
        a[..], b[0..<0], ids[..], mask[0..<0], out[..], scratch[..], false);
    #assert gather.status == PsionicOpStatus.ok && gather.written == 9;
    #assert out[0] == 4 && out[3] == 1 && out[8] == 6;
    print("gather [1,0,1]: first = %, middle = %, last = %\n", out[0], out[3], out[8]);

    logits := [2] f32.[1000, 1000];
    softmax := psionic_ops_checked(PsionicRequest.{op = PsionicOp.softmax, rows = 1, width = 2},
        logits[..], b[0..<0], ids[0..<0], mask[..], out[..], scratch[..], false);
    #assert softmax.status == PsionicOpStatus.ok && out[0] == 0.5f32 && out[1] == 0.5f32;
    print("softmax [1000,1000]: [%, %]\n", out[0], out[1]);
    values := [2] f32.[3, 4];
    rms := psionic_ops_checked(PsionicRequest.{op = PsionicOp.rmsnorm, rows = 1, width = 2, epsilon = 3.5},
        values[..], b[0..<0], ids[0..<0], mask[0..<0], out[..], scratch[..], false);
    #assert rms.status == PsionicOpStatus.ok && out[0] == 0.75f32 && out[1] == 1;
    print("RMSNorm [3,4], epsilon 3.5: [%, %]\n", out[0], out[1]);
    values[0] = 1; values[1] = 3;
    layer := psionic_ops_checked(PsionicRequest.{op = PsionicOp.layernorm, rows = 1, width = 2, epsilon = 3},
        values[..], b[0..<0], ids[0..<0], mask[0..<0], out[..], scratch[..], false);
    #assert layer.status == PsionicOpStatus.ok && out[0] == -0.5f32 && out[1] == 0.5f32;
    print("LayerNorm [1,3], epsilon 3: [%, %]\n", out[0], out[1]);

    mask[0] = 0; mask[1] = 0;
    for i: 0..<10 { out[i] = -7; scratch[i] = -9; }
    refused := psionic_ops_checked(PsionicRequest.{op = PsionicOp.softmax, rows = 1, width = 2},
        logits[..], b[0..<0], ids[0..<0], mask[..], out[..], scratch[..], false);
    #assert refused.status == PsionicOpStatus.mask && refused.written == 0;
    for i: 0..<10 { #assert out[i] == -7 && scratch[i] == -9; }
    print("refused: all-masked row; destination and scratch unchanged\n");
}
```

Empty borrowed views represent unused operands. Their backing arrays are not
read. Reusing output/scratch across calls is safe because each call completes
before the next one begins. The final call changes the mask to all zeros and
checks both writable arrays for preservation.

## Check against something independent

The authored Coax checks precede implementation. They cover exact rectangular
products, signed dots, large cancellation, repeated gathers, shifts and masks,
normalization statistics, capacity/alias precedence and failure publication.
Both new authority files require zero surviving generated mutants:

```shell
coax/bin/coax test psionic/lib/ops-contract.coax
coax/bin/coax mutate psionic/lib/ops-contract.coax
coax/bin/coax test psionic/lib/ops.coax
coax/bin/coax mutate psionic/lib/ops.coax
```

The native harness also uses [ops_oracle.py](../tests/ops_oracle.py) to build
120 independent cases with 120-digit Decimal arithmetic. Their inputs come
from a named fixed integer generator and exact binary32 encodings. The
reference uses direct high-range exponentials for softmax and a wide-moment
variance formula for LayerNorm, rather than copying the Coax algorithms.
It consumes no output from the implementation under test.

For these bounded dyadic fixtures, add/sum/dot/matmul are checked exactly;
gather is checked by selected input bits. Softmax and normalization use an
absolute allowance of `2^-23` plus `2^-21 * abs(reference)`. Inputs for these
normalizations are at most four in magnitude, widths are at most seven, and
epsilon is the exact `f32` encoding of `1e-5`: binary64 rounding/conditioning
errors are well below that allowance, and it leaves several binary32 rounding
units for narrowing and the target scalar math. This is a fixture-specific
qualification, not a universal libm error theorem or device guarantee.

Additional analytic cases exercise maximum finite values, signed zero,
subnormals and deliberate underflow; wide cancellation must give exactly
zero or one as specified. Real infinities/NaNs are injected at every position
of each float operand. Other cases use actual short float/index/mask
allocations, typed/partial/tail aliases and a complete matrix multiply with
1,048,576 multiply-add pairs. The same generated library runs plain and under
ASan/UBSan in debug and optimized builds. Earlier chapters run as regressions.

The [verification record](../../docs/verification/psionic-ch03-2026-10-28.md)
retains the command, identities, results and remaining limits. The book checker
matches listings to source and every program to its transcript and exit status.
This evidence establishes bounded CPU numerical execution, not autodiff,
checkpoint compatibility, GPU performance or correctness for every possible
input.

## Try a change

Change B in the matrix example to a three-element column `[1,-1,2]`, set
`columns = 1`, and derive the two results before running it. Then try masking
one of the equal softmax scores: the allowed entry should become one and the
other exact zero. Finally, place an out-of-range index at the end of a gather
request and verify that neither writable buffer changes.

Next, [read original weight artifacts](ch04-reading-original-weights.md). The bounded numerical operations
remain shared library code as real-model architecture and tokenizer work begins.
