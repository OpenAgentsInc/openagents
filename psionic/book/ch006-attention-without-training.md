# 6. Attention without training

[Chapter 5](ch005-text-becomes-a-model-request.md) gave us token IDs and
complete request branches. This chapter takes a smaller step toward executing
a model: a causal dense decoder block operating on already-embedded vectors.
The weights are supplied by the caller. There is no training, tokenizer lookup,
logit readout, or real Elephant/Mammoth execution here.

Our block uses learned RMS normalization, grouped-query attention, interleaved
rotary positions, a residual connection, a second RMS normalization and a
SwiGLU feed-forward network. We can process a short sequence or advance it one
position at a time with a key/value cache. The complete runnable example is
[attention.coax](../examples/attention.coax).

## Run it

```shell
coax/bin/coax run psionic/examples/attention.coax
psionic/check.sh
```

```jai
full causal block: [1.5, -1.5, -1.0, 1.0]
cached position 0: [1.5, -1.5]
cached position 1: [-1.0, 1.0]
cached values: [0.5, -0.5], [-0.5, 0.5]
refused: position exceeds cache capacity
```

The example chooses zero query/key projections, identity value/output
projections and zero feed-forward projections. Both visible positions have
equal attention scores. Each input has RMS-normalized value `[0.5, -0.5]`
or its opposite: the first output adds that first value to its residual, and
the second adds their mean, zero. The independent fixtures also use nonzero
queries, keys and feed-forward weights; the example alone is not their test.

## Give every axis a job

A vector has `width` channels. Query heads divide those channels into
`heads` groups of `head_width` channels. Several query heads may share a
key/value head; `heads` must be divisible by `kv_heads`. A cache holds at most
`capacity` positions, and the feed-forward intermediate has `hidden` channels.

<!-- source: psionic/lib/attention.coax types -->
```jai
PaShape :: struct { width: int; heads: int; kv_heads: int; head_width: int; hidden: int; capacity: int; epsilon: f32; }
PaStatus :: enum u8 { ok; shape; capacity; alias; number; arithmetic; }
PaResult :: struct { status: PaStatus; written: int; }
```

This first CPU profile accepts even `head_width` of at least two, width up to
16, hidden width up to 32, and 1–16 positions. The width must be exactly
`heads * head_width`. Positive finite epsilon goes inside each RMS square root.
These are educational CPU bounds, not Elephant or Mammoth dimensions. Shape
admission uses division and remainder before any product, including for hostile
integer inputs. Once the shape is admitted the small products cannot overflow.

<!-- source: psionic/lib/attention.coax plan -->
```jai
pa_valid :: (s: PaShape) -> bool {
    if s.width/2>8 || s.heads<1 || s.kv_heads<1 || s.head_width<2 || s.hidden<1 || s.hidden>32 || s.capacity<1 || s.capacity>16 {return false;}
    if s.head_width%2!=0 {return false;}
    if s.width/s.head_width!=s.heads || s.width%s.head_width!=0 || s.heads%s.kv_heads!=0 {return false;}
    return psionic_tensor_finite_bits(f32_bits(s.epsilon)) && s.epsilon>0;
}
pa_weight_count :: (s: PaShape) -> int {
    d:=s.width;k:=s.kv_heads*s.head_width;f:=s.hidden;
    return 2*d+2*d*d+2*k*d+3*f*d;
}
pa_cache_count :: (s: PaShape) -> int {return 2*s.capacity*s.kv_heads*s.head_width;}
pa_rope_count :: (s: PaShape) -> int {return s.capacity*s.head_width;}
```

The weights form one exact-length row-major span. Matrix rows are output
channels; each row's input channels are contiguous:

| Order | Shape | Purpose |
| --- | --- | --- |
| First norm scale | `[width]` | Scale normalized input. |
| Query | `[width, width]` | One query per query head. |
| Key, then value | `[kv_heads * head_width, width]` each | Shared key/value heads. |
| Attention output | `[width, width]` | Mix concatenated head results. |
| Second norm scale | `[width]` | Scale the next normalized residual. |
| Gate, then up | `[hidden, width]` each | Feed-forward branches. |
| Down | `[width, hidden]` | Project the feed-forward result back. |

These projections have no biases. The profile uses ordinary learned norm
scales; a particular model may require a different convention, and must get
its own profile and reference tests. Chapter 4's reader can supply finite
weights, but this chapter does not yet map real model tensor names or execute
an original checkpoint.

## Queries, keys and values

For each position, RMSNorm divides by `sqrt(mean(x*x) + epsilon)` and
multiplies by the learned scale. The normalized vector is projected to Q, K
and V. A query head reads the key/value head at
`query_head / (heads / kv_heads)`. For a query at position `p`, the score of
an earlier or current position `t` is:

```jai
score[p,t] = dot(query[p], key[t]) / sqrt(head_width)
probabilities[p] = softmax(score[p,0..p])
head[p] = sum(probabilities[p,t] * value[t], t = 0..p)
```

Future positions do not participate. Softmax subtracts the largest **rounded
binary32** score before exponentiation. Products and reductions use the
chapter 3 binary64 compensated accumulator; intermediate vectors are narrowed
to binary32 once at each stated projection or operation. The code refuses
non-finite inputs and intermediates outside finite binary32 range. A compiler
and host must retain the no-FMA, no-reassociation profile described in
[chapter 3](ch003-numbers-and-matrices.md).

## Positions are rotations

For each position and each consecutive coordinate pair, the caller supplies
one `[cos, sin]` pair. All heads use that pair for that position. The
convention is interleaved, so `(x, y)` becomes `(x*cos - y*sin, x*sin + y*cos)`:

<!-- source: psionic/lib/attention.coax rotate -->
```jai
pa_rotate :: (v:[]f32,heads:int,dim:int,rope:[]f32,position:int)->bool {
    for h:0..<heads {
        for pair:0..<dim/2 {
            i:=h*dim+2*pair; r:=position*dim+2*pair;
            x:=f64(v[i]);y:=f64(v[i+1]);c:=f64(rope[r]);s:=f64(rope[r+1]);
            if !pa_narrow(x*c-y*s,v,i) || !pa_narrow(x*s+y*c,v,i+1) {return false;}
        }
    }
    return true;
}
```

A usual choice is angle `position / 10000^(2 * pair / head_width)`, but the
library deliberately receives the table rather than assuming a model's rotary
base, scaling or partial-rotation scheme. Its span has exactly
`capacity * head_width` floats, laid out by position then pair. Coefficients
must be finite and have magnitude at most one; the caller is responsible for
supplying the intended unit rotations. A table of `[1, 0]` pairs, as in the
example, leaves vectors unchanged. Keys are cached **after** rotation.

## From attention to a block

Concatenate head outputs, apply the output matrix, and add the input residual.
Normalize again; the feed-forward network then computes

```jai
gate = gate_matrix * normalized_residual
up = up_matrix * normalized_residual
activation = gate * sigmoid(gate) * up
output = residual + down_matrix * activation
```

The implementation evaluates sigmoid in a stable form for either sign. It
stages the new K, V and output in local storage; neither public output nor
cache is published if any operation fails. The cache is position-major:
`[key for all KV heads, value for all KV heads]` at each position. Existing
cache entries are borrowed inputs; the selected new position is updated only
after successful computation.

<!-- source: psionic/lib/attention.coax step -->
```jai
pa_step :: (s:PaShape,w:[]f32,rope:[]f32,x:[]f32,position:int,cache:[]f32,out:[]f32,overlap:bool)->PaResult #export_c {
    if !pa_valid(s) {return PaResult.{status=PaStatus.shape};}
    d:=s.width;k:=s.kv_heads*s.head_width;f:=s.hidden;
    if w.count!=pa_weight_count(s) || rope.count!=pa_rope_count(s) || x.count!=d || position<0 || position>=s.capacity || cache.count<pa_cache_count(s) || out.count<d {return PaResult.{status=PaStatus.capacity};}
    if overlap {return PaResult.{status=PaStatus.alias};}
    if pa_data_status(w)!=PaStatus.ok {return PaResult.{status=PaStatus.number};}
    if pa_data_status(rope)!=PaStatus.ok {return PaResult.{status=PaStatus.number};}
    if pa_data_status(x)!=PaStatus.ok {return PaResult.{status=PaStatus.number};}
    if pa_data_status(cache[0..<pa_cache_prefix(position,k)])!=PaStatus.ok {return PaResult.{status=PaStatus.number};}
    for r:rope {if abs(r)>1 {return PaResult.{status=PaStatus.number};}}
    qoff:=d;koff:=qoff+d*d;voff:=koff+k*d;ooff:=voff+k*d;n2off:=ooff+d*d;goff:=n2off+d;uoff:=goff+f*d;doff:=uoff+f*d;
    norm:[16]f32;q:[16]f32;key:[16]f32;value:[16]f32;context:[16]f32;projected:[16]f32;residual:[16]f32;gate:[32]f32;up:[32]f32;product:[32]f32;down:[16]f32;result:[16]f32;
    if !pa_norm(x,w,0,s.epsilon,norm[0..<d]) || !pa_linear(w,qoff,d,d,norm[..],q[..]) || !pa_linear(w,koff,k,d,norm[..],key[..]) || !pa_linear(w,voff,k,d,norm[..],value[..]) {return PaResult.{status=PaStatus.arithmetic};}
    if !pa_rotate(q[0..<d],s.heads,s.head_width,rope,position) || !pa_rotate(key[0..<k],s.kv_heads,s.head_width,rope,position) {return PaResult.{status=PaStatus.arithmetic};}
    for head:0..<s.heads {
        kv:=head/(s.heads/s.kv_heads); scores:[16]f32;mask:[16]u8;visible:=pa_visible(position);
        for t:0..<visible {
            sum:PsionicAccumulator;
            for c:0..<s.head_width {
                kval:=key[kv*s.head_width+c];if t<position {kval=cache[t*2*k+kv*s.head_width+c];}
                sum=psionic_ops_accumulate(sum,f64(q[head*s.head_width+c])*f64(kval));
            }
            score:=psionic_ops_total(sum)/sqrt(f64(s.head_width));
            if !pa_narrow(score,scores[..],t) {return PaResult.{status=PaStatus.arithmetic};}
            mask[t]=1;
        }
        maximum:=f64(scores[psionic_ops_argmax(scores[..],mask[..],0,visible)]);
        denom:PsionicAccumulator;
        for t:0..<visible {denom=psionic_ops_accumulate(denom,exp(f64(scores[t])-maximum));}
        total:=psionic_ops_total(denom);
        for c:0..<s.head_width {
            sum:PsionicAccumulator;
            for t:0..<visible {
                val:=value[kv*s.head_width+c];if t<position {val=cache[t*2*k+k+kv*s.head_width+c];}
                sum=psionic_ops_accumulate(sum,(exp(f64(scores[t])-maximum)/total)*f64(val));
            }
            if !pa_narrow(psionic_ops_total(sum),context[..],head*s.head_width+c) {return PaResult.{status=PaStatus.arithmetic};}
        }
    }
    if !pa_linear(w,ooff,d,d,context[..],projected[..]) {return PaResult.{status=PaStatus.arithmetic};}
    for i:0..<d {if !pa_narrow(f64(x[i])+f64(projected[i]),residual[..],i) {return PaResult.{status=PaStatus.arithmetic};}}
    if !pa_norm(residual[0..<d],w,n2off,s.epsilon,norm[0..<d]) || !pa_linear(w,goff,f,d,norm[..],gate[..]) || !pa_linear(w,uoff,f,d,norm[..],up[..]) {return PaResult.{status=PaStatus.arithmetic};}
    for i:0..<f {
        g:=f64(gate[i]);e:=exp(-abs(g));sigmoid:=exp((g-abs(g))/2)/(1+e);
        if !pa_narrow(g*sigmoid*f64(up[i]),product[..],i) {return PaResult.{status=PaStatus.arithmetic};}
    }
    if !pa_linear(w,doff,d,f,product[..],down[..]) {return PaResult.{status=PaStatus.arithmetic};}
    for i:0..<d {if !pa_narrow(f64(residual[i])+f64(down[i]),result[..],i) {return PaResult.{status=PaStatus.arithmetic};}}
    for i:0..<k {cache[position*2*k+i]=key[i];cache[position*2*k+k+i]=value[i];}
    for i:0..<d {out[i]=result[i];}
    return PaResult.{written=d};
}
```

`pa_step` takes one position and uses the earlier cache prefix. A caller must
populate positions in order and retain the same shape, weights and rotary table
for that cache. It must not fabricate a prefix, silently replace weights, or
reuse the cache across a different sequence. The function validates sizes,
finiteness and overlap observations, but cannot prove that history.

## Full and incremental execution

For the sequence interface, the library starts a private empty cache, runs
positions in order, and publishes its staged output only after all positions
succeed. This uses the same block step as incremental execution:

<!-- source: psionic/lib/attention.coax full -->
```jai
pa_forward :: (s:PaShape,w:[]f32,rope:[]f32,x:[]f32,steps:int,out:[]f32,overlap:bool)->PaResult #export_c {
    if !pa_valid(s) {return PaResult.{status=PaStatus.shape};}
    if steps<1 {return PaResult.{status=PaStatus.capacity};}
    if steps>s.capacity {return PaResult.{status=PaStatus.capacity};}
    if x.count!=steps*s.width {return PaResult.{status=PaStatus.capacity};}
    if out.count<steps*s.width {return PaResult.{status=PaStatus.capacity};}
    if overlap {return PaResult.{status=PaStatus.alias};}
    cache:[512]f32;staged:[256]f32;
    for p:0..<steps {
        r:=pa_step(s,w,rope,x[p*s.width..<(p+1)*s.width],p,cache[0..<pa_cache_count(s)],staged[p*s.width..<steps*s.width],false);
        if r.status!=PaStatus.ok {return PaResult.{status=r.status};}
    }
    for i:0..<steps*s.width {out[i]=staged[i];}
    return PaResult.{written=steps*s.width};
}
```

Therefore equality of these two APIs is a useful cache and publication check,
not an independent proof of the mathematics. The owned Python fixture tool
computes a separate full-sequence reference, materializing Q/K/V for all
positions first. Its three fixed cases cover ordinary multi-head attention,
shared KV heads, nontrivial rotation, varied weights and nonzero feed-forward
paths. Coax checks both results and cache entries; native debug/release and
ASan/UBSan runs compare the same independent data, check causal isolation and
exercise actual short, aliased and non-finite storage.

The example implements the hand calculation above:

<!-- source: psionic/examples/attention.coax example -->
```jai
main :: () {
    shape:=PaShape.{width=2,heads=1,kv_heads=1,head_width=2,hidden=1,capacity=3,epsilon=3};
    // Q/K and feed-forward projections are zero; V and output are identity.
    weights:=[26]f32.[1,1, 0,0,0,0, 0,0,0,0, 1,0,0,1, 1,0,0,1, 1,1, 0,0, 0,0, 0,0];
    rotary:=[6]f32.[1,0,1,0,1,0];tokens:=[4]f32.[1,-1,-1,1];out:[4]f32;
    full:=pa_forward(shape,weights[..],rotary[..],tokens[..],2,out[..],false);
    #assert full.status==PaStatus.ok;
    print("full causal block: [%, %, %, %]\n",out[0],out[1],out[2],out[3]);
    cache:[12]f32;step:[2]f32;
    for p:0..<2 {
        r:=pa_step(shape,weights[..],rotary[..],tokens[p*2..<(p+1)*2],p,cache[..],step[..],false);
        #assert r.status==PaStatus.ok && step[0]==out[p*2] && step[1]==out[p*2+1];
        print("cached position %: [%, %]\n",p,step[0],step[1]);
    }
    #assert cache[2]==0.5f32 && cache[3]==-0.5f32 && cache[6]==-0.5f32 && cache[7]==0.5f32;
    print("cached values: [%, %], [%, %]\n",cache[2],cache[3],cache[6],cache[7]);
    r:=pa_step(shape,weights[..],rotary[..],tokens[0..<2],3,cache[..],step[..],false);
    #assert r.status==PaStatus.capacity;
    print("refused: position exceeds cache capacity\n");
}
```

## What the boundary promises

Weights, input and rotary views have exact lengths. Output and cache views
must have sufficient capacity; their unused tails remain unchanged. Writable
views must be disjoint from one another and from the entire declared read-only
views. Read-only views may share storage. The [native adapter](../platform/attention-memory.c)
observes address ranges; Coax owns numerical and capacity admission. Callers
retain valid pointer, allocation extent, alignment, lifetime and exclusive
access obligations. Bypassing the adapter also requires a truthful overlap
observation. Publication is synchronous, not a concurrent transaction.

This is causal attention in one small dense block. The decision-model path
still needs exact Qwen3.5 normalization/rotary conventions, its hybrid
recurrence, decision-specific block-bidirectional masks, branch state
isolation, slot readout and real-weight reference comparisons. Those are the
next chapters; matching this teaching profile does not qualify a model.
