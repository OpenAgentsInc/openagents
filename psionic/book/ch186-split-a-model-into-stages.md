# 186. Split a model into stages

> **Status: partly implemented.** A stage planner, a stage boundary
> contract and a pipelined run of chapter 81's Ministral decoder work on
> the CPU (profiles stage-plan-v1 and stage-run-v1). The planner cuts the
> layers into contiguous stages that fit per-stage memory budgets and
> balances their work, and it gives a typed refusal when no split fits. The
> boundary contract carries hidden-state rows bound to the artifact digest,
> the receiver's first layer, the position and the protocol version. The
> stages run in one process. A split run equals the unsplit forward bit for
> bit on a synthetic six-layer model and on Ministral 3 3B's first two
> original layers. Still to build: a cached continuation (chapter 81 has
> no cache yet), stages on separate threads or machines, a `proto/` schema
> for the message, all 26 layers on original weights, and the residual
> streams and sidebands that DeepSeek and Mistral Large boundaries carry.

Part XX splits a model that no single machine holds. The simplest split is
the pipeline: cut the decoder's layers into contiguous ranges and give
each range to a stage. Stage 0 also holds the token embedding. The last
stage also holds the final norm and the readout. Between two stages, only
the residual stream crosses: one row of `width` values for each token.

This chapter builds three things:

- A planner that chooses the cut. Each stage has a memory budget, and the
  planner finds the contiguous split whose slowest stage does the least work
  per token among the splits that fit.
- A boundary contract. It says exactly what crosses and what each stage
  keeps, and it refuses a message meant for another model, layer, position
  or protocol version.
- A runner that passes requests through the stages and checks that the
  split changes no bit of the output.

The planner and the contract are in an `#authority` file,
[`stage-plan.coax`](../lib/stage-plan.coax), because they admit and refuse.
The runner is [`stage-run.coax`](../lib/stage-run.coax). It calls
[chapter 81](ch081-execute-ministral-4.md)'s `ms_embed`, `ms_layer` and
`ms_readout` and adds no arithmetic of its own.

```shell
coax/bin/coax run psionic/examples/stage-split.coax
coax/bin/coax run psionic/examples/stage-split-refuse.coax
psionic/check.sh --only stage-plan stage-run
COAX_CC=gcc psionic/tests/check_stage_slices.sh ~/models/ministral-3-3b-instruct-2512-bf16-b6d637be
```

## What a stage costs

The planner sees a model as a table. For each layer it has four numbers:
the weight bytes, the cache bytes per position, the multiply-adds per
token outside attention, and attention's multiply-adds per cached
position. It also has the shared assets: the embedding, the readout
matrix, the final norm, the logits buffers, the readout's multiply-adds,
and the activation bytes that every stage allocates for each position.
When the readout is tied to the embedding (Ministral's is), a stage that
holds both keeps one copy. A split puts them on different stages, so each
of those stages keeps its own copy.

<!-- source: psionic/lib/stage-plan.coax model -->
```jai
// What the planner knows about a decoder, layer by layer: bytes of weights,
// bytes its cache keeps per position, multiply-adds one token costs apart
// from attention, and attention's multiply-adds per cached position. The
// shared assets: the embedding (first stage), the readout matrix, final
// norm and logits buffers (last stage), the readout's multiply-adds per
// token, and the activation bytes per position every stage allocates. A
// tied readout is the embedding itself: a stage holding both keeps one copy.
StgModel :: struct {
    layers:int;weight:[128]int;kv:[128]int;macs:[128]int;attend:[128]int;
    embed:int;head:int;norm:int;head_macs:int;act:int;out:int;tied:bool;
}
stg_in :: (x:int,lo:int,hi:int)->bool {return x>=lo && x<=hi;}
// The larger of two counts below 2^53, without a comparison.
stg_max :: (a:int,b:int)->int {return (a+b+cast(int)abs(f64(a-b)))/2;}
stg_valid :: (m:StgModel)->bool {
    if !stg_in(m.layers,1,STG_LAYERS_MAX) {return false;}
    for j:0..<m.layers {
        if !stg_in(m.weight[j],1,STG_BIG) || !stg_in(m.macs[j],0,STG_BIG) || !stg_in(m.kv[j],0,STG_RATE) || !stg_in(m.attend[j],0,STG_RATE) {return false;}
    }
    return stg_in(m.embed,1,STG_BIG) && stg_in(m.head,1,STG_BIG) && stg_in(m.norm,0,STG_BIG) && stg_in(m.head_macs,0,STG_BIG) &&
        stg_in(m.act,0,STG_RATE) && stg_in(m.out,0,STG_BIG);
}
// The bytes of a stage running layers from..<to over `context` positions.
stg_bytes :: (m:StgModel,from:int,to:int,first:bool,last:bool,context:int)->int {
    b:=m.act*context;
    for j:from..<to {b+=m.weight[j]+m.kv[j]*context;}
    if first {b+=m.embed;}
    if last {b+=m.norm+m.out;if !first || !m.tied {b+=m.head;}}
    return b;
}
// Its multiply-adds for one token at the end of the context.
stg_macs :: (m:StgModel,from:int,to:int,last:bool,context:int)->int {
    c:=0;for j:from..<to {c+=m.macs[j]+m.attend[j]*context;}
    if last {c+=m.head_macs;}
    return c;
}
```

For Ministral 3 3B the table follows from the shape. One layer has 4,096
query rows (32 heads of 128), 1,024 key rows and 1,024 value rows (8
heads), a SwiGLU block of 9,216, and width 3,072:

- Projections: 2 × 4,096 × 3,072 + 2 × 1,024 × 3,072 + 3 × 9,216 × 3,072 =
  116,391,936 multiply-adds per token. That is also the layer's matrix
  parameter count.
- Weights: two norms add 2 × 3,072 parameters. At two bytes each (BF16),
  that is 232,796,160 bytes.
- Cache: keys and values for 8 heads of 128 in f32, which is
  2 × 1,024 × 4 = 8,192 bytes per position. That is what `ms_layer` keeps,
  and the f32 cache keeps a split bit-exact.
- Attention: each of the 32 query heads does a 128-wide dot product with
  every cached key and a 128-wide weighted sum of the values, which is
  2 × 4,096 = 8,192 multiply-adds per cached position.
- Shared assets: the embedding is 131,072 × 3,072 × 2 = 805,306,368 bytes,
  and the readout costs 131,072 × 3,072 = 402,653,184 multiply-adds per
  token. The runner's buffers per position are two residual rows and one
  layer's queries, keys and values: 4 × (2 × 3,072 + 4,096 + 2,048) =
  49,152 bytes. The last stage also holds logits and readout work:
  4 × (2 × 131,072 + 3,072) = 1,060,864 bytes.

<!-- source: psionic/lib/stage-run.coax shape -->
```jai
// Chapter 81's decoder as the planner sees it, per layer: BF16 weights, an
// f32 cache of keys and values for every key-value head, the projections'
// multiply-adds, and attention's (scores and weighted values for every
// query head) per cached position. The runner's buffers per position: two
// residual rows, and one layer's queries, keys and values. The readout is
// the tied embedding; its logits and work are vocab + width + vocab f32.
stg_ministral :: (s:MsShape)->StgModel {
    m:StgModel;m.layers=s.layers;d:=s.width;q:=s.heads*s.head_dim;kv:=s.kv_heads*s.head_dim;
    macs:=2*q*d+2*kv*d+3*s.hidden*d;
    for j:0..<s.layers {m.weight[j]=2*(macs+2*d);m.kv[j]=4*2*kv;m.macs[j]=macs;m.attend[j]=2*q;}
    m.embed=2*s.vocab*d;m.head=m.embed;m.norm=2*d;m.head_macs=s.vocab*d;
    m.act=4*(2*d+q+2*kv);m.out=4*(2*s.vocab+d);m.tied=true;
    return m;
}
```

As a check, add up the bytes that one stage holding the whole model would
load: 26 layers of weights, the embedding and the final norm. The total is
6,858,012,672 bytes, which is twice chapter 81's inventory count of
3,429,006,336 text parameters. A test asserts this. It ties the planner's
table to the pinned checkpoint and not only to the shape.

## A worked split

Take a context of 4,096 positions:

- One layer: 232,796,160 weight bytes plus 4,096 × 8,192 = 33,554,432 cache
  bytes, which is 266,350,592 bytes.
- One layer's work per token at the end of the context:
  116,391,936 + 4,096 × 8,192 = 149,946,368 multiply-adds.
- Each stage's buffers: 4,096 × 49,152 = 201,326,592 bytes.

With two stages and plenty of memory, the question is where to cut. A naive
13 + 13 split ignores the readout, which costs 402,653,184 multiply-adds per
token, about 2.7 layers. It lands on the last stage. If stage 0 takes k
layers:

| k | stage 0 multiply-adds | stage 1 multiply-adds | slowest |
| --- | --- | --- | --- |
| 13 | 1,949,302,784 | 13 × 149,946,368 + 402,653,184 = 2,351,955,968 | 2,351,955,968 |
| 14 | 2,099,249,152 | 2,202,009,600 | 2,202,009,600 |
| 15 | 2,249,195,520 | 2,052,063,232 | 2,249,195,520 |

The best cut is after layer 14. Each stage's bytes are its layers, its
buffers and its share of the shared assets:

- Stage 0: 14 × 266,350,592 + 805,306,368 (embedding) + 201,326,592 =
  4,735,541,248 bytes.
- Stage 1: 12 × 266,350,592 + 805,306,368 (its own copy, as readout) +
  6,144 (final norm) + 1,060,864 + 201,326,592 = 4,203,907,072 bytes.

Memory can force a slower split. With 4.5 GB a stage, 14 layers no longer
fit on stage 0 (4,735,541,248 bytes), and 12 layers plus the readout no
longer fit on stage 1 (4,736,608,256 bytes). Only 13 + 13 fits: it needs
4,469,190,656 bytes and 4,470,257,664 bytes, and its slowest stage does
2,351,955,968 multiply-adds per token.

The split needs more memory than one machine would. One 8 GB stage holds
the model in 7,932,815,360 bytes. Two stages need 8,939,448,320 bytes in
all: a second embedding copy (805,306,368) and a second set of buffers
(201,326,592). So two 4.4 GB stages are refused before any search. They are
139,448,320 bytes short. Budgets of 4.46 GB and 4.48 GB pass that total
test, but they still have no cut: stage 0 holds at most 12 layers, and the
14 left need 4,736,608,256 bytes on stage 1. That is a separate refusal.

## The planner

The planner checks its inputs and then finds the split. Each refusal says
what is wrong:

1. **Model, stages and context.** It refuses a model table outside its
   bounds, a stage count that is not 1 to 16 or is more than the layers, a
   budget outside 0 to 2^56, and a context outside 1 to 2^20.
2. **The smallest end stages.** The smallest first stage is the embedding,
   one layer and the buffers. The smallest last stage is one layer, the
   readout and the buffers. If either is over its stage's budget, the
   planner refuses with `first` or `last`, and gives the bytes short.
3. **The total.** Every split holds every layer, each shared asset once
   (twice for a tied readout across stages) and N sets of buffers. If the
   budgets add up to less, the planner refuses with `total`.
4. **The search.** Only then does it search. `best[i][b]` is the smallest
   possible slowest stage when stages 0 to i cover layers 0 to b. The
   planner tries each start a for stage i and keeps the start whose stage
   fits and whose slowest stage is smallest. Running sums make each try
   constant time, so a plan costs N × L² steps. If no split fits, the
   refusal is `split`.

On a tie the earlier start wins. The search is exact: it finds the same
slowest stage as a brute force that tries every split.

<!-- source: psionic/lib/stage-plan.coax plan -->
```jai
// Stage i runs layers cut[i]..<cut[i+1]; its bytes, the part of them its
// cache takes, and its multiply-adds per token. slowest is the largest of
// those multiply-adds. cause: the bytes the refused stage or total needed
// beyond its budget.
StgPlan :: struct {status:StgStatus;stages:int;cause:int;slowest:int;cut:[17]int;bytes:[16]int;kv:[16]int;macs:[16]int;}
stg_plan :: (m:StgModel,budget:[]int,context:int)->StgPlan {
    n:=budget.count;p:=StgPlan.{stages=n};
    if !stg_valid(m) {p.status=StgStatus.model;return p;}
    if n<1 || n>STG_STAGES_MAX || n>m.layers {p.status=StgStatus.stages;return p;}
    for b:budget {if !stg_in(b,0,STG_BUDGET_MAX) {p.status=StgStatus.stages;return p;}}
    if !stg_in(context,1,STG_CONTEXT_MAX) {p.status=StgStatus.context;return p;}
    L:=m.layers;
    // The smallest first stage and the smallest last stage must fit.
    need:=stg_bytes(m,0,1,true,n==1,context);
    if need>budget[0] {p.status=StgStatus.first;p.cause=need-budget[0];return p;}
    need=stg_bytes(m,L-1,L,n==1,true,context);
    if need>budget[n-1] {p.status=StgStatus.last;p.cause=need-budget[n-1];return p;}
    // Every split holds every layer, each shared asset, and N activation buffers.
    need=stg_bytes(m,0,L,true,true,context)+(n-1)*m.act*context;
    if n>1 && m.tied {need+=m.head;}
    room:=0;for b:budget {room+=b;}
    if need>room {p.status=StgStatus.total;p.cause=need-room;return p;}
    // Running sums of the layers' bytes and multiply-adds; an empty range
    // gives a stage's shared part.
    bytes:[129]int;macs:[129]int;
    for j:0..<L {bytes[j+1]=bytes[j]+m.weight[j]+m.kv[j]*context;macs[j+1]=macs[j]+m.macs[j]+m.attend[j]*context;}
    // best[i][b]: the smallest slowest stage over stages 0..i when stage i
    // ends before layer b (-1: none fits). On a tie the earlier start wins.
    best:[16][129]int;start:[16][129]int;
    for i:0..<n {for b:1..<L+1 {
        best[i][b]=-1;
        for a:i..<b {
            reach:=ifx i==0 then a==0 else best[i-1][a]>=0;
            if !reach || stg_bytes(m,a,a,i==0,i==n-1,context)+bytes[b]-bytes[a]>budget[i] {continue;}
            c:=stg_macs(m,a,a,i==n-1,context)+macs[b]-macs[a];
            if i>0 {c=stg_max(c,best[i-1][a]);}
            if best[i][b]<0 || c<best[i][b] {best[i][b]=c;start[i][b]=a;}
        }
    }}
    if best[n-1][L]<0 {p.status=StgStatus.split;return p;}
    p.cut[n]=L;p.slowest=best[n-1][L];
    for k:0..<n {i:=n-1-k;p.cut[i]=start[i][p.cut[i+1]];}
    for i:0..<n {
        a:=p.cut[i];b:=p.cut[i+1];
        p.bytes[i]=stg_bytes(m,a,b,i==0,i==n-1,context);
        p.macs[i]=stg_macs(m,a,b,i==n-1,context);
        for j:a..<b {p.kv[i]+=m.kv[j]*context;}
    }
    return p;
}
```

## The boundary contract

Only one thing crosses a boundary: the residual stream after the sender's
last layer. That is `count` rows of `width` f32 values for positions
`start` to `start + count`. The bytes are those values plus a 36-byte
header. For Ministral 3 3B, one token is 12,288 bytes plus the header, so
12,324 bytes. Ten tokens are 122,916 bytes. The cache does not cross: each
stage keeps the keys and values of its own layers, and the planner counts
them in that stage's bytes.

The header binds the rows to their meaning:

- the protocol version;
- the artifact digest: the first 8 bytes of the pinned file's SHA-256 for
  Ministral 3 3B, `0x722430146632596c`;
- the receiver's first layer, which is the sender's last layer plus one;
- the first position;
- the row count, the width and the dtype.

A checksum covers every word. The receiver checks the header in a fixed
order and writes nothing when it refuses. It refuses a message that:

- is cut short or has extra words;
- has another version;
- was made by another model;
- is meant for another layer or position;
- has another width or dtype;
- has a count outside 1 to 64;
- does not match its checksum;
- does not fit in the receiver's buffer.

<!-- source: psionic/lib/stage-plan.coax boundary -->
```jai
// What crosses from one stage to the next: the residual stream after the
// sender's last layer, `count` rows of `width` f32 values for positions
// start..<start+count. Nothing else crosses: each stage keeps the cache of
// its own layers. The message is 32-bit words: version, the model's digest
// (low and high halves), the receiver's first layer, start, count, width,
// dtype, a checksum, then the values' bits.
StgRefusal :: enum u8 {none;partial;version;model;layer;position;geometry;count;excess;checksum;room;}
StgExpect :: struct {model:u64;layer:int;start:int;width:int;}
StgReceived :: struct {refusal:StgRefusal;count:int;}
stg_message_bytes :: (count:int,width:int)->int #export_c {return 4*(STG_HEAD+count*width);}
// A chained hash over every word but word 8, which holds it.
stg_sum :: (w:[]u32)->u32 {
    h:=0;for k:0..<w.count {if k!=8 {h=(h*31+cast(int)w[k]+1)%STG_HASH;}}
    return cast(u32)h;
}
// Pack rows for the stage whose first layer is `layer`. Returns the words
// written, or -1 when the rows are not whole rows of 1 to 64 positions of a
// width up to 8,192, the layer is not after a first stage, start is
// negative, or out is too small.
stg_pack :: (model:u64,layer:int,start:int,width:int,rows:[]f32,out:[]u32)->int {
    if !stg_in(width,1,STG_WIDTH_MAX) || layer<1 || start<0 {return -1;}
    count:=rows.count/width;n:=STG_HEAD+rows.count;
    if !stg_in(count,1,STG_TOKENS_MAX) || count*width!=rows.count || out.count<n {return -1;}
    head:=[8]u32.[STG_VERSION,cast(u32)(model&4294967295),cast(u32)(model>>32),cast(u32)layer,cast(u32)start,cast(u32)count,cast(u32)width,STG_F32];
    for k:0..<8 {out[k]=head[k];}
    for k:0..<rows.count {out[STG_HEAD+k]=f32_bits(rows[k]);}
    out[8]=stg_sum(out[0..<n]);
    return n;
}
// The receiver's checks, in order: a whole header, the version, the model,
// its own first layer, the next position it expects, its width and f32, a
// count of 1 to 64 rows, exactly that many values, the checksum.
stg_check :: (want:StgExpect,wire:[]u32)->StgRefusal {
    if wire.count<STG_HEAD {return StgRefusal.partial;}
    if wire[0]!=STG_VERSION {return StgRefusal.version;}
    if wire[1]!=cast(u32)(want.model&4294967295) || wire[2]!=cast(u32)(want.model>>32) {return StgRefusal.model;}
    if cast(int)wire[3]!=want.layer {return StgRefusal.layer;}
    if cast(int)wire[4]!=want.start {return StgRefusal.position;}
    if cast(int)wire[6]!=want.width || wire[7]!=STG_F32 {return StgRefusal.geometry;}
    count:=cast(int)wire[5];
    if !stg_in(count,1,STG_TOKENS_MAX) {return StgRefusal.count;}
    n:=STG_HEAD+count*want.width;
    if wire.count<n {return StgRefusal.partial;}
    if wire.count>n {return StgRefusal.excess;}
    if wire[8]!=stg_sum(wire) {return StgRefusal.checksum;}
    return StgRefusal.none;
}
// Check, then write the rows back bit for bit. Nothing is written on a
// refusal.
stg_receive :: (want:StgExpect,wire:[]u32,out:[]f32)->StgReceived {
    r:=StgReceived.{refusal=stg_check(want,wire)};
    if r.refusal!=StgRefusal.none {return r;}
    count:=cast(int)wire[5];
    if out.count<count*want.width {r.refusal=StgRefusal.room;return r;}
    for k:0..<count*want.width {out[k]=f32_from_bits(wire[STG_HEAD+k]);}
    r.count=count;return r;
}
```

This is a planner-level contract. Every model so far crosses with one
hidden-state tensor. The DeepSeek V4.1 plan has four residual streams,
indexer source layers and Engram output, and Mistral Large's layer sharing
is similar. Those boundaries need a list of typed parts and a `proto/`
schema, and neither is built.

## Running the stages

Each logical stage owns its layers, two residual buffers and its work
area. The runner passes requests through the stages like a bucket brigade:
at tick t, stage k works on request t − k. Within a tick the stages run
from last to first. That way a stage reads its one-slot mailbox before the
stage before it writes the next request there. The run reports the order,
the number of messages and the bytes that crossed.

<!-- source: psionic/lib/stage-run.coax run -->
```jai
// Why a run stopped: its inputs, a boundary message a stage refused, or the
// decoder (chapter 81's status). stage and request say where.
StgRunStatus :: enum u8 {ok;shape;capacity;refused;decoder;}
StgRun :: struct {
    status:StgRunStatus;refusal:StgRefusal;decoder:MsResult;stage:int;request:int;
    ticks:int;messages:int;bytes:int;order:[23][16]int;
}
// Floats each stage owns for requests of up to `tokens` tokens: two
// residual buffers, and work for a layer or the readout. Words of one
// boundary mailbox.
stg_stage_floats :: (s:MsShape,tokens:int)->int #export_c {
    w:=ms_layer_work(s,tokens);r:=s.width+s.vocab;
    return 2*tokens*s.width+stg_max(w,r);
}
stg_mailbox_words :: (s:MsShape,tokens:int)->int #export_c {return STG_HEAD+tokens*s.width;}
// Run requests (tokens[starts[r]..<starts[r+1]]) through stages cut[k]..<cut[k+1].
// digests[k] is the artifact stage k loaded; the sender stamps its own.
// logits[r*vocab..] receives request r's last-position logits.
stg_pipeline :: (s:MsShape,digests:[]u64,embed:[]u8,norm:[]u8,layers:[]MsLayer,cut:[]int,
                 tokens:[]int,starts:[]int,logits:[]f32,mem:[]f32,wire:[]u32)->StgRun {
    run:StgRun;n:=cut.count-1;R:=starts.count-1;
    for t:0..<STG_TICKS_MAX {for k:0..<16 {run.order[t][k]=-1;}}
    if !ms_shape_valid(s) || layers.count!=s.layers || n<1 || n>STG_STAGES_MAX || digests.count!=n || cut[0]!=0 || cut[n]!=s.layers ||
        R<1 || R>STG_REQUESTS_MAX || starts[0]!=0 || starts[R]!=tokens.count {run.status=StgRunStatus.shape;return run;}
    for k:0..<n {if cut[k+1]<=cut[k] {run.status=StgRunStatus.shape;return run;}}
    longest:=starts[1];
    for r:0..<R {
        c:=starts[r+1]-starts[r];
        if c<1 || c>STG_TOKENS_MAX {run.status=StgRunStatus.shape;run.request=r;return run;}
        longest=stg_max(longest,c);
    }
    F:=stg_stage_floats(s,longest);B:=stg_mailbox_words(s,longest);d:=s.width;
    if logits.count<R*s.vocab || mem.count<n*F || wire.count<(n-1)*B {run.status=StgRunStatus.capacity;return run;}
    run.ticks=R+n-1;
    for t:0..<run.ticks {
        // Downstream first, so a stage reads its mailbox before the stage
        // before it writes the next request there.
        for j:0..<n {
            k:=n-1-j;r:=t-k;
            if r<0 || r>=R {continue;}
            run.order[t][k]=r;run.stage=k;run.request=r;
            count:=starts[r+1]-starts[r];rows:=count*d;own:=mem[k*F..<(k+1)*F];
            a:=own[0..<rows];b:=own[longest*d..<longest*d+rows];work:=own[2*longest*d..<F];x:=a;
            if k==0 {
                e:=ms_embed(s,embed,tokens[starts[r]..<starts[r+1]],x);
                if e.status!=MsStatus.ok {run.status=StgRunStatus.decoder;run.decoder=e;return run;}
            } else {
                want:=StgExpect.{model=digests[k],layer=cut[k],start=0,width=d};
                got:=stg_receive(want,wire[(k-1)*B..<k*B][0..<STG_HEAD+rows],x);
                if got.refusal!=StgRefusal.none {run.status=StgRunStatus.refused;run.refusal=got.refusal;return run;}
            }
            // The stage's layers, back and forth between its two buffers:
            // even steps a to b, odd steps b to a.
            step:=0;
            for l:cut[k]..<cut[k+1] {
                odd:=step%2==1;step+=1;
                e:=ms_layer(s,layers[l],ifx odd then b else a,count,ifx odd then a else b,work[0..<ms_layer_work(s,count)]);
                if e.status!=MsStatus.ok {e.cause=l;run.status=StgRunStatus.decoder;run.decoder=e;return run;}
            }
            if step%2==1 {x=b;}
            if k==n-1 {
                e:=stg_readout(s,embed,norm,x[rows-d..<rows],logits[r*s.vocab..<(r+1)*s.vocab],work,STG_READOUT_ROWS);
                if e.status!=MsStatus.ok {run.status=StgRunStatus.decoder;run.decoder=e;return run;}
            } else {
                w:=stg_pack(digests[k],cut[k+1],0,d,x,wire[k*B..<(k+1)*B]);
                run.messages+=1;run.bytes+=4*w;
            }
        }
    }
    run.stage=0;run.request=0;
    return run;
}
```

Chapter 81's `ms_readout` reads at most 32,768 rows at a time, which is
chapter 7's limit for a matrix-vector product. The last stage reads out the
131,072-entry vocabulary in chunks. Each logit is its own row's dot
product, so chunking changes no bit.

<!-- source: psionic/lib/stage-run.coax readout -->
```jai
// Chapter 81's readout takes at most 32,768 rows at a time (chapter 7's
// product's limit), so the vocabulary is read out in chunks of `chunk`
// rows. Each logit is its own row's dot product, so the chunking changes
// no bit.
STG_READOUT_ROWS :: 32768;
stg_min :: (a:int,b:int)->int {return a+b-stg_max(a,b);}
stg_readout :: (s:MsShape,embed:[]u8,norm:[]u8,row:[]f32,logits:[]f32,work:[]f32,chunk:int)->MsResult {
    if chunk<1 || chunk>STG_READOUT_ROWS {return MsResult.{status=MsStatus.shape};}
    if embed.count!=2*s.vocab*s.width || logits.count<s.vocab {return MsResult.{status=MsStatus.shape};}
    lo:=0;
    while lo<s.vocab {
        n:=stg_min(s.vocab-lo,chunk);
        e:=ms_readout(s,embed[2*lo*s.width..<2*(lo+n)*s.width],n,norm,row,logits[lo..<lo+n],work);
        if e.status!=MsStatus.ok {e.cause=lo;return e;}
        lo+=chunk;
    }
    return MsResult.{written=s.vocab};
}
```

The synthetic model is chapter 81's formula with up to six layers. Its first
two layers are chapter 81's tiny model byte for byte, so a two-stage run of
it can be checked against chapter 81's float64 Python reference.

<!-- source: psionic/lib/stage-run.coax tiny -->
```jai
// Chapter 81's synthetic model with up to six layers: the same formula, the same
// order of tensors, so its first two layers are chapter 81's.
StgTiny :: struct {shape:MsShape;embed:[]u8;norm:[]u8;layers:[6]MsLayer;}
stg_tiny_bytes :: (layers:int)->int {return 2*(16*8+8+layers*(16+2*16*8+2*8*8+3*8*12));}
stg_tiny :: (layers:int,bytes:[]u8)->StgTiny {
    t:StgTiny;t.shape=ms_tiny();t.shape.layers=layers;s:=t.shape;d:=s.width;q:=s.heads*s.head_dim;kv:=s.kv_heads*s.head_dim;
    at:=ms_tiny_fill(bytes,0,s.vocab*d,0,false);t.embed=bytes[0..<at];
    b:=at;at=ms_tiny_fill(bytes,at,d,99,true);t.norm=bytes[b..<at];
    for l:0..<layers {
        k:=1+l*7;w:MsLayer;
        b=at;at=ms_tiny_fill(bytes,at,q*d,k,false);w.wq=bytes[b..<at];
        b=at;at=ms_tiny_fill(bytes,at,kv*d,k+1,false);w.wk=bytes[b..<at];
        b=at;at=ms_tiny_fill(bytes,at,kv*d,k+2,false);w.wv=bytes[b..<at];
        b=at;at=ms_tiny_fill(bytes,at,d*q,k+3,false);w.wo=bytes[b..<at];
        b=at;at=ms_tiny_fill(bytes,at,s.hidden*d,k+4,false);w.w1=bytes[b..<at];
        b=at;at=ms_tiny_fill(bytes,at,d*s.hidden,k+5,false);w.w2=bytes[b..<at];
        b=at;at=ms_tiny_fill(bytes,at,s.hidden*d,k+6,false);w.w3=bytes[b..<at];
        b=at;at=ms_tiny_fill(bytes,at,d,100+2*l,true);w.attention_norm=bytes[b..<at];
        b=at;at=ms_tiny_fill(bytes,at,d,101+2*l,true);w.ffn_norm=bytes[b..<at];
        t.layers[l]=w;
    }
    return t;
}
```

## The example

The [example](../examples/stage-split.coax) plans Ministral 3 3B's stages
and prints the hand-worked numbers above. Then it plans the synthetic model
under three 6,000-byte budgets, runs four requests through the plan's
stages, and compares every logit with chapter 81's unsplit `ms_forward`.

<!-- source: psionic/examples/stage-split.coax plan-example -->
```jai
main :: () {
    m:=stg_ministral(MS_3B);C:=4096;
    print("Ministral 3 3B at % positions\n",C);
    print("one layer: % weight bytes, % cache bytes, % multiply-adds a token\n",m.weight[0],m.kv[0]*C,m.macs[0]+m.attend[0]*C);
    print("readout: % multiply-adds a token; buffers % bytes a stage\n",m.head_macs,m.act*C);
    two:=[2]int.[6000000000,6000000000];show("two stages of 6 GB",stg_plan(m,two[..],C));
    two=[2]int.[4500000000,4500000000];show("two stages of 4.5 GB",stg_plan(m,two[..],C));
    four:=[4]int.[3000000000,3000000000,3000000000,3000000000];show("four stages of 3 GB",stg_plan(m,four[..],C));
    one:=[1]int.[8000000000];show("one stage of 8 GB",stg_plan(m,one[..],C));
    two=[2]int.[4400000000,4400000000];show("two stages of 4.4 GB",stg_plan(m,two[..],C));
    two=[2]int.[4460000000,4480000000];show("4.46 GB and 4.48 GB",stg_plan(m,two[..],C));
    two=[2]int.[1000000000,9000000000];show("1 GB and 9 GB",stg_plan(m,two[..],C));
    print("one token crossing a boundary: % bytes; ten tokens: % bytes\n",stg_message_bytes(1,MS_3B.width),stg_message_bytes(10,MS_3B.width));
```

<!-- source: psionic/examples/stage-split.coax run-example -->
```jai
    // Six synthetic layers, three stages of 6,000 bytes, four requests.
    bytes:[8528]u8;t:=stg_tiny(6,bytes[..]);s:=t.shape;
    budgets:=[3]int.[6000,6000,6000];p:=stg_plan(stg_ministral(s),budgets[..],6);
    show("synthetic, three stages of 6000 bytes",p);
    tokens:=[16]int.[1,3,7,12,4,5,9,2,0,15,8,8,11,6,14,10];starts:=[5]int.[0,5,8,14,16];
    digests:=[3]u64.[STG_TINY_DIGEST,STG_TINY_DIGEST,STG_TINY_DIGEST];
    logits:[64]f32;mem:[3*288]f32;wire:[2*57]u32;
    run:=stg_pipeline(s,digests[..],t.embed,t.norm,t.layers[0..<6],p.cut[0..<4],tokens[..],starts[..],logits[..],mem[..],wire[..]);
    print("run: %, % ticks, % messages, % bytes across boundaries\n",stg_run_status_name(run.status),run.ticks,run.messages,run.bytes);
    for tick:0..<run.ticks {
        print("  tick %:",tick);
        for k:0..<3 {if run.order[tick][k]>=0 {print(" stage % request %;",k,run.order[tick][k]);}}
        print("\n");
    }
    same:=0;hidden:[28*8]f32;work:[14*32]f32;whole:[16]f32;
    for r:0..<4 {
        f:=ms_forward(s,t.embed,t.norm,t.layers[0..<6],tokens[starts[r]..<starts[r+1]],hidden[0..<2*8*(starts[r+1]-starts[r])],whole[..],work[..]);
        for i:0..<16 {if f.status==MsStatus.ok && f32_bits(whole[i])==f32_bits(logits[16*r+i]) {same+=1;}}
        print("  request %: greedy token % split, % whole\n",r,ms_argmax(logits[16*r..<16*r+16]),ms_argmax(whole[..]));
    }
    print("logits equal to the unsplit forward bit for bit: % of 64\n",same);
}
```

```text
Ministral 3 3B at 4096 positions
one layer: 232796160 weight bytes, 33554432 cache bytes, 149946368 multiply-adds a token
readout: 402653184 multiply-adds a token; buffers 201326592 bytes a stage
two stages of 6 GB: slowest stage 2202009600 multiply-adds a token
  stage 0: layers 0..<14, 4735541248 bytes (cache 469762048), 2099249152 multiply-adds
  stage 1: layers 14..<26, 4203907072 bytes (cache 402653184), 2202009600 multiply-adds
two stages of 4.5 GB: slowest stage 2351955968 multiply-adds a token
  stage 0: layers 0..<13, 4469190656 bytes (cache 436207616), 1949302784 multiply-adds
  stage 1: layers 13..<26, 4470257664 bytes (cache 436207616), 2351955968 multiply-adds
four stages of 3 GB: slowest stage 1152385024 multiply-adds a token
  stage 0: layers 0..<7, 2871087104 bytes (cache 234881024), 1049624576 multiply-adds
  stage 1: layers 7..<14, 2065780736 bytes (cache 234881024), 1049624576 multiply-adds
  stage 2: layers 14..<21, 2065780736 bytes (cache 234881024), 1049624576 multiply-adds
  stage 3: layers 21..<26, 2339452928 bytes (cache 167772160), 1152385024 multiply-adds
one stage of 8 GB: slowest stage 4301258752 multiply-adds a token
  stage 0: layers 0..<26, 7932815360 bytes (cache 872415232), 4301258752 multiply-adds
two stages of 4.4 GB: refused, total does not fit, 139448320 bytes short
4.46 GB and 4.48 GB: refused, no contiguous split fits, 0 bytes short
1 GB and 9 GB: refused, first stage does not fit, 272983552 bytes short
one token crossing a boundary: 12324 bytes; ten tokens: 122916 bytes
synthetic, three stages of 6000 bytes: slowest stage 1856 multiply-adds a token
  stage 0: layers 0..<2, 4928 bytes (cache 768), 1728 multiply-adds
  stage 1: layers 2..<4, 4672 bytes (cache 768), 1728 multiply-adds
  stage 2: layers 4..<6, 5104 bytes (cache 768), 1856 multiply-adds
run: ok, 6 ticks, 8 messages, 1312 bytes across boundaries
  tick 0: stage 0 request 0;
  tick 1: stage 0 request 1; stage 1 request 0;
  tick 2: stage 0 request 2; stage 1 request 1; stage 2 request 0;
  tick 3: stage 0 request 3; stage 1 request 2; stage 2 request 1;
  tick 4: stage 1 request 3; stage 2 request 2;
  tick 5: stage 2 request 3;
  request 0: greedy token 14 split, 14 whole
  request 1: greedy token 9 split, 9 whole
  request 2: greedy token 12 split, 12 whole
  request 3: greedy token 7 split, 7 whole
logits equal to the unsplit forward bit for bit: 64 of 64
```

With four stages of 3 GB, the planner gives the last stage 5 layers and the
others 7. The readout is about 2.7 layers of work, so the last stage
carries 5 layers plus the readout. In the synthetic run, four requests over
three stages take 6 ticks, which is 4 + 3 − 1. Each request crosses two
boundaries, so there are 8 messages. A request of t tokens sends
4 × (9 + 8t) bytes per crossing, and the 16 tokens add up to 1,312 bytes.

## Refusals

The [second example](../examples/stage-split-refuse.coax) sends boundary
messages that are each wrong in one way. It also runs a pipeline whose
third stage loaded another artifact, and one whose stages stop before the
last layer.

<!-- source: psionic/examples/stage-split-refuse.coax refuse -->
```jai
main :: () {
    rows:=[16]f32.[1,2,3,4,5,6,7,8,-1,-2,-3,-4,-5,-6,-7,-8];wire:[32]u32;
    n:=stg_pack(STG_TINY_DIGEST,2,0,8,rows[..],wire[..]);
    want:=StgExpect.{model=STG_TINY_DIGEST,layer=2,start=0,width=8};
    print("a whole message of % words: %\n",n,stg_refusal_name(stg_check(want,wire[0..<n])));
    c:=wire;c[0]=2;print("protocol version 2: %\n",stg_refusal_name(stg_check(want,c[0..<n])));
    w:=want;w.model=STG_MINISTRAL_DIGEST;print("a receiver holding Ministral 3 3B: %\n",stg_refusal_name(stg_check(w,wire[0..<n])));
    w=want;w.layer=3;print("a receiver whose first layer is 3: %\n",stg_refusal_name(stg_check(w,wire[0..<n])));
    w=want;w.start=2;print("a receiver expecting position 2: %\n",stg_refusal_name(stg_check(w,wire[0..<n])));
    w=want;w.width=16;print("a receiver of width 16: %\n",stg_refusal_name(stg_check(w,wire[0..<n])));
    c=wire;c[20]^=1;print("one value's lowest bit flipped: %\n",stg_refusal_name(stg_check(want,c[0..<n])));
    print("the last word lost: %\n",stg_refusal_name(stg_check(want,wire[0..<n-1])));
    print("one word too many: %\n",stg_refusal_name(stg_check(want,wire[0..<n+1])));

    bytes:[8528]u8;t:=stg_tiny(6,bytes[..]);s:=t.shape;cut:=[4]int.[0,2,4,6];
    tokens:=[5]int.[1,3,7,12,4];starts:=[2]int.[0,5];logits:[16]f32;mem:[3*240]f32;mail:[2*49]u32;
    digests:=[3]u64.[STG_TINY_DIGEST,STG_TINY_DIGEST,STG_MINISTRAL_DIGEST];
    r:=stg_pipeline(s,digests[..],t.embed,t.norm,t.layers[0..<6],cut[..],tokens[..],starts[..],logits[..],mem[..],mail[..]);
    print("stage 2 loaded another artifact: %, %, at stage % on request %\n",stg_run_status_name(r.status),stg_refusal_name(r.refusal),r.stage,r.request);
    digests[2]=STG_TINY_DIGEST;cut[3]=5;
    r=stg_pipeline(s,digests[..],t.embed,t.norm,t.layers[0..<6],cut[..],tokens[..],starts[..],logits[..],mem[..],mail[..]);
    print("stages that stop at layer 5 of 6: %\n",stg_run_status_name(r.status));
}
```

```text
a whole message of 25 words: none
protocol version 2: version
a receiver holding Ministral 3 3B: model
a receiver whose first layer is 3: layer
a receiver expecting position 2: position
a receiver of width 16: geometry
one value's lowest bit flipped: checksum
the last word lost: partial
one word too many: excess
stage 2 loaded another artifact: refused, model, at stage 2 on request 0
stages that stop at layer 5 of 6: shape
```

The run stops at the first refusal and says which stage refused and on
which request. Nothing after that point runs.

## The original weights

The slice harness checks the split on Ministral 3 3B's original BF16
weights:

1. It authenticates the pinned file with `ministral_reference.py pin`.
2. It maps layers 0 and 1, the embedding and the final norm. Together
   they form a two-layer model.
3. It runs the 10-token chat-format prompt from chapter 81 twice. The whole
   run uses chapter 81's functions directly: the embedding, both layers,
   and the full readout in chunks of 4,096 rows. The split run uses
   `stg_pipeline` with one layer on each stage: stage 0's rows cross a
   checked boundary message, and stage 1 reads out in chunks of 32,768
   rows.
4. It compares all 131,072 logits bit for bit.
5. It gives stage 1 a digest one higher than the pinned one, and stage 1
   must refuse.

On this machine (Linux, gcc, one CPU thread):

```text
ministral-manifest.coax matches the pinned checkpoint's header
10 tokens: [1, 3, 7493, 1395, 1278, 8961, 1307, 5498, 1063, 4]
whole: layers 0..<2 and the readout over 10 tokens, 7.279 s
split: stage 0 runs layer 0, stage 1 runs layer 1 and the readout, 7.181 s
boundary: 1 message of 122916 bytes (10 rows of 3072 f32 and a 36-byte header)
logits differing in any bit: 0 of 131072; greedy token 4 both ways
stage 1 holding another artifact: status 3, refusal 3 (model), stage 1
Stage slices: 31 s
```

A split changes no arithmetic, so equal bits are the expected result. The
check is still needed: if the message lost a bit, reordered rows or used the
wrong buffer, the logits would differ. Greedy token 4 is the token after
two layers, not the model's answer. Running all 26 layers on the original
weights is a whole-model run on the CPU, which the repository does not
repeat as a routine check; it waits for a recorded reference or the GPU
path.

## Checks

- **Planner** (`stage-plan.coax`, `#authority`):
  - Ministral 3 3B's stage sizes, splits and refusals are compared with the
    hand-derived numbers above, including the exact byte where each budget
    stops fitting.
  - A `#forall` compares the planner with a brute force over every
    contiguous split, on 400 random models with 1 to 4 stages. The brute
    force computes its own bytes and multiply-adds.
  - Further checks cover the bounds, zero-cost layers, ties, 128 layers over
    16 stages, and each way a boundary message can be wrong.
  - `coax mutate` leaves no survivors (337 mutants).
- **Runner** (`stage-run.coax`):
  - The planner table built from `MS_3B` equals the hand-written one, and
    its bytes equal chapter 81's inventory.
  - A two-stage run of chapter 81's tiny model is within 2 × 10⁻⁶ of the
    float64 Python reference.
  - Five splits of the six-layer model (1, 3 + 3, 1 + 2 + 3, 2 + 2 + 2 and
    six stages of one layer) equal `ms_forward` bit for bit for four
    requests, and 16 stages equal it for one request.
  - The order, message count and bytes of each run match the formulas.
    Chunked readouts equal a whole readout.
  - Every input refusal is checked at its boundary.
  - `coax mutate --root-only` leaves no survivors (372 mutants).

## What remains

- **A cached continuation.** Chapter 81's decoder recomputes every position
  on each call and keeps no cache. Stages that keep their own layers'
  caches and decode one token at a time (with `start` > 0 in the message)
  wait for that.
- **Separate threads and machines.** The stages run one after another in
  one process. Threads, the peer link of
  [chapter 188](ch188-peers-transport-and-discovery.md) and measured link
  costs come next, and [chapter 189](ch189-place-the-model.md) uses them
  to place stages.
- **A protocol schema.** The message is defined in Coax, not in `proto/`.
- **The whole model on original weights.** The two-layer slice is bit-exact.
  All 26 layers have not run on the original weights.
- **Larger boundaries.** Residual streams, shared-KV and recurrent sidebands
  for DeepSeek V4.1 and Mistral Large.
- **Smaller boundary messages.** Exact activation compression is
  [chapter 193](ch193-compress-activations-exactly.md).

## Dependencies

- [Chapter 7](ch007-reading-original-weights.md): artifact identities and
  the matrix-vector product's row limit.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): counting
  bytes and work before running.
- [Chapter 81](ch081-execute-ministral-4.md): the Ministral decoder each
  stage runs.
- [Chapter 182](ch182-disaggregate-prefill-and-decode.md): the checked,
  versioned handoff message this boundary follows.
- [Chapter 183](ch183-split-a-model-across-devices.md): the tensor-parallel
  split inside one machine; this chapter splits between machines.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Mistral target](../docs/targets/mistral.md)
- Y. Huang et al., "GPipe: Efficient Training of Giant Neural Networks
  using Pipeline Parallelism", 2019.
