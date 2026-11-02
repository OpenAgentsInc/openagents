# 141. Distill Mammoth into Elephant

> **Status: partly implemented.** Mammoth's and Elephant's answers to a
> 72-request corpus are recorded on the RTX 4080 through chapters 44 and 46's
> CUDA paths and pinned with both models' identities. Chapter 140's adapter,
> on a stand-in student that reads Elephant's recorded answers, is trained on
> the KL divergence to Mammoth and judged on held-out families by a frozen
> rule, on the CPU in binary64. The adapter on Elephant's own weights (and the
> backward pass through Elephant on the GPU), the exposure ledger's split
> artifact from chapter 247 and a corpus large and hard enough for the two
> models to disagree remain.

Mammoth gives better decisions than Elephant and costs more to run. On the
RTX 4080 one Mammoth decision takes about 1.5 seconds, because most of its
51 GB of weights cross the PCIe link every time; one Elephant decision takes
about 0.23 seconds. Distillation trains the cheaper model to answer the way
the expensive one does. Mammoth's probabilities are the targets, not labels:
a request with a Mammoth answer and no checked outcome counts toward
agreement with Mammoth only, and accuracy and calibration come from gold
labels written before either model ran.

Training Elephant's own weights needs a backward pass through Elephant that
doesn't exist yet. This chapter does every other part for real and puts
chapter 140's stand-in decoder where Elephant's trainable layers would be:

- a request corpus, with gold labels, written before any model run;
- Mammoth's and Elephant's answers to every request, recorded on the GPU and
  pinned with each model's identity;
- targets admitted only from Mammoth's identity and the CUDA profile, with
  each request family on one side of the split;
- a student whose input is Elephant's recorded answer, with a low-rank
  adapter trained on the KL divergence to Mammoth;
- a card for Elephant, the student before and after, and Mammoth on the
  held-out families, and a rule frozen before those families were read.

```shell
coax/bin/coax run psionic/examples/distill-mammoth.coax
psionic/check.sh --only distill-mammoth
```

```text
targets: 72 requests, 48 training (families 0-15), 24 held out (families 16-23): ok
teacher mammoth-1 4570fecb199493c2... on cuda; features from elephant-1 4744758962cc73b7...
recorded time per decision on the RTX 4080: Mammoth 1543.2 ms, Elephant 227.0 ms (medians)
student before: ab8ddec85f16bd9c... (base 192443657adaf5ec...)
training KL to Mammoth: 0.021 before, 0.0201 after 400 AdamW steps: ok
student after: 595f1c3da87eef4a...
held out, 24 requests (against Mammoth: agree, mean KL; against gold: right, Brier, ECE):
  Elephant      : agree 24 of 24, KL 0.0131, right 24 of 24, Brier 0.0017, ECE 0.0182
  student before: agree 24 of 24, KL 0.012, right 24 of 24, Brier 0.0018, ECE 0.0192
  student after : agree 24 of 24, KL 0.0127, right 24 of 24, Brier 0.0031, ECE 0.0262
  Mammoth       : agree 24 of 24, KL 0.0, right 24 of 24, Brier 0.0011, ECE 0.0166
verdict: not promoted (no closer to Mammoth)
```

The distilled student is not promoted. Training lowered its KL divergence to
Mammoth on the training families only slightly, from 0.0210 to 0.0201, and
on the held-out families it moved further from Mammoth (0.0120 to 0.0127)
and further from the gold labels (Brier score 0.0018 to 0.0031). The frozen
rule's first test fails, and the result stands: the recipe was fixed before
the held-out families were read, and it is not tuned on them afterwards.

The corpus explains most of this. Both models answer all 72 requests
correctly and agree on every one, so there is no decision for Mammoth to
teach; what is left is the shape of the confidence, and the two models'
confidences differ by a mean KL of only 0.013 nats on the held-out families.
A distillation that can show a gain needs requests where the two models
disagree.

## The corpus and the recorded answers

The corpus is 72 yes-or-no questions in 24 families of three, each family a
short message with three questions about it
([corpus.tsv](../fixtures/distill-mammoth/corpus.tsv)):

```text
0	0	1	The kettle was switched off an hour ago.	Has the water had time to cool?
1	0	0	The kettle was switched off an hour ago.	Is the kettle still boiling?
2	0	0	The kettle was switched off an hour ago.	Was the kettle switched off a minute ago?
```

The columns are item, family, gold label (1 for yes), message and question.
Families 0 to 15 train and families 16 to 23 are held out; a family never
spans the split, so the student is never judged on a message it trained on.

Chapters 44 and 46's harnesses read the corpus when `PSIONIC_DISTILL_CORPUS`
names it. Each prepares the request with options `yes` and `no`, runs one whole decision
on the placed CUDA engine, and prints the slot probabilities and slot mass
through the same readout as its own decision:

```shell
PSIONIC_DISTILL_CORPUS=psionic/fixtures/distill-mammoth/corpus.tsv \
  psionic/tests/run_mammoth_device.sh /path/to/mammoth-directory 1
```

Each harness first runs its own chapter's decision and checks it against
the recorded CPU result and the independent reference, as before; both
checks passed on these runs. Mammoth's run opened its session in 238 s
(authenticating both shards), placed 13.5 GiB on the card, streamed the
other 34.2 GiB per decision and took 1.54 s for every corpus request; its
peak memory was 73.7 GiB. The two logs are kept as recorded
([elephant-cuda.log](../fixtures/distill-mammoth/elephant-cuda.log),
[mammoth-cuda.log](../fixtures/distill-mammoth/mammoth-cuda.log)), and
`psionic/tests/distill_mammoth_fixture.py` turns them into
[records.coax](../fixtures/distill-mammoth/records.coax): each request's
family, gold label, both models' yes probability, slot mass and time, and
each model's identity, the SHA-256 of its pinned inventory in
`psionic/fixtures/models/pins/`. The script also computes the two recorded
models' held-out scores in Python; a check compares the Coax scores with
them.

The corpus is small, and that is the limit of this chapter's evidence:
72 requests, 24 held out. Mammoth's 72 decisions took under two minutes of
device time, but the session's open and authentication take four minutes,
and the whole run has to fit in one bounded command.

## The student

The student reads Elephant's recorded answer to the request: its yes logit
`ln(p/(1-p))` divided by 16, its slot mass, the logit's size and a constant
1. Chapter 140's frozen base turns that back into an answer: unit 0 is
`tanh(x0)` and the yes logit is `16·h0`, so the base keeps Elephant's logit
up to the saturation of tanh. Units 1 to 3 have zero weights.

<!-- source: psionic/lib/distill-mammoth.coax features -->
```jai
// Elephant's recorded answer to a request, as the student's input: its yes
// logit over 16, its slot mass, the logit's size, and 1. A certain answer
// has no logit: its x0 is not a number, which admission refuses.
dm_features :: (p:f64,mass:f64)->[4]f64 {
    if !(p>0 && p<1) {return [4]f64.[0.0/0.0,mass,0,1];}
    z:=log(p/(1-p))/16;
    return [4]f64.[z,mass,abs(z),1];
}
// The frozen base: unit 0 is tanh(x0) and the yes logit 16 h0, which keeps
// Elephant's own logit to within its saturation.
dm_base :: ()->PtBase {b:PtBase;b.w1[0]=1;b.w2[0]=16;return b;}
```

The adapter is chapter 140's, rank 2 with scale 2 on the hidden layer: it
adds `(α/r)·B·(A·x)` to the hidden pre-activations. Through unit 0 it can
rescale Elephant's logit, shift it, and make it depend on the slot mass. This
is the stand-in for an adapter on Elephant's own layers: the training loop,
the admission, the identities and the card are what carry over.

## The targets

A target names its teacher and the numerical profile that produced it.
Mammoth's answers from chapter 15's CPU reference and from chapter 46's CUDA
path are two target sets, and a run trains on one. Admission refuses a row
from another teacher or profile, a probability outside [0, 1], a gold label
other than 0 or 1, a family out of range, a feature that isn't finite, and a
family with rows on both sides of the split; the refusal names the row:

<!-- source: psionic/lib/distill-mammoth.coax admit -->
```jai
// Every target must name this teacher and profile, carry a probability, a
// 0 or 1 gold label, a family in range and finite features; a family may
// not have rows on both sides of the split, and the refusal names the later
// row of the pair.
dm_admit :: (teacher:[32]u8,profile:DmProfile,rows:[]DmRow)->DmAdmit {
    if rows.count<1 || rows.count>PT_ROWS_MAX {return DmAdmit.{status=DmStatus.shape,row=-1};}
    sides:[DM_FAMILIES]u8;a:=DmAdmit.{row=-1};
    for r,i:rows {
        if !pt_same(r.teacher,teacher) {return DmAdmit.{status=DmStatus.teacher,row=i};}
        if r.profile!=profile {return DmAdmit.{status=DmStatus.profile,row=i};}
        ok:=tb_in(r.p,0,1) && (r.gold==0 || r.gold==1) && r.family>=0 && r.family<DM_FAMILIES;
        for v:r.x {if !dm_finite(v) {ok=false;}}
        if !ok {return DmAdmit.{status=DmStatus.number,row=i};}
        // Bit 1: held out; bit 2: training.
        if r.held {a.heldout+=1;} else {a.train+=1;}
        sides[r.family]|=ifx r.held then cast(u8)1 else cast(u8)2;
        if sides[r.family]==3 {return DmAdmit.{status=DmStatus.leak,row=i};}
    }
    return a;
}
```

## The loss, by hand

For one answer the loss is the binary KL divergence from the teacher's yes
probability `t` to the student's `q`, with `0·log 0 = 0`:

<!-- source: psionic/lib/distill-mammoth.coax kl -->
```jai
// KL(t || s) for one yes-or-no answer, with 0 log 0 = 0.
dm_kl :: (t:f64,s:f64)->f64 {
    k:=0.0;
    if t>0 {k+=t*log(t/s);}
    if t<1 {k+=(1-t)*log((1-t)/(1-s));}
    return k;
}
```

Chapter 140's cross-entropy, with the teacher's probability as a soft label,
is this KL plus the teacher's entropy. The entropy doesn't depend on any
weight, so the two losses have the same gradient, `q − t` at the logit, and
training reuses chapter 140's `pt_gradient` unchanged:

<!-- source: psionic/lib/distill-mammoth.coax loss -->
```jai
// The mean KL from the teacher over one side's rows. Chapter 140's
// cross-entropy with the teacher's probability as the label is this plus
// the teacher's entropy, so its gradient is this loss's gradient.
dm_loss :: (b:PtBase,a:PtAdapter,rows:[]DmRow,held:bool)->f64 {
    total:=0.0;n:=0;
    for r:rows {if r.held==held {total+=dm_kl(r.p,pt_forward(b,a,r.x).p);n+=1;}}
    return total/f64(n);
}
dm_entropy :: (rows:[]DmRow,held:bool)->f64 {
    total:=0.0;n:=0;
    for r:rows {if r.held==held {total+=dm_kl(r.p,0.5)-log(2.0);n+=1;}}
    return -total/f64(n);
}
```

Take one request where Elephant said yes with probability 0.75, its slot
mass was 0.5, and Mammoth said 0.6:

- `x0 = ln 3 / 16 = 0.0686633`, and the input is `[0.0686633, 0.5,
  0.0686633, 1]`.
- The base gives `h0 = tanh(0.0686633)`, `z = 16·h0 = 1.0968890` (Elephant's
  own logit is `ln 3 = 1.0986123`) and `q = σ(z) = 0.7496767`.
- `KL(0.6 ‖ q) = 0.6·ln(0.6/q) + 0.4·ln(0.4/(1−q)) = 0.0538571`.
- At the logit the gradient is `q − 0.6 = 0.1496767`; through `16·tanh` it is
  `g0 = 16·(1 − h0²)·0.1496767 = 2.3835726`.
- `B` starts at zero, so only `B`'s row for unit 0 has a gradient:
  `s·g0·(A_k·x)` for each of the two columns, with `s = α/r = 1`. `A` has
  none yet, and neither do `B`'s rows for units 1 to 3, which the base never
  reads.
- AdamW's first step moves each of those two weights by the learning rate,
  0.02, against its gradient's sign, and the KL falls.

The check `one target by hand` asserts each of these numbers, and a
`#forall` compares the gradient with central differences of the mean KL.

## Training

Training is full-batch AdamW on the training families. Everything is
admitted before the first step, and a refusal returns the trainer unchanged:

<!-- source: psionic/lib/distill-mammoth.coax train -->
```jai
// Full-batch AdamW on the training families only. Everything is admitted
// before the first step; a refusal returns the trainer unchanged.
dm_train :: (b:PtBase,t:PtTrainer,teacher:[32]u8,profile:DmProfile,rows:[]DmRow,steps:int)->DmTrain {
    r:=DmTrain.{trainer=t};
    if steps<0 || steps>PT_STEPS_MAX || t.adapter.rank==0 {r.status=DmStatus.shape;return r;}
    ps:=pt_adapter_ok(t.adapter);
    if ps!=PtStatus.ok {r.status=ifx ps==PtStatus.shape then DmStatus.shape else DmStatus.number;return r;}
    if !pt_same(t.adapter.base,pt_digest(b)) {r.status=DmStatus.base;return r;}
    admitted:=dm_admit(teacher,profile,rows);
    if admitted.status!=DmStatus.ok {r.status=admitted.status;return r;}
    if admitted.train==0 {r.status=DmStatus.empty;return r;}
    n:=pt_count(t.adapter);
    for k:n..<16 {if t.m[k]!=0 || t.v[k]!=0 {r.status=DmStatus.shape;return r;}}
    group:=[1]AwGroup.[AwGroup.{first=0,count=n,lr=DM_LR,beta1=0.9,beta2=0.999,eps=1e-8,decay=0}];
    if !aw_state(t.state) {r.status=DmStatus.number;return r;}
    for k:0..<n {if !aw_finite(t.m[k]) || !tb_in(t.v[k],0,AW_F64_MAX) {r.status=DmStatus.number;return r;}}
    train:[PT_ROWS_MAX]PtRow;count:=dm_targets(rows,false,train[..]);
    g:[16]f64;none:[1]f32;
    for step:0..<steps {
        _:=pt_gradient(b,r.trainer.adapter,train[0..<count],0,g[..]);
        w:=aw_step(group[..],AwPolicy.{},r.trainer.state,r.trainer.adapter.p[0..<n],g[0..<n],r.trainer.m[0..<n],r.trainer.v[0..<n],none[0..<0],false);
        // Admission above leaves AdamW nothing to refuse.
        #assert w.status==AwStatus.ok;
        r.trainer.state=w.state;
    }
    r.kl=dm_loss(b,r.trainer.adapter,rows,false);
    return r;
}
```

A checkpoint is the whole trainer, so 12 steps, a checkpoint and 18 more give
the same bits as 30. Changing a held-out target changes nothing; changing a
training target changes the adapter.

## The card and the frozen rule

One model is scored on the held-out rows against Mammoth (agreement on the
answer and mean KL from Mammoth) and against the gold labels through chapter
16's measure (right answers, Brier score, top-label calibration error over 15
bins):

<!-- source: psionic/lib/distill-mammoth.coax score -->
```jai
// One model's yes probabilities p on rows, against the teacher and the gold
// labels. An answer is yes when p is at least one half.
dm_score :: (rows:[]DmRow,p:[]f64)->DmScore {
    s:DmScore;probs:[2*PT_ROWS_MAX]f64;items:[PT_ROWS_MAX]PqItem;
    for r,i:rows {
        if (p[i]>=0.5)==(r.p>=0.5) {s.agree+=1;}
        s.kl+=dm_kl(r.p,p[i]);
        probs[2*i]=p[i];probs[2*i+1]=1-p[i];
        items[i]=PqItem.{kind=QrKind.choice,count=2,start=2*i,gold=1-r.gold,abstain=-1};
    }
    m:=pq_measure(items[0..<rows.count],probs[..]);
    // Every row is a valid, answered item (a refused measure answers none).
    #assert m.answered==rows.count;
    s.kl/=f64(rows.count);s.right=m.correct;s.brier=m.brier;s.ece=m.ece;
    return s;
}
```

The card scores Elephant's recorded answers, the student before and after
training, and Mammoth itself, all on the same 24 requests. These constants
were fixed before the held-out families were read:

<!-- source: psionic/lib/distill-mammoth.coax frozen -->
```jai
// Frozen before the held-out families were read.
DM_HELD_FIRST :: 16;     // families 16 to 23 are held out
DM_FAMILIES :: 24;
DM_RANK :: 2;
DM_ALPHA :: 2.0;
DM_SEED :: 141;
DM_STEPS :: 400;
DM_LR :: 0.02;
DM_BRIER_SLACK :: 0.02;  // the student may lose at most this much Brier score against gold
```

The rule promotes the student only if it is closer to Mammoth than the
undistilled student, agrees with Mammoth at least as often, and loses no more
than 0.02 of Brier score against gold:

<!-- source: psionic/lib/distill-mammoth.coax gate -->
```jai
// The frozen rule: closer to Mammoth than the undistilled student, agreeing
// with it at least as often, and no more than DM_BRIER_SLACK worse against
// gold.
dm_gate :: (c:DmCard)->DmVerdict {
    if c.status!=DmStatus.ok || c.n<1 {return DmVerdict.{reason=DmReason.card};}
    if c.after.kl>=c.before.kl {return DmVerdict.{reason=DmReason.kl};}
    if c.after.agree<c.before.agree {return DmVerdict.{reason=DmReason.agreement};}
    if c.after.brier>c.before.brier+DM_BRIER_SLACK {return DmVerdict.{reason=DmReason.brier};}
    return DmVerdict.{promoted=true};
}
```

The student before training is the base: its untrained adapter has `B = 0`,
and a check shows its card equals the base's. Its scores differ slightly
from Elephant's because `16·tanh(x0)` saturates.

## The student's identity

The student is a new model. Its identity is a SHA-256 over the base's digest
and the adapter's region, rank, scale and parameters, so any change to any of
them is a different student, and no result from Elephant or Mammoth is
reused for it:

<!-- source: psionic/lib/distill-mammoth.coax student -->
```jai
// The student's identity: SHA-256 over the base's digest, the adapter's
// region, rank and scale, and its 16 parameter words. No result of either
// parent is reused for it.
dm_student :: (b:PtBase,a:PtAdapter)->[32]u8 {
    buf:[170]u8;d:=pt_digest(b);
    for v,i:d {buf[i]=v;}
    buf[32]=cast(u8)a.region;buf[33]=cast(u8)a.rank;n:=pt_put(buf[..],34,a.alpha);
    for v:a.p {n=pt_put(buf[..],n,v);}
    s:[32]u8;#assert sha256_hash_checked(buf[..],s[..],false)==SHA_OK;
    return s;
}
```

## Refusals

Each broken input is refused before the first step, and the trainer stays at
step 0:

```shell
coax/bin/coax run psionic/examples/distill-mammoth-refuse.coax
```

```text
Elephant's answer as a target: teacher (admission row 5); trainer at step 0
one target from Mammoth on the CPU: profile (admission row 9); trainer at step 0
training for Elephant as the teacher: teacher (admission row 0); trainer at step 0
family 16 on both sides: leak (admission row 49); trainer at step 0
Elephant certain, with no logit: number (admission row 3); trainer at step 0
only held-out rows: empty (admission row -1); trainer at step 0
the adapter on another base: base (admission row -1); trainer at step 0
a card over Elephant's answer: teacher
```

<!-- source: psionic/examples/distill-mammoth-refuse.coax refuse -->
```jai
main :: () {
    rows:[72]DmRow;#assert dm_rows(DM_MAMMOTH_ID,DmProfile.cuda,rows[..])==72;
    b:=dm_base();t:=pt_trainer(pt_new(b,PtRegion.hidden,DM_RANK,DM_ALPHA,DM_SEED).adapter);
    bad:=rows;bad[5].teacher=DM_ELEPHANT_ID;
    ex_try("Elephant's answer as a target",b,t,DM_MAMMOTH_ID,DmProfile.cuda,bad[..]);
    bad=rows;bad[9].profile=DmProfile.cpu;
    ex_try("one target from Mammoth on the CPU",b,t,DM_MAMMOTH_ID,DmProfile.cuda,bad[..]);
    ex_try("training for Elephant as the teacher",b,t,DM_ELEPHANT_ID,DmProfile.cuda,rows[..]);
    bad=rows;bad[48].held=false;
    ex_try("family 16 on both sides",b,t,DM_MAMMOTH_ID,DmProfile.cuda,bad[..]);
    bad=rows;bad[3].x=dm_features(1,0.5);
    ex_try("Elephant certain, with no logit",b,t,DM_MAMMOTH_ID,DmProfile.cuda,bad[..]);
    ex_try("only held-out rows",b,t,DM_MAMMOTH_ID,DmProfile.cuda,rows[48..<72]);
    c:=b;c.w2[0]=8;
    ex_try("the adapter on another base",c,t,DM_MAMMOTH_ID,DmProfile.cuda,rows[..]);
    bad=rows;bad[60].teacher=DM_ELEPHANT_ID;
    print("a card over Elephant's answer: %\n",dm_card(b,t.adapter,DM_MAMMOTH_ID,DmProfile.cuda,bad[..]).status);
}
```

## Checks

`coax test psionic/lib/distill-mammoth.coax` runs the hand cases above, the
gradient against central differences, admission's every refusal, training's
resume and split, the card and the rule, and the student's identity. The
independent check compares the recorded models' held-out agreement, KL,
right answers, Brier score and calibration error with the values the Python
script computed from the logs. The unit is `#authority`, because admission
decides which targets train a model and the rule decides promotion; mutation
testing leaves no survivors among 234 mutants. Both examples print the same
bytes in debug and release builds.

## What remains

- **The adapter on Elephant.** The student here is chapter 140's four-input
  stand-in reading Elephant's answer. An adapter on Elephant's own layers
  needs the backward pass through Gated DeltaNet and chapter 12's mask, and
  on the 4080 it needs that backward pass on the GPU.
- **The split artifact.** Splits here are by family, frozen with the corpus.
  Chapter 247's exposure ledger should assign them, and every target should
  name the ledger artifact that did.
- **A harder, larger corpus.** On these 72 requests Elephant and Mammoth
  never disagree, so the only thing to learn is the shape of confidence. A
  corpus with disagreements, and Mammoth's answers recorded in several
  bounded runs, is needed before a distilled student can show a gain.
- **The student's own cost.** The cost compared here is Elephant's and
  Mammoth's recorded device time; a distilled Elephant's latency is
  Elephant's plus its adapter's, measured when it exists.

## Dependencies

- [Chapter 14](ch014-execute-elephant.md) and
  [chapter 15](ch015-execute-mammoth.md): both models.
- [Chapter 16](ch016-are-the-probabilities-any-good.md): quality metrics.
- [Chapters 44](ch044-elephant-on-cuda.md) and
  [46](ch046-mammoth-on-cuda.md): the recorded answers.
- [Chapter 140](ch140-post-train-a-decision-model.md): the adapter, forward
  pass, gradient and digest.
- [Chapter 247](ch247-outcomes-become-training-rows.md): training rows and
  the exposure ledger.

## Sources

- [Chapter plan](PLAN.md)
- [The library](../lib/distill-mammoth.coax),
  [the example](../examples/distill-mammoth.coax),
  [the refusals](../examples/distill-mammoth-refuse.coax)
