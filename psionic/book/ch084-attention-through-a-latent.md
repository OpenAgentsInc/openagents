# 84. Attention through a latent

> **Status: partly implemented.**

An ordinary attention cache stores every head's key and value for every
token. Multi-head latent attention (MLA), used by DeepSeek V2 and V3 and by
Mistral Small 4, Large 3 and Large 4, stores one small **latent** vector per
token instead, plus one rotary key that every head shares. Each head's keys
and values are linear projections of that latent. This chapter shows the
same attention in two forms, proves they compute the same thing, runs both,
and accounts the size of each cache exactly.

[Chapter 90](ch090-execute-deepseek-v2-lite.md) already implements one MLA
step in both forms as `ml_attention`, for DeepSeek V2-Lite. This chapter
does not duplicate it. It adds a runner that carries either form through a
sequence of positions and measures the cache it writes, exact cache
accounting for a whole model, a dense binary64 oracle written separately,
and a frozen tolerance for comparing the two forms.

```shell
coax/bin/coax run psionic/examples/latent-attention.coax
coax/bin/coax test psionic/lib/latent-attention.coax
coax/bin/coax mutate psionic/lib/latent-attention.coax --root-only --jobs 8
```

The [library](../lib/latent-attention.coax), its
[check oracle](../tests/latent-attention-oracle.coax), the
[example](../examples/latent-attention.coax) and chapter 90's
[latent attention](../lib/deepseek/mla.coax) are the whole implementation.
Inputs are small owned BF16 weights and synthetic `f32` vectors; no original
checkpoint is read. The last section lists what remains.

## The two forms

Take one head. A token at position `t` contributes a latent `z_t` (width
`latent`) and a rotary key `r_t` (width `rope`, already rotated for position
`t`). The head has two matrices: `W_UK` (`nope x latent`) makes its
non-rotary key and `W_UV` (`value x latent`) makes its value. The query at
position `p` has a non-rotary part `q_n` and a rotary part `q_r`.

The **expanded** form projects each latent once and caches the results:

```text
k_t = W_UK z_t        v_t = W_UV z_t
s_t = (q_n . k_t + q_r . r_t) * scale
out = sum_t softmax(s)_t v_t
```

The **absorbed** form moves the query into latent space instead. Because
`q_n . (W_UK z_t) = (W_UK^T q_n) . z_t`, and because `W_UV` is linear and the
softmax weights sum over `t`:

```text
a   = W_UK^T q_n                 (the query in latent space, once per step)
s_t = (a . z_t + q_r . r_t) * scale
c   = sum_t softmax(s)_t z_t     (a context in latent space)
out = W_UV c
```

The scores are equal term by term, so the softmax weights are equal, and
`W_UV c = sum_t softmax(s)_t W_UV z_t` is the expanded output. The cache in
this form is just `z_t` and `r_t`.

The rotary key is why `r_t` sits beside the latent instead of inside it.
A rotation depends on the position, so a rotated key is not a fixed linear
function of `z_t`, and no single matrix could be absorbed into the query for
it. MLA keeps a separate rotary part, `rope` values wide, computes it once
per token, and shares it across every head. Only the non-rotary part goes
through the latent.

## Work one head by hand

Use one head with `nope = 1`, `rope = 2`, `value = 1` and `latent = 2`:

```text
W_UK = [1  2]        W_UV = [3  -1]
position 0: z = [1 0], r = [1 0]
position 1: z = [0 1], r = [0 0]
query at position 1: q_n = [1], q_r = [1 0], scale 0.5
```

Expanded: the cached keys are `k_0 = 1`, `k_1 = 2` and the values are
`v_0 = 3`, `v_1 = -1`. The scores are

```text
s_0 = (1*1 + [1 0].[1 0]) * 0.5 = (1 + 1) * 0.5 = 1
s_1 = (1*2 + [1 0].[0 0]) * 0.5 = (2 + 0) * 0.5 = 1
```

Equal scores give weights `1/2` and `1/2`, so the output is
`(3 + -1) / 2 = 1`.

Absorbed: `a = W_UK^T [1] = [1 2]`. The scores are
`([1 2].[1 0] + 1) * 0.5 = 1` and `([1 2].[0 1] + 0) * 0.5 = 1`, the same
as before. The context is `c = (z_0 + z_1) / 2 = [0.5 0.5]` and the output
is `W_UV c = 1.5 - 0.5 = 1`. At position 0 there is one key, so both forms
output `v_0 = 3`.

Every number here is exact in `f32`, so both forms give exactly 1. The
rotary key matters: without it the unscaled scores would be 1 and 2, and
with scale 1 the output would be `(3 - e) / (1 + e)`. A check asserts both.

The cache rows differ in size even here. An absorbed row is `[z r]`, four
values. An expanded row is `[k v r]`: one key, one value and the shared
rotary key, also four. With one head and a latent as wide as the key and
value together, MLA saves nothing; the saving comes from many heads sharing
one latent.

## Account the cache exactly

Per token and per layer, the three layouts hold:

```text
latent   (absorbed)   latent + rope
expanded (chapter 90) heads * (nope + value) + rope
ordinary              heads * (nope + rope + value)
```

The expanded row is chapter 90's: it stores the rotary key once, since every
head shares it. An ordinary multi-head cache, which this chapter uses only
for comparison, stores a whole key, rotary part included, for each head.

For DeepSeek V2-Lite (16 heads, `nope` 128, `rope` 64, `value` 128, latent
512, 27 layers), in BF16 (2 bytes) for one token:

```text
latent    512 + 64              =  576 values x 2 x 27 =  31,104 bytes
expanded  16 x (128 + 128) + 64 = 4160 values x 2 x 27 = 224,640 bytes
ordinary  16 x (128 + 64 + 128) = 5120 values x 2 x 27 = 276,480 bytes
```

At 4,096 tokens the latent cache is 127,401,984 bytes and the ordinary one
1,132,462,080 bytes, 8.9 times larger. The formulas are one function:

<!-- source: psionic/lib/latent-attention.coax types -->
```jai
// Frozen before any comparison: two runs agree when every output differs by
// at most LA_TOLERANCE * (1 + |reference|).
LA_TOLERANCE :: 0.00001;
// Cache values per token and layer, and bytes for layers x tokens at one
// element size: latent (the absorbed form's row: latent plus the shared
// rotary key), expanded (chapter 90's expanded row: each head's non-rotary
// key and value plus the shared rotary key) and full (ordinary multi-head
// attention: each head's whole key and value).
LaCache :: struct {status:DsStatus;latent:int;expanded:int;full:int;latent_bytes:int;expanded_bytes:int;full_bytes:int;}
// steps: positions completed; cache: f32 values written to the cache.
LaRun :: struct {status:DsStatus;steps:int;cache:int;}
LaGap :: struct {worst:f64;at:int;}
```

<!-- source: psionic/lib/latent-attention.coax cache -->
```jai
la_cache :: (s:MlShape,layers:int,tokens:int,bytes:int)->LaCache {
    if !ml_shape(s) {return LaCache.{status=DsStatus.shape};}
    if layers<1 || layers>128 || tokens<1 || tokens>s.capacity {return LaCache.{status=DsStatus.capacity};}
    if bytes!=1 && bytes!=2 && bytes!=4 {return LaCache.{status=DsStatus.format};}
    c:=LaCache.{latent=s.latent+s.rope,expanded=s.heads*(s.nope+s.value)+s.rope,full=s.heads*(s.nope+s.rope+s.value)};
    n:=layers*tokens*bytes;
    c.latent_bytes=n*c.latent;c.expanded_bytes=n*c.expanded;c.full_bytes=n*c.full;
    return c;
}
```

It refuses a shape chapter 90 refuses (`shape`), layers outside 1 to 128 or
tokens outside 1 to the shape's capacity (`capacity`), and element sizes
other than 1, 2 and 4 bytes (`format`), with every count zero. Inside those
bounds the largest count, 128 layers of 163,840 tokens of 64 heads of 640
`f32` values, is below 2^63. A check asserts the hand formulas above, the
largest admitted shape and every refusal.

## Run a sequence and measure the cache

Chapter 90's `ml_attention` runs one position. Its compressed mode is the
absorbed form, and its expanded mode is the expanded form:

<!-- source: psionic/lib/deepseek/mla.coax attention -->
```jai
ml_attention :: (s:MlShape,mode:MlMode,w:[]u8,q:[]f32,z:[]f32,key_rope:[]f32,history:[]f32,position:int,scale:f64,out:[]f32,row:[]f32,work:[]f32,scores:[]f64,overlap:bool)->MlResult #export_c {
    p:=ml_plan(s,mode);if p.status!=DsStatus.ok {return MlResult.{status=p.status};}
    if position<0 || position>=s.capacity {return MlResult.{status=DsStatus.state};}
    if w.count!=2*s.heads*(s.nope+s.value)*s.latent {return MlResult.{status=DsStatus.weights};}
    if q.count!=s.heads*(s.nope+s.rope) || z.count!=s.latent || key_rope.count!=s.rope || history.count!=position*p.row || out.count<p.output || row.count<p.row || work.count<p.floats || scores.count<s.heads*(position+1) {return MlResult.{status=DsStatus.capacity};}
    if overlap {return MlResult.{status=DsStatus.alias};}
    if !ml_scale(scale) || !ml_weights(w) {return MlResult.{status=DsStatus.number};}
    for v:q {if !ds_finite(v) {return MlResult.{status=DsStatus.number};}}
    for v:z {if !ds_finite(v) {return MlResult.{status=DsStatus.number};}}
    for v:key_rope {if !ds_finite(v) {return MlResult.{status=DsStatus.number};}}
    for v:history {if !ds_finite(v) {return MlResult.{status=DsStatus.number};}}
    views:=ml_views(work,p);current:=views.current;absorbed:=views.absorbed;context:=views.context;attended:=views.attended;
    status:=ml_current(s,mode,w,z,key_rope,current);if status!=DsStatus.ok {return MlResult.{status=status};}
    if mode==MlMode.compressed {status=ml_absorb(s,w,q,absorbed);if status!=DsStatus.ok {return MlResult.{status=status};}}
    for head:0..<s.heads {
        prob:=scores[head*(position+1)..<(head+1)*(position+1)];for t:0..<position+1 {prob[t]=ml_score(s,mode,q,absorbed,history,current,position,head,t)*scale;}den:=ds_exp_scores(prob);
        dim:=ifx mode==MlMode.compressed then s.latent else s.value;
        for j:0..<dim {acc:PsionicAccumulator;for t:0..<position+1 {at:=ifx mode==MlMode.compressed then j else head*(s.nope+s.value)+s.nope+j;v:=ml_past(history,current,p.row,position,t,at);acc=psionic_ops_accumulate(acc,prob[t]*f64(v));}
            value:=psionic_ops_total(acc)/den;if !psionic_ops_fits_f32(value) {return MlResult.{status=DsStatus.arithmetic};}
            if mode==MlMode.compressed {context[head*s.latent+j]=f32(value);}else {attended[head*s.value+j]=f32(value);}
        }
        if mode==MlMode.compressed {for j:0..<s.value {acc:PsionicAccumulator;for k:0..<s.latent {at:=(head*(s.nope+s.value)+s.nope+j)*s.latent+k;acc=psionic_ops_accumulate(acc,f64(ds_bf(w,at))*f64(context[head*s.latent+k]));}
            value:=psionic_ops_total(acc);if !psionic_ops_fits_f32(value) {return MlResult.{status=DsStatus.arithmetic};}attended[head*s.value+j]=f32(value);}}
    }
    for i:0..<p.output {out[i]=attended[i];}for i:0..<p.row {row[i]=current[i];}return MlResult.{written=p.output,row_written=p.row};
}
```

`la_run` carries either form through positions `0` to `steps - 1`. Each step
reads the rows already written as its history, writes its own row after
them, and writes its output:

<!-- source: psionic/lib/latent-attention.coax run -->
```jai
la_run :: (s:MlShape,mode:MlMode,w:[]u8,q:[]f32,z:[]f32,pe:[]f32,steps:int,scale:f64,out:[]f32,cache:[]f32,work:[]f32,scores:[]f64)->LaRun {
    p:=ml_plan(s,mode);if p.status!=DsStatus.ok {return LaRun.{status=p.status};}
    if steps<1 || steps>s.capacity {return LaRun.{status=DsStatus.capacity};}
    qn:=s.heads*(s.nope+s.rope);
    if q.count!=steps*qn || z.count!=steps*s.latent || pe.count!=steps*s.rope || out.count<steps*p.output || cache.count<steps*p.row {return LaRun.{status=DsStatus.capacity};}
    for pos:0..<steps {
        r:=ml_attention(s,mode,w,q[pos*qn..<(pos+1)*qn],z[pos*s.latent..<(pos+1)*s.latent],pe[pos*s.rope..<(pos+1)*s.rope],
            cache[0..<pos*p.row],pos,scale,out[pos*p.output..<(pos+1)*p.output],cache[pos*p.row..<(pos+1)*p.row],work,scores,false);
        if r.status!=DsStatus.ok {return LaRun.{status=r.status,steps=pos,cache=pos*p.row};}
    }
    return LaRun.{steps=steps,cache=steps*p.row};
}
```

It checks every count before the first step, so a wrong count changes
nothing. A step that fails on its numbers, such as an infinite latent or an
overflow in a projection, stops the run; earlier positions keep their rows
and outputs, and the result says how many steps completed. The cache it
reports is the number of `f32` values it actually wrote, and the checks
compare four times that with `la_cache` at 4 bytes per element.

## Compare the forms

The two forms round to `f32` at different places: the expanded form rounds
each key and value, the absorbed form rounds the latent-space query, the
latent context and the output. So they agree closely, not bit for bit. The
profile `latent-attention-cpu-f64-v1` fixes the comparison before any run:
two outputs agree when they differ by at most `LA_TOLERANCE x (1 + |reference|)`
with `LA_TOLERANCE` = 0.00001. Where every intermediate is exact in `f32`, as
in the hand case, both forms are compared for exact equality.

The [check oracle](../tests/latent-attention-oracle.coax) is a separate dense
reference. It expands every past latent into keys and values for each query,
in binary64 with plain sums, applies a plain softmax, and never rounds to
`f32`. A generated case with 4 heads, `nope` 8, `rope` 4, `value` 6 and
latent 5 runs 8 positions through both forms. Every output of both forms
meets the oracle within the tolerance, and the forms meet each other.

Two more checks cover the rotary key. Moving the shared rotary key changes
each head's score by exactly `q_r . r` in both forms, with the latent part
untouched; the stored rows hold that key once, at the end. And chapter 90's
YaRN rotation makes the rotary score depend only on the distance between
query and key: positions 7 and 3 score the same as 1,007 and 1,003, within
f32 rounding of the rotation coefficients, and differently from 7 and 4.

## Run it

<!-- source: psionic/examples/latent-attention.coax example -->
```jai
main :: () {
    print("profile latent-attention-cpu-f64-v1, tolerance % x (1 + |reference|)\n",LA_TOLERANCE);
    // The hand case: one head, latent 2, W_UK = [1 2], W_UV = [3 -1].
    s:=MlShape.{heads=1,nope=1,rope=2,value=1,latent=2,capacity=2};
    w:=[8]u8.[0x80,0x3f,0x00,0x40,0x40,0x40,0x80,0xbf];
    q:=[6]f32.[1,1,0,1,1,0];z:=[4]f32.[1,0,0,1];pe:=[4]f32.[1,0,0,0];
    names:=[2]string.["absorbed","expanded"];
    for mode:0..<2 {
        out:[2]f32;cache:[8]f32;work:[16]f32;scores:[2]f64;form:=cast(MlMode)mode;
        r:=la_run(s,form,w[..],q[..],z[..],pe[..],2,0.5,out[..],cache[..],work[..],scores[..]);
        #assert r.status==DsStatus.ok;
        print("hand case, %: cache rows [% % % %] [% % % %], outputs % %\n",names[mode],
            cache[0],cache[1],cache[2],cache[3],cache[4],cache[5],cache[6],cache[7],out[0],out[1]);
        if form==MlMode.compressed {v:=ml_views(work[..],ml_plan(s,form));
            print("  query in latent space % %, context % %\n",v.absorbed[0],v.absorbed[1],v.context[0],v.context[1]);}
    }

    lite:=MlShape.{heads=16,nope=128,rope=64,value=128,latent=512,capacity=163840};
    c:=la_cache(lite,27,1,2);
    print("DeepSeek-V2-Lite shape, 27 layers, BF16, one token:\n");
    print("  latent row % values, % bytes\n",c.latent,c.latent_bytes);
    print("  expanded row % values, % bytes\n",c.expanded,c.expanded_bytes);
    print("  ordinary keys and values % values, % bytes\n",c.full,c.full_bytes);
    long:=la_cache(lite,27,4096,2);
    print("  4096 tokens: latent % bytes, ordinary % bytes\n",long.latent_bytes,long.full_bytes);

    // A generated case: 4 heads, non-rotary 8, rotary 4, value 6, latent 5.
    g:=MlShape.{heads=4,nope=8,rope=4,value=6,latent=5,capacity=8};n:=8;
    gw:[2*4*14*5]u8;for i:0..<gw.count/2 {lo_bf16(lo_value(i,23),gw[..],i);}
    gq:[8*4*12]f32;for i:0..<gq.count {gq[i]=lo_value(i+5,19);}
    gz:[8*5]f32;for i:0..<gz.count {gz[i]=lo_value(i+3,13);}
    gp:[8*4]f32;for i:0..<gp.count {gp[i]=lo_value(i+1,11);}
    outs:[2][8*4*6]f32;gc:[8*(4*14+4)]f32;gwork:[256]f32;gs:[64]f64;
    a:=la_cache(g,1,n,4);accounted:=[2]int.[a.latent_bytes,a.expanded_bytes];
    print("generated case, 8 positions, f32 cache:\n");
    for mode:0..<2 {
        r:=la_run(g,cast(MlMode)mode,gw[..],gq[..],gz[..],gp[..],n,0.25,outs[mode][..],gc[..],gwork[..],gs[..]);
        #assert r.status==DsStatus.ok;
        worst:=0.0;ref:[24]f64;
        for p:0..<n {lo_attend(g,gw[..],gq[p*48..<(p+1)*48],gz[0..<(p+1)*5],gp[0..<(p+1)*4],p,0.25,ref[..]);
            d:=lo_gap(outs[mode][p*24..<(p+1)*24],ref[..]);if d>worst {worst=d;}}
        print("  %: measured cache % bytes, accounted %; largest gap to the dense oracle %\n",names[mode],4*r.cache,accounted[mode],worst);
    }
    gap:=la_gap(outs[0][..],outs[1][..]);
    print("  absorbed against expanded: largest gap % at output %, within tolerance %\n",gap.worst,gap.at,gap.worst<=LA_TOLERANCE);

    qr:=[2]f32.[0.75,-0.5];kr:=[2]f32.[0.25,1];
    print("rotary score, positions 7 and 3: %; 1007 and 1003: %; 7 and 4: %\n",
        lo_rotary_score(qr[..],kr[..],7,3),lo_rotary_score(qr[..],kr[..],1007,1003),lo_rotary_score(qr[..],kr[..],7,4));

    out:=[2]f32.[-7,-7];cache:[8]f32;work:[16]f32;scores:[2]f64;
    odd:=s;odd.rope=3;
    r:=la_run(odd,MlMode.compressed,w[..],q[..],z[..],pe[..],2,0.5,out[..],cache[..],work[..],scores[..]);
    print("refused: rotary width 3: %, steps %, output %\n",r.status,r.steps,out[0]);
    r=la_run(s,MlMode.compressed,w[..],q[..],z[0..<3],pe[..],2,0.5,out[..],cache[..],work[..],scores[..]);
    print("refused: 3 latent values for 2 positions of width 2: %, steps %, output %\n",r.status,r.steps,out[0]);
    print("refused: 5 bytes per element: %\n",la_cache(lite,1,1,5).status);
    bits:u32=0x7f000000;bits+=0x00800000;bad:=z;bad[2]=f32_from_bits(bits);
    r=la_run(s,MlMode.compressed,w[..],q[..],bad[..],pe[..],2,0.5,out[..],cache[..],work[..],scores[..]);
    print("refused: an infinite latent at position 1: %, steps %, outputs % %\n",r.status,r.steps,out[0],out[1]);
}
```

```text
profile latent-attention-cpu-f64-v1, tolerance 0.00001 x (1 + |reference|)
hand case, absorbed: cache rows [1.0 0.0 1.0 0.0] [0.0 1.0 0.0 0.0], outputs 3.0 1.0
  query in latent space 1.0 2.0, context 0.5 0.5
hand case, expanded: cache rows [1.0 3.0 1.0 0.0] [2.0 -1.0 0.0 0.0], outputs 3.0 1.0
DeepSeek-V2-Lite shape, 27 layers, BF16, one token:
  latent row 576 values, 31104 bytes
  expanded row 4160 values, 224640 bytes
  ordinary keys and values 5120 values, 276480 bytes
  4096 tokens: latent 127401984 bytes, ordinary 1132462080 bytes
generated case, 8 positions, f32 cache:
  absorbed: measured cache 288 bytes, accounted 288; largest gap to the dense oracle 6.7679306011059066e-09
  expanded: measured cache 1920 bytes, accounted 1920; largest gap to the dense oracle 6.082250388836328e-09
  absorbed against expanded: largest gap 1.263610522686301e-08 at output 48, within tolerance true
rotary score, positions 7 and 3: -0.45793853653614036; 1007 and 1003: -0.45793858097974294; 7 and 4: 0.43285268051225145
refused: rotary width 3: shape, steps 0, output -7.0
refused: 3 latent values for 2 positions of width 2: capacity, steps 0, output -7.0
refused: 5 bytes per element: format
refused: an infinite latent at position 1: number, steps 1, outputs 3.0 -7.0
```

The generated case shows the size difference at small scale: 9 values a row
for the latent cache against 60 for the expanded one. The largest gap between
the two forms, about 1.3e-8, is far inside the frozen tolerance.

## Refusals

The last four lines are deliberate failures. A rotary width of 3 is refused
as `shape` (rotary widths are even). Three latent values for two positions
of width 2 are refused as `capacity` before any step runs, and the output
keeps its `-7`. An element size of 5 bytes is refused as `format`. An
infinite latent at position 1 is refused as `number` by chapter 90's check
inside that step: position 0 has already run, so its output is 3 and
position 1's output is still `-7`. Checks assert each refusal, a run of
three positions past a capacity of two, an `f32` overflow in a value
projection in both forms, and that refused runs leave their outputs alone.

## What remains

- The absorbed and expanded forms have not been compared on selected layers
  of an original checkpoint. Chapter 90 runs DeepSeek V2-Lite's original
  weights in both forms with separate references, but no recorded
  comparison of the two forms on those layers exists yet.
- Mistral Small 4, Large 3 and Large 4 shapes and checkpoints are not read.
  Their latent attention would reuse `ml_attention` once their shapes are
  pinned.
- Only the CPU profile runs. No device kernel computes either form.

## Dependencies

- [Chapter 10](ch010-attention-without-training.md): attention and caches.
- [Chapter 41](ch041-attention-in-tiles.md): a named profile with a tolerance
  frozen before comparison, checked against a dense oracle.
- [Chapter 90](ch090-execute-deepseek-v2-lite.md): `ml_attention` in both
  forms and the YaRN rotation.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md).
- [DeepSeek V2-Lite configuration](../fixtures/deepseek/v2-original/README.md).
