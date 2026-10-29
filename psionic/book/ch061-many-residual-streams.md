# 61. Many residual streams

V4.1 carries four residual streams through every attention and FFN sub-block.
Each sub-block learns how to collapse those streams, how strongly to add its
output back into each stream, and how to mix the residual streams with one
another. The collapse uses coefficients produced by the previous sub-block.
Those four coefficients are live model state alongside the four streams.

This chapter implements `deepseek-hyper-cpu-wide-v1`, a bounded CPU equation
reference. The pinned original
[block](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/model.py),
[coefficient kernel](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/kernel.py)
and [configuration](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/config.json)
are read-only references. Their SHA-256 identities, dimensions and equations
are retained in [hyper-contract.json](../fixtures/deepseek/hyper-contract.json).
No upstream Python or kernel implementation runs in the library or its checks.

## Derive three kinds of coefficient

For width `d`, flatten the four streams to `4*d` values. A learned F32 matrix
with 24 rows projects that vector. Normalize its result using one RMS statistic
over the entire flattened input, with epsilon 1e-20. At the original width
5,120 the projection is 24 × 20,480. Three learned scales and 24 learned biases
then produce the affine inputs for four pre coefficients, four post coefficients
and a 4 × 4 combination matrix.

| Coefficients | Equation and use |
| --- | --- |
| Pre, four values | Sigmoid plus 1e-6; retained for the next sub-block's collapse |
| Post, four values | Twice sigmoid; multiply the current sublayer output before adding it to each destination stream |
| Combination, 4 × 4 | Row softmax plus 1e-6, followed by the fixed Sinkhorn normalization schedule; redistribute the current residual |

The first column normalization divides by each column sum plus 1e-6. Nineteen
more row/column normalization pairs complete the original 20-iteration
schedule, with 1e-6 in every denominator. Keep that finite schedule and epsilon:
row and column sums are approximate. The intentionally slow fixture still has
row sums from 0.9837 to 1.0198 after iteration 20. Iterations 19 and 20 differ
by 0.000922, well above the fixture's 1.2e-7 tolerance.

The [implementation](../lib/deepseek/hyper.coax) stores activations, projection
results, inverse RMS, affine inputs, exponentials and coefficients as F32.
Products and reductions use compensated binary64; conversions mark explicit
F32 boundaries. Sigmoid and softmax handle large finite offsets without
exponential overflow. This defines the CPU profile separately from the
original device's BF16 activations and F32 kernel reductions. Whole-model
original arithmetic qualification belongs to chapter 68.

## Read the previous mix, return the next one

The initial carry is one-hot: `[1, 0, 0, 0]`. The first attention input therefore
reads the first stream, even though its current coefficient projection already
produced a different pre-mix.

[dh_prepare](../lib/deepseek/hyper.coax) computes the current coefficients but
collapses `x` with the supplied `previous` carry. The prepared input goes to
attention or an FFN. `dh_finish` expands that sublayer's output and returns the
current pre coefficients as the carry for the next sub-block.

<!-- source: psionic/lib/deepseek/hyper.coax prepare -->
```jai
dh_prepare :: (width:int,x:[]f32,previous:DhCarry,w:DhWeights,out:[]f32,stage:[]f32,overlap:bool)->DhMixResult #export_c {
    if !dh_width(width) {return DhMixResult.{status=DsStatus.shape};}
    if out.count<width || stage.count<width {return DhMixResult.{status=DsStatus.capacity};}
    if overlap {return DhMixResult.{status=DsStatus.alias};}
    projected:=dh_project(width,x,w.projection);if projected.status!=DsStatus.ok {return DhMixResult.{status=projected.status};}
    next:=dh_split(projected.values,w.scale,w.base);if next.status!=DsStatus.ok {return next;}
    collapsed:=dh_collapse_values(width,x,previous,out,stage);if collapsed.status!=DsStatus.ok {return DhMixResult.{status=collapsed.status};}return next;
}
```

The combination matrix is indexed **source, destination**. For destination
`t`, the updated stream is `post[t] * sublayer + sum_s(comb[s,t] * residual[s])`.
The independent fixture changes by 0.1021 if that matrix is transposed. The
attention sub-block's pre-mix becomes the FFN's collapse mix; the FFN's pre-mix
becomes the following attention's mix. A standalone `dh_collapse` consumes the
last returned carry for the final vocabulary head, without another learned
coefficient projection.

| API | Public result |
| --- | --- |
| `dh_initial` | Initial one-hot `DhCarry` |
| `dh_prepare` | Current `DhMix`, plus input collapsed with the previous carry |
| `dh_finish` | Four updated streams, plus `DhCarry` containing current `mix.pre` |
| `dh_collapse` | Collapsed vector for final readout or an explicit previous-mix read |

The live boundary is the pair `(streams[4,d], previous_pre[4])`: at the original
width it contains 81,936 F32 bytes. Transferring only the streams loses state.
The native test moves both parts to separate storage and destroys the sender
before resuming at each of the seven boundaries in a six-sub-block chain,
including the boundary before final readout. All outputs and the final collapse
match the independent uninterrupted reference. Resetting the carry at an
intermediate boundary is an explicit failing negative control.

This chapter's handoff is in-memory F32 state. BF16 stage transport, complete
model-boundary state, identity, fencing and recovery are chapter 141's gates.

## Run the component

The [example](../examples/deepseek-hyper.coax) uses an owned affine sublayer to
show the two handoffs without requiring model weights. Its second input changes
if the carry is reset. It finishes by collapsing the four updated streams for
readout.

```shell
coax/bin/coax run psionic/examples/deepseek-hyper.coax
coax/bin/coax run psionic/examples/deepseek-hyper.coax --release
psionic/tests/check_deepseek_hyper.sh
psionic/check.sh --only deepseek/hyper --jobs 8
```

<!-- source: psionic/examples/deepseek-hyper.coax example -->
```jai
main :: () {
    streams:=[4]f32.[1,2,3,4];carry:=dh_initial();projection:[96]f32;base:[24]f32;
    for i:0..<24 {projection[4*i+i%4]=f32(i%5-2)/16;base[i]=f32(i%7-3)/8;}
    weights:=DhWeights.{projection=projection[..],scale=[3]f32.[0.5,0.25,0.75],base=base};
    input:[1]f32;value:[1]f32;out:[4]f32;work:[4]f32;
    print("Sinkhorn: 20 iterations on 4 x 4 combination coefficients\n");
    for step:0..<2 {
        prepared:=dh_prepare(1,streams[..],carry,weights,input[..],work[..],false);#assert prepared.status==DsStatus.ok;
        if step==0 {#assert input[0]==1;print("first collapse reads the initial stream: %\n",input[0]);}
        else {
            wrong:[1]f32;scratch:[1]f32;reset:=dh_collapse(1,streams[..],dh_initial(),wrong[..],scratch[..],false);
            #assert reset.status==DsStatus.ok && abs(wrong[0]-input[0])>0.1f32;
            print("second collapse uses the carried pre-mix\n");
        }
        // An owned affine stands in for attention/FFN in this component example.
        value[0]=input[0]+0.25f32;
        finished:=dh_finish(1,streams[..],value[..],prepared.mix,out[..],work[..],false);#assert finished.status==DsStatus.ok;
        streams=out;carry=finished.carry; // Both parts cross a sub-block boundary.
    }
    head:=dh_collapse(1,streams[..],carry,input[..],work[..],false);#assert head.status==DsStatus.ok;
    print("final collapse includes all four updated streams\n");
}
```

## Publication and evidence

Widths 1–8,192 are supported. The caller supplies trustworthy coefficient
identity, matching contiguous stream layout, the initial or returned carry,
unmodified successful mixes, valid allocations/extents, immutable inputs and
exclusive disjoint writable spans. The C adapter observes borrowed-span overlap;
Coax owns validation, arithmetic and publication.

Prepare publishes its collapsed input and successful mix only after both
calculations succeed. Finish stages all four updated streams before publishing
them and returning the next carry. Final collapse also stages its output.
Failures preserve public spans; caller-owned private scratch may change. Keep
the old stream/carry pair when a sub-block refuses. None of these functions
allocates storage or authenticates a checkpoint.

The [independent oracle](../tests/deepseek_hyper_oracle.py) uses scalar wide
reductions and 80-digit Decimal sigmoid/softmax to author nonuniform, saturated
and slowly converging coefficients, six complete temporal transitions and the
final collapse. Native checks additionally cover complete projections at widths
5,120 and 8,192, actual nested aliases, first/final NaN/Inf, untouched tails and
late overflow. The [verification record](../../docs/verification/psionic-ch61-2026-10-29.md)
contains actual commands, source identities and mutation results. These are
component equations and state-transfer checks; original checkpoint binding,
complete V4.1 execution and device performance remain separate gates.
