# 197. DeepSeek V4.1 Flash across stages

> **Status: partly implemented.** Three parts run. The boundary contract
> says exactly which bytes cross each of the 39 possible cuts for one token,
> and refuses a message that misses, adds or resizes any part. The planner
> splits the pinned checkpoint's 40 layers over up to 16 hosts from the
> header manifest's own per-layer bytes. And chapter 100's synthetic stack,
> run as a pipeline, equals the unsplit run bit for bit at every cut. No
> original weights were loaded, and the stages ran one after another in one
> process: real peers, measured per-hop latency and tokens per second,
> chapter 190's fencing and stage recovery, and a BF16 wire all remain.

DeepSeek V4.1 Flash has 510.3 GB of original files and 40 layers.
[Chapter 100](ch100-execute-deepseek-v41-flash-on-one-machine.md) showed
that one machine with an RTX 4080 cannot hold it. This chapter splits the
layers into contiguous stages, one per host, and asks two questions: what
has to travel from one stage to the next for each token, and which split
fits a given set of hosts.

[Chapter 186](ch186-split-a-model-into-stages.md) answered both for a plain
decoder: only the hidden state crosses a cut, and every stage keeps the
cache of its own layers. V4.1 Flash breaks that rule in three ways. It has
four residual streams and a hyper-connection carry instead of one hidden
state ([chapter 93](ch093-many-residual-streams.md)). Its Engram layers 1
and 14 read rows by hashed token history
([chapter 94](ch094-memory-by-lookup.md)). And its compressed attention
shares state across layers
([chapter 92](ch092-compressed-and-shared-attention.md)): layers 3–7 read
the compressed rows that layer 2 writes, the indexers at 24, 28, 32 and 36
read layer 20's index keys and candidate mask, and so on. When a cut falls
between an owner and a reader, the owner's state must cross too.

## What crosses a cut

`da_flash_links` from chapter 92 gives each layer's owners. Reading it
layer by layer gives, for every part, the cuts it crosses. A cut "at c"
sits before layer c: stage k ends with layer c − 1 and stage k + 1 begins
with layer c.

| Part | Owner | Read by | Crosses cuts |
| --- | --- | --- | --- |
| Engram ids | layer 1 | layer 1 | 1 |
| Engram ids | layer 14 | layer 14 | 1–14 |
| compressed row | 2, 8, 14 | 3–7, 9–13, 15–19 | 3–7, 9–13, 15–19 |
| compressed row | 20 | 21–39 | 21–39 |
| index-key row | 20 | indexers 24, 28, 32, 36 | 21–36 |
| selection | 2, 8, 14, 20 | 3–7, 9–13, 15–19, 21–23 | the same ranges |
| selection | 24, 28, 32, 36 | 25–27, 29–31, 33–35, 37–39 | the same ranges |
| candidate mask | 20 | indexers 24–36 | 21–36 |

Every cut also carries the four residual streams and the carry. Cuts 2, 8,
14 and 20 carry the least extra state: an owner starts the next stage, so
nothing it writes has to cross.

The compressed rows are sent as they are written, not as a whole cache: at
step p a ratio-2 owner writes row (p + 1)/2 when p is odd, and layer 20
writes row p + 1 at every step. The receiving stage keeps its own copy of
the owner's rows and adds each one as it arrives. The selection and the
mask change at every step and are sent whole.

`dst_layout` builds this list for a cut. `dst_needed` scans the layers
after the cut for a reader; the checks compare every cut's list with the
table above, typed in by hand.

<!-- source: psionic/lib/deepseek/flash-split.coax needed -->
```jai
// Does a layer at or after the cut read this part of owner s? Every layer
// with compression reads its KV owner's rows; an indexer reads its KV
// owner's keys; a layer reads its indexer's selection; the indexers after
// layer 20 read its candidate mask.
dst_needed :: (cut:int,kind:DstKind,s:int)->bool {
    for l:cut..<dv_flash_config().layers {k:=da_flash_links(l);
        if kind==DstKind.comp && k.kv==s {return true;}
        if kind==DstKind.keys && k.produce_index && k.kv==s {return true;}
        if kind==DstKind.index && k.index==s {return true;}
        if kind==DstKind.mask && k.produce_index && k.candidate==s {return true;}
    }
    return false;
}
// The parts that cross the cut before layer `cut`, in order. An invalid
// geometry or a cut outside 1..39 gives no parts.
dst_layout :: (g:DstGeometry,cut:int)->DstLayout {
    out:DstLayout;if !dst_valid(g) || cut<1 || cut>=DST_LAYERS {return out;}
    kinds:[8]DstKind;sources:[8]int;kinds[1]=DstKind.carry;sources[0]=cut;sources[1]=cut;n:=2;
    for e:DST_ENGRAM {if e>=cut {kinds[n]=DstKind.engram;sources[n]=e;n+=1;}}
    for s:DST_KV {if s<cut {
        if dst_needed(cut,DstKind.comp,s) {kinds[n]=DstKind.comp;sources[n]=s;n+=1;}
        if dst_needed(cut,DstKind.keys,s) {kinds[n]=DstKind.keys;sources[n]=s;n+=1;}}}
    for s:DST_INDEX {if s<cut && dst_needed(cut,DstKind.index,s) {kinds[n]=DstKind.index;sources[n]=s;n+=1;}}
    if cut>20 && dst_needed(cut,DstKind.mask,20) {kinds[n]=DstKind.mask;sources[n]=20;n+=1;}
    out.words=DST_HEAD;out.count=n;
    for i:0..<n {w:=dst_words(g,kinds[i]);out.parts[i]=DstPart.{kind=kinds[i],source=sources[i],words=w,offset=out.words+2};out.words+=2+w;}
    return out;
}
```

## Work out the bytes by hand

A message is 32-bit words: a 9-word header, then per part a tag word, a
length word and the data. For the pinned model at 4,096 positions:

- the four residual streams are 4 × 5,120 f32 values: 20,480 words;
- the carry is 4 words;
- a compressed row is 512 FP4 values in 256 bytes plus 32 E4M3 scales, 288
  bytes or 72 words, after one word for the row number: 73 words;
- an index-key row is 128 FP4 values in 64 bytes plus 4 UE8M0 scales, 68
  bytes or 17 words, plus the row word: 18 words;
- a selection is a count and 512 positions: 513 words;
- the candidate mask is one byte per compressed row, 4,096 bytes: 1,024 words.

At cut 20 only the streams and the carry cross:
9 + (2 + 20,480) + (2 + 4) = 20,497 words, **81,988 bytes**. At cut 21
layer 20's row, keys, selection and mask join them:
20,497 + 75 + 20 + 515 + 1,026 = 22,133 words, **88,532 bytes**. At cut 24
the selection no longer crosses (indexer 24 starts the next stage):
86,472 bytes. The synthetic stack's message at cut 21 is
9 + 130 + 6 + 8 + 8 + 6 + 4 = 171 words, 684 bytes.

The residual streams travel as f32, not BF16. The run below is checked bit
for bit against an unsplit run, and the streams between layers are f32;
rounding them to BF16 at a cut would change the result. A BF16 wire is a
separate, qualified change for later.

## The message and what the receiver checks

The header binds the message to the model's digest, the receiver's first
layer and the step. The receiver checks every listed part, in order, before
it reads a value.

<!-- source: psionic/lib/deepseek/flash-split.coax boundary -->
```jai
// The message is 32-bit words: version, the model's digest (two halves),
// the receiver's first layer, the step, the part count, the width, f32, a
// checksum over every other word (chapter 186's), then per part a tag
// (kind + 256 x source layer), its length and its data.
DstRefusal :: enum u8 {none;partial;version;model;layer;position;geometry;missing;size;extra;excess;checksum;}
DstExpect :: struct {model:u64;cut:int;step:int;}
DstVerdict :: struct {refusal:DstRefusal;part:int=-1;}
// Write the header and the part directory with zeroed data; the sender
// fills each part at its offset, then seals. -1: no such cut or step, or
// the buffer is too small.
dst_frame :: (g:DstGeometry,model:u64,cut:int,step:int,wire:[]u32)->int {
    l:=dst_layout(g,cut);if l.count==0 || !stg_in(step,0,DST_STEPS-1) || wire.count<l.words {return -1;}
    head:=[9]u32.[DST_VERSION,cast(u32)(model&4294967295),cast(u32)(model>>32),cast(u32)cut,cast(u32)step,cast(u32)l.count,cast(u32)g.width,STG_F32,0];
    for k:0..<DST_HEAD {wire[k]=head[k];}
    for i:0..<l.count {p:=l.parts[i];wire[p.offset-2]=dst_tag(p);wire[p.offset-1]=cast(u32)p.words;for k:0..<p.words {wire[p.offset+k]=0;}}
    return l.words;
}
dst_seal :: (wire:[]u32) {wire[8]=stg_sum(wire);}
// The receiver's checks, in order: a whole header, version, model, its own
// first layer, the step it expects, the geometry, then every listed part
// in place with its length, nothing more, and the checksum.
dst_check :: (g:DstGeometry,want:DstExpect,wire:[]u32)->DstVerdict {
    v:DstVerdict;l:=dst_layout(g,want.cut);
    if wire.count<DST_HEAD {v.refusal=DstRefusal.partial;return v;}
    if wire[0]!=DST_VERSION {v.refusal=DstRefusal.version;return v;}
    if wire[1]!=cast(u32)(want.model&4294967295) || wire[2]!=cast(u32)(want.model>>32) {v.refusal=DstRefusal.model;return v;}
    if cast(int)wire[3]!=want.cut || l.count==0 {v.refusal=DstRefusal.layer;return v;}
    if cast(int)wire[4]!=want.step {v.refusal=DstRefusal.position;return v;}
    if cast(int)wire[6]!=g.width || wire[7]!=STG_F32 {v.refusal=DstRefusal.geometry;return v;}
    parts:=cast(int)wire[5];
    for i:0..<l.count {p:=l.parts[i];v.part=i;
        if i>=parts {v.refusal=DstRefusal.missing;return v;}
        if wire.count<p.offset {v.refusal=DstRefusal.partial;return v;}
        if wire[p.offset-2]!=dst_tag(p) {v.refusal=DstRefusal.missing;return v;}
        if wire[p.offset-1]!=cast(u32)p.words {v.refusal=DstRefusal.size;return v;}
    }
    v.part=-1;
    if parts>l.count {v.refusal=DstRefusal.extra;v.part=l.count;return v;}
    if wire.count<l.words {v.refusal=DstRefusal.partial;return v;}
    if wire.count>l.words {v.refusal=DstRefusal.excess;return v;}
    if wire[8]!=stg_sum(wire) {v.refusal=DstRefusal.checksum;}
    return v;
}
```

The checksum is chapter 186's `stg_sum`. A message without one of its parts
is refused as `missing`, naming the part; the checks build that message for
every part of every cut.

## Plan the stages from the checkpoint's bytes

`dst_model` reads the per-layer bytes from chapter 100's header manifest.
A plain layer holds 7,219,445,760 bytes of routed experts (288.8 GB / 40)
and 170,055,128 bytes of everything else. A token reads the dense and shared
weights whole, 6 of the 384 routed experts (1/64 of their bytes) and 24 rows
of each Engram table it passes: 170,055,128 + 112,803,840 = 282,858,968 bytes
for a plain layer. The Engram tables are 101.4 GB each, at layers 1 and 14.

A stage's bytes are its weights, a 67,584-byte window ring per layer, the
compressed rows its layers own over the context, the rows it mirrors from
owners before it, its two message buffers, and the embedding or readout.
Layer 39 alone at 4,096 positions:
7,389,500,888 + 67,584 + 288 × 4,096 (layer 20's rows) + 84,348 (the message
in) + 1,323,837,440 (the readout) = **8,714,669,908 bytes**.

<!-- source: psionic/lib/deepseek/flash-split.coax stage -->
```jai
// A stage running layers a..<b: its weights (and Engram tables when they
// are resident), a window ring per layer, the caches it owns over the
// context, the borrowed rows it mirrors, its incoming and outgoing message
// buffers, the embedding if first and the readout if last.
dst_stage_inner :: (m:DstModel,a:int,b:int,context:int,resident:bool)->int {
    n:=0;
    for l:a..<b {n+=m.weight[l]+DST_WINDOW*DST_WINDOW_ROW+m.cache[l]*context;if resident {n+=m.engram[l];}}
    for s:DST_KV {if s<a {r:=da_flash_links(s).ratio;comp:=false;keys:=false;
        for l:a..<b {k:=da_flash_links(l);if k.kv==s {comp=true;if k.produce_index {keys=true;}}}
        // Only layer 20 (ratio 1) lends its keys: the indexers after it read them.
        if comp {n+=DST_COMP_ROW/r*context;}if keys {n+=DST_KEY_ROW*context;}}}
    if a==0 {n+=m.embed;}
    if b==DST_LAYERS {n+=m.head;}
    return n;
}
dst_stage_bytes :: (m:DstModel,a:int,b:int,context:int,resident:bool)->int {
    g:=dst_real(context);return dst_stage_inner(m,a,b,context,resident)+dst_message_bytes(g,a)+dst_message_bytes(g,b);
}
// The weight bytes it reads for one token: its layers, one embedding row
// if first, the whole readout if last.
dst_stage_read :: (m:DstModel,a:int,b:int)->int {
    n:=0;for l:a..<b {n+=m.read[l];}
    if a==0 {n+=m.embed/129280;}
    if b==DST_LAYERS {n+=m.head;}
    return n;
}
```

The planner is chapter 186's dynamic program over contiguous splits: among
the splits whose every stage fits its host, it keeps the one whose slowest
stage reads the fewest weight bytes per token, and the earlier cut on a tie.

<!-- source: psionic/lib/deepseek/flash-split.coax plan -->
```jai
DstStatus :: enum u8 {ok;model;stages;context;total;split;}
// Stage i runs layers cut[i]..<cut[i+1]: its bytes, the weight bytes it
// reads per token, and the message it sends per token. cause: the bytes
// the hosts lack in total.
DstPlan :: struct {status:DstStatus;stages:int;cause:int;slowest:int;cut:[17]int;bytes:[16]int;read:[16]int;sent:[16]int;}
dst_plan :: (m:DstModel,budget:[]int,context:int,resident:bool)->DstPlan {
    n:=budget.count;p:=DstPlan.{stages=n};
    if !dst_model_valid(m) {p.status=DstStatus.model;return p;}
    if n<1 || n>DST_STAGES {p.status=DstStatus.stages;return p;}
    for b:budget {if !stg_in(b,0,STG_BUDGET_MAX) {p.status=DstStatus.stages;return p;}}
    if !stg_in(context,1,STG_CONTEXT_MAX) {p.status=DstStatus.context;return p;}
    // One stage holding everything is the least any split needs in total.
    need:=dst_stage_bytes(m,0,DST_LAYERS,context,resident);room:=0;for b:budget {room+=b;}
    if need>room {p.status=DstStatus.total;p.cause=need-room;return p;}
    // Every stage's bytes, once: sizes[a][b] for layers a..<b. sent[c] is
    // the message into layer c; none enters layer 0 or leaves layer 39.
    sent:[40]int;for c:1..<DST_LAYERS {sent[c]=dst_message_bytes(dst_real(context),c);}
    sizes:[40][41]int;
    for b:1..<DST_LAYERS+1 {out:=ifx b<DST_LAYERS then sent[b] else 0;for a:0..<b {sizes[a][b]=dst_stage_inner(m,a,b,context,resident)+sent[a]+out;}}
    // best[i][b]: the smallest slowest stage over stages 0..i when stage i
    // ends before layer b, if fits[i][b]. On a tie the earlier start wins.
    best:[16][41]int;start:[16][41]int;fits:[16][41]bool;
    for i:0..<n {for b:1..<DST_LAYERS+1 {
        for a:i..<b {
            reach:=ifx i==0 then a==0 else fits[i-1][a];
            if !reach || sizes[a][b]>budget[i] {continue;}
            c:=dst_stage_read(m,a,b);if i>0 {c=stg_max(c,best[i-1][a]);}
            if !fits[i][b] || c<best[i][b] {best[i][b]=c;start[i][b]=a;fits[i][b]=true;}
        }
    }}
    if !fits[n-1][DST_LAYERS] {p.status=DstStatus.split;return p;}
    p.cut[n]=DST_LAYERS;p.slowest=best[n-1][DST_LAYERS];
    for k:0..<n {i:=n-1-k;p.cut[i]=start[i][p.cut[i+1]];}
    for i:0..<n {a:=p.cut[i];b:=p.cut[i+1];p.bytes[i]=sizes[a][b];p.read[i]=dst_stage_read(m,a,b);if i<n-1 {p.sent[i]=sent[b];}}
    return p;
}
```

The checks compare it with a brute force over every split of two and three
hosts.

## Run the stages

`dfs_pipeline` runs chapter 100's synthetic stack (small widths, the
original layer map, Engram at 1 and 14) as stages. Each stage owns a full
set of caches and calls chapter 100's `df_layers` on its own range. The
first stage hashes and embeds the token; every other stage receives a
message, checks it, and writes the borrowed rows into its own copy of the
owner's cache before it runs its layers.

<!-- source: psionic/lib/deepseek/flash-stages.coax pipeline -->
```jai
// Run tokens through stages cut[k]..<cut[k+1] (cut[0] is t.first, the last
// is t.layers); stage k loaded the model whose digest is model[k]. wire
// holds one mailbox of DFS_WORDS per boundary.
dfs_pipeline :: (t:DfTiny,cut:[]int,model:[]u64,tokens:[]int,trace:DfTrace,arena:[]u8,wire:[]u32,fault:DfsFault)->DfsRun {
    n:=cut.count-1;count:=tokens.count;run:DfsRun;
    if n<1 || n>DFS_STAGES || model.count!=n || count<1 || count>DF_CAP || cut[0]!=t.first || cut[n]!=t.layers || t.first<0 || t.layers>DF_L {run.status=DfsStatus.shape;return run;}
    for k:0..<n {if cut[k]>=cut[k+1] {run.status=DfsStatus.shape;return run;}}
    if (t.engram && arena.count<DF_ARENA) || trace.routes.count<count*DF_L*DF_K || trace.picks.count<count*DF_L*DF_TOPK || trace.scores.count<count*DF_L*DF_CAP ||
        trace.logits.count<count*DF_V || wire.count<(n-1)*DFS_WORDS {run.status=DfsStatus.capacity;return run;}
    g:=dst_tiny();stores:[16]DfsStore;caches:[16]DfCaches;layouts:[16]DstLayout;sizes:[16]int;
    for k:0..<n {caches[k]=DfCaches.{window=stores[k].window[..],window_scales=stores[k].window_scales[..],comp=stores[k].comp[..],comp_scales=stores[k].comp_scales[..],
        keys=stores[k].keys[..],key_scales=stores[k].key_scales[..],old_kv=stores[k].old_kv[..],old_gate=stores[k].old_gate[..],picks=stores[k].picks[..],scores=stores[k].scores[..],
        counts=stores[k].counts[..],mask=stores[k].mask[..]};layouts[k]=dst_layout(g,cut[k+1]);run.sent[k]=ifx k<n-1 then 4*layouts[k].words else 0;}
    x:[16][128]f32;carry:[16]DhCarry;hashes:[16][48]int;
    map:=df_engram_map();emap:=EgMap.{raw=DF_V,compressed=99092,pad=2,data=map.data[..]};history:=eg_start();
    run.ticks=count+n-1;
    for tick:0..<run.ticks {for j:0..<n {k:=n-1-j;p:=tick-k;if p<0 || p>=count {continue;}
        run.stage=k;run.step=p;
        if k==0 {one:=[1]int.[tokens[p]];live:=[1]u8.[1];hs:[48]int;
            e:=eg_hash(emap,history,one[..],live[..],hashes[0][..],hs[..],false);if e.status!=EgStatus.ok {run.status=DfsStatus.token;return run;}history=e.next;
            df_embed(tokens[p],x[0][..]);carry[0]=dh_initial();}
        else {box:=wire[(k-1)*DFS_WORDS..<(k-1)*DFS_WORDS+sizes[k-1]];
            v:=dst_check(g,DstExpect.{model=model[k],cut=cut[k],step=p},box);
            if v.refusal!=DstRefusal.none {run.status=DfsStatus.refused;run.refusal=v.refusal;run.part=v.part;return run;}
            carry[k]=dfs_unpack(layouts[k-1],box,x[k][..],hashes[k][..],caches[k]);}
        s:=df_layers(t,p,cut[k],cut[k+1],x[k][..],carry[k],hashes[k][..],caches[k],trace,arena);
        if s.result.status!=DfRun.ok {run.status=DfsStatus.piece;run.piece=s.result;return run;}
        carry[k]=s.carry;
        if k==n-1 {r:=df_head(p,t.layers,x[k][..],carry[k],trace.logits[p*DF_V..<(p+1)*DF_V]);if r.status!=DfRun.ok {run.status=DfsStatus.piece;run.piece=r;return run;}}
        else {box:=wire[k*DFS_WORDS..<(k+1)*DFS_WORDS];size:=dst_frame(g,model[k],cut[k+1],p,box);
            dfs_pack(layouts[k],p,x[k][..],carry[k],hashes[k][..],caches[k],box);dst_seal(box[0..<size]);
            if fault.stage==k && fault.step==p && fault.part>=0 {size=dfs_drop(layouts[k],fault.part,box);}
            if fault.stage==k && fault.step==p && fault.word>=0 {box[fault.word]^=1;}
            sizes[k]=size;run.messages+=1;run.bytes+=4*size;}
    }}
    run.stage=-1;run.step=-1;return run;
}
```

Within a tick the stages run from the last to the first, so each stage
reads its mailbox before the stage in front of it writes the next step
into it.

## Run it

[deepseek-flash-stages.coax](../examples/deepseek-flash-stages.coax) plans
the real checkpoint for several sets of hosts, runs the
synthetic stack unsplit for seven tokens, then split at every cut from 1 to
39 and at the cuts the planner chose, and checks each run against the
unsplit one. Then it breaks three messages on purpose.

<!-- source: psionic/examples/deepseek-flash-stages.coax example -->
```jai
dfse_run :: (cut:[]int,tokens:[]int,want:DfTrace,arena:[]u8,fault:DfsFault,d:[]u64)->DfsRun {
    routes:[560]int;picks:[840]int;scores:[2240]f32;logits:[448]f32;wire:[3000]u32;
    for i:0..<840 {picks[i]=7;}for i:0..<2240 {scores[i]=99;}
    got:=DfTrace.{routes=routes[..],picks=picks[..],scores=scores[..],logits=logits[..]};
    r:=dfs_pipeline(DfTiny.{},cut,d[0..<cut.count-1],tokens,got,arena,wire[..],fault);
    if r.status==DfsStatus.ok {s:=dfs_same(cut,7,got,want);
        #assert s.routes==560 && s.picks==840 && s.logits==448 && s.scores==s.scored;r.part=s.scored;}
    return r;
}
main :: () {
    rows:=DV_MANIFEST;m:=dst_model(rows[..]);gib:=1073741824;h:[16]int;
    print("message bytes per token at 4,096 positions:");for c:[7]int.[1,3,14,20,21,24,39] {print(" cut %: %",c,dst_message_bytes(dst_real(4096),c));}print("\n");
    for n:[2]int.[14,13] {for i:0..<n {h[i]=24*gib;}p:=dst_plan(m,h[0..<n],4096,false);
        print("% x 24 GiB GPUs, Engram on the hosts: %\n",n,dst_status_name(p.status));
        if p.status==DstStatus.ok {for i:0..<n {print("  stage %: layers %-%, % bytes, reads % a token, sends %\n",i,p.cut[i],p.cut[i+1]-1,p.bytes[i],p.read[i],p.sent[i]);}}}
    for i:0..<16 {h[i]=16*gib;}p:=dst_plan(m,h[0..<16],4096,false);print("16 x 16 GiB GPUs: % (% bytes short)\n",dst_status_name(p.status),p.cause);
    for n:[2]int.[4,5] {for i:0..<n {h[i]=134828441600-8589934592;}p=dst_plan(m,h[0..<n],4096,true);
        print("% RAM hosts of 126.2 GB, Engram resident: %",n,dst_status_name(p.status));
        if p.status==DstStatus.ok {print(", cuts");for i:1..<n {print(" %",p.cut[i]);}print(", slowest stage reads % bytes a token",p.slowest);}print("\n");}
    // The synthetic stack, unsplit, then split at every cut.
    tokens:=DVG_TOKENS;routes:[560]int;picks:[840]int;scores:[2240]f32;logits:[448]f32;arena:[984000]u8;d:[16]u64;for k:0..<16 {d[k]=0x0197;}
    want:=DfTrace.{routes=routes[..],picks=picks[..],scores=scores[..],logits=logits[..]};
    #assert df_forward(DfTiny.{},tokens[..],want,arena[..]).status==DfRun.ok;
    same:=0;
    for c:1..<40 {cut:=[3]int.[0,c,40];r:=dfse_run(cut[..],tokens[..],want,arena[..],DfsFault.{},d[..]);#assert r.status==DfsStatus.ok;same+=1;
        if c==1 || c==3 || c==14 || c==20 || c==21 || c==24 {print("cut %: % bytes a token, % messages; routes, selections, logits identical\n",c,r.sent[0],r.messages);}}
    print("two stages, cuts 1-39: % of 39 runs identical to the unsplit stack\n",same);
    plan:=[15]int.[0,3,6,9,12,15,18,21,24,27,30,33,36,39,40];r:=dfse_run(plan[..],tokens[..],want,arena[..],DfsFault.{},d[..]);
    print("the 14-GPU plan's cuts: % stages, % ticks, % messages, % bytes; identical\n",14,r.ticks,r.messages,r.bytes);
    ram:=[6]int.[0,3,14,17,31,40];r=dfse_run(ram[..],tokens[..],want,arena[..],DfsFault.{},d[..]);
    print("the 5-host plan's cuts: % stages, % messages, % bytes; identical\n",5,r.messages,r.bytes);
    // What a stage refuses.
    cut:=[4]int.[0,14,21,40];
    r=dfse_run(cut[..],tokens[..],want,arena[..],DfsFault.{stage=1,step=4,part=5},d[..]);
    print("stage 1 leaves out part 5 (layer 20's mask) at step 4: stage % refuses step %: % part %\n",r.stage,r.step,dst_refusal_name(r.refusal),r.part);
    r=dfse_run(cut[..],tokens[..],want,arena[..],DfsFault.{stage=0,step=2,word=60},d[..]);
    print("one bit flips in stage 0's message for step 2: stage % refuses step %: %\n",r.stage,r.step,dst_refusal_name(r.refusal));
    d[2]=0x0198;r=dfse_run(cut[..],tokens[..],want,arena[..],DfsFault.{},d[..]);
    print("stage 2 loaded another model: stage % refuses step %: %\n",r.stage,r.step,dst_refusal_name(r.refusal));
}
```

```shell
coax/bin/coax run psionic/examples/deepseek-flash-stages.coax --release --run-steps 40000000
```

```text
message bytes per token at 4,096 positions: cut 1: 82196 cut 3: 84452 cut 14: 82092 cut 20: 81988 cut 21: 88532 cut 24: 86472 cut 39: 84348
14 x 24 GiB GPUs, Engram on the hosts: ok
  stage 0: layers 0-2, 23667061868 bytes, reads 1022309192 a token, sends 84452
  stage 1: layers 3-5, 22169464144 bytes, reads 848576904 a token, sends 84452
  stage 2: layers 6-8, 22186387024 bytes, reads 864770696 a token, sends 84452
  stage 3: layers 9-11, 22169464144 bytes, reads 848576904 a token, sends 84452
  stage 4: layers 12-14, 22343908840 bytes, reads 1022298952 a token, sends 84348
  stage 5: layers 15-17, 22169463936 bytes, reads 848576904 a token, sends 84348
  stage 6: layers 18-20, 22181877208 bytes, reads 859527816 a token, sends 88532
  stage 7: layers 21-23, 22170060068 bytes, reads 848576904 a token, sends 86472
  stage 8: layers 24-26, 22175914276 bytes, reads 854152584 a token, sends 88532
  stage 9: layers 27-29, 22175916336 bytes, reads 854152584 a token, sends 88532
  stage 10: layers 30-32, 22175916336 bytes, reads 854152584 a token, sends 88532
  stage 11: layers 33-35, 22170060068 bytes, reads 848576904 a token, sends 86472
  stage 12: layers 36-38, 22175910092 bytes, reads 854152584 a token, sends 84348
  stage 13: layers 39-39, 8714669908 bytes, reads 1606696408 a token, sends 0
13 x 24 GiB GPUs, Engram on the hosts: split: no contiguous split fits the hosts
16 x 16 GiB GPUs: total: the hosts together are too small (23753020864 bytes short)
4 RAM hosts of 126.2 GB, Engram resident: split: no contiguous split fits the hosts
5 RAM hosts of 126.2 GB, Engram resident: ok, cuts 3 14 17 31, slowest stage reads 3982127824 bytes a token
cut 1: 788 bytes a token, 7 messages; routes, selections, logits identical
cut 3: 740 bytes a token, 7 messages; routes, selections, logits identical
cut 14: 684 bytes a token, 7 messages; routes, selections, logits identical
cut 20: 580 bytes a token, 7 messages; routes, selections, logits identical
cut 21: 684 bytes a token, 7 messages; routes, selections, logits identical
cut 24: 660 bytes a token, 7 messages; routes, selections, logits identical
two stages, cuts 1-39: 39 of 39 runs identical to the unsplit stack
the 14-GPU plan's cuts: 14 stages, 20 ticks, 91 messages, 62468 bytes; identical
the 5-host plan's cuts: 5 stages, 28 messages, 19208 bytes; identical
stage 1 leaves out part 5 (layer 20's mask) at step 4: stage 2 refuses step 4: missing part 5
one bit flips in stage 0's message for step 2: stage 1 refuses step 2: checksum
stage 2 loaded another model: stage 2 refuses step 0: model
```

Fourteen 24 GiB GPUs hold the text model when the Engram tables stay in
host memory or on disk, as chapter 100's plan keeps them; the last stage
holds only layer 39 because the 1.3 GB readout makes it the slowest anyway.
Thirteen do not fit. With the Engram tables resident, four hosts of
126.2 GB hold enough bytes in total but no contiguous split puts a
101.4 GB table and its neighbours on one host; five do.

Every split equals the unsplit run in its routes, selections and logits,
and in each indexer's scores. A layer that borrows its selection from an
earlier stage receives the positions, not the scores behind them, so its
recorded scores are compared at the indexer that made them.

## What it refuses

`dst_check` refuses, in order: a message shorter than its header
(`partial`), another version, another model, another first layer, another
step (`position`), another width or value type (`geometry`), a part that is
absent or out of place (`missing`, with its index), a wrong length
(`size`), cut-off data (`partial`), extra parts or words (`extra`,
`excess`), and a checksum that does not match. The transcript's last three
lines are such refusals inside a run: the run stops at the refusing stage
and step. `dst_plan` refuses a model with a layer of no bytes, 0 or more
than 16 hosts, a budget out of range, a context outside 1–1,048,576, hosts
too small in total (`total`, with the bytes missing), and hosts on which
no contiguous split fits (`split`).

## What remains

- **Real peers.** The stages run one after another in one process. Sending
  the messages between machines, measuring each hop's latency, bytes and
  tokens per second, and chapter 190's generation fencing and stage
  recovery have not been done for this model.
- **Real weights.** No original weights were loaded; the planner uses the
  checkpoint's header bytes only. Chapter 100's list of missing pieces
  applies to every stage.
- **BF16 on the wire.** The contract carries f32 streams; a BF16 boundary
  needs its own qualification under [chapter 17](ch017-qualify-honestly.md).
- **Licence.** The V4.1 checkpoint's redistribution licence is an open
  owner question before stage packages are distributed.

## Checks

```shell
coax/bin/coax test psionic/lib/deepseek/flash-split.coax
coax/bin/coax mutate psionic/lib/deepseek/flash-split.coax --root-only
coax/bin/coax test psionic/lib/deepseek/flash-stages.coax --run-steps 40000000
```

`flash-split.coax` is `#authority`: its mutation run kills all 455 mutants.
`flash-stages.coax` checks two-stage runs of layers 0–20 at cuts 1, 3, 14
and 20 and a three-stage run of layers 20–24 over eight steps against
chapter 100's composed oracle records, Engram ids crossing after layer 0,
and every refusal; the example runs every cut of the whole stack natively.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [DeepSeek target](../docs/targets/deepseek.md)
- [Chapter 186: split a model into stages](ch186-split-a-model-into-stages.md)
- [Chapter 100: V4.1 Flash on one machine](ch100-execute-deepseek-v41-flash-on-one-machine.md)
