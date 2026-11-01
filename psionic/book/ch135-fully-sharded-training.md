# 135. Fully sharded training

> **Status: partly implemented.** Chapter 118's trainer runs fully sharded
> on logical CPU ranks: parameters, gradients and both AdamW moments are
> split into equal shards, the parameters are all-gathered before each
> step, the gradient is reduce-scattered in chapter 118's addition order,
> and each rank updates only its shard. Chapter 115's decoder trains this
> way to chapter 118's bits at every rank count that divides its 220
> parameters, and a sharded checkpoint reloads at another rank count and
> continues to the same bits. Each rank's bytes are accounted by hand
> against chapter 116's planner, and each step's bytes against chapter
> 133's closed forms. Measured scaling and per-device peak memory on real
> devices or machines, and gathering one layer at a time with prefetch,
> remain.

Data-parallel training ([chapter 134](ch134-data-parallel-training.md))
keeps a full copy of the training state on every rank: the parameters,
the gradient and both AdamW moments. Fully sharded training splits all of
them across the ranks, so each rank holds a quarter of the state at four
ranks. The price is communication: a rank must gather the parameters it
lacks before it can run the model, and the gradient must reach the rank
that owns each piece of it.

This chapter keeps chapter 118's trainer and makes three changes:

- **Shards.** Rank `r` owns values `r * c` up to `(r + 1) * c` of the
  parameters, the gradient sum and both moments, `c = n / ranks`. The
  shards are the same size; an uneven split is refused.
- **Collectives.** Before a step each rank all-gathers the parameters.
  After backward the gradient is reduce-scattered: shard `s` is summed
  along the ranks and handed to rank `s`.
- **The step.** Each rank clips and updates only its own shard with
  chapter 124's AdamW. Nothing is written until every rank's update is
  admitted.

The result is chapter 118's step, bit for bit. The decisions are in Coax,
in [`fully-sharded.coax`](../lib/fully-sharded.coax) (`#authority`): the
plan and its refusals, the order of every addition, which ranks must
agree before anything is written, the sharded checkpoint and its reload,
and the byte accounts. The ranks are logical: blocks of one buffer on
this CPU, run one after another.

## Who owns what

A plan names the rank count, the parameter count `n` and the step's
micro-batch count. Micro-batches are split as in chapter 134: rank `r`
owns micro-batches `r * micro / ranks` up to `(r + 1) * micro / ranks`.
Unlike chapter 134, a rank may own no micro-batch: it still holds its
shard, so it still takes part.

<!-- source: psionic/lib/fully-sharded.coax plan -->
```jai
// n parameters over `ranks` ranks: rank r owns values r*c up to (r+1)*c,
// c = n / ranks, and micro-batches fs_first(r) up to fs_first(r+1) of each
// step (chapter 134's split). A rank may own no micro-batch; it still owns
// its shard. Every shard is the same size: an uneven split is refused.
FsPlan :: struct {status:FsStatus;ranks:int;n:int;micro:int;}
fs_plan :: (ranks:int,n:int,micro:int)->FsPlan {
    p:=FsPlan.{ranks=ranks,n=n,micro=micro};
    if ranks<1 || ranks>FS_RANKS {p.status=FsStatus.ranks;return p;}
    if n<1 || n>CK_VALUES {p.status=FsStatus.shape;return p;}
    if n%ranks!=0 {p.status=FsStatus.uneven;return p;}
    if micro<1 || micro>TR_ACCUMULATE {p.status=FsStatus.micro;}
    return p;
}
fs_size :: (p:FsPlan)->int {return p.n/p.ranks;}
fs_first :: (p:FsPlan,r:int)->int {return r*p.micro/p.ranks;}
// The configuration this file trains: the plan is admitted, it names
// AdamW, and chapter 118 admits it.
fs_admit :: (c:TrConfig,p:FsPlan,docs:[]PkDoc,tokens:[]u8)->FsStatus {
    if p.status!=FsStatus.ok {return p.status;}
    if c.optimizer!=TrOptimizer.adamw {return FsStatus.optimizer;}
    if tr_admit(c,p.n,docs,tokens)!=TrStatus.ok {return FsStatus.config;}
    return FsStatus.ok;
}
```

For chapter 115's decoder at the checks' small shape, `n = 220` and a
step has 4 micro-batches of one row. At 4 ranks each shard is
`220 / 4 = 55` values: rank 0 holds values 0 to 54, rank 3 holds 165 to
219, and each rank owns one micro-batch. At 5 ranks the shards are 44
values; `fs_first` gives `0`, `4 / 5 = 0`, `8 / 5 = 1`, `12 / 5 = 2`,
`16 / 5 = 3` and `20 / 5 = 4`, so rank 0 owns no micro-batch. At 3 ranks
`220 / 3` is not whole and the plan is refused.

## The gather and the reduce-scatter by hand

Take 4 values over 2 ranks (shards of 2) and a step of 3 micro-batches.
Rank 0 owns micro-batch 0; rank 1 owns micro-batches 1 and 2. The
micro-batch gradients are:

```text
micro-batch 0 (rank 0):   3    6   -9   1.5
micro-batch 1 (rank 1):   0    3    9   6
micro-batch 2 (rank 1):   6   -3    0   3
```

**All-gather.** Each rank copies its own shard into a working copy of all
4 values and receives the other shard from its owner: 2 values each.

**Reduce-scatter.** Chapter 118's `tr_add` adds the micro-batches in
order, each divided by the count, starting from zero:
`((0 + g0/3) + g1/3) + g2/3`. Floating-point addition is not associative,
so that order is the one to keep. The ranks keep it by passing a running
sum along, as chapter 134's chain does, one shard at a time:

- Shard 0 (values 0 and 1). Rank 0 starts at zero and adds `3/3` and
  `6/3`: it sends `1, 2` to rank 1 (2 values). Rank 1 adds micro-batch
  1, then 2: `1 + 0/3 + 6/3 = 3` and `2 + 3/3 + -3/3 = 2`. The sum ends
  at rank 1, but rank 0 owns the shard, so rank 1 sends it on: 2 more
  values, 4 in all.
- Shard 1 (values 2 and 3). Rank 0 sends `-9/3 = -3` and `1.5/3 = 0.5`.
  Rank 1 adds: `-3 + 9/3 + 0/3 = 0` and `0.5 + 6/3 + 3/3 = 3.5`. Rank 1
  owns this shard: 2 values in all.

The sum is `3, 2, 0, 3.5`, the same bits as `tr_add`. The loss passes
along the same way: `3/3 + 6/3 + 1.5/3 = 3.5`.

The chain always ends at the last rank, because the last rank owns the
last micro-batches. So every shard except the last rank's own makes one
extra hop to its owner. That hop is the cost of keeping chapter 118's
bits; a reduce-scatter that sums at the owner would add in another order.

<!-- source: psionic/lib/fully-sharded.coax collectives -->
```jai
// All-gather: rank r's working copy of the whole vector. Its own shard is
// copied in place; every other shard comes from its owner. Returns the
// values rank r received.
fs_gather :: (p:FsPlan,r:int,w:[]f64,work:[]f64)->int {
    c:=fs_size(p);moved:=0;
    for s:0..<p.ranks {for i:s*c..<(s+1)*c {work[i]=w[i];}if s!=r {moved+=c;}}
    return moved;
}
// Reduce-scatter of shard s. grads holds micro-batch m's whole gradient at
// m*n, on the rank that owns m. Rank 0 starts at zero and adds its
// micro-batches' values divided by the step's micro-batch count, in order;
// rank 1 continues from rank 0's sum in msg, and so on: chapter 118's
// tr_add order. The last rank then sends the sum to the shard's owner, rank
// s, unless it is rank s. out receives the shard. Returns the values sent.
fs_scatter :: (p:FsPlan,s:int,grads:[]f64,msg:[]f64,out:[]f64)->int {
    c:=fs_size(p);count:=f64(p.micro);moved:=0;
    for r:0..<p.ranks {
        for i:0..<c {acc:=0.0;if r>0 {acc=msg[i];}
            for m:fs_first(p,r)..<fs_first(p,r+1) {acc+=grads[m*p.n+s*c+i]/count;}
            msg[i]=acc;}
        if r>0 {moved+=c;}
    }
    if s!=p.ranks-1 {moved+=c;}
    for i:0..<c {out[s*c+i]=msg[i];}
    return moved;
}
// The step's loss: each rank adds its micro-batches' losses to the running
// sum, in micro-batch order.
fs_loss :: (p:FsPlan,losses:[]f64)->f64 {
    count:=f64(p.micro);loss:=0.0;
    for r:0..<p.ranks {for m:fs_first(p,r)..<fs_first(p,r+1) {loss+=losses[m]/count;}}
    return loss;
}
```

## The step on each rank

The update needs two numbers that depend on every shard: the loss, which
must be finite, and the gradient's global norm, for clipping and for the
check that its squares do not overflow. Chapter 118 adds the squares in
index order. Since shard `s` holds the `s`-th run of values, the ranks
keep that order by passing the running sum of squares from rank 0 to the
last, the same way as the loss. Then each rank clips its own shard by the
global norm and stages its own AdamW update.

AdamW treats each value on its own, so a shard can be updated alone. Its
groups only need to say where weight decay starts and stops inside the
shard: with decay on values 2 to 4 and 9 to 10 of 12, the shard of values
4 to 7 has two groups, one decayed value and then three that are not.

Before anything is written, the ranks vote. If AdamW refuses any rank's
shard, every rank refuses with `optimizer`; otherwise, if any new value
on any rank is beyond the bound, every rank refuses with `bound`. This is
the order chapter 118 reports them in.

<!-- source: psionic/lib/fully-sharded.coax apply -->
```jai
// AdamW groups for the shard first..<first+count, counted from its start:
// a new group wherever weight decay starts or stops (chapter 118 decays
// the values in its spans only). AdamW treats each value on its own, so
// these groups give chapter 118's values. Returns how many there are.
fs_groups :: (c:TrConfig,rate:f64,first:int,count:int,out:[]AwGroup)->int {
    k:=0;prev:bool;
    for i:0..<count {d:=tr_decayed(c,first+i);
        if i==0 || d!=prev {out[k]=AwGroup.{first=i,lr=rate,beta1=c.beta1,beta2=c.beta2,eps=c.eps,decay=ifx d then c.decay else 0};k+=1;prev=d;}
        out[k-1].count+=1;}
    return k;
}
// One step from the scattered sum. w, m, v and g hold every rank's shard,
// rank r's at r*c. The ranks first pass two running sums along, rank 0 to
// the last: the loss (fs_loss) and the gradient's squares, shard by shard,
// which is chapter 118's order. Each rank clips its own shard by the global
// norm and stages its AdamW update. Nothing is written until every rank's
// update is admitted and every new value is within the bound.
fs_apply :: (c:TrConfig,p:FsPlan,run:TrRun,b:TrBatch,s:TrSum,w:[]f64,m:[]f64,v:[]f64,g:[]f64,stage:[]f64,order:[]int)->TrStep {
    n:=p.n;fail:=TrStep.{run=run};
    if p.status!=FsStatus.ok || c.optimizer!=TrOptimizer.adamw {fail.status=TrStatus.config;return fail;}
    if b.status!=TrStatus.ok || s.status!=TrStatus.ok || b.revision!=run.revision || b.micro!=p.micro || s.micro!=b.micro {fail.status=TrStatus.order;return fail;}
    if w.count!=n || m.count!=n || v.count!=n || g.count!=n || stage.count!=3*n || (c.source==TrSource.rows && order.count<c.examples) {fail.status=TrStatus.shape;return fail;}
    cs:=fs_size(p);sum:=0.0;
    for r:0..<p.ranks {for i:r*cs..<(r+1)*cs {sum+=g[i]*g[i];}}
    if !tr_finite(s.loss) || !tr_finite(sum) {fail.status=TrStatus.number;return fail;}
    norm:=sqrt(sum);rate:=tr_rate(c,run.state.step);state:=run.state;bad:=TrStatus.ok;none32:[1]f32;groups:[9]AwGroup;
    for r:0..<p.ranks {
        f:=r*cs;sp:=stage[f..<f+cs];sm:=stage[n+f..<n+f+cs];sv:=stage[2*n+f..<2*n+f+cs];gs:=g[f..<f+cs];
        for i:0..<cs {sp[i]=w[f+i];sm[i]=m[f+i];sv[i]=v[f+i];if c.clip>0 && norm>c.clip {gs[i]=gs[i]*c.clip/norm;}}
        k:=fs_groups(c,rate,f,cs,groups[..]);
        a:=aw_step(groups[0..<k],AwPolicy.{},run.state,sp,gs,sm,sv,none32[0..<0],false);
        if a.status!=AwStatus.ok {bad=TrStatus.optimizer;}
        state=a.state;
    }
    // The vote: the first refusal is the optimizer's, then the bound.
    if bad==TrStatus.ok {for i:0..<n {if !(abs(stage[i])<=c.bound) {bad=TrStatus.bound;}}}
    if bad!=TrStatus.ok {fail.status=bad;return fail;}
    for i:0..<n {w[i]=stage[i];m[i]=stage[n+i];v[i]=stage[2*n+i];}
    r:=run;r.state=state;r.revision+=1;r.cursor=b.after;
    if c.source==TrSource.rows && c.shuffle && b.after.epoch>run.cursor.epoch {next:=tr_shuffle(order[0..<c.examples],c.seed,cast(u64)(b.after.epoch*(c.examples-1)));}
    return TrStep.{run=r,loss=s.loss,rate=rate,norm=norm};
}
```

## The caller's loop

Each rank gathers the parameters and runs chapter 115's model on its own
micro-batches, keeping each micro-batch's gradient in its own slot until
the reduce-scatter (the chain needs them apart). Then the shards are
scattered and `fs_apply` makes the step.

<!-- source: psionic/tests/fully-sharded-checks.coax drive -->
```jai
// Chapter 118's loop, fully sharded. Each rank gathers the parameters,
// runs the model on its own micro-batches, each gradient in its own slot,
// and keeps them until the reduce-scatter; then fs_apply makes the step,
// each rank on its own shard.
fsc_drive :: (f:TtFix,c:TrConfig,x:FsRun,ranks:int,stop:int)->FsRun {
    s:=TT_SMALL;y:=x;g:=f;n:=220;plan:=fs_plan(ranks,n,c.accumulate);y.steps=0;
    if fs_admit(c,plan,g.docs[..],g.tokens[0..<g.n])!=FsStatus.ok {y.status=TrStatus.config;return y;}
    cs:=fs_size(plan);work:[220]f64;grads:[880]f64;ml:[4]f64;msg:[220]f64;sum:[220]f64;stage:[660]f64;w:[600]f64;dw:[600]f64;rows:[32]u8;none:[1]int;
    while y.run.revision<stop {
        b:=tr_next(c,y.run,g.docs[..],g.tokens[0..<g.n],rows[..]);if b.status!=TrStatus.ok {y.status=b.status;return y;}
        for j:0..<ranks {r:=ranks-1-j;y.moved+=fs_gather(plan,r,y.p[..],work[..]);
            for mb:fs_first(plan,r)..<fs_first(plan,r+1) {sp:=tr_rows(c,b,mb);
                d:=tm_decoder_step(s,work[..],rows[sp.first*c.length..<(sp.first+sp.count)*c.length],w[..],dw[..],grads[mb*n..<(mb+1)*n]);
                if d.status!=TdStatus.ok {y.status=TrStatus.number;return y;}
                ml[mb]=d.loss;}}
        for k:0..<ranks {y.moved+=fs_scatter(plan,k,grads[0..<b.micro*n],msg[0..<cs],sum[..]);}
        t:=TrSum.{micro=b.micro,loss=fs_loss(plan,ml[0..<b.micro])};
        a:=fs_apply(c,plan,y.run,b,t,y.p[..],y.m[..],y.v[..],sum[..],stage[..],none[0..<0]);
        if a.status!=TrStatus.ok {y.status=a.status;return y;}
        y.losses[y.steps]=a.loss;y.run=a.run;y.steps+=1;
    }
    return y;
}
```

## Bytes on each rank

`fs_memory` counts one rank's bytes for chapter 115's decoder, 8 bytes a
value, in the same terms as [chapter 116](ch116-train-within-a-memory-budget.md):

- **The shard:** `c` parameters, `2c` moment values and `c` values of the
  scattered gradient sum: `32c` bytes.
- **The gathered copy:** all `p` parameters while the step runs: `8p`.
- **The held gradients:** one whole gradient for each micro-batch the rank
  owns, `ceil(micro / ranks)` at most: `8p` each.
- **One row's activations and scratch,** as chapter 116 counts them.

<!-- source: psionic/lib/fully-sharded.coax memory -->
```jai
// One rank's bytes for chapter 115's decoder trained fully sharded, one row
// a micro-batch, binary64 throughout. The shard: parameters, both moments
// and the scattered gradient, c values each. The working set: the gathered
// copy of all p parameters, and one whole gradient for each micro-batch the
// rank owns, held until the reduce-scatter (the fixed order needs each one
// apart). One row's saved values and scratch at a time, as chapter 116
// counts them.
FsBytes :: struct {status:FsStatus;parameters:int;moments:int;gradients:int;gathered:int;held:int;activations:int;workspace:int;total:int;}
fs_memory :: (s:TdShape,ranks:int,micro:int,recompute:bool)->FsBytes {
    np:=td_parameter_count(s);p:=fs_plan(ranks,np,micro);b:=FsBytes.{status=p.status};
    if p.status!=FsStatus.ok {return b;}
    c:=fs_size(p);b.parameters=8*c;b.moments=16*c;b.gradients=8*c;b.gathered=8*np;b.held=8*np*tr_ceil(micro,ranks);
    b.activations=8*mb_saved(s,recompute);b.workspace=8*mb_scratch(s,recompute);
    b.total=b.parameters+b.moments+b.gradients+b.gathered+b.held+b.activations+b.workspace;
    return b;
}
// The fewest ranks whose every rank fits the budget, or a refusal that
// names the bytes at the most ranks that divide the parameters.
FsFit :: struct {status:FsStatus;ranks:int;bytes:FsBytes;need:int;}
fs_fit :: (s:TdShape,micro:int,recompute:bool,budget:int)->FsFit {
    out:=FsFit.{status=FsStatus.ranks};np:=td_parameter_count(s);ranks:=1;
    while fs_plan(ranks,np,micro).status!=FsStatus.ranks {b:=fs_memory(s,ranks,micro,recompute);
        if b.status==FsStatus.micro {out.status=b.status;return out;}
        if b.status==FsStatus.ok {if b.total<=budget {return FsFit.{ranks=ranks,bytes=b};}out.need=b.total;}
        ranks+=1;}
    return out;
}
```

By hand for chapter 115's decoder (`p = 220`, 600 saved values and 600
scratch values a row, 4 micro-batches) at 4 ranks, `c = 55`:

```text
shard        32 x 55          =  1760
gathered      8 x 220         =  1760
held          8 x 220 x 1     =  1760
activations   8 x 600         =  4800
scratch       8 x 600         =  4800
                                 14880 bytes a rank
```

Chapter 116's planner, on one device, needs at least
`8p + 16p + 8p x 2 + 4800 + 4800 = 18400` bytes for one row at a time: it
refuses a device of 15000 bytes for every plan. Two ranks need 18400 bytes
each (the shard halves, but each rank holds two micro-batch gradients); 4
ranks need 14880 and fit. `fs_fit` finds that: the fewest ranks whose every
rank fits. One rank pays chapter 116's bytes plus the gathered copy and a
gradient for each micro-batch beyond the first.

## Bytes a step moves

<!-- source: psionic/lib/fully-sharded.coax comm -->
```jai
// Bytes one step moves, 8 a value. The all-gather: each rank receives the
// n - c values it lacks. The reduce-scatter: each shard makes R - 1 hops
// along the ranks, then every shard but the last rank's one more to its
// owner. The scalars: the loss and the squares pass along the ranks and
// come back (2 values, 2 (R - 1) hops), and the vote on admission does too
// (1 value).
FsComm :: struct {gather:int;scatter:int;scalars:int;total:int;}
fs_comm :: (p:FsPlan)->FsComm {
    c:=fs_size(p);x:FsComm;r:=p.ranks-1;
    x.gather=8*r*p.n;x.scatter=8*(r*p.n+r*c);x.scalars=8*3*2*r;x.total=x.gather+x.scatter+x.scalars;
    return x;
}
```

At 4 ranks with `n = 220` and `c = 55`:

- The all-gather moves `3 x 220 = 660` values, 5280 bytes. That is chapter
  133's `sd_cost` for a sharded tensor made replicated.
- The reduce-scatter moves `3 x 220` values along the chain plus `3 x 55`
  to the owners: 825 values, 6600 bytes. Chapter 133's `sd_cost` for a
  partial sum made sharded is 5280; the difference is the extra hop.
- The scalars move `3 x 2 x 3 = 18` values, 144 bytes.

The gather and the scatter together move chapter 134's all-reduce bytes,
`2 x 3 x 220` values, plus the extra hop. The whole model is gathered once
a step and kept for backward, so the parameters cross the links once, not
twice.

## A sharded checkpoint

Each rank writes its own shard as a chapter 118 record: chapter 122's
record of `c` values, then the trainer tail. A manifest, written last,
names the rank count, the parameter count, the revision and each record's
digest, and is signed itself. A save writes every record and the manifest,
or nothing.

<!-- source: psionic/lib/fully-sharded.coax save -->
```jai
// A sharded checkpoint: one chapter 118 record a rank (chapter 122's record
// of its shard, then the trainer tail), and a manifest written last:
//   "PSFS" u8 version=1, 3 zero bytes; ranks, n, revision (u64 each);
//   each shard record's digest in rank order; SHA-256 (tag
//   psionic.fully-sharded.v1) of every byte before it.
fs_manifest_size :: (ranks:int)->int {return 64+32*ranks;}
// Writes every record and the manifest, or nothing.
fs_save :: (ids:CkIds,p:FsPlan,run:TrRun,w:[]f64,m:[]f64,v:[]f64,bytes:[]u8,manifest:[]u8)->CkStatus {
    c:=fs_size(p);size:=tr_size(c);
    if p.status!=FsStatus.ok || w.count!=p.n || m.count!=p.n || v.count!=p.n || bytes.count!=p.ranks*size || manifest.count!=fs_manifest_size(p.ranks) {return CkStatus.shape;}
    stage:[103744]u8;h:[576]u8;pointer:[80]u8;h[0]=80;h[1]=83;h[2]=70;h[3]=83;h[4]=1;
    at:=ck_put64(h[..],8,cast(u64)p.ranks);at=ck_put64(h[..],at,cast(u64)p.n);at=ck_put64(h[..],at,cast(u64)run.revision);
    for r:0..<p.ranks {f:=r*c;rec:=stage[r*size..<(r+1)*size];
        s:=tr_save(ids,run,w[f..<f+c],m[f..<f+c],v[f..<f+c],rec,pointer[..]);if s!=CkStatus.ok {return s;}
        at=ck_put(h[..],at,ck_get(rec,ck_size(c)-32));}
    at=ck_put(h[..],at,ck_digest("psionic.fully-sharded.v1",h[0..<at]));
    for i:0..<bytes.count {bytes[i]=stage[i];}
    for i:0..<at {manifest[i]=h[i];}
    return CkStatus.ok;
}
```

A reload names its own plan, which may have another rank count. New rank
`r` covers values `r * c'` up to `(r + 1) * c'`, and reads only the old
records that overlap that range. From 4 ranks (shards of 55) to 5 ranks
(shards of 44): new rank 0 covers 0 to 43 and reads record 0; rank 1
covers 44 to 87 and reads records 0 and 1; ranks 2 and 3 each read two;
rank 4 reads record 3. That is 8 reads. Each record must be the one the
manifest names, at the manifest's revision, and agree with record 0 on
everything but its values. Last, the values are written as one chapter
118 record and read back by `tr_resume`, so every check chapter 118 makes
on a resumed run holds here too.

<!-- source: psionic/lib/fully-sharded.coax load -->
```jai
FsLoad :: struct {status:FsStatus;record:CkStatus;shard:int=-1;reads:int;run:TrRun;}
// Two shard records of one save agree on everything but their values: the
// parent, revision, cursor and optimizer state in the head, and the tail.
fs_agree :: (a:[]u8,b:[]u8,body:int)->bool {
    for i:136..<CK_HEAD {if a[i]!=b[i] {return false;}}
    for i:body+8..<body+48 {if a[i]!=b[i] {return false;}}
    return true;
}
// Reloads a checkpoint saved at any admitted rank count into plan p. Each
// new rank reads only the old shards its range overlaps, and each record
// must be the one the manifest names and agree with the others. The values
// are then written as one chapter 118 record and read back by tr_resume,
// so every check chapter 118 makes on a run holds here too. Refusals write
// nothing.
fs_load :: (c:TrConfig,ids:CkIds,p:FsPlan,manifest:[]u8,bytes:[]u8,docs:[]PkDoc,tokens:[]u8,w:[]f64,m:[]f64,v:[]f64,order:[]int)->FsLoad {
    a:=fs_admit(c,p,docs,tokens);if a!=FsStatus.ok {return FsLoad.{status=a};}
    n:=p.n;if w.count!=n || m.count!=n || v.count!=n {return FsLoad.{status=FsStatus.shape};}
    k:=(manifest.count-64)/32;head:=[8]u8.[80,83,70,83,1,0,0,0];
    if k<1 || manifest.count!=fs_manifest_size(k) {return FsLoad.{status=FsStatus.manifest};}
    for i:0..<8 {if manifest[i]!=head[i] {return FsLoad.{status=FsStatus.manifest};}}
    end:=manifest.count-32;if !ck_equal(ck_digest("psionic.fully-sharded.v1",manifest[0..<end]),ck_get(manifest,end)) {return FsLoad.{status=FsStatus.manifest};}
    if ck_get64(manifest,8)!=cast(u64)k || ck_get64(manifest,16)!=cast(u64)n {return FsLoad.{status=FsStatus.manifest};}
    old:=fs_plan(k,n,p.micro);if old.status!=FsStatus.ok {return FsLoad.{status=old.status};}
    c0:=fs_size(old);size:=tr_size(c0);body:=ck_size(c0);
    if bytes.count<k*size {return FsLoad.{status=FsStatus.record,record=CkStatus.truncated};}
    if bytes.count>k*size {return FsLoad.{status=FsStatus.record,record=CkStatus.corrupt};}
    want:=ids;want.optimizer=tr_identity(c);out:=FsLoad.{};run:TrRun;
    sp:[4096]f64;sm:[4096]f64;sv:[4096]f64;tp:[4096]f64;tm:[4096]f64;tv:[4096]f64;cn:=fs_size(p);
    for r:0..<p.ranks {lo:=r*cn;hi:=lo+cn;
        for s:lo/c0..<(hi-1)/c0+1 {rec:=bytes[s*size..<(s+1)*size];
            x:=ck_read(rec[0..<body],want,tp[0..<c0],tm[0..<c0],tv[0..<c0],false);
            if x.status!=CkStatus.ok {return FsLoad.{status=FsStatus.record,record=x.status,shard=s};}
            t:=tr_tail(rec[body..<size],x.digest,x.meta.revision);
            if t.status!=CkStatus.ok {return FsLoad.{status=FsStatus.record,record=t.status,shard=s};}
            if !ck_equal(x.digest,ck_get(manifest,32+32*s)) || ck_get64(manifest,24)!=cast(u64)x.meta.revision || !fs_agree(rec,bytes[0..<size],body) {return FsLoad.{status=FsStatus.shard,shard=s};}
            run=t.run;run.cursor=x.meta.cursor;run.state=x.meta.state;
            base:=s*c0;from:=lo+base-tr_min(lo,base);to:=tr_min(hi,base+c0);
            ps:=sp[lo..<hi];ms:=sm[lo..<hi];vs:=sv[lo..<hi];pt:=tp[0..<c0];mt:=tm[0..<c0];vt:=tv[0..<c0];
            for at:from..<to {ps[at-lo]=pt[at-base];ms[at-lo]=mt[at-base];vs[at-lo]=vt[at-base];}
            out.reads+=1;}}
    // Chapter 118's own reader admits the whole run.
    whole:[98644]u8;pointer:[80]u8;
    s:=tr_save(want,run,sp[0..<n],sm[0..<n],sv[0..<n],whole[0..<tr_size(n)],pointer[..]);
    if s!=CkStatus.ok {return FsLoad.{status=FsStatus.record,record=s};}
    back:=tr_resume(c,want,pointer[..],whole[0..<tr_size(n)],docs,tokens,w,m,v,order);
    if back.status!=CkStatus.ok {return FsLoad.{status=FsStatus.record,record=back.status};}
    out.run=back.run;return out;
}
```

## The program

The example, [`psionic/examples/fully-sharded.coax`](../examples/fully-sharded.coax),
prints the plan and the hand example, trains chapter 115's decoder for 4
steps at 1, 2, 4, 5, 10 and 11 ranks against chapter 118's own loop,
prints the bytes moved and each rank's bytes, then saves a checkpoint
after step 2 at 4 ranks and reloads it at 2, 5, 1 and 11 ranks.

<!-- source: psionic/examples/fully-sharded.coax example -->
```jai
// Chapter 135: chapter 118's trainer, fully sharded. The ranks run one after
// another on this CPU, last rank first; the bits do not depend on that.
main :: () {
    p:=fs_plan(4,220,4);print("220 parameters and 4 micro-batches over 4 ranks: %\n",fs_status(p.status));
    for r:0..<4 {print("  rank % holds values % to % and owns micro-batch %\n",r,r*fs_size(p),(r+1)*fs_size(p)-1,fs_first(p,r));}

    // The hand example: 3 micro-batches of 4 values over 2 ranks.
    h:=fs_plan(2,4,3);grads:=[12]f64.[3,6,-9,1.5, 0,3,9,6, 6,-3,0,3];losses:=[3]f64.[3,6,1.5];msg:[2]f64;g:[4]f64;
    a:=fs_scatter(h,0,grads[..],msg[..],g[..]);b:=fs_scatter(h,1,grads[..],msg[..],g[..]);
    want:[4]f64;t:TrSum;for k:0..<3 {t=tr_add(TrBatch.{micro=3},t,losses[k],grads[4*k..<4*k+4],want[..]);}
    print("hand example, 2 ranks: shard 0 is % % (% values sent), shard 1 is % % (% sent), loss %; equal to tr_add bit for bit: %\n",
        g[0],g[1],a,g[2],g[3],b,fs_loss(h,losses[..]),tr_same(g[..],want[..]) && f64_bits(t.loss)==f64_bits(fs_loss(h,losses[..])));

    // Chapter 115's decoder at the checks' small shape, 4 rows a step.
    f:=tt_fix();c:=tm_decoder(TT_SMALL,4);
    q:[220]f64;qm:[220]f64;qv:[220]f64;got:[4]f64;none:[1]int;held:[1]int;seed:=td_init(TT_SMALL,CrSeed.{seed=TT_INIT},q[..]);
    o:=tm_decoder_drive(TT_SMALL,c,f.docs[..],f.tokens[0..<f.n],held[0..<0],tr_begin(c,220,f.docs[..],f.tokens[0..<f.n],none[0..<0]).run,q[..],qm[..],qv[..],4,got[..]);
    print("chapter 115's decoder, 220 parameters, chapter 118's loop: % steps, losses % % % %\n",o.steps,got[0],got[1],got[2],got[3]);
    for ranks:[6]int.[1,2,4,5,10,11] {r:=fsc_run(f,c,ranks,4);x:=fs_comm(fs_plan(ranks,220,4));
        print("  % ranks: parameters, moments and losses equal bit for bit: %; % bytes moved a step (gather %, reduce-scatter %)\n",ranks,
            r.status==TrStatus.ok && tr_same(r.p[..],q[..]) && tr_same(r.m[..],qm[..]) && tr_same(r.v[..],qv[..]) && tr_same(r.losses[0..<4],got[..]),8*r.moved/4,x.gather,x.scatter);}
    print("  3 ranks: %\n",fs_status(fs_plan(3,220,4).status));
    x:=fs_comm(p);
    print("bytes a step at 4 ranks: gather % (chapter 133: %), reduce-scatter % (chapter 133: %), scalars %, total %\n",
        x.gather,sd_cost(220,1,4,sd_shard(0),sd_replicated()),x.scatter,sd_cost(220,1,4,sd_partial(),sd_shard(0)),x.scalars,x.total);

    // Bytes on each rank, against chapter 116's planner on one device.
    m:=mb_bytes(TT_SMALL,1,false);print("one device, chapter 116, one row at a time: % bytes; a 15000-byte device: %, need %\n",m.total,mb_plan(TT_SMALL,4,15000).status,mb_plan(TT_SMALL,4,15000).need);
    for ranks:[6]int.[1,2,4,5,10,11] {e:=fs_memory(TT_SMALL,ranks,4,false);
        print("  % ranks: shard % + % + %, gathered %, held %, activations %, scratch %: % bytes a rank\n",ranks,e.parameters,e.moments,e.gradients,e.gathered,e.held,e.activations,e.workspace,e.total);}
    fit:=fs_fit(TT_SMALL,4,false,15000);print("  the fewest ranks that fit 15000 bytes: %, at % bytes a rank\n",fit.ranks,fit.bytes.total);

    // A sharded checkpoint after step 2 at 4 ranks, reloaded at other counts.
    whole:=fsc_run(f,c,4,4);half:=fsc_run(f,c,4,2);bytes:[6640]u8;manifest:[192]u8;
    s:=fs_save(fsc_ids(c),p,half.run,half.p[..],half.m[..],half.v[..],bytes[..],manifest[..]);
    print("checkpoint after step 2 at 4 ranks: %, 4 records of % bytes and a manifest of % bytes\n",s,tr_size(55),fs_manifest_size(4));
    for ranks:[4]int.[2,5,1,11] {y:=fsc_resume(f,c,ranks,bytes[..],manifest[..]);z:=fsc_continue(f,c,ranks,y,4);
        print("  reloaded at % ranks: %, % records read; steps 3 and 4 losses % %; equal to the uninterrupted run: %\n",ranks,fs_status(y.status),y.reads,z.losses[0],z.losses[1],
            tr_same(z.p[..],whole.p[..]) && tr_same(z.m[..],whole.m[..]) && tr_same(z.v[..],whole.v[..]));}
}
```

```text
220 parameters and 4 micro-batches over 4 ranks: admitted
  rank 0 holds values 0 to 54 and owns micro-batch 0
  rank 1 holds values 55 to 109 and owns micro-batch 1
  rank 2 holds values 110 to 164 and owns micro-batch 2
  rank 3 holds values 165 to 219 and owns micro-batch 3
hand example, 2 ranks: shard 0 is 3.0 2.0 (4 values sent), shard 1 is 0.0 3.5 (2 sent), loss 3.5; equal to tr_add bit for bit: true
chapter 115's decoder, 220 parameters, chapter 118's loop: 4 steps, losses 2.9360807937186078 2.761001189796394 2.7895427091516334 2.733675497919888
  1 ranks: parameters, moments and losses equal bit for bit: true; 0 bytes moved a step (gather 0, reduce-scatter 0)
  2 ranks: parameters, moments and losses equal bit for bit: true; 4400 bytes moved a step (gather 1760, reduce-scatter 2640)
  4 ranks: parameters, moments and losses equal bit for bit: true; 11880 bytes moved a step (gather 5280, reduce-scatter 6600)
  5 ranks: parameters, moments and losses equal bit for bit: true; 15488 bytes moved a step (gather 7040, reduce-scatter 8448)
  10 ranks: parameters, moments and losses equal bit for bit: true; 33264 bytes moved a step (gather 15840, reduce-scatter 17424)
  11 ranks: parameters, moments and losses equal bit for bit: true; 36800 bytes moved a step (gather 17600, reduce-scatter 19200)
  3 ranks: refused: the rank count must divide the parameter count, so every shard is the same size
bytes a step at 4 ranks: gather 5280 (chapter 133: 5280), reduce-scatter 6600 (chapter 133: 5280), scalars 144, total 12024
one device, chapter 116, one row at a time: 18400 bytes; a 15000-byte device: budget, need 18400
  1 ranks: shard 1760 + 3520 + 1760, gathered 1760, held 7040, activations 4800, scratch 4800: 25440 bytes a rank
  2 ranks: shard 880 + 1760 + 880, gathered 1760, held 3520, activations 4800, scratch 4800: 18400 bytes a rank
  4 ranks: shard 440 + 880 + 440, gathered 1760, held 1760, activations 4800, scratch 4800: 14880 bytes a rank
  5 ranks: shard 352 + 704 + 352, gathered 1760, held 1760, activations 4800, scratch 4800: 14528 bytes a rank
  10 ranks: shard 176 + 352 + 176, gathered 1760, held 1760, activations 4800, scratch 4800: 13824 bytes a rank
  11 ranks: shard 160 + 320 + 160, gathered 1760, held 1760, activations 4800, scratch 4800: 13760 bytes a rank
  the fewest ranks that fit 15000 bytes: 4, at 14880 bytes a rank
checkpoint after step 2 at 4 ranks: ok, 4 records of 1660 bytes and a manifest of 192 bytes
  reloaded at 2 ranks: admitted, 4 records read; steps 3 and 4 losses 2.7895427091516334 2.733675497919888; equal to the uninterrupted run: true
  reloaded at 5 ranks: admitted, 8 records read; steps 3 and 4 losses 2.7895427091516334 2.733675497919888; equal to the uninterrupted run: true
  reloaded at 1 ranks: admitted, 4 records read; steps 3 and 4 losses 2.7895427091516334 2.733675497919888; equal to the uninterrupted run: true
  reloaded at 11 ranks: admitted, 14 records read; steps 3 and 4 losses 2.7895427091516334 2.733675497919888; equal to the uninterrupted run: true
```

Every rank count gives chapter 118's parameters, moments and losses bit
for bit, and the bytes the loop moved equal the closed forms. Every
reload reads the records its ranges overlap and finishes steps 3 and 4
with the uninterrupted run's bits.

## Refusals

[`psionic/examples/fully-sharded-refuse.coax`](../examples/fully-sharded-refuse.coax)
shows refused plans, two refused steps and damaged checkpoints.

<!-- source: psionic/examples/fully-sharded-refuse.coax refuse -->
```jai
// Chapter 135: plans, steps and checkpoints that are refused. A refusal
// writes nothing.
main :: () {
    print("0 ranks: %\n",fs_status(fs_plan(0,220,4).status));
    print("17 ranks: %\n",fs_status(fs_plan(17,4096,4).status));
    print("220 parameters over 3 ranks: %\n",fs_status(fs_plan(3,220,4).status));
    print("0 micro-batches a step: %\n",fs_status(fs_plan(4,220,0).status));
    c:=fsc_adamw();none:[1]PkDoc;nothing:[1]u8;d:=c;d.optimizer=TrOptimizer.sgd;
    print("SGD: %\n",fs_status(fs_admit(d,fs_plan(2,12,2),none[0..<0],nothing[0..<0])));

    // One step on 3 ranks with a bound only the last rank's new value exceeds.
    e:=c;e.bound=fsc_bound(c)*0.999999;w:[12]f64;m:[12]f64;v:[12]f64;g:[12]f64;order:[6]int;
    a:=fsc_apply(e,3,1,w[..],m[..],v[..],g[..],order[..]);
    print("a new value beyond the bound on rank 2: status %, revision %, value 11 still %\n",a.status,a.run.revision,w[11]);
    a=fsc_apply(c,3,2,w[..],m[..],v[..],g[..],order[..]);
    print("a second moment below zero on rank 2: status %, value 0 on rank 0 still %\n",a.status,w[0]);

    // A checkpoint after step 2 at 4 ranks, damaged before it is reloaded at 2.
    f:=tt_fix();t:=tm_decoder(TT_SMALL,4);half:=fsc_run(f,t,4,2);good:[6640]u8;gm:[192]u8;
    s:=fs_save(fsc_ids(t),fs_plan(4,220,4),half.run,half.p[..],half.m[..],half.v[..],good[..],gm[..]);
    what:=[6]string.["the manifest's digest is broken","the manifest says 3 shards of 220 values","records 1 and 2 are swapped",
        "a value in record 2 is changed","record 3's tail is changed","every record says a step was skipped"];
    cases:=[6]int.[4,7,8,9,10,14];
    for k:0..<6 {x:=fsc_damaged(f,t,good[..],gm[..],cases[k]);
        print("%: %, record %, shard %; anything written: %\n",what[k],fs_status(x.status),x.record,x.shard,x.p[0]!=7 || x.m[219]!=7 || x.v[100]!=7);}
    print("reloaded at 3 ranks: %\n",fs_status(fsc_resume(f,t,3,good[..],gm[..]).status));
}
```

```text
0 ranks: refused: from 1 to 16 ranks
17 ranks: refused: from 1 to 16 ranks
220 parameters over 3 ranks: refused: the rank count must divide the parameter count, so every shard is the same size
0 micro-batches a step: refused: from 1 to 64 micro-batches a step
SGD: refused: fully sharded training shards AdamW's moments, and the configuration names SGD
a new value beyond the bound on rank 2: status bound, revision 2, value 11 still 3.5
a second moment below zero on rank 2: status optimizer, value 0 on rank 0 still -2.0
the manifest's digest is broken: refused: the checkpoint's manifest is damaged or names other parameters, record ok, shard -1; anything written: false
the manifest says 3 shards of 220 values: refused: the rank count must divide the parameter count, so every shard is the same size, record ok, shard -1; anything written: false
records 1 and 2 are swapped: refused: a shard's record is not the one the manifest names, record ok, shard 1; anything written: false
a value in record 2 is changed: refused: a shard's record is refused by chapter 122, record corrupt, shard 2; anything written: false
record 3's tail is changed: refused: a shard's record is refused by chapter 122, record corrupt, shard 3; anything written: false
every record says a step was skipped: refused: a shard's record is refused by chapter 122, record corrupt, shard -1; anything written: false
reloaded at 3 ranks: refused: the rank count must divide the parameter count, so every shard is the same size
```

A refused step leaves every rank's values as they were, even when only
one rank's shard was at fault. A damaged checkpoint writes nothing into
the reloading ranks. Both programs exit with status 0; the refusals are
their output.

## How it is checked

- `psionic/lib/fully-sharded.coax` (`#authority`): plans by hand and every
  refusal, including uneven splits; micro-batch ownership for every rank
  count from 1 to 16; the gather and the reduce-scatter by hand against
  `tr_add`, a rank with no micro-batch, and `-0`; for rank counts 1, 2, 3,
  4, 6 and 12 and 1 to 8 micro-batches, the reduce-scatter equals
  `tr_add` bit for bit and moves the closed form's bytes.
- The sharded step against chapter 118's `tr_apply`, the independent
  reference, at 1 to 12 ranks: clipping (on, off, and a clip equal to the
  norm, where scaling would change some values), decay spans that cross
  shard boundaries, an epoch's shuffle and a step inside an epoch; the
  shard groups by hand.
- Every rank agrees: a bound only one rank's new value exceeds, an
  optimizer refusal on the last or the first rank's shard, an overflowing
  gradient and a loss that is not finite each refuse the whole step and
  write nothing; steps out of order and buffers that do not fit.
- Chapter 115's decoder fully sharded at 1, 2, 4, 5, 10 and 11 ranks
  equals chapter 118's own loop (`tm_decoder_drive`) bit for bit over 4
  steps, and the values the loop moved equal `fs_comm`.
- A checkpoint saved at 4 ranks reloads at 4, 2, 5, 1 and 11 ranks, one
  saved at 5 reloads at 2 and one saved at 1 at 4, each continuing to the
  uninterrupted bits; in rows mode the order is rebuilt for the epoch.
  Seventeen kinds of damage to the manifest or the records are refused with
  the expected status and shard, and write nothing.
- Each rank's bytes by hand for chapter 115's decoder and chapter 116's
  test shape, against `mb_bytes` and `mb_plan`; the bytes a step moves
  against chapter 133's `sd_cost`.
- Mutation testing (`--root-only`) kills all 474 mutants.
- The examples' transcripts, compared exactly in debug and release.

## What is not here yet

- **Measured scaling and peak memory.** The ranks are blocks of one buffer
  run one after another on one CPU thread. The per-rank bytes are an
  account, not a measurement, and the "device" that cannot hold the run is
  a byte budget given to chapter 116's planner, not a real GPU. Measuring
  scaling and per-device peak memory needs ranks on real devices or
  machines and the links between them.
- **One layer at a time.** The whole model is one unit: it is gathered
  once a step and kept for backward. Gathering one layer just before it
  runs, prefetching the next while it runs, and freeing it after would
  make the gathered copy one layer instead of `p` values; chapter 115's
  gradient code returns the whole gradient at once, so that needs a
  backward pass that works layer by layer.
- **Uneven shards.** A rank count must divide the parameter count. Padding
  the last shard is not supported.
- **Other optimizers.** Only AdamW is sharded; a configuration naming SGD
  is refused.

## Public API

- `psionic/lib/fully-sharded.coax`: `fs_plan`, `fs_size`, `fs_first`,
  `fs_admit`, `fs_gather`, `fs_scatter`, `fs_loss`, `fs_groups`,
  `fs_apply`, `fs_manifest_size`, `fs_save`, `fs_agree`, `fs_load`,
  `fs_memory`, `fs_fit`, `fs_comm`, `fs_status`, and the types `FsPlan`,
  `FsLoad`, `FsBytes`, `FsFit`, `FsComm` and `FsStatus`.
- `psionic/tests/fully-sharded-checks.coax`: the fully sharded loop for
  chapter 115's decoder (`fsc_drive`, `fsc_run`, `fsc_resume`,
  `fsc_continue`).

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
- [Chapter 116](ch116-train-within-a-memory-budget.md): the byte account
  and planner the per-rank bytes are compared with.
- [Chapter 118](ch118-one-trainer-for-every-model.md): the trainer whose
  sum and step this chapter keeps.
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md): the
  record each shard is written as.
- [Chapter 124](ch124-adamw-and-explicit-precision.md): AdamW.
- [Chapter 133](ch133-a-tensor-spread-over-devices.md): layouts and the
  closed forms of their collectives.
- [Chapter 134](ch134-data-parallel-training.md): the micro-batch split
  and the chain.
