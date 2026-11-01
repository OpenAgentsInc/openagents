# 140. Post-train a decision model

> **Status: partly implemented.** A low-rank adapter on a frozen four-input
> stand-in decoder trains, resumes, serves and is promoted only through both of
> chapter 241's gates, on the CPU in binary64. Adapters on Elephant and
> Mammoth themselves, the backward pass through Gated DeltaNet and chapter
> 12's block-bidirectional mask, the abstention term, the serving engine's
> adapter slot and a quantized-base (QLoRA) recipe remain.

Elephant and Mammoth answer typed decision questions with probabilities.
Post-training changes a model so that it decides a new kind of question
better. The pre-reset fine-tuning raised accuracy and made confidence worse,
so an adapter here is promoted only when it passes both of
[chapter 241](ch241-gates-that-can-say-unverifiable.md)'s gates: the decision
gate, for a gain beyond the noise floor, and the probability gate, for log
loss and confident errors.

Fine-tuning Elephant's original weights doesn't fit in a CPU check. This
chapter builds the whole procedure on a model small enough to check by hand:
a frozen decoder with four inputs, one tanh layer of four units and one
yes-or-no logit. The parts that carry over unchanged to a large model are:
- the adapter `W + (α/r)·BA` on a selected region, with `W` frozen;
- a backward pass that passes through the frozen layers but changes only `A`
  and `B`;
- rows admitted only for the model they were recorded on, with no held-out
  group leaking into training;
- a checkpoint that resumes to the same bits;
- an adapter that records its base model and is refused on any other.

```shell
coax/bin/coax run psionic/examples/post-train-decision.coax
psionic/check.sh --only post-train
```

```text
base 83b865c4d32b41e8...: h0 = tanh(x0 + x1), h1 = tanh(x0 - x1), z = 4 h0
held out: 400 rows in 50 groups; the true model's accuracy 0.915 (new task), 0.81 (retained)
zero adapter: 400 of 400 held-out probabilities equal the base's
hidden, 480 rows, cross-entropy + Brier: training loss 0.3688
  new task accuracy 0.675 -> 0.92, floor 0.029, 0 of 1000 at or below it: passed (beyond the floor)
  retained accuracy 0.81 -> 0.8, floor 0.018, is the base ahead? unverifiable (near the floor)
  log loss 0.6272 -> 0.3083, confident errors 34 -> 5, calibration error 0.113 -> 0.051: passed (lower log loss)
  promoted: true
readout, 480 rows, cross-entropy + Brier: training loss 0.7279
  new task accuracy 0.675 -> 0.675, floor 0.032, 1000 of 1000 at or below it: failed (within the floor)
  retained accuracy 0.81 -> 0.81, floor 0.015, is the base ahead? failed (within the floor)
  log loss 0.6272 -> 0.5584, confident errors 34 -> 0, calibration error 0.113 -> 0.041: passed (lower log loss)
  promoted: false
hidden, 16 rows, cross-entropy only: training loss 0.0186
  new task accuracy 0.675 -> 0.9, floor 0.111, 1 of 1000 at or below it: passed (beyond the floor)
  retained accuracy 0.81 -> 0.71, floor 0.126, is the base ahead? unverifiable (near the floor)
  log loss 0.6272 -> 0.7872, confident errors 34 -> 76, calibration error 0.113 -> 0.178: failed (higher log loss)
  promoted: false
resume after 120 of 300 steps: 16 of 16 parameters the same bits, step 300; 10 parameters moved; base digest unchanged: true
a training row recorded on another base: identity at row 7
a held-out group that also trains: leak at row 496
training the adapter on another base: identity, still at step 300
serving it on another base: identity
that base with the adapter unloaded: ok (p 0.0318)
```

The [example](../examples/post-train-decision.coax) runs in about ten
seconds, most of it compiling.

The first adapter is promoted. The second lowers log loss but can't learn the
new task, so the decision gate fails it. The third passes the decision gate,
raising new-task accuracy from 0.675 to 0.9, and makes confidence worse: its
log loss rises and its confident errors go from 34 to 76. This is the
pre-reset failure, and the probability gate catches it.

## The base model and the task

The base computes `h0 = tanh(x0 + x1)` and `h1 = tanh(x0 − x1)`. Units 2 and 3
have zero weights, so they're always 0. It answers yes with probability
`σ(4·h0)`.

Each row is a decision with a recorded outcome. Its group and time decide its
split, as in [chapter 247](ch247-outcomes-become-training-rows.md). Even rows
are the new task; odd rows are a task the base already does:

<!-- source: psionic/lib/post-train.coax world -->
```jai
// per rows in each of groups groups, from group first; group g's rows are
// recorded at time g. Even rows are the new task (x2 matters, x3 = 1), odd
// rows the retained one (x2 = x3 = 0). The truth's logit is 3 (x0 + x1),
// plus 8 x2 - 2 on the new task.
pt_world :: (seed:u64,first:int,groups:int,per:int,model:[32]u8,rows:[]PtRow)->int {
    if groups<1 || per<1 || per>8 || first<0 || first+groups>PT_GROUPS_MAX || rows.count!=groups*per {return -1;}
    for g:0..<groups {for j:0..<per {
        c:=cast(u64)((first+g)*64+j*8);
        r:=PtRow.{group=first+g,time=first+g,model=model,task=j%2};
        r.x[0]=2*tb_uniform(seed,c)-1;r.x[1]=2*tb_uniform(seed,c+1)-1;
        z:=3*(r.x[0]+r.x[1]);
        if r.task==0 {r.x[2]=2*tb_uniform(seed,c+2)-1;r.x[3]=1;z+=8*r.x[2]-2;}
        r.y=floor(tb_uniform(seed,c+3)+tb_sigmoid(z));
        rows[g*per+j]=r;
    }}
    return groups*per;
}
```

On the retained task the truth is `σ(3(x0 + x1))`, which the base already
follows. On the new task the answer also depends on `x2`, which the base never
reads, so its accuracy there is 0.675. The true model reaches 0.915 on the
same held-out rows. No adapter can be expected to do better.

## The adapter

The adapter adds `(α/r)·B·(A·u)` to one matrix of the base, where `u` is that
matrix's input. `A` is `r × 4` and `B` is `out × r`. On the hidden region, `u`
is `x` and `out` is 4. On the readout region, `u` is the hidden layer and
`out` is 1. All 25 base weights stay frozen.

<!-- source: psionic/lib/post-train.coax forward -->
```jai
// The frozen layers, plus (alpha / rank) B (A u) on the selected region,
// where u is the input to that region's matrix.
pt_forward :: (b:PtBase,a:PtAdapter,x:[4]f64)->PtPass {
    q:PtPass;on:=a.rank>0;s:=0.0;if on {s=a.alpha/f64(a.rank);}
    hidden:=on && a.region==PtRegion.hidden;
    readout:=on && a.region==PtRegion.readout;
    if hidden {q.u=x;}
    for k:0..<a.rank {if hidden {for i:0..<4 {q.au[k]+=a.p[k*4+i]*x[i];}}}
    for o:0..<PT_H {
        pre:=b.c1[o];
        for i:0..<PT_D {pre+=b.w1[o*4+i]*x[i];}
        if hidden {
            delta:=0.0;for k:0..<a.rank {delta+=a.p[a.rank*4+o*a.rank+k]*q.au[k];}
            pre+=s*delta;
        }
        q.h[o]=tb_tanh(pre);
    }
    q.z=b.c2;for o:0..<PT_H {q.z+=b.w2[o]*q.h[o];}
    if readout {
        q.u=q.h;delta:=0.0;
        for k:0..<a.rank {for i:0..<4 {q.au[k]+=a.p[k*4+i]*q.h[i];}delta+=a.p[a.rank*4+k]*q.au[k];}
        q.z+=s*delta;
    }
    q.p=tb_sigmoid(q.z);
    return q;
}
```

A new adapter draws `A` from its seed and sets `B` to zero. Then `B·(A·u)` is
zero, so the adapter adds exactly +0 to each pre-activation. The example
checks every held-out row: 400 of 400 probabilities match the base's, bit for
bit.

The region decides what the adapter can see. The readout reads `h`, and `h`
doesn't contain `x2`, so no readout adapter can learn the new task. The
transcript shows this: the readout candidate's new-task accuracy stays at
0.675.

## One step by hand

Take one row, `x = [0.5, 0, 1, 1]`, label yes, and a hidden-region adapter
with rank 1, α = 1, `A = [0, 0, 1, 0]` and `B = 0`. The scale is α/r = 1.

Forward, the adapter adds nothing:
- `h0 = tanh(0.5) = 0.462117`, `h1 = tanh(0.5)`, `h2 = h3 = 0`
- `z = 4·h0 = 1.848469`, `p = σ(z) = 0.863947`
- cross-entropy `softplus(z) − z = 0.146244`

Backward, through the frozen readout and tanh:
- `dz = p − y = −0.136053`
- `g0 = dz · w2[0] · (1 − h0²) = −0.136053 · 4 · 0.786448 = −0.427994`
- `g1 = g2 = g3 = 0`, because `w2[1..3]` are 0
- `A·x = 1`, so `dB[0] = 1 · g0 · 1 = −0.427994`
- `dA = 1 · (Bᵀg) · x = 0`, because `B` is still 0

Only `B[0]` has a gradient. AdamW's first step moves a parameter by about
`lr · g / |g|`, because bias correction makes the first moment `g` and the
second `g²`. With `lr = 0.125`, `B[0]` becomes 0.125. Then:
- the pre-activation of `h0` is `0.5 + 1 · 0.125 · (A·x) = 0.625`
- `z = 4·tanh(0.625) = 2.218399`, `p = 0.901890`
- cross-entropy falls from 0.146244 to 0.103263

The check `one AdamW step by hand` asserts these values, the moments
`(1 − β1)·g` and `(1 − β2)·g²`, and that no other parameter moved.

## Backward through the frozen layers

The loss is cross-entropy plus a weight times the Brier score. Brier is
`(p − y)²`, so it adds `2(p − y)·p(1 − p)` to `dz`.

<!-- source: psionic/lib/post-train.coax gradient -->
```jai
// Mean of cross-entropy plus brier times the Brier score over rows, and its
// gradient in the adapter's used parameters (A, then B). The upstream
// gradient g passes back through the frozen readout and tanh to the hidden
// region; the frozen weights get none.
pt_gradient :: (b:PtBase,a:PtAdapter,rows:[]PtRow,brier:f64,grad:[]f64)->f64 {
    for k:0..<16 {grad[k]=0;}
    s:=a.alpha/f64(a.rank);out:=pt_out(a.region);total:=0.0;
    for row:rows {
        q:=pt_forward(b,a,row.x);e:=q.p-row.y;
        total+=tb_softplus(q.z)-row.y*q.z+brier*e*e;
        dz:=e+brier*2*e*q.p*(1-q.p);
        g:[4]f64;
        if a.region==PtRegion.hidden {for o:0..<PT_H {g[o]=dz*b.w2[o]*(1-q.h[o]*q.h[o]);}}
        else {g[0]=dz;}
        for k:0..<a.rank {
            back:=0.0;
            for o:0..<out {back+=a.p[a.rank*4+o*a.rank+k]*g[o];grad[a.rank*4+o*a.rank+k]+=s*g[o]*q.au[k];}
            for i:0..<4 {grad[k*4+i]+=s*back*q.u[i];}
        }
    }
    n:=f64(rows.count);
    for k:0..<pt_count(a) {grad[k]/=n;}
    return total/n;
}
pt_loss :: (b:PtBase,a:PtAdapter,rows:[]PtRow,brier:f64)->f64 {g:[16]f64;return pt_gradient(b,a,rows,brier,g[..]);}
```

The check `backward through frozen layers matches central differences`
compares every gradient entry with a finite-difference estimate of the loss,
for both regions, rank 2, with and without the Brier term. A parameter past
the adapter's used count must have exactly zero gradient.

The frozen weights get no gradient, but the dead units show up anyway. The
hidden adapter's `B` rows for units 1 to 3 never get a gradient, because
`w2[1..3]` are 0. So 10 of the 16 parameters move: all of `A` and the first
row of `B`. The transcript's resume line counts them.

## Which rows may train

<!-- source: psionic/lib/post-train.coax admit -->
```jai
// Every row must come from this base, with a finite input, a 0 or 1 label
// and task, and a group in range. A group with a held-out row may have no
// row in any other split; the refusal names the later row of the pair.
pt_admit :: (model:[32]u8,rows:[]PtRow,cuts:TrCuts)->PtAdmit {
    n:=rows.count;
    if n<1 || n>PT_ROWS_MAX || !tr_cuts_ok(cuts) {return PtAdmit.{status=PtStatus.shape,row=-1};}
    roles:[PT_GROUPS_MAX]u8;a:=PtAdmit.{row=-1};
    for r,i:rows {
        if !pt_same(r.model,model) {return PtAdmit.{status=PtStatus.identity,row=i};}
        ok:=r.group>=0 && r.group<PT_GROUPS_MAX && (r.y==0 || r.y==1) && (r.task==0 || r.task==1);
        for v:r.x {if !pt_finite(v) {ok=false;}}
        if !ok {return PtAdmit.{status=PtStatus.number,row=i};}
        // Bit 1: held out; bit 2: anything else.
        role:=tr_role(cuts,r.time);held:=role==TrRole.heldout;
        if held {a.heldout+=1;} else if role==TrRole.train {a.train+=1;}
        roles[r.group]|=ifx held then cast(u8)1 else cast(u8)2;
        if roles[r.group]==3 {return PtAdmit.{status=PtStatus.leak,row=i};}
    }
    return a;
}
```

Every row carries the digest of the base it was recorded on. A row recorded on
any other base is refused, because its outcome may depend on what that model
said. A group with a held-out row may have no row in any other split. If it
did, the adapter would train on the same situations it's judged on. Either
refusal names the row, and it rejects the whole set: there is no partial
admission.

## Train, checkpoint, resume

<!-- source: psionic/lib/post-train.coax train -->
```jai
// Full-batch AdamW on the training split only. Everything is admitted
// before the first step; a refusal returns the trainer unchanged.
pt_train :: (b:PtBase,t:PtTrainer,rows:[]PtRow,cuts:TrCuts,recipe:PtRecipe,steps:int)->PtTrain {
    r:=PtTrain.{trainer=t};
    if steps<0 || steps>PT_STEPS_MAX {r.status=PtStatus.shape;return r;}
    r.status=pt_adapter_ok(t.adapter);
    if r.status==PtStatus.ok && t.adapter.rank==0 {r.status=PtStatus.shape;}
    if r.status!=PtStatus.ok {return r;}
    n:=pt_count(t.adapter);
    for k:n..<16 {if t.m[k]!=0 || t.v[k]!=0 {r.status=PtStatus.shape;return r;}}
    if !pt_same(t.adapter.base,pt_digest(b)) {r.status=PtStatus.identity;return r;}
    admitted:=pt_admit(t.adapter.base,rows,cuts);
    if admitted.status!=PtStatus.ok {r.status=admitted.status;return r;}
    if admitted.train==0 {r.status=PtStatus.empty;return r;}
    if !tb_in(recipe.brier,0,PT_BIG) {r.status=PtStatus.number;return r;}
    group:=[1]AwGroup.[AwGroup.{first=0,count=n,lr=recipe.lr,beta1=0.9,beta2=0.999,eps=1e-8,decay=recipe.decay}];
    if !aw_hyper(group[0]) || !aw_state(t.state) {r.status=PtStatus.number;return r;}
    for k:0..<n {if !aw_finite(t.m[k]) || !tb_in(t.v[k],0,AW_F64_MAX) {r.status=PtStatus.number;return r;}}
    train:[PT_ROWS_MAX]PtRow;count:=0;
    for row:rows {if tr_role(cuts,row.time)==TrRole.train {train[count]=row;count+=1;}}
    g:[16]f64;none:[1]f32;
    for step:0..<steps {
        l:=pt_gradient(b,r.trainer.adapter,train[0..<count],recipe.brier,g[..]);
        w:=aw_step(group[..],AwPolicy.{},r.trainer.state,r.trainer.adapter.p[0..<n],g[0..<n],r.trainer.m[0..<n],r.trainer.v[0..<n],none[0..<0],false);
        // Admission above leaves AdamW nothing to refuse.
        #assert w.status==AwStatus.ok;
        r.trainer.state=w.state;
    }
    r.loss=pt_loss(b,r.trainer.adapter,train[0..<count],recipe.brier);
    return r;
}
```

Training uses full-batch AdamW, from
[chapter 124](ch124-adamw-and-explicit-precision.md), on the training split
only. The checks confirm this: changing a held-out or
development label leaves the run's bits unchanged. Changing a training label
changes them.

Only `A` and `B` carry optimizer state. A checkpoint is the whole `PtTrainer`:
the adapter, both moments and the step count. The example trains 300 steps
straight, then 120 steps, a copy and 180 more. All 16 parameters match bit for
bit. The checks also show that resuming with the moments reset gives a
different result.

## Identity and serving

<!-- source: psionic/lib/post-train.coax digest -->
```jai
pt_put :: (buf:[]u8,n:int,v:f64)->int {
    w:=f64_bits(v);
    for i:0..<8 {buf[n+i]=cast(u8)((w>>cast(u64)(i*8))&0xff);}
    return n+8;
}
// SHA-256 over w1, c1, w2 and c2 in that order.
pt_digest :: (b:PtBase)->[32]u8 {
    buf:[200]u8;n:=0;
    for v:b.w1 {n=pt_put(buf[..],n,v);}
    for v:b.c1 {n=pt_put(buf[..],n,v);}
    for v:b.w2 {n=pt_put(buf[..],n,v);}
    n=pt_put(buf[..],n,b.c2);
    d:[32]u8;#assert sha256_hash_checked(buf[..],d[..],false)==SHA_OK;
    return d;
}
```

The base digest is SHA-256 over the 25 weights. An adapter records the digest
of the base it was made for, and training and serving both compare it to the
base in hand:

<!-- source: psionic/lib/post-train.coax serve -->
```jai
// Serving checks the adapter against the base it is loaded on; rank 0 is
// the base alone.
pt_serve :: (b:PtBase,a:PtAdapter,x:[4]f64)->PtServe {
    s:=pt_adapter_ok(a);
    if s!=PtStatus.ok {return PtServe.{status=s};}
    if a.rank>0 && !pt_same(a.base,pt_digest(b)) {return PtServe.{status=PtStatus.identity};}
    for v:x {if !pt_finite(v) {return PtServe.{status=PtStatus.number};}}
    return PtServe.{p=pt_forward(b,a,x).p};
}
```

An adapter with rank 0 is no adapter. Unloading one serves the base alone.
The example changes one bias of the base, to 0.25. Training or serving the
adapter on that base is refused with `identity`, and the trainer stays at step
300. The base with nothing loaded still serves.

## The gates decide promotion

Every candidate trains 16 times. Each seed draws its own training rows and
its own `A`, and uses the same 400 held-out rows in 50 groups. Seed 1's
adapter is the one offered for promotion. All 16 seeds together measure how
far accuracy moves by itself:
- **New task:** chapter 241's decision gate compares seed 1 with the base on
  the new-task rows, against a floor from the 16 seeds as blocks of one.
- **Probability:** the probability gate compares seed 1's yes probabilities
  with the base's on all 400 rows.
- **Retained task:** the decision gate runs the other way, asking whether the
  base is clearly ahead on the retained rows.

An adapter is promoted when the new-task gate passes, the probability gate
passes, and the base is not clearly ahead on the retained task.

| Candidate | New task | Probability | Promoted |
| --- | --- | --- | --- |
| Hidden region, 480 rows, cross-entropy + Brier | 0.675 → 0.92, passed | log loss 0.6272 → 0.3083, passed | yes |
| Readout region, 480 rows, cross-entropy + Brier | 0.675 → 0.675, failed | log loss 0.6272 → 0.5584, passed | no |
| Hidden region, 16 rows, cross-entropy only | 0.675 → 0.9, passed | log loss 0.6272 → 0.7872, 76 confident errors, failed | no |

Each gate stops one of the two failing candidates:
- **The readout candidate** removes all 34 of the base's confident errors and
  lowers log loss, so the probability gate passes it. It has learned nothing
  about the new task, and the decision gate fails it.
- **The 16-row candidate** learns the new task from 16 rows, and the decision
  gate passes it. Its probabilities get worse on every measure, and the
  probability gate fails it. This is the case the chapter exists for: a
  decision gate alone would have promoted it.

The retained check is the weakest of the three. It's clear only for the
readout candidate, which can't move far from the base. For the other two it
says unverifiable (near the floor). The 16-row candidate's retained accuracy
falls from 0.81 to 0.71, yet its seeds vary so much that the floor is 0.126.
On 200 retained rows the check can't rule out a regression of that size. The
rule blocks only a clear regression, and the transcript prints unverifiable
rather than reporting a pass.

## Checks

The unit's checks, before the implementation in the
[source](../lib/post-train.coax):
- the base's forward pass and the adapter's, by hand, for both regions and
  both ranks;
- a zero adapter reproduces the base exactly on 64 random inputs, for both
  regions and both ranks;
- the gradient against central differences, and one row and one AdamW step by
  hand;
- the digest changes with each of the 25 weights and with a signed zero, and
  matches SHA-256 of the hand-built bytes;
- admission: another base's row, a leaked group in either order, labels,
  tasks, groups, inputs, cuts and the 1,024-row limit;
- training touches only the used adapter parameters, uses only the training
  split, resumes to the same bits, and refuses before any step;
- serving loads, unloads and refuses another base.

## What remains

- Adapters on Elephant and Mammoth's own matrices, with the backward pass
  through Gated DeltaNet and chapter 12's block-bidirectional mask,
  qualified against [chapter 106](ch106-differentiate-any-tensor-program.md).
  That run belongs on the GPU path.
- An abstention term in the loss.
- Loading and unloading through the serving engine of
  [chapter 173](ch173-serve-a-model-with-an-honest-capability.md), rather than
  through `pt_serve`.
- A quantized base, which needs its full recipe named before it is called
  QLoRA.
- Rows recorded from real decisions, admitted through chapter 247's ledger,
  instead of the synthetic world.

## Sources

- [Chapter plan](PLAN.md)
- [The library](../lib/post-train.coax) and the
  [example](../examples/post-train-decision.coax)
