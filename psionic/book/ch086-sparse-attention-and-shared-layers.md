# 86. Sparse attention and shared layers

> **Status: partly implemented.**

Mistral Large 4 adds several attention features that no smaller model in the
ladder has. Its 1,060.6 GB of weights do not fit on one machine, so this
chapter builds and checks each feature on its own small fixture, in binary64
on the CPU:

- A **lightning indexer**: a cheap scorer that rates every past key for a
  query and keeps the top `k`. Attention then runs over those keys only.
- **Cross-layer KV sharing**: some later layers read an earlier layer's cache
  instead of keeping their own.
- **NoPE layers**: layers with no rotary positions, mixed with rotary ones.
- **Tanh soft-capping** of logits, with its slope.
- **YaRN** frequencies for a long context.

Large 4's own numbers come from its pinned configuration file. No weights are
read; the last section lists what remains for real layers.

```shell
coax/bin/coax run psionic/examples/sparse-attention.coax
coax/bin/coax test psionic/lib/sparse-attention.coax
coax/bin/coax mutate psionic/lib/sparse-attention.coax --root-only --jobs 8
```

The [library](../lib/sparse-attention.coax), its
[check oracle](../tests/sparse-attention-oracle.coax), the
[Large 4 layer maps](../fixtures/mistral-large4-layers.coax) and the
[example](../examples/sparse-attention.coax) are the whole implementation.
Chapter 82's [FP8 decoder](../lib/fp8.coax) decodes the indexer keys.

## What Large 4 configures

The text section of `config.json` at revision
`395730aefd2f928daaf7b69fbd9fefab0ee69b64` (8,220 bytes, SHA-256
`3081fb94f5001f34a21e08437df710bb0b6423beada9184b566a8f5b1ee46277`) sets:

```text
index_n_heads 32, index_head_dim 128, index_topk 2048
indexer_types      72 entries, "full" or "shared"
kv_source_layers   72 entries: the layer whose KV each layer reads
no_rope_layers     72 entries: 1 rotary, 0 NoPE ("rope, rope, rope, nope")
kv_lora_rank 512, qk_rope_head_dim 64
final_logit_softcapping 30.0
rope_parameters    yarn, theta 10000, factor 2048, original 512,
                   beta_fast 32, beta_slow 1, attention_factor 1.0
max_position_embeddings 1048576
```

The [fixture](../fixtures/mistral-large4-layers.coax) copies the three
per-layer lists and these numbers, with the file's hash. The checks run the
pinned maps through this chapter's code.

## The profile

Profile `sparse-attention-cpu-f64-v1` fixes the arithmetic before any
comparison. Inputs and outputs are binary64. Indexer keys are FP8 E4M3 codes
with one UE8M0 (power-of-two) scale per key, decoded exactly by chapter 82's
`fp8_unit` and `fp8_power`. Every dot product, softmax denominator and
weighted sum uses chapter 4's compensated accumulator. A result agrees with
the oracle when every value differs by at most `SA_TOLERANCE x (1 + |oracle|)`:

<!-- source: psionic/lib/sparse-attention.coax types -->
```jai
SaStatus :: enum u8 {ok;shape;range;capacity;number;later;chain;mismatch;}
// Frozen before any comparison: a result agrees with the oracle when every
// value differs by at most SA_TOLERANCE * (1 + |oracle|).
SA_TOLERANCE :: 0.000000000001;
SA_TOPK_MAX :: 2048;          // Large 4's index_topk
SA_CONTEXT_MAX :: 1048576;    // Large 4's max_position_embeddings
SA_HEADS_MAX :: 64;
SA_WIDTH_MAX :: 256;
SA_PAIRS_MAX :: 128;
SA_ROW_MAX :: 65536;
SA_LAYERS_MAX :: 128;
SaResult :: struct {status:SaStatus;written:int;}
// YaRN: rotary width dim, base theta, scale factor over original positions,
// band edges in turns (fast > slow), and the attention factor that
// multiplies cos and sin.
SaYarn :: struct {dim:int;theta:f64;factor:f64;original:int;fast:f64;slow:f64;attention:f64;}
SaBands :: struct {status:SaStatus;low:int;high:int;positions:int;}
// owners: layers that keep their own KV; at: the first refused layer, or -1.
SaMap :: struct {status:SaStatus;layers:int;owners:int;at:int;}
// Values per token over all layers, and bytes for tokens at one element size.
SaCache :: struct {status:SaStatus;shared:int;unshared:int;shared_bytes:int;unshared_bytes:int;}
```

Every function returns a typed status. A refusal for a bad shape, count or
input happens before anything is written.

## Score the keys

For one query, the indexer has `H` heads. Head `j` has a query vector `q_j`
and a weight `w_j`. Every past key `s` has one indexer key `k_s`, shared by
the heads. The score of key `s` is

```text
I_s = sum over j of  w_j * max(0, q_j . k_s)
```

That is `H` dot products and a ReLU per key, much cheaper than full attention
with its many heads and wide latent rows. In Large 4, `H` is 32 and each
`k_s` holds 128 FP8 values.

Work a small case by hand: two heads of width 2, `q_0 = [1 0]`, `q_1 = [0 1]`,
weights 1 and 0.5, and four keys stored as FP8 E4M3 codes with scale byte
127 (2^0):

```text
key   values      codes       head 0      head 1             score
k_0   [1 1]       38 38       1 -> 1      1 -> 0.5 * 1 = 0.5   1.5
k_1   [2 -1]      40 b8       2 -> 2     -1 -> 0              2
k_2   [-1 4]      b8 48      -1 -> 0      4 -> 0.5 * 4 = 2     2
k_3   [0.5 0.5]   30 30     0.5 -> 0.5  0.5 -> 0.25           0.75
```

The ReLU removes the negative dot products of `k_1` and `k_2`. A scale byte
of 128 on `k_3` doubles its values to `[1 1]` and its score to 1.5.

<!-- source: psionic/lib/sparse-attention.coax index -->
```jai
// Scores n keys for one query: the sum over indexer heads j of
// weight_j * max(0, q_j . k_s). Keys are FP8 E4M3 codes, width per key, with
// one UE8M0 scale per key. A score that is not finite stops the scorer.
sa_index :: (heads:int,width:int,n:int,q:[]f64,weights:[]f64,codes:[]u8,scales:[]u8,out:[]f64)->SaResult {
    if !sa_shape(heads,width) {return SaResult.{status=SaStatus.shape};}
    if n<1 {return SaResult.{status=SaStatus.range};}
    if q.count!=heads*width || weights.count!=heads || codes.count!=n*width || scales.count!=n || out.count<n {return SaResult.{status=SaStatus.capacity};}
    if !sa_all_finite(q) || !sa_all_finite(weights) {return SaResult.{status=SaStatus.number};}
    for c:codes {if fp8_nan(c) {return SaResult.{status=SaStatus.number};}}
    for c:scales {if c==255 {return SaResult.{status=SaStatus.number};}}
    for s:0..<n {
        unit:=fp8_power(scales[s]);total:PsionicAccumulator;
        for j:0..<heads {
            dot:PsionicAccumulator;
            for i:0..<width {dot=psionic_ops_accumulate(dot,q[j*width+i]*(fp8_unit(codes[s*width+i])*unit));}
            d:=psionic_ops_total(dot);
            total=psionic_ops_accumulate(total,weights[j]*((d+abs(d))/2));
        }
        score:=psionic_ops_total(total);
        if !sa_finite(score) {return SaResult.{status=SaStatus.number,written=s};}
        out[s]=score;
    }
    return SaResult.{written=n};
}
```

`max(0, d)` is written `(d + |d|) / 2`. A score that is not finite stops the
scorer; the scores before it stay written, and `written` says how many.

## Keep the top k

The selection rule is total and deterministic: a larger score wins, and an
equal score goes to the earlier position. With the scores above,
`[1.5 2 2 0.75]`:

```text
k = 1   keys 1          (keys 1 and 2 tie at 2; key 1 is earlier)
k = 2   keys 1 2
k = 3   keys 0 1 2
k = 9   keys 0 1 2 3    (k at least the context keeps every key)
```

<!-- source: psionic/lib/sparse-attention.coax select -->
```jai
// The top k of scores[0..n): larger scores first, an equal score to the
// earlier position. ids receives the chosen positions in increasing order;
// written is min(k, n).
sa_select :: (scores:[]f64,n:int,k:int,ids:[]int)->SaResult {
    if k<1 || k>SA_TOPK_MAX {return SaResult.{status=SaStatus.range};}
    if n<1 || scores.count<n || (ids.count<k && ids.count<n) {return SaResult.{status=SaStatus.capacity};}
    for t:0..<n {if !sa_finite(scores[t]) {return SaResult.{status=SaStatus.number};}}
    // ids[0..count) holds the best so far, best first.
    count:=0;
    for t:0..<n {
        j:=count;
        if count<k {count+=1;}else if scores[t]>scores[ids[k-1]] {j=k-1;}else {continue;}
        while j>0 && scores[t]>scores[ids[j-1]] {ids[j]=ids[j-1];j-=1;}
        ids[j]=t;
    }
    // The last kept key is the threshold; keep everything that beats it, in position order.
    last:=scores[ids[count-1]];at:=ids[count-1];c:=0;
    for t:0..<n {if scores[t]>last || (scores[t]==last && t<=at) {ids[c]=t;c+=1;}}
    return SaResult.{written=count};
}
```

The first loop keeps the best `k` so far in rank order. A later key only
displaces a kept one with a strictly larger score, so ties keep the earlier
key. The second loop rewrites the kept keys in position order: everything
that beats the last kept key, or equals it at or before its position. Each
key costs at most `k` comparisons, so a full pass is `O(n k)`. `k` must be
from 1 to 2,048, Large 4's `index_topk`.

## Attend over the kept keys

Attention then runs over the kept keys only. This chapter uses multi-query
attention: every head reads the same keys and values, as the absorbed form of
[chapter 84](ch084-attention-through-a-latent.md)'s latent attention does.

<!-- source: psionic/lib/sparse-attention.coax attend -->
```jai
// Multi-query attention over the selected keys only: every head reads the
// same keys and values. ids must increase and lie in 0..n-1.
sa_attend :: (heads:int,dk:int,dv:int,n:int,q:[]f64,keys:[]f64,values:[]f64,ids:[]int,scale:f64,cap:f64,out:[]f64,work:[]f64)->SaResult {
    status:=sa_check(heads,dk,dv,n,q,keys,values,scale,cap,out);if status!=SaStatus.ok {return SaResult.{status=status};}
    m:=ids.count;if m<1 {return SaResult.{status=SaStatus.range};}
    for i:0..<m {if ids[i]<0 || ids[i]>=n || (i>0 && ids[i]<=ids[i-1]) {return SaResult.{status=SaStatus.range};}}
    if work.count<m {return SaResult.{status=SaStatus.capacity};}
    for h:0..<heads {
        for i:0..<m {work[i]=sa_logit(sa_dot(q,h*dk,keys,ids[i]*dk,dk),scale,cap);}
        top:=work[sa_argmax(work[0..<m])];sum:PsionicAccumulator;
        for i:0..<m {work[i]=exp(work[i]-top);sum=psionic_ops_accumulate(sum,work[i]);}
        den:=psionic_ops_total(sum);
        for j:0..<dv {acc:PsionicAccumulator;for i:0..<m {acc=psionic_ops_accumulate(acc,work[i]*values[ids[i]*dv+j]);}out[h*dv+j]=psionic_ops_total(acc)/den;}
    }
    return SaResult.{written=heads*dv};
}
```

The claim to check is that this equals dense attention with every other key
masked out. `sa_dense` computes that dense form independently: a logit for
every key, minus infinity for each masked key, and the same softmax over all
`n` keys:

<!-- source: psionic/lib/sparse-attention.coax dense -->
```jai
// The same attention over every key, with each unmasked key's logit set to
// minus infinity: its weight exp(-inf) is exactly 0.
sa_dense :: (heads:int,dk:int,dv:int,n:int,q:[]f64,keys:[]f64,values:[]f64,mask:[]u8,scale:f64,cap:f64,out:[]f64,work:[]f64)->SaResult {
    status:=sa_check(heads,dk,dv,n,q,keys,values,scale,cap,out);if status!=SaStatus.ok {return SaResult.{status=status};}
    if mask.count!=n || work.count<n {return SaResult.{status=SaStatus.capacity};}
    any:=false;for t:0..<n {if mask[t]!=0 {any=true;}}
    if !any {return SaResult.{status=SaStatus.range};}
    hole:=-1.0e300;hole*=1.0e300;   // minus infinity
    for h:0..<heads {
        for t:0..<n {work[t]=hole;if mask[t]!=0 {work[t]=sa_logit(sa_dot(q,h*dk,keys,t*dk,dk),scale,cap);}}
        top:=work[sa_argmax(work[0..<n])];sum:PsionicAccumulator;
        for t:0..<n {work[t]=exp(work[t]-top);sum=psionic_ops_accumulate(sum,work[t]);}
        den:=psionic_ops_total(sum);
        for j:0..<dv {acc:PsionicAccumulator;for t:0..<n {acc=psionic_ops_accumulate(acc,work[t]*values[t*dv+j]);}out[h*dv+j]=psionic_ops_total(acc)/den;}
    }
    return SaResult.{written=heads*dv};
}
```

The two are equal bit for bit, not only within a tolerance. A masked key's
weight is `exp(-inf - top) = 0` exactly. Adding an exact zero to a
compensated sum leaves both its sum and its correction unchanged, and
`0 * v` is a zero for finite `v`. The kept keys are visited in the same
increasing order in both, and the first largest logit is the same, so every
rounding happens identically. A check also feeds a kept logit of `-1e300`:
the masked keys still get weight 0, because their logit is minus infinity,
not a large negative number.

In the hand case, take keys 1 and 2 with a zero attention key, so both
logits are 0, and values `[10 4 2 2]`. The two kept keys get weight 1/2
each, and the output is `(4 + 2) / 2 = 3`, from `sa_attend` and from
`sa_dense` with mask `[0 1 1 0]`. Every key gives `(10 + 4 + 2 + 2) / 4 = 4.5`.
So sparse attention equals full dense attention only when `k` covers the
context.

## Check against an oracle

The [check oracle](../tests/sparse-attention-oracle.coax) is written
separately. It decodes FP8 codes from chapter 82's generated table of exact
values instead of `fp8_unit`, chooses the top `k` by counting, for each key,
how many keys beat it, computes attention with a plain two-pass softmax and
plain sums, and computes tanh as a ratio of exponentials.

A generated case has 3 indexer heads of width 4, 12 keys with scale bytes
126 to 128 and two keys that tie, and two attention heads of key width 6 and
value width 5. For every `k` from 1 to 14, and with and without a soft cap:

- the indexer's scores equal the oracle's exactly (the values are dyadic, so
  no sum rounds);
- the selection equals the brute-force top `k`, ties included;
- sparse attention equals masked dense attention bit for bit;
- both are within the tolerance of the oracle;
- sparse attention equals full dense attention bit for bit exactly when
  `k >= 12`.

## Share KV across layers

Large 4's `kv_source_layers` gives, for each layer, the layer whose KV cache
it reads. Most layers read their own. Layers 39, 43, 47, ..., 71, every
fourth layer from 39, read layer 35's. `sa_map` checks a map:

<!-- source: psionic/lib/sparse-attention.coax map -->
```jai
// Checks a KV sharing map. source[l] is the layer whose KV layer l reads (l
// itself when it keeps its own). A source must not be later, must keep its
// own KV, and must match the reader's row width and position kind (rotary 1,
// NoPE 0).
sa_map :: (source:[]int,rotary:[]u8,width:[]int)->SaMap {
    n:=source.count;
    if n<1 || n>SA_LAYERS_MAX || rotary.count!=n || width.count!=n {return SaMap.{status=SaStatus.capacity,at=-1};}
    owners:=0;
    for l:0..<n {
        s:=source[l];bad:=SaStatus.ok;
        if width[l]<1 || width[l]>SA_ROW_MAX || rotary[l]>1 {bad=SaStatus.shape;}
        else if s<0 {bad=SaStatus.range;}
        else if s>l {bad=SaStatus.later;}
        else if source[s]!=s {bad=SaStatus.chain;}
        else if width[s]!=width[l] || rotary[s]!=rotary[l] {bad=SaStatus.mismatch;}
        if bad!=SaStatus.ok {return SaMap.{status=bad,layers=n,at=l};}
        if s==l {owners+=1;}
    }
    return SaMap.{layers=n,owners=owners,at=-1};
}
```

A source must not be later than its reader (`later`), must itself keep its
own KV (`chain`), and must match the reader's row width and position kind
(`mismatch`). The last rule matters because a rotary layer stores keys
already rotated for their positions, and a NoPE layer stores them unrotated;
one cannot read the other's keys. Large 4's map passes: layer 35 and all
nine readers are NoPE layers.

With sharing, only owners store KV. Per token, Large 4 stores one latent
row of 512 + 64 = 576 values per owning layer:

```text
owners    72 - 9 = 63 layers   63 x 576 = 36,288 values per token
unshared       72 layers       72 x 576 = 41,472 values per token
```

Sharing saves 5,184 values per token, 12.5%. At 1,048,576 tokens and one byte
a value that is 38,050,725,888 bytes against 43,486,543,872. These are the
latent rows alone; the FP8 scales and the precision of the rotary part are not
pinned in this chapter.

<!-- source: psionic/lib/sparse-attention.coax cache -->
```jai
// Exact cache values per token with sharing (owners only) and without (every
// layer), and bytes for tokens at 1, 2 or 4 bytes a value.
sa_cache :: (source:[]int,rotary:[]u8,width:[]int,tokens:int,bytes:int)->SaCache {
    m:=sa_map(source,rotary,width);if m.status!=SaStatus.ok {return SaCache.{status=m.status};}
    if tokens<1 || tokens>SA_CONTEXT_MAX || (bytes!=1 && bytes!=2 && bytes!=4) {return SaCache.{status=SaStatus.range};}
    c:SaCache;
    for l:0..<source.count {c.unshared+=width[l];if source[l]==l {c.shared+=width[l];}}
    c.shared_bytes=c.shared*tokens*bytes;c.unshared_bytes=c.unshared*tokens*bytes;
    return c;
}
// Where layer l's KV starts in a cache that stores each owner's tokens x width
// values in layer order. A sharing layer gets its source's offset. Call it
// on a map sa_map accepts; a layer outside the map gives -1.
sa_offset :: (source:[]int,width:[]int,tokens:int,layer:int)->int {
    if layer<0 || layer>=source.count {return -1;}
    at:=0;for o:0..<source[layer] {if source[o]==o {at+=width[o]*tokens;}}
    return at;
}
```

`sa_offset` lays the cache out by owner, in layer order, and gives a sharing
layer its source's offset. A check builds a small model in Large 4's
pattern: eight layers, rope, rope, rope, NoPE twice, rows of width 3 (rotary)
and 2 (NoPE), with layer 7 reading layer 3. Over 4 tokens the owners write
`20 x 4 = 80` values, which is the accounted 320 `f32` bytes against 352
unshared, and layer 7's read returns layer 3's values exactly: the same
memory.

`indexer_types` marks 44 layers "full" and 28 "shared", so 44 x 128 = 5,632
indexer values per token instead of 9,216. The configuration does not say
which full layer a shared indexer reads, so this chapter does not model
indexer sharing beyond that count.

## Mix NoPE and rotary layers

`no_rope_layers` makes every fourth layer, 3, 7, ..., 71, a NoPE layer: 18 of
72. `sa_place` places one query or key row at a position. A rotary layer
rotates the row's rotary part; a NoPE layer copies it unchanged:

<!-- source: psionic/lib/sparse-attention.coax place -->
```jai
// Places a query or key row (nope values, then rope values) at a position.
// A rotary layer rotates the rope values in split halves, pair i being
// (i, i + rope/2), by position * freqs[i], with cos and sin multiplied by
// the attention factor; a NoPE layer copies them unchanged.
sa_place :: (rotary:bool,nope:int,rope:int,x:[]f64,position:int,freqs:[]f64,attention:f64,out:[]f64)->SaResult {
    if nope<0 || rope<2 || rope%2!=0 || nope+rope>SA_WIDTH_MAX {return SaResult.{status=SaStatus.shape};}
    if position<0 || position>=SA_CONTEXT_MAX {return SaResult.{status=SaStatus.range};}
    if x.count!=nope+rope || out.count<nope+rope || freqs.count<rope/2 {return SaResult.{status=SaStatus.capacity};}
    if !sa_all_finite(x) || !sa_all_finite(freqs[0..<rope/2]) || !(attention>0) || !sa_finite(attention) {return SaResult.{status=SaStatus.number};}
    for i:0..<nope+rope {out[i]=x[i];}
    if rotary {
        h:=rope/2;
        for i:0..<h {
            angle:=f64(position)*freqs[i];c:=cos(angle)*attention;s:=sin(angle)*attention;
            a:=x[nope+i];b:=x[nope+h+i];
            out[nope+i]=a*c-b*s;out[nope+h+i]=a*s+b*c;
        }
    }
    return SaResult.{written=nope+rope};
}
```

Rotation is split-half: pair `i` is values `i` and `i + rope/2`, as in
[chapter 62](ch062-attention-with-sinks-and-a-window.md). At position 0 the
rotation is the identity. The checks compare a rotation at position 3 with
the oracle's complex multiplication, `(x_i + i x_{i+h}) e^{i p f_i}`, and
show the two kinds of layer apart: a rotary layer's score depends only on
the distance between query and key (positions 7 and 3 score the same as
1,007 and 1,003, within rounding, and differently from 7 and 4), while a
NoPE layer's score is the same at positions 7 and 3 as at 900,000 and 2.

## Cap the logits

Tanh soft-capping squeezes a logit `x` into `(-cap, cap)`:

```text
cap(x) = cap * tanh(x / cap)
cap'(x) = 1 - tanh(x / cap)^2
```

For small `x` it is close to `x` with slope near 1. At `x = cap = 30`,
`tanh(1) = 0.7615941559557649`, so the capped value is 22.847824678672946 and
the slope is `1 - 0.7615941559557649^2 = 0.41997434161402614`. At `x = 90`
the value is 29.851642610601914 and the slope 0.009866: the cap passes almost
no gradient there.

<!-- source: psionic/lib/sparse-attention.coax cap -->
```jai
// Soft-capping: cap * tanh(x / cap), and its slope 1 - tanh(x / cap)^2.
// A cap of 0 leaves x alone.
sa_tanh :: (a:f64)->f64 {t:=exp(-2*abs(a));v:=(1-t)/(1+t);if a<0 {return -v;}return v;}
sa_cap :: (x:f64,cap:f64)->f64 {if cap==0 {return x;}return cap*sa_tanh(x/cap);}
sa_slope :: (x:f64,cap:f64)->f64 {if cap==0 {return 1;}t:=sa_tanh(x/cap);return 1-t*t;}
sa_logit :: (dot:f64,scale:f64,cap:f64)->f64 {return sa_cap(dot*scale,cap);}
```

Large 4's configuration names this cap `final_logit_softcapping`: it applies
to the output logits. `sa_attend` and `sa_dense` also accept a cap on
attention logits (0 turns it off), and the generated checks run both. The
checks compare `sa_cap` with the oracle's tanh at 41 points, confirm
`|cap(x)| < 30`, and compare `sa_slope` with a central difference.

## YaRN for a long context

Rotary pair `i` of a width `dim` turns at frequency `theta^(-2i/dim)`. Over
the `original` positions of training, it makes `original * f_i / (2 pi)`
turns. YaRN keeps the fast pairs, those that turn at least `beta_fast` times,
and divides the slow ones, at most `beta_slow` turns, by `factor`, so they
still fit `original * factor` positions. Pairs between blend linearly. The
band edges are pair numbers:

```text
d    = dim / (2 ln theta)
low  = floor(d * ln(original / (2 pi beta_fast)))
high = ceil(d * ln(original / (2 pi beta_slow)))
r_i  = clamp((i - low) / (high - low), 0, 1)
f'_i = f_i * (1 - r_i) + f_i / factor * r_i
```

For Large 4, `dim` 64, `theta` 10000, `original` 512, `factor` 2048,
`beta_fast` 32 and `beta_slow` 1:

```text
d    = 64 / (2 x 9.21034)                  = 3.47436
low  = floor(3.47436 x ln(512 / 201.062))  = floor(3.47436 x 0.93471) = floor(3.2475) = 3
high = ceil(3.47436 x ln(512 / 6.28319))   = ceil(3.47436 x 4.40045)  = ceil(15.2887) = 16
positions = 512 x 2048 = 1,048,576
```

So pairs 0 to 3 keep their frequency, pairs 16 to 31 are divided by 2,048,
and pairs 4 to 15 blend. Pair 4 has `f_4 = 10000^(-1/8) = 0.316228` and
`r_4 = 1/13`, so `f'_4 = 0.316228 x (12/13 + 1/(13 x 2048)) = 0.291914`.
Pair 16 has `f_16 = 0.01` and becomes `0.01 / 2048 = 4.8828e-6`, a
wavelength of about 1.29 million positions. The attention factor, which
multiplies cos and sin, is pinned at 1.0.

<!-- source: psionic/lib/sparse-attention.coax yarn -->
```jai
// YaRN inverse frequencies for pairs 0..dim/2-1. Pair i turns
// original * theta^(-2i/dim) / (2 pi) times over the original context.
// Pairs at or below low (at least fast turns) keep their frequency; pairs at
// or above high (at most slow turns) are divided by factor; pairs between
// blend linearly. fast >= 2 slow keeps low < high; a band outside 0..dim-1
// is refused.
sa_yarn :: (y:SaYarn,freqs:[]f64)->SaBands {
    if y.dim%2!=0 || y.dim<2 || y.dim/2>SA_PAIRS_MAX {return SaBands.{status=SaStatus.shape};}
    if !(y.theta>1) || !(y.factor>=1) || !(y.slow>0) || !(y.fast>=2*y.slow) || !(y.attention>0) {return SaBands.{status=SaStatus.number};}
    if !sa_finite(y.theta) || !sa_finite(y.factor) || !sa_finite(y.fast) || !sa_finite(y.attention) {return SaBands.{status=SaStatus.number};}
    if y.original<1 || f64(y.original)*y.factor>f64(SA_CONTEXT_MAX) {return SaBands.{status=SaStatus.range};}
    if freqs.count<y.dim/2 {return SaBands.{status=SaStatus.capacity};}
    d:=f64(y.dim)/(2*log(y.theta));
    low:=floor(d*log(f64(y.original)/(y.fast*2*SA_PI)));
    high:=-floor(-d*log(f64(y.original)/(y.slow*2*SA_PI)));
    if low<0 || high>f64(y.dim-1) {return SaBands.{status=SaStatus.range};}
    for i:0..<y.dim/2 {
        base:=exp(-f64(2*i)/f64(y.dim)*log(y.theta));
        r:=(f64(i)-low)/(high-low);r=(r+abs(r))/2;r=1-((1-r)+abs(1-r))/2;
        freqs[i]=base*(1-r)+base/y.factor*r;
    }
    return SaBands.{low=int(low),high=int(high),positions=int(f64(y.original)*y.factor)};
}
```

The function refuses a band edge below 0 or above `dim - 1` (`range`) instead
of clamping it. It requires `beta_fast >= 2 * beta_slow`: then the two
continuous edges differ by at least `d ln 2`, which keeps `low < high` and
the ramp's denominator positive. The oracle finds the edges another way, by
bisecting on the continuous pair number where the turn count crosses
`beta_fast` and `beta_slow`, and its frequencies agree within the tolerance.

## Run it

<!-- source: psionic/examples/sparse-attention.coax example -->
```jai
main :: () {
    print("profile sparse-attention-cpu-f64-v1, tolerance % x (1 + |oracle|)\n",SA_TOLERANCE);
    // The hand case: two indexer heads, four FP8 keys [1 1] [2 -1] [-1 4] [0.5 0.5].
    q:=[4]f64.[1,0,0,1];w:=[2]f64.[1,0.5];codes:=so_hand_codes();scales:=[4]u8.[127,127,127,127];scores:[4]f64;
    r:=sa_index(2,2,4,q[..],w[..],codes[..],scales[..],scores[..]);#assert r.status==SaStatus.ok;
    print("hand indexer scores % % % %\n",scores[0],scores[1],scores[2],scores[3]);
    for k:1..<4 {ids:[4]int;s:=sa_select(scores[..],4,k,ids[..]);#assert s.status==SaStatus.ok;
        print("  top %: keys",k);for i:0..<s.written {print(" %",ids[i]);}print("\n");}
    aq:=[2]f64.[1,0];keys:[8]f64;values:=[4]f64.[10,4,2,2];o:[1]f64;work:[4]f64;
    sel:=[2]int.[1,2];mask:=[4]u8.[0,1,1,0];every:=[4]int.[0,1,2,3];
    #assert sa_attend(1,2,1,4,aq[..],keys[..],values[..],sel[..],1,0,o[..],work[..]).status==SaStatus.ok;sparse:=o[0];
    #assert sa_dense(1,2,1,4,aq[..],keys[..],values[..],mask[..],1,0,o[..],work[..]).status==SaStatus.ok;dense:=o[0];
    #assert sa_attend(1,2,1,4,aq[..],keys[..],values[..],every[..],1,0,o[..],work[..]).status==SaStatus.ok;
    print("  attention over keys 1 and 2: sparse %, masked dense %, every key %\n",sparse,dense,o[0]);

    // A generated case: 3 indexer heads of width 4, 12 keys, 2 attention heads.
    n:=12;gq:[12]f64;for i:0..<12 {gq[i]=so_value(i+2,13);}gw:=[3]f64.[1,0.5,0.25];
    gc:[48]u8;for i:0..<48 {gc[i]=so_code(i);}gs:[12]u8;for i:0..<n {gs[i]=cast(u8)(126+i%3);}
    for i:0..<4 {gc[28+i]=gc[8+i];}gs[7]=gs[2];
    gscore:[12]f64;ref:[12]f64;#assert sa_index(3,4,n,gq[..],gw[..],gc[..],gs[..],gscore[..]).status==SaStatus.ok;
    so_index(3,4,n,gq[..],gw[..],gc[..],gs[..],ref[..]);
    equal:=true;for i:0..<n {if gscore[i]!=ref[i] {equal=false;}}
    print("generated: 12 indexer scores equal the oracle's %; keys 2 and 7 tie at %\n",equal,gscore[2]);
    hq:[12]f64;for i:0..<12 {hq[i]=so_value(i+5,11);}hk:[72]f64;for i:0..<72 {hk[i]=so_value(i+3,17);}
    hv:[60]f64;for i:0..<60 {hv[i]=so_value(i+7,19);}all:[12]u8;for t:0..<n {all[t]=1;}
    full:[10]f64;fw:[12]f64;#assert sa_dense(2,6,5,n,hq[..],hk[..],hv[..],all[..],0.5,0,full[..],fw[..]).status==SaStatus.ok;
    ks:=[4]int.[3,6,11,2048];
    for c:0..<4 {
        ids:[12]int;chosen:[12]u8;gm:[12]u8;s:=sa_select(gscore[..],n,ks[c],ids[..]);#assert s.status==SaStatus.ok;
        so_topk(gscore[..],n,ks[c],chosen[..]);same:=true;at:=0;
        for t:0..<n {if chosen[t]==1 {if ids[at]!=t {same=false;}at+=1;gm[t]=1;}}
        a:[10]f64;d:[10]f64;oracle:[10]f64;
        #assert sa_attend(2,6,5,n,hq[..],hk[..],hv[..],ids[0..<s.written],0.5,0,a[..],fw[..]).status==SaStatus.ok;
        #assert sa_dense(2,6,5,n,hq[..],hk[..],hv[..],gm[..],0.5,0,d[..],fw[..]).status==SaStatus.ok;
        so_attend(2,6,5,n,hq[..],hk[..],hv[..],chosen[..],0.5,0,oracle[..]);
        bits:=true;asfull:=true;for i:0..<10 {if f64_bits(a[i])!=f64_bits(d[i]) {bits=false;}if f64_bits(a[i])!=f64_bits(full[i]) {asfull=false;}}
        print("  k %: % keys, brute force agrees %, sparse = masked dense bit for bit %, = every key %, oracle gap %\n",
            ks[c],s.written,same && at==s.written,bits,asfull,so_gap(a[..],oracle[..]));
    }

    // Mistral Large 4's pinned maps.
    src:=ML4_KV_SOURCE;rot:=ML4_ROTARY;row:[72]int;for l:0..<72 {row[l]=ML4_KV_ROW;}
    m:=sa_map(src[..],rot[..],row[..]);
    nope:=0;for l:0..<72 {if rot[l]==0 {nope+=1;}}full_index:=0;for l:0..<72 {if ML4_INDEXER_FULL[l]==1 {full_index+=1;}}
    print("Large 4 map: %, % layers, % keep their own KV, % NoPE layers, layer 71 reads layer %\n",m.status,m.layers,m.owners,nope,src[71]);
    c:=sa_cache(src[..],rot[..],row[..],1048576,1);
    print("  KV values per token: % shared, % unshared; at 1048576 tokens and 1 byte: % and % bytes\n",c.shared,c.unshared,c.shared_bytes,c.unshared_bytes);
    print("  indexer: % of 72 layers marked full, % index values per token against %\n",full_index,full_index*ML4_INDEX_WIDTH,72*ML4_INDEX_WIDTH);

    // A small model in the same pattern: layer 7 reads layer 3.
    ss:=[8]int.[0,1,2,3,4,5,6,3];sr:=[8]u8.[1,1,1,0,1,1,1,0];sw:=[8]int.[3,3,3,2,3,3,3,2];
    sc:=sa_cache(ss[..],sr[..],sw[..],4,4);
    cache:[80]f64;for l:0..<8 {if ss[l]==l {at:=sa_offset(ss[..],sw[..],4,l);for i:0..<4*sw[l] {cache[at+i]=f64(100*l+i);}}}
    seven:=sa_offset(ss[..],sw[..],4,7);
    print("small shared model, 4 tokens, f32: % bytes shared, % unshared; layer 7 reads offset % (layer 3's), first values % %\n",
        sc.shared_bytes,sc.unshared_bytes,seven,cache[seven],cache[seven+1]);

    // NoPE and rotary placement.
    y:=SaYarn.{dim=4,theta=10000,factor=8,original=512,fast=32,slow=0.002,attention=1};fr:[2]f64;#assert sa_yarn(y,fr[..]).status==SaStatus.ok;
    qx:=[6]f64.[1,0.5,-1,0.25,0.75,1];kx:=[6]f64.[0.5,0.5,1,-1,0.5,2];
    print("rotary scores, positions (7,3) % (1007,1003) % (7,4) %\n",so_pair_score(true,qx[..],kx[..],7,3,fr[..]),
        so_pair_score(true,qx[..],kx[..],1007,1003,fr[..]),so_pair_score(true,qx[..],kx[..],7,4,fr[..]));
    print("NoPE scores, positions (7,3) % (900000,2) %\n",so_pair_score(false,qx[..],kx[..],7,3,fr[..]),so_pair_score(false,qx[..],kx[..],900000,2,fr[..]));

    // Soft-capping at Large 4's 30.
    for x:0..<4 {v:=f64(x*30);print("cap 30: x % -> %, slope %\n",v,sa_cap(v,ML4_SOFTCAP),sa_slope(v,ML4_SOFTCAP));}

    // YaRN at Large 4's parameters.
    ly:=SaYarn.{dim=ML4_ROPE_DIM,theta=ML4_ROPE_THETA,factor=ML4_YARN_FACTOR,original=ML4_YARN_ORIGINAL,fast=ML4_YARN_FAST,slow=ML4_YARN_SLOW,attention=ML4_YARN_ATTENTION};
    lf:[32]f64;b:=sa_yarn(ly,lf[..]);
    print("YaRN: %, low %, high %, positions %\n",b.status,b.low,b.high,b.positions);
    pairs:=[6]int.[0,3,4,8,16,31];for i:0..<6 {p:=pairs[i];print("  pair %: frequency %, wavelength % positions\n",p,lf[p],2*SA_PI/lf[p]);}

    // Refusals.
    ids:[4]int;
    print("refused: k 0: %; k 2049: %\n",sa_select(scores[..],4,0,ids[..]).status,sa_select(scores[..],4,2049,ids[..]).status);
    bad:=scores;bad[1]=so_nan();print("refused: a NaN score: %\n",sa_select(bad[..],4,2,ids[..]).status);
    later:=ss;later[2]=5;lm:=sa_map(later[..],sr[..],sw[..]);print("refused: layer 2 reads layer 5: % at layer %\n",lm.status,lm.at);
    chain:=ss;chain[5]=4;chain[7]=5;cm:=sa_map(chain[..],sr[..],sw[..]);print("refused: layer 7 reads layer 5, which reads layer 4: % at layer %\n",cm.status,cm.at);
    mixed:=ss;mixed[7]=2;mw:=sw;mw[7]=3;mm:=sa_map(mixed[..],sr[..],mw[..]);print("refused: NoPE layer 7 reads rotary layer 2: % at layer %\n",mm.status,mm.at);
    wide:=sw;wide[7]=3;wm:=sa_map(ss[..],sr[..],wide[..]);print("refused: width 3 reads width 2: % at layer %\n",wm.status,wm.at);
    slow:=ly;slow.fast=1.5;print("refused: YaRN fast 1.5 against slow 1: %\n",sa_yarn(slow,lf[..]).status);
    nanq:=q;nanq[0]=so_nan();print("refused: a NaN indexer query: %\n",sa_index(2,2,4,nanq[..],w[..],codes[..],scales[..],scores[..]).status);
}
```

```text
profile sparse-attention-cpu-f64-v1, tolerance 1e-12 x (1 + |oracle|)
hand indexer scores 1.5 2.0 2.0 0.75
  top 1: keys 1
  top 2: keys 1 2
  top 3: keys 0 1 2
  attention over keys 1 and 2: sparse 3.0, masked dense 3.0, every key 4.5
generated: 12 indexer scores equal the oracle's true; keys 2 and 7 tie at 3.9736328125
  k 3: 3 keys, brute force agrees true, sparse = masked dense bit for bit true, = every key false, oracle gap 0.0
  k 6: 6 keys, brute force agrees true, sparse = masked dense bit for bit true, = every key false, oracle gap 6.910406069286122e-17
  k 11: 11 keys, brute force agrees true, sparse = masked dense bit for bit true, = every key false, oracle gap 4.640850727473248e-17
  k 2048: 12 keys, brute force agrees true, sparse = masked dense bit for bit true, = every key true, oracle gap 2.3532612866991853e-17
Large 4 map: ok, 72 layers, 63 keep their own KV, 18 NoPE layers, layer 71 reads layer 35
  KV values per token: 36288 shared, 41472 unshared; at 1048576 tokens and 1 byte: 38050725888 and 43486543872 bytes
  indexer: 44 of 72 layers marked full, 5632 index values per token against 9216
small shared model, 4 tokens, f32: 320 bytes shared, 352 unshared; layer 7 reads offset 36 (layer 3's), first values 300.0 301.0
rotary scores, positions (7,3) 3.8963223124948048 (1007,1003) 3.8963223124948043 (7,4) 2.973822799109444
NoPE scores, positions (7,3) 1.875 (900000,2) 1.875
cap 30: x 0.0 -> 0.0, slope 1.0
cap 30: x 30.0 -> 22.847824678672946, slope 0.41997434161402614
cap 30: x 60.0 -> 28.920827402274504, slope 0.07065082485316465
cap 30: x 90.0 -> 29.851642610601917, slope 0.00986603716543999
YaRN: ok, low 3, high 16, positions 1048576
  pair 0: frequency 1.0, wavelength 6.283185307179586 positions
  pair 3: frequency 0.4216965034285822, wavelength 14.899780425245321 positions
  pair 4: frequency 0.29191443079161006, wavelength 21.52406542609394 positions
  pair 8: frequency 0.06155724158653845, wavelength 102.07061176298086 positions
  pair 16: frequency 4.8828124999999976e-06, wavelength 1286796.35091038 positions
  pair 31: frequency 6.511335117984979e-08, wavelength 96496113.21378285 positions
refused: k 0: range; k 2049: range
refused: a NaN score: number
refused: layer 2 reads layer 5: later at layer 2
refused: layer 7 reads layer 5, which reads layer 4: chain at layer 7
refused: NoPE layer 7 reads rotary layer 2: mismatch at layer 7
refused: width 3 reads width 2: mismatch at layer 7
refused: YaRN fast 1.5 against slow 1: number
refused: a NaN indexer query: number
```

The generated rows show the selection matching brute force at every `k`,
sparse and masked dense attention equal bit for bit, and sparse attention
equal to attention over every key only at `k = 2048`, where the 12 keys all
fit. The oracle gaps are at most a few units of binary64 rounding. The rotary
scores at positions (7, 3) and (1007, 1003) differ in the 16th digit; the
NoPE scores are identical.

## Refusals

The last lines of the transcript are deliberate failures, and the run still
exits 0 because each refusal is a typed result:

- `k` of 0 or 2,049 is refused as `range`.
- A NaN score is refused as `number` before any key is chosen.
- Layer 2 reading layer 5 is refused as `later`; layer 7 reading layer 5,
  which itself reads layer 4, as `chain`; a NoPE layer reading a rotary one,
  and a width-3 layer reading a width-2 one, as `mismatch`. Each names the
  first bad layer.
- YaRN with `beta_fast` 1.5 against `beta_slow` 1 is refused as `number`.
- A NaN indexer query is refused as `number`.

The checks cover every refusal of every function, in order: shapes, counts,
non-finite values, too-short outputs and work buffers, unordered or repeated
key lists, a score overflow that stops the scorer after one key, and maps of
0 or 129 layers. A refused call leaves its outputs unchanged. `coax mutate`
kills every mutant of the library (486 of 486).

## What remains

- **Real layers.** No Large 4 weights are read. The draft's goal, indexer
  selections matching references for selected layers and tokens, needs the
  indexer weights (`indexer.wk`, `indexer.weights_proj` and the query
  projection) of chosen layers and reference selections recorded from the
  reference implementation. The checkpoint is 54 consolidated shards of
  about 20 GB each; neither the range reads into them nor the reference
  selections exist yet.
- **The indexer's full path.** The scorer takes indexer queries, weights and
  keys as given. Their projections, any normalization or rotation of the
  indexer key, and the weight scaling Large 4 applies are not implemented.
- **Cache layout.** The accounting counts latent-row values. Large 4's FP8
  KV cache layout, with its scales and the precision of the rotary part, is
  not pinned.
- **Indexer sharing.** Which full layer a "shared" indexer layer reads is not
  in the configuration and is not modeled.
- **Conventions.** Split-half rotary pairs and YaRN's rounded band edges
  follow chapter 62 and the common YaRN implementation; neither is confirmed
  against Large 4's reference code.
- **Devices and scale.** Only the CPU profile runs. Selecting 2,048 of a
  million keys per query needs a device kernel, and the whole model runs in
  [chapter 199](ch199-le-chonk-across-a-mesh.md).

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): the compensated accumulator.
- [Chapter 62](ch062-attention-with-sinks-and-a-window.md): split-half YaRN
  rotary positions.
- [Chapter 82](ch082-original-fp8-weights.md): FP8 E4M3 decode and UE8M0
  scales.
- [Chapter 84](ch084-attention-through-a-latent.md): latent attention and
  exact cache accounting.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md): Mistral Large 4 as
  configured.
- [Large 4 configuration](https://huggingface.co/mistralai/Mistral-Large-4-1T-A52B/blob/395730aefd2f928daaf7b69fbd9fefab0ee69b64/config.json)
  at the pinned revision.
