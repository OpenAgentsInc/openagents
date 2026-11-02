# 187. Stage packages

> **Status: partly implemented.** Stage package manifests, a verifier and
> a loader work on the CPU (profile stage-package-v1). A package names the
> tensors that one stage of [chapter 186](ch186-split-a-model-into-stages.md)'s
> split needs, as byte ranges into the original safetensors file, each with
> its SHA-256. It is bound to the parent file's SHA-256, the planner's
> `StgPlan` digest, the stage's place in the plan and the boundary
> contract's version. A stage loads only from a verified package, through
> [chapter 178](ch178-start-fast.md)'s verify and map phases. Ministral 3
> 3B's two packages are built and verified on the original weights, and
> chapter 186's two-layer slice runs from packaged stages bit for bit.
> Still to build: fetching a package over the network with each chunk
> checked as it arrives, a peak-memory measurement, and packages for
> checkpoints with an untied readout or extra shared assets (DeepSeek
> V4.1's tokenizer and shared experts, whose redistribution licence is
> also an open owner question).

A stage of a split model runs only some of the layers. It should verify and
load only the tensors it runs. This chapter defines the stage package: a
short text manifest that lists those tensors as byte ranges into the
original checkpoint file. Nothing is copied or re-encoded. A stage that
holds the original file (or, later, just the byte ranges it fetched)
checks its package against its own bytes before it maps a single tensor.

The library is [`stage-package.coax`](../lib/stage-package.coax). It is an
`#authority` file, because it decides which bytes a stage may run on. It
reuses chapter 186's planner and boundary contract, chapter 178's
readiness machine, chapter 7's safetensors inventory, and the streaming
SHA-256 from `coax/lib/crypto`.

```shell
coax/bin/coax run psionic/examples/stage-package.coax
coax/bin/coax run psionic/examples/stage-package-refuse.coax
psionic/check.sh --only stage-package
COAX_CC=gcc psionic/tests/check_stage_packages.sh ~/models/ministral-3-3b-instruct-2512-bf16-b6d637be
```

## What a stage needs

Chapter 186's planner cuts the decoder's layers into contiguous ranges.
Stage `i` runs layers `cut[i]..<cut[i+1]`. The first stage also embeds the
tokens, and the last stage also applies the final norm and the readout.
Ministral's readout is tied to the embedding, so the last stage needs the
embedding matrix too.

A tensor's name says what it is. The library maps a name to chapter 81's
inventory index `e`: 0 is `norm.weight`, 1 is `tok_embeddings.weight`, and
`2 + 9 × layer + k` is a layer's `k`-th tensor, in chapter 81's fixed order
of nine. Any other name, such as the image encoder's tensors, gets −1, and
no text stage needs it. Then the rule is short:

- a layer tensor belongs to the stage whose range holds its layer;
- the embedding belongs to the first stage and to the last;
- the final norm belongs to the last stage.

<!-- source: psionic/lib/stage-package.coax select -->
```jai
// Which tensor a name is: chapter 81's inventory index e (0 the final norm,
// 1 the tied embedding, 2 + 9 x layer + k a layer's k-th tensor), or -1
// for a tensor no text stage runs.
sp_entry :: (name:string)->int {
    if name=="norm.weight" {return 0;}
    if name=="tok_embeddings.weight" {return 1;}
    prefix:="layers.";
    for i:0..<7 {if i>=name.count || name[i]!=prefix[i] {return -1;}}
    at:=7;layer:=0;
    while at<name.count && name[at]>='0' && name[at]<='9' {
        layer=layer*10+cast(int)(name[at]-'0');at+=1;
        if layer>=STG_LAYERS_MAX {return -1;}
    }
    // One or more digits, no leading zero, then one of a layer's nine names.
    digits:=at-7;
    if digits<1 || (name[7]=='0' && digits>1) {return -1;}
    for k:0..<9 {if name[at..<name.count]==MS_LAYER_NAMES[k] {return 2+9*layer+k;}}
    return -1;
}
// Does stage `stage` of plan p need tensor e? Its own layers; the embedding
// if it is first, and again as the tied readout if it is last; the final
// norm if it is last.
sp_needs :: (e:int,p:StgPlan,stage:int)->bool {
    if e<0 {return false;}
    last:=stage==p.stages-1;
    if e==0 {return last;}
    if e==1 {return stage==0 || last;}
    l:=(e-2)/9;
    return l>=p.cut[stage] && l<p.cut[stage+1];
}
// The inventory entries stage `stage` needs, in the file's header order.
SpSelected :: struct {status:SpStatus;count:int;bytes:int;}
sp_select :: (inventory:[]P5Tensor,p:StgPlan,stage:int,out:[]int)->SpSelected {
    if p.status!=StgStatus.ok || stage<0 || stage>=p.stages {return SpSelected.{status=SpStatus.stage};}
    r:SpSelected;
    for i:0..<inventory.count {
        if !sp_needs(sp_entry(inventory[i].name),p,stage) {continue;}
        if r.count==out.count {return SpSelected.{status=SpStatus.capacity};}
        out[r.count]=i;r.count+=1;r.bytes+=inventory[i].end-inventory[i].begin;
    }
    return r;
}
// A tensor's bytes in chapter 81's inventory for shape s.
sp_size :: (s:MsShape,e:int)->int {x:=ms_expected(s,e);n:=2*x.rows;if x.rank==2 {n*=x.cols;}return n;}
```

The selection keeps the file's header order, so two builds of the same
stage list the same tensors in the same order.

### Worked sizes: Ministral 3 3B in two stages

Chapter 186 splits Ministral 3 3B after layer 14 when each of two stages
has 6 GB at 4,096 positions. One layer is nine BF16 tensors, 232,796,160
bytes. The embedding is 131,072 × 3,072 × 2 = 805,306,368 bytes, and the
final norm is 3,072 × 2 = 6,144 bytes.

- Stage 0 needs the embedding and 14 layers: 1 + 14 × 9 = 127 tensors and
  805,306,368 + 14 × 232,796,160 = 4,064,452,608 bytes.
- Stage 1 needs 12 layers, the embedding again as its readout, and the
  final norm: 12 × 9 + 2 = 110 tensors and
  12 × 232,796,160 + 805,306,368 + 6,144 = 3,598,866,432 bytes.

Together the two stages hold 7,663,319,040 bytes. That is 805,306,368
bytes (one embedding) more than the text model, because each stage keeps
its own copy. The file is 7,698,234,952 bytes. It also holds the image
encoder and the header, which neither package names.

These are the planner's own numbers. With no context, `stg_bytes` counts a
stage's weights, plus the logits buffers on the last stage. A package's
bytes must equal that count minus those buffers. A `#forall` checks this
on 240 plans of Ministral 3 3B's inventory, as described in
[Checks](#checks).

## The manifest

A manifest is plain text. Five header lines bind the package. Then comes
one line per tensor, in the file's header order: its name, the first byte
and the byte after the last in the original file, and the SHA-256 of
those bytes.

<!-- source: psionic/lib/stage-package.coax text -->
```jai
// SHA-256 of data as 64 lowercase hex digits, fed `chunk` bytes at a time
// (chapter 7's streaming limit is 1 MiB a call).
sp_hex_chunks :: (data:[]u8,chunk:int)->[64]u8 {
    state:[112]u8;digest:[32]u8;out:[64]u8;at:=0;
    #assert sha256_init(state[..])==SHA_OK;
    while at<data.count {n:=stg_min(chunk,data.count-at);#assert sha256_update_checked(state[..],data[at..<at+n],false)==SHA_OK;at+=n;}
    #assert sha256_final_checked(state[..],digest[..],false)==SHA_OK;
    digits:="0123456789abcdef";
    for i:0..<32 {out[2*i]=digits[digest[i]>>4];out[2*i+1]=digits[digest[i]&15];}
    return out;
}
sp_hex :: (data:[]u8)->[64]u8 {return sp_hex_chunks(data,CB_MAX_BYTES);}
// The plan as text: its stages, slowest stage, cuts, and each stage's
// bytes, cache bytes and multiply-adds. Its SHA-256 is the plan's digest.
sp_plan_text :: (p:StgPlan,out:[]u8)->int {
    at:=pv_decimal(p.slowest,out,pv_text(" ",out,pv_decimal(p.stages,out,pv_text("psionic-stage-plan 1 ",out,0))));
    for i:0..<p.stages+1 {at=pv_decimal(p.cut[i],out,pv_text(" ",out,at));}
    for i:0..<p.stages {
        at=pv_decimal(p.bytes[i],out,pv_text(" ",out,at));
        at=pv_decimal(p.kv[i],out,pv_text(" ",out,at));
        at=pv_decimal(p.macs[i],out,pv_text(" ",out,at));
    }
    return pv_text("\n",out,at);
}
sp_plan_digest :: (p:StgPlan)->[64]u8 {text:[2048]u8;return sp_hex(text[0..<sp_plan_text(p,text[..])]);}
// The manifest's five header lines, k = 0..<5:
//   psionic-stage-package 1
//   model SHA256          the parent safetensors file
//   plan SHA256           the planner's StgPlan
//   boundary 1            chapter 186's boundary contract version
//   stage I of N layers A B
// then one line a tensor, in the file's header order:
//   tensor NAME BEGIN END SHA256    (BEGIN..<END: bytes of the original file)
sp_head :: (k:int,model:string,plan:[]u8,p:StgPlan,stage:int,out:[]u8)->int {
    if k==0 {return pv_text("\n",out,pv_decimal(SP_VERSION,out,pv_text("psionic-stage-package ",out,0)));}
    if k==1 {return pv_text("\n",out,pv_text(model,out,pv_text("model ",out,0)));}
    if k==2 {return pv_text("\n",out,pv_text(cast(string)plan,out,pv_text("plan ",out,0)));}
    if k==3 {return pv_text("\n",out,pv_decimal(STG_VERSION,out,pv_text("boundary ",out,0)));}
    at:=pv_decimal(p.stages,out,pv_text(" of ",out,pv_decimal(stage,out,pv_text("stage ",out,0))));
    at=pv_decimal(p.cut[stage+1],out,pv_text(" ",out,pv_decimal(p.cut[stage],out,pv_text(" layers ",out,at))));
    return pv_text("\n",out,at);
}
SP_NEWLINE :: [1]u8.['\n'];
SP_HEAD_REFUSALS :: [5]SpStatus.[SpStatus.version,SpStatus.model,SpStatus.plan,SpStatus.boundary,SpStatus.stage];
sp_name_piece :: (t:P5Tensor,out:[]u8)->int {return pv_text(" ",out,pv_text(t.name,out,pv_text("tensor ",out,0)));}
sp_range_piece :: (t:P5Tensor,out:[]u8)->int {return pv_text(" ",out,pv_decimal(t.end,out,pv_text(" ",out,pv_decimal(t.begin,out,0))));}
```

The plan's digest is the SHA-256 of a short text: the number of stages, the
slowest stage's multiply-adds, the cuts, and each stage's bytes, cache bytes
and multiply-adds. Two plans with the same cuts but a different budget or
context give different texts, so a package names exactly the plan it was
built for.

### Worked package: two layers split after layer 0

The checks build chapter 81's synthetic model, the model chapter 186 uses,
as a real safetensors file. Its header lists the tensors in chapter 81's
inventory order, with the same names and shapes as the Ministral
checkpoint. The payload is in `stg_tiny`'s order, and 8 bytes of an image
tensor come last. With two layers, the header is 1,845 bytes, so the
payload starts at byte 8 + 1,845 = 1,853. The embedding comes first in the
payload (16 × 8 × 2 = 256 bytes, bytes 1,853..<2,109), then the final norm
(16 bytes), then layer 0. Layer 0 starts at 2,125. Its `wq` takes 256
bytes, so its `wk` is bytes 2,381..<2,509.

The model, worked in chapter 186's units: width 8, 16 query values, 8
key-value values, hidden size 12, vocabulary 16. A layer's projections cost
2 × 16 × 8 + 2 × 8 × 8 + 3 × 12 × 8 = 672 multiply-adds, and its weights are
2 × (672 + 16) = 1,376 bytes. Two stages, budgets of 100,000 bytes, 4
positions:

| | Stage 0, layer 0 | Stage 1, layer 1 |
| --- | --- | --- |
| Activations, 192 × 4 | 768 | 768 |
| Layer weights | 1,376 | 1,376 |
| Cache, 64 × 4 | 256 | 256 |
| Embedding, or tied readout | 256 | 256 |
| Final norm and logits buffers | — | 16 + 160 |
| Bytes | 2,656 | 2,832 |
| Multiply-adds: 672 + 32 × 4, plus 128 for the readout | 800 | 928 |

So the plan's text is

```text
psionic-stage-plan 1 2 928 0 1 2 2656 256 800 2832 256 928
```

and the plan line is that text's SHA-256. Stage 0's package holds the
embedding and layer 0: 10 tensors, 256 + 1,376 = 1,632 bytes. Stage 1's
holds the final norm, the embedding and layer 1: 11 tensors,
16 + 256 + 1,376 = 1,648 bytes. Stage 0's manifest starts like this:

```text
psionic-stage-package 1
model SHA-256 of the whole file
plan SHA-256 of the plan text above
boundary 1
stage 0 of 2 layers 0 1
tensor tok_embeddings.weight 1853 2109 SHA-256 of those 256 bytes
tensor layers.0.attention.wk.weight 2381 2509 ...
```

A check compares every number here with the built text. It also checks
each line's hash with chapter 7's `pw_digest`, which compares a digest
through its own code path.

## Building a package

The builder reads the file's header with chapter 7's checked inventory,
selects the stage's tensors, and hashes each tensor's bytes where they
lie. It writes the manifest into a caller's buffer and refuses with
`capacity` if the buffer is too small.

<!-- source: psionic/lib/stage-package.coax build -->
```jai
// Scratch for the file's inventory and the chosen entries.
SpScratch :: struct {inventory:[]P5Tensor;draft:[]P5Tensor;picked:[]int;}
SpBuilt :: struct {status:SpStatus;length:int;count:int;bytes:int;}
// Build stage `stage`'s manifest from the original file, whose SHA-256 is
// `model`: read its header (chapter 7), choose the stage's tensors, hash
// each one's bytes in place, and write the text into out.
sp_build :: (file:[]u8,model:string,p:StgPlan,stage:int,w:SpScratch,out:[]u8)->SpBuilt {
    if !pw_hex_digest(model) {return SpBuilt.{status=SpStatus.shape};}
    scan:=p5_inspect_checked(file,w.inventory,w.draft,false);
    if scan.status!=PwStatus.ok {return SpBuilt.{status=SpStatus.shape};}
    s:=sp_select(w.inventory[0..<scan.written],p,stage,w.picked);
    if s.status!=SpStatus.ok {return SpBuilt.{status=s.status};}
    plan:=sp_plan_digest(p);line:[256]u8;at:=0;
    for k:0..<5 {at=pv_text(cast(string)line[0..<sp_head(k,model,plan[..],p,stage,line[..])],out,at);}
    for j:0..<s.count {
        t:=w.inventory[w.picked[j]];
        at=pv_text(cast(string)line[0..<sp_name_piece(t,line[..])],out,at);
        at=pv_text(cast(string)line[0..<sp_range_piece(t,line[..])],out,at);
        hex:=sp_hex(file[t.begin..<t.end]);
        at=pv_text("\n",out,pv_text(cast(string)hex[..],out,at));
    }
    if at>out.count {return SpBuilt.{status=SpStatus.capacity};}
    return SpBuilt.{length=at,count=s.count,bytes=s.bytes};
}
```

## Verifying a package

Whoever publishes the package (the coordinator that placed the stage) also
publishes the manifest's own SHA-256. A stage verifies its package against
its own copy of the file. It does not parse the manifest. It builds the
manifest it would have written and requires the received text to match it
byte for byte. Where they first differ names the refusal:

| Where the text differs | Refusal |
| --- | --- |
| Its SHA-256 is not the published digest | `digest` |
| Line 1, the format version | `version` |
| Line 2, the parent file | `model` |
| Line 3, the plan | `plan` |
| Line 4, the boundary contract version | `boundary` |
| Line 5, the stage and its layers | `stage` |
| A tensor line's name, or a line missing or extra | `set` |
| A tensor line's byte range | `range` |
| A tensor line's hash, or the bytes behind it | `hash` |

Names and ranges are compared for every tensor before any tensor is hashed,
so a wrong package is refused without reading the weights. The result
names the manifest line where it stopped and how many bytes it hashed
before that.

<!-- source: psionic/lib/stage-package.coax verify -->
```jai
SpStage :: struct {
    status:SpStatus;tensor:int;hashed:int;count:int;life:CsLife;
    tag:u64;embed:[]u8;norm:[]u8;tensors:int;read:int;
}
// Is piece at text[at..]?
sp_same :: (text:[]u8,at:int,piece:[]u8)->bool {
    if at+piece.count>text.count {return false;}
    for i:0..<piece.count {if text[at+i]!=piece[i] {return false;}}
    return true;
}
// Verify a package against the stage's own copy of the file: the text must
// hash to the published digest, then equal, line by line, the manifest
// this stage would build. The header lines and every tensor's name and
// range are compared before any tensor is hashed. The result's tensor is
// the manifest line (0 = first tensor) where the text first differs.
sp_verify :: (text:[]u8,digest:string,file:[]u8,model:string,p:StgPlan,stage:int,w:SpScratch)->SpStage {
    if !pw_hex_digest(digest) || !pw_hex_digest(model) {return SpStage.{status=SpStatus.shape};}
    if !pw_digest(text,digest) {return SpStage.{status=SpStatus.digest};}
    scan:=p5_inspect_checked(file,w.inventory,w.draft,false);
    if scan.status!=PwStatus.ok {return SpStage.{status=SpStatus.shape};}
    s:=sp_select(w.inventory[0..<scan.written],p,stage,w.picked);
    if s.status!=SpStatus.ok {return SpStage.{status=s.status};}
    plan:=sp_plan_digest(p);line:[256]u8;at:=0;
    for k:0..<5 {
        n:=sp_head(k,model,plan[..],p,stage,line[..]);
        if !sp_same(text,at,line[0..<n]) {return SpStage.{status=SP_HEAD_REFUSALS[k]};}
        at+=n;
    }
    head:=at;nl:=SP_NEWLINE;
    for j:0..<s.count {
        t:=w.inventory[w.picked[j]];n:=sp_name_piece(t,line[..]);
        if !sp_same(text,at,line[0..<n]) {return SpStage.{status=SpStatus.set,tensor=j};}
        at+=n;n=sp_range_piece(t,line[..]);
        if !sp_same(text,at,line[0..<n]) {return SpStage.{status=SpStatus.range,tensor=j};}
        at+=n+65;
        if !sp_same(text,at-1,nl[..]) {return SpStage.{status=SpStatus.hash,tensor=j};}
    }
    if at!=text.count {return SpStage.{status=SpStatus.set,tensor=s.count};}
    r:=SpStage.{count=s.count};at=head;
    for j:0..<s.count {
        t:=w.inventory[w.picked[j]];at+=sp_name_piece(t,line[..])+sp_range_piece(t,line[..]);
        hex:=sp_hex(file[t.begin..<t.end]);
        if !sp_same(text,at,hex[..]) {r.status=SpStatus.hash;r.tensor=j;return r;}
        at+=65;r.hashed+=t.end-t.begin;
    }
    return r;
}
```

## Loading only a verified package

`sp_stage` is the only way to get a stage's tensors. It runs chapter 178's
readiness machine. The verify phase is `sp_verify`. The map phase checks
the stage's tensor count and each tensor's size against chapter 81's
shapes, then hands out views into the file for exactly the package's
tensors. The views are slices of the mapping, not copies. On any refusal
the stage fails for good, and no view is handed out. A stage that passes
both phases waits in `arenas`. The host still allocates its arenas and runs
its golden warm-ups before chapter 178 reports it ready.

<!-- source: psionic/lib/stage-package.coax load -->
```jai
// The boundary tag chapter 186's messages carry: the parent digest's first
// 8 bytes.
sp_tag :: (model:string)->u64 {
    t:u64=0;
    for i:0..<16 {c:=model[i];v:=ifx c>='a' then c-'a'+10 else c-'0';t|=cast(u64)v<<(60-4*i);}
    return t;
}
// Bring up stage `stage` of model shape s from its package: chapter 178's
// verify phase is sp_verify, and its map phase hands out views into the
// file for exactly the package's tensors, each the size chapter 81
// expects, into layers[cut..] (and embed, norm). On any refusal the stage
// fails for good and gets no views. read counts the bytes handed out.
sp_stage :: (text:[]u8,digest:string,file:[]u8,model:string,p:StgPlan,stage:int,s:MsShape,golden:int,w:SpScratch,layers:[]MsLayer)->SpStage {
    life:=cs_start(golden);
    if life.state==CsState.failed {return SpStage.{status=SpStatus.shape,life=life};}
    r:=sp_verify(text,digest,file,model,p,stage,w);
    r.life=cs_step(life,CsEvent.verified,r.status==SpStatus.ok);
    if r.status!=SpStatus.ok {return r;}
    // The map phase: the stage's tensor count, then every size, before any view is handed out.
    last:=stage==p.stages-1;need:=9*(p.cut[stage+1]-p.cut[stage]);
    if stage==0 || last {need+=1;}
    if last {need+=1;}
    if !ms_shape_valid(s) || layers.count!=s.layers || p.cut[p.stages]!=s.layers {return sp_unmapped(r,0);}
    if r.count!=need {return sp_unmapped(r,r.count);}
    for j:0..<r.count {t:=w.inventory[w.picked[j]];if t.end-t.begin!=sp_size(s,sp_entry(t.name)) {return sp_unmapped(r,j);}}
    v:=r;v.tag=sp_tag(model);
    for j:0..<r.count {
        t:=w.inventory[w.picked[j]];e:=sp_entry(t.name);view:=file[t.begin..<t.end];v.tensors+=1;v.read+=view.count;
        if e==0 {v.norm=view;continue;}
        if e==1 {v.embed=view;continue;}
        l:=(e-2)/9;k:=(e-2)%9;x:=layers[l];
        if k==0 {x.wk=view;}if k==1 {x.wo=view;}if k==2 {x.wq=view;}if k==3 {x.wv=view;}if k==4 {x.attention_norm=view;}
        if k==5 {x.w1=view;}if k==6 {x.w2=view;}if k==7 {x.w3=view;}if k==8 {x.ffn_norm=view;}
        layers[l]=x;
    }
    v.life=cs_step(r.life,CsEvent.mapped,true);
    return v;
}
sp_unmapped :: (r:SpStage,tensor:int)->SpStage {u:=r;u.status=SpStatus.shape;u.tensor=tensor;u.life=cs_step(r.life,CsEvent.mapped,false);return u;}
```

The loader's `read` and `tensors` are its inventory of what it handed out.
They are compared with the package and with the planner in the checks
below. A further check gives each stage a copy of the file in which every
byte outside its package is a NaN pattern. The stages then run chapter
186's pipeline from those copies, and the logits still equal the unsplit
forward bit for bit. A stage that read any byte outside its package would
have produced a NaN.

## The example

The [example](../examples/stage-package.coax) prints Ministral 3 3B's two
packages from the inventory alone, then builds, verifies and loads the
synthetic six-layer model's three packages for chapter 186's split, and
runs the stages from their views.

<!-- source: psionic/examples/stage-package.coax sizes-example -->
```jai
main :: () {
    inv:[240]P5Tensor;names:[240*64]u8;count:=spx_ministral(inv[..],names[..]);
    two:=[2]int.[6000000000,6000000000];p:=stg_plan(stg_ministral(MS_3B),two[..],4096);picked:[240]int;
    print("Ministral 3 3B, two stages of 6 GB at 4096 positions: layers 0..<% and %..<26\n",p.cut[1],p.cut[1]);
    for k:0..<2 {
        s:=sp_select(inv[0..<count],p,k,picked[..]);
        print("  stage %: % tensors, % bytes; first %, last %\n",k,s.count,s.bytes,inv[picked[0]].name,inv[picked[s.count-1]].name);
    }
```

<!-- source: psionic/examples/stage-package.coax build-example -->
```jai
    w:SpxWork;n:=spx_file(6,w.file[..]);file:=w.file[0..<n];
    sc:=SpScratch.{inventory=w.inventory[..],draft=w.draft[..],picked=w.picked[..]};model:[64]u8;sha:=spx_hex(file,model[..]);
    print("synthetic file: % bytes, header % bytes, sha256 %\n",n,pw_header(file).data-8,sha);
    three:=[3]int.[6000,6000,6000];q:=spx_plan(6,three[..],6);
    texts:[3][4096]u8;lengths:[3]int;digests:[3][64]u8;
    for k:0..<3 {
        r:=sp_build(file,sha,q,k,sc,texts[k][..]);lengths[k]=r.length;digests[k]=sp_hex(texts[k][0..<r.length]);
        print("stage %: layers %..<%, % tensors, % bytes of %, manifest % bytes, sha256 %\n",
            k,q.cut[k],q.cut[k+1],r.count,r.bytes,n,r.length,cast(string)digests[k][..]);
    }
    // The header and the first three tensor lines of stage 2's manifest.
    end:=0;lines:=0;while lines<8 {if texts[2][end]=='\n' {lines+=1;}end+=1;}
    print("stage 2's manifest, first 8 of % lines:\n%",5+20,cast(string)texts[2][0..<end]);
```

<!-- source: psionic/examples/stage-package.coax load-example -->
```jai
    layers:[6]MsLayer;tags:[3]u64;embed:[]u8;norm:[]u8;
    for k:0..<3 {
        c:=sp_stage(texts[k][0..<lengths[k]],cast(string)digests[k][..],file,sha,q,k,MS_TINY_LAYERS6,1,sc,layers[..]);
        print("stage %: %, % bytes hashed, % tensors handed out (% bytes), next phase %\n",
            k,sp_status_name(c.status),c.hashed,c.tensors,c.read,cs_state_name(c.life.state));
        tags[k]=c.tag;if k==2 {embed=c.embed;norm=c.norm;}
    }
    tokens:=[16]int.[1,3,7,12,4,5,9,2,0,15,8,8,11,6,14,10];starts:=[5]int.[0,5,8,14,16];
    logits:[64]f32;mem:[3*288]f32;wire:[2*57]u32;
    run:=stg_pipeline(MS_TINY_LAYERS6,tags[..],embed,norm,layers[..],q.cut[0..<4],tokens[..],starts[..],logits[..],mem[..],wire[..]);
    bt:[8528]u8;t:=stg_tiny(6,bt[..]);same:=0;hidden:[28*8]f32;work:[14*32]f32;whole:[16]f32;
    for r:0..<4 {
        f:=ms_forward(t.shape,t.embed,t.norm,t.layers[0..<6],tokens[starts[r]..<starts[r+1]],hidden[0..<2*8*(starts[r+1]-starts[r])],whole[..],work[..]);
        for i:0..<16 {if f.status==MsStatus.ok && f32_bits(whole[i])==f32_bits(logits[16*r+i]) {same+=1;}}
    }
    print("run from the packaged stages: %, % messages; logits equal to the unsplit forward bit for bit: % of 64\n",
        stg_run_status_name(run.status),run.messages,same);
}
```

```text
Ministral 3 3B, two stages of 6 GB at 4096 positions: layers 0..<14 and 14..<26
  stage 0: 127 tensors, 4064452608 bytes; first tok_embeddings.weight, last layers.13.ffn_norm.weight
  stage 1: 110 tensors, 3598866432 bytes; first norm.weight, last layers.25.ffn_norm.weight
synthetic file: 13625 bytes, header 5081 bytes, sha256 f33f223babd314373cbcdadf5d9ba30309e82da9d23e0b359706cda57f4cc067
stage 0: layers 0..<2, 19 tensors, 3008 bytes of 13625, manifest 2316 bytes, sha256 e3c7bf5c43525bef42a81d010dde7c1b72586e85871ea7ae26e58b6b80ee48fc
stage 1: layers 2..<4, 18 tensors, 2752 bytes of 13625, manifest 2225 bytes, sha256 3fd586c6685dabdc3c335adf13f9f0c81790f2ac60c71c7c2f0101d2d4ecb847
stage 2: layers 4..<6, 20 tensors, 3024 bytes of 13625, manifest 2446 bytes, sha256 de890cb8744b4d0e859f18cad142490d507a6bb71597ae58ea01df68bf416ab8
stage 2's manifest, first 8 of 25 lines:
psionic-stage-package 1
model f33f223babd314373cbcdadf5d9ba30309e82da9d23e0b359706cda57f4cc067
plan 32ec04db5e89006c8bc0e8c020ace8aeecd060ba081ed961f0a3c82171fdf166
boundary 1
stage 2 of 3 layers 4 6
tensor norm.weight 5345 5361 5092a98b60f8a07dff417a46931ce0d25cd85d72a114413aeafb271cac9a622a
tensor tok_embeddings.weight 5089 5345 3b6e6d9c8d19b04e6e938c604abe023aea680c060e1055cafbf46d9943b37610
tensor layers.4.attention.wk.weight 11121 11249 19168903bfdaec432aadd41886d740000c29f5f3bc53f3a90765431c381a22f2
stage 0: ok, 3008 bytes hashed, 19 tensors handed out (3008 bytes), next phase arenas
stage 1: ok, 2752 bytes hashed, 18 tensors handed out (2752 bytes), next phase arenas
stage 2: ok, 3024 bytes hashed, 20 tensors handed out (3024 bytes), next phase arenas
run from the packaged stages: ok, 8 messages; logits equal to the unsplit forward bit for bit: 64 of 64
```

Each stage hashes only its own bytes: 3,008 + 2,752 + 3,024 bytes of the
13,625-byte file, of which 256 (the embedding) are hashed by both the first
and the last stage. The three manifests each have a different digest, so a
coordinator can publish them separately.

## Refusals

The [second example](../examples/stage-package-refuse.coax) changes stage
1's package of the two-layer model in one way at a time.

<!-- source: psionic/examples/stage-package-refuse.coax refuse -->
```jai
main :: () {
    w:SpxWork;fn:=spx_file(2,w.file[..]);file:=w.file[0..<fn];
    sc:=SpScratch.{inventory=w.inventory[..],draft=w.draft[..],picked=w.picked[..]};model:[64]u8;sha:=spx_hex(file,model[..]);
    s:=ms_tiny();b:=[2]int.[100000,100000];p:=spx_plan(2,b[..],4);layers:[2]MsLayer;
    r:=sp_build(file,sha,p,1,sc,w.text[..]);good:=w.text[0..<r.length];dg:[64]u8;digest:=spx_hex(good,dg[..]);
    c:=sp_stage(good,digest,file,sha,p,1,s,2,sc,layers[..]);show("stage 1's own package",c);
    // The host allocates its arenas and passes two golden warm-ups (chapter 178).
    l:=cs_step(cs_step(cs_step(c.life,CsEvent.allocated,true),CsEvent.warmed,true),CsEvent.warmed,true);
    print("  after its arenas and two golden warm-ups: phase %, ready %\n",cs_state_name(l.state),cs_ready(l));
    bad:[8192]u8;n:=spx_edit(good,"boundary 1","boundary 2",bad[..]);
    show("one byte of the manifest changed, the published digest kept",sp_stage(bad[0..<n],digest,file,sha,p,1,s,2,sc,layers[..]));
    // Each change below comes with its manifest's digest recomputed.
    edits:=[7][3]string.[[3]string.["boundary 1","boundary 2","boundary contract version 2"],
        [3]string.["stage 1 of 2","stage 0 of 2","the package of stage 0"],
        [3]string.["tensor norm.weight","tensor normx.weight","a tensor renamed"],
        [3]string.[" 1853 2109 "," 1854 2109 ","the embedding's range moved by one byte"],
        [3]string.["tensor layers.1.attention.wk","tensor layers.0.attention.wk","layer 0's tensor in layer 1's place"],
        [3]string.["model ","model 0","another parent file"],[3]string.["plan ","plan 0","another plan"]];
    for e:edits {n=spx_edit(good,e[0],e[1],bad[..]);d:[64]u8;show(e[2],sp_stage(bad[0..<n],spx_hex(bad[0..<n],d[..]),file,sha,p,1,s,2,sc,layers[..]));}
    hl:=good.count-65;bad=w.text;bad[hl]=ifx good[hl]=='0' then '1' else '0';d:[64]u8;
    show("the last tensor's hash, one digit changed",sp_stage(bad[0..<good.count],spx_hex(bad[0..<good.count],d[..]),file,sha,p,1,s,2,sc,layers[..]));
    f:=w.file;f[1853+100]^=1;
    show("one bit of the embedding changed in the file",sp_stage(good,digest,f[0..<fn],sha,p,1,s,2,sc,layers[..]));
    f=w.file;f[2125+5]^=1;
    show("one bit of layer 0 changed, which stage 1 does not hold",sp_stage(good,digest,f[0..<fn],sha,p,1,s,2,sc,layers[..]));
    show("the right package, checked as stage 0",sp_stage(good,digest,file,sha,p,0,s,2,sc,layers[..]));
    t:=s;t.hidden=13;show("a model whose feed-forward is 13 wide",sp_stage(good,digest,file,sha,p,1,t,2,sc,layers[..]));
}
```

```text
stage 1's own package: ok; phase arenas, ready false
  after its arenas and two golden warm-ups: phase ready, ready true
one byte of the manifest changed, the published digest kept: digest; phase failed, ready false
boundary contract version 2: boundary; phase failed, ready false
the package of stage 0: stage; phase failed, ready false
a tensor renamed: set at manifest tensor 0, 0 bytes hashed first; phase failed, ready false
the embedding's range moved by one byte: range at manifest tensor 1, 0 bytes hashed first; phase failed, ready false
layer 0's tensor in layer 1's place: set at manifest tensor 2, 0 bytes hashed first; phase failed, ready false
another parent file: model; phase failed, ready false
another plan: plan; phase failed, ready false
the last tensor's hash, one digit changed: hash at manifest tensor 10, 1632 bytes hashed first; phase failed, ready false
one bit of the embedding changed in the file: hash at manifest tensor 1, 16 bytes hashed first; phase failed, ready false
one bit of layer 0 changed, which stage 1 does not hold: ok; phase arenas, ready false
the right package, checked as stage 0: stage; phase failed, ready false
a model whose feed-forward is 13 wide: shape at manifest tensor 7, 1648 bytes hashed first; phase failed, ready false
```

A change to one byte of the manifest with the published digest kept is
refused before the file is read. A change with the digest recomputed is
caught by the line it touches. A change to one bit of the embedding in the
file is refused by its hash after the 16 bytes of the final norm, which
come first in header order. A change to layer 0, which stage 1 does not
hold, does not concern stage 1, and it verifies. In every refusal the
stage's phase is `failed` and it never reports ready. A package that
verifies but describes a different model shape is refused in the map
phase, after the hash, at the first tensor whose size differs: layer 1's
`w1`, the eighth tensor in header order.

## The original weights

The harness `check_stage_packages.sh` runs on the pinned Ministral 3 3B
file, mapped once. It first authenticates the file with chapter 81's
`ministral_reference.py pin`. It then builds and verifies both stages of
the 14 + 12 split, writes their manifests to a temporary directory, and
loads every layer from them. Next it builds, verifies and loads chapter
186's two-layer slice from packages, and runs the split from the loaded
views. It runs the whole from chapter 7's views by name and compares all
131,072 logits. Finally it changes one byte of layer 1's `wq` in its own
private copy of the page (the file on disk is never written) and
re-verifies the slice's stage 1. No weight byte is copied to disk.

On this machine (Linux, gcc, one CPU thread):

```text
ministral-manifest.coax matches the pinned checkpoint's header
10 tokens: [1, 3, 7493, 1395, 1278, 8961, 1307, 5498, 1063, 4]
full stage 0: layers 0..<14, 127 tensors, 4064452608 bytes (52.8% of the file), manifest 15850 bytes, built in 22.2 s
  manifest sha256 6abcc2e3dbab5a282878eab4ea4cddc36830aa26267f298cf341d7e85d3ffc50
full stage 1: layers 14..<26, 110 tensors, 3598866432 bytes (46.7% of the file), manifest 13900 bytes, built in 15.7 s
  manifest sha256 c6913023eb2475088e3bd2ec979f8cfed4183049a0cdd8ba0cacb774631ffbbb
full stage 0: ok, 4064452608 bytes hashed, 127 tensors and 4064452608 bytes handed out, phase arenas, 17.3 s
full stage 1: ok, 3598866432 bytes hashed, 110 tensors and 3598866432 bytes handed out, phase arenas, 14.2 s
full: 26 of 26 layers have views; both stages hold the embedding (805306368 bytes each)
slice stage 0: layers 0..<1, 10 tensors, 1038102528 bytes (13.5% of the file), manifest 1396 bytes, built in 3.9 s
  manifest sha256 a43d822c53a5c2b546ffd32ff2a2d60907f6448b999ddc5880ba95f1d2d01a90
slice stage 1: layers 1..<2, 11 tensors, 1038108672 bytes (13.5% of the file), manifest 1518 bytes, built in 4.0 s
  manifest sha256 a5e2d0fbb48b13708e219e35e342a644f1a48ac468d53de00d080d4958c73ec6
psionic-stage-package 1
model 722430146632596c6c41137b2e582095ed01a50342932b035cb8662f7855a340
plan 8f25f7c38fd162689653d75002adce1f371ac8a5fe7e23ead26f23d8bbb7a231
boundary 1
stage 1 of 2 layers 1 2
tensor layers.1.attention.wk.weight 232851016 239142472 6b81bed206ecf040c153bfb2b8bd49cc019eb79bf533...
slice stage 0: ok, 1038102528 bytes hashed, 10 tensors and 1038102528 bytes handed out, phase arenas, 4.0 s
slice stage 1: ok, 1038108672 bytes hashed, 11 tensors and 1038108672 bytes handed out, phase arenas, 3.9 s
slice over 10 tokens: logits differing in any bit between packaged stages and the named whole: 0 of 131072; greedy token 4
slice, one byte of layer 1 changed, stage 1: hash, 31457280 bytes hashed, 0 tensors and 0 bytes handed out, phase failed, 0.2 s
  refused at manifest tensor 2 (layers.1.attention.wq.weight); ready: no
Stage packages: 138 s
```

Per stage, the four measurements of the draft come out as follows:

- **Fetched bytes.** The package's bytes: 4,064,452,608 and 3,598,866,432
  for the two stages, 52.8% and 46.7% of the file. Here they are read from
  a local mapping rather than fetched.
- **Materialized bytes.** None beyond the mapping: the loader hands out
  views, and a stage copies no weight.
- **Time to readiness.** Verify and map took 17.3 s and 14.2 s, almost all
  of it hashing (about 230 MB/s). The arenas and warm-ups that follow
  belong to the runtime, not to the package.
- **Peak memory.** Not measured yet; see below.

The slice's stage 1 refuses the changed byte at its third tensor, layer 1's
`wq`, after hashing the 31,457,280 bytes of the two tensors before it.

## Checks

- **Package** (`stage-package.coax`, `#authority`):
  - Names map to chapter 81's inventory index for all 236 Ministral 3 3B
    names. Near misses map to −1: leading zeros, a layer of 128 or more, a
    missing dot, an unknown suffix, and digits followed by other
    characters.
  - SHA-256 agrees with the published `abc` and empty-input vectors in
    chunks of 1 to 4 bytes. A tensor of 1 MiB + 1 byte, hashed in two
    streaming calls, agrees with chapter 7's `pw_digest_chunks`.
  - The hand-worked two-layer package above: the plan text, the counts,
    the header and the first two tensor lines, byte for byte.
  - A `#forall` over 240 plans of Ministral 3 3B (1 to 4 stages, random
    budgets and contexts). For each stage, the package's bytes equal the
    planner's `stg_bytes` with no context, minus the logits buffers. Each
    layer tensor has exactly one stage, the norm one, the embedding two
    (one when there is a single stage), and the image tensor none. A
    refused plan gives no package.
  - On the synthetic file, twelve plans of 1 to 3 stages are each built,
    verified and loaded. The bytes handed out equal the planner's, and
    only the stage's own layers get views.
  - The NaN-poisoned copies described above: the views equal
    chapter 186's tensors, and the pipeline equals the unsplit forward bit
    for bit.
  - Every refusal in the table, a missing, added or cut line, a short
    hash, a changed byte inside and outside the package, the wrong stage,
    plan or model, an invalid golden count, too little scratch, a file
    missing a needed tensor, and model shapes the file does not match.
    Each refused stage ends `failed`, and a verified stage reaches
    `ready` only after its arenas and golden warm-ups.
  - `coax mutate --root-only` leaves no survivors (363 mutants).
- **Original weights** (`check_stage_packages.sh`): the transcript above.

## What remains

- **Fetching over a network.** Here every stage maps the whole local
  file, and the verifier hashes each tensor whole. A stage that fetches
  only its ranges from a peer
  ([chapter 188](ch188-peers-transport-and-discovery.md)) should check each
  chunk as it arrives. That needs a hash per chunk in the manifest, not
  per tensor.
- **Peak memory.** The harness maps the whole file, so its resident size
  says little about one stage. A stage process that maps only its ranges
  would make this measurable.
- **Other checkpoints.** The rule here is Ministral's: a tied readout and
  no other shared assets. An untied `output.weight`, a tokenizer file, and
  DeepSeek V4.1's shared experts need their own entries. Distributing
  DeepSeek V4.1's packages also waits on the owner's answer about its
  redistribution licence.
- **Placement.** [Chapter 189](ch189-place-the-model.md) uses the package
  sizes to place stages, and [chapter 190](ch190-stage-lifecycle-and-fencing.md)
  starts them in order.

## Dependencies

- [Chapter 7](ch007-reading-original-weights.md): the checked safetensors
  inventory and SHA-256 identities.
- [Chapter 81](ch081-execute-ministral-4.md): tensor names, shapes and the
  pinned checkpoint.
- [Chapter 178](ch178-start-fast.md): the readiness machine whose verify
  and map phases the loader runs.
- [Chapter 186](ch186-split-a-model-into-stages.md): the planner, the
  boundary contract and the pipeline.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [DeepSeek target](../docs/targets/deepseek.md)
- [Safetensors format](https://github.com/huggingface/safetensors)
