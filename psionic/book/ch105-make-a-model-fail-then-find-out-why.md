# 105. Make a model fail, then find out why

A training bug often produces no error. The loss still goes down, or it goes
down a little less. This chapter takes the hidden-layer classifier from
[chapter 103](ch103-learning-with-arrays.md), trains it through
[chapter 104](ch104-gradients-as-a-program.md)'s tape, and breaks the training
on purpose in six ways:
1. **Stale gradients:** the gradient buffer is never cleared between steps.
2. **Shuffled targets:** the targets are shuffled once on their own, so every
   row is paired with another row's target.
3. **Padding in the loss:** each batch is packed into a fixed number of slots,
   and the loss averages over the empty slots too.
4. **Rate too large:** the learning rate is 16 times the working one.
5. **Identical units:** every hidden unit starts with the same weights.
6. **Dropout at evaluation:** input dropout, a training-only random
   operation, stays on when the model is evaluated.

Each run leaves a record of what it did. A diagnosis reads two records, the
run and the same run at a tenth of the rate, and names every fault they point
at. On the chapter's data each faulty run is named as exactly its fault, and
the healthy run as nothing.

```shell
coax/bin/coax run psionic/examples/diagnose-faults.coax
psionic/check.sh --only diagnose
```

```text
line at rate 0.45: loss 10.0 -> 1.4134776518227987e-07 after 40 steps; found nothing
line at rate 0.55: loss 10.0 -> 17281827.696082696 after 40 steps; found rate too large
line at rate 4: diverged true after 4 steps, w = -101248.0; found rate too large
line, buffer never cleared, 2 steps at rate 0.25: buffer 8.246211251235321 before step 2, (w, b) = (4.0, 1.375), loss 8.1953125; found stale gradients
line, 2 rows in 4 slots, padding in the loss: batch loss 5.0, gap 0.45454545454545453, (w, b) = (1.0, 0.25); found padding in the loss
deadline data, seed 70: 1440 training rows (12 a step in 16 slots), 480 held out, constant predictor 0.4088
healthy: held-out loss 0.1295 (0.1248 at a tenth of the rate), batch loss 0.2428 -> 0.1703
  buffer 0.0, loss gap 0.0, distinct units 8, repeat 0.0: found nothing
stale gradients: held-out loss 40.0537 (3.8325 at a tenth of the rate), batch loss 0.718 -> 43.8487
  buffer 55.3089, loss gap 0.0, distinct units 8, repeat 0.0: found stale gradients
shuffled targets: held-out loss 0.4155 (0.4091 at a tenth of the rate), batch loss 0.4463 -> 0.433
  buffer 0.0, loss gap 0.0, distinct units 8, repeat 0.0: found shuffled targets
padding in the loss: held-out loss 0.1187 (0.1747 at a tenth of the rate), batch loss 0.5563 -> 0.16
  buffer 0.0, loss gap 0.0489, distinct units 8, repeat 0.0: found padding in the loss
identical units: held-out loss 0.1467 (0.1247 at a tenth of the rate), batch loss 0.2592 -> 0.1842
  buffer 0.0, loss gap 0.0, distinct units 1, repeat 0.0: found identical units
dropout at evaluation: held-out loss 0.178 (0.1673 at a tenth of the rate), batch loss 0.2428 -> 0.1703
  buffer 0.0, loss gap 0.0, distinct units 8, repeat 0.6225: found dropout at evaluation
rate 8: held-out loss 1.3174 (0.1304 at a tenth of the rate), batch loss 0.7832 -> 0.4624
  buffer 0.0, loss gap 0.0, distinct units 8, repeat 0.0: found rate too large
the healthy run again from the same seeds, as after any repair: the same record bit for bit: true
overfit one batch of 12, 300 steps, training loss:
  healthy: 0.0193 (reaches 0.05)
  stale gradients: 0.7943 (does not reach 0.05)
  shuffled targets: 0.7685 (does not reach 0.05)
  padding in the loss: 0.0288 (reaches 0.05)
  identical units: 0.0152 (reaches 0.05)
  dropout at evaluation: 0.014 (reaches 0.05)
  rate 8: 0.0003 (reaches 0.05)
data seeds 71 to 74: 28 of 28 runs found exactly their planted fault, or nothing when healthy
a batch of 12 in 8 slots: shape
512 epochs of 120 batches, 61,440 steps: shape
dropout 1: shape
training rows that are all on time: shape
47 parameter slots for 49 parameters: capacity
16 slots of a model with 8 features and 16 units: capacity
```

The [example](../examples/diagnose-faults.coax) runs in about 4 seconds in
release mode and about 45 seconds in debug mode.

## The trainer and its record

The trainer is chapter 104's `tp_train` with three additions:
- **Packed batches.** Each step copies `batch` rows into a block of `slots`
  slots. The slots after the rows are padding: features 0 and target 0. The
  loss is the mean over the slots the mask names, which is the rows only
  unless the padding fault is on.
- **Input dropout.** During training each copied feature is zeroed with
  chance `dropout` and otherwise scaled by 1 / (1 − `dropout`).
- **Five fault switches** in `DgFaults`. The sixth fault is a value of
  `DgConfig.rate`, not a switch.

With every switch off, no padding in the batch and no dropout, the trainer is
chapter 104's trainer step for step. A check trains a two-layer classifier
both ways and compares every parameter for equality.

<!-- source: psionic/lib/diagnose.coax train -->
```jai
// Chapter 104's tape, driven by a trainer whose settings can be wrong. Each
// step packs `batch` rows into `slots` slots, takes the mean loss over the
// slots the mask names, runs the tape forward and backward, and moves every
// parameter by `rate` times its gradient. The record says what happened.
dg_train :: (m:TbModel,c:DgConfig,f:DgFaults,x:[]f64,y:[]f64,train:[]int,held:[]int,params:[]f64)->DgRecord {
    admitted:=dg_admit(m,c,x,y,train,held,params);
    if admitted!=DgStatus.ok {return DgRecord.{status=admitted};}
    n:=tb_param_count(m);d:=m.features;nt:=train.count;rec:DgRecord;
    // The largest program a step builds has every slot in the loss.
    nodes:[4096]TpNode;values:[4096]f64;z:[16]f64;slot:[16]int;zero:[128]f64;
    for s:1..<16 {slot[s]=s;}
    if tp_tabular(m,zero[0..<c.slots*d],zero[0..<c.slots],slot[0..<c.slots],nodes[..],false).status!=TpStatus.ok {return DgRecord.{status=DgStatus.capacity};}
    p:[161]f64;g:[161]f64;tb_initialize(m,c.init_seed,p[..]);
    if f.identical_init {
        // Every hidden unit copies unit 0's input weights and output weight.
        base:=m.hidden*d+m.hidden;
        for k:1..<m.hidden {for j:0..<d {p[k*d+j]=p[j];}p[base+k]=p[base];}
    }
    // The target each training row is paired with.
    target:[4096]f64;for i:0..<y.count {target[i]=y[i];}
    if f.shuffled_targets {
        order:[4096]int;for i:0..<nt {order[i]=train[i];}
        unused:=tb_shuffle(order[0..<nt],c.shuffle_seed+1,0);
        for i:0..<nt {target[train[i]]=y[order[i]];}
    }
    o:[4096]int;for i:0..<nt {o[i]=train[i];}
    counter:=(cast(u64)0);
    for epoch:0..<c.epochs {
        counter=tb_shuffle(o[0..<nt],c.shuffle_seed,counter);
        sum:=0.0;batches:=0;start:=0;
        for i:0..<nt {
            // A batch closes when it holds `batch` rows or the epoch runs out of rows.
            if i+1-start!=c.batch && i+1!=nt {continue;}
            real:=i+1-start;bx:[128]f64;by:[16]f64;
            for s:0..<real {
                r:=o[start+s];by[s]=target[r];
                for j:0..<d {bx[s*d+j]=x[r*d+j]*dg_keep(c.drop_seed,cast(u64)((rec.steps*c.slots+s)*d+j),c.dropout);}
            }
            masked:=real;if f.padding_in_loss {masked=c.slots;}
            // Admission built a block of `slots` rows, so this one fits, and it
            // bounds every value the tape sees, so neither pass can refuse.
            b:=tp_tabular(m,bx[0..<c.slots*d],by[0..<c.slots],slot[0..<masked],nodes[..],false);
            if !f.stale_gradients {for k:0..<n {g[k]=0;}}
            rec.stale=dg_norm(g[0..<n]);
            forward:=tp_forward(nodes[0..<b.count],p[0..<n],values[..],false);
            backward:=tp_backward(nodes[0..<b.count],values[..],g[0..<n],false);
            loss:=values[b.count-1];sum+=loss;batches+=1;
            // The loss over the batch's own rows, through chapter 103's forward pass.
            own:=dg_loss(m,dg_block(m,p[0..<n],bx[0..<real*d],z[..]),by[0..<real],slot[0..<real]);
            rec.mask=abs(loss-own)/(1+abs(own));
            next:[161]f64;
            for k:0..<n {next[k]=p[k]-c.rate*g[k];if !tb_in(next[k],-TB_DOMAIN,TB_DOMAIN) {rec.diverged=true;}}
            if rec.diverged {break;}
            for k:0..<n {p[k]=next[k];}
            rec.steps+=1;start=i+1;
        }
        if epoch==0 {rec.first_loss=sum/f64(batches);}
        rec.last_loss=sum/f64(batches);
        if rec.diverged {break;}
    }
    // Evaluation: dropout stays off unless the fault leaves it on.
    drop:=0.0;if f.dropout_at_eval {drop=c.dropout;}
    zt:[4096]f64;zh:[4096]f64;again:[4096]f64;
    dg_outputs(m,p[0..<n],x,train,c.drop_seed,DG_EVAL_COUNTER,drop,zt[..]);
    dg_outputs(m,p[0..<n],x,held,c.drop_seed,DG_EVAL_COUNTER+(cast(u64)1<<32),drop,zh[..]);
    dg_outputs(m,p[0..<n],x,held,c.drop_seed,DG_EVAL_COUNTER+(cast(u64)2<<32),drop,again[..]);
    rec.train_loss=dg_loss(m,zt[..],y,train);rec.held_loss=dg_loss(m,zh[..],y,held);
    change:=0.0;for i:0..<held.count {change+=abs(zh[i]-again[i]);}
    rec.repeat=change/f64(held.count);
    // The constant predictor: the training rate (or mean), as an output.
    mean:=0.0;for r:train {mean+=y[r];}mean/=f64(nt);
    constant:=mean;if m.logistic {constant=log(mean/(1-mean));}
    for i:0..<held.count {again[i]=constant;}
    rec.base_loss=dg_loss(m,again[..],y,held);
    rec.distinct=dg_distinct(m,p[0..<n],x,held);
    for k:0..<n {params[k]=p[k];}
    return rec;
}
```

The record keeps a few numbers per run, each chosen so that one fault moves
it and the healthy trainer leaves it at a known value:

| Field | What it measures | Healthy value |
| --- | --- | --- |
| `stale` | Norm of the gradient buffer just before the last backward pass | 0 |
| `mask` | Gap between the tape's batch loss and the loss of the batch's rows alone, through chapter 103's forward pass, divided by 1 + that loss | Rounding, below 10⁻⁹ |
| `first_loss`, `last_loss` | Mean batch loss over the first and the last epoch | Falls |
| `held_loss`, `base_loss` | Held-out loss of the model, and of a constant that predicts the training on-time rate | Model well below the constant |
| `distinct` | Hidden units whose activations on the held-out rows differ from every earlier unit's | All of them |
| `repeat` | Mean change in an output when the held-out rows are evaluated twice | 0 |

`mask` compares two computations of the same number. The tape's loss is the
one the optimizer actually minimizes. Chapter 103's `tb_output` gives each
real row's output separately, and their mean loss is what the training should
minimize. When they differ by more than rounding, the loss includes something
that isn't a row.

## Work the faults by hand first

A line with two rows is small enough to follow exactly. The rows are x = −2
and x = 2, the targets are −2 and 6, and w = b = 2 fits both. The loss is the
mean of half the squared errors:

L(w, b) = 4 (w − 2)² / 2 + (b − 2)² / 2

because the mean of x² is 4 and the mean of x is 0. Its gradient is
(4 (w − 2), b − 2), so its curvature is 4 in w and 1 in b. Training starts at
w = b = 0, where L = 10, and takes the full batch every step.

**The rate.** One step at rate r multiplies the error in w by 1 − 4r and the
error in b by 1 − r. The error in w shrinks only while |1 − 4r| < 1, that is,
for r below 2 / L = 0.5, where L = 4 is the larger curvature:
- At 0.45 it is multiplied by −0.8 a step. After 40 steps the loss is about
  2 (2 · 0.8⁴⁰)² ≈ 1.41 × 10⁻⁷.
- At 0.55 it is multiplied by −1.2 a step. After 40 steps the loss is about
  2 (2 · 1.2⁴⁰)² ≈ 1.728 × 10⁷, and the last epoch's loss is above the
  first's.
- At 4 it is multiplied by −15 a step. After 4 steps w = 2 − 2 · 15⁴ =
  −101,248. The fifth step would move w to 2 + 2 · 15⁵ = 1,518,752, outside
  chapter 103's parameter domain of ±2²⁰, so the trainer stops and marks the
  run as diverged.

**A buffer never cleared.** At (0, 0) the residuals are 2 and −6, so the
gradient is ((2 · −2 + −6 · 2) / 2, (2 − 6) / 2) = (−8, −2). At rate 0.25 the
step lands on (2, 0.5). There the residuals are both −1.5 and the gradient is
(0, −1.5). Without clearing, the buffer already holds (−8, −2) before that
backward pass, a norm of √68 ≈ 8.2462. The step applies the sum, (−8, −3.5),
and lands on (4, 1.375) instead of (2, 0.875). The loss there is
4 · 2² / 2 + 0.625² / 2 = 8.1953125.

**Padding in the loss.** Put the two rows in 4 slots. At (0, 0) the rows'
losses are 2 and 18, and each padding slot's is 0. With padding in the mask,
the tape's batch loss is (2 + 18 + 0 + 0) / 4 = 5, but the rows' own mean is
10. The gap is |5 − 10| / (1 + 10) = 5/11 ≈ 0.4545. The gradient is divided by
4 instead of 2, so one step at rate 0.25 lands on (1, 0.25) instead of
(2, 0.5).

The first five lines of the transcript are these runs, with the same values.

## Dropout that can be replayed

Every dropout draw is chapter 103's SplitMix64 at a counter. A training draw
for step t, slot s and feature j uses counter (t · slots + s) · features + j.
Evaluation draws start at 2⁴⁰, and each of the three evaluation passes (the
training rows and the held-out rows twice) has its own range 2³² apart. So a
run is a function of its three seeds, and the transcript is the same in debug
and release builds.

<!-- source: psionic/lib/diagnose.coax keep -->
```jai
// Inverted dropout: a feature is zeroed with chance `drop`, and otherwise
// scaled by 1 / (1 - drop), so its expected value is unchanged.
dg_keep :: (seed:u64,counter:u64,drop:f64)->f64 {
    if tb_uniform(seed,counter)<drop {return 0;}
    return 1/(1-drop);
}
```

## The diagnosis

<!-- source: psionic/lib/diagnose.coax diagnose -->
```jai
// A run learned something when its held-out loss beats the constant predictor's.
dg_learned :: (r:DgRecord)->bool {return r.held_loss<=r.base_loss-DG_LEARNED;}
// Every cause the two records point at, one bit each: `r` is the run, and
// `slower` the same run at a tenth of the rate.
dg_diagnose :: (m:TbModel,r:DgRecord,slower:DgRecord)->int {
    found:=0;
    if r.stale>0 {found|=DG_STALE;}
    else if r.diverged || r.last_loss>r.first_loss || slower.held_loss<r.held_loss-DG_LEARNED {found|=DG_RATE;}
    else if !dg_learned(r) {found|=DG_TARGETS;}
    if r.mask>DG_MASK {found|=DG_PADDING;}
    if m.hidden>1 && r.distinct==1 {found|=DG_IDENTICAL;}
    if r.repeat>0 {found|=DG_EVAL_RANDOM;}
    return found;
}
// The run, the same run at a tenth of the rate, and what they point at.
dg_investigate :: (m:TbModel,c:DgConfig,f:DgFaults,x:[]f64,y:[]f64,train:[]int,held:[]int,params:[]f64)->DgCase {
    slow:=c;slow.rate=c.rate/10;scratch:[161]f64;
    out:=DgCase.{record=dg_train(m,c,f,x,y,train,held,params),slower=dg_train(m,slow,f,x,y,train,held,scratch[..])};
    if out.record.status!=DgStatus.ok {out.status=out.record.status;}
    else if out.slower.status!=DgStatus.ok {out.status=out.slower.status;}
    else {out.found=dg_diagnose(m,out.record,out.slower);}
    return out;
}
```

Each rule reads the symptom its fault produces:
- **Stale gradients:** the buffer isn't zero before a backward pass. This
  fault also makes the loss rise and the held-out loss poor, so it is checked
  first, and the rate and target rules aren't applied when it is found.
- **Rate too large:** the run diverged, its last epoch's loss is above its
  first's, or the same run at a tenth of the rate reaches a held-out loss
  more than 0.05 lower. The last condition is what catches rate 8 on the
  classifier. Its loss falls from 0.7832 to 0.4624 over the epochs, but its
  held-out loss is 1.3174 against 0.1304 at rate 0.8.
- **Shuffled targets:** the run learned nothing: its held-out loss doesn't
  beat the constant predictor by 0.05, and the slower run doesn't do better
  either. A run whose features carry no information about its targets can only
  learn the base rate. The rate rule comes first because a rate too large
  also leaves a run near the constant predictor, and only the slower run tells
  them apart.
- **Padding in the loss:** the gap between the tape's loss and the rows' own
  loss is more than 10⁻⁹. The healthy gap is rounding, 0.0 to the transcript's
  four places.
- **Identical units:** a model with more than one hidden unit has only one
  distinct unit. Units that start the same receive the same gradient, so they
  stay the same, bit for bit, for the whole run.
- **Dropout at evaluation:** evaluating the same rows twice gives different
  outputs.

`dg_investigate` trains both runs and applies the diagnosis. The rate rule
compares against the slower run every time, so every investigation costs two
runs.

## What the classifier shows

The classifier is chapter 103's: 4 scaled features, 8 tanh units, a logistic
output, trained on seed 70's 1,440 training rows for 20 epochs at rate 0.5,
12 rows a step in 16 slots, with input dropout 0.1. The 480 held-out rows are
chapter 103's validation sessions. The constant predictor scores 0.4088 on
them, and the healthy model 0.1295.

Two faults barely change the held-out loss:
- **Padding in the loss** scores 0.1187, better than healthy on this seed.
  The padding slots all share one input, the feature means, so the extra
  loss there moves the predictions on real rows little. Only the loss gap,
  0.0489 on the last step, shows the fault.
- **Identical units** scores 0.1467. Eight identical units compute what one
  unit computes, and on this data that is still a useful model. Only the
  count of distinct units, 1, shows the fault.

**Dropout at evaluation** scores 0.178. It trains exactly as the healthy run,
with the same batch losses, but its outputs change by 0.6225 on average when
the same rows are evaluated again.

**Shuffled targets** scores 0.4155 against the constant's 0.4088, and 0.4091
at a tenth of the rate. **Stale gradients** ends with a batch loss of 43.85,
because every step applies the sum of all earlier gradients.

**Overfitting one batch.** Before a full run, train on one small batch until
the loss is near zero. Here the batch is six late and six on-time training
rows, trained for 300 steps:
- the healthy model reaches 0.0193;
- stale gradients (0.7943) and shuffled targets (0.7685) don't reach 0.05;
- the other four faults reach it, rate 8 included (0.0003).

So this check catches the two faults that stop the optimizer from fitting any
data. It doesn't catch the ones that only show on held-out rows or in how the
loss is computed. Twelve rows with mismatched targets aren't memorized within
300 steps at this rate.

**Repair.** Each fault is a setting, so repairing it gives the healthy
trainer again. The healthy run from the same seeds gives the same record, bit
for bit, so the repaired run restores the baseline's loss exactly.

**More seeds.** On chapter 103's data seeds 71 to 74 the same seven runs give
28 of 28 exact diagnoses: each fault alone, and nothing for the healthy run.

## Supported profile

`cpu-f64-diagnose-v1`:
- binary64, chapter 103's models and data bounds, chapter 104's tape;
- 1 to 512 epochs and at most 60,000 steps;
- 1 to 16 slots, at least as many as the batch, and a program for a full
  block that fits the tape's 4,096 nodes;
- learning rates in (0, 16] and dropout in [0, 1);
- a classifier's training rows must include both classes, with targets 0
  or 1;
- every feature and target of the table within ±2²⁰.

Anything else is refused as `shape`, or as `capacity` when the caller's
parameter storage or the tape's nodes are too small, before any work. The
caller's parameters are written only when a run finishes. A run that diverges
writes the last parameters inside the domain.

## How it is checked

- **Coax checks:** 13 checks in debug and release. They cover:
  - the line at rates 0.25, 0.45, 0.55 and 4, against the values above,
    including the step where the rate-4 run diverges and its last epoch's
    loss, 2 · 101,250² + 162² / 2;
  - the stale buffer after two and three steps (√68 and √76.25, landing on
    (4, 1.375) and (4, 2.40625));
  - the padding gap of 5/11 and its step to (1, 0.25);
  - the healthy trainer against chapter 104's `tp_train`, parameter for
    parameter, and its losses against chapter 103's `tb_log_loss` and
    `tb_base_logit`;
  - shuffled targets against the healthy trainer on targets permuted
    beforehand, parameter for parameter;
  - identical units staying identical after training;
  - dropout masks by step, slot and feature, and evaluation passes by their
    counters, recomputed outside the trainer;
  - every diagnosis rule at its threshold, and every refusal at its bound;
  - the deadline data: chapter 103's split and scaling.
- **The example** compares its transcript exactly in debug and release. It
  covers the classifier's seven runs on seed 70, overfitting one batch, a
  rerun of the healthy trainer, the 28 runs on seeds 71 to 74, and six
  refusals.
- **Mutation testing** of `diagnose.coax` with `--root-only` kills all 352
  mutants. Early runs left survivors where a check couldn't tell two
  layouts apart, such as two dropout counter layouts that drew the same masks
  for a two-slot batch. Each was fixed by sharpening the check or simplifying
  the code.

## What this does not do yet

- **Summaries, not curves.** The record keeps the first and last epochs'
  losses and the last step's buffer and loss gap. It doesn't keep per-step
  series, per-layer activation distributions or per-parameter gradient norms.
- **Fixed thresholds for this data.** The held-out margin of 0.05 and the gap
  of 10⁻⁹ were set for chapter 103's data and models. They aren't calibrated
  against a noise floor the way
  [chapter 240](ch240-how-far-a-metric-moves-by-itself.md) calibrates one.
- **A milder rate can hide.** A rate only a few times too large can still
  train well enough on some data that neither the loss curve nor the slower
  run shows anything to find.
- **Known faults only.** The faults are switches in this trainer, and each
  rule reads the symptom of one of them. A different bug can produce several
  of these symptoms or none.
