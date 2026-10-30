# 3a. A standard library of tensor operators

A useful tensor library needs consistent rules across its operators: what
shape comes out, which values are read, what happens on bad input, and which
derivatives are available. Running one model does not answer those questions.

[Chapter 2a](ch002a-a-tensor-is-a-view-of-memory.md) supplied strided views and
storage versions. This chapter gives them a bounded CPU library of **27
operators**, pure shape/dtype inference, explicit value gradients, and a
[machine-readable support manifest](../fixtures/tensor-ops/operators.json).
The [Coax implementation](../lib/tensor-ops-backward.coax) owns the arithmetic
and admission; the [C adapter](../platform/tensor-ops-memory.c) observes ranges.

## Run it

The [example](../examples/tensor-operators.coax) multiplies a matrix by
broadcast row weights, differentiates the sum, and selects each row's top two:

```shell
COAX=/path/to/qualified/coax psionic/check.sh --only tensor-ops
coax/bin/coax run psionic/examples/tensor-operators.coax
```

Its [frozen transcript](../fixtures/tensor-operators.out) is:

```jai
multiply [2, 3] by [3] -> [2, 3]: 10.0, 40.0, 90.0, 40.0, 100.0, 180.0
sum-of-products gradient for row weights: 5.0, 7.0, 9.0
top-2 per row: 3.0, 2.0, 6.0, 5.0; axis indices: 2, 1, 2, 1
operator registry: 27 CPU f32 operators; comparisons have no VJP
conv -> mean squared loss: 16.0; filter gradient: -24.0, -32.0, -48.0, -56.0
```

## Ask for a plan before asking for values

`to_plan` reads only the request, input shapes and index count. It returns the
output shape, value/auxiliary dtypes, logical counts and admitted work. A scalar
output has rank zero and one element; an empty output has zero elements.
Always inspect status before using any returned payload.

<!-- source: psionic/lib/tensor-ops-contract.coax contract -->
```jai
ToOp :: enum u8 {add;subtract;multiply;divide;negate;relu;exponential;logarithm;equal;less;sum;mean;maximum;softmax;take;update;concat;sort;topk;matmul;conv2d;maxpool2d;avgpool2d;nearest2d;bilinear2d;mse;cross_entropy;}
ToStatus :: enum u8 {ok;operation;shape;parameter;limit;capacity;alias;number;index;arithmetic;nondiff;view;}
ToDtype :: enum u8 {none;f32;i64;}
ToRequest :: struct {op:ToOp;axis:int;keep:bool;descending:bool=true;k:int;height:int;width:int;stride:int=1;}
ToPlan :: struct {status:ToStatus;shape:TvShape;na:int;nb:int;n:int;aux:int;inner:int;work:int;value_dtype:ToDtype;aux_dtype:ToDtype;}
ToResult :: struct {status:ToStatus;written:int;aux_written:int;}
```

There are exactly two borrowed input-owner records and two views. A unary
operator uses a live empty second view and a zero-length second backing span.
Axes are nonnegative; only fields named by an operator have semantics. The
other request fields are ignored. The native checks include extreme ignored
fields so they cannot accidentally become array indices.

The support manifest names each operator's arity, shape rule, dtype, backend,
numerical profile and differentiation status. Native checks compare that
registry with the compiled implementation. Values, including comparison masks,
are `f32`; sort/top-k also return `i64` axis indices, and indexed operations
consume `i64` indices. Dtype promotion and device dispatch remain chapter 3b.

| Family | Operators | This chapter's shape/parameter subset |
| --- | --- | --- |
| Elementwise | add, subtract, multiply, divide, negate, ReLU, exponential, natural logarithm, equal, less | Rank 0–4; binary operations broadcast from the right. Comparisons produce `f32` zero/one masks. |
| Reduction | sum, mean, maximum | One explicit axis, optionally retained with extent one. |
| Normalization | softmax | One explicit nonempty axis; subtract its maximum before exponentiating. |
| Indexed selection/update | take, update | One axis and a one-dimensional index list; updates return a new value. |
| Joining | concat | One axis; equal ranks and matching other extents. |
| Ordering | sort, top-k | Stable per-axis values and original axis indices; ascending or descending. |
| Matrix | matmul | Rank-two `[M,K]` by `[K,N]`, including empty inner products. |
| Convolution | conv2d | NCHW input, OIHW filters, valid cross-correlation, stride one, one group; other requested strides refuse. |
| Pooling | maxpool2d, avgpool2d | NCHW, positive valid window, one positive spatial stride; no padding. |
| Interpolation | nearest2d, bilinear2d | NCHW, positive source/target spatial extents; floor mapping or clipped half-pixel coordinates. |
| Loss | MSE, cross entropy | Mean squared error on equal nonempty shapes; mean stable log-softmax loss on rank-two logits with one integer class label per row. |

Existing chapter 3 operators keep their original contiguous interfaces and
profile. This table does not imply arbitrary convolution options, every dtype,
CUDA/Metal execution, or every operation of another framework.

## Broadcasting is a shape rule and a derivative rule

For a binary operation, align axes from the right. Extents must match or one
must be one. Thus `[2,3]` and `[3]` produce `[2,3]`, while `[2,3]` and `[2]`
refuse. One can expand to zero; zero does not match an unrelated positive
extent. No value buffer is expanded merely to implement broadcasting.

Suppose each row is multiplied by the same three weights:

```jai
Y[row, col] = A[row, col] * B[col]
L = sum(Y)
dL/dA[row, col] = B[col]
dL/dB[col] = sum_over_rows(A[row, col])
```

The smaller operand's gradient sums every use of that operand. Returning just
one row would have the right shape and the wrong derivative. Our example's
row weights receive `[1+4, 2+5, 3+6] = [5,7,9]`.

Views retain their logical coordinate space. Negative strides, transposed
storage and repeated reads are admitted by the same view rules. Returned
gradients are dense tensors in each input's **logical** shape. Accumulating
those logical gradients into shared backing storage, including view pullbacks,
is a separate responsibility of the later autodiff engine.

## Stable discrete choices have explicit conventions

A sort compares values, then resolves equal values to the lower original axis
index. Top-k takes the first `k` entries of that order; `k=0` produces an empty
result. The auxiliary indices refer to the selected axis, not flattened storage.

`take` permits repeated indices; its input gradient adds the corresponding
output gradients. `update` is functional: it copies the input value and applies
indexed replacements. The last replacement of a repeated destination wins.
An overwritten replacement receives zero gradient; an untouched input cell
receives its output gradient. The input storage version never changes during
these pure value operations.

Maximum reduction chooses the lowest axis position on a tie. Max pooling
chooses the first row-major position in its window. Their value gradients
follow those choices. ReLU uses derivative zero at zero. Comparisons and all
integer indices/parameters have no VJP in this profile. Sort/top-k's integer
indices have no derivative; their floating value outputs do.

These are frozen subgradient conventions at nonsmooth points. We check them
explicitly rather than pretending central differences select a unique answer
at a tie.

## Numerical work has a named contract

`CPU-f32-wide-views-v1` stores finite `f32` operands, computes products in
`f64`, uses ordered compensated reductions, and narrows once per output after
checking the finite `f32` interval. Compilation disables FMA contraction and
fast-math reassociation. The shared work limit is 1,048,576 profile work units;
view element limits remain those of chapter 2a.

Matrix and convolution work is outputs times inner products. Stable reference
sorting searches candidate ranks, so its bound is output count times the
squared axis width. Indexed updates also bound every index search. These are
small understandable CPU references, with explicit refusals for larger work.

An empty sum has identity zero. Mean, maximum and softmax require a nonempty
reduced axis. Empty matrix inner products yield zero. Mean losses require
nonempty inputs. Division by zero, logarithms outside the positive domain,
exponential overflow and results outside finite `f32` refuse. Logits are shifted
before softmax/cross-entropy exponentiation; cross entropy uses log-sum-exp
rather than taking the logarithm of a rounded output probability.

All finite checks cover logical operand values, including values later
unselected or overwritten. Unused backing-storage cells outside a view are not
operands. Interpolation uses the declared half-pixel/floor mapping; its spatial
parameters receive no gradients.

## Stage the whole result before publication

The C adapter compares every writable span, including its declared tail, with
all read spans and other writable spans. This includes owner/view metadata,
integer inputs and the auxiliary integer outputs. Read-only input spans may
alias each other. Coax owns admission and all movement/arithmetic.

The result buffers must be fresh and unpublished. To replace an existing
registered tensor, compute into such a result and pass it through chapter 2a's
scatter boundary so its authoritative version advances. Pointing an output at
some other registered allocation would bypass that allocation's mutation
contract and is outside this interface.

<!-- source: psionic/lib/tensor-ops.coax forward -->
```jai
to_forward_checked :: (r:ToRequest,owners:[]TvStorage,views:[]TvView,a:[]f32,b:[]f32,indices:[]int,out:[]f32,scratch:[]f32,aux:[]int,aux_stage:[]int,overlap:bool)->ToResult #export_c {
    p:=to_admit(r,owners,views,a,b,indices);if p.status!=ToStatus.ok {return ToResult.{status=p.status};}
    if out.count<p.n || scratch.count<p.n || aux.count<p.aux || aux_stage.count<p.aux {return ToResult.{status=ToStatus.capacity};}
    if overlap {return ToResult.{status=ToStatus.alias};}
    for i:0..<p.n {value:=to_eval(r,p,views[0],views[1],a,b,indices,i);if value.status!=ToStatus.ok || !to_fits(value.value) {return ToResult.{status=ToStatus.arithmetic};}scratch[i]=f32(value.value);if p.aux>0 {aux_stage[i]=value.index;}}
    for i:0..<p.n {out[i]=scratch[i];}for i:0..<p.aux {aux[i]=aux_stage[i];}return ToResult.{written=p.n,aux_written=p.aux};
}
```

Metadata, finite-input, index, capacity and overlap refusals precede staging.
A computed arithmetic refusal may dirty scratch but leaves every public value
and auxiliary output unchanged, with zero published counts. Successful calls
preserve inputs, metadata and spare writable capacity. This is synchronous
publication under exclusive access, not a concurrent transaction.

## An explicit VJP before an automatic tape

A vector-Jacobian product takes the upstream gradient `dy` and contracts it
with each operator's analytical derivative:

```jai
dA[i] = sum_j(dY[j]/dA[i] * dy[j])
dB[i] = sum_j(dY[j]/dB[i] * dy[j])
```

[`tensor-ops-backward.coax`](../lib/tensor-ops-backward.coax) implements those
entries for all 25 differentiable value operators. It differentiates the wide
value equations at `f32` inputs before final narrowing; it does not attempt to
differentiate IEEE rounding. Both gradient tensors stage completely before
either publishes. The same finite, work, alias and arithmetic rules apply.

This deliberately direct implementation has its own admission bound on
input/output derivative pairs and operator-specific traversal factors. A
forward call can succeed while its dense reference VJP refuses the work limit.
For example, larger matrix gradients need direct accumulation kernels rather
than this entry-by-entry reference. Forward-domain validation also runs before
a derivative is published.

There is no automatic tape here. A caller building a saved forward graph must
check chapter 2a's saved-version tokens before calling a VJP with the saved
values. General graph construction, alias/view pullbacks, efficient large
backward kernels and module integration remain later chapters.

## Work a convolution and its loss backward

The example also composes valid cross-correlation and mean squared error,
then calls their VJPs in reverse order. With one image and one filter:

```jai
image = [[1, 2, 3],
         [4, 5, 6],
         [7, 8, 9]]
filter = [[1,  0],
          [0, -1]]
y = [[-4, -4],
     [-4, -4]]
L = mean(y * y) = 16
dL/dy = 2 * y / 4 = [[-2, -2], [-2, -2]]
```

Each filter coefficient sees four image cells. For its top-left coefficient,
those cells are `1,2,4,5`, so its gradient is `-2*(1+2+4+5) = -24`.
The other coefficients give `-32,-48,-56`. The input gradient adds every
window's contribution, including cancellation where windows overlap:

```jai
dL/dimage = [[-2, -2, 0],
            [-2,  0, 2],
            [ 0,  2, 2]]
```

Those complete gradients are checked in the runnable Coax example. This is
manual reverse composition of shared operators; an automatic tape will later
record the same dependencies rather than asking the caller to wire them.

## What the checks establish

The [independent oracle](../tests/tensor_ops_oracle.py) constructs dense scalar
expected outputs without calling Coax. Central differences of its wide value
equations check analytical VJPs for differentiable cases. The fixed bounds are
scaled error at most `2e-6` for outputs and `3e-5` for gradient references.
These are numerical checks, not an assumption of bitwise agreement with a
framework or a different reduction tree.

The [native harness](../tests/tensor_ops.c) runs 38 cases over all 27 operators
in three layout modes: contiguous, reversed last axis where applicable, and
transposed rank-two storage where applicable. That gives 114 forward
and 114 VJP calls, including the six expected comparison refusals. It checks
all output shapes/dtypes, auxiliary indices, untouched tails and immutable
inputs. Separate checks exercise ties, zero ReLU, invalid encodings/indices,
actual float/integer/metadata aliases, exact work ceilings and late forward or
gradient overflow without partial public publication. Debug/release and
ASan/UBSan accompany the authored Coax checks and exact runnable transcript.

The [dated record](../../docs/verification/psionic-ch003a-2026-10-29.md) binds
those results to the tested build. Chapter 3b can add dtype/device placement
without treating an unlisted backend or operator as silently available.
