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

Two implementations run. The first is a bounded CPU implementation over
projected Q, K and V: the online softmax, chunked prefill, grouped-query tile
sharing, chapter 12's decision mask, chapter 62's sinks and sliding windows,
and split-K decode, each compared with the earlier chapters' code. The second
is a group kernel in [chapter 37](ch037-a-coax-compute-subset.md)'s compute
subset, with [chapter 68](ch068-coax-kernels-cooperate.md)'s shared tiles and
barriers, that runs on an RTX 4080 at gpt-oss-20b's attention shapes and on
the original weights of one gpt-oss-20b layer. The last section lists what
remains.

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
[example](../examples/tiled-attention.coax) are the CPU implementation. Its
inputs are synthetic, already-projected `f32` vectors. The device kernel
([kernels](../lib/device/tiled-attention-kernels.coax),
[launch checks](../lib/device/tiled-attention-plan.coax),
[oracle](../tests/tiled-attention-device-oracle.coax),
[harness](../tests/tiled_attention_device.c)) is described in
[On the device](#on-the-device); it also reads layer 0 of the original
gpt-oss-20b weights.

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

## On the device

The CPU fold copies a tile once and runs every row against it. A GPU does the
same with a **group kernel** (chapter 68): one block of threads per work
item, threadgroup (CUDA `__shared__`) arrays the whole block reads, and a
barrier around each phase. `td_attend` runs one group of 64 threads for each
block of 64 query rows and each head. Thread `t` owns row `block*64 + t` and
keeps that row's query, running maximum, denominator and 64 value sums in its
own registers. For each tile of 32 keys some row of the block can see, the
group first copies the tile's keys and values into two shared arrays, each
thread copying half a key row and half a value row, and then every thread
scores its row against all 32 keys and updates its running state:

<!-- source: psionic/lib/device/tiled-attention-kernels.coax shape -->
```jai
TD_THREADS :: 64;   // query rows per group, one per thread
TD_TILE :: 32;      // keys per shared tile
TD_DIM :: 64;       // head width; the scale is 1/sqrt(64) = 0.125
TD_EMPTY :: -1000000000000000000000000000000.0;  // the maximum before any key, -1e30
TD_HIDDEN :: -300000000000000000000000000000000000000.0;  // a hidden key's score, -3e38
// Mask kinds as chapter 41's TaMaskKind: 0 causal, 1 window, 2 decision.
td_sees :: (kind:int,window:int,block:int,query:int,key:int)->bool {
    if kind==2 {return query>=block || key<block;}
    if key>query {return false;}
    return kind!=1 || key>query-window;
}
```

`td_sees` is the same rule as `ta_allowed`, so the device sees exactly the
keys the CPU profile sees. A hidden key keeps the score `TD_HIDDEN`, whose
exponential is exactly 0 in `f32`, so it adds nothing to the denominator or
the sums. The kernel:

<!-- source: psionic/lib/device/tiled-attention-kernels.coax kernel -->
```jai
// Group `item` is query block item/heads of head item%heads. Rows r of the
// call are positions start + r, r < rows; q row r of head h is
// q[r*q_stride + h*64 ...]. Key t of key/value head g is
// kv[t*kv_stride + g*64 ...] and its value is v_offset further on. out row r
// of head h is out[(r*heads + h)*64 ...], and its final running maximum
// and denominator are state[(r*heads + h)*2 ...], what a later merge of
// split keys would need. sinks is empty or one per head. tiles[item]
// receives the number of tiles the group loaded.
td_attend :: (item:int,heads:int,kv_heads:int,start:int,rows:int,tokens:int,kind:int,window:int,block:int,
    q_stride:int,kv_stride:int,v_offset:int,q:[]f32,kv:[]f32,sinks:[]f32,out:[]f32,state:[]f32,tiles:[]int) #export_c {
    qr:[4096]f32;acc:[4096]f32;sc:[2048]f32;mx:[64]f32;den:[64]f32;kt:[2048]f32;vt:[2048]f32;
    head:=item%heads;kh:=head/(heads/kv_heads);r0:=item/heads*TD_THREADS;
    last:=r0+TD_THREADS-1;if last>=rows {last=rows-1;}
    // The tiles holding keys some row of the group can see: from tile lo
    // to the tile of key end - 1.
    lo:=0;end:=start+last+1;
    reach:=start+r0-window+1;
    if kind==1 && reach>=TD_TILE {lo=reach/TD_TILE;}
    if kind==2 {end=ifx start+last<block then block else tokens;}
    for thread:0..<TD_THREADS {
        r:=r0+thread;
        for i:0..<TD_DIM {v:f32=0;if r<rows {v=q[r*q_stride+head*TD_DIM+i];}qr[thread*TD_DIM+i]=v;}
        m:f32=TD_EMPTY;l:f32=0;
        if sinks.count>0 {m=sinks[head];l=1;}
        mx[thread]=m;den[thread]=l;
    }
    loaded:=0;
    for t:lo..<(end+TD_TILE-1)/TD_TILE {
        t0:=t*TD_TILE;loaded+=1;
        // Each thread copies half of one key row and half of its value row.
        for thread:0..<TD_THREADS {
            key:=t0+thread/2;d0:=thread%2*32;
            for c:0..<32 {
                k:f32=0;v:f32=0;
                if key<tokens {at:=key*kv_stride+kh*TD_DIM+d0+c;k=kv[at];v=kv[at+v_offset];}
                kt[thread*32+c]=k;vt[thread*32+c]=v;
            }
        }
        // Threads past the last row compute on zero queries; their results
        // are never written.
        for thread:0..<TD_THREADS {
            query:=start+r0+thread;
            {
                // A hidden key keeps the score TD_HIDDEN, whose exp is 0.
                old:=mx[thread];m:=old;
                for j:0..<TD_TILE {
                    s:f32=TD_HIDDEN;
                    if t0+j<tokens && td_sees(kind,window,block,query,t0+j) {
                        dot:f32=0;
                        for i:0..<TD_DIM {dot+=qr[thread*TD_DIM+i]*kt[j*TD_DIM+i];}
                        s=dot*0.125f32;
                    }
                    sc[thread*TD_TILE+j]=s;
                    if s>m {m=s;}
                }
                rescale:=exp(old-m);sum:f32=0;
                for j:0..<TD_TILE {p:=exp(sc[thread*TD_TILE+j]-m);sc[thread*TD_TILE+j]=p;sum+=p;}
                den[thread]=den[thread]*rescale+sum;mx[thread]=m;
                for i:0..<TD_DIM {
                    a:=acc[thread*TD_DIM+i]*rescale;
                    for j:0..<TD_TILE {a+=sc[thread*TD_TILE+j]*vt[j*TD_DIM+i];}
                    acc[thread*TD_DIM+i]=a;
                }
            }
        }
    }
    for thread:0..<TD_THREADS {
        r:=r0+thread;
        if r<rows {
            l:=den[thread];at:=r*heads+head;
            for i:0..<TD_DIM {out[at*TD_DIM+i]=acc[thread*TD_DIM+i]/l;}
            state[2*at]=mx[thread];state[2*at+1]=l;
        }
        // Every thread writes the same count.
        tiles[item]=loaded;
    }
}
// gpt-oss stores its sinks as BF16; the kernel reads f32.
td_sinks :: (item:int,bits:[]u16,out:[]f32) #export_c {out[item]=f32_from_bits(cast(u32)bits[item]<<16);}
```

The arithmetic is the CPU fold's, in `f32`. Inside a tile the running
maximum moves to the largest visible score (a tie keeps the old one), the
old denominator and sums are multiplied once by `exp(old - new)`, and each
visible key adds `exp(score - maximum)` and its share of the values in key
order. Before any key the maximum is `TD_EMPTY`, so the first rescale is
`exp(-1e30 - score) = 0`; a sink starts the state at its logit and
denominator 1 instead, exactly as chapter 62 adds it. Threads past the last
row of a ragged block compute on zero queries and write nothing.

The group loads only the tiles that some row of its block can see: from the
tile of the window's first key to the tile of the last row's last key (for a
decision block, the prefix or every key). It reports the count in
`tiles[item]`, which the checks pin, and writes each row's final maximum and
denominator to `state`, which a split-K merge would need.

### The hand trace on the device

The kernel's head width is fixed at 64, so the two-tile trace above becomes:
query `8` in dimension 0 (score = key value), key 0 scoring 1, key 1 scoring
0, keys 2 to 31 scoring −1000, keys 32 and 33 scoring 3, values 2, 4, 1000,
..., 6 and 8. `exp(-1001)` is 0 in `f32`, so tile 1's partial is the hand
trace's tile 1, and tile 2 moves the maximum from 1 to 3:

```text
tile 1 (keys 0..31):  m = 1   l = 1 + e^-1   a = 2 + 4 e^-1
tile 2 (keys 32..33): rescale e^-2, m = 3
                      l = (1 + e^-1) e^-2 + 2
                      a = (2 + 4 e^-1) e^-2 + 6 + 8
output a / l = 6.621972 (f32), state (3, l), tiles loaded 2
```

The check asserts these `f32` expressions bit for bit and the output within
`1e-6` of the dense `6.621971913526893`.

### Admission and launch checks

Before a launch, `tdp_check` refuses a shape, range, mask or view length that
does not fit, with chapter 41's status names, and `tdp_admit` asks chapter
68's `da_admit` whether the device the host reports can run 64 threads with
16,384 shared bytes:

<!-- source: psionic/lib/device/tiled-attention-plan.coax plan -->
```jai
// What td_attend needs: 64 threads, two shared tiles of 32 x 64 f32.
TDP_NEEDS :: DaNeeds.{threads=64,shared=16384,mma=false};
tdp_admit :: (major:int,minor:int,max_threads:int,max_shared:int)->DaStatus #export_c {
    return da_admit(TDP_NEEDS,DaDevice.{major=major,minor=minor,max_threads=max_threads,max_shared=max_shared});
}
// Groups of a launch: one per block of 64 rows and head.
tdp_groups :: (heads:int,rows:int)->int {return (rows+63)/64*heads;}
// The checks a launch must pass, in order. counts: q, kv, sinks, out,
// state and tiles view lengths.
tdp_check :: (heads:int,kv_heads:int,start:int,rows:int,tokens:int,kind:int,window:int,block:int,
    q_stride:int,kv_stride:int,v_offset:int,q:int,kv:int,sinks:int,out:int,state:int,tiles:int)->TaStatus #export_c {
    if heads<1 || heads>64 || kv_heads<1 || kv_heads>8 || heads%kv_heads!=0 {return TaStatus.shape;}
    if tokens<1 || tokens>131072 || rows<1 || start<0 || start+rows>tokens {return TaStatus.range;}
    if kind<0 || kind>2 || (kind==1)!=(window>0) || window<0 || (kind==2)!=(block>0) || block>=tokens {return TaStatus.mask;}
    width:=heads*64;kw:=kv_heads*64;
    if q_stride<width || kv_stride<kw+v_offset || v_offset<kw {return TaStatus.capacity;}
    if q!=(rows-1)*q_stride+width || kv!=(tokens-1)*kv_stride+v_offset+kw {return TaStatus.capacity;}
    if (sinks!=0 && sinks!=heads) || out!=rows*width || state!=2*rows*heads || tiles!=tdp_groups(heads,rows) {return TaStatus.capacity;}
    return TaStatus.ok;
}
```

`psionic/lib/device/tiled-attention-plan.coax` is `#authority`: its checks
include every refusal at its endpoint, and `coax mutate --root-only` leaves
no survivor in it or in the kernel file.

### The profile and its tolerance

Profile `tiled-attention-cuda-f32-v1`:

- `f32` Q, K, V, sinks, scores, exponentials, sums and outputs.
- A score is the `f32` dot product in dimension order, times `0.125`.
- Per tile: the new maximum, one rescale of the running state, then each
  visible key's `exp(score - maximum)` and value products in key order.
- Each output is divided once, at the end. CUDA is compiled with
  `--fmad=false`, so the device and the C build of the same source differ
  only in `exp`.

The tolerance was frozen in the check oracle before the first comparison
ran:

<!-- source: psionic/tests/tiled-attention-device-oracle.coax tolerance -->
```jai
TD_TOLERANCE :: 0.001;
```

An output may differ from the binary64 oracle, from chapter 41's CPU profile
or from another `f32` attention by at most `TD_TOLERANCE * (1 + the largest
|value|)`. The checks in `tiled-attention-kernels.coax` run the C build of
the kernel, groups in reverse order:

- Against a binary64 dense oracle written separately (it spells the mask as
  each row's key range): causal, window 5, window 40 and decision block 37,
  with and without sinks, 2 query heads over 1 key/value head, 70 rows (a
  full block and a ragged one) and 70 keys (tiles of 32, 32 and 6). The
  largest difference is below `1e-6`.
- 4 query heads over 2 key/value heads read the right key/value head.
- A chunk of rows 40 to 69 equals the whole prefill bit for bit.
- Hidden keys and values set to `1e30` or −1000 give the same bits.
- The tile count of each group, at every edge: a window that starts exactly
  on a tile boundary, a block whose last row reaches the next tile, decision
  prefixes.
- Scores of −200 everywhere still normalize, and a sink of 0 takes all the
  weight; a sink of −0 ties a score of +0 and keeps the maximum.
- `tiled-attention-plan.coax` runs chapter 41's `ta_attend`, one row per
  call, over the kernel's layouts: within `1e-6` for every mask.

### On the RTX 4080

`psionic/tests/check_tiled_attention_device.sh` runs those checks in debug
and release builds, emits the CUDA source, and runs the harness. With a
directory holding gpt-oss-20b's `original/model.safetensors` it also runs
layer 0 of the original model:

```shell
COAX_CC=gcc psionic/tests/check_tiled_attention_device.sh ~/.cache/psionic/gptoss-original/20b/REVISION
```

The harness compares, for each shape:

- the tiled kernel against chapter 66/67's prefill attention kernels
  (`gk_batch_scores`, `gk_batch_softmax`, `gk_batch_context_part`,
  `gk_batch_finish`), every output;
- the tiled kernel against the C build of the same source, on the first and
  last query block of one head per key/value head;
- the last 64 rows against chapter 41's CPU profile and the binary64 oracle,
  and chapter 66/67's last 64 rows against the oracle too.

Inputs are projected Q, K and V at gpt-oss-20b's attention shape: 64 query
heads over 8 key/value heads, width 64, q rows inside the fused 5,120-wide
projection and keys then values in 1,024-wide cache rows, BF16 sinks. The
existing kernels run as chapter 67's prefill program runs them: chunks of 128
rows, a full score row per query and head, a 128-slot ring cache for windowed
layers and a cache of 2,048 positions for full layers. At 4,096 positions a
full layer needs a 4,096-position cache, for which those kernels take their
generic path. Times are chapter 34's warm runs (one warm-up, 10 recorded
runs, `cm_two_pass` and `cm_compare`); each tiled time is one launch, each
existing time the sum of its chunks' kernel times, with the ring-cache
uploads between chunks left out.

```text
device: NVIDIA GeForce RTX 4080 (sm_89); td_attend (64 threads, 16384 shared bytes): admitted
synthetic projected Q/K/V at gpt-oss-20b's attention shape: 64 heads over 8 key/value heads, width 64, BF16 sinks
512 tokens, causal: 512 groups of 64 threads, 4608 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 7.75e-07, 228507 of 2097152 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 2.98e-07, 26856 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 8.64e-07, 12930 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 8.55e-07, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 6.93e-07, within tolerance
  tiled kernel, one launch: mean 0.778 ms, sd 0.006 ms, min 0.771, max 0.792 (10 warm runs)
  chapter 66/67 kernels, chunks of 128: mean 6.154 ms, sd 0.004 ms, min 6.147, max 6.159 (10 warm runs)
  chapter 34 verdict: the tiled kernel is faster (time ratio old/tiled 7.91, standard error 0.002 ms)
512 tokens, window 128: 512 groups of 64 threads, 2688 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 8.94e-07, 287540 of 2097152 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 2.98e-07, 29805 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 1.13e-06, 18748 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 1.12e-06, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 1.21e-06, within tolerance
  tiled kernel, one launch: mean 0.457 ms, sd 0.012 ms, min 0.439, max 0.479 (10 warm runs)
  chapter 66/67 kernels, chunks of 128: mean 2.792 ms, sd 0.018 ms, min 2.776, max 2.832 (10 warm runs)
  chapter 34 verdict: the tiled kernel is faster (time ratio old/tiled 6.12, standard error 0.007 ms)
1024 tokens, causal: 1024 groups of 64 threads, 17408 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 8.94e-07, 343369 of 4194304 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 2.38e-07, 25944 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 8.64e-07, 10227 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 8.63e-07, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 4.75e-07, within tolerance
  tiled kernel, one launch: mean 2.392 ms, sd 0.136 ms, min 2.180, max 2.528 (10 warm runs)
  chapter 66/67 kernels, chunks of 128: mean 22.768 ms, sd 1.254 ms, min 21.221, max 24.694 (10 warm runs)
  chapter 34 verdict: the tiled kernel is faster (time ratio old/tiled 9.52, standard error 0.399 ms)
1024 tokens, window 128: 1024 groups of 64 threads, 5760 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 8.94e-07, 544953 of 4194304 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 2.38e-07, 30459 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 9.54e-07, 18862 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 9.33e-07, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 8.37e-07, within tolerance
  tiled kernel, one launch: mean 0.815 ms, sd 0.016 ms, min 0.798, max 0.843 (10 warm runs)
  chapter 66/67 kernels, chunks of 128: mean 5.799 ms, sd 0.004 ms, min 5.794, max 5.809 (10 warm runs)
  chapter 34 verdict: the tiled kernel is faster (time ratio old/tiled 7.12, standard error 0.005 ms)
2048 tokens, causal: 2048 groups of 64 threads, 67584 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 1.37e-06, 504796 of 8388608 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 2.38e-07, 24773 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 8.94e-07, 7881 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 8.86e-07, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 3.65e-07, within tolerance
  tiled kernel, one launch: mean 7.845 ms, sd 0.265 ms, min 7.665, max 8.556 (10 warm runs)
  chapter 66/67 kernels, chunks of 128: mean 87.102 ms, sd 2.388 ms, min 84.809, max 91.654 (10 warm runs)
  chapter 34 verdict: the tiled kernel is faster (time ratio old/tiled 11.10, standard error 0.760 ms)
2048 tokens, window 128: 2048 groups of 64 threads, 11904 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 1.37e-06, 1057972 of 8388608 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 2.38e-07, 30492 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 1.19e-06, 18537 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 1.19e-06, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 8.61e-07, within tolerance
  tiled kernel, one launch: mean 1.622 ms, sd 0.012 ms, min 1.601, max 1.643 (10 warm runs)
  chapter 66/67 kernels, chunks of 128: mean 11.990 ms, sd 0.011 ms, min 11.969, max 12.006 (10 warm runs)
  chapter 34 verdict: the tiled kernel is faster (time ratio old/tiled 7.39, standard error 0.005 ms)
4096 tokens, causal: 4096 groups of 64 threads, 266240 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 1.34e-06, 739832 of 16777216 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 2.38e-07, 23726 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 7.45e-07, 6122 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 7.36e-07, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 3.57e-07, within tolerance
  tiled kernel, one launch: mean 29.498 ms, sd 0.177 ms, min 29.151, max 29.685 (10 warm runs)
  chapter 66/67 generic kernels (cache 4096), chunks of 128: mean 685.180 ms, sd 1.427 ms, min 683.371, max 687.897 (10 warm runs)
  chapter 34 verdict: the tiled kernel is faster (time ratio old/tiled 23.23, standard error 0.455 ms)
4096 tokens, window 128: 4096 groups of 64 threads, 24192 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 1.19e-06, 2094825 of 16777216 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 2.38e-07, 30656 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 1.13e-06, 18644 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 1.12e-06, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 9.56e-07, within tolerance
  tiled kernel, one launch: mean 2.977 ms, sd 0.144 ms, min 2.766, max 3.078 (10 warm runs)
  chapter 66/67 kernels, chunks of 128: mean 23.287 ms, sd 1.072 ms, min 21.784, max 24.088 (10 warm runs)
  chapter 34 verdict: the tiled kernel is faster (time ratio old/tiled 7.82, standard error 0.342 ms)
```

Every comparison held. On these synthetic inputs (uniform in ±2) the
largest difference against the binary64 oracle was about `1e-6` for both
kernels, well inside the tolerance. The tiled kernel was faster at every
length, and the gap grows with the prompt: the existing kernels write and
read a score for every visible key of every row, and at 4,096 positions they
also fall to their generic path. These are single-kernel attention times on
one card, not end-to-end prefill.

### Original weights

For the real-weight check the harness reads layer 0's tensors from the
original checkpoint: the embedding rows of the prompt's tokens, the attention
RMS-norm scale, the BF16 QKV weight and bias, and the sinks. Chapter 66/67's
own kernels compute the embedding, the RMS norm, the projection and the YaRN
rotation (`gk_batch_embed`, `gk_batch_norm_part`, `gk_batch_norm`,
`gk_batch_bf_part`, `gk_batch_finish`, `gk_batch_rotate`, with the rotary
table from `go_yarn`). The prompt is the 76 recorded tokens of
`psionic/fixtures/gptoss/qualification/20b/generation.i64`, repeated to 512
positions. Layer 0 is a sliding-window layer; the harness also runs the same
Q, K and V under the full layers' causal mask:

```text
original gpt-oss-20b layer 0 on 76 recorded tokens repeated to 512 positions (non-finite projection flags: 0)
512 tokens, window 128: 512 groups of 64 threads, 2688 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 3.81e-06, 670414 of 2097152 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 4.77e-07, 47928 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 7.15e-06, 20704 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 7.26e-06, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 7.74e-06, within tolerance
  single runs: tiled 0.434 ms, chapter 66/67 kernels 2.600 ms
the same layer-0 Q/K/V under the full layers' causal mask:
512 tokens, causal: 512 groups of 64 threads, 4608 tile loads
  tiled kernel against the chapter 66/67 kernels: largest difference 8.58e-06, 480420 of 2097152 bit-identical, within tolerance
  tiled kernel against its C build (16 groups): largest difference 9.54e-07, 48594 of 65536 bit-identical, within tolerance
  last 64 rows against chapter 41's CPU profile: largest difference 3.01e-05, 18315 of 262144 bit-identical, within tolerance
  last 64 rows against the binary64 oracle: largest difference 3.01e-05, within tolerance
  chapter 66/67 kernels' last 64 rows against the oracle: largest difference 3.02e-05, within tolerance
  single runs: tiled 0.783 ms, chapter 66/67 kernels 5.901 ms
chapter 41 on CUDA: every comparison within the frozen tolerance
```

On the real layer the largest difference between the two device kernels was
`3.8e-6` under the window and `8.6e-6` under the full mask, and both stayed
within `3.1e-5` of the binary64 oracle, against a bound of `0.001 * (1 + the
largest |value|)`. The real scores are larger than the synthetic ones, so the
differences are larger too, but they remain far inside the tolerance. The
single-run times are reported, not compared: the statistics are the
synthetic runs above.

## What this does not do yet

- **Serving.** `td_attend` is not in chapter 66/67's device program; prefill
  still runs the score-matrix kernels. Replacing them needs the program's
  plan, flags and replay to adopt the kernel.
- **Split-K decode on the device.** The kernel writes each row's maximum and
  denominator, but no device merge of split partials runs yet.
- **Other layers and devices.** The real-weight check covers layer 0's
  attention on one prompt. Layers after it need the whole layer stack. The
  kernel has not been compiled with `coax metal-compute` or run on an Apple
  GPU.
- **Tuning.** The register arrays are indexed in loops, so NVRTC may keep
  them in local memory; the tile and block sizes are fixed and untuned.
- **Chapter 233's measurements.** Chapter 233 has not adopted either profile.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): the grouped-query
  and split-K attention designs.
- [Chapter 68](ch068-coax-kernels-cooperate.md): group kernels, shared tiles
  and device admission; [chapter 34](ch034-benchmark-against-llama-cpp-and-cublas.md):
  the warm-run statistics.
- Milakov and Gimelshein, *Online normalizer calculation for softmax* (2018),
  and Dao et al., *FlashAttention* (2022): the running maximum and rescale.
