# 199. Le Chonk across a mesh

> **Status: partly implemented.** The boundary contract, the draft head's
> placement and a modeled round of drafting and verification across
> stages run on the CPU (profile chonk-mesh-v1). The contract lists every
> part that crosses each cut of Mistral Large 4: the hidden rows, layer
> 35's shared cache, the shared indexer selections and the draft head's
> target activations, and a receiver refuses a missing, extra or changed
> part. A small model in Large 4's pattern decodes greedily with a draft
> head across simulated stages and expert peers and gives the same tokens
> as plain decoding of the whole model; its acceptance is measured. Times
> are modeled from assumed rates, not measured. Still to build: Large 4's
> real weights and its real draft head on real machines, measured per-hop
> latency, per-stage memory and acceptance, parity with a whole-model
> reference on selected layers and tokens, Large 4's own layer arithmetic
> (NoPE layers, the lightning indexer, FP8 block scales) and an indexer
> selection carried by the small model's messages.

Mistral Large 4 (`mistralai/Mistral-Large-4-1T-A52B` at revision
`395730ae`), called Le Chonk, has about 1T total and 52B active
parameters. [Chapter 198](ch198-mistral-large-4-across-a-mesh.md) read its
54 shard headers, counted its bytes layer by layer and cut its 72 layers
into stages, each stage a group of peers sharing the routed experts. This
chapter adds the model's own draft head
([chapter 87](ch087-the-models-own-draft-head.md)) to that mesh, and with
it a third kind of state that crosses stage boundaries.

The draft head does not read only the last layer. Large 4's params.json
names its inputs: `mtp.target_activation_layers` is `"1,18,36,54,71"`, and
the head's `backbone.target_projection.weight` is 6,144 × 30,720, five
rows of width 6,144 side by side. So the head needs the outputs of layers
1, 18, 36, 54 and 71 for the position it drafts from. On a mesh those
layers sit on different machines.

Serving Large 4 to others needs a separate commercial licence from
Mistral, an owner decision. Running it for OpenAgents' own agents is
internal use. Nothing in this chapter serves the model.

The library is [`chonk-mesh.coax`](../lib/chonk-mesh.coax), an
`#authority` file because it decides what a stage must receive, refuses
what does not match, and refuses a mesh with no room for the head. It
reuses chapter 198's plan, layer bytes and small model, chapter 87's draft
head cell and inventory, chapter 186's boundary messages and chapter 191's
expert exchange. The examples are
[`chonk-mesh.coax`](../examples/chonk-mesh.coax) and
[`chonk-mesh-refuse.coax`](../examples/chonk-mesh-refuse.coax).

```shell
coax/bin/coax run psionic/examples/chonk-mesh.coax
coax/bin/coax run psionic/examples/chonk-mesh-refuse.coax
```

## The boundary contract

Chapter 198 counted the bytes each cut sends. A receiving stage also has
to know what it is owed, so that it can refuse a message set with a part
missing. The contract for a cut is a list of parts, in order:

1. The hidden rows, one 6,144-value row a position.
2. For each layer before the cut, its cache rows when a layer after the
   cut reads its cache (layers 39, 43, ... 71 read layer 35's), and its
   indexer selection when a shared-indexer layer after the cut reuses it.
3. When the draft head runs, every tap layer before the cut. The head sits
   on the last stage, beside layer 71 and the readout, so every earlier
   tap travels forward through each cut after it.

Each part is one of chapter 186's checked f32 messages, with its own
digest naming the part's kind and layer.

<!-- source: psionic/lib/chonk-mesh.coax contract -->
```jai
// A part of a boundary message: the hidden rows (layer: the receiver's
// first layer), a shared cache's new rows (layer: its source), a shared
// indexer selection (layer: the full indexer layer that chose it), or a
// draft head's target activation (layer: the layer whose output it is).
// width: values a position.
CkKind :: enum u8 {hidden;cache;selection;tap;}
CkPart :: struct {kind:CkKind;layer:int;width:int;}
CK_PARTS_MAX :: 16;
// A model's sharing maps: src[l] is the layer whose cache layer l reads
// (l itself when it keeps its own), full[l] is 1 when layer l has its own
// indexer (a shared layer reuses the nearest earlier full one), row[l] the
// cache values a position layer l keeps, select the ids a selection holds,
// taps the layers whose outputs the draft head reads.
CkModel :: struct {layers:int;hidden:int;select:int;src:[128]int;full:[128]int;row:[128]int;taps:[8]int;ntaps:int;}
// The parts that cross the cut before layer `cut`, in order: the hidden
// rows; for each earlier layer x, its cache rows when a layer after the
// cut reads them and its selection when a layer after the cut reuses it;
// then, when the draft head runs (it sits on the last stage), every tap
// before the cut. Returns how many.
ck_contract :: (m:CkModel,cut:int,head:bool,parts:[]CkPart)->int {
    parts[0]=CkPart.{kind=CkKind.hidden,layer=cut,width=m.hidden};n:=1;
    for x:0..<cut {
        reads:=false;picks:=false;
        for l:cut..<m.layers {
            j:=l;while m.full[j]==0 {j-=1;}
            if m.src[l]==x {reads=true;}
            if j==x {picks=true;}
        }
        if reads {parts[n]=CkPart.{kind=CkKind.cache,layer=x,width=m.row[x]};n+=1;}
        if picks {parts[n]=CkPart.{kind=CkKind.selection,layer=x,width=m.select};n+=1;}
    }
    for j:0..<m.ntaps {if head && m.taps[j]<cut {parts[n]=CkPart.{kind=CkKind.tap,layer=m.taps[j],width=m.hidden};n+=1;}}
    return n;
}
// The bytes those parts cost for `positions` positions, each part one of
// chapter 186's f32 messages.
ck_bytes :: (parts:[]CkPart,positions:int)->int {b:=0;for x:parts {b+=stg_message_bytes(positions,x.width);}return b;}
```

Large 4's maps come from chapter 86's fixture and the taps from
params.json:

<!-- source: psionic/lib/chonk-mesh.coax large4 -->
```jai
// Large 4's maps (chapter 86's fixture), its cache rows (chapter 198), the
// selection at `context` positions, and params.json's
// mtp.target_activation_layers "1,18,36,54,71".
CK_TAPS :: [5]int.[1,18,36,54,71];
ck_large4 :: (c:L4Config,context:int)->CkModel {
    m:=CkModel.{layers=c.layers,hidden=c.dim,select=l4_min(context,c.index_topk),ntaps=5};
    for l:0..<c.layers {m.src[l]=cast(int)ML4_KV_SOURCE[l];m.full[l]=cast(int)ML4_INDEXER_FULL[l];m.row[l]=l4_row(c,l);}
    for j:0..<5 {m.taps[j]=CK_TAPS[j];}
    return m;
}
// The draft head's bytes from chapter 87's derived shapes (BF16): the
// backbone (its three layers, norms and the projection of the taps), read
// once a round, and the autoregressive head (embedding, joint projection,
// output), read once a draft. cache: its three layers' latent rows over
// the last 2,048 positions (mtp.ragged_attention "2048,2048,2048").
CkHead :: struct {backbone:int;step:int;cache:int;}
CK_HEAD_WINDOW :: 2048;
ck_head :: (m:L4Mesh)->CkHead {
    s:=DH_LARGE4;h:CkHead;
    for e:0..<8+10*s.layers {
        t:=dh_large4_tensor(s,e);b:=2*t.rows*(ifx t.cols==0 then 1 else t.cols);
        // Tensors 0 to 3 are autoregressive_head's, in chapter 87's name order.
        if e<4 {h.step+=b;} else {h.backbone+=b;}
    }
    h.cache=s.layers*(s.rank+s.rope)*l4_min(m.context,CK_HEAD_WINDOW)*m.kv;
    return h;
}
```

Without the head, the contract costs exactly chapter 198's boundary bytes;
a check compares the two at every cut from 1 to 71, at 32,768 and at
1,000 positions.

### Cut 39 for four positions, by hand

With three drafts, a verification pass carries four positions: the pending
token and the three drafts. At cut 39 the contract is the hidden rows,
layer 35's cache rows (layers 39 to 71 read them) and taps 1, 18 and 36.
A message is `4 × (9 + positions × width)` bytes:

```text
hidden rows      4 × (9 + 4 × 6,144) =  98,340
layer 35's rows  4 × (9 + 4 × 512)   =   8,228
taps 1, 18, 36   3 × 98,340          = 295,020
in all                                 401,588 bytes
```

Over chapter 198's assumed link between groups (500 µs and 1,250 MB/s)
that is `500,000 + ceil(401,588 × 1,000 / 1,250) = 821,271` ns. Without
the head, one position costs 24,612 + 2,084 = 26,696 bytes, chapter 198's
number. The taps make the cut about four times heavier per position.

## The draft head on the last stage

Chapter 87's inventory of the head gives its shapes. Its backbone (three
layers, norms and the projection of the five taps) is 3,758,484,480 bytes
of BF16; its autoregressive part (embedding, joint projection and output
over a 512-wide state) is 441,453,568 bytes. Together they are the
4,199,938,048 bytes that chapter 198 found under `mtp_dspark.` in the
headers, counted a second way. Its three attention layers keep a 576-value
latent row a position over at most 2,048 positions
(`mtp.ragged_attention`), 3,538,944 bytes at one byte a value.

The model assumes the backbone runs once a round and the autoregressive
part once a draft. How the real head schedules its layers is not checked
here. With three drafts the head reads
`3,758,484,480 + 3 × 441,453,568 = 5,082,845,184` bytes, 12,707,113 ns at
400,000 MB/s.

A round on chapter 198's plan is: the head drafts on the last stage's
leader; the pending token and the drafts pass through every stage, each
stage reading its weights once for all the positions; each cut sends its
contract; the last stage sends the token ids back to the first. A stage
reads every non-routed weight once, but each position may route to
different experts, so the model counts each position's experts as
distinct, up to the peer's share. That is the most a verification pass can
read, and it is why drafts are not free on a mesh that reads many experts
from one memory.

<!-- source: psionic/lib/chonk-mesh.coax round -->
```jai
// The leader's bytes read when stage a..<b verifies `tokens` positions at
// once: every non-routed weight once, ceil(top_k / width) experts a
// mixture layer for each position up to the peer's share (the positions'
// experts assumed distinct, the most reading), each position's cache rows
// and index keys, the embedding rows on the first stage and the readout
// once on the last. One token is chapter 198's l4_stage read.
ck_read :: (c:L4Config,m:L4Mesh,y:[]L4Layer,t:L4Top,a:int,b:int,i:int,n:int,tokens:int)->int {
    share:=c.experts/m.width;active:=(c.top_k+m.width-1)/m.width;sel:=l4_min(m.context,c.index_topk);r:=0;
    for l:a..<b {
        for p:0..<L4_PARTS {if p!=L4_EXPERT {r+=y[l].part[p];}}
        r+=l4_min(share,tokens*active)*y[l].one+tokens*l4_row(c,ML4_KV_SOURCE[l])*sel*m.kv;
        if ML4_INDEXER_FULL[l]==1 {r+=tokens*c.index_dim*m.context*m.kv;}
    }
    if i==0 {r+=tokens*2*c.dim;}
    if i==n-1 {r+=t.head;}
    return r;
}
// One mixture layer's expert exchange for `tokens` positions, each routed
// as chapter 198's l4_exchange routes one: slot k on peer 1 + k mod
// (width - 1).
ck_exchange :: (c:L4Config,m:L4Mesh,tokens:int)->int {
    if m.width==1 {return 0;}
    owner:[256]int;o:=owner[0..<c.experts];
    if xm_contiguous(c.experts,m.width,o)!=XmStatus.ok {return -1;}
    share:=c.experts/m.width;routes:[9]M4Route;homes:[9]int;
    one:=M4Route.{count=c.top_k};for k:0..<c.top_k {one.ids[k]=share*(1+k%(m.width-1))+k;}
    for t:0..<tokens {routes[t]=one;}
    b:=xm_bytes(routes[0..<tokens],tokens,homes[0..<tokens],o,m.width,c.dim);
    lat:[8][8]int;bw:[8][8]int;for x:0..<8 {for y:0..<8 {lat[x][y]=m.lat_in;bw[x][y]=m.bw_in;}}
    return xm_exchange(b,m.width,lat,bw);
}
// A round on chapter 198's plan p for the mesh: with k drafts, the head on
// the last stage's leader reads its backbone and k autoregressive steps,
// then the k drafts and the pending token (k + 1 positions) pass through
// every stage, each cut carrying its contract for k + 1 positions, and the
// last stage sends k + 1 token ids back to the first. k = 0 is plain
// decoding, no head: the round is chapter 198's latency. On head the last
// leader has no room for the head (gap: the bytes missing).
CkRound :: struct {status:CkStatus;plan:L4Plan;k:int;compute:[8]int;send:[8]int;hop:[8]int;draft:int;round:int;gap:int;}
CK_DRAFTS_MAX :: 8;
ck_round :: (c:L4Config,m:L4Mesh,p:L4Plan,k:int)->CkRound {
    r:=CkRound.{k=k,plan=p};
    if k<0 || k>CK_DRAFTS_MAX {r.status=CkStatus.input;return r;}
    if p.status!=L4Status.ok {r.status=CkStatus.plan;return r;}
    n:=m.stages;h:=ck_head(m);last:=r.plan.stage[n-1].leader;
    if k>0 {
        need:=last+h.backbone+h.step+h.cache;
        if need>m.budget {r.status=CkStatus.head;r.gap=need-m.budget;return r;}
        r.draft=plc_ns(h.backbone+k*h.step,m.read);
    }
    x:=ck_exchange(c,m,k+1);model:=ck_large4(c,m.context);parts:[CK_PARTS_MAX]CkPart;r.round=r.draft;
    y:[128]L4Layer;l4_all(c,y[..]);t:=l4_top(c);
    for i:0..<n {
        r.compute[i]=plc_ns(ck_read(c,m,y[..],t,p.cut[i],p.cut[i+1],i,n,k+1),m.read)+r.plan.stage[i].mixture*x;
        r.send[i]=4*(k+1);
        if i<n-1 {r.send[i]=ck_bytes(parts[0..<ck_contract(model,r.plan.cut[i+1],k>0,parts[..])],k+1);}
        if n>1 {r.hop[i]=plc_send(m.lat_out,m.bw_out,r.send[i]);}
        r.round+=r.compute[i]+r.hop[i];
    }
    return r;
}
// Time a token at per-draft acceptance alpha (each draft kept with
// probability alpha, independently, an assumption): a round emits
// 1 + alpha + ... + alpha^k tokens on average.
ck_per_token :: (r:CkRound,alpha:f64)->int {
    e:=1.0;a:=1.0;for j:0..<r.k {a*=alpha;e+=a;}
    v:=cast(f64)r.round/e;t:=cast(int)v;
    return ifx cast(f64)t<v then t+1 else t;
}
// Of rounds that are ok, the one with the least time a token; the earlier
// on a tie (-1: none is ok).
ck_pick :: (rounds:[]CkRound,alpha:f64)->int {
    best:=-1;
    for i:0..<rounds.count {if rounds[i].status==CkStatus.ok && (best<0 || ck_per_token(rounds[i],alpha)<ck_per_token(rounds[best],alpha)) {best=i;}}
    return best;
}
// The pick of every k from 0 to CK_DRAFTS_MAX on plan p: the k with the
// least time a token (-1 when the plan is refused).
ck_best :: (c:L4Config,m:L4Mesh,p:L4Plan,alpha:f64)->int {
    rounds:[CK_DRAFTS_MAX+1]CkRound;for k:0..<CK_DRAFTS_MAX+1 {rounds[k]=ck_round(c,m,p,k);}
    return ck_pick(rounds[..],alpha);
}
```

With no drafts, a round is chapter 198's latency plus 4 bytes on the hop
back to the first stage, which chapter 198 counts as zero bytes in; a
check holds this for four meshes.

### Time a token at 70% acceptance, by hand

On 2 stages of 4 peers, a round with two drafts takes 131,739,560 ns. If
each draft is kept with probability 0.7, independently (an assumption,
not a measurement), a round emits `1 + 0.7 + 0.49 = 2.19` tokens on
average, so a token takes `131,739,560 / 2.19 = 60,155,051` ns, 60.1 ms,
against 77.5 ms for plain decoding.

## Run it

<!-- source: psionic/examples/chonk-mesh.coax example -->
```jai
main :: () {
    c:=l4_large4();m:=ck_large4(c,32768);parts:[CK_PARTS_MAX]CkPart;
    print("contracts at 32,768 positions (bytes for 1 position without the head; for 4 with it):\n");
    for cut:[5]int.[15,17,39,45,60] {
        n:=ck_contract(m,cut,true,parts[..]);print("  cut %:",cut);
        for x:parts[0..<n] {print(" % %",ck_kind_name(x.kind),x.layer);}
        print("; % and % bytes\n",ck_bytes(parts[0..<ck_contract(m,cut,false,parts[..])],1),ck_bytes(parts[0..<n],4));
    }
    h:=ck_head(l4c_mesh(1,1));
    print("draft head: backbone % bytes once a round, % a draft, cache % bytes; % in all (chapter 198 counts % in the headers)\n",
        h.backbone,h.step,h.cache,h.backbone+h.step+h.cache,l4_inventory(c,L4_PINNED_MANIFEST).draft);
    print("mesh: chapter 198's 256 GB peers and links (assumed rates); a draft kept with probability alpha, independently (assumed)\n");
    for nw:[3]int.[51,24,18] {
        g:=l4c_mesh(nw/10,nw%10);p:=l4_plan(c,g);n:=g.stages;peers:=n*g.width;
        print("  % x % = % peers, last leader ",n,g.width,peers);ckx_gb(p.stage[n-1].leader);print(" GB + head ");ckx_gb(h.backbone+h.step+h.cache);print(" GB\n");
        for k:[4]int.[0,1,2,4] {
            r:=ck_round(c,g,p,k);print("    k %: round ",k);ckx_ms(r.round);print(" (draft ");ckx_ms(r.draft);print("), first cut sends % bytes\n",r.send[0]);
        }
        for a:[3]int.[5,7,9] {
            alpha:=cast(f64)a/10;k:=ck_best(c,g,p,alpha);t:=ck_per_token(ck_round(c,g,p,k),alpha);
            print("    alpha 0.%: best k %, a token ",a,k);ckx_ms(t);print(" (plain ");ckx_ms(ck_per_token(ck_round(c,g,p,0),alpha));print("), % peer-ms a token\n",peers*t/1000000);
        }
    }
    // The small model: 6 layers, layers 3 and 5 read layer 1's cache; the head reads layers 1, 3 and 5.
    src:=[6]int.[0,1,2,1,4,1];taps:=[3]int.[1,3,5];prompt:[4]int;drafted:=0;accepted:=0;
    for seed:0..<3 {for n:[2]int.[2,3] {for k:[2]int.[2,4] {
        ckc_prompt(seed,4,prompt[..]);cut:=ckc_cut(seed,6,n);
        r:=ckc_run(seed,src[..],taps[..],cut[0..<n+1],2,k,prompt[..],12,CkFault.{},-1);
        print("small model, seed %, stages at",seed);for i:1..<n {print(" %",cut[i]);}
        print(", k %: % rounds, % of % drafts kept, same tokens as plain decoding %, % bytes sent (counted %)\n",k,r.split.rounds,r.split.accepted,r.split.drafted,r.same,r.split.sent,r.want);
        drafted+=r.split.drafted;accepted+=r.split.accepted;
    }}}
    print("small model acceptance: % of % drafts\n",accepted,drafted);
}
```

```text
contracts at 32,768 positions (bytes for 1 position without the head; for 4 with it):
  cut 15: hidden 15 tap 1; 24612 and 196680 bytes
  cut 17: hidden 17 selection 16 tap 1; 32840 and 229484 bytes
  cut 39: hidden 39 cache 35 tap 1 tap 18 tap 36; 26696 and 401588 bytes
  cut 45: hidden 45 cache 35 selection 44 tap 1 tap 18 tap 36; 34924 and 434392 bytes
  cut 60: hidden 60 cache 35 tap 1 tap 18 tap 36 tap 54; 26696 and 499928 bytes
draft head: backbone 3758484480 bytes once a round, 441453568 a draft, cache 3538944 bytes; 4203476992 in all (chapter 198 counts 4199938048 in the headers)
mesh: chapter 198's 256 GB peers and links (assumed rates); a draft kept with probability alpha, independently (assumed)
  5 x 1 = 5 peers, last leader 179.694 GB + head 4.203 GB
    k 0: round 136.1 ms (draft 0.0 ms), first cut sends 24612 bytes
    k 1: round 228.2 ms (draft 10.4 ms), first cut sends 98376 bytes
    k 2: round 310.7 ms (draft 11.6 ms), first cut sends 147528 bytes
    k 4: round 475.7 ms (draft 13.8 ms), first cut sends 245832 bytes
    alpha 0.5: best k 0, a token 136.1 ms (plain 136.1 ms), 680 peer-ms a token
    alpha 0.7: best k 1, a token 134.2 ms (plain 136.1 ms), 671 peer-ms a token
    alpha 0.9: best k 3, a token 114.3 ms (plain 136.1 ms), 571 peer-ms a token
  2 x 4 = 8 peers, last leader 130.872 GB + head 4.203 GB
    k 0: round 77.5 ms (draft 0.0 ms), first cut sends 26696 bytes
    k 1: round 109.3 ms (draft 10.4 ms), first cut sends 200884 bytes
    k 2: round 131.7 ms (draft 11.6 ms), first cut sends 301236 bytes
    k 4: round 176.4 ms (draft 13.8 ms), first cut sends 501940 bytes
    alpha 0.5: best k 1, a token 72.9 ms (plain 77.5 ms), 583 peer-ms a token
    alpha 0.7: best k 2, a token 60.1 ms (plain 77.5 ms), 481 peer-ms a token
    alpha 0.9: best k 6, a token 42.4 ms (plain 77.5 ms), 339 peer-ms a token
  1 x 8 = 8 peers, last leader 153.541 GB + head 4.203 GB
    k 0: round 76.3 ms (draft 0.0 ms), first cut sends 4 bytes
    k 1: round 107.9 ms (draft 10.4 ms), first cut sends 8 bytes
    k 2: round 130.0 ms (draft 11.6 ms), first cut sends 12 bytes
    k 4: round 174.3 ms (draft 13.8 ms), first cut sends 20 bytes
    alpha 0.5: best k 1, a token 71.9 ms (plain 76.3 ms), 575 peer-ms a token
    alpha 0.7: best k 2, a token 59.3 ms (plain 76.3 ms), 475 peer-ms a token
    alpha 0.9: best k 6, a token 41.9 ms (plain 76.3 ms), 335 peer-ms a token
small model, seed 0, stages at 4, k 2: 9 rounds, 4 of 18 drafts kept, same tokens as plain decoding true, 5160 bytes sent (counted 5160)
small model, seed 0, stages at 4, k 4: 9 rounds, 4 of 36 drafts kept, same tokens as plain decoding true, 7392 bytes sent (counted 7392)
small model, seed 0, stages at 1 4, k 2: 9 rounds, 4 of 18 drafts kept, same tokens as plain decoding true, 6480 bytes sent (counted 6480)
small model, seed 0, stages at 1 4, k 4: 9 rounds, 4 of 36 drafts kept, same tokens as plain decoding true, 9288 bytes sent (counted 9288)
small model, seed 1, stages at 5, k 2: 11 rounds, 1 of 22 drafts kept, same tokens as plain decoding true, 6192 bytes sent (counted 6192)
small model, seed 1, stages at 5, k 4: 11 rounds, 1 of 44 drafts kept, same tokens as plain decoding true, 8920 bytes sent (counted 8920)
small model, seed 1, stages at 3 5, k 2: 11 rounds, 1 of 22 drafts kept, same tokens as plain decoding true, 10800 bytes sent (counted 10800)
small model, seed 1, stages at 3 5, k 4: 11 rounds, 1 of 44 drafts kept, same tokens as plain decoding true, 15552 bytes sent (counted 15552)
small model, seed 2, stages at 4, k 2: 6 rounds, 6 of 12 drafts kept, same tokens as plain decoding true, 3612 bytes sent (counted 3612)
small model, seed 2, stages at 4, k 4: 6 rounds, 6 of 24 drafts kept, same tokens as plain decoding true, 5100 bytes sent (counted 5100)
small model, seed 2, stages at 2 3, k 2: 6 rounds, 6 of 12 drafts kept, same tokens as plain decoding true, 5376 bytes sent (counted 5376)
small model, seed 2, stages at 2 3, k 4: 6 rounds, 6 of 24 drafts kept, same tokens as plain decoding true, 7584 bytes sent (counted 7584)
small model acceptance: 44 of 312 drafts
```

On five single-peer stages, drafts barely help: each stage reads its
experts from one memory, and four positions read up to 16 experts a layer
instead of 4, so a round with one draft costs 228.2 ms against 136.1 ms
plain. Only at 90% acceptance does the head win clearly. On 2 stages of 4
peers the leader reads one expert a position, the round grows slowly, and
two drafts save about a fifth of each token's time at 70% acceptance. All
of these times come from assumed rates. Acceptance on Large 4 is not
measured here.

### The small model, with a head across stages

Large 4's weights are not on this machine, so the speculation is checked
on chapter 198's small model: chapter 85's latent attention and mixture
layers, 6 layers, layers 3 and 5 reading layer 1's cache, here with an
8-token vocabulary. The draft head reads the outputs of layers 1, 3 and 5,
as Large 4's reads layers 1 to 71. It is chapter 87's recurrent cell,
started from a fixed projection of the taps and sharing the model's
embedding and readout.

<!-- source: psionic/lib/chonk-mesh.coax run -->
```jai
// The small model with a vocabulary: tokens enter as rows of embed and
// leave as the lowest-ID maximum of out times the final row (in f64).
// The draft head is chapter 87's cell (vocab and width 8), started from
// proj times the taps' rows, softsigned.
CkToy :: struct {base:L4Toy;model:CkModel;vocab:int;embed:[]f32;out:[]f32;}
CkDraft :: struct {cell:DhCell;proj:[]f64;}
CK_POS_MAX :: 32;CK_HIDDEN :: 8;CK_ROW :: 7;
ck_next :: (t:CkToy,x:[]f32)->int {
    best:=-1;top:=0.0;h:=x.count;
    for v:0..<t.vocab {s:=0.0;for j:0..<h {s+=cast(f64)t.out[v*h+j]*cast(f64)x[j];}if best<0 || s>top {best=v;top=s;}}
    return best;
}
// The head's state from the taps' rows, then chapter 87's greedy drafts.
ck_start :: (d:CkDraft,taps:[]f32,g:[]f64) {
    for i:0..<g.count {s:=0.0;for j:0..<taps.count {s+=d.proj[i*taps.count+j]*cast(f64)taps[j];}g[i]=s/(1+abs(s));}
}
ck_draft :: (d:CkDraft,taps:[]f32,pending:int,k:int,drafts:[]int) {
    g:[DH_WIDTH_MAX]f64;q:[DH_DRAFTS_MAX*DH_VOCAB_MAX]f64;none:[1]f64;
    ck_start(d,taps,g[0..<d.cell.width]);
    used:=dh_draft(d.cell,g[0..<d.cell.width],pending,k,true,none[..],drafts,q[..]);#assert used==0;
}
// A fault on the first boundary sent (a check's hook): drop part `drop`,
// append a copy of part 0 (extra), or set word `word` of part 0 to value.
CkFault :: struct {drop:int=-1;extra:bool;word:int=-1;value:u32;}
// The state a run keeps, as views of ck_generate's storage: each layer's
// cache (owned by its stage), each stage's copies of earlier caches
// (copy[8 i + x]), the rows a pass works on, each stage's tap rows
// (taps[4 i + j]), and the wire.
CkState :: struct {cache:[][]f32;copy:[][]f32;rows:[]f32;taps:[][]f32;wire:[]u32;work:[]f32;fwork:[]f64;}
CK_DIGEST :: 0x436b;
// A run's outcome. part: the part refused (missing, extra or boundary);
// refusal: chapter 186's reason on boundary. passes and positions: the
// passes through the stages and the positions they carried; sent: bytes;
// digest: of every draft, in order (x 31 + token + 1, mod 2^31 - 1).
CkRun :: struct {status:CkStatus;refusal:StgRefusal;part:CkPart;written:int;rounds:int;passes:int;positions:int;drafted:int;accepted:int;sent:int;digest:int;}
// The rows a part reads or writes on stage i (whose first layer is a) for
// positions p..<p+count: hidden from the pass rows, a cache from the
// stage's own cache or its copy, a tap from the stage's tap rows.
ck_rows :: (st:CkState,t:CkToy,x:CkPart,i:int,a:int,p:int,count:int)->[]f32 {
    if x.kind==CkKind.hidden {return st.rows[0..<count*CK_HIDDEN];}
    if x.kind==CkKind.tap {j:=0;while t.model.taps[j]!=x.layer {j+=1;}return st.taps[4*i+j][0..<count*CK_HIDDEN];}
    if x.layer>=a {return st.cache[x.layer][p*CK_ROW..<(p+count)*CK_ROW];}
    return st.copy[i*8+x.layer][p*CK_ROW..<(p+count)*CK_ROW];
}
// One pass: tokens at positions p.. through every stage, stage by stage.
// Each stage runs its layers on every position, keeps the taps it
// computes, then sends its cut's contract as one message a part; the next
// stage receives each part it expects, in order, through chapter 186's
// checks, and refuses a missing or extra part.
ck_pass :: (st:CkState,t:CkToy,cut:[]int,head:bool,p:int,tokens:[]int,peers:[][]XmPeer,owner:[][]int,fault:CkFault,given:CkRun)->CkRun {
    r:=given;
    s:=m4o_shape();n:=cut.count-1;count:=tokens.count;parts:[CK_PARTS_MAX]CkPart;
    for q:0..<count {for j:0..<CK_HIDDEN {st.rows[q*CK_HIDDEN+j]=t.embed[tokens[q]*CK_HIDDEN+j];}}
    for i:0..<n {
        a:=cut[i];b:=cut[i+1];
        view:[8][]f32;for l:0..<t.base.layers {view[l]=ifx l>=a then st.cache[l] else st.copy[i*8+l];}
        for q:0..<count {
            x:=st.rows[q*CK_HIDDEN..<(q+1)*CK_HIDDEN];
            for l:a..<b {
                if !l4_layers(s,t.base,l,l+1,p+q,x,view[0..<t.base.layers],peers,owner,st.work[..],st.fwork[..]) {r.status=CkStatus.run;return r;}
                for j:0..<t.model.ntaps {if t.model.taps[j]==l {for v:0..<CK_HIDDEN {st.taps[4*i+j][q*CK_HIDDEN+v]=x[v];}}}
            }
        }
        if i==n-1 {continue;}
        // Send: each part packed into the wire; frame k is at[k]..<at[k+1].
        np:=ck_contract(t.model,b,head,parts[..]);at:[CK_PARTS_MAX+2]int;frames:=0;first:=r.passes==0 && i==0;
        for k:0..<np+1 {
            if k==np && !(first && fault.extra) {break;}
            x:=parts[k%np];
            if first && k==fault.drop {continue;}
            w:=stg_pack(CK_DIGEST+256*cast(u64)x.kind+cast(u64)x.layer,b,p,x.width,ck_rows(st,t,x,i,a,p,count),st.wire[at[frames]..<st.wire.count]);
            at[frames+1]=at[frames]+w;frames+=1;r.sent+=4*w;
        }
        if first && fault.word>=0 {st.wire[fault.word]=fault.value;}
        // Receive, in the contract's order: each part from the frame whose
        // header names it (the last such; digests differ in their low
        // word), refused when none does.
        for k:0..<np {
            x:=parts[k];id:=CK_DIGEST+256*cast(u64)x.kind+cast(u64)x.layer;f:=-1;
            for j:0..<frames {if st.wire[at[j]+1]==cast(u32)(id&4294967295) {f=j;}}
            if f<0 {r.status=CkStatus.missing;r.part=x;return r;}
            got:=stg_receive(StgExpect.{model=id,layer=b,start=p,width=x.width},st.wire[at[f]..<at[f+1]],ck_rows(st,t,x,i+1,b,p,count));
            if got.refusal!=StgRefusal.none {r.status=CkStatus.boundary;r.refusal=got.refusal;r.part=x;return r;}
        }
        if frames>np {r.status=CkStatus.extra;r.part=parts[0];return r;}
    }
    r.passes+=1;r.positions+=count;
    return r;
}
// Greedy decoding across the stages at cut. The prompt's tokens but the
// last pass first; then each round the head drafts k tokens from the last
// accepted position's taps and the pending token, one pass verifies the
// pending token and the drafts (k + 1 positions), the drafts are kept up
// to the first that differs from the model's own choice, and that choice
// (or, when all are kept, the next one) becomes pending. A rejected
// position's cache rows are written again by the next pass before any
// later position reads them. k = 0 is plain decoding, one position a pass.
ck_generate :: (t:CkToy,d:CkDraft,cut:[]int,prompt:[]int,steps:int,k:int,peers:[][]XmPeer,owner:[][]int,fault:CkFault,out:[]int)->CkRun {
    r:CkRun;n:=cut.count-1;L:=t.base.layers;
    // Cuts rising from 0 to the layers (at most 8) make at most 8 stages.
    ok:=k>=0 && k<=CK_DRAFTS_MAX && prompt.count>=2 && steps>=1 && out.count>=steps && prompt.count+steps+k<=CK_POS_MAX+1 && cut.count>0 && t.model.ntaps<=4;
    ok=ok && cut[0]==0 && cut[n]==L;
    for i:0..<n {ok=ok && cut[i]<cut[i+1];}
    for x:prompt {ok=ok && x>=0 && x<t.vocab;}
    if !ok {r.status=CkStatus.input;return r;}
    head:=k>0;
    cache:[8][CK_POS_MAX*CK_ROW]f32;copy:[64][CK_POS_MAX*CK_ROW]f32;rows:[CK_POS_MAX*CK_HIDDEN]f32;taps:[32][CK_POS_MAX*CK_HIDDEN]f32;
    wire:[(CK_PARTS_MAX+1)*(STG_HEAD+CK_POS_MAX*CK_HIDDEN)]u32;work:[7*8*6+64]f32;fwork:[80]f64;
    cv:[8][]f32;pv:[64][]f32;tv:[32][]f32;
    for l:0..<64 {pv[l]=copy[l][..];if l<32 {tv[l]=taps[l][..];}if l<8 {cv[l]=cache[l][..];}}
    st:=CkState.{cache=cv[..],copy=pv[..],rows=rows[..],taps=tv[..],wire=wire[..],work=work[..],fwork=fwork[..]};
    r=ck_pass(st,t,cut,head,0,prompt[0..<prompt.count-1],peers,owner,fault,r);if r.status!=CkStatus.ok {return r;}
    pos:=prompt.count-1;last:=pos-1;pending:=prompt[pos];
    tokens:[CK_DRAFTS_MAX+1]int;made:[CK_DRAFTS_MAX+1]int;tap:[4*CK_HIDDEN]f32;
    while r.written<steps {
        tokens[0]=pending;
        if head {
            from:=tap[0..<t.model.ntaps*CK_HIDDEN];
            for e:0..<from.count {from[e]=st.taps[4*(n-1)+e/CK_HIDDEN][last*CK_HIDDEN+e%CK_HIDDEN];}
            ck_draft(d,from,pending,k,tokens[1..<k+1]);
            for j:1..<k+1 {r.digest=(r.digest*31+tokens[j]+1)%2147483647;}
        }
        r=ck_pass(st,t,cut,head,pos,tokens[0..<k+1],peers,owner,fault,r);if r.status!=CkStatus.ok {return r;}
        kept:=0;
        for q:0..<k+1 {made[q]=ck_next(t,st.rows[q*CK_HIDDEN..<(q+1)*CK_HIDDEN]);}
        while kept<k && tokens[kept+1]==made[kept] {kept+=1;}
        for q:0..<kept+1 {if r.written<steps {out[r.written]=made[q];r.written+=1;}}
        r.rounds+=1;r.drafted+=k;r.accepted+=kept;
        last=kept;pending=made[kept];pos+=kept+1;
    }
    return r;
}
```

A pass runs stage by stage, each stage on every position, as a pipeline
verifies a block. The receiver takes each part from the frame whose header
names it, so a dropped part is refused as missing by name rather than as a
misread neighbour. A rejected draft's cache rows are left in place: the
next pass starts at the first rejected position and writes those rows
again before any later position reads them.

With one stage cut at 4 and two drafts, a pass carries three positions:
the hidden rows `4 × (9 + 3 × 8) = 132` bytes, layer 1's rows
`4 × (9 + 3 × 7) = 120` and taps 1 and 3, `2 × 132 = 264`: 516 bytes. Seed
0 makes 9 rounds after a prompt pass, 10 passes of three positions,
5,160 bytes, as the transcript counts.

Every run gives the same tokens as plain decoding of the whole model on
one stage and one peer, and as a plain loop over positions and layers
written in the checks without stages, messages or drafts. The small head
keeps 44 of its 312 drafts. That number says nothing about Large 4's head;
it shows the acceptance is measured and the tokens do not depend on it.

## When it is refused

<!-- source: psionic/examples/chonk-mesh-refuse.coax refuse -->
```jai
main :: () {
    c:=l4_large4();m:=l4c_mesh(2,4);p:=l4_plan(c,m);
    print("9 drafts: %\n",ck_status_name(ck_round(c,m,p,9).status));
    g:=l4c_mesh(4,1);g.budget=1000000000;
    print("1 GB peers: %\n",ck_status_name(ck_round(c,g,l4_plan(c,g),1).status));
    h:=l4c_mesh(3,2);h.budget=183000000000;q:=l4_plan(c,h);r:=ck_round(c,h,q,1);
    print("3 stages of 2 peers at 183 GB, 1 draft: %, % bytes short; without drafts: %\n",ck_status_name(r.status),r.gap,ck_status_name(ck_round(c,h,q,0).status));

    src:=[6]int.[0,1,2,1,4,1];taps:=[3]int.[1,3,5];prompt:[4]int;ckc_prompt(1,4,prompt[..]);cut:=[4]int.[0,2,4,6];
    s:=ckc_run(1,src[..],taps[..],cut[..],2,2,prompt[0..<1],6,CkFault.{},-1);
    print("a prompt of one token: %\n",ck_status_name(s.split.status));
    bad:=[3]int.[0,3,3];s=ckc_run(1,src[..],taps[..],bad[..],2,2,prompt[..],6,CkFault.{},-1);
    print("cuts 0 3 3: %\n",ck_status_name(s.split.status));
    names:=[3]string.["hidden rows","layer 1's cache rows","tap 1"];
    for d:0..<3 {
        s=ckc_run(1,src[..],taps[..],cut[..],2,2,prompt[..],6,CkFault.{drop=d},-1);
        print("% dropped at cut 2: %, % %\n",names[d],ck_status_name(s.split.status),ck_kind_name(s.split.part.kind),s.split.part.layer);
    }
    s=ckc_run(1,src[..],taps[..],cut[..],2,2,prompt[..],6,CkFault.{extra=true},-1);
    print("hidden rows sent twice: %\n",ck_status_name(s.split.status));
    s=ckc_run(1,src[..],taps[..],cut[..],2,2,prompt[..],6,CkFault.{word=9,value=7},-1);
    print("a value in the hidden rows changed: %, %\n",ck_status_name(s.split.status),stg_refusal_name(s.split.refusal));
    s=ckc_run(1,src[..],taps[..],cut[..],2,2,prompt[..],6,CkFault.{word=1},-1);
    print("their digest changed: %, % %\n",ck_status_name(s.split.status),ck_kind_name(s.split.part.kind),s.split.part.layer);
    s=ckc_run(1,src[..],taps[..],cut[..],2,2,prompt[..],6,CkFault.{},2);
    print("layer 1's attention overflows: whole %, split %\n",ck_status_name(s.whole.status),ck_status_name(s.split.status));
}
```

```text
9 drafts: input
1 GB peers: chapter 198's plan refused
3 stages of 2 peers at 183 GB, 1 draft: no room for the draft head on the last stage, 4069523456 bytes short; without drafts: ok
a prompt of one token: input
cuts 0 3 3: input
hidden rows dropped at cut 2: a part is missing, hidden 2
layer 1's cache rows dropped at cut 2: a part is missing, cache 1
tap 1 dropped at cut 2: a part is missing, tap 1
hidden rows sent twice: an extra part
a value in the hidden rows changed: a part refused, checksum
their digest changed: a part is missing, hidden 2
layer 1's attention overflows: whole a layer's arithmetic failed, split a layer's arithmetic failed
```

The program exits with status 0 after printing every refusal. On 3 stages
of 2 peers at 183 GB, chapter 198's plan fits: the last leader holds
182,866,046,464 bytes. The head needs 4,199,938,048 bytes of weights and
3,538,944 of cache, so the stage is 4,069,523,456 bytes short. The plan is
not changed to make room; plain decoding on the same mesh is admitted.
With the low word of the hidden rows' digest set to zero, no frame names
the hidden rows, so they are missing; a changed value fails chapter 186's
checksum.

## Checks

The `#test` and `#forall` checks in [`chonk-mesh.coax`](../lib/chonk-mesh.coax)
cover:

- **Contract.** Without the head, its bytes equal chapter 198's
  `l4_boundary` at every cut and two context lengths. Hand-worked parts at
  cuts 1, 15, 17, 18 and 39, and the bytes at cut 39 for four positions.
- **Head.** Its bytes from chapter 87's shapes equal chapter 198's count
  from the headers; its cache at two context lengths and cache widths.
- **Rounds.** One position's reads equal chapter 198's for every stage of
  two plans; reads for several positions by hand, capped at a peer's
  share; the exchange for one position equals chapter 198's at every
  width. Plain decoding equals chapter 198's latency on four meshes. Hand
  values for the head's time, the cut's message and its hop, and the
  hop back; refusals for drafts out of range, a refused plan and a last
  stage with no room, with the exact gap, admitted at exactly enough
  room. Time a token by hand, and the pick of drafts with ties.
- **Runs.** Over 3 seeds, 1 to 3 stages and 0 to 3 drafts, with 1 to 3
  expert peers, the split run's tokens equal plain decoding of the whole
  and the checks' plain loop, and the bytes sent equal a count made in the
  checks. The rounds, the drafts kept and a digest of every draft equal a
  replay of the schedule from the plain loop's tokens and tap rows with
  chapter 87's `dh_draft`. Acceptance on two seeds, a cut right after a
  cache's source and a model of no layers. The head's starting state is
  checked value for value; the readout takes the lowest ID of the largest.
- **Refusals.** A dropped hidden, cache or tap part is missing, by name;
  a part sent twice is extra, with the bytes it cost; a changed version,
  layer, position or value is refused by chapter 186's checks; a changed
  digest leaves a part missing or fails the model check; a fault on the
  first boundary leaves later ones alone; a layer that overflows stops
  the run whole and split. Each
  input bound of a run: drafts, prompt length, steps, output room,
  positions, the cuts, the taps and the prompt's tokens.

`coax mutate psionic/lib/chonk-mesh.coax --root-only` leaves no
surviving mutant.

## What remains

- Large 4's real weights and its real draft head on real machines, over
  [chapter 188](ch188-peers-transport-and-discovery.md)'s transport, with
  per-hop latency, per-stage memory and acceptance measured rather than
  modeled or assumed.
- Parity with a whole-model reference on selected layers and tokens under
  [chapter 17](ch017-qualify-honestly.md)'s rules, and the draft head's
  greedy parity on the real model.
- Large 4's own forward pass (NoPE layers, the lightning indexer's
  selection, FP8 block scales) and the real head's arithmetic. The small
  model uses chapter 85's layers and chapter 87's cell, and has no indexer,
  so its messages carry no selection part; the selection parts are listed
  and counted for Large 4 only.
- Placing the head where the plan has room instead of refusing, and a
  price per token in sats once peers are paid.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Mistral target](../docs/targets/mistral.md)
- `mistralai/Mistral-Large-4-1T-A52B` at revision
  `395730aefd2f928daaf7b69fbd9fefab0ee69b64`: params.json (the `mtp`
  section) and the headers chapters 87 and 198 pinned.
