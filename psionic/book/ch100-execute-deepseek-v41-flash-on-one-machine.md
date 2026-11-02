# 100. Execute DeepSeek V4.1 Flash on one machine

> **Status: partly implemented.** Three parts run. The pinned checkpoint's
> 48 shard headers, read without the payload, match the shapes the pinned
> configuration implies, row for row. A byte plan for this host (an RTX 4080,
> 135 GB of RAM, one NVMe disk) admits the GPU tier with streamed experts and
> models 2.13 tokens per second, then refuses the host: the 501.4 GB text
> checkpoint does not fit on its 37.6 GB of free disk. And the whole 40-layer
> stack, composed from the pieces of chapters 91–96 and 98, runs on small synthetic
> weights and matches an independent composed oracle bit for bit. The real
> model has not run: no original weights were downloaded, nothing ran on a
> GPU, and chapter 17's qualification against a reference capture, measured
> time to first token, decode speed and peak memory all remain. The list at
> the end of this chapter names every missing piece.

DeepSeek V4.1 Flash is `deepseek-ai/DeepSeek-V4.1-Flash-1017` at revision
`1bae0e10…`: 48 shards and 510.3 GB. The chapters before this one built its
pieces one at a time: routers ([chapter 91](ch091-routers-that-balance-themselves.md)),
compressed and shared attention ([chapter 92](ch092-compressed-and-shared-attention.md)),
four residual streams ([chapter 93](ch093-many-residual-streams.md)), Engram
lookups ([chapter 94](ch094-memory-by-lookup.md)), FP4 and FP8 experts
([chapter 95](ch095-v41s-expert-weights.md)), the expert cache
([chapter 96](ch096-hot-experts-in-fast-memory.md)), layer-major prefill
([chapter 97](ch097-prefill-layer-by-layer.md)) and the packed caches
([chapter 98](ch098-pack-the-cache-exactly.md)). This chapter puts them
together and asks what one machine needs to run the original.

## Read the checkpoint without downloading it

A safetensors shard starts with an 8-byte length and a JSON header that names
every tensor's dtype, shape and byte range. `psionic/tests/deepseek_v41_manifest.py`
fetches `config.json`, the index and, by HTTP range requests, only those
headers: about 10.7 MB for all 48 shards. It pins the sha256 of the config,
the index and the concatenated headers, checks that each header's tensors
tile its shard exactly up to the shard's size, and checks that the index
names the same shard for every tensor. It writes 116 rows to
`psionic/fixtures/deepseek/v41-manifest.coax`: the 384 experts of a layer
collapse into one row per kind, and per-layer tensors with one dtype and
shape collapse into one row with a set of layers. The Hub lists the
repository as MIT and ungated. Whether we may redistribute the weights or
packages built from them is still an owner question; nothing here does
either.

`dv_expected` derives the 116 rows from the pinned configuration alone, and
`dv_check_manifest` compares them field by field with the header rows. A few
shapes are not spelled out in the config and are derived from it: the Engram
projection is [(4 + 1) × 5,120, 3 × 8 × 256] = [25,600, 6,144], and the
grouped output projection `wo_a` is [8 × 1,024, 64 × 512 / 8] = [8,192, 4,096].
The headers also hold a vision tower, an aligner and three image tokens that
`text_config` does not describe; they are counted separately.

The bytes split as follows:

| What | Bytes |
| --- | --- |
| Routed experts, 40 layers × 384 | 288,777,830,400 |
| Engram tables, layers 1 and 14 | 202,758,032,400 |
| Draft (MTP) layers | 7,932,874,632 |
| Dense layer weights | 5,782,123,968 |
| Embeddings, head and final norm | 2,647,664,640 |
| Shared experts | 1,416,960,000 |
| Vision, aligner and image tokens | 970,536,960 |
| Total, 96,085 tensors | 510,286,023,000 |

One routed expert is three FP4 matrices of 2,304 × 5,120 values stored two
to a byte (5,898,240 bytes each) and three UE8M0 scale grids of one byte per
32 values (368,640 bytes each): 3 × 5,898,240 + 3 × 368,640 = 18,800,640
bytes. Times 15,360 experts that is 288,777,830,400, the routed row above.

## Plan the bytes on this host

A decode token without drafting needs the text weights: everything except the
draft layers and the vision tower, 501,382,611,408 bytes. The plan puts them
in three tiers.

- **GPU.** The dense weights, the shared experts and the embeddings are read
  every token, so they never stream: 9,846,748,608 bytes. Chapter 98's cache
  budget for a 4,096-token context is 40 × 128 × 528 bytes of window rows plus
  (3 × 2,048 + 4,096) × 356 bytes of compressed rows and index keys: 6,348,800
  bytes, and 28,928 bytes of step scratch. Chapter 33's `rp_admit`, with one
  routed expert as the stream slot, keeps two slots free and leaves
  17,171,480,576 − 6,377,728 − 2 × 18,800,640 = 17,127,501,568 resident bytes:
  (17,127,501,568 − 9,846,748,608) / 18,800,640 = 387 experts on the GPU.
- **RAM.** Chapter 96's expert cache holds the same number of slots in every
  layer. With 8 GiB kept for the system, 126,238,507,008 / 18,800,640 / 40 =
  167 slots per layer, 6,680 experts, and `dc_config` admits that cache.
- **Disk.** The other 15,360 − 387 − 6,680 = 8,293 experts and both Engram
  tables stay mapped from the NVMe disk. A token reads 24 Engram rows per
  Engram layer, a few kilobytes, which the time model leaves out.

A token routes 6 experts in each of 40 layers: 240 expert reads. If routing
were uniform, each held expert would account for 240 × 18,800,640 / 15,360 =
293,760 bytes per token. The token then reads 9,846,748,608 + 387 × 293,760 =
9,960,433,728 bytes on the GPU, 6,680 × 293,760 = 1,962,316,800 bytes over
PCIe and 8,293 × 293,760 = 2,436,151,680 bytes from disk. At 716.8 GB/s on
the GPU, a modeled 25 GB/s over PCIe 4.0 x16 and a measured 6.447 GB/s from
the disk (4 GiB of 16 MiB direct reads took 0.666 s), the three take 13,895,
78,492 and 377,873 microseconds: 470,260 microseconds, or 2.13 tokens per
second. The disk dominates. Routing is not uniform: the pre-reset single-box
stage saw 89% hits with 30% of the experts on its GPU, so a recorded routing
profile would move reads off the disk. That needs the real router running.

The plan then compares the text checkpoint with the free disk: 37,567,934,464
bytes against 501,382,611,408. It refuses this host.

<!-- source: psionic/lib/deepseek/flash-plan.coax plan -->
```jai
dv_plan :: (b:DvBytes,h:DvHost,context:int)->DvPlan #export_c {
    if b.routed<DV_EXPERTS || b.routed%DV_EXPERTS!=0 || b.shared<1 || b.engram<1 || b.dense<1 || b.embed<1 {return DvPlan.{status=DvVerdict.bytes};}
    if h.gpu<1 || h.ram<1 || h.reserve<0 || h.disk_free<0 || h.gpu_bw<1 || h.link_bw<1 || h.disk_bw<1 {return DvPlan.{status=DvVerdict.bytes};}
    kv:=dk_flash_cache_budget(context,1,0,h.gpu,h.gpu);if kv.status!=DkStatus.ok {return DvPlan.{status=DvVerdict.cache};}
    p:=DvPlan.{expert=b.routed/DV_EXPERTS,text=b.routed+b.shared+b.engram+b.dense+b.embed};
    vectors:=b.shared+b.dense+b.embed;
    // Dense weights, the shared expert and the embeddings never stream; one
    // routed expert is the stream slot; Engram rows are read from disk.
    w:=RpWeights.{total=p.text,vectors=vectors,slot=p.expert,embedding=b.embed,expert=p.expert};
    p.gpu=rp_admit(w,kv.cache,kv.step,true,h.gpu);if p.gpu.status!=RpStatus.ok {p.status=DvVerdict.gpu;return p;}
    p.vram=rp_min(DV_EXPERTS,(p.gpu.resident-vectors)/p.expert);
    usable:=h.ram-h.reserve;if usable<40*p.expert {p.status=DvVerdict.ram;return p;}
    slots:=rp_min(384,(usable/p.expert)/40);
    cache:=DcConfig.{layers=40,experts=384,slots=slots,stride=p.expert,budget=usable,identity="deepseek-v4.1-flash-1017"};
    if dc_config(cache)!=DsStatus.ok {p.status=DvVerdict.ram;return p;}
    p.ram=rp_min(DV_EXPERTS-p.vram,40*slots);p.disk=DV_EXPERTS-p.vram-p.ram;
    // Uniform routing: a token's 240 expert reads split as the tiers hold experts.
    p.bytes_gpu=vectors+DV_ACTIVE*p.expert/DV_EXPERTS*p.vram;
    p.bytes_link=DV_ACTIVE*p.expert/DV_EXPERTS*p.ram;p.bytes_disk=DV_ACTIVE*p.expert/DV_EXPERTS*p.disk;
    // At least one microsecond, so a tiny model never divides by zero.
    p.micros=rp_max(1,p.bytes_gpu*1000000/h.gpu_bw+p.bytes_link*1000000/h.link_bw+p.bytes_disk*1000000/h.disk_bw);
    p.milli_tokens=1000000000/p.micros;
    if h.disk_free<p.text {p.status=DvVerdict.disk;}
    return p;
}
```

## Compose the whole stack on small weights

`df_forward` in `psionic/lib/deepseek/flash.coax` runs every decode step
through all 40 layers with the original layer structure and every component
from the earlier chapters; only the widths are small. Each layer:

1. adds the Engram value at layers 1 and 14: `eg_hash` (chapter 94) hashes the
   token history into 24 row ids per table, `eg_decode` turns the FP8 rows
   into BF16, one FP8 projection gives four keys and a value, and each
   residual stream adds the value through its own gated dot product;
2. collapses the four streams with `dh_prepare` (chapter 93), normalizes, and
   runs attention: low-rank queries, one shared KV head written to its packed
   FP8 window ring (`da_store`, chapter 98), and, on layers with compression,
   chapter 92's pooling (`da_pool`, gated at ratio 2), FP4 compressed rows and
   index keys, the indexer (`da_index`), layer 20's candidate blocks
   (`da_candidates`) and the top positions (`da_select`); `da_attend` reads the
   window and the selected rows with the sink, the rotation is removed, and
   the grouped low-rank output projects back;
3. expands with `dh_finish`, collapses again, normalizes and routes with
   `dr_route` (chapter 91: square-root-softplus scores, a bias that only
   selects, scale 1.5), then runs two FP4 routed experts and the FP8 shared
   expert through `de_mixture` (chapter 95), clamped at 10.

Which layers produce or reuse compressed rows, index keys and candidates comes
from `da_flash_links`, chapter 92's copy of the original layer map: ratio 0 at
layers 0–1, ratio 2 at 2–19, ratio 1 at 20–39, KV owners 2, 8, 14 and 20,
index owners 2, 8, 14, 20, 24, 28, 32 and 36. Every dense projection is an
FP8 matrix with 32 × 32 UE8M0 block scales, as in the checkpoint, applied by
chapter 95's `de_project`, which quantizes its input to FP8 first.

The small shape: width 32, two query heads of 32 with 8 rotary dimensions,
a 3-token window, an 8-position cache, 8 experts with 2 active, an expert
hidden width of 32, two index heads, top 3 positions, candidate blocks of 3
with 2 kept, 16 rows per Engram table and a 64-token vocabulary. The Engram
rows keep the original 24 × 256 layout, so its projection is the original
6,144 columns wide.

The weights come from one integer hash. An FP8 code has exponent field 1–7
and either sign; its block scale is 2^-6 to 2^-4, so every dense weight lies
between 2^-12 and 0.12:

<!-- source: psionic/lib/deepseek/flash.coax generate -->
```jai
df_hash :: (seed:int,i:int)->int {
    x:=(seed*2654435761+i*40503+12345)&0xffffffff;x=((x^(x>>13))*1274126177)&0xffffffff;return x^(x>>16);
}
// FP8 E4M3 codes with exponent field 1..7 and either sign, UE8M0 block
// scales 2^-6..2^-4: every weight is finite and between 2^-12 and 0.12.
df_fp8 :: (seed:int,rows:int,cols:int,data:[]u8,scales:[]u8)->DeMatrix {
    for i:0..<rows*cols {h:=df_hash(seed,i);code:=h%56+8;if (h>>8)%2==1 {code+=128;}data[i]=cast(u8)code;}
    n:=((rows+31)/32)*(cols/32);for b:0..<n {scales[b]=cast(u8)(121+df_hash(seed+500000,b)%3);}
    return DeMatrix.{kind=DeKind.fp8,rows=rows,columns=cols,data=data[0..<rows*cols],scales=scales[0..<n]};
}
// FP4 E2M1 nibbles, any code, one UE8M0 scale per row and 32 columns.
df_fp4 :: (seed:int,rows:int,cols:int,data:[]u8,scales:[]u8)->DeMatrix {
    n:=rows*(cols/32);for i:0..<rows*cols/2 {data[i]=cast(u8)(df_hash(seed,i)%256);}for b:0..<n {scales[b]=cast(u8)(122+df_hash(seed+500000,b)%3);}
    return DeMatrix.{kind=DeKind.fp4,rows=rows,columns=cols,data=data[0..<rows*cols/2],scales=scales[0..<n]};
}
// k/256 for |k| <= 128 and 1 + k/64 for |k| <= 8 are exact BF16 values.
df_bf16 :: (seed:int,i:int)->u16 {return cast(u16)(f32_bits(f32(f64(df_hash(seed,i)%257-128)/256))>>16);}
df_norm :: (seed:int,i:int)->u16 {return cast(u16)(f32_bits(f32(1+f64(df_hash(seed,i)%17-8)/64))>>16);}
df_unit :: (seed:int,i:int,shift:int)->f32 {return f32(f64(df_hash(seed,i)%2001-1000)/f64(1<<shift));}
df_seed :: (layer:int,id:int)->int {return 1000*(layer+1)+id;}
```

The feed-forward half of a layer shows the composition style: each piece is
called as its own chapter defined it, and a refusal names the step, the
layer, the stage and the piece's own status.

<!-- source: psionic/lib/deepseek/flash.coax moe -->
```jai
df_moe :: (t:DfTiny,layer:int,p:int,x:[]f32,routes:[]int,out:[]f32)->DfResult {
    xn:[32]f32;status:=df_rms(x,df_seed(layer,61),xn[..]);if status!=DkStatus.ok {return df_fail(p,layer,DfStage.norm,cast(u8)status);}
    logits:[8]f32;bias:[8]f32;
    for e:0..<DF_E {acc:PsionicAccumulator;for j:0..<DF_D {acc=psionic_ops_accumulate(acc,f64(df_value(df_bf16(df_seed(layer,80),e*DF_D+j)))*f64(xn[j]));}
        logits[e]=f32(psionic_ops_total(acc));bias[e]=df_unit(df_seed(layer,81),e,12);}
    scores:[8]f64;ranked:[8]f64;
    route:=dr_route(DrConfig.{profile=DrProfile.v41,k=DF_K,scale=1.5},logits[..],bias[..],scores[..],ranked[..],false);
    if route.status!=DrStatus.ok {return df_fail(p,layer,DfStage.route,cast(u8)route.status);}
    ids:[2]int;weights:[2]f32;for i:0..<DF_K {ids[i]=route.ids[i];weights[i]=f32(route.weights[i]);routes[i]=route.ids[i];}
    // Only the two routed experts are generated, as an expert cache would
    // hold only what the router picked; they sit in slots 0 and 1.
    store:[3]DfStore;experts:[2]DeExpert;slots:=[2]int.[0,1];
    for i:0..<DF_K {s:=df_seed(layer,20+3*ids[i]);
        experts[i]=DeExpert.{gate=df_fp4(s,DF_F,DF_D,store[i].gate[..],store[i].gate_scales[..]),up=df_fp4(s+1,DF_F,DF_D,store[i].up[..],store[i].up_scales[..]),
            down=df_fp4(s+2,DF_D,DF_F,store[i].down[..],store[i].down_scales[..])};}
    shared:=DeExpert.{gate=df_fp8(df_seed(layer,50),DF_F,DF_D,store[2].gate[..],store[2].gate_scales[..]),up=df_fp8(df_seed(layer,51),DF_F,DF_D,store[2].up[..],store[2].up_scales[..]),
        down=df_fp8(df_seed(layer,52),DF_D,DF_F,store[2].down[..],store[2].down_scales[..])};
    if t.poison_layer==layer {store[2].down_scales[0]=255;}
    floats:[224]f32;wide:[64]f64;acc:[32]PsionicAccumulator;
    r:=de_mixture(experts[..],shared,xn[..],slots[..],weights[..],10,out,DeWork.{floats=floats[..],wide=wide[..]},acc[..],false);
    if r.status!=DsStatus.ok {return df_fail(p,layer,DfStage.experts,cast(u8)r.status);}
    return DfResult.{};
}
```

## Check it against a composed oracle

`psionic/tests/deepseek_flash_oracle.py` builds the same 40-layer stack in
Python from the per-chapter oracles only: chapter 92's rotation, packing,
indexer, selection and attention oracles, chapter 93's hyper-connection
oracle, chapter 91's router oracle (which evaluates its scores in 220-digit
decimal arithmetic), chapter 94's hash oracle, chapter 95's expert oracle and
chapter 98's cache oracle. It shares no code with `flash.coax`; it shares the
hash formula and the shapes. It writes the routed experts, the selected
positions and the logits of seven tokens to
`psionic/fixtures/deepseek/flash-goldens.coax`. The smallest router margin
across all 280 routing decisions is 2.5 × 10^-5, so no expert choice is a
near tie.

The compile-time evaluator that runs `#test` cannot afford the full stack
for seven tokens, so `DfTiny`'s `first` and `layers` pick a run of layers
`first..<layers` that each step enters straight from the embedding, and the
oracle takes the same range. The checks in `flash.coax` cover every path in
pieces: layers 0–20 without Engram for two tokens (window-only layers, ratio-2
pooling with its indexers, and the candidate source at layer 20); layers 20–39
for two tokens (ratio-1 pooling, the indexers at 24, 28, 32 and 36 that read
layer 20's keys and candidate mask, and the borrowing layers); layers 20–24
for all eight positions (the window wraps, and from the seventh token the
candidate mask drops a block); Engram after layer 0 for two tokens and at
layer 14 alone for one; the Engram gate on its own with an all-zero stream;
the weight generators; and every refusal, with the poisoned scale at layer 0.
Each compares every route, selection, index score and logit bit for bit: the
trace also records the eight index scores behind each layer's selection,
because a small change to a key moves a score long before it changes which
positions win. Together they leave no `coax mutate` survivor in `flash.coax`.
The example runs the whole stack natively.

## Run it

[deepseek-flash.coax](../examples/deepseek-flash.coax) checks the manifest, plans this host,
runs the seven tokens and counts what equals the oracle:

<!-- source: psionic/examples/deepseek-flash.coax example -->
```jai
main :: () {
    rows:=DV_MANIFEST;check:=dv_check_manifest(dv_flash_config(),rows[..]);totals:=dv_totals(rows[..]);b:=dv_bytes(rows[..]);
    #assert check.status==DvStatus.ok;
    print("manifest: % rows match the config; % tensors, % bytes\n",check.rows,totals.tensors,totals.bytes);
    print("routed experts %, Engram %, draft %, dense %, embeddings %, shared %, vision %\n",b.routed,b.engram,b.draft,b.dense,b.embed,b.shared,b.vision);
    plan:=dv_plan(b,dv_this_host(),4096);
    print("plan: GPU % with % experts; RAM cache % slots per layer, % experts; disk % experts\n",rp_mode_name(plan.gpu.mode),plan.vram,plan.ram/40,plan.ram,plan.disk);
    print("a token reads % bytes on the GPU, % over PCIe, % from disk: % us, % milli-tokens/s modeled\n",plan.bytes_gpu,plan.bytes_link,plan.bytes_disk,plan.micros,plan.milli_tokens);
    print("verdict: %\n",dv_verdict_name(plan.status));
    tokens:=DVG_TOKENS;routes:[560]int;picks:[840]int;scores:[2240]f32;logits:[448]f32;arena:[984000]u8;
    r:=df_forward(DfTiny.{},tokens[..],DfTrace.{routes=routes[..],picks=picks[..],scores=scores[..],logits=logits[..]},arena[..]);#assert r.status==DfRun.ok;
    same_routes:=0;same_picks:=0;same_logits:=0;
    for i:0..<560 {if routes[i]==DVG_ROUTES[i] {same_routes+=1;}}for i:0..<840 {if picks[i]==DVG_PICKS[i] {same_picks+=1;}}
    for i:0..<448 {if f32_bits(logits[i])==DVG_LOGITS[i] {same_logits+=1;}}
    print("synthetic stack, 7 tokens x 40 layers: routes %/560, selections %/840, logits %/448 identical to the oracle\n",same_routes,same_picks,same_logits);
    #assert same_routes==560 && same_picks==840 && same_logits==448;
    for p:0..<7 {best:=0;for v:1..<64 {if logits[p*64+v]>logits[p*64+best] {best=v;}}print("step %: token % -> top logit % (%)\n",p,tokens[p],best,logits[p*64+best]);}
    at:=6*40+24;print("layer 24 at step 6: experts % and %, compressed rows % % %\n",routes[at*2],routes[at*2+1],picks[at*3],picks[at*3+1],picks[at*3+2]);
}
```

```shell
coax/bin/coax run psionic/examples/deepseek-flash.coax --run-steps 40000000
```

```text
manifest: 116 rows match the config; 96085 tensors, 510286023000 bytes
routed experts 288777830400, Engram 202758032400, draft 7932874632, dense 5782123968, embeddings 2647664640, shared 1416960000, vision 970536960
plan: GPU streamed with 387 experts; RAM cache 167 slots per layer, 6680 experts; disk 8293 experts
a token reads 9960433728 bytes on the GPU, 1962316800 over PCIe, 2436151680 from disk: 470260 us, 2126 milli-tokens/s modeled
verdict: disk: the text checkpoint does not fit on the free disk
synthetic stack, 7 tokens x 40 layers: routes 560/560, selections 840/840, logits 448/448 identical to the oracle
step 0: token 3 -> top logit 10 (3.1599278)
step 1: token 17 -> top logit 2 (3.1962523)
step 2: token 42 -> top logit 56 (2.6274624)
step 3: token 5 -> top logit 29 (3.2233024)
step 4: token 61 -> top logit 28 (3.7668037)
step 5: token 17 -> top logit 33 (2.8405933)
step 6: token 8 -> top logit 3 (3.0961647)
layer 24 at step 6: experts 7 and 4, compressed rows 3 4 5
```

All 560 routes, 840 selections and 448 logits equal the oracle's, in debug and
release.

## What it refuses

A 0xFF UE8M0 scale is NaN. Put one in layer 7's shared expert and the stack
stops at step 0, layer 7, in the experts stage, with chapter 95's `number`
status (4). A token outside the vocabulary, more tokens than cache positions,
a short arena, a header row that disagrees with the configuration, and hosts
that are too small each refuse with their own reason
([deepseek-flash-refuse.coax](../examples/deepseek-flash-refuse.coax)):

<!-- source: psionic/examples/deepseek-flash-refuse.coax refuse -->
```jai
main :: () {
    tokens:=DVG_TOKENS;routes:[560]int;picks:[840]int;scores:[2240]f32;logits:[448]f32;arena:[984000]u8;trace:=DfTrace.{routes=routes[..],picks=picks[..],scores=scores[..],logits=logits[..]};
    stages:=STAGES;runs:=RUNS;checks:=CHECKS;
    // A 0xFF UE8M0 scale (NaN) in layer 7's shared expert.
    r:=df_forward(DfTiny.{poison_layer=7},tokens[..],trace,arena[..]);
    print("NaN scale in layer 7: % at step %, layer %, stage %, piece status %\n",runs[cast(int)r.status],r.step,r.layer,stages[cast(int)r.stage],r.code);
    bad:=tokens;bad[4]=64;r=df_forward(DfTiny.{},bad[..],trace,arena[..]);print("token 64 of a 64-token vocabulary: % at step %\n",runs[cast(int)r.status],r.step);
    nine:=[9]int.[1,2,3,4,5,6,7,8,9];r=df_forward(DfTiny.{},nine[..],trace,arena[..]);print("nine tokens in an 8-position cache: %\n",runs[cast(int)r.status]);
    r=df_forward(DfTiny.{},tokens[..],trace,arena[0..<1000]);print("a 1,000-byte arena: %\n",runs[cast(int)r.status]);
    // One header row whose shape disagrees with the configuration.
    rows:=DV_MANIFEST;at:=dv_find(rows[..],"layers.#.attn.wkv.weight",dv_span(40));
    if at>=0 {rows[at].shape[0]+=1;}c:=dv_check_manifest(dv_flash_config(),rows[..]);
    print("wkv rows 513 instead of 512: % at manifest row %\n",checks[cast(int)c.status],c.bad);
    rows=DV_MANIFEST;b:=dv_bytes(rows[..]);h:=dv_this_host();
    print("this host: %\n",dv_verdict_name(dv_plan(b,h,4096).status));
    h.gpu=8000000000;print("an 8 GB GPU: %\n",dv_verdict_name(dv_plan(b,h,4096).status));
    h=dv_this_host();h.ram=16000000000;h.disk_free=1000000000000;print("16 GB of RAM and 1 TB free: %\n",dv_verdict_name(dv_plan(b,h,4096).status));
}
```

```text
NaN scale in layer 7: piece at step 0, layer 7, stage experts, piece status 4
token 64 of a 64-token vocabulary: token at step 4
nine tokens in an 8-position cache: shape
a 1,000-byte arena: capacity
wkv rows 513 instead of 512: shape at manifest row 21
this host: disk: the text checkpoint does not fit on the free disk
an 8 GB GPU: gpu: the never-streamed weights, cache and two expert slots exceed the GPU
16 GB of RAM and 1 TB free: ok
```

## What running the real model needs

- **Disk.** At least 501.4 GB free for the text checkpoint (510.3 GB with the
  draft layers and vision); this disk has 37.6 GB free. Then the download
  itself, verified against the 48 shard sha256 values the manifest script
  records.
- **The licence question.** The Hub lists MIT. Redistributing weights or
  staged packages stays an owner decision.
- **A weight loader.** Views of the mapped shards as chapter 95 matrices,
  authenticated with chapter 7's identities, and Engram rows read through
  chapter 94's `eg_reads`. The composition here generates its weights.
- **The missing layer pieces.** The indexer's BF16 key projection `wk` and its
  `k_norm` (the composition normalizes the latent directly, as chapter 92's
  chain does), the draft layers (chapter 102) and the vision tower.
- **The engine.** Chapter 96's cache wired into the forward path (dispatch,
  prefetch and installs between steps), chapter 97's layer-major prefill (this
  forward runs one token at a time, so chapter 97 is not used yet), and the GPU path: chapter 99's recorded
  decode step for V4.1, with the packed caches and the expert cache on the
  device. Everything in this chapter ran on the CPU.
- **Qualification.** A reference capture (the pre-reset one cost $36.85 on
  eight B200s; a new or retained capture is an owner decision), chapter 17's
  frozen distribution checks with router near-tie regret, and measured time
  to first token, decode tokens per second and peak memory on a named host
  with build and library identities.

## Sources

- [Chapter plan](PLAN.md)
- [DeepSeek target notes](../docs/targets/deepseek.md)
- [deepseek_v41_manifest.py](../tests/deepseek_v41_manifest.py), the header manifest
- [deepseek_flash_oracle.py](../tests/deepseek_flash_oracle.py), the composed oracle
- [check_deepseek_flash.sh](../tests/check_deepseek_flash.sh), the chapter's checks
