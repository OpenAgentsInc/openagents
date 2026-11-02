# 101. Maya's recipe: V4.1 Flash on one card

> **Status: partly implemented.** The recipe runs as a plan on the CPU
> (profile deepseek-v41-recipe-v1). Every tensor kind of the pinned V4.1
> Flash checkpoint gets the format Project Maya's Maya-S quant gives it, and
> the byte totals come from chapter 100's header manifest exactly: a
> 178,203,530,688-byte model file and 161,284,798,500 bytes of Engram tables,
> which reproduce Maya's published 178.2 GB and 161.3 GB. The plan checks the
> recipe's published figures against that arithmetic and flags two that it
> does not support. It derives the prompt-skip bound of 2,414 positions from
> the layers that keep state, places each layer's experts across a 16 GB GPU,
> RAM and NVMe disk, replays a routing trace through chapter 191's cache
> simulator, and refuses this host because the 339.5 GB of files do not fit
> on its 37.6 GB of free disk. Still to build: the IQ2 and IQ3 codecs and
> their quality cards, the importance-weighted GPTQ rounding, the requantized
> files themselves, real routing traces, the same-logits check of the prompt
> skip, and the measured quality card against chapter 100's reference. All
> of them need the 501 GB of original weights.

[Project Maya](https://github.com/mw00/project-maya) (MIT) serves DeepSeek
V4.1 Flash from one PC. The model's 543.6 billion routed-expert weights don't
fit in a consumer card, so Maya stores them at 2 to 3 bits each, keeps the
most-used experts in GPU memory, and reads the rest from RAM or disk. Before
the reset we ran Maya once and rebuilt its designs in the Rust stage, on one
rented RTX PRO 6000 (96 GB) with 176 GiB of RAM, not on an RTX 4080.

This chapter writes Maya's recipe down as a plan you can check. It answers
four questions from the checkpoint's headers alone: what each tensor becomes,
how many bytes the files take, where those bytes live on a given machine, and
how fast one token could be. Where the recipe's published numbers disagree
with that arithmetic, the plan says so.

The library is [`deepseek/recipe.coax`](../lib/deepseek/recipe.coax), an
`#authority` file because it admits or refuses a host. It loads chapter 100's
[`flash-plan.coax`](../lib/deepseek/flash-plan.coax) and reuses its manifest,
its host record, chapter 98's cache budget and chapter 33's admission.

```shell
coax/bin/coax run psionic/examples/deepseek-recipe.coax
coax/bin/coax run psionic/examples/deepseek-recipe-refuse.coax
```

The programs are [`deepseek-recipe.coax`](../examples/deepseek-recipe.coax)
and [`deepseek-recipe-refuse.coax`](../examples/deepseek-recipe-refuse.coax).

## What the recipe is

The source is our pre-reset notes on Maya (`mw00/project-maya` at
`451f9cb6a2`, read 2026-10-21, kept in history at
`docs/research/deepseek/2026-10-21-project-maya.md`) and Maya-S, its V4.1
quant (`peasantsmith/DeepSeek-V4.1-Flash-1017-Maya-GGUF`, MIT). Maya-S is not
a new format. It is a choice of ordinary GGUF block types, one per tensor
kind, plus a calibration recipe for the routed experts:

- Routed experts, gate and up projections (`w1`, `w3`): IQ2_XXS, and IQ2_S
  in the first and last four layers.
- Routed experts, down projection (`w2`): IQ3_XXS, and IQ3_S in the first
  and last four layers.
- Attention, shared experts, embeddings, the output head and Engram's value
  projection: Q8_0.
- Router, norms, hyper-connection mixing and attention sinks: F32. Indexer
  projections that are BF16 in the checkpoint stay BF16.
- Engram tables: Q6_K, from DeepSeek's FP8 tables.
- Calibration: 128 sequences of 2,048 tokens, an importance matrix for every
  expert, and GPTQ-style error feedback within each 256-weight block.

Maya's drafter (DSpark) keeps DeepSeek's FP4 experts and moves its FP8
weights to Q8_0, in a file of its own.

A GGUF block type stores a fixed number of values in a fixed number of
bytes. These are the sizes from ggml's block structs:

<!-- source: psionic/lib/deepseek/recipe.coax formats -->
```jai
// The formats a tensor can take. released keeps the checkpoint's bytes;
// dropped is a scale tensor folded into the new blocks. The rest are ggml
// block types: GGUF type number, values per block and bytes per block,
// from ggml's block structs (block_q8_0, block_q6_K, block_iq2_xxs,
// block_iq2_s, block_iq3_xxs, block_iq3_s).
MrFormat :: enum u8 {released;dropped;f32;q8_0;q6_k;iq2_xxs;iq2_s;iq3_xxs;iq3_s;}
MR_GGUF :: [9]int.[-1,-1,0,8,14,16,22,18,21];
MR_SPAN :: [9]int.[1,1,1,32,256,256,256,256,256];
MR_BLOCK :: [9]int.[1,0,4,34,210,66,82,98,110];
// Layers 0-3 and 36-39 keep their experts at a higher rate.
MR_OUTER :: 0xf00000000f;
```

IQ2_XXS is 66 bytes per 256 weights, 2.0625 bits each; IQ3_XXS is 98 bytes,
3.0625 bits. Q8_0 is 34 bytes per 32 values: a 2-byte scale and 32 one-byte
codes.

`mr_format` is the recipe as code. It reads only a tensor's name, its dtype
and whether its layer is one of the outer eight:

<!-- source: psionic/lib/deepseek/recipe.coax recipe -->
```jai
// Maya-S, tensor kind by tensor kind:
//   routed experts, gate and up (w1, w3): IQ2_XXS; IQ2_S in the outer layers
//   routed experts, down (w2):            IQ3_XXS; IQ3_S in the outer layers
//   the router (ffn.gate.weight):         F32
//   every other FP8 tensor, the embedding and the output head: Q8_0
//   the Engram tables:                    Q6_K
//   the drafter's routed experts:         as released (FP4)
//   norms, biases, mixing, sinks, BF16 compressor and indexer: as released
// An FP8 or FP4 tensor's UE8M0 scales are folded into its new blocks.
mr_format :: (name:string,dtype:int,outer:bool)->MrFormat {
    experts:=dv_has(name,".ffn.experts.");
    if experts && dv_starts(name,"mtp.") {return MrFormat.released;}
    if dtype==DV_E8M0 {return MrFormat.dropped;}
    if experts {
        if dv_has(name,".w2.") {return ifx outer then MrFormat.iq3_s else MrFormat.iq3_xxs;}
        return ifx outer then MrFormat.iq2_s else MrFormat.iq2_xxs;
    }
    if dtype==DV_F8 {return ifx dv_has(name,".engram.embed.") then MrFormat.q6_k else MrFormat.q8_0;}
    if name=="embed.weight" || name=="head.weight" {return MrFormat.q8_0;}
    if dv_has(name,".ffn.gate.weight") {return MrFormat.f32;}
    return MrFormat.released;
}
```

## Count the bytes by hand

Start with one routed expert in an inner layer. Its gate and up projections
are 2,304 rows of 5,120 weights; its down projection is 5,120 rows of 2,304.
Each projection has 2,304 × 5,120 = 11,796,480 weights, or 46,080 blocks of
256.

- Gate and up at IQ2_XXS: 2 × 46,080 × 66 = 6,082,560 bytes.
- Down at IQ3_XXS: 46,080 × 98 = 4,515,840 bytes.
- One inner expert: 10,598,400 bytes. This is the 10.6 MB expert Maya's own
  timings name.

In an outer layer the same expert is 46,080 × (82 + 82 + 110) = 12,625,920
bytes. Forty layers of 384 experts, 32 inner layers and 8 outer:

384 × (32 × 10,598,400 + 8 × 12,625,920) = 169,019,965,440 bytes.

The experts hold 3 × 11,796,480 × 15,360 = 543,581,798,400 weights, so the
recipe averages 169,019,965,440 × 8 / 543,581,798,400 = 2.487 bits per expert
weight.

The Engram tables have 384,006,168 and 384,016,682 rows of 256 FP8 values.
At Q6_K one row is one 210-byte block: 768,022,850 × 210 = 161,284,798,500
bytes.

The rest of the model file is the shared experts, the dense layers and the
embedding and head, at the formats above. Everything FP8 becomes Q8_0, at
34/32 bytes a value; its UE8M0 scales are folded into the new blocks. The
BF16 embedding and head (129,280 × 5,120 each) become Q8_0 too:
2 × 661,913,600 / 32 × 34 = 1,406,566,400 bytes, plus the 10,240-byte final
norm as released. The BF16 router, 40 × 384 ×
5,120 values, doubles to 314,572,800 bytes as F32.

## Rewrite the manifest

`mr_size` turns one tensor's bytes into its bytes in a new format. Chapter
100's manifest gives FP4 shapes in bytes, two weights to a byte, so the row
width doubles for those:

<!-- source: psionic/lib/deepseek/recipe.coax size -->
```jai
// One tensor's bytes in format f, or -1 when a row's width is not a whole
// number of the format's blocks.
mr_size :: (f:MrFormat,r:DvRow,bytes:int)->int {
    i:=cast(int)f;
    if f==MrFormat.released {return bytes;}
    if mr_width(r)%MR_SPAN[i]!=0 {return -1;}
    return bytes*2/mr_nibbles(r.dtype)/MR_SPAN[i]*MR_BLOCK[i];
}
```

`mr_requant` rewrites every manifest row with its new byte count, then hands
the rewritten rows to chapter 100's `dv_bytes`, so the split into routed
experts, shared experts, Engram, dense layers, embeddings, drafter and vision
tower is the same code that split the original checkpoint:

<!-- source: psionic/lib/deepseek/recipe.coax bytes -->
```jai
// The recipe's bytes. inner and outer are one routed expert's bytes in an
// inner and an outer layer; values counts the routed experts' weights;
// model is the file without the Engram tables, the drafter and the vision
// tower, files adds the Engram tables.
MrBytes :: struct {status:MrStatus;bad:int=-1;split:DvBytes;inner:int;outer:int;values:int;model:int;files:int;}
mr_requant :: (rows:[]DvRow,out:[]DvRow)->MrBytes #export_c {
    b:MrBytes;
    if out.count<rows.count {b.status=MrStatus.capacity;return b;}
    for i:0..<rows.count {
        r:=rows[i];
        if r.dtype<0 || r.dtype>DV_I8 {b.status=MrStatus.dtype;b.bad=i;return b;}
        tensors:=ifx r.layers==0 then r.count else r.count*dv_popcount(r.layers);outer:=r.count*dv_popcount(r.layers&MR_OUTER);
        one:=r.bytes/tensors;
        a:=mr_size(mr_format(r.name,r.dtype,false),r,one);z:=mr_size(mr_format(r.name,r.dtype,true),r,one);
        if rp_min(a,z)<0 || one*tensors!=r.bytes {b.status=MrStatus.block;b.bad=i;return b;}
        out[i]=r;out[i].bytes=(tensors-outer)*a+outer*z;
        if dv_starts(r.name,"layers.") && dv_has(r.name,".ffn.experts.") {
            b.inner+=a;b.outer+=z;if r.dtype==DV_I8 {b.values+=r.bytes*2;}
        }
    }
    b.split=dv_bytes(out[0..<rows.count]);s:=b.split;
    b.model=s.routed+s.shared+s.dense+s.embed;b.files=b.model+s.engram;
    return b;
}
```

A row whose width is not a whole number of the format's blocks, or whose
bytes don't divide evenly among its tensors, is refused with its index. The
model file is the routed experts, shared experts, dense layers and
embeddings: 178,203,530,688 bytes, Maya's 178.2 GB. It matches only with the
vision tower left out of the files, and only with the router at F32.

## The prompt skip

Maya skips layers 21–39 for most of a long prompt. The argument: only four
layers (2, 8, 14 and 20) keep state for every earlier position, in their
compressors and indexer keys. The four other indexer layers (24, 28, 32, 36)
have query weights only. So a layer after 20 sees earlier tokens only through
layer 20's state and its own sliding window of 128.

`mr_skip` reads that from the manifest rather than trusting it. It finds the
last layer with a compressor or an indexer key projection, then counts the
positions where that layer's output is still needed:

<!-- source: psionic/lib/deepseek/recipe.coax skip -->
```jai
// The prompt skip. A layer after the last one that keeps per-position state
// (a compressor or an indexer key projection) reads earlier tokens only
// through that layer's state and its own window, so the state layer's output
// is needed on the last (layers after it) * (window - 1) + 1 positions:
// the last token, and window - 1 more for each layer it passes through.
MrSkip :: struct {last:int;after:int;tokens:int;}
mr_skip :: (rows:[]DvRow,c:DvConfig)->MrSkip #export_c {
    s:=MrSkip.{last=-1};
    for r:rows {
        if dv_starts(r.name,"layers.") && (dv_has(r.name,".attn.compressor.") || dv_has(r.name,".indexer.wk.")) {
            for l:0..<64 {if ((r.layers>>u64(l))&1)==1 {s.last=rp_max(s.last,l);}}
        }
    }
    s.after=c.layers-1-s.last;s.tokens=s.after*(c.window-1)+1;
    return s;
}
```

By hand: the last token needs layer 39 at one position. Layer 39 reads layer
38 over its window, the last 128 positions, which reaches 127 positions
further back. Each of the 19 layers from 21 to 39 adds 127, so layer 20's
output is needed on 19 × 127 + 1 = 2,414 positions. The next token's
attention needs each layer's keys at the last 127 positions, which those
windows already cover. The draft's 2,414 holds; Maya's own description says
"about 2,700", which is safe but larger than needed.

This is a bound on which positions can matter. That skipping the work gives
the same logits as running every layer still needs the original weights and
chapter 100's reference.

## Place the experts

`mr_place` splits the bytes across one machine. The GPU holds what every
token reads: the shared experts, dense layers, embedding and head (9.18 GB),
chapter 98's packed cache and chapter 100's one-token step, through chapter
33's `rp_admit` with one outer expert as the stream slot. What's left of the
GPU holds the same number of experts in every layer, as Maya's placement
does. A "slot in every layer" costs 32 inner experts plus 8 outer ones,
440,156,160 bytes. RAM past the reserve holds more slots the same way; the
rest of each layer's 384 are read from disk. The files must fit on the free
disk.

<!-- source: psionic/lib/deepseek/recipe.coax place -->
```jai
// Where the experts go. The GPU holds the dense weights, the cache and a
// one-token step (chapter 100's budget, through chapter 33's admission with
// one outer expert as the stream slot); the rest of it holds the same number
// of experts in every layer. RAM, less the reserve, holds the same number
// again; the rest of each layer's 384 are read from disk. The files must fit
// on the free disk.
MrPlace :: struct {status:DvVerdict;gpu:RpPlan;vectors:int;stride:int;vram:int;ram:int;disk:int;}
mr_place :: (b:MrBytes,h:DvHost,context:int)->MrPlace #export_c {
    s:=b.split;
    if b.status!=MrStatus.ok || b.inner<1 || b.outer<1 || s.shared<1 || s.dense<1 || s.embed<1 || s.engram<1 {return MrPlace.{status=DvVerdict.bytes};}
    if h.gpu<1 || h.ram<1 || h.reserve<0 || h.disk_free<0 || h.gpu_bw<1000000 || h.link_bw<1000000 || h.disk_bw<1000000 {return MrPlace.{status=DvVerdict.bytes};}
    kv:=dk_flash_cache_budget(context,1,0,h.gpu,h.gpu);if kv.status!=DkStatus.ok {return MrPlace.{status=DvVerdict.cache};}
    // One slot in every layer: 32 inner experts and 8 outer ones.
    p:=MrPlace.{vectors=s.shared+s.dense+s.embed,stride=32*b.inner+8*b.outer};
    w:=RpWeights.{total=b.model,vectors=p.vectors,slot=b.outer,embedding=s.embed,expert=b.outer};
    p.gpu=rp_admit(w,kv.cache,kv.step,true,h.gpu);if p.gpu.status!=RpStatus.ok {p.status=DvVerdict.gpu;return p;}
    // A GPU that holds the whole model holds exactly 384 slots a layer.
    p.vram=(p.gpu.resident-p.vectors)/p.stride;
    usable:=h.ram-h.reserve;if usable<p.stride {p.status=DvVerdict.ram;return p;}
    p.ram=rp_min(384-p.vram,usable/p.stride);p.disk=384-p.vram-p.ram;
    if h.disk_free<b.files {p.status=DvVerdict.disk;}
    return p;
}
```

On this host (chapter 100's record: an RTX 4080 with 17,171,480,576 bytes,
134.8 GB of RAM with 8 GiB reserved, 37.6 GB of free disk):

- GPU: 17,171,480,576 − 6,348,800 (cache) − 28,928 (step) − 2 × 12,625,920
  (stream slots) = 17,139,851,008 bytes resident. Less 9,183,565,248 bytes of
  dense weights leaves 7,956,285,760, which is 18 slots per layer (720
  experts, 4.7%).
- RAM: 126,238,507,008 / 440,156,160 = 286 slots per layer.
- Disk: 384 − 18 − 286 = 80 experts per layer.
- Verdict: disk. 339,488,329,188 bytes of files against 37,567,934,464 free.

The reference machine, as the old run recorded it, comes out as 197 GPU
slots per layer (7,880 experts, 51%), 187 in RAM and none on disk: admitted.
Maya placed 7,393 experts on that GPU; it also keeps room for prefill chunks
of 32,768 tokens, which this plan's one-token step does not.

<!-- source: psionic/lib/deepseek/recipe.coax reference -->
```jai
// The recipe's reference machine, as the old run recorded it: one RTX PRO
// 6000 (96 GB, 1,792 GB/s), 176 GiB of RAM with 8 GiB kept for the system,
// a 900 GB disk. Link and disk rates are Maya's own start-up timings there:
// one 10,598,400-byte expert in 0.34 ms over PCIe and 5.3 ms from disk.
mr_reference_host :: ()->DvHost #export_c {
    return DvHost.{gpu=96000000000,ram=188978561024,reserve=8589934592,disk_free=900000000000,gpu_bw=1792000000000,link_bw=31171764705,disk_bw=1999698113};
}
```

## The cost of one token

A token reads the dense weights once and 240 experts (6 in each of 40
layers). Each expert read comes from the GPU, from RAM over the PCIe link,
or from disk, and each tier's bytes take their time at the host's rate:

<!-- source: psionic/lib/deepseek/recipe.coax rate -->
```jai
// A replay's expert reads, by tier and by inner ([0]) or outer ([1]) layer.
// Every token also reads the dense weights from the GPU. Each tier's bytes
// take their time at the host's rate, rounded up to a microsecond.
MrReads :: struct {tokens:int;gpu:[2]int;link:[2]int;disk:[2]int;}
MrRate :: struct {status:DvVerdict;bytes_gpu:int;bytes_link:int;bytes_disk:int;micros:int;milli_tokens:int;}
mr_us :: (bytes:int,rate:int)->int {per:=rate/1000000;return (bytes+per-1)/per;}
mr_rate :: (b:MrBytes,h:DvHost,t:MrReads)->MrRate #export_c {
    s:=b.split;
    if t.tokens<1 || b.inner<1 || b.outer<1 || h.gpu_bw<1000000 || h.link_bw<1000000 || h.disk_bw<1000000 {return MrRate.{status=DvVerdict.bytes};}
    for i:0..<2 {if t.gpu[i]<0 || t.link[i]<0 || t.disk[i]<0 {return MrRate.{status=DvVerdict.bytes};}}
    r:MrRate;
    r.bytes_gpu=t.tokens*(s.shared+s.dense+s.embed)+t.gpu[0]*b.inner+t.gpu[1]*b.outer;
    r.bytes_link=t.link[0]*b.inner+t.link[1]*b.outer;r.bytes_disk=t.disk[0]*b.inner+t.disk[1]*b.outer;
    total:=mr_us(r.bytes_gpu,h.gpu_bw)+mr_us(r.bytes_link,h.link_bw)+mr_us(r.bytes_disk,h.disk_bw);
    r.micros=(total+t.tokens-1)/t.tokens;r.milli_tokens=t.tokens*1000000000/total;
    return r;
}
```

How many reads land in each tier depends on the routing. Two models bound
it. Uniform routing, chapter 100's assumption, spreads reads as the tiers
hold experts: on this host 18/384 from the GPU, 286/384 from RAM and 80/384
from disk. By hand for one token, inner and outer layers mixed:
240 × 18/384 × 11,003,904 average bytes is about 0.12 GB on the GPU beside
the 9.18 GB of dense weights (13.0 ms at 716.8 GB/s), 1.97 GB over the link
(78.7 ms at 25 GB/s) and 0.55 GB from disk (85.3 ms at 6.447 GB/s). That is
about 177 ms, or 5.65 tokens a second.

The other model replays a routing trace through chapter 191's cache. Real
V4.1 traces need the weights, so the example builds a stand-in from chapter
191's synthetic router. That router takes at most 256 experts, so each
layer's 384 are two halves of 192 that each pick their own top 3:

<!-- source: psionic/examples/deepseek-recipe.coax replay -->
```jai
// Routing traces shaped like V4.1's: 40 layers of 384 experts, 6 a token.
// Chapter 85's router takes at most 256 experts, so each layer's 384 are two
// halves of 192, and each half routes its own top 3 with its own gate (seeds
// 1 and 2). Prompt 1 records the profile; prompt 2 is measured after 128
// warm tokens. Each layer is its own cache (Maya's placement gives every
// layer the same slots): the GPU tier replays with the plan's GPU slots, the
// GPU and RAM tiers together with both; their misses are read from disk.
rc_replay :: (p:MrPlace,a:[]int,b:[]int,pa:[]int,pb:[]int,policy:XsPolicy)->MrReads {
    r:=MrReads.{tokens=RC_TOKENS-RC_WARM};local:[RC_TOKENS*6]int;profile:[384]int;work:[1536]int;
    counted:=(RC_TOKENS-RC_WARM)*6;
    for l:0..<40 {
        for k:0..<384 {profile[k]=0;}
        for t:0..<RC_TOKENS {for j:0..<3 {
            at:=(t*40+l)*3+j;
            local[t*6+j]=a[at]-l*RC_HALF;local[t*6+3+j]=b[at]-l*RC_HALF+RC_HALF;
            profile[pa[at]-l*RC_HALF]+=1;profile[pb[at]-l*RC_HALF+RC_HALF]+=1;
        }}
        gpu:=0;disk:=0;
        if p.vram>0 {gpu=xs_replay(policy,local[..],6,RC_WARM*6,p.vram,384,profile[..],work[..]).hits;}
        if p.vram+p.ram<384 {disk=xs_replay(policy,local[..],6,RC_WARM*6,p.vram+p.ram,384,profile[..],work[..]).misses;}
        side:=ifx l<4 || l>=36 then 1 else 0;
        r.gpu[side]+=gpu;r.disk[side]+=disk;r.link[side]+=counted-gpu-disk;
    }
    return r;
}
```

The replay counts hits; `mr_rate` prices them. The synthetic trace reuses
far fewer experts than real routing does, so its hit rates are an optimistic
bound, not a prediction.

## Run it

<!-- source: psionic/examples/deepseek-recipe.coax example -->
```jai
main :: () {
    rows:=DV_MANIFEST;out:[DV_EXPECTED]DvRow;b:=mr_requant(rows[..],out[..]);s:=b.split;native:=dv_bytes(rows[..]);
    names:=[9]string.["as released","folded","F32","Q8_0","Q6_K","IQ2_XXS","IQ2_S","IQ3_XXS","IQ3_S"];
    print("Maya-S from the pinned headers (status %):\n",b.status);
    for f:[5]MrFormat.[MrFormat.iq2_xxs,MrFormat.iq2_s,MrFormat.iq3_xxs,MrFormat.iq3_s,MrFormat.q6_k] {
        i:=cast(int)f;print("  % (GGUF type %): % bytes per % values\n",names[i],MR_GGUF[i],MR_BLOCK[i],MR_SPAN[i]);
    }
    print("  routed experts % bytes (",s.routed);gb(s.routed);print(" GB), % weights at ",b.values);milli(s.routed*8000/b.values);print(" bits\n");
    print("  one expert: % bytes in layers 4-35, % in layers 0-3 and 36-39\n",b.inner,b.outer);
    print("  shared experts %, dense layers %, embedding and head %\n",s.shared,s.dense,s.embed);
    print("  model file % (",b.model);gb(b.model);print(" GB), Engram % (",s.engram);gb(s.engram);print(" GB), together % (",b.files);gb(b.files);print(" GB)\n");
    print("  drafter % (",s.draft);gb(s.draft);print(" GB, its own file), vision tower % (not in the files)\n",s.vision);

    print("published figures against the arithmetic:\n");
    tenth:=(b.model+50000000)/100000000;
    claim("Maya-S model file 178.2 GB",tenth==1782);
    claim("Maya-S Engram 161.3 GB",(s.engram+50000000)/100000000==1613);
    claim("Maya-S 339.5 GB in all",(b.files+50000000)/100000000==3395);
    claim("Maya's drafter 7.97 GB",(s.draft+5000000)/10000000==797);
    claim("one expert 10.6 MB",(b.inner+50000)/100000==106);
    at26:=b.values*26/80;
    print("  the draft's routed experts at about 2.6 bits would be % bytes; with the rest, % (",at26,at26+b.model-s.routed);gb(at26+b.model-s.routed);print(" GB)\n");
    claim("routed experts at about 2.6 bits a weight make 178 GB",(at26+50000000)/1000000000==178);
    claim("native: 289 GB of routed experts and 203 GB of Engram",(native.routed+500000000)/1000000000==289 && (native.engram+500000000)/1000000000==203);
    p:=mr_place(b,dv_this_host(),4096);need:=(384-p.vram)*p.stride+dv_this_host().reserve;
    print("  beside a 16 GB card the quant's other experts and the reserve need % bytes of RAM (",need);gb(need);print(" GB)\n");
    claim("the 178 GB quant fits only beside about 192 GB of RAM",(need+5000000000)/10000000000==19);
    q:=mr_place(b,mr_reference_host(),4096);
    print("  reference machine: % of 15360 experts on the GPU in this plan; Maya placed 7393\n",40*q.vram);

    k:=mr_skip(rows[..],dv_flash_config());
    print("prompt skip: the last state layer is %, % layers after it, % prompt positions need them\n",k.last,k.after,k.tokens);

    print("chapter 51's stand-in cards (declared: KL + 2 SE <= 0.01, agreement - 2 SE >= 0.9):\n");
    for profile:[4]int.[0,6,2,3] {
        c:=kq_standin_card(profile);
        print("  %: agreement % (low %), KL % (high %), promoted %\n",KQ_PROFILE_NAMES[profile],c.agreement,c.agreement_low,c.kl,c.kl_high,c.promoted);
    }
    for f:[4]MrFormat.[MrFormat.iq2_xxs,MrFormat.iq2_s,MrFormat.iq3_xxs,MrFormat.iq3_s] {
        i:=cast(int)f;print("  %: %\n",names[i],kq_plan(MR_GGUF[i],1,256).status);
    }

    a:[RC_TOKENS*40*3]int;c:[RC_TOKENS*40*3]int;pa:[RC_TOKENS*40*3]int;pc:[RC_TOKENS*40*3]int;gate:[40*RC_HALF*XS_WIDTH]f32;f:[RC_HALF]f64;
    t1:=xs_trace(1,2,RC_TOKENS,40,RC_HALF,3,a[..],gate[..],f[..]);t2:=xs_trace(1,1,RC_TOKENS,40,RC_HALF,3,pa[..],gate[..],f[..]);
    t3:=xs_trace(2,2,RC_TOKENS,40,RC_HALF,3,c[..],gate[..],f[..]);t4:=xs_trace(2,1,RC_TOKENS,40,RC_HALF,3,pc[..],gate[..],f[..]);
    print("traces % % % %: % tokens, % measured\n",t1.status,t2.status,t3.status,t4.status,RC_TOKENS,RC_TOKENS-RC_WARM);
    rc_host("this host (RTX 4080)",b,dv_this_host(),a[..],c[..],pa[..],pc[..]);
    rc_host("reference (RTX PRO 6000)",b,mr_reference_host(),a[..],c[..],pa[..],pc[..]);
    n:=dv_plan(native,dv_this_host(),4096);print("chapter 100, native weights on this host: tokens/s ");milli(n.milli_tokens);print("\n");
}
```

```text
Maya-S from the pinned headers (status ok):
  IQ2_XXS (GGUF type 16): 66 bytes per 256 values
  IQ2_S (GGUF type 22): 82 bytes per 256 values
  IQ3_XXS (GGUF type 18): 98 bytes per 256 values
  IQ3_S (GGUF type 21): 110 bytes per 256 values
  Q6_K (GGUF type 14): 210 bytes per 256 values
  routed experts 169019965440 bytes (169.0 GB), 543581798400 weights at 2.487 bits
  one expert: 10598400 bytes in layers 4-35, 12625920 in layers 0-3 and 36-39
  shared experts 1504051200, dense layers 6272937408, embedding and head 1406576640
  model file 178203530688 (178.2 GB), Engram 161284798500 (161.3 GB), together 339488329188 (339.5 GB)
  drafter 7971546504 (8.0 GB, its own file), vision tower 970536960 (not in the files)
published figures against the arithmetic:
  Maya-S model file 178.2 GB -> holds
  Maya-S Engram 161.3 GB -> holds
  Maya-S 339.5 GB in all -> holds
  Maya's drafter 7.97 GB -> holds
  one expert 10.6 MB -> holds
  the draft's routed experts at about 2.6 bits would be 176664084480 bytes; with the rest, 185847649728 (185.8 GB)
  routed experts at about 2.6 bits a weight make 178 GB -> does not hold
  native: 289 GB of routed experts and 203 GB of Engram -> holds
  beside a 16 GB card the quant's other experts and the reserve need 169687089152 bytes of RAM (169.7 GB)
  the 178 GB quant fits only beside about 192 GB of RAM -> does not hold
  reference machine: 7880 of 15360 experts on the GPU in this plan; Maya placed 7393
prompt skip: the last state layer is 20, 19 layers after it, 2414 prompt positions need them
chapter 51's stand-in cards (declared: KL + 2 SE <= 0.01, agreement - 2 SE >= 0.9):
  Q8_0: agreement 1.0 (low 1.0), KL 0.00022960896719454973 (high 0.00028397003336496973), promoted true
  Q6_K: agreement 0.96875 (low 0.9252518409153905), KL 0.002086438434421452 (high 0.0026257607518881446), promoted true
  Q2_K: agreement 0.578125 (low 0.454660307583757), KL 0.4218520012100292 (high 0.5204050973854987), promoted false
  Q3_K: agreement 0.8125 (low 0.714921906275025), KL 0.12899541365876996 (high 0.1644972986082705), promoted false
  IQ2_XXS: kind
  IQ2_S: kind
  IQ3_XXS: kind
  IQ3_S: kind
traces ok ok ok ok: 384 tokens, 256 measured
this host (RTX 4080): disk: the model and Engram files do not fit on the free disk
  each layer: 18 experts on the GPU, 286 in RAM, 80 from disk (720 of 15360 on the GPU)
  uniform routing: 177004 us a token, tokens/s 5.649
  heat, profiled: of 61440 reads, 31053 GPU (50 percent), 28839 RAM, 1548 disk; 74636 us a token, tokens/s 13.398
  lru: of 61440 reads, 38905 GPU (63 percent), 20987 RAM, 1548 disk; 61601 us a token, tokens/s 16.233
reference (RTX PRO 6000): ok
  each layer: 197 experts on the GPU, 187 in RAM, 0 from disk (7880 of 15360 on the GPU)
  uniform routing: 47140 us a token, tokens/s 21.213
  heat, profiled: of 61440 reads, 59892 GPU (97 percent), 1548 RAM, 0 disk; 8698 us a token, tokens/s 114.974
  lru: of 61440 reads, 59892 GPU (97 percent), 1548 RAM, 0 disk; 8698 us a token, tokens/s 114.974
chapter 100, native weights on this host: tokens/s 2.126
```

What the run shows:

- **The files reproduce Maya-S exactly.** Model 178.2 GB, Engram 161.3 GB,
  339.5 GB in all, drafter 7.97 GB, one inner expert 10.6 MB.
- **"Routed experts at about 2.6 bits a weight (178 GB)" does not hold.**
  The experts are 2.487 bits and 169.0 GB. At 2.6 bits they would be
  176.7 GB, and the model file 185.8 GB. The 178 GB is the whole model file
  without Engram, not the experts. The pre-reset `consumer-q` build may have
  differed from Maya-S; its files are gone, so the plan uses Maya-S's
  published recipe and flags the draft's figure.
- **"The 178 GB quant fits only beside about 192 GB of RAM" overstates.**
  With 18 slots per layer on a 16 GB card, the other 366 per layer take
  161.1 GB, and with the 8 GiB reserve 169.7 GB of RAM holds them all.
- **The 16 GB card is refused on disk, not on memory.** This host's GPU and
  RAM admit the plan; its 37.6 GB of free disk cannot hold the files.
- **The modeled speed brackets, it does not predict.** On this host, uniform
  routing gives 5.649 tokens a second and the synthetic trace 13.4 to 16.2.
  On the reference machine, uniform routing gives 21.2 tokens a second and
  the synthetic trace 115. Maya measured 35.3 there, and the old Rust stage
  24.5 with this kind of quant. The synthetic trace's 97% GPU hit rate is far
  above what real routing gave on that machine (Maya's CPU lane served 16–26%
  of expert reads with 48% of experts on the GPU), so the uniform figure is
  the one to plan with until real traces exist. Maya also computes RAM
  experts on CPU threads instead of moving them over PCIe, which this plan
  does not model.
- **Chapter 51's stand-in cards.** Q8_0 and Q6_K, the recipe's formats for
  the dense layers and the Engram tables, pass chapter 50's declared bound on
  the stand-in. Psionic has no IQ2 or IQ3 codec yet (`kind`), so the expert
  formats have no card. Q2_K and Q3_K, plain rounding at nearby rates, fail
  the bound on the stand-in (agreement 0.578 and 0.813), which is why Maya's
  recipe adds per-expert importance and GPTQ error feedback. Whether that
  closes the gap is the quality card this chapter cannot run yet.

## What it refuses

<!-- source: psionic/examples/deepseek-recipe-refuse.coax refuse -->
```jai
main :: () {
    statuses:=[4]string.["ok","dtype","block","capacity"];out:[DV_EXPECTED]DvRow;
    // Layer experts' down projections 2,302 weights wide: not whole 256-value blocks.
    rows:=DV_MANIFEST;at:=dv_find(rows[..],"layers.#.ffn.experts.*.w2.weight",dv_span(40));
    rows[at].shape[1]=1151;b:=mr_requant(rows[..],out[..]);
    print("w2 rows of 2,302 weights: % at manifest row % (%)\n",statuses[cast(int)b.status],b.bad,rows[b.bad].name);
    rows=DV_MANIFEST;rows[at].bytes+=1;b=mr_requant(rows[..],out[..]);
    print("w2 one byte longer: % at manifest row %\n",statuses[cast(int)b.status],b.bad);
    rows=DV_MANIFEST;rows[5].dtype=7;b=mr_requant(rows[..],out[..]);
    print("dtype code 7: % at manifest row %\n",statuses[cast(int)b.status],b.bad);
    rows=DV_MANIFEST;b=mr_requant(rows[..],out[0..<100]);print("room for 100 rows: %\n",statuses[cast(int)b.status]);
    b=mr_requant(rows[..],out[..]);h:=dv_this_host();
    print("this host: %\n",mr_verdict_name(mr_place(b,h,4096).status));
    h.gpu=8000000000;print("an 8 GB GPU: %\n",mr_verdict_name(mr_place(b,h,4096).status));
    h=dv_this_host();h.ram=h.reserve+400000000;print("400 MB of RAM past the reserve: %\n",mr_verdict_name(mr_place(b,h,4096).status));
    h=dv_this_host();print("a 2M-token context: %\n",mr_verdict_name(mr_place(b,h,2000000).status));
    h.disk_bw=999999;print("a disk rate under 1 MB/s: %\n",mr_verdict_name(mr_place(b,h,4096).status));
    print("a replay of 0 tokens: %\n",mr_verdict_name(mr_rate(b,dv_this_host(),MrReads.{}).status));
    h=dv_this_host();h.disk_free=b.files;print("this host with % bytes free: %\n",b.files,mr_verdict_name(mr_place(b,h,4096).status));
}
```

```text
w2 rows of 2,302 weights: block at manifest row 42 (layers.#.ffn.experts.*.w2.weight)
w2 one byte longer: block at manifest row 42
dtype code 7: dtype at manifest row 5
room for 100 rows: capacity
this host: disk: the model and Engram files do not fit on the free disk
an 8 GB GPU: gpu: the dense weights, cache, step and two expert slots exceed the GPU
400 MB of RAM past the reserve: ram: RAM past the reserve cannot hold one expert in every layer
a 2M-token context: cache: the KV cache plan refused the context
a disk rate under 1 MB/s: bytes: a byte count is not positive, the recipe refused the manifest, or a host rate is under 1 MB/s
a replay of 0 tokens: bytes: a byte count is not positive, the recipe refused the manifest, or a host rate is under 1 MB/s
this host with 339488329188 bytes free: ok
```

A row the recipe can't convert names its manifest row. A host that can't run
the plan gets the tier that fails: the GPU, RAM, the cache's context, a rate,
or the disk. The same host with enough free disk is admitted.

## Checks

`psionic/lib/deepseek/recipe.coax` checks itself against independent
references in
[`deepseek-recipe-checks.coax`](../tests/deepseek-recipe-checks.coax):

- `mrc_total` recounts the files tensor by tensor and layer by layer from the
  shapes, with its own format table, and must equal `mr_requant`'s model,
  Engram, drafter and vision totals.
- `mrc_needed` marks the needed positions of layer 20's output one by one,
  layer by layer, and must count 2,414 for 19 layers and a window of 128, and
  `after × (window − 1) + 1` for every small case.
- The placement, token time and refusals are checked against the hand
  values in this chapter.

`coax mutate psionic/lib/deepseek/recipe.coax --root-only` kills all 265
mutants. `psionic/tests/check_deepseek_recipe.sh` runs the checks and both
transcripts in debug and release.

## What remains

Running the recipe needs the original weights (501 GB; this host has 37.6 GB
free):

- **IQ2_XXS, IQ2_S, IQ3_XXS and IQ3_S codecs** in Psionic, with their lattice
  tables, and stand-in quality cards for them by chapter 50's method.
- **Importance-weighted GPTQ** from calibration activations of the original
  model, and the requantized files with their digests.
- **The quality card against chapter 100's reference**: top-1 agreement, mean
  KL and G2 answers, with the host, cache size and miss lane named. The old
  run's 91% top-1 and mean KL 0.117 stay targets. The `consumer-q` profile
  is never qualified and never joins a qualified group.
- **The prompt skip's same-logits check** on prompts longer than 2,414
  tokens.
- **Real routing traces** in place of the synthetic stand-in, a per-machine
  calibration of the link and disk rates, and a measured decode rate on the
  16 GB card or this chapter's disk refusal.

## Dependencies

- [Chapter 100](ch100-execute-deepseek-v41-flash-on-one-machine.md): the
  header manifest, the host record, the byte split and the reference.
- [Chapter 191](ch191-experts-across-machines-and-from-disk.md): the
  synthetic routing trace and the cache replay.
- [Chapter 96](ch096-hot-experts-in-fast-memory.md): the decayed-heat rule
  the replay uses.
- [Chapter 98](ch098-pack-the-cache-exactly.md) and
  [chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the cache budget and GPU
  admission.
- [Chapter 50](ch050-quantized-weights-and-caches.md) and
  [chapter 51](ch051-k-quants-and-lattice-quants.md): the quality card and the
  stand-in, and the rule that a quantized profile never inherits the
  original's evidence.
- [Chapter 94](ch094-memory-by-lookup.md) and
  [chapter 97](ch097-prefill-layer-by-layer.md): Engram rows and layer-major
  prefill, which a real run uses.

## Sources

- [Chapter plan](PLAN.md)
- [DeepSeek target notes](../docs/targets/deepseek.md), "One card, the way
  Maya does it"
- Project Maya notes, `git show 0b401561a0^:docs/research/deepseek/2026-10-21-project-maya.md`
- ggml's block structs (`ggml-common.h`), for the block sizes
