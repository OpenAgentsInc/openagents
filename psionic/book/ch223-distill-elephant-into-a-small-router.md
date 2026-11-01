# 223. Distill Elephant into a small router

> **Status: partly implemented.** One head of [chapter 222](ch222-a-decision-in-milliseconds.md)'s
> encoder (fe-tiny-v1, body frozen) is distilled from a recorded teacher's
> answer distributions under [chapter 118](ch118-one-trainer-for-every-model.md)'s
> trainer, on a leak-free split, with a fitted temperature, a quality card,
> a rule frozen before the test split was read, a cascade with written costs
> and a latency receipt on one CPU (taken on a loaded host; the 20 ms gate
> was missed there and still needs a quiet-host run). The teacher is a recorded stand-in, not
> Elephant; the student fails the frozen rule. Elephant as the teacher,
> chapter 247's `distill-v1` targets, training the encoder's body and
> shadow promotion are not built; the last section lists them.

Elephant answers decisions well but costs a full decoder pass: on
[chapter 16](ch016-are-the-probabilities-any-good.md)'s CPU reference
backend each recorded decision took 69 to 81 minutes. Chapter 222's encoder
answers in about 12 ms. If the small model could imitate Elephant on the easy
requests, a router could answer those itself and send only the rest to
Elephant. This chapter trains such a student by distillation, measures it
against both its teacher and the outcomes, and lets a written cost rule decide
how much it may answer.

The answer here is honest and mostly negative. The student learns the teacher
only partly; once its confidence is calibrated, the cost rule lets it answer
one test request in 96, and the frozen rule refuses it. The machinery around
that verdict is the point of the chapter: every step that would let a weak
router look strong is checked.

```shell
coax/bin/coax test psionic/lib/distill.coax
coax/bin/coax run psionic/examples/distill-router.coax
psionic/tests/check_distill.sh 200 RECEIPT.json   # times the trained student, on this CPU
```

## The teacher is a stand-in

Distillation needs the teacher's distribution on every training, development
and test request. The repository holds 12 recorded Elephant decisions with
full probabilities ([chapter 16](ch016-are-the-probabilities-any-good.md)'s
held-out run). That is too few to train on and still hold any out, and each
new one costs over an hour on the CPU. So the teacher here is a recorded
stand-in, written down plainly:

- **Requests.** 64 families of 8 requests, 16 token IDs each. A family's
  first 8 IDs are shared (SplitMix64 seed 2233, below 15); the last 8 are the
  request's own (seed 2234, below 64).
- **Teacher.** For each of three options and each token ID, a fixed value in
  [-1, 1) (seed 2236). The option's logit is the sum of its values over the
  request's 16 tokens, and the teacher's probabilities are their softmax at
  temperature 1.
- **Outcomes.** One label per request, drawn from the teacher's own
  probabilities (seed 2235). The teacher is therefore calibrated by
  construction, and it is right exactly as often as its probabilities say.

[`distill_oracle.py --teacher`](../tests/distill_oracle.py) writes the
probabilities, labels and the teacher's identity, FNV-1a over the 1,536
probabilities' bits, to [a fixture](../fixtures/distill/teacher.coax). The
library reads that file as it would read recorded Elephant output: it checks
each row is a distribution and that the identity matches, and a teacher with
one probability moved by 10^-16 is a different target set.

The stand-in's scale was chosen on a pilot draw with other request seeds and
on this draw's training and development splits only. Its test split was first
read after the rule below was written.

## The student and the loss

The student is chapter 222's model, unchanged except for head 1: three rows
of 16 weights that turn the pooled vector into three logits, divided by the
head's fixed temperature 1/2. The three layers below stay frozen. Training
them needs gradients through attention, which this chapter does not build.

The pooled vector is read through chapter 222's own forward pass rather than
a copy of it: with head row o set to the unit vector o, logit o equals pooled
value o exactly, so two passes give all 16.

<!-- source: psionic/lib/distill.coax features -->
```jai
// The frozen encoder's pooled vector, read through chapter 222's own forward
// pass: with head row o set to unit vector o, logit o is pooled value o
// exactly. Two passes give all 16. The caller's weights are not changed.
// Pooled values first to first + 8 (those below 16), through the 9 head rows.
ds_read :: (v:[]f64,k:FeWork,ids:[]int,first:int,out:[]f64)->FeStatus {
    for o:0..<FE_LOGITS {for c:0..<DS_FEATURES {v[FE_HEAD_AT+o*DS_FEATURES+c]=ifx c==first+o then 1 else 0;}}
    z:[9]f64;st:=fe_forward(FeEngine.reference,v,FeQ8.{},k,ids,z[..]);
    if st!=FeStatus.ok {return st;}
    for o:0..<FE_LOGITS {if first+o<DS_FEATURES {out[first+o]=z[o];}}
    return FeStatus.ok;
}
ds_features :: (w:[]f64,k:FeWork,ids:[]int,out:[]f64)->FeStatus #export_c {
    if w.count!=FE_WEIGHTS || out.count!=DS_FEATURES {return FeStatus.storage;}
    v:[8848]f64;for i:0..<FE_WEIGHTS {v[i]=w[i];}
    st:=ds_read(v[..],k,ids,0,out);if st!=FeStatus.ok {return st;}
    return ds_read(v[..],k,ids,FE_LOGITS,out);
}
// Writes a trained head into chapter 222's weights: the student is the same
// model shape with a new identity.
ds_install :: (w:[]f64,head:[]f64)->DsStatus #export_c {
    if w.count!=FE_WEIGHTS || head.count!=DS_PARAMS {return DsStatus.shape;}
    for i:0..<DS_PARAMS {w[DS_HEAD_AT+i]=head[i];}
    return DsStatus.ok;
}
```

At distillation temperature T, the teacher's distribution p becomes p_T,
proportional to p^(1/T), and the student's logits s become
q_T = softmax(s / (T/2)). The loss for one request is

    L = T² KL(p_T ‖ q_T) = T² Σ_c p_T,c (log p_T,c − log q_T,c)

and its gradient with respect to logit c is (T / (1/2)) (q_T,c − p_T,c), so
with respect to weight (c, k) it is that times feature k. The T² keeps the
gradient's size comparable across temperatures.

<!-- source: psionic/lib/distill.coax loss -->
```jai
// The student's logits: head row c times the features, in chapter 222's order.
// Not a number when the slices are not 48 weights and 16 features.
ds_logits :: (w:[]f64,x:[]f64)->[3]f64 {
    s:[3]f64;
    if w.count!=DS_PARAMS || x.count!=DS_FEATURES {for c:0..<DS_CLASSES {s[c]=0.0/0.0;}return s;}
    for c:0..<DS_CLASSES {acc:=0.0;for k:0..<DS_FEATURES {acc+=w[c*DS_FEATURES+k]*x[k];}s[c]=acc;}
    return s;
}
ds_log_soft :: (a:[3]f64)->[3]f64 {
    top:=qz_larger(qz_larger(a[0],a[1]),a[2]);sum:=0.0;
    for c:0..<3 {sum+=exp(a[c]-top);}
    l:[3]f64;for c:0..<3 {l[c]=a[c]-top-log(sum);}
    return l;
}
// One request: T^2 KL(p_T || q_T), where p_T is the teacher at temperature T
// (log p / T, renormalized) and q_T the student at T (logits over 1/2 x T).
// Adds the gradient, (T / (1/2)) (q_T - p_T)_c times feature k, into g. A
// slice of the wrong length gives not a number and adds nothing.
ds_loss :: (w:[]f64,x:[]f64,p:[]f64,t:f64,g:[]f64)->f64 {
    if w.count!=DS_PARAMS || x.count!=DS_FEATURES || p.count!=DS_CLASSES || g.count!=DS_PARAMS {return 0.0/0.0;}
    a:[3]f64;b:[3]f64;s:=ds_logits(w,x);
    for c:0..<DS_CLASSES {a[c]=s[c]/(DS_HEAD_TEMP*t);b[c]=log(p[c])/t;}
    lq:=ds_log_soft(a);lp:=ds_log_soft(b);kl:=0.0;
    for c:0..<DS_CLASSES {
        pt:=exp(lp[c]);kl+=pt*(lp[c]-lq[c]);
        d:=(t/DS_HEAD_TEMP)*(exp(lq[c])-pt);
        for k:0..<DS_FEATURES {g[c*DS_FEATURES+k]+=d*x[k];}
    }
    return t*t*kl;
}
// The mean loss and gradient over requests rows.
ds_batch :: (w:[]f64,d:DsData,rows:[]int,t:f64,g:[]f64)->f64 {
    for i:0..<DS_PARAMS {g[i]=0;}
    loss:=0.0;
    for r:rows {loss+=ds_loss(w,d.feats[r*DS_FEATURES..<(r+1)*DS_FEATURES],d.teacher[r*DS_CLASSES..<(r+1)*DS_CLASSES],t,g);}
    n:=f64(rows.count);for i:0..<DS_PARAMS {g[i]/=n;}
    return loss/n;
}
```

## Work it by hand first

**A KL gradient.** Take zero weights, features (1, −1/2, 0, ...), teacher
p = (2/3, 1/6, 1/6) and T = 2. Then p_T is proportional to √p: (2, 1, 1)/√6,
which normalizes to (1/2, 1/4, 1/4). The student is uniform, q_T = 1/3 each.
The loss is 4 (1/2 ln(3/2) + 2 × 1/4 ln(3/4)) = 2 ln(9/8) ≈ 0.2356. The
gradient on the logits is 4 (1/3 − 1/2, 1/3 − 1/4, 1/3 − 1/4) =
(−2/3, 1/3, 1/3), so weight (0, 0) gets −2/3 and weight (0, 1) gets +1/3; the
weights on zero features get nothing. A check asserts these values, and
central finite differences agree with the analytic gradient on six random
cases.

**One distillation step.** Chapter 124's AdamW, from zero moments at step 1,
moves each weight by −0.05 g/(|g| + 10^-8): almost exactly the rate, 0.05,
against the gradient's sign. So weights (0, 0) and (0, 1) become +0.05 and
−0.05, rows 1 and 2 the opposite. The new logits are
0.05 × 1 + (−0.05)(−1/2) = 0.075 for option 0 and −0.075 for the others, and
the loss falls. A check runs this step through chapter 118's `tr_apply` and
compares within 10^-8.

**The split.** Chapter 120 joins rows that share a task. Families 2k and 2k+1
share task k, and family f's time is f, so the pair's group ends at time
2k + 1. With the plan "training up to time 39, development up to 51", families
0 to 39 train (320 requests), 40 to 51 are development (96) and 52 to 63 are
test (96). No family, and no pair, straddles a split.

**The cut.** An answer from the router costs nothing if right and 10 if wrong;
sending a request to the teacher costs 1 and then the teacher's own error
rate. Answering is cheaper when 10 (1 − p) ≤ 1 + 10 (1 − a), where p is the
router's calibrated confidence and a the teacher's accuracy, that is when
p ≥ a − 1/10. On the development split the teacher was right on 69 of 96, so
the cut is 69/96 − 0.1 = 0.61875.

## Data and a leak-free split

<!-- source: psionic/lib/distill.coax requests -->
```jai
// Request r of family f = r / 8: the family's 8 token IDs (SplitMix64 seed
// 2233, below 15, so chapter 120's split can read them), then 8 of its own
// (seed 2234, below 64).
ds_request :: (r:int,ids:[]int) {
    f:=r/DS_PER;
    for i:0..<DS_TOKENS {
        ids[i]=ifx i<DS_TEMPLATE then cast(int)(cr_bits(2233,cast(u64)(f*DS_TEMPLATE+i))%15)
            else cast(int)(cr_bits(2234,cast(u64)(r*(DS_TOKENS-DS_TEMPLATE)+i-DS_TEMPLATE))%64);
    }
}
// One split row per family: families 2k and 2k+1 share task k, family f has
// its own session and time f, and its tokens are its 8 shared IDs.
ds_families :: (rows:[]SpRow,tokens:[]u8) {
    ids:[16]int;
    for f:0..<DS_FAMILIES {
        ds_request(f*DS_PER,ids[..]);
        rows[f]=SpRow.{id=f,licence=1,task=f/2,session=f,time=f,outcome=f,first=f*DS_TEMPLATE,count=DS_TEMPLATE};
        for i:0..<DS_TEMPLATE {tokens[f*DS_TEMPLATE+i]=cast(u8)ids[i];}
    }
}
// An audit that ran and counted nothing across splits.
ds_clean :: (a:SpAudit)->bool {
    return a.status==SpStatus.ok && a.exact==0 && a.task==0 && a.session==0 && a.derived==0 && a.near==0 && a.post==0;
}
// Chapter 120 assigns each group of families one split; the audit must find
// no shared content, task, session, link or near duplicate across splits.
// Every request takes its family's split.
ds_split :: (split:[]PkSplit)->DsStatus #export_c {
    if split.count!=DS_REQUESTS {return DsStatus.shape;}
    rows:[64]SpRow;tokens:[512]u8;fam:[64]PkSplit;cells:[1]SpCell;
    ds_families(rows[..],tokens[..]);
    if sp_assign(rows[..],tokens[..],DS_PLAN,fam[..])!=SpStatus.ok {return DsStatus.split;}
    a:=sp_audit(rows[..],tokens[..],fam[..],cells[0..<0],0);
    if !ds_clean(a) {return DsStatus.leak;}
    for r:0..<DS_REQUESTS {split[r]=fam[r/DS_PER];}
    return DsStatus.ok;
}
```

The data step admits the teacher, assigns the split from provenance and
computes features. Every later step re-derives the split and compares: a
request moved out of its family's split is a leak and is refused before any
training.

<!-- source: psionic/lib/distill.coax data -->
```jai
// Admits the teacher and the split from provenance, then computes the
// features of requests first..<end (all 512 in the example, a few in checks).
ds_data :: (w:[]f64,k:FeWork,d:DsData,first:int,end:int)->DsStatus #export_c {
    if d.feats.count!=DS_REQUESTS*DS_FEATURES || d.split.count!=DS_REQUESTS || first<0 || end<=first || end>DS_REQUESTS {return DsStatus.shape;}
    s:=ds_teacher(d.teacher,d.labels,DS_TEACHER_ID);if s!=DsStatus.ok {return s;}
    s=ds_split(d.split);if s!=DsStatus.ok {return s;}
    ids:[16]int;
    for r:first..<end {
        ds_request(r,ids[..]);
        if ds_features(w,k,ids[..],d.feats[r*DS_FEATURES..<(r+1)*DS_FEATURES])!=FeStatus.ok {return DsStatus.encoder;}
    }
    return DsStatus.ok;
}
// What training, fitting and the card read: shapes, the teacher, and a split
// equal to the one provenance gives, so a moved request is a leak.
ds_admit :: (d:DsData)->DsStatus {
    if d.feats.count!=DS_REQUESTS*DS_FEATURES || d.split.count!=DS_REQUESTS {return DsStatus.shape;}
    s:=ds_teacher(d.teacher,d.labels,fe_model_id(d.teacher));if s!=DsStatus.ok {return s;}
    want:[512]PkSplit;s=ds_split(want[..]);if s!=DsStatus.ok {return s;}
    for r:0..<DS_REQUESTS {if d.split[r]!=want[r] {return DsStatus.leak;}}
    return DsStatus.ok;
}
```

## Training under chapter 118's trainer

The trainer owns the batch order, the steps and early stopping; the caller
only supplies each batch's loss and gradient and the development loss when
asked. AdamW at rate 0.05, batches of 32 reshuffled each epoch, at most 40
epochs, the development loss once per epoch, and a stop after three
evaluations without improvement.

<!-- source: psionic/lib/distill.coax train -->
```jai
// Chapter 118's trainer in rows mode: AdamW (rate 0.05, betas 0.9 and 0.999,
// eps 10^-8, no decay), batches of 32 reshuffled each epoch (seed 223), 40
// epochs, the development loss once per epoch from step 0, and a stop after
// 3 evaluations without improvement.
// ds_train sets the examples to the training split's size and evaluates
// once an epoch.
ds_config :: ()->TrConfig #export_c {
    return TrConfig.{optimizer=TrOptimizer.adamw,rate=0.05,beta1=0.9,beta2=0.999,eps=0.00000001,bound=100,
        batch=32,epochs=40,shuffle=true,seed=223,patience=3};
}
ds_rows :: (d:DsData,want:PkSplit,out:[]int)->int {n:=0;for r:0..<DS_REQUESTS {if d.split[r]==want {out[n]=r;n+=1;}}return n;}
// Trains head w (48 values, zero to start) on the training split.
ds_train :: (d:DsData,t:f64,base:TrConfig,w:[]f64)->DsRun #export_c {
    if !(t>0 && t<=16) {return DsRun.{status=DsStatus.temperature};}
    if w.count!=DS_PARAMS {return DsRun.{status=DsStatus.shape};}
    s:=ds_admit(d);if s!=DsStatus.ok {return DsRun.{status=s};}
    train:[512]int;dev:[512]int;n:=ds_rows(d,PkSplit.train,train[..]);nd:=ds_rows(d,PkSplit.validation,dev[..]);
    c:=base;c.examples=n;c.every=tr_ceil(n,c.batch);order:[512]int;none:[]PkDoc;nothing:[]u8;
    st:=tr_begin(c,w.count,none,nothing,order[..]);
    out:=DsRun.{trainer=st.status};if st.status!=TrStatus.ok {out.status=DsStatus.trainer;return out;}
    run:=st.run;m:[48]f64;v:[48]f64;total:[48]f64;stage:[144]f64;g:[48]f64;idx:[32]int;
    // The trainer asks for an evaluation (the development loss) or a step
    // (each micro-batch's mean loss and gradient), until it is done or stops.
    while true {
        b:=tr_next(c,run,none,nothing,nothing);
        if b.status==TrStatus.done || b.status==TrStatus.stopped {break;}
        now:=b.status;
        if now==TrStatus.evaluate {
            e:=tr_eval(c,run,ds_batch(w,d,dev[0..<nd],t,g[..]));now=e.status;
            if now==TrStatus.ok {run=e.run;out.evals+=1;}
        } else if now==TrStatus.ok {
            sum:TrSum;
            for j:0..<b.micro {
                r:=tr_rows(c,b,j);for i:0..<r.count {idx[i]=train[order[r.first+i]];}
                sum=tr_add(b,sum,ds_batch(w,d,idx[0..<r.count],t,g[..]),g[..],total[..]);
            }
            a:=tr_apply(c,run,b,sum,w,m[..],v[..],total[..],stage[..],order[..]);now=a.status;
            if now==TrStatus.ok {run=a.run;out.steps+=1;}
        }
        if now!=TrStatus.ok {out.status=DsStatus.trainer;out.trainer=now;return out;}
    }
    out.stopped=run.stopped;out.best=run.best;
    return out;
}
```

## A fitted temperature

The student's raw confidence is not a probability of being right. As in
[chapter 215](ch215-fit-our-own-calibration.md), one temperature is fitted on
the development split against the outcome labels: the first of 1/4, 3/8, ...,
4 with the least mean log loss. It is fitted on outcomes, not on the teacher.

<!-- source: psionic/lib/distill.coax fit -->
```jai
// The calibrated student: softmax of the logits over 1/2 x tau, into three
// slots (anything else is left as it was).
ds_probs :: (w:[]f64,x:[]f64,tau:f64,out:[]f64) {
    s:=ds_logits(w,x);a:[3]f64;for c:0..<DS_CLASSES {a[c]=s[c]/(DS_HEAD_TEMP*tau);}
    if out.count!=DS_CLASSES {return;}
    l:=ds_log_soft(a);for c:0..<DS_CLASSES {out[c]=exp(l[c]);}
}
// Mean log loss against the outcome labels on the development split.
ds_nll :: (d:DsData,w:[]f64,tau:f64)->f64 {
    dev:[512]int;n:=ds_rows(d,PkSplit.validation,dev[..]);loss:=0.0;
    for r:dev[0..<n] {
        s:=ds_logits(w,d.feats[r*DS_FEATURES..<(r+1)*DS_FEATURES]);a:[3]f64;
        for c:0..<DS_CLASSES {a[c]=s[c]/(DS_HEAD_TEMP*tau);}
        l:=ds_log_soft(a);loss-=l[d.labels[r]];
    }
    return loss/f64(n);
}
// The temperature: the first of 1/4, 3/8, ..., 4 with the least development
// log loss. 0 when the data are refused.
ds_fit :: (d:DsData,w:[]f64)->f64 #export_c {
    if w.count!=DS_PARAMS || ds_admit(d)!=DsStatus.ok {return 0;}
    best:=0.25;least:=ds_nll(d,w,best);
    for k:3..<33 {tau:=f64(k)/8;l:=ds_nll(d,w,tau);if l<least {least=l;best=tau;}}
    return best;
}
```

## The card, the rule and the costs

The card reads the test split once. It counts agreement with the teacher and
rightness against the outcome labels separately, and measures calibration
error (15 bins, top label) and the Brier score with chapter 16's
`pq_measure`, for the calibrated student and for the teacher.

<!-- source: psionic/lib/distill.coax card -->
```jai
// The router answers when its calibrated confidence is at least the
// teacher's development accuracy less one call's cost over one error's.
ds_cut :: (right:int,n:int)->f64 {return f64(right)/f64(n)-DS_CALL/DS_WRONG;}
ds_answers :: (confidence:f64,cut:f64)->bool {return confidence>=cut;}
// The test split's card. probs and items are scratch for chapter 16's
// measure: the student's calibrated probabilities, then the teacher's.
ds_card :: (d:DsData,w:[]f64,tau:f64,probs:[]f64,items:[]PqItem)->DsCard #export_c {
    if !(tau>0 && tau<=16) {return DsCard.{status=DsStatus.temperature};}
    if w.count!=DS_PARAMS || probs.count!=DS_REQUESTS*DS_CLASSES || items.count!=DS_REQUESTS {return DsCard.{status=DsStatus.shape};}
    s:=ds_admit(d);if s!=DsStatus.ok {return DsCard.{status=s};}
    c:=DsCard.{tau=tau};dev:[512]int;test:[512]int;
    c.dev=ds_rows(d,PkSplit.validation,dev[..]);c.n=ds_rows(d,PkSplit.test,test[..]);
    for r:dev[0..<c.dev] {if fe_argmax(d.teacher,r*3,3)-r*3==d.labels[r] {c.teacher_dev+=1;}}
    c.cut=ds_cut(c.teacher_dev,c.dev);
    for r,i:test[0..<c.n] {
        q:=probs[DS_CLASSES*i..<DS_CLASSES*(i+1)];ds_probs(w,d.feats[r*DS_FEATURES..<(r+1)*DS_FEATURES],tau,q);
        y:=d.labels[r];st:=fe_argmax(q,0,3);te:=fe_argmax(d.teacher,r*3,3)-r*3;
        items[i]=PqItem.{kind=QrKind.choice,count=3,start=3*i,gold=y,abstain=-1};
        if st==te {c.agree+=1;}
        if st==y {c.student_right+=1;}
        if te==y {c.teacher_right+=1;}
        if ds_answers(q[st],c.cut) {
            c.answered+=1;
            if st==te {c.answered_agree+=1;}
            if st==y {c.answered_right+=1;}
            if te==y {c.teacher_answered_right+=1;}
        }
    }
    // Chapter 16's measure: every item answered, none abstaining (a refused
    // measure answers none).
    m:=pq_measure(items[0..<c.n],probs);
    if m.answered!=c.n {return DsCard.{status=DsStatus.shape};}
    c.ece=m.ece;c.brier=m.brier;
    for r,i:test[0..<c.n] {for k:0..<3 {probs[3*i+k]=d.teacher[3*r+k];}}
    m=pq_measure(items[0..<c.n],probs);c.teacher_ece=m.ece;c.teacher_brier=m.brier;
    return c;
}
```

The rule was frozen before the test split was read:

<!-- source: psionic/lib/distill.coax frozen -->
```jai
// Frozen before the test split was read.
DS_T :: 2.0;               // distillation temperature
DS_WRONG :: 10.0;          // written cost of a wrong answer
DS_CALL :: 1.0;            // written cost of one teacher call
DS_ANSWERED_MIN :: 0.1;    // the router answers at least a tenth of the test requests
DS_AGREE_MIN :: 0.8;       // and agrees with the teacher on at least 80% of them
DS_ECE_MAX :: 0.1;         // calibration error against outcomes, all test requests
DS_PRECISION_DROP :: 0.05; // right on its answers within 5 points of the teacher there
```

<!-- source: psionic/lib/distill.coax gate -->
```jai
// The frozen rule, in order: a card, enough answers, agreement with the
// teacher on them, calibration on every test request, and right answers
// within 5 points of what the teacher would have been on the same requests.
ds_gate :: (c:DsCard)->DsVerdict #export_c {
    if c.status!=DsStatus.ok || c.n<1 {return DsVerdict.{reason=DsReason.card};}
    if f64(c.answered)<DS_ANSWERED_MIN*f64(c.n) {return DsVerdict.{reason=DsReason.answered};}
    if f64(c.answered_agree)<DS_AGREE_MIN*f64(c.answered) {return DsVerdict.{reason=DsReason.agreement};}
    if c.ece>DS_ECE_MAX {return DsVerdict.{reason=DsReason.calibration};}
    if f64(c.answered_right)<f64(c.teacher_answered_right)-DS_PRECISION_DROP*f64(c.answered) {return DsVerdict.{reason=DsReason.precision};}
    return DsVerdict.{promoted=true};
}
// Escalated requests cost a call and are right when the teacher is.
ds_cost :: (c:DsCard)->DsCost {
    k:=DsCost.{calls=c.n-c.answered,teacher_calls=c.n,teacher_wrong=c.n-c.teacher_right};
    k.wrong=(c.answered-c.answered_right)+(k.calls-(c.teacher_right-c.teacher_answered_right));
    k.cost=DS_CALL*f64(k.calls)+DS_WRONG*f64(k.wrong);
    k.teacher_cost=DS_CALL*f64(k.teacher_calls)+DS_WRONG*f64(k.teacher_wrong);
    return k;
}
```

## Run it

The [example](../examples/distill-router.coax) computes every feature, trains,
fits, reads the test split once and applies the rule:

<!-- source: psionic/examples/distill-router.coax example -->
```jai
main :: () {
    w:[8848]f64;#assert fe_weights(w[..])==FeStatus.ok;
    print("encoder %, identity %: frozen; head 1 (3 classes, temperature 1/2) is trained\n",FE_PROFILE,fe_model_id(w[..]));
    f:[3072]f64;k:=FeWork.{f=f[..]};feats:[8192]f64;t:[1536]f64;y:[512]int;s:[512]PkSplit;
    #assert ds_recorded(t[..],y[..])==DsStatus.ok;
    d:=DsData.{feats=feats[..],teacher=t[..],labels=y[..],split=s[..]};
    print("teacher stand-in %, 512 requests: %\n",DS_TEACHER_ID,ds_status_name(ds_data(w[..],k,d,0,512)));
    n:[3]int;for r:0..<512 {n[cast(int)s[r]]+=1;}
    worst:=0.0;for i:0..<8192 {worst=qz_larger(worst,abs(feats[i]-DS_ORACLE_FEATS[i]));}
    print("split by family: % training, % development, % test requests; largest feature difference from the oracle %\n",n[0],n[1],n[2],worst);
    head:[48]f64;r:=ds_train(d,DS_T,ds_config(),head[..]);
    print("distilled at T = %: %, % steps, % evaluations, stopped early: %, best development loss %\n",
        DS_T,ds_status_name(r.status),r.steps,r.evals,r.stopped,r.best);
    tau:=ds_fit(d,head[..]);
    probs:[1536]f64;items:[512]PqItem;c:=ds_card(d,head[..],tau,probs[..],items[..]);
    print("temperature fitted on development: %; the teacher was right on % of % there, so the cut is %\n",tau,c.teacher_dev,c.dev,c.cut);
    print("test: % requests; the student agrees with the teacher on %; right % times, the teacher % times\n",c.n,c.agree,c.student_right,c.teacher_right);
    print("calibration error: student %, teacher %; Brier: student %, teacher %\n",c.ece,c.teacher_ece,c.brier,c.teacher_brier);
    print("the router answers %: % agree with the teacher, % right; the teacher would be right on % of them\n",
        c.answered,c.answered_agree,c.answered_right,c.teacher_answered_right);
    v:=ds_gate(c);
    print("frozen rule: %\n",ifx v.promoted then "promoted" else ds_reason_name(v.reason));
    o:=ds_cost(c);
    print("router: % calls, % wrong, cost %; teacher alone: % calls, % wrong, cost %\n",o.calls,o.wrong,o.cost,o.teacher_calls,o.teacher_wrong,o.teacher_cost);
    #assert ds_install(w[..],head[..])==DsStatus.ok;
    print("student model identity %; multiply-adds per 512-token decision %\n",fe_model_id(w[..]),fe_macs(512));
}
```

```text
encoder fe-tiny-v1, identity 872817745440994249: frozen; head 1 (3 classes, temperature 1/2) is trained
teacher stand-in 8153232079184106977, 512 requests: ok
split by family: 320 training, 96 development, 96 test requests; largest feature difference from the oracle 3.0531133177191805e-15
distilled at T = 2.0: ok, 70 steps, 8 evaluations, stopped early: true, best development loss 0.8967355806659106
temperature fitted on development: 2.5; the teacher was right on 69 of 96 there, so the cut is 0.61875
test: 96 requests; the student agrees with the teacher on 51; right 49 times, the teacher 74 times
calibration error: student 0.13352489849405125, teacher 0.09142585830869063; Brier: student 0.5979107450275571, teacher 0.29967905790541377
the router answers 1: 1 agree with the teacher, 1 right; the teacher would be right on 1 of them
frozen rule: fewer than a tenth of the test requests answered
router: 95 calls, 22 wrong, cost 315.0; teacher alone: 96 calls, 22 wrong, cost 316.0
student model identity 14272328053237282941; multiply-adds per 512-token decision 16281744
```

What the run says, line by line:

- **Training** stopped early after 7 epochs (70 steps): the development loss
  was lowest after the fourth epoch.
- **Temperature 2.5.** The trained head is overconfident; dividing its
  logits by a further 2.5 gives the least development log loss.
- **Agreement 51 of 96.** Chance is a third. The frozen random encoder's
  16 pooled values carry only part of what the teacher reads, and a linear
  head cannot recover the rest. On the outcomes the student is right 49
  times, the teacher 74.
- **Calibration.** Even after the fitted temperature, the student's
  calibration error on test is 0.134 against the teacher's 0.091; its Brier
  score, 0.598, is twice the teacher's.
- **One answer.** Only one test request reaches a calibrated confidence of
  0.61875. The rule's first gate, at least a tenth of the requests answered,
  fails; the calibration gate would fail next.

The costs, by hand from the card. The router sends 95 requests to the teacher
and answers 1, correctly. The teacher is right on 74 of 96 in all and on the
1 the router answered, so it is wrong on 95 − (74 − 1) = 22 of the escalated
requests: 22 errors, cost 95 × 1 + 22 × 10 = 315. The teacher alone makes 96
calls and 22 errors, cost 96 + 220 = 316. The router saves one call in 96 and
changes no outcome. At chapter 16's recorded 69 to 81 minutes per Elephant
decision on the CPU, that is about 1% of the teacher's time, at the price of
one student pass on every request (about 12 ms on a quiet host, chapter 222's
measurement).

## Latency after training

Training changed 48 weights and no shape: `fe_macs(512)` is still 16,281,744.
[`check_distill.sh`](../tests/check_distill.sh) runs the checks and both
transcripts in debug and release, then compiles the library with
[`distill_bench.c`](../tests/distill_bench.c), which trains the student,
installs its head, quantizes it and runs chapter 222's frozen workload: parity
and the card on eight 512-token requests, then interleaved warm pairs. The C
host only reads the clock and writes the receipt; the verdict is chapter
222's `fe_gate`.

On the Intel Core i7-14700K (Linux 6.18, GCC 15.2, `-O2`), one thread, 200
warm pairs, with the trained student (identity `c6117ebf130cf87d`):

| Check | Result |
| --- | --- |
| Parity, q8 against the reference | 24 of 24 head decisions equal, largest logit difference 0.0191 (bound 0.125) |
| Chapter 50's card | promoted: agreement 1.0, mean KL 3.5 × 10^-5 |
| q8 warm P50 / P99 | 55.13 ms / 206.36 ms |
| Reference warm P50 / P99 | 48.49 ms / 176.41 ms |
| Gate (P99 ≤ 20 ms) | **missed** |

This run does not settle the latency question. The host's load average was
62 on 28 hardware threads while other jobs ran; chapter 222's run of the same
shape, at load 9, measured 12.32 ms P50 and 12.75 ms P99. The student does
exactly the same work, 16,281,744 multiply-adds, so a slower run means a
busier machine, not a slower model. The gate is not claimed as met here: a
run on a quiet host is still owed. The
[receipt](../fixtures/distill/receipt-cpu-2026-11-01.json) keeps every raw
time, the build, the load and the training record.

## What is refused

The [refusal example](../examples/distill-router-refuse.coax) breaks one thing
at a time, on the recorded data:

<!-- source: psionic/examples/distill-router-refuse.coax refuse -->
```jai
judge :: (what:string,c:DsCard) {v:=ds_gate(c);print("%: %\n",what,ifx v.promoted then "promoted" else ds_reason_name(v.reason));}
main :: () {
    feats:[8192]f64;t:[1536]f64;y:[512]int;s:[512]PkSplit;
    for i:0..<8192 {feats[i]=DS_ORACLE_FEATS[i];}
    #assert ds_recorded(t[..],y[..])==DsStatus.ok;
    #assert ds_split(s[..])==DsStatus.ok;
    d:=DsData.{feats=feats[..],teacher=t[..],labels=y[..],split=s[..]};w:[48]f64;
    print("the recorded data: %\n",ds_status_name(ds_admit(d)));
    t[1]=t[1]+0.001;
    print("teacher row 0 sums to 1.001: %\n",ds_status_name(ds_train(d,DS_T,ds_config(),w[..]).status));
    t[1]=DS_TEACHER_P[1];t[0]=DS_TEACHER_P[0]+0.0000000000000001;t[2]=DS_TEACHER_P[2]-0.0000000000000001;
    print("teacher row 0 moved by 10^-16: %\n",ds_status_name(ds_teacher(t[..],y[..],DS_TEACHER_ID)));
    t[0]=DS_TEACHER_P[0];t[2]=DS_TEACHER_P[2];y[9]=3;
    print("outcome label 3: %\n",ds_status_name(ds_train(d,DS_T,ds_config(),w[..]).status));
    y[9]=DS_LABELS[9];s[420]=PkSplit.train;
    print("request 420 (a test family's) moved to training: %\n",ds_status_name(ds_train(d,DS_T,ds_config(),w[..]).status));
    s[420]=PkSplit.test;
    print("distillation temperature 0: %\n",ds_status_name(ds_train(d,0,ds_config(),w[..]).status));
    print("a head of 47 weights: %\n",ds_status_name(ds_train(d,DS_T,ds_config(),w[0..<47]).status));
    // The frozen rule on teaching cards, not measurements.
    c:=DsCard.{n=96,answered=40,answered_agree=34,answered_right=30,teacher_answered_right=31,ece=0.05};
    judge("a card that meets every gate",c);
    b:=c;b.answered=9;b.answered_agree=9;judge("9 of 96 answered",b);
    b=c;b.answered_agree=31;judge("31 of 40 answers agree",b);
    b=c;b.ece=0.12;judge("calibration error 0.12",b);
    b=c;b.answered_right=28;judge("28 right where the teacher is right 31 times",b);
    b=c;b.status=DsStatus.leak;judge("a card from leaked data",b);
}
```

```text
the recorded data: ok
teacher row 0 sums to 1.001: a teacher row that is not a distribution
teacher row 0 moved by 10^-16: not the recorded teacher
outcome label 3: a label outside 0 to 2
request 420 (a test family's) moved to training: a request outside its family's split
distillation temperature 0: a temperature outside (0, 16]
a head of 47 weights: a slice of the wrong length
a card that meets every gate: promoted
9 of 96 answered: fewer than a tenth of the test requests answered
31 of 40 answers agree: agreement with the teacher below 80% on the answers
calibration error 0.12: calibration error above 0.1
28 right where the teacher is right 31 times: more than 5 points less often right than the teacher on the answers
a card from leaked data: no card
```

The gate lines use teaching cards, not measurements.

## The independent checks

- **An oracle.** [`distill_oracle.py`](../tests/distill_oracle.py) writes the
  whole pipeline again in Python from this chapter's description: features
  through [chapter 222's Python oracle](../tests/fast_encoder_oracle.py), the
  split, the loss, chapter 118's batch order and early stopping, AdamW, the
  temperature grid, the cut, the counts, calibration error and Brier score.
  Trained from its features, the library's head matches the oracle's within
  10^-9, with the same steps, evaluations and stop; the temperature and every
  count match exactly, and the four quality numbers within 10^-12. The
  library's own features match the oracle's within 10^-12 (the run above
  prints the largest difference over all 8,192).
- **Finite differences** of the loss match its analytic gradient on six
  random cases, at temperatures 1/2 to 3.
- **Hand values:** the KL gradient and loss above, the AdamW step, the
  log-softmax at ±1,000, the development log loss of zero weights (ln 3), the
  rule at each boundary, and the costs on a hand card.
- **Bit equality:** the trained head installed in chapter 222's weights gives
  the router's logits bit for bit through `fe_forward`.
- **Refusals before work:** each refusal above is a check, and `coax mutate`
  leaves no mutant of the library alive.

## What is not built

- **Elephant as the teacher.** Only 12 recorded Elephant decisions exist; a
  real target set needs hundreds, recorded once and pinned by identity. The
  stand-in's results say nothing about Elephant.
- **Chapter 247's `distill-v1` targets** with the shared exposure ledger.
  Here the split comes from chapter 120 over the families directly.
- **Training the encoder's body.** Only the head is trained. Gradients
  through attention, GeGLU and the norms would let the student learn
  features the teacher uses; that is the most likely way past the first gate.
- **A quiet-host latency run** of the trained student against chapter 222's
  20 ms gate.
- **Shadow promotion** ([chapter 224](ch224-shadow-a-new-model-then-promote-it.md))
  and verdicts that can say unverifiable
  ([chapter 241](ch241-gates-that-can-say-unverifiable.md)).
- **Cheap-answer precision against real outcome rows.** The outcome labels
  are drawn from the stand-in; chapter 247's rows with labels from checks and
  diffs are not used.
