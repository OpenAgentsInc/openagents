# 35. A few experts, then a router

A mixture of experts gives a token several possible computations. A router
selects a few of them, each selected expert produces a vector, and a weighted
sum combines those vectors. The choice is per token; it is not a choice of
which machine will serve a request.

We will build a small complete mixture that we can calculate by hand, then
use the same router at gpt-oss's 32- and 128-expert sizes. The experts in this
chapter are deliberately small affine functions. They exercise selection,
dispatch, combination and resource admission before we add the checkpoint's
nonlinear expert layers.

```shell
coax/build.sh
coax/bin/coax run psionic/examples/few-experts.coax
psionic/tests/check_moe.sh
```

```jai
selected experts: 1, 2 (weights 0.5, 0.5)
mixture: 3.0, 6.0; resident payload: 72 bytes
top-four order: 1, 3, 2, 4
missing selected expert refused without output
```

This is the next independent gpt-oss branch chapter after
[original MXFP4 weights](ch34-original-mxfp4-weights.md). Its small matrix
operations use the shared [chapter 3 kernels](ch03-numbers-and-matrices.md).
The example requires no downloaded weights or upstream runtime.

## Three experts we can check by hand

Let the input vector be `[1, 2]`. Each expert computes a row-major matrix
times the input, then adds a bias. We have three expert IDs:

| Expert | Function | Output for `[1, 2]` |
| --- | --- | --- |
| 0 | `x + [-100, -100]` | `[-99, -98]` |
| 1 | `2*x + [1, -1]` | `[3, 3]` |
| 2 | `4*x + [-1, 1]` | `[3, 9]` |

Suppose the router logits are `[0, 2, 2]` and we select two experts. IDs 1
and 2 have the largest logits. They have equal weight after normalization,
so their mixture is:

```jai
0.5 * [3, 3] + 0.5 * [3, 9] = [3, 6]
```

Expert zero contributes nothing. Its numerical computation is skipped, even
though its parameters remain resident and count against the memory budget.
Sparse computation alone does not make the unused parameters disappear.

## Select first, normalize second

The pinned [gpt-oss MLP source](https://github.com/openai/gpt-oss/blob/5681a6b6552635e7b29cde94a61ca6a7aee90ebd/gpt_oss/torch/model.py)
selects the sorted top four logits and applies softmax only to those selected
values. Our general teaching router accepts one through four selected experts;
gpt-oss uses `mo_route(logits, 4)` with its own admitted model dimensions.

```jai
selected = largest k logits, in descending order
weight[i] = exp(selected[i] - maximum) / sum(exp(selected - maximum))
```

Subtracting the maximum leaves the distribution unchanged and prevents
overflow. At least one numerator is one, so the denominator is positive.
An extremely small selected weight may underflow to zero; it remains a
selected expert and must still be resident.

Normalizing all expert logits first and then discarding the unselected
weights would give a different mixture. In our three-expert example, the
discarded expert would keep part of the probability mass. The selected-only
weights must instead sum to one, within their stated rounding tolerance.

<!-- source: psionic/lib/moe.coax route -->
```jai
mo_route :: (logits:[]f32,k:int)->MoRoute #export_c {
    if logits.count<1 || logits.count>128 || k<1 || k>4 || k>logits.count {return MoRoute.{status=MoStatus.shape};}
    for value:logits {if !psionic_tensor_finite_bits(f32_bits(value)) {return MoRoute.{status=MoStatus.number};}}
    result:MoRoute;
    for slot:0..<k {
        best:=-1;
        for expert:0..<logits.count {
            used:=false;for prior:0..<slot {if result.ids[prior]==expert {used=true;}}
            if !used {if best<0 || logits[expert]>logits[best] {best=expert;}}
        }
        result.ids[slot]=best;
    }
    numerators:[4]f64;denominator:PsionicAccumulator;
    for slot:0..<k {
        numerators[slot]=exp(f64(logits[result.ids[slot]])-f64(logits[result.ids[0]]));
        denominator=psionic_ops_accumulate(denominator,numerators[slot]);
    }
    total:=psionic_ops_total(denominator);
    for slot:0..<k {result.weights[slot]=f32(numerators[slot]/total);}
    result.count=k;return result;
}
```

The router scans expert IDs in ascending order and retains the earlier ID
on equal logits. Repeating that selection produces descending logits with
lowest-ID tie breaking. This is an explicit rule of our CPU profile. The
upstream source requests sorted `torch.topk`; it does not specify this
cross-backend tie contract, so ties are not an automatic upstream parity
claim.

The named profile is `cpu-topk-affine-v1`: one token, 1–128 finite `f32`
logits, `1 <= k <= min(4, expert_count)`, `f64` exponentials and compensated
denominator accumulation, then `f32` routing weights. Non-finite logits
refuse even when they would be unselected. The original checkpoint's
BF16 arithmetic is a separate model qualification; sharing its selection
formula does not establish numerical equivalence.

## Which experts are actually available?

Expert ID is its identity within this tiny model, not its array position.
The caller can store resident experts in any order. The implementation finds
each selected ID, computes that expert and combines outputs in the router's
sorted order. It rejects duplicate IDs, out-of-range IDs or malformed matrix
and bias views in the entire resident table.

A selected expert must be present. Dropping a missing expert and normalizing
the remaining weights again would execute a different model. A future loader
can make the required expert resident and retry; this chapter returns a typed
`resident` refusal with no output.

Each affine expert of width `d` has `d*d` matrix values and `d` bias values.
The budget counts four bytes for each value of every resident entry:

```jai
resident_bytes = resident_count * (d*d + d) * 4
```

<!-- source: psionic/lib/moe.coax plan -->
```jai
mo_plan :: (width:int,residents:int,budget:int)->MoPlan #export_c {
    if width<1 || width>16 || residents<1 || residents>128 {return MoPlan.{status=MoStatus.shape};}
    bytes:=(width*width+width)*residents*4;
    if budget<bytes {return MoPlan.{status=MoStatus.budget};}
    return MoPlan.{bytes=bytes};
}
```

Width is bounded to 1–16 and resident count to 1–128. In the example, three
width-two experts use `3 * (4 + 2) * 4 = 72` logical payload bytes. At the
largest admitted dimensions, the payload is 139,264 bytes. This is a
conservative logical parameter budget: it does not deduplicate shared views
or measure mapped pages, allocator overhead, input/output buffers or local
temporary storage. It is not an operating-system memory reservation.

## Dispatch and gather through the shared kernels

The forward path validates the route, plan, output capacity and overlap,
then checks resident metadata and resolves all selected IDs. It calls the
shared matrix-vector and addition operations for each selected expert. Each
expert result is narrowed according to chapter 3's arithmetic profile;
the final weighted sum uses compensated `f64` accumulation and one checked
`f32` narrowing per component.

<!-- source: psionic/lib/moe.coax forward -->
```jai
mo_forward :: (logits:[]f32,k:int,x:[]f32,experts:[]MoExpert,budget:int,out:[]f32,overlap:bool)->MoResult #export_c {
    route:=mo_route(logits,k);if route.status!=MoStatus.ok {return MoResult.{status=route.status};}
    d:=x.count;plan:=mo_plan(d,experts.count,budget);if plan.status!=MoStatus.ok {return MoResult.{status=plan.status};}
    if experts.count>logits.count {return MoResult.{status=MoStatus.shape};}
    if out.count<d {return MoResult.{status=MoStatus.capacity};}
    if overlap {return MoResult.{status=MoStatus.alias};}
    for i:0..<experts.count {
        e:=experts[i];if e.id<0 || e.id>=logits.count {return MoResult.{status=MoStatus.resident};}
        if e.matrix.count!=d*d || e.bias.count!=d {return MoResult.{status=MoStatus.shape};}
        for j:0..<i {if experts[j].id==e.id {return MoResult.{status=MoStatus.resident};}}
    }
    positions:[4]int;
    for slot:0..<route.count {
        found:=-1;for i:0..<experts.count {if experts[i].id==route.ids[slot] {found=i;}}
        if found<0 {return MoResult.{status=MoStatus.resident};}positions[slot]=found;
    }
    mat:[16]f32;affine:[16]f32;scratch:[16]f32;stage:[16]f32;
    indices:[1]int;mask:[1]u8;sums:[16]PsionicAccumulator;
    for slot:0..<route.count {
        e:=experts[positions[slot]];
        multiply:=psionic_ops_checked(PsionicRequest.{op=PsionicOp.matmul,rows=d,width=d,columns=1},e.matrix,x,indices[0..<0],mask[0..<0],mat[0..<d],scratch[0..<d],false);
        if multiply.status!=PsionicOpStatus.ok {return MoResult.{status=MoStatus.arithmetic,cause=cast(int)multiply.status};}
        add:=psionic_ops_checked(PsionicRequest.{rows=1,width=d},mat[0..<d],e.bias,indices[0..<0],mask[0..<0],affine[0..<d],scratch[0..<d],false);
        if add.status!=PsionicOpStatus.ok {return MoResult.{status=MoStatus.arithmetic,cause=cast(int)add.status};}
        for j:0..<d {sums[j]=psionic_ops_accumulate(sums[j],f64(route.weights[slot])*f64(affine[j]));}
    }
    for j:0..<d {
        value:=psionic_ops_total(sums[j]);if !psionic_ops_fits_f32(value) {return MoResult.{status=MoStatus.arithmetic,cause=cast(int)PsionicOpStatus.arithmetic};}
        stage[j]=f32(value);
    }
    for j:0..<d {out[j]=stage[j];}
    return MoResult.{written=d,route=route,resident_bytes=plan.bytes};
}
```

All temporary vectors are bounded local arrays. The public output is copied
only after every selected expert and every combination component succeeds.
A missing expert, exhausted payload budget, bad metadata, overlap, selected
non-finite value or arithmetic overflow leaves the whole destination
unchanged. Successful execution preserves unused output capacity. A numerical
failure within a shared kernel is reported as `arithmetic` with that kernel's
cause code. An unrepresentable final mixture also refuses; the implementation
does not clamp it or silently change the weights.

Only selected parameter values are numerically inspected or computed. Native
checks include a NaN in an unselected matrix and a NaN in a selected matrix
to make that distinction observable. Metadata and address observations still
cover every resident entry.

The [native adapter](../platform/moe-memory.c) observes output overlap with
the input, logits, resident descriptors and nested matrix/bias spans. Callers
retain responsibility for valid pointers, truthful extents, alignment,
immutable parameters, lifetimes, exclusive access and consistent model IDs.
The public router returns its small result by value and writes no input.

## Run the small mixture

The [example](../examples/few-experts.coax) stores the residents in a different
order from their IDs and makes a missing selected expert visible:

<!-- source: psionic/examples/few-experts.coax example -->
```jai
main :: () {
    logits:=[3]f32.[0,2,2];x:=[2]f32.[1,2];
    a:=[4]f32.[1,0,0,1];b:=[4]f32.[2,0,0,2];c:=[4]f32.[4,0,0,4];
    ba:=[2]f32.[-100,-100];bb:=[2]f32.[1,-1];bc:=[2]f32.[-1,1];
    experts:=[3]MoExpert.[MoExpert.{id=2,matrix=c[..],bias=bc[..]},MoExpert.{id=0,matrix=a[..],bias=ba[..]},MoExpert.{id=1,matrix=b[..],bias=bb[..]}];
    out:=[3]f32.[-7,-7,-7];r:=mo_forward(logits[..],2,x[..],experts[..],72,out[..],false);
    #assert r.status==MoStatus.ok && out[2]==-7;
    print("selected experts: %, % (weights %, %)\n",r.route.ids[0],r.route.ids[1],r.route.weights[0],r.route.weights[1]);
    print("mixture: %, %; resident payload: % bytes\n",out[0],out[1],r.resident_bytes);
    scores:=[6]f32.[0,4,2,3,1,-1];top:=mo_route(scores[..],4);
    #assert top.status==MoStatus.ok;
    print("top-four order: %, %, %, %\n",top.ids[0],top.ids[1],top.ids[2],top.ids[3]);
    out[0]=-7;r=mo_forward(logits[..],2,x[..],experts[1..<3],72,out[..],false);
    #assert r.status==MoStatus.resident && r.written==0 && out[0]==-7;
    print("missing selected expert refused without output\n");
}
```

The [independent oracle](../tests/moe_oracle.py) sorts IDs separately and
uses Decimal arithmetic at 100-digit precision for softmax. Its fixtures
cover ties, adjacent binary32 logits, extreme finite magnitudes and the
32/128-expert top-four shapes. Native checks compare the weighted affine
outputs, exercise genuine nested aliases and late failures, and execute the
maximum resident teaching profile. The [verification record](../../docs/verification/psionic-ch35-2026-10-29.md)
retains the commands, tolerances and mutation results.

The next gpt-oss chapter adds attention sinks, windowing and its clamped
SwiGLU. A complete original MLP still needs normalized inputs, the learned
router matrix and bias, original expert projections and biases, nonlinear
activation, and the residual connection. Our tiny experts return the mixture
alone. Chapter 34's decoded MXFP4 values will feed those later operators;
this chapter's synthetic affine parameters and routing tests do not qualify
a complete gpt-oss checkpoint.
