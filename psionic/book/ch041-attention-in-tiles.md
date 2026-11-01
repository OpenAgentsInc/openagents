# 41. Attention in tiles

> **Status: partly implemented.**

The attention in [chapter 10](ch010-attention-without-training.md),
[chapter 12](ch012-decisions-have-a-different-mask.md) and
[chapter 62](ch062-attention-with-sinks-and-a-window.md) scores a query
against every visible key, finds the largest score, and only then
exponentiates and sums. Applied to a whole prompt at once, that means a score
for every pair of query and key: memory that grows with the square of the
prompt length. This chapter computes the same attention one tile of keys and
values at a time with an **online softmax**, so no query-by-key score matrix
ever exists. The order of the additions changes, so the result gets its own
named numerical profile and a tolerance against the dense references, frozen
before any comparison.

What runs is a bounded CPU implementation over projected Q, K and V:
the online softmax, chunked prefill, grouped-query tile sharing, chapter 12's
decision mask, chapter 62's sinks and sliding windows, and split-K decode,
each compared with the earlier chapters' code. The tiled kernel in
[chapter 37](ch037-a-coax-compute-subset.md)'s device compute subset is not
built yet; the last section lists what remains.

```shell
coax/bin/coax run psionic/examples/tiled-attention.coax
coax/bin/coax test psionic/lib/tiled-attention.coax
```

```text
profile tiled-attention-cpu-f64-v1, tolerance 0.00001 x (1 + largest |value|)
tile 1: maximum 1.0, denominator 1.3678794411714423, value sum 3.4715177646857693
tile 2: maximum 3.0, denominator 2.0, value sum 14.0
merged: maximum moved true, rescale 0.1353352832366127, denominator 2.185122351604477, value sum 14.469818839944681
output 6.621972, tile loads 2, rescales 1
chapter 12 dense attention, causal, window 3 and decision block 6: largest difference 0.0
chapter 62 sinks, full and window 4: largest difference 0.0
chunked prefill 3 + 3 + 2 equals whole prefill bit for bit: true
split-K decode, causal, at position 7 in 8 tiles of one key:
  1 splits: bit for bit true, largest difference 0.0, rescales 2
  2 splits: bit for bit true, largest difference 0.0, rescales 4
  3 splits: bit for bit true, largest difference 0.0, rescales 5
  4 splits: bit for bit true, largest difference 0.0, rescales 3
  5 splits: bit for bit true, largest difference 0.0, rescales 3
  6 splits: bit for bit true, largest difference 0.0, rescales 3
  7 splits: bit for bit true, largest difference 0.0, rescales 1
  8 splits: bit for bit true, largest difference 0.0, rescales 1
tile loads, 16 rows, 2 key/value heads, 16 keys in tiles of 4:
  2 query heads: 8 loads; one copy per query head would be 8
  4 query heads: 8 loads; one copy per query head would be 16
  8 query heads: 8 loads; one copy per query head would be 32
work for 64-row chunks, 8 heads over 2 key/value heads, width 64, tiles of 32 (f64 elements):
  512 tokens: tiled work 37986; a full score matrix would hold 2097152
  1024 tokens: tiled work 37986; a full score matrix would hold 8388608
  2048 tokens: tiled work 37986; a full score matrix would hold 33554432
  4096 tokens: tiled work 37986; a full score matrix would hold 134217728
refused: decision block 5 past 4 keys: mask, output -7.0
refused: an infinite key: number, output -7.0
```

The [library](../lib/tiled-attention.coax), its
[check oracles](../tests/tiled-attention-oracle.coax) and the
[example](../examples/tiled-attention.coax) are the whole implementation.
Inputs are synthetic, already-projected `f32` vectors; no model weights are
read.

## Work two tiles by hand

Take one head of width 1, the query `1`, four keys `1, 0, 3, 3` and their
values `2, 4, 6, 8`. With a head width of 1, the scale `1 / sqrt(1)` is 1, so
the scores equal the keys. Split the keys into two tiles of two.

Each tile on its own produces a **partial**: its largest score `m`, the
denominator `l = sum exp(s - m)` and the value sums `a = sum exp(s - m) v`.

```text
tile 1: scores 1, 0    m = 1   l = 1 + e^-1 = 1.3678794411714423
                               a = 2 + 4 e^-1 = 3.4715177646857693
tile 2: scores 3, 3    m = 3   l = 1 + 1 = 2
                               a = 6 + 8 = 14
```

The running state starts empty and takes tile 1's partial as it is. When
tile 2 arrives, the running maximum moves from 1 to 3. Everything summed so
far was measured against the old maximum, so it is rescaled by
`exp(1 - 3) = e^-2 = 0.1353352832366127` before tile 2 is added:

```text
m = 3
l = 1.3678794411714423 * e^-2 + 2  = 2.185122351604477
a = 3.4715177646857693 * e^-2 + 14 = 14.469818839944681
output = a / l = 6.621971913526893, narrowed to f32: 6.621972
```

The dense formula gives the same value:
`(2e + 4 + 6e^3 + 8e^3) / (e + 1 + 2e^3) = 6.621971913526893`. The example
prints each line of this trace, and a check asserts each number against
`exp` expressions written in the check itself.

## Merge partials

A state and a partial have the same layout: `[maximum, denominator, value
sums...]`. A zero denominator means empty, because any visible key
contributes `exp(0) = 1` to its own partial. Merging rescales both sides to
the larger maximum and adds them:

<!-- source: psionic/lib/tiled-attention.coax merge -->
```jai
ta_merge :: (a:[]f64,b:[]f64)->bool {
    if b[1]==0 {return false;}
    if a[1]==0 {for i:0..<a.count {a[i]=b[i];}return false;}
    moved:=b[0]>a[0];m:=ifx moved then b[0] else a[0];
    ra:=exp(a[0]-m);rb:=exp(b[0]-m);
    a[0]=m;
    for i:1..<a.count {a[i]=a[i]*ra+b[i]*rb;}
    return moved;
}
```

`ta_merge` returns whether the running maximum moved. The kernel counts these
as **rescales**: the hand trace has one. Equal maxima do not count, so a tie
keeps the earlier side's maximum unchanged.

A tile's partial skips every key the mask hides. If the mask hides the whole
tile, the partial reports `-1` and nothing is merged. Scores are `f64`; the
dot product and the sums inside a tile use chapter 4's compensated
accumulator:

<!-- source: psionic/lib/tiled-attention.coax partial -->
```jai
ta_partial :: (d:int,q:[]f32,tk:[]f64,tv:[]f64,lo:int,n:int,m:TaMask,query:int,scores:[]f64,part:[]f64)->int {
    best:=-1;
    for j:0..<n {
        if !ta_allowed(m,query,lo+j) {continue;}
        dot:PsionicAccumulator;
        for i:0..<d {dot=psionic_ops_accumulate(dot,f64(q[i])*tk[j*d+i]);}
        scores[j]=psionic_ops_total(dot)/sqrt(f64(d));
        if best<0 || scores[j]>scores[best] {best=j;}
    }
    if best<0 {return -1;}
    maximum:=scores[best];total:PsionicAccumulator;
    for j:0..<n {if ta_allowed(m,query,lo+j) {scores[j]=exp(scores[j]-maximum);total=psionic_ops_accumulate(total,scores[j]);}}
    part[0]=maximum;part[1]=psionic_ops_total(total);
    for i:0..<d {
        sum:PsionicAccumulator;
        for j:0..<n {if ta_allowed(m,query,lo+j) {sum=psionic_ops_accumulate(sum,scores[j]*tv[j*d+i]);}}
        part[2+i]=psionic_ops_total(sum);
    }
    return best;
}
```

## The masks still hold

One rule decides which keys a query row may see:

<!-- source: psionic/lib/tiled-attention.coax allowed -->
```jai
ta_allowed :: (m:TaMask,query:int,key:int)->bool {
    if m.kind==TaMaskKind.decision {return query>=m.block || key<m.block;}
    if key>query {return false;}
    return m.kind!=TaMaskKind.window || key>query-m.window;
}
```

- **Causal**: keys up to and including the query's position.
- **Window**: the last `window` of those, the current key included, as in
  chapter 62. gpt-oss uses 128.
- **Decision**: chapter 12's block-bidirectional rule with two blocks. A
  prefix row sees every prefix key, and none from the block. A row in the
  block, from position `block` on, sees every key.

Sinks are not a mask. A sink is a partial with the sink logit as its maximum,
denominator 1 and no value, merged into each head's state before the first
tile. It joins the denominator exactly as chapter 62 adds it.

## Load each key/value tile once

The fold walks key/value heads, then tiles. For each tile some row can see,
it copies that head's keys and values once, then runs every query row and
every query head in the head's group against the copy:

<!-- source: psionic/lib/tiled-attention.coax fold -->
```jai
ta_fold :: (s:TaShape,m:TaMask,start:int,rows:int,tokens:int,q:[]f32,keys:[]f32,values:[]f32,first:int,end:int,state:[]f64,scratch:[]f64)->TaResult {
    d:=s.head_dim;kv:=s.kv_heads*d;group:=s.heads/s.kv_heads;one:=d+2;
    tk:=scratch[0..<s.tile*d];tv:=scratch[s.tile*d..<2*s.tile*d];
    scores:=scratch[2*s.tile*d..<2*s.tile*d+s.tile];part:=scratch[2*s.tile*d+s.tile..<2*s.tile*d+s.tile+one];
    count:TaResult;last:=ta_tiles(s,tokens)-1;
    for g:0..<s.kv_heads {
        for t:first..<end {
            lo:=t*s.tile;n:=ifx t==last then tokens-lo else s.tile;
            if !ta_needed(m,start,rows,lo,n) {continue;}
            for j:0..<n {for i:0..<d {tk[j*d+i]=f64(keys[(lo+j)*kv+g*d+i]);tv[j*d+i]=f64(values[(lo+j)*kv+g*d+i]);}}
            count.loads+=1;
            for r:0..<rows {
                for h:g*group..<(g+1)*group {
                    row:=r*s.heads+h;
                    if ta_partial(d,q[row*d..<(row+1)*d],tk,tv,lo,n,m,start+r,scores,part)>=0 {if ta_merge(ta_slot(state,row,one),part) {count.rescales+=1;}}
                }
            }
        }
    }
    return count;
}
```

The result counts these copies as `loads`. With 2 key/value heads and 16
keys in tiles of 4, there are 8 loads whether 2, 4 or 8 query heads share
them. A loop that copied a tile for each query head would need 8, 16 or 32.
A window also skips tiles that no row in the call can see: rows 12 to 15 with
a window of 2 need only keys 11 to 15, so 4 loads instead of 8.

## Chunked prefill

`ta_attend` runs rows `start` to `start + count - 1` against keys `0` to
`tokens - 1`:

<!-- source: psionic/lib/tiled-attention.coax attend -->
```jai
ta_attend :: (s:TaShape,m:TaMask,start:int,count:int,tokens:int,q:[]f32,keys:[]f32,values:[]f32,sinks:[]f32,out:[]f32,work:[]f64,overlap:bool)->TaResult {
    p:=ta_plan(s,count,tokens,0);
    if p.status==TaStatus.ok && (start<0 || start+count>tokens) {p.status=TaStatus.range;}
    status:=ta_admit(p,s,m,tokens,q,keys,values,sinks,out,work,overlap);
    if status!=TaStatus.ok {return TaResult.{status=status};}
    state:=work[0..<p.state];ta_begin(s,count,sinks,state);
    c:=ta_fold(s,m,start,count,tokens,q,keys,values,0,ta_tiles(s,tokens),state,work[p.state..<p.work]);
    r:=ta_finish(s,count,state,out);r.loads=c.loads;r.rescales=c.rescales;return r;
}
```

Chunked prefill calls it once per chunk with `tokens = start + count`, so each
chunk sees the cache as it has grown. Tiles are numbered from key 0 whatever
the chunk, and each row folds the same tiles in the same order, so a chunked
prefill equals a whole prefill bit for bit. A decision prefix is chunked the
same way, except that every prefix chunk gets the whole prefix as keys, as
chapter 12 prepares the whole block before any query attends.

The work a call needs depends on the rows in a chunk, the heads and the tile,
not on the prompt length:

<!-- source: psionic/lib/tiled-attention.coax plan -->
```jai
ta_shape_valid :: (s:TaShape)->bool {
    if s.heads<1 || s.heads>64 || s.kv_heads<1 || s.kv_heads>8 || s.heads%s.kv_heads!=0 {return false;}
    return s.head_dim>=1 && s.head_dim<=256 && s.tile>=1 && s.tile<=64;
}
// rows: query rows per call (at most 64); tokens: keys (at most 4096);
// splits: 0 for one pass, or the key splits of a one-row decode.
ta_plan :: (s:TaShape,rows:int,tokens:int,splits:int)->TaPlan {
    if !ta_shape_valid(s) {return TaPlan.{status=TaStatus.shape};}
    if rows<1 || rows>64 || tokens<1 || tokens>4096 || splits<0 || splits>16 {return TaPlan.{status=TaStatus.range};}
    if splits>0 && (rows!=1 || splits>ta_tiles(s,tokens)) {return TaPlan.{status=TaStatus.range};}
    one:=s.head_dim+2;state:=rows*s.heads*one*(ifx splits>0 then 2 else 1);scratch:=2*s.tile*s.head_dim+s.tile+one;
    return TaPlan.{q=rows*s.heads*s.head_dim,kv=tokens*s.kv_heads*s.head_dim,state=state,scratch=scratch,work=state+scratch};
}
```

For 64-row chunks, 8 heads over 2 key/value heads, width 64 and tiles of 32,
the work is 37,986 `f64` elements at 512, 1,024, 2,048 and 4,096 tokens. A
full score matrix for the same heads would hold `8 x tokens^2` elements:
2,097,152 at 512 tokens and 134,217,728 at 4,096. These are logical element
counts from the plan, not allocator measurements. The keys and values
themselves still grow linearly with the prompt.

## Split-K decode

One decode row over a long cache leaves most of a device idle. Split-K cuts
the tiles into contiguous splits, folds each split from empty into its own
partial state, and merges those states in key order after the sink:

<!-- source: psionic/lib/tiled-attention.coax split -->
```jai
ta_decode_split :: (s:TaShape,m:TaMask,tokens:int,splits:int,q:[]f32,keys:[]f32,values:[]f32,sinks:[]f32,out:[]f32,work:[]f64,overlap:bool)->TaResult {
    p:=ta_plan(s,1,tokens,splits);
    if p.status==TaStatus.ok && splits<1 {p.status=TaStatus.range;}
    status:=ta_admit(p,s,m,tokens,q,keys,values,sinks,out,work,overlap);
    if status!=TaStatus.ok {return TaResult.{status=status};}
    one:=s.head_dim+2;block:=s.heads*one;tiles:=ta_tiles(s,tokens);c:TaResult;
    final:=ta_slot(work,0,block);part:=ta_slot(work,1,block);ta_begin(s,1,sinks,final);
    for j:0..<splits {
        ta_begin(s,1,sinks[0..<0],part);
        f:=ta_fold(s,m,tokens-1,1,tokens,q,keys,values,j*tiles/splits,(j+1)*tiles/splits,part,work[p.state..<p.work]);
        c.loads+=f.loads;c.rescales+=f.rescales;
        for h:0..<s.heads {if ta_merge(ta_slot(final,h,one),ta_slot(part,h,one)) {c.rescales+=1;}}
    }
    r:=ta_finish(s,1,final,out);r.loads=c.loads;r.rescales=c.rescales;return r;
}
```

With one tile per split, the merges happen in exactly the order of the
one-pass fold, and the result is equal bit for bit. Coarser splits merge in a
different order, so floating-point addition may round differently; the
profile allows those the same tolerance as the dense comparison. On the
example's 8 keys every split count from 1 to 8 happens to match bit for bit
after narrowing to `f32`. This CPU version runs the splits one after another
and reuses one partial state; the splits are independent, which is what a
device kernel would run in parallel.

## The profile and its tolerance

Profile `tiled-attention-cpu-f64-v1`:

- `f32` Q, K, V, sinks and outputs; `f64` scores, exponentials and sums.
- The scale is `1 / sqrt(head width)`, applied after the dot product.
- Inside a tile, compensated sums in key order. Across tiles, partials merged
  in key order. A sink is merged first.
- Each output is divided and narrowed to `f32` once, after every row and head
  has finished.

The tolerance was frozen in the check oracles before the first comparison
ran:

<!-- source: psionic/tests/tiled-attention-oracle.coax tolerance -->
```jai
TA_TOLERANCE :: 0.00001;
```

The checks compare the tiled kernel with the earlier chapters' code, which
they load unchanged:

- Chapter 12's `qf_attend`, one query row at a time with a full mask row,
  for causal, window 3 and decision masks over 10 keys, 4 query heads and 2
  key/value heads in tiles of 3, so the last tile is partial. Its gate input
  is 40, whose sigmoid is exactly 1 in `f64`.
- Chapter 62's `ga_step` over 8 positions with two sinks, full and with a
  window of 4, head width 64. The checks keep the rotated queries and keys
  `ga_step` leaves in its work and cache buffers and give those to the tiled
  kernel.

At these sizes the largest difference is 0: every output matches bit for bit
after narrowing to `f32`, although the dense references sum in a different
order and chapter 12's narrows each score to `f32`. The tolerance stays the
contract for inputs where they do not.

Other checks are exact:

- Keys a mask hides can hold any value. The checks overwrite every hidden key
  and value with ±1000 and get the same bits.
- Chunked prefill (3 + 3 + 2 rows, full and windowed, with sinks) and a
  decision prefill (prefix in chunks of 4 and 2, then the block) equal the
  whole prefill bit for bit.
- Split-K with one tile per split equals the one-pass decode bit for bit;
  splits of 1, 3 and 5 stay within the tolerance of both the one-pass result
  and chapter 62's dense output.
- The tile loads are 8 for 2, 4 and 8 query heads over 2 key/value heads.
- Chapter 62's single-sink example, equal logits on one value 3, gives 1.5.

`coax mutate psionic/lib/tiled-attention.coax --root-only` changes the
library one site at a time; every mutant fails a check.

## Refusals

Each call checks everything before it writes. A refusal leaves the output
unchanged and names one cause:

| Status | When |
| --- | --- |
| `shape` | heads outside 1–64, key/value heads outside 1–8 or not dividing the heads, head width outside 1–256, tile outside 1–64 |
| `range` | rows outside 1–64, tokens outside 1–4,096, rows past the last key, or splits outside 1–16, more than the tiles, or with more than one row |
| `mask` | a window below 1, a decision block past the last key, or a field the mask kind does not use set |
| `capacity` | a Q, K, V, sink, output or work view of the wrong length |
| `alias` | the caller reports overlapping views |
| `number` | a non-finite input |
| `arithmetic` | an output that does not fit `f32` |

The example ends with two of them: a decision block that starts at 5 when
there are only 4 keys is a `mask` refusal, and an infinite key is a `number`
refusal. The output stays `-7.0` in both. The checks cover the others; a
non-finite value cannot be built inside a Coax check, so only the compiled
example exercises `number`.

## What this does not do yet

- **A device kernel.** No tiled kernel in chapter 37's compute subset runs
  yet, so there is no `coax cuda` or Metal target and no device timing. The
  CPU fold above is the order a device kernel would have to match.
- **Real model shapes and weights.** The bounds (4,096 tokens, 64 rows per
  call, head width 256) are teaching limits; nothing here reads original
  weights or replaces the attention inside chapters 14, 15 or 64.
- **Chapter 233's measurements.** Chapter 233 has not adopted this profile.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): the grouped-query
  and split-K attention designs.
- Milakov and Gimelshein, *Online normalizer calculation for softmax* (2018),
  and Dao et al., *FlashAttention* (2022): the running maximum and rescale.
