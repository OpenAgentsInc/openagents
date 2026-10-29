# 43. Attention with sinks and a window

Chapter 42 selected experts. Before those experts can become part of a gpt-oss
layer, we need its attention and activation rules. Several look familiar from
chapter 6, but their details change the answer: a learned sink absorbs attention
mass, half the layers forget old keys, rotary coefficients exceed one, and the
expert activation clamps its two inputs differently.

We will implement these rules in Coax, starting with an attention result we can
calculate without a model download.

```shell
coax/build.sh
coax/bin/coax run psionic/examples/attention-sinks.coax
psionic/tests/check_gptoss_attention.sh
```

```jai
window two: 1.5 3.0 5.0 7.0
full causal: 1.5 3.0 4.5 6.0
YaRN at zero: cos > 1 = true, sin = 0.0
SwiGLU linear -1: 0.0; clamped lanes: 1
full cache exhausted: refused without output
```

The [example](../examples/attention-sinks.coax),
[attention unit](../lib/gptoss-attention.coax) and
[math unit](../lib/gptoss-math.coax) implement the named
`gpt-oss-attention-cpu-wide-v1` profile. Inputs are synthetic, projected `f32`
Q/K/V vectors and per-head sink logits. This chapter qualifies bounded CPU
primitives; original checkpoint binding and a complete decoder remain chapter
38's work. Read chapters [3](ch003-numbers-and-matrices.md),
[6](ch006-attention-without-training.md) and
[35](ch042-a-few-experts-then-a-router.md) first.

## Let the sink keep some probability

Suppose a head has one visible value, `3`, its query/key dot product is zero,
and its sink logit is zero. Ordinary attention would give the value all the
weight. Here softmax sees two equal logits, so the value gets one half. The
other half belongs to the sink and contributes no value. The result is `1.5`.

With two visible values, `3` and `6`, and all three logits zero:

```jai
weights = softmax([key_score_0, key_score_1, sink])
        = [1/3, 1/3, 1/3]
output  = (3 + 6) / 3 = 3
```

There is one learned sink logit per **query head**. It enters the denominator
as an extra unscaled logit after the real key scores have been scaled by
`1 / sqrt(64) = 0.125`. It has no key, position or value vector and occupies
no KV-cache slot. A very large sink can send the output toward zero. A very
negative sink approaches ordinary attention.

The pinned [gpt-oss reference source](https://github.com/openai/gpt-oss/blob/5681a6b6552635e7b29cde94a61ca6a7aee90ebd/gpt_oss/torch/model.py)
concatenates the sink before softmax, then removes its weight before the value
sum. Our implementation uses the equivalent separate denominator term.
Subtract the maximum across **both** key scores and the sink to keep every
exponential at most one.

<!-- source: psionic/lib/gptoss-attention.coax softmax -->
```jai
    for h:0..<s.heads {
        kh:=h/(s.heads/s.kv_heads);scores:[128]f64;
        for slot:0..<visible {
            t:=first+slot;
            key:=kr;if t<position {entry:=ga_slot(s.capacity,plan.kv,t,cache);key=entry[0..<plan.kv];}
            dot:PsionicAccumulator;for d:0..<64 {dot=psionic_ops_accumulate(dot,f64(qr[h*64+d])*f64(key[kh*64+d]));}
            scores[slot]=psionic_ops_total(dot)*0.125;
        }
        largest:=ga_max_index(scores[0..<visible],f64(sinks[h]));
        maximum:=ifx largest<0 then f64(sinks[h]) else scores[largest];
        denominator:PsionicAccumulator;denominator=psionic_ops_accumulate(denominator,exp(f64(sinks[h])-maximum));
        for slot:0..<visible {scores[slot]=exp(scores[slot]-maximum);denominator=psionic_ops_accumulate(denominator,scores[slot]);}
        total:=psionic_ops_total(denominator);
        for d:0..<64 {
            sum:PsionicAccumulator;
            for slot:0..<visible {
                t:=first+slot;
                value:=v[kh*64+d];if t<position {entry:=ga_slot(s.capacity,plan.kv,t,cache);value=entry[plan.kv+kh*64+d];}
                sum=psionic_ops_accumulate(sum,scores[slot]*f64(value));
            }
            stage[h*64+d]=f32(psionic_ops_total(sum)/total);
        }
    }
```

The grouped-query mapping is `query_head / (heads / kv_heads)`. With the
original 64 query heads and eight KV heads, query heads 0–7 share KV head zero,
8–15 share KV head one, and so on. Each query head still has its own sink.

## A window includes the current token

For zero-based position `p`, causal attention sees positions `0` through `p`.
A window of `w` keeps:

```jai
first_visible = max(0, p + 1 - w)
visible_keys  = first_visible ... p
```

The pinned source uses a 128-token window on even-numbered layers, starting at
layer zero, and full causal attention on odd-numbered layers. The window
includes the current token. At position 127, all 128 tokens remain; at 128,
position zero expires.

The example uses a window of two so we can see the difference immediately:

| Position | Current value | Window-two values | With sink | Full-causal values | With sink |
| --- | --- | --- | --- | --- | --- |
| 0 | 3 | 3 | 1.5 | 3 | 1.5 |
| 1 | 6 | 3, 6 | 3 | 3, 6 | 3 |
| 2 | 9 | 6, 9 | 5 | 3, 6, 9 | 4.5 |
| 3 | 12 | 9, 12 | 7 | 3, 6, 9, 12 | 6 |

Every visible key and the sink have equal weight here. Dividing only by the
number of visible keys would miss the sink in every row.

A sliding layer stores only `window` physical slots. Position `p` writes slot
`p % capacity`. Each slot contains all KV heads' rotated keys followed by all
unrotated values. Expired slots are neither validated nor read; the current
entry replaces its slot only after the complete output succeeds. Keep the
**absolute** position for rotary math even as the physical slot wraps.

A full layer uses a growing prefix and refuses when its capacity is exhausted.
This chapter limits full capacity to 128 tokens. Sliding layers can process
positions through 131,071 while keeping at most 128 entries. This is a teaching
execution bound, separate from the target model's full-context implementation.
The caller chooses each layer's full/window shape from the admitted model.

<!-- source: psionic/lib/gptoss-attention.coax plan -->
```jai
ga_plan :: (s:GaShape)->GaPlan #export_c {
    if s.heads<1 || s.heads>64 || s.kv_heads<1 || s.kv_heads>8 || s.heads%s.kv_heads!=0 || s.capacity<1 || s.capacity>128 || s.window<0 {return GaPlan.{status=GoStatus.shape};}
    if s.window!=0 && s.capacity!=s.window {return GaPlan.{status=GoStatus.shape};}
    q:=s.heads*64;kv:=s.kv_heads*64;
    return GaPlan.{q=q,kv=kv,cache=2*s.capacity*kv,work=3*q+kv};
}
```

`ga_plan` gives element counts, not byte counts: `q = heads*64`,
`kv = kv_heads*64`, `cache = 2*capacity*kv`, and `work = 3*q+kv`.
At 64/8 heads and window 128, the cache contains 131,072 floats, or 524,288
logical bytes, and work contains 12,800 floats. These are buffer payloads,
not process-memory or allocator measurements.

## Rotate halves, and keep the YaRN concentration

The pinned source splits a 64-wide head into two halves. Coordinate `i`
rotates with `i+32`, rather than its adjacent coordinate. The table stores
32 interleaved cosine/sine pairs for each absolute position:

```jai
left[i]  = first_half[i] * cos[i] - second_half[i] * sin[i]
right[i] = second_half[i] * cos[i] + first_half[i]  * sin[i]
```

YaRN uses base 150,000, scaling factor 32 and original context 4,096. The
frequency transitions from unscaled to divided by 32 through a clamped linear
ramp. The ramp endpoints are continuous expressions; rounding them to integers
would change coefficients. Both sine and cosine are multiplied by
`1 + 0.1 * log(32)`, about `1.34657`. At position zero, every cosine is already
larger than one. Reusing a helper that rejects coefficients above one would
reject this model's valid rotary table.

<!-- source: psionic/lib/gptoss-math.coax yarn -->
```jai
go_yarn :: (start:int,count:int,out:[]f32)->GoResult #export_c {
    if !go_range(start,count) {return GoResult.{status=GoStatus.range};}
    if out.count<64*count {return GoResult.{status=GoStatus.capacity};}
    low:=32*log(4096.0/(32*2*3.141592653589793))/log(150000.0);
    high:=32*log(4096.0/(2*3.141592653589793))/log(150000.0);
    concentration:=1+0.1*log(32.0);
    for p:0..<count {for i:0..<32 {
        ramp:=(f64(i)-low)/(high-low);
        ramp=(ramp+abs(ramp))/2;
        ramp=1-(1-ramp+abs(1-ramp))/2;
        frequency:=exp(-log(150000.0)*f64(2*i)/64);
        angle:=f64(start+p)*frequency*(1-ramp+ramp/32);
        out[p*64+2*i]=f32(cos(angle)*concentration);
        out[p*64+2*i+1]=f32(sin(angle)*concentration);
    }}
    return GoResult.{written=64*count};
}
```

The two absolute-value expressions clamp the small, bounded ramp to `[0, 1]`.
They implement the same ramp as the pinned source. Coefficients are computed
in `f64` and stored in `f32`; the source's Torch float intermediates and BF16
casts are a different numerical profile.

<!-- source: psionic/lib/gptoss-math.coax rotate -->
```jai
go_rotate :: (heads:int,x:[]f32,c:[]f32,out:[]f32,work:[]f32,overlap:bool)->GoResult #export_c {
    if heads<1 || heads>64 {return GoResult.{status=GoStatus.shape};}
    n:=heads*64;
    if x.count!=n || c.count!=64 || out.count<n || work.count<n {return GoResult.{status=GoStatus.capacity};}
    if overlap {return GoResult.{status=GoStatus.alias};}
    if go_data(x)!=GoStatus.ok {return GoResult.{status=GoStatus.number};}
    if go_data(c)!=GoStatus.ok {return GoResult.{status=GoStatus.number};}
    for h:0..<heads {for i:0..<32 {
        a:=f64(x[h*64+i]);b:=f64(x[h*64+32+i]);co:=f64(c[2*i]);si:=f64(c[2*i+1]);
        left:=a*co-b*si;right:=b*co+a*si;
        if !psionic_ops_fits_f32(left) || !psionic_ops_fits_f32(right) {return GoResult.{status=GoStatus.arithmetic};}
        work[h*64+i]=f32(left);work[h*64+32+i]=f32(right);
    }}
    for i:0..<n {out[i]=work[i];}return GoResult.{written=n};
}
```

Rotation stages every head before copying to the destination. A finite input
near the largest `f32` can overflow after multiplication by the concentration;
that returns `arithmetic` with no partial public output.

## Biased projections stay explicit

The source normalizes the residual stream, projects Q/K/V with a bias, rotates
Q and K, computes attention, then applies another biased projection and a
residual addition. `ga_step` owns the projected-Q/K/V attention portion.
`go_project` supplies a reusable `matrix * input + bias` operation for later
composition, using the shared chapter 3 kernels. The bias is applied before
rotating Q/K and after attention for the output projection.

<!-- source: psionic/lib/gptoss-math.coax project -->
```jai
go_project :: (rows:int,columns:int,a:[]f32,x:[]f32,bias:[]f32,out:[]f32,work:[]f32,overlap:bool)->GoResult #export_c {
    request:=PsionicRequest.{op=PsionicOp.matmul,rows=rows,width=columns,columns=1};plan:=psionic_ops_plan(request);
    if plan.status!=PsionicOpStatus.ok {return GoResult.{status=GoStatus.shape};}
    if a.count!=plan.a || x.count!=columns || bias.count!=rows || out.count<rows || work.count<3*rows {return GoResult.{status=GoStatus.capacity};}
    if overlap {return GoResult.{status=GoStatus.alias};}
    w:=work[0..<3*rows];mat:=w[0..<rows];scratch:=w[rows..<w.count-rows];stage:=w[w.count-rows..<w.count];indices:[1]int;mask:[1]u8;
    m:=psionic_ops_checked(request,a,x,indices[0..<0],mask[0..<0],mat,scratch,false);
    if m.status!=PsionicOpStatus.ok {return GoResult.{status=GoStatus.arithmetic,cause=cast(int)m.status};}
    b:=psionic_ops_checked(PsionicRequest.{rows=1,width=rows},mat,bias,indices[0..<0],mask[0..<0],stage,scratch,false);
    if b.status!=PsionicOpStatus.ok {return GoResult.{status=GoStatus.arithmetic,cause=cast(int)b.status};}
    for i:0..<rows {out[i]=stage[i];}return GoResult.{written=rows};
}
```

The matrix is row-major; matrix, input and bias spans have exact lengths.
Output has at least `rows` elements and work at least `3*rows`. The shared
kernel's bounded matrix budget still applies, including its 1,048,576-MAC
limit. This helper does not yet qualify original full-size projections or bind
MXFP4 tensors. A shared-kernel refusal returns `arithmetic` with that kernel's
status in `cause`. Public output publishes after both multiplication and bias
addition succeed; work may change on failure.

## The expert activation has two different clamps

An expert's first projection stores interleaved gate/linear pairs. For each
pair, the source computes:

```jai
gate   = min(raw_gate, 7)              // upper bound only
linear = clamp(raw_linear, -7, 7)
output = gate * sigmoid(1.702 * gate) * (linear + 1)
```

A gate of `-8` stays `-8`. Clamping it to `-7` would change the answer. The
linear input is bounded on both sides, and its plus-one means a linear input
of `-1` produces zero. Those details distinguish this activation from the
Qwen SwiGLU helper.

<!-- source: psionic/lib/gptoss-math.coax swiglu -->
```jai
go_swiglu :: (pairs:[]f32,out:[]f32,overlap:bool)->GoResult #export_c {
    n:=pairs.count/2;
    if !go_pairs(n) || pairs.count%2!=0 {return GoResult.{status=GoStatus.shape};}
    if out.count<n {return GoResult.{status=GoStatus.capacity};}
    if overlap {return GoResult.{status=GoStatus.alias};}
    if go_data(pairs)!=GoStatus.ok {return GoResult.{status=GoStatus.number};}
    clamped:=0;
    for i:0..<n {
        raw:=f64(pairs[2*i]);gate:=raw;if raw>7 {gate=7;clamped+=1;}
        linear:=f64(pairs[2*i+1]);if linear>7 {linear=7;clamped+=1;}if linear< -7 {linear=-7;clamped+=1;}
        z:=1.702*gate;sigmoid:=exp((z-abs(z))/2)/(1+exp(-abs(z)));
        out[i]=f32(gate*sigmoid*(linear+1));
    }
    return GoResult.{written=n,clamped=clamped};
}
```

The sigmoid formula keeps both exponential arguments nonpositive, including
for a huge finite negative gate. All inputs are validated before writing; the
bounded expression then produces finite `f32` outputs. `clamped` counts gate
and linear lanes whose value changed, including two when both lanes of a pair
were outside their bounds. Values exactly at a bound do not count as clamped.
The unit accepts 1–16,384 pairs; it does not dispatch experts or map weights.

## Publish the context and cache together

`ga_step` validates shape, absolute position, exact Q/K/V/sink lengths, writable
capacities, observed overlap and finite inputs before rotation. It validates
only the old cache entries visible to this step. It stages rotated Q/K and the
whole output in caller work. The last operations commit the new cache entry
and the context vector:

<!-- source: psionic/lib/gptoss-attention.coax publish -->
```jai
    base:=(position%s.capacity)*2*plan.kv;
    for i:0..<plan.kv {cache[base+i]=kr[i];cache[base+plan.kv+i]=v[i];}
    for i:0..<plan.q {out[i]=stage[i];}
    return GaResult.{written=plan.q,next=position+1};
```

Refusal leaves public cache and output unchanged. Spare cache/output capacity
is unchanged on success. Work may change on refusal. The native adapter observes
every writable span against all inputs and other writable spans, then passes
that observation to Coax; it performs no attention arithmetic. Raw Coax calls
pass `overlap=false` only when the caller has established disjoint storage.

The caller retains valid aligned allocations, truthful extents, immutable
inputs and exclusive writable access. It must also associate the cache with
the same shape, model and sequential absolute positions. `next` reports the
next position on success; it does not authenticate that history. A fresh call
at position zero ignores old cache bytes. Checkpoint identity and session
ownership belong above this primitive.

## Run the two cache policies

The same library call runs both rows of the example. Setting `window=0`
selects full causal attention; a positive window selects bounded sliding
storage. Input vectors are already projected, and the caller owns the buffers.

<!-- source: psionic/examples/attention-sinks.coax example -->
```jai
main :: () {
    q:[64]f32;k:[64]f32;v:[64]f32;sinks:[1]f32;cache:[512]f32;out:[64]f32;work:[256]f32;
    for mode:0..<2 {
        shape:=GaShape.{heads=1,kv_heads=1,window=ifx mode==0 then 2 else 0,capacity=ifx mode==0 then 2 else 4};
        if mode==0 {print("window two:");}else {print("full causal:");}
        for p:0..<4 {
            v[0]=f32(3*(p+1));r:=ga_step(shape,p,q[..],k[..],v[..],sinks[..],cache[..],out[..],work[..],false);
            #assert r.status==GoStatus.ok && r.next==p+1;
            print(" %",out[0]);
        }
        print("\n");
    }
    rotary:[64]f32;yr:=go_yarn(0,1,rotary[..]);#assert yr.status==GoStatus.ok;
    print("YaRN at zero: cos > 1 = %, sin = %\n",rotary[0]>1,rotary[1]);
    pairs:=[4]f32.[1,-1,8,0];activated:[2]f32;
    ar:=go_swiglu(pairs[..],activated[..],false);#assert ar.status==GoStatus.ok;
    print("SwiGLU linear -1: %; clamped lanes: %\n",activated[0],ar.clamped);
    out[0]=-7;r:=ga_step(GaShape.{heads=1,kv_heads=1,capacity=4},4,q[..],k[..],v[..],sinks[..],cache[..],out[..],work[..],false);
    #assert r.status==GoStatus.range && out[0]==-7;
    print("full cache exhausted: refused without output\n");
}
```

## Independent checks make the details visible

The [owned scalar oracle](../tests/gptoss_oracle.py) reads no Coax output and
imports no upstream runtime. It uses direct powers for rotary frequencies,
Decimal(100) for activation and a dense, full-prefix attention reference that
stores every historical key before applying a visibility mask. Comparing that
reference with the incremental ring checks grouping, nonzero scores, sinks,
rotation, causal visibility and expiry together.

The local gate covers:

- Six YaRN positions, including 4,095/4,096 and 131,071; split-half rotation
  over two heads; sixteen activation pairs and exact clamp diagnostics.
- Four nonzero full-prefix/cached attention sequences, including shared and
  independent KV heads and ring wraparound. Attention tolerance is
  `2e-5 * (1 + abs(expected))`; math tolerances are `2e-6` with the activation
  scaled by `1 + abs(expected)`.
- A hand-derived 130-step sequence crossing the original 128-token window,
  64/8 heads, maximum activation count and the last admitted absolute position.
- Short and negative span counts, every input's NaN/infinities, visible versus
  expired/future cache NaNs, actual aliases and late rotation overflow, with
  unchanged public state on refusal.
- Debug/release Coax checks, zero-survivor mutation, native plain/ASan/UBSan
  builds, and exact debug/release example output.

The [verification record](../../docs/verification/psionic-ch36-2026-10-29.md)
states the measured scope. This wide CPU profile keeps compensated `f64` dot
products, scores, exponentials and weighted reductions, stores rotated vectors
and context in `f32`, and disables reassociation and FP contraction. It does
not claim Torch/BF16 bitwise parity, original checkpoint execution, a complete
attention block or a GPU implementation. Full-size full-context attention,
normalization/residual composition, original projection/expert binding and
complete-model intermediate/logit references remain explicit chapter 45 gates.

The next branch chapter is **37. The harmony format**: turn structured messages
into the exact model token stream and read its channel and turn boundaries.
Attention semantics and conversation framing need their own checks before we
join them in complete gpt-oss generation.
