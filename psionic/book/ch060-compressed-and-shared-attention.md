# 60. Compressed and shared attention

V4.1 attends to a local window and a few selected compressed positions through
one shared KV head. A compression source produces new KV and index keys;
later layers reuse them. Indexer sources choose positions for layers that
follow, and one source chooses candidate blocks for subsequent indexers.
This chapter implements those numerical and state boundaries in Coax.

The profile is `deepseek-sparse-cpu-wide-v1`. Inputs enter at learned projection
boundaries: BF16 query/window vectors, F32 compressor gate/KV projections and
BF16 projected index weights. It supplies compression, normalization, rotary
operations, exact cache grids, index scores, selection, sharing checks and
attention mixing. Learned projection binding and complete V4.1 execution
belong to chapter 68; these inputs do not constitute a full transformer.

The pinned [model](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/model.py)
and [kernel](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/kernel.py)
were read as source. Their retained identities/text and original configuration
are shared with [chapter 66](ch066-pack-the-cache-exactly.md), in
[cache-source.json](../fixtures/deepseek/cache-source.json).

## Produce a compressed row

Ratio 1 is an ungated BF16 projection followed by RMS normalization. Ratio 2
has independent F32 KV and gate projections for each channel. Softmax across
the two tokens gives that channel's pooling weights. A completed pooled value
rounds through F32 to BF16 before RMS normalization, with epsilon `1e-20`
inside the square root and BF16 norm weights.

An odd trailing token remains as F32 projected KV and gate state. It produces
no compressed row until its partner arrives. The latent represents the first
token of its group, so group j uses rotary position `j*ratio`. Canonical state
retains only the unfinished tail; completed groups clear that tail to zero.

`da_pool_chunk` handles a whole chunk and returns the same completed latents
and tail as one-token calls. It stages every latent and both outgoing tail
vectors. Failure in the last token publishes none of them. The chunk plan
counts completed groups across the incoming partial group and exposes exact
work sizes: `6*width` F32 slots and `(completed_rows+4)*width` BF16 slots.
These compressor buffers are separate from chapter 66's codec work inventory.

<!-- source: psionic/lib/deepseek/sparse-chunk.coax pool-chunk -->
```jai
da_pool_chunk :: (ratio:int,position:int,chunk:int,width:int,kv:[]f32,gate:[]f32,old_kv:[]f32,old_gate:[]f32,norm:[]u16,latent:[]u16,next_kv:[]f32,next_gate:[]f32,work:DaPoolWork,overlap:bool)->DaResult #export_c {
    p:=da_chunk_plan(ratio,position,chunk,width);if p.status!=DkStatus.ok {return DaResult.{status=p.status};}
    if kv.count!=chunk*width || gate.count!=chunk*width || old_kv.count!=width || old_gate.count!=width || norm.count!=width || latent.count<p.values || next_kv.count<width || next_gate.count<width || work.floats.count<p.floats || work.bits.count<p.bits {return DaResult.{status=DkStatus.capacity};}if overlap {return DaResult.{status=DkStatus.alias};}
    v:=da_chunk_views(work,p,width);for j:0..<width {v.a[j]=old_kv[j];v.ag[j]=old_gate[j];}count:=0;
    for t:0..<chunk {a:=ifx t%2==0 then v.a else v.b;ag:=ifx t%2==0 then v.ag else v.bg;b:=ifx t%2==0 then v.b else v.a;bg:=ifx t%2==0 then v.bg else v.ag;
        r:=da_pool(ratio,position+t,kv[t*width..<(t+1)*width],gate[t*width..<(t+1)*width],a,ag,norm,v.one,b,bg,DaPoolWork.{floats=v.pool,bits=v.temp},false);if r.status!=DkStatus.ok {return r;}
        if r.ready {for j:0..<width {v.batch[count*width+j]=v.one[j];}count+=1;}}
    a:=ifx chunk%2==0 then v.a else v.b;ag:=ifx chunk%2==0 then v.ag else v.bg;
    for j:0..<p.values {latent[j]=v.batch[j];}for j:0..<width {next_kv[j]=a[j];next_gate[j]=ag[j];}
    return DaResult.{written=p.values,next=position+chunk,first=ifx p.rows>0 then position-position%ratio else 0,ready=p.rows>0};
}
```

Index keys derive from the normalized latent **before** compressed-KV RoPE
overwrites it. The key projection/norm then takes its own rotary operation
and per-32 FP4 grid. Compressed KV uses per-16 FP4 with FP8 scales. Window
KV uses per-32 FP8 with exponent scales. `da_store` stages and publishes one
packed row; callers use a private cache shadow when a larger operation must
commit several rows or layers together.

## Keep rotary profiles distinct

Pure-window layers use base 10,000 without YaRN. Layers with compression use
base 160,000, original context 65,536, factor 16 and beta 32/1. Both their
window and compressed paths use that frequency profile. The original rotary
tail is 64 values, arranged as adjacent complex pairs; V2's output permutation
does not apply here. The attention result takes the inverse rotation at the
query's position before the output projection's FP8 input grid.

The CPU profile makes frequency and angle F32 boundaries explicit, with wide
elementary functions and paired products. Its coefficients are independently
checked through position 1,048,575. GPU elementary operations and reduction
orders still require their own qualification.

## Share by producer and query

| Layers | Compressed KV/index-key owner | Selected-position owner | Candidate owner |
| --- | --- | --- | --- |
| 0–1 | None | None | None |
| 2–7 | 2, ratio 2 | 2 | None |
| 8–13 | 8, ratio 2 | 8 | None |
| 14–19 | 14, ratio 2 | 14 | None |
| 20–23 | 20, ratio 1 | 20 | 20 |
| 24–27 | 20 | 24 | 20 |
| 28–31 | 20 | 28 | 20 |
| 32–35 | 20 | 32 | 20 |
| 36–39 | 20 | 36 | 20 |
| 40–42, draft windows | None | None | None |

`da_flash_links` reproduces all 43 original layer positions from the pinned
configuration. `da_reuse` checks publication receipts. KV storage may cover
a later prefix, but must cover this query and name its pinned owner. Reused
selected-position and candidate rows must name both the pinned owner and this
exact query position. Declared receipts do not authenticate model identity;
the caller owns matching immutable parameters and state.

The visible compressed length is `floor((position+1)/ratio)`. Future rows,
unselected compressed rows and unfilled window slots are not read. The live
window is traversed oldest first through its physical ring slots.

## Select blocks, then positions

The indexer rounds query/key inputs to their original FP4 grid. It forms BF16
dot results, applies ReLU, multiplies BF16 scaled signed head weights and
combines heads through BF16 boundaries. Wide compensated reductions define
the CPU profile. It returns the BF16 score values as F32 for selection.

Candidate selection scores each block by its best reachable position. It pins
the newest, possibly partial block, then keeps the other highest blocks. The
original block size is eight with 2,048 blocks kept. Tiny cases use smaller
counts so dropping a block is observable. Later indexers reuse the source's
mask, score positions with their own queries/weights and select only reachable
positions in those blocks.

A bounded heap gives score priority, with lowest-position ties. Selected
positions are then returned in position order. This makes the CPU profile
deterministic; upstream device top-k tie behavior is not asserted. Candidate
masks may include future positions in a selected partial block, but the
visible-prefix bound still excludes those positions from selection.

## Mix one shared KV head and retain sink mass

`da_attend` concatenates the live window's positions with the selected compressed
positions. Every query head reads the same KV vectors. It computes scaled dot
scores and a stable softmax including the learned per-head sink logit; the
sink contributes denominator mass with no value vector. Results publish as
BF16 only after all heads succeed.

This CPU profile uses an ideal wide softmax. The upstream GPU kernel uses
online FP32 reductions and BF16 probability tiles; that rounding path is not
reproduced here. Device/distribution qualification remains in chapter 68.
Cache grids, causal visibility, sharing, rotary placement and selection are
explicit rather than hidden behind that numerical distinction.

The [example](../examples/deepseek-sparse.coax) runs both compressors, packs a
window and compressed cache, pins a newest block and performs two-head sparse
attention with sink mass.

```shell
coax/bin/coax run psionic/examples/deepseek-sparse.coax
coax/bin/coax run psionic/examples/deepseek-sparse.coax --release
psionic/tests/check_deepseek_sparse.sh
psionic/check.sh --only deepseek/sparse --jobs 8
```

<!-- source: psionic/examples/deepseek-sparse.coax example -->
```jai
main :: () {
    kv:[288]f32;gate:[288]f32;norm:[32]u16;old:[32]f32;old_gate:[32]f32;
    for i:0..<288 {kv[i]=f32((i*7)%19-9)/8;gate[i]=f32((i*11)%23-11)/4;}for i:0..<32 {norm[i]=0x3f80;}
    latent:[288]u16;tail:[32]f32;tail_gate:[32]f32;floats:[192]f32;bits:[416]u16;
    for ratio:1..<3 {
        r:=da_pool_chunk(ratio,0,9,32,kv[..],gate[..],old[..],old_gate[..],norm[..],latent[..],tail[..],tail_gate[..],DaPoolWork.{floats=floats[..],bits=bits[..]},false);
        #assert r.status==DkStatus.ok && r.next==9;
        print("ratio %: % complete rows, % pending token\n",ratio,r.written/32,9%ratio);
    }
    compressed_bytes:[64]u8;compressed_scales:[8]u8;window_bytes:[96]u8;window_scales:[3]u8;pack:[66]u8;
    compressed:=DkPacked.{data=compressed_bytes[..],scales=compressed_scales[..]};window:=DkPacked.{data=window_bytes[..],scales=window_scales[..]};
    rotated:[64]u16;stage:[64]u16;
    for row:0..<4 {
        r:=da_rotate(true,2*row,1,32,8,false,latent[row*32..<row*32+32],rotated[..],stage[..],false);#assert r.status==DkStatus.ok;
        r=da_store(1,4,32,row,rotated[0..<32],compressed,pack[..],false);#assert r.status==DkStatus.ok;
    }
    input:[32]u16;normalized:[32]u16;
    for pos:6..<9 {
        for j:0..<32 {input[j]=dk_bf16(f32((pos+j*3)%17-8)/8).bits;}
        r:=da_norm(input[..],norm[..],normalized[..],stage[..],false);#assert r.status==DkStatus.ok;
        r=da_rotate(true,pos,1,32,8,false,normalized[..],rotated[..],stage[..],false);#assert r.status==DkStatus.ok;
        r=da_store(0,3,32,pos%3,rotated[0..<32],window,pack[..],false);#assert r.status==DkStatus.ok;
    }
    scores:=[4]f32.[0.5,8,2,-1];mask:[4]u8;mask_stage:[4]u8;blocks:[2]f32;heap:[2]int;ids:[2]int;
    selection:=da_candidates(scores[..],4,2,1,mask[..],mask_stage[..],blocks[..],heap[..],false);#assert selection.status==DkStatus.ok;
    selection=da_select(scores[..],4,mask[..],2,ids[..],heap[..],false);#assert selection.status==DkStatus.ok && selection.count==2;
    query:[64]u16;for j:0..<64 {query[j]=dk_bf16(f32(j%13-6)/8).bits;}
    r:=da_rotate(true,8,2,32,8,false,query[..],rotated[..],stage[..],false);#assert r.status==DkStatus.ok;
    sink:=[2]f32.[0,0];answer:[64]u16;work_scores:[5]f64;work_bits:[64]u16;
    r=da_attend(DaAttention.{heads=2,width=32,window=3,capacity=9,ratio=2},8,rotated[..],window,4,compressed,ids[..],sink[..],answer[..],DaAttentionWork.{scores=work_scores[..],bits=work_bits[..]},false);#assert r.status==DkStatus.ok && r.written==64;
    link:=da_flash_links(24);
    print("layer 24: KV owner %, index owner %, candidate owner %\n",link.kv,link.index,link.candidate);
    print("newest candidate block stays; selected positions %, %\n",ids[0],ids[1]);
    print("two heads attend over three window rows, two compressed rows and sink mass\n");
}
```

The independent prefix reference covers a 43-node graph over 19 projected
tokens. It checks all 52,288 output words, all selected positions and candidate
masks, complete packed caches and canonical tails. Whole, single-token and
irregular chunk compressor calls agree. Separate native fixtures execute
32×128 index scoring, 64×512 rotary/attention and a wrapped 128-row window.
The [verification record](../../docs/verification/psionic-ch60-2026-10-29.md)
contains actual commands, source/build identities and the qualification scope.
