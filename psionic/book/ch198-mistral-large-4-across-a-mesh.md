# 198. Mistral Large 4 across a mesh

> **Status: partly implemented.** The checkpoint inventory, the per-layer
> byte counts, the mesh plan and a small model split across stages and
> expert peers run on the CPU (profile large4-mesh-v1). The inventory of
> Mistral Large 4's 54 shards comes from their safetensors headers alone
> and is checked against the shapes in params.json. The plan cuts the 72
> layers into pipeline stages, splits each stage's routed experts over a
> group of peers, counts the cache copies and boundary bytes that the
> shared-cache and shared-indexer layers add, and finds the smallest mesh
> of identical peers that holds the model. Times are modeled from assumed
> rates, not measured. Still to build: Large 4's real weights on real
> machines over [chapter 188](ch188-peers-transport-and-discovery.md)'s
> transport, parity with a whole-model reference on selected layers and
> tokens, measured per-hop latency, stage packages that fetch only their
> own tensors ([chapter 187](ch187-stage-packages.md)), Large 4's own
> layer arithmetic (NoPE layers, the lightning indexer and FP8 block
> scales in one forward pass), and [chapter 194](ch194-who-can-read-the-prompt.md)'s
> trust rules on the groups.

The plan's first target for this chapter was Mistral Large 3 (675B).
Chapters [86](ch086-sparse-attention-and-shared-layers.md) and
[87](ch087-the-models-own-draft-head.md) pinned its successor, Mistral
Large 4 (`mistralai/Mistral-Large-4-1T-A52B` at revision `395730ae`), and
this chapter uses the same pin. Its 54 shards hold 1,060,548,427,328
bytes of tensors. No machine on hand holds that, so the model has to be
split.

A split must decide three things: which layers each machine runs, which
experts each machine holds, and what has to cross between machines for
every token. Large 4 makes the third question harder than a plain stack
of layers, because some layers read state that an earlier layer wrote:

- Layers 39, 43, 47, ... 71 keep no cache of their own. They read layer
  35's latent cache ([chapter 86](ch086-sparse-attention-and-shared-layers.md)).
- Layers marked `shared` in the indexer map (17, 18, 21, 22, ... 70) have
  no indexer weights. This chapter reads that as reusing the selection of
  the nearest earlier `full` layer. The headers confirm the missing
  weights; the reuse comes from the config's naming, not from reference
  code.

When a cut falls between such a layer and its source, the later stage
needs a copy of the source's cache, and every token's boundary message
has to carry the new row. This chapter counts those bytes exactly.

The library is [`large4-mesh.coax`](../lib/large4-mesh.coax), an
`#authority` file because it decides which machine holds which layers
and experts and refuses meshes that do not fit. It reuses chapter 85's
layer arithmetic, chapter 186's boundary messages, chapter 189's placement
solver and chapter 191's expert exchange. The examples are
[`large4-mesh.coax`](../examples/large4-mesh.coax) and
[`large4-mesh-refuse.coax`](../examples/large4-mesh-refuse.coax).

```shell
coax/bin/coax run psionic/examples/large4-mesh.coax
coax/bin/coax run psionic/examples/large4-mesh-refuse.coax
```

## The inventory from 54 headers

A safetensors file starts with 8 bytes that give the length of a JSON
header, then the header, which names every tensor with its dtype, shape
and byte range. [`large4_manifest.py`](../tests/large4_manifest.py)
downloads params.json, the index and, by HTTP range requests, only the
54 headers (about 6 MB in all). It checks that each header's tensors tile
its shard exactly up to the shard's size, that the index places each
tensor in the shard whose header holds it, and that the tensors add up
to the index's `total_size`. It then groups the 56,563 tensors into 79
kinds: one name with its layer and expert numbers taken out, one dtype,
one shape. The layers that hold a kind are written as ranges. For
example, the rotary layers' cache projection:

```text
layers.0-2,4-6,8-10,...,68-70.attention.wkv_a_with_mqa.weight BF16 576,6144
```

The Coax side derives 61 of those 79 lines from params.json's numbers and
the layer maps of chapter 86, renders them the same way, and requires
each to appear exactly once. The other 18 lines are the draft head that
[chapter 87](ch087-the-models-own-draft-head.md) checks. Every line is
then parsed and counted, and the totals must equal the pins: 56,563
tensors and 1,060,548,427,328 bytes.

The params.json numbers the shapes come from:

<!-- source: psionic/lib/large4-mesh.coax config -->
```jai
// params.json's numbers for the text model and the vision encoder. nope and
// rope: a rotary layer's per-head query split; a NoPE layer's query is all
// non-rotary (nope + rope wide) and its cache row is the latent alone.
L4Config :: struct {
    dim:int;layers:int;heads:int;nope:int;rope:int;value:int;kv_rank:int;vocab:int;
    experts:int;top_k:int;expert_mlp:int;dense_mlp:int;dense_layers:int;
    index_heads:int;index_dim:int;index_topk:int;block:int;
    vision_dim:int;vision_mlp:int;vision_layers:int;patch:int;channels:int;merge:int;
}
l4_large4 :: ()->L4Config {
    return L4Config.{dim=6144,layers=72,heads=64,nope=64,rope=64,value=128,kv_rank=512,vocab=204800,
        experts=128,top_k=4,expert_mlp=6144,dense_mlp=24576,dense_layers=1,index_heads=32,index_dim=128,index_topk=2048,block=128,
        vision_dim=2048,vision_mlp=6144,vision_layers=30,patch=14,channels=3,merge=2};
}
```

The headers held two facts that the config does not state directly. The
NoPE layers (3, 7, ... 71) have no rotary key, so their cache row is the
512-value latent alone (`wkv_a_with_mqa` is 512 × 6144, not 576 × 6144),
and their query is all non-rotary, so `wkv_b` is 16,384 × 512 rather
than 12,288 × 512. Weights are FP8 E4M3 in 128 × 128 blocks, each with
an F32 scale (`qscale_weight`).

<!-- source: psionic/lib/large4-mesh.coax inventory -->
```jai
// Read a manifest line: how many tensors it stands for (layers in its
// ranges times experts in its ranges) and their bytes. -1 tensors when the
// line does not parse.
L4Line :: struct {tensors:int;bytes:int;draft:bool;}
// A decimal number at s[at..]: its value, where it ends, and whether it
// has a digit at all.
L4Num :: struct {value:int;end:int;ok:bool;}
l4_digit :: (c:u8)->bool {return c>='0' && c<='9';}
l4_num :: (s:string,at:int)->L4Num {
    r:=L4Num.{end=at};
    while r.end<s.count && l4_digit(s[r.end]) {r.value=r.value*10+cast(int)(s[r.end]-'0');r.end+=1;r.ok=true;}
    return r;
}
// The numbers in ranges such as "0-16,19-20,23": 0 when it does not parse.
l4_count :: (s:string)->int {
    n:=0;at:=0;
    while true {
        a:=l4_num(s,at);
        if !a.ok {return 0;}
        b:=a;
        if a.end<s.count && s[a.end]=='-' {b=l4_num(s,a.end+1);if !b.ok || b.value<=a.value {return 0;}}
        n+=b.value-a.value+1;
        if b.end<s.count {if s[b.end]!=',' {return 0;}at=b.end+1;continue;}
        return n;
    }
    return n;
}
l4_parse :: (line:string)->L4Line {
    r:=L4Line.{tensors=-1};sp:[2]int;spaces:=0;
    for i:0..<line.count {if line[i]==' ' {if spaces<2 {sp[spaces]=i;}spaces+=1;}}
    if spaces!=2 {return r;}
    name:=line[0..<sp[0]];dtype:=line[sp[0]+1..<sp[1]];dims:=line[sp[1]+1..<line.count];
    size:=0;for t:0..<3 {if dtype==L4_DTYPES[t] {size=L4_SIZE[t];}}
    if size==0 {return r;}
    // Dimensions: numbers joined by commas, each at least 1.
    values:=1;at:=0;
    while true {
        x:=l4_num(dims,at);
        if !x.ok || x.value<1 {return r;}
        values*=x.value;
        if x.end<dims.count {if dims[x.end]!=',' {return r;}at=x.end+1;continue;}
        break;
    }
    // Ranges follow "layers." and "experts."; a name without them is one tensor.
    tensors:=1;start:=0;
    for i:0..<name.count+1 {
        if i<name.count && name[i]!='.' {continue;}
        part:=name[start..<i];start=i+1;
        if l4_num(part,0).ok {c:=l4_count(part);if c==0 {return r;}tensors*=c;}
    }
    r.tensors=tensors;r.bytes=tensors*values*size;r.draft=name.count>11 && name[0..<11]=="mtp_dspark.";
    return r;
}
// The inventory: every derived line found exactly once, every line parsed,
// and the totals equal to the pins. bad names the first derived kind missing
// or repeated (-1: none), line the first line that does not parse.
L4Inventory :: struct {status:L4Status;lines:int;tensors:int;bytes:int;draft:int;bad:int=-1;line:int=-1;}
L4_LINES_MAX :: 128;
l4_inventory :: (c:L4Config,manifest:string)->L4Inventory {
    r:L4Inventory;ends:[129]int;ends[0]=-1;
    for i:0..<manifest.count {if manifest[i]=='\n' {
        if r.lines==L4_LINES_MAX {r.status=L4Status.manifest;return r;}
        r.lines+=1;ends[r.lines]=i;
    }}
    for k:0..<r.lines {
        p:=l4_parse(manifest[ends[k]+1..<ends[k+1]]);
        if p.tensors<1 {r.status=L4Status.manifest;r.line=k;return r;}
        r.tensors+=p.tensors;r.bytes+=p.bytes;if p.draft {r.draft+=p.bytes;}
    }
    kinds:[L4_KINDS]L4Kind;l4_kinds(c,kinds[..]);buf:[512]u8;
    for e:0..<L4_KINDS {
        n:=l4_line(c,kinds[e],buf[..]);line:=cast(string)buf[0..<n];found:=0;
        for k:0..<r.lines {from:=ends[k]+1;if ends[k+1]-from==n && manifest[from..<ends[k+1]]==line {found+=1;}}
        if found!=1 {r.status=L4Status.manifest;r.bad=e;return r;}
    }
    if r.tensors!=L4_PINNED_TENSORS || r.bytes!=L4_PINNED_TENSOR_BYTES {r.status=L4Status.total;}
    return r;
}
```

### One routed expert, by hand

A routed expert has three 6,144 × 6,144 matrices (`w1`, `w2`, `w3`) at
one byte a value, and each has a 48 × 48 grid of F32 block scales
(6,144 / 128 = 48):

```text
3 × (6,144 × 6,144 + 48 × 48 × 4) = 3 × (37,748,736 + 9,216) = 113,273,856 bytes
```

A mixture layer holds 128 of them: 14,499,053,568 bytes. Layer 1 adds
120,348,928 bytes of attention, 27,163,264 of indexer, one shared expert
of the same 113,273,856 bytes, a 128 × 6,144 BF16 router (1,572,864) and
25,600 bytes of norms: 14,761,438,080 in all. Routed experts are 98.2%
of each mixture layer. Layer 0 is dense (a 24,576-wide feed-forward
block) and holds 600,633,216 bytes.

A decode token reads every non-routed weight of a layer and 4 of its 128
experts: 262,384,512 + 4 × 113,273,856 = 715,479,936 bytes for layer
1. Over 71 mixture layers, layer 0 and the 2.5 GB BF16 readout that is
about 54 GB a token. The model's name says 52B active parameters; most
of them are stored at one byte each.

## What crosses a cut

A stage of layers `a..<b` is held by its group's first peer, the
leader: every non-routed weight, its share of the experts, each layer's
own cache, and a copy of any cache it reads from before `a`. The other
peers in the group hold their share of the experts. A layer with its own
cache keeps 576 values a position (rotary) or 512 (NoPE); a layer with
its own indexer also keeps 128 index-key values a position. Both caches
are FP8 in params.json, so the plan counts one byte a value.

<!-- source: psionic/lib/large4-mesh.coax mesh -->
```jai
// The mesh: `stages` groups of `width` peers each. Every peer has `budget`
// bytes and reads memory at `read` MB/s. Within a group links have latency
// lat_in ns and bandwidth bw_in MB/s; between groups lat_out and bw_out.
// context: the positions the cache holds. kv: bytes a cached value (1:
// params.json's fp8_kv_cache and fp8_indexer).
L4Mesh :: struct {stages:int;width:int;budget:int;read:int;lat_in:int;bw_in:int;lat_out:int;bw_out:int;context:int;kv:int;}
// A stage of layers a..<b in pipeline position i of n. leader: the group's
// first peer, which holds the attention, indexers, router, shared expert,
// the dense block, its share of the experts, the caches, a copy of any
// cache it reads from an earlier stage, and the embedding (first) or the
// readout (last). peer: any other peer, its share of the experts. read:
// the leader's bytes read for one decode token: every non-routed weight,
// ceil(top_k / width) experts a mixture layer, the selected cache rows
// (index_topk at most) and every index key. imported: the bytes of copied
// caches.
L4Stage :: struct {leader:int;peer:int;read:int;imported:int;mixture:int;}
// The latent row a position of layer l keeps: rotary layers the latent and
// the rotary key, NoPE layers the latent; 0 for a layer reading another's.
l4_min :: (a:int,b:int)->int {return a+b-stg_max(a,b);}
l4_row :: (c:L4Config,l:int)->int {
    if ML4_KV_SOURCE[l]!=l {return 0;}
    return ifx ML4_ROTARY[l]==1 then c.kv_rank+c.rope else c.kv_rank;
}
// The nearest full indexer layer at or before l.
l4_indexer :: (l:int)->int {j:=l;while ML4_INDEXER_FULL[j]==0 {j-=1;}return j;}
// y: every layer's bytes (l4_layer), t: the top level's (l4_top).
// Layer l added to a stage whose first layer is a. A layer reading a
// cache from before a imports it, once per stage: only its first reader in
// the stage counts it.
l4_add :: (c:L4Config,m:L4Mesh,y:[]L4Layer,given:L4Stage,a:int,l:int)->L4Stage {
    s:=given;share:=c.experts/m.width;active:=(c.top_k+m.width-1)/m.width;
    sel:=l4_min(m.context,c.index_topk);
    base:=0;for p:0..<L4_PARTS {if p!=L4_EXPERT {base+=y[l].part[p];}}
    s.leader+=base+share*y[l].one;s.peer+=share*y[l].one;s.read+=base+active*y[l].one;
    if l4_in(c,8,l) {s.mixture+=1;}
    src:=ML4_KV_SOURCE[l];row:=l4_row(c,src);first:=src<a;
    for k:a..<l {if ML4_KV_SOURCE[k]==src {first=false;}}
    if first {s.imported+=row*m.context*m.kv;s.leader+=row*m.context*m.kv;}
    s.leader+=l4_row(c,l)*m.context*m.kv;s.read+=row*sel*m.kv;
    if ML4_INDEXER_FULL[l]==1 {s.leader+=c.index_dim*m.context*m.kv;s.read+=c.index_dim*m.context*m.kv;}
    return s;
}
// The top level's share: the embedding on the first stage (and one row of
// it read a token), the readout and final norm on the last.
l4_ends :: (c:L4Config,t:L4Top,given:L4Stage,i:int,n:int)->L4Stage {
    s:=given;
    if i==0 {s.leader+=t.embed+t.rope;s.read+=2*c.dim;}
    if i==n-1 {s.leader+=t.head;s.read+=t.head;}
    return s;
}
l4_stage :: (c:L4Config,m:L4Mesh,y:[]L4Layer,t:L4Top,a:int,b:int,i:int,n:int)->L4Stage {
    s:L4Stage;for l:a..<b {s=l4_add(c,m,y,s,a,l);}
    return l4_ends(c,t,s,i,n);
}
// Every layer's bytes, once.
l4_all :: (c:L4Config,y:[]L4Layer) {for l:0..<c.layers {y[l]=l4_layer(c,l);}}
// The bytes stage i sends stage i + 1 for one decode token, as chapter
// 186's f32 messages: the hidden row, each cache row a later stage reads
// from a layer at or before this cut, and each indexer selection (32-bit
// ids) a later shared-indexer layer reuses from this side of the cut.
l4_boundary :: (c:L4Config,m:L4Mesh,cut:int)->int {
    bytes:=stg_message_bytes(1,c.dim);
    sel:=l4_min(m.context,c.index_topk);
    rows:[128]bool;ids:[128]bool;
    for l:cut..<c.layers {
        src:=ML4_KV_SOURCE[l];
        if src<cut && !rows[src] {rows[src]=true;bytes+=stg_message_bytes(1,l4_row(c,src));}
        j:=l4_indexer(l);
        if j<cut && !ids[j] {ids[j]=true;bytes+=stg_message_bytes(1,sel);}
    }
    return bytes;
}
// One mixture layer's expert exchange for a decode token on the leader: the
// token's top_k experts on distinct peers other than the leader where there
// are enough (the spread the plan assumes), priced by chapter 191.
l4_exchange :: (c:L4Config,m:L4Mesh)->int {
    if m.width==1 {return 0;}
    // Chapter 191's contiguous split; -1 when it refuses the width.
    owner:[256]int;o:=owner[0..<c.experts];
    if xm_contiguous(c.experts,m.width,o)!=XmStatus.ok {return -1;}
    r:=M4Route.{count=c.top_k};share:=c.experts/m.width;
    // Slot k takes expert k of peer 1 + k mod (width - 1).
    for k:0..<c.top_k {r.ids[k]=share*(1+k%(m.width-1))+k;}
    routes:=[1]M4Route.[r];home:=[1]int.[0];
    b:=xm_bytes(routes[..],1,home[..],o,m.width,c.dim);
    lat:[8][8]int;bw:[8][8]int;for x:0..<8 {for y:0..<8 {lat[x][y]=m.lat_in;bw[x][y]=m.bw_in;}}
    return xm_exchange(b,m.width,lat,bw);
}
```

### Boundary bytes, by hand

Every cut sends chapter 186's f32 message: a 9-word header and the
hidden row, `4 × (9 + 6,144) = 24,612` bytes. Then:

- A cut at 39 (or anywhere from 36 to 71) puts layer 35 before the cut
  and some of its readers after it. The message also carries layer 35's
  new 512-value row: `4 × (9 + 512) = 2,084` bytes, 26,696 in all.
- A cut at 17, 18 or 25 separates a shared-indexer layer from the full
  layer whose selection it reuses (16, 16 and 24). The message also
  carries the selection, 2,048 ids: `4 × (9 + 2,048) = 8,228` bytes. At
  1,000 positions the selection is 1,000 ids and costs 4,036 bytes.

A stage after the cut at 39 also holds a copy of layer 35's cache,
512 × 32,768 = 16,777,216 bytes at 32,768 positions, counted once per
stage no matter how many of its layers read it.

## The plan

For a mesh of `stages` groups of `width` peers each, the planner cuts the
layers so every leader fits its budget and the slowest stage is as fast
as possible. A stage's time for one decode token is its leader's bytes
read at the peer's memory rate (decode is memory-bound, chapter 33's
roofline) plus one expert exchange per mixture layer when the group has
more than one peer. The exchange is chapter 191's `xm_bytes` and
`xm_exchange` for one token whose 4 experts sit on distinct peers other
than the leader where there are enough. That spread is an assumption;
a real router may do better or worse.

The cut is a dynamic program over stage starts. Each stage grows one
layer at a time from its start, so the cache copies are counted exactly.
Chapter 189's `plc_place` then assigns the stages to the groups and
reports latency and the busiest step. The width is 1, 2, 4 or 8, which
divide the 128 experts, so every peer holds the same share; the leader,
which also holds the rest, is the one that binds. The smallest mesh is
the one with the fewest peers; among meshes with as many peers, the
widest expert split wins, because its leader reads the fewest experts a
token.

<!-- source: psionic/lib/large4-mesh.coax plan -->
```jai
// The plan: cut[i]..<cut[i+1] are stage i's layers, chosen so every leader
// fits and the slowest stage (its reads at the peer's rate plus its expert
// exchange) is as fast as it can be; the earlier cut wins a tie. Chapter
// 189's solver then places the stages on the groups for the lowest latency.
// latency: one token through every stage, hop and exchange; period: the
// busiest stage with its send, the time between tokens when the pipeline
// is full. On memory: the one-layer stage that fits nowhere (stage, layer)
// and its excess (gap); on split: no cut fits every stage.
L4Plan :: struct {status:L4Status;stages:int;width:int;cut:[9]int;stage:[8]L4Stage;send:[8]int;time:[8]int;exchange:[8]int;latency:int;period:int;layer:int=-1;gap:int;placed:PlcPlacement;}
L4_STAGES_MAX :: 8;
l4_valid :: (c:L4Config,m:L4Mesh)->bool {
    if !plc_in(m.stages,1,L4_STAGES_MAX) || !plc_in(m.budget,1,PLC_AMOUNT_MAX) || !plc_in(m.read,1,PLC_RATE_MAX) || !plc_in(m.context,1,STG_CONTEXT_MAX) || !plc_in(m.kv,1,4) || c.experts%8!=0 {return false;}
    return plc_in(m.lat_in,0,PLC_AMOUNT_MAX) && plc_in(m.bw_in,1,PLC_RATE_MAX) && plc_in(m.lat_out,0,PLC_AMOUNT_MAX) && plc_in(m.bw_out,0,PLC_RATE_MAX) && c.layers==ML4_LAYERS;
}
l4_cost :: (c:L4Config,m:L4Mesh,s:L4Stage,exchange:int)->int {return plc_ns(s.read,m.read)+s.mixture*exchange;}
l4_plan :: (c:L4Config,m:L4Mesh)->L4Plan {
    p:=L4Plan.{stages=m.stages,width=m.width};n:=m.stages;L:=c.layers;
    if !l4_valid(c,m) {p.status=L4Status.input;return p;}
    if m.width!=1 && m.width!=2 && m.width!=4 && m.width!=8 {p.status=L4Status.width;return p;}
    x:=l4_exchange(c,m);y:[128]L4Layer;l4_all(c,y[..]);t:=l4_top(c);
    // A layer alone in the middle must fit (it is the least any stage holds).
    // Checked from the last layer down; the first that does not fit is named.
    for k:0..<L {
        l:=L-1-k;empty:L4Stage;need:=l4_add(c,m,y[..],empty,l,l).leader;
        if need>m.budget {p.status=L4Status.memory;p.layer=l;p.gap=need-m.budget;return p;}
    }
    // best[i][b]: the slowest stage over stages 0..i when stage i ends
    // before layer b (-1: none fits); start[i][b]: where stage i begins.
    // Each stage grows one layer at a time from each start a.
    best:[8][73]int;start:[8][73]int;seen:[8][73]bool;
    for i:0..<n {
        for a:i..<L {
            if (i==0 && a!=0) || (i>0 && !seen[i-1][a]) {continue;}
            s:L4Stage;
            for b:a+1..<L+1 {
                s=l4_add(c,m,y[..],s,a,b-1);e:=l4_ends(c,t,s,i,n);
                if e.leader>m.budget {continue;}
                k:=l4_cost(c,m,e,x);
                if i>0 {k=stg_max(k,best[i-1][a]);}
                if !seen[i][b] || k<best[i][b] {best[i][b]=k;start[i][b]=a;seen[i][b]=true;}
            }
        }
    }
    if !seen[n-1][L] {p.status=L4Status.split;return p;}
    p.cut[n]=L;
    for k:0..<n {i:=n-1-k;p.cut[i]=start[i][p.cut[i+1]];}
    j:=PlcJob.{stages=n};f:=PlcFleet.{peers=n,deadline=PLC_AMOUNT_MAX};
    for i:0..<n {
        p.stage[i]=l4_stage(c,m,y[..],t,p.cut[i],p.cut[i+1],i,n);p.exchange[i]=p.stage[i].mixture*x;
        p.send[i]=ifx i==n-1 then 4 else l4_boundary(c,m,p.cut[i+1]);
        j.stage[i]=PlcStage.{bytes=p.stage[i].leader,read=p.stage[i].read,out=p.send[i]};
        f.peer[i]=PlcPeer.{live=true,budget=m.budget,macs=m.read,read=m.read,fetch=m.read,verify=m.read};
        for g:0..<n {f.lat[i][g]=m.lat_out;f.bw[i][g]=m.bw_out;}
    }
    zero:[1]int;p.placed=plc_place(j,f,PlcGoal.latency,zero[0..<0]);
    if p.placed.status!=PlcStatus.ok {p.status=L4Status.place;return p;}
    p.latency=p.placed.latency;
    for i:0..<n {
        p.time[i]=p.placed.compute[i]+p.exchange[i];p.latency+=p.exchange[i];
        p.period=stg_max(p.period,p.time[i]+p.placed.hop[i+1]);
    }
    return p;
}
// The smallest mesh of identical peers that admits the model: the fewest
// peers (stages x width), and among as many peers the widest expert split
// (which reads the fewest experts a token), then the fewest stages. status
// split when no mesh up to 8 x 8 does.
l4_smallest :: (c:L4Config,m:L4Mesh)->L4Plan {
    best:=L4Plan.{status=L4Status.split};
    for w:[4]int.[8,4,2,1] {
        // One stage more each time, until the plan refuses the count.
        g:=m;g.width=w;g.stages=1;p:=l4_plan(c,g);
        while p.status!=L4Status.input {
            if p.status==L4Status.ok && (best.status!=L4Status.ok || g.stages*w<best.stages*best.width) {best=p;}
            g.stages+=1;p=l4_plan(c,g);
        }
    }
    return best;
}
```

### The smallest mesh of 256 GB peers, by hand

The model's tensors without the draft head and the vision encoder take
1,052,940,052,032 bytes, so at least 5 peers of 256 GB are needed.
A width must divide the 128 experts, so a mesh is stages × width
with width 1, 2, 4 or 8, and 5 peers can only be 5 stages of one peer.
That works: about 15 layers a stage, 15 × 14.76 GB = 221 GB plus caches.
The program agrees: 5 stages × 1 peer, layers 0–14, 15–29, 30–44, 45–59
and 60–71. (The vision encoder and the draft head are not placed; this
plan serves text.)

## Run it

<!-- source: psionic/examples/large4-mesh.coax example -->
```jai
main :: () {
    c:=l4_large4();inv:=l4_inventory(c,L4_PINNED_MANIFEST);
    print("inventory: %, % lines, % tensors, % bytes (pinned % and %), draft head % bytes\n",
        l4_status_name(inv.status),inv.lines,inv.tensors,inv.bytes,L4_PINNED_TENSORS,L4_PINNED_TENSOR_BYTES,inv.draft);
    names:=[7]string.["attention","indexer","experts","shared","router","dense","norms"];
    for l:[5]int.[0,1,3,17,39] {
        y:=l4_layer(c,l);total:=0;print("layer %:",l);
        for p:0..<L4_PARTS {total+=y.part[p];if y.part[p]>0 {print(" % %",names[p],y.part[p]);}}
        print(", % in all\n",total);
    }
    all:=0;for l:0..<c.layers {y:=l4_layer(c,l);for p:0..<L4_PARTS {all+=y.part[p];}}
    t:=l4_top(c);
    print("72 layers % bytes; embedding %, readout and norm %, vision %, lerope %; one routed expert % bytes\n",
        all,t.embed,t.head,t.vision,t.rope,l4_layer(c,1).one);

    // Meshes of 256 GB peers: for each expert width, the fewest stages that fit.
    print("mesh: 256 GB peers reading 400,000 MB/s; 20 us and 12,500 MB/s inside a group, 500 us and 1,250 MB/s between (assumed); 32,768 positions\n");
    for w:[4]int.[1,2,4,8] {
        for n:1..<L4_STAGES_MAX+1 {
            p:=l4_plan(c,l4c_mesh(n,w));
            if p.status!=L4Status.ok {continue;}
            print("  % x % = % peers: layers",n,w,n*w);for i:0..<n {print(" %-%",p.cut[i],p.cut[i+1]-1);}
            print("\n    leaders (GB)");for i:0..<n {print(" ");l4x_gb(p.stage[i].leader);}
            print("; other peers (GB) ");l4x_gb(p.stage[0].peer);
            print("\n    sends (bytes)");for i:0..<n {print(" %",p.send[i]);}
            print("; expert exchange a layer % ns\n    a token: ",l4_exchange(c,l4c_mesh(n,w)));l4x_ms(p.latency);
            print("; full pipeline, one token every ");l4x_ms(p.period);print("\n");
            break;
        }
    }
    for b:[2]int.[256000000000,150000000000] {
        m:=l4c_mesh(1,1);m.budget=b;s:=l4_smallest(c,m);
        print("smallest mesh of ");l4x_gb(b);print(" GB peers: % stages x % peers, a token ",s.stages,s.width);l4x_ms(s.latency);print("\n");
    }

    // The small model: 6 layers, layers 3 and 5 read layer 1's cache.
    src:=[6]int.[0,1,2,1,4,1];
    for seed:0..<3 {for n:2..<4 {for w:[2]int.[1,3] {
        r:=l4c_run(seed,6,src[..],n,w,4,-1,-1);
        print("small model, seed %, stages at",seed);for i:1..<n {print(" %",r.cut[i]);}
        print(", % expert peers: whole %, split %, equal bit for bit %, % bytes sent (counted %)\n",w,r.whole,r.split.ok,r.same,r.split.sent,r.want);
    }}}
}
```

```text
inventory: ok, 79 lines, 56563 tensors, 1060548427328 bytes (pinned 56563 and 1060548427328), draft head 4199938048 bytes
layer 0: attention 120348928 indexer 27163264 dense 453095424 norms 25600, 600633216 in all
layer 1: attention 120348928 indexer 27163264 experts 14499053568 shared 113273856 router 1572864 norms 25600, 14761438080 in all
layer 3: attention 123756800 indexer 27163264 experts 14499053568 shared 113273856 router 1572864 norms 25600, 14764845952 in all
layer 17: attention 120348928 experts 14499053568 shared 113273856 router 1572864 norms 25600, 14734274816 in all
layer 39: attention 117465344 indexer 27163264 experts 14499053568 shared 113273856 router 1572864 norms 24576, 14758553472 in all
72 layers 1047906874880 bytes; embedding 2516582400, readout and norm 2516594688, vision 3408437248, lerope 64; one routed expert 113273856 bytes
mesh: 256 GB peers reading 400,000 MB/s; 20 us and 12,500 MB/s inside a group, 500 us and 1,250 MB/s between (assumed); 32,768 positions
  5 x 1 = 5 peers: layers 0-14 15-29 30-44 45-59 60-71
    leaders (GB) 210.127 221.553 221.507 221.446 179.694; other peers (GB) 202.986
    sends (bytes) 24612 32840 34924 26696 4; expert exchange a layer 0 ns
    a token: 136.1 ms; full pipeline, one token every 27.4 ms
  3 x 2 = 6 peers: layers 0-24 25-50 51-71
    leaders (GB) 183.862 195.409 160.323; other peers (GB) 173.988
    sends (bytes) 32840 26696 4; expert exchange a layer 49833 ns
    a token: 98.4 ms; full pipeline, one token every 32.8 ms
  2 x 4 = 8 peers: layers 0-38 39-71
    leaders (GB) 151.364 130.872; other peers (GB) 137.741
    sends (bytes) 26696 4; expert exchange a layer 45900 ns
    a token: 77.5 ms; full pipeline, one token every 39.1 ms
  1 x 8 = 8 peers: layers 0-71
    leaders (GB) 153.541; other peers (GB) 128.679
    sends (bytes) 4; expert exchange a layer 43934 ns
    a token: 76.3 ms; full pipeline, one token every 76.3 ms
smallest mesh of 256.000 GB peers: 5 stages x 1 peers, a token 136.1 ms
smallest mesh of 150.000 GB peers: 2 stages x 4 peers, a token 77.5 ms
small model, seed 0, stages at 4, 1 expert peers: whole true, split true, equal bit for bit true, 528 bytes sent (counted 528)
small model, seed 0, stages at 4, 3 expert peers: whole true, split true, equal bit for bit true, 528 bytes sent (counted 528)
small model, seed 0, stages at 1 4, 1 expert peers: whole true, split true, equal bit for bit true, 800 bytes sent (counted 800)
small model, seed 0, stages at 1 4, 3 expert peers: whole true, split true, equal bit for bit true, 800 bytes sent (counted 800)
small model, seed 1, stages at 5, 1 expert peers: whole true, split true, equal bit for bit true, 528 bytes sent (counted 528)
small model, seed 1, stages at 5, 3 expert peers: whole true, split true, equal bit for bit true, 528 bytes sent (counted 528)
small model, seed 1, stages at 3 5, 1 expert peers: whole true, split true, equal bit for bit true, 1056 bytes sent (counted 1056)
small model, seed 1, stages at 3 5, 3 expert peers: whole true, split true, equal bit for bit true, 1056 bytes sent (counted 1056)
small model, seed 2, stages at 4, 1 expert peers: whole true, split true, equal bit for bit true, 528 bytes sent (counted 528)
small model, seed 2, stages at 4, 3 expert peers: whole true, split true, equal bit for bit true, 528 bytes sent (counted 528)
small model, seed 2, stages at 2 3, 1 expert peers: whole true, split true, equal bit for bit true, 1056 bytes sent (counted 1056)
small model, seed 2, stages at 2 3, 3 expert peers: whole true, split true, equal bit for bit true, 1056 bytes sent (counted 1056)
```

The 5 × 1 mesh is the fewest peers but the slowest token: each stage
reads its 4 routed experts a layer from one memory, 136.1 ms in all.
Width 8 spreads each layer's 4 experts over 4 peers, so the leader reads
one expert a layer and the token takes 76.3 ms, but uses 8 peers. With
150 GB peers the smallest mesh is 2 stages of 4 peers, 77.5 ms a token.
All of these times come from the assumed rates in the output, not from
measurement.

### The small model split, bit for bit

Large 4's weights do not fit here, so the split is checked on a small
model in the same pattern: chapter 85's latent attention and mixture
layers at its checks' shape (hidden 8, 5 experts, 2 a token), 6 layers,
with layers 3 and 5 reading layer 1's cache. A reader runs chapter 85's
attention but reads the source's rows and writes none.

<!-- source: psionic/lib/large4-mesh.coax run -->
```jai
// A small model in Large 4's pattern: chapter 85's latent attention and
// mixture layers at its checks' shape, where layer l reads the cache of
// layer src[l] (src[l] < l for a reader, which keeps no cache of its own).
// Layer l's weights are f[81 l ..] (attention's 20 values, then the
// mixture's 61) and c[646 l ..] (214 codes, then 432).
L4Toy :: struct {layers:int;src:[8]int;f:[]f32;c:[]u8;}
L4_F :: 81;L4_C :: 646;
l4_attn :: (t:L4Toy,l:int)->M4Attn {return m4_attn_view(t.f[l*L4_F..<t.f.count],t.c[l*L4_C..<t.c.count]);}
l4_moe :: (t:L4Toy,l:int)->M4Moe {return m4_moe_view(t.f[l*L4_F+20..<(l+1)*L4_F],t.c[l*L4_C+214..<(l+1)*L4_C]);}
// A reader's attention: chapter 85's m4_attention for one token at position
// p, except the cache is the source layer's, read and never written.
l4_read :: (s:M4Shape,w:M4Attn,p:int,x:[]f32,cache:[]f32,work:[]f32,fwork:[]f64)->bool {
    at:=m4_layout(s);kv:=work[0..<s.rope];normed:=work[at[1]..<at[2]];qa:=work[at[2]..<at[3]];q:=work[at[3]..<at[4]];
    latent:=work[at[4]..<at[5]];attended:=work[at[5]..<at[6]];out:=work[at[6]..<at[7]];
    ok:=m4_norm(x,w.norm,normed) && m4_linear(w.q_a,normed,qa) && m4_norm(qa,w.q_norm,qa) && m4_linear(w.q_b,qa,q);
    for head:0..<s.heads {
        r:=q[head*(s.nope+s.rope)+s.nope..<(head+1)*(s.nope+s.rope)];
        ok=ok && m4_rotate(r,p,kv[0..<s.rope]);
        for j:0..<s.rope {r[j]=kv[j];}
    }
    ok=ok && m4_latent(s,w.kv_b,q,cache,p,m4_softmax_scale(s)*m4_position_scale(p),latent,attended,fwork);
    return ok && m4_linear(w.o,attended,out) && m4_residual(x,out);
}
// One token through layers a..<b at position p. caches[l] is layer l's
// cache (a reader's source cache may be a stage's copy); peers and owner
// split each layer's experts (one peer: chapter 85's m4_moe).
l4_layers :: (s:M4Shape,t:L4Toy,a:int,b:int,p:int,x:[]f32,caches:[][]f32,peers:[][]XmPeer,owner:[][]int,work:[]f32,fwork:[]f64)->bool {
    for l:a..<b {
        if t.src[l]==l {if m4_attention(s,l4_attn(t,l),1,p,x,caches[l],work,fwork).status!=M4Status.ok {return false;}}
        else if !l4_read(s,l4_attn(t,l),p,x,caches[t.src[l]],work,fwork) {return false;}
        routes:[1]M4Route;home:[1]int;
        if peers[l].count==1 {if m4_moe(s,l4_moe(t,l),1,x,routes[..],work,fwork).status!=M4Status.ok {return false;}}
        else if xm_moe(s,l4_moe(t,l),peers[l],owner[l],1,home[..],x,routes[..],work,fwork).status!=XmStatus.ok {return false;}
    }
    return true;
}
// The model split at cut (stages of layers cut[i]..<cut[i+1]), token by
// token. Each stage owns the caches of its layers and a copy of each cache
// it reads from an earlier stage; between stages the hidden row and each
// such cache row travel as chapter 186's messages, checked on receipt.
// sent counts their bytes; refusal is the first receipt refused. tamper
// (a check's hook; -1: none) flips bit 0 of word tamper % 64 of message
// tamper / 64, counting from 0 in the order sent.
L4Run :: struct {ok:bool;sent:int;refusal:StgRefusal;}
L4_TOY_DIGEST :: 0x4c34;
l4_tamper :: (wire:[]u32,tamper:int,sent:int) {if tamper>=0 && tamper/64==sent {wire[tamper%64]^=1;}}
l4_split :: (s:M4Shape,t:L4Toy,cut:[]int,tokens:int,hidden:[]f32,caches:[][]f32,copies:[][]f32,peers:[][]XmPeer,owner:[][]int,work:[]f32,fwork:[]f64,wire:[]u32,tamper:int)->L4Run {
    r:=L4Run.{ok=true};h:=s.hidden;row:=m4_row(s);n:=cut.count-1;sent:=0;
    for p:0..<tokens {
        x:=hidden[p*h..<(p+1)*h];
        for i:0..<n {
            a:=cut[i];b:=cut[i+1];
            // This stage's view of the caches: its own, then its copies.
            view:[8][]f32;for l:0..<t.layers {view[l]=ifx l>=a then caches[l] else copies[i*8+l];}
            if !l4_layers(s,t,a,b,p,x,view[0..<t.layers],peers,owner,work,fwork) {r.ok=false;return r;}
            if i==n-1 {continue;}
            // To stage i + 1: the hidden row, then each source row it reads.
            k:=stg_pack(L4_TOY_DIGEST,b,p,h,x,wire);r.sent+=4*k;
            l4_tamper(wire,tamper,sent);sent+=1;
            got:=stg_receive(StgExpect.{model=L4_TOY_DIGEST,layer=b,start=p,width=h},wire[0..<k],x);
            if got.refusal!=StgRefusal.none {r.ok=false;r.refusal=got.refusal;return r;}
            for src:0..<b {
                needed:=false;for l:b..<t.layers {if t.src[l]==src && l!=src {needed=true;}}
                if !needed {continue;}
                from:=ifx src>=a then caches[src] else copies[i*8+src];
                k=stg_pack(L4_TOY_DIGEST+1+cast(u64)src,b,p,row,from[p*row..<(p+1)*row],wire);r.sent+=4*k;
                l4_tamper(wire,tamper,sent);sent+=1;
                got=stg_receive(StgExpect.{model=L4_TOY_DIGEST+1+cast(u64)src,layer=b,start=p,width=row},wire[0..<k],copies[(i+1)*8+src][p*row..<(p+1)*row]);
                if got.refusal!=StgRefusal.none {r.ok=false;r.refusal=got.refusal;return r;}
            }
        }
    }
    return r;
}
```

Run whole, it is one stage with every expert on one machine, using
chapter 85's `m4_moe`. Split, each stage receives the hidden row and each
needed cache row through chapter 186's checked messages and runs its
mixture layers through chapter 191's `xm_moe`. The final rows equal the
whole run's bit for bit, for every seed, cut and width in the checks.
With a cut at 4, a token sends the 8-value hidden row (68 bytes) and
layer 1's 7-value row (64 bytes), because layer 5 reads layer 1 across
the cut: 4 tokens × 132 = 528 bytes, as counted.

## When it is refused

<!-- source: psionic/examples/large4-mesh-refuse.coax refuse -->
```jai
main :: () {
    c:=l4_large4();
    m:=L4_PINNED_MANIFEST;first:=0;for i:0..<m.count {if m[i]=='\n' {first=i+1;break;}}
    r:=l4_inventory(c,m[first..<m.count]);
    print("first line dropped: %, derived kind % missing, % lines\n",l4_status_name(r.status),r.bad,r.lines);
    extra:[8192]u8;n:=m4_put(extra[..],0,m);n=m4_put(extra[..],n,"extra BF16 1\n");
    r=l4_inventory(c,cast(string)extra[0..<n]);
    print("one tensor added: %, % tensors and % bytes\n",l4_status_name(r.status),r.tensors,r.bytes);
    r=l4_inventory(c,"layers.3-2.attention.wq.weight F8_E4M3 8192,6144\n");
    print("a backwards range: %, line %\n",l4_status_name(r.status),r.line);
    p:=l4_plan(c,l4c_mesh(2,3));print("3 expert peers a stage: %\n",l4_status_name(p.status));
    g:=l4c_mesh(4,1);g.budget=1000000000;p=l4_plan(c,g);
    print("1 GB peers: %, layer % needs % bytes more\n",l4_status_name(p.status),p.layer,p.gap);
    g.budget=200000000000;p=l4_plan(c,g);print("4 stages of one 200 GB peer: %\n",l4_status_name(p.status));
    g=l4c_mesh(2,4);g.bw_out=0;p=l4_plan(c,g);print("2 groups with no link between them: %, %\n",l4_status_name(p.status),plc_status_name(p.placed.status));
    g=l4c_mesh(1,1);g.budget=10000000000;print("smallest mesh of 10 GB peers: %\n",l4_status_name(l4_smallest(c,g).status));
    src:=[4]int.[0,1,2,1];s:=l4c_run(1,4,src[..],2,2,3,9,-1);
    print("one bit changed in the first boundary message: run stopped %, refused: %\n",!s.split.ok,stg_refusal_name(s.split.refusal));
}
```

```text
first line dropped: manifest line, derived kind 0 missing, 78 lines
one tensor added: totals differ from the pins, 56564 tensors and 1060548427330 bytes
a backwards range: manifest line, line 0
3 expert peers a stage: width is not 1, 2, 4 or 8
1 GB peers: a layer fits on no peer, layer 71 needs 13779524992 bytes more
4 stages of one 200 GB peer: no split fits
2 groups with no link between them: placement refused, no linked chain of peers
smallest mesh of 10 GB peers: no split fits
one bit changed in the first boundary message: run stopped true, refused: checksum
```

The program exits with status 0 after printing every refusal. Layers are
checked from the last down. A 1 GB peer misses layer 71 by
13,779,524,992 bytes: the layer's 14,758,553,472 bytes, a copy of layer
35's cache (512 × 32,768 bytes) and its own index keys (128 × 32,768),
less the budget. Four 200 GB
peers hold 800 GB, less than the model, so no split fits.

## Checks

The `#test` and `#forall` checks in [`large4-mesh.coax`](../lib/large4-mesh.coax)
cover:

- **Inventory.** The 61 derived lines match the fixture written from the
  headers, and the totals match the pins. A dropped line, an extra
  tensor, and malformed lines (unknown dtype, zero or missing
  dimension, a backwards or open range, a trailing comma) are refused.
- **Bytes.** Per-layer bytes by part, compared with sums over the
  headers made by an independent script, add up to the pinned total.
- **Plan.** The planner's slowest stage equals a brute force over every
  split into 1 to 3 stages, at widths 1, 2, 4 and 8. Hand-worked cut
  bytes, cache copies, exchange times, cuts and times for five meshes,
  the smallest mesh at five peer sizes, and refusals for each input
  bound, a width other than 1, 2, 4 or 8, memory (the layer and the
  bytes missing), no split and unlinked groups.
- **Run.** A reader with its source's weights equals chapter 85's own
  attention bit for bit. The small model split over 1 to 3 stages and 1
  to 3 expert peers equals the whole run bit for bit, the whole run
  equals a plain loop over tokens and layers written in the checks, and
  the bytes sent equal the count. A changed bit in a hidden-row or a
  cache-row message is refused (checksum, version, position or layer,
  by the word changed), and a layer whose arithmetic overflows stops the
  run whole and split.

`coax mutate psionic/lib/large4-mesh.coax --root-only` leaves no
surviving mutant.

## What remains

- Large 4's real weights on real machines, over chapter 188's transport,
  with per-hop latency and per-stage memory measured rather than modeled.
- Parity with a whole-model reference on selected layers and tokens
  under [chapter 17](ch017-qualify-honestly.md)'s rules.
- Large 4's own forward pass: NoPE layers, the lightning indexer's
  selection and FP8 block scales in one layer. The small model uses
  chapter 85's arithmetic.
- Stage packages that fetch only their own tensors, checked as in
  chapter 187.
- Trust rules from chapter 194 applied to the groups, and routing taken
  from real traces instead of an assumed spread.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Mistral target](../docs/targets/mistral.md)
- `mistralai/Mistral-Large-4-1T-A52B` at revision
  `395730aefd2f928daaf7b69fbd9fefab0ee69b64`: params.json,
  consolidated.safetensors.index.json and the 54 shard headers.
