# 156. Fit our own calibration, and refuse a stale one

[Chapter 12](ch012-are-the-probabilities-any-good.md) measured whether a
model's probabilities are any good, using the calibration the model shipped
with. This chapter fits our own. For each question it fits three kinds of map
on one split of data, then keeps one only if it passes a gate on a second
split, and reports it once on a third. Each map records the model and the
question set it was fitted for. It is refused for any other, so a new model
version can't silently reuse an old model's calibration.

[Chapter 157](ch157-thresholds-from-written-costs.md) derives decisions from
probabilities and assumes they are calibrated. This chapter is what makes that
assumption checkable.

```shell
coax/bin/coax run psionic/examples/calibration.coax
psionic/check.sh --only calibrate
```

```jai
question 0, gate split: raw ECE 0.0995, log loss 0.5306
  temperature: ECE 0.0184, log loss 0.4506, passes
  binned: ECE 0.0112, log loss 0.458, passes
  isotonic: ECE 0.012, log loss 0.4659, passes
  chosen: temperature
question 1, gate split: raw ECE 0.0428, log loss 0.5283
  temperature: ECE 0.0314, log loss 0.5255, passes
  binned: ECE 0.0254, log loss 0.4614, passes
  isotonic: ECE 0.0324, log loss 0.4674, passes
  chosen: binned
test split, raw -> calibrated:
  question 0: ECE 0.098 -> 0.01, Brier 0.318 -> 0.2978, log loss 0.5451 -> 0.4622, confident errors 149 -> 43
  question 1: ECE 0.0368 -> 0.0133, Brier 0.3533 -> 0.3052, log loss 0.5312 -> 0.4724, confident errors 4 -> 23
question 0 reliability, mean probability / rate of yes per tenth:
  raw: 0.0267/0.1244 0.1494/0.2748 0.2468/0.3871 0.3499/0.4658 0.4538/0.5 0.551/0.5696 0.6509/0.6047 0.7516/0.6279 0.8547/0.6977 0.9747/0.8877
  calibrated: 0.069/0.0714 0.1448/0.153 0.2482/0.2138 0.3483/0.3742 0.4511/0.4882 0.55/0.5779 0.6492/0.65 0.7529/0.7719 0.8582/0.869 0.9315/0.9186
model 2 through model 1's maps: stale
model 1's maps for question set 2: stale
model 2, refitted on its own splits:
  question 0: binned, ECE 0.0428 -> 0.0304, log loss 0.5297 -> 0.4774
  question 1: temperature, ECE 0.0732 -> 0.0158, log loss 0.5182 -> 0.4841
```

The [example](../examples/calibration.coax) runs in about two seconds.

## A model that is wrong in two different ways

The data are synthetic, so the true probabilities are known:
- Row i asks question i mod 2.
- Its true logit is uniform in [−3, 3], and its label is yes with exactly
  that probability.
- The model reports σ(scale · logit + shift), with a scale and a shift per
  question.

<!-- source: psionic/lib/calibrate.coax simulate -->
```jai
// calibrate-sim-v1. Row i asks question i mod 2; its true logit is uniform in
// [-3, 3] at counter 2i, its label yes with that probability at 2i+1 (yes when
// u + p reaches 1). The model reports sigmoid(scale * logit + shift), with a
// scale and shift per question: model = [scale0, shift0, scale1, shift1].
cf_simulate :: (seed:u64,model:[]f64,q:[]int,p:[]f64,y:[]f64,overlap:bool)->CfStatus #export_c {
    n:=p.count;
    if n<1 || n>CF_ROWS_MAX || model.count!=4 {return CfStatus.shape;}
    if q.count<n || y.count<n {return CfStatus.capacity;}
    if overlap {return CfStatus.alias;}
    for v:model {if !tb_in(v,-8,8) {return CfStatus.number;}}
    for i:0..<n {
        k:=i%2;l:=6*tb_uniform(seed,cast(u64)(2*i))-3;
        q[i]=k;y[i]=floor(tb_uniform(seed,cast(u64)(2*i+1))+tb_sigmoid(l));
        p[i]=tb_sigmoid(model[2*k]*l+model[2*k+1]);
    }
    return CfStatus.ok;
}
```

Model 1 is wrong in a different way on each question:
- On question 0 it has scale 2. It is **overconfident**: when it says 0.97,
  the rate of yes is 0.89.
- On question 1 it has scale 0.6 and shift −0.6. It is **underconfident**, and
  it also leans towards no.

Each split has 4,000 rows, 2,000 per question. The fit split uses seed 156,
the gate split 157 and the test split 158.

## Three kinds of map

<!-- source: psionic/lib/calibrate.coax fit -->
```jai
// Temperature: the slope a minimizing log loss of sigmoid(a z) over z = logit(p).
// The loss is convex in a, so its derivative sum (sigmoid(a z) - y) z rises
// with a; halving finds where it crosses zero, within [0.05, 20]. A
// derivative of exactly zero counts as still falling.
cf_slope :: (p:[]f64,y:[]f64,rows:[]int)->f64 {
    lo:=CF_SLOPE_MIN;hi:=CF_SLOPE_MAX;
    while true {
        mid:=(lo+hi)/2;
        if mid==lo || mid==hi {break;}
        g:PsionicAccumulator;
        for r:rows {z:=cf_logit(p[r]);g=psionic_ops_accumulate(g,(tb_sigmoid(mid*z)-y[r])*z);}
        if psionic_ops_total(g)>0 {hi=mid;} else {lo=mid;}
    }
    return hi;
}
// Binned: ten groups of equal count by probability, each mapped to its rate of yes.
// Isotonic: pool adjacent groups while a rate fails to rise (pool-adjacent-violators).
// Both knots are a group's smallest probability; rates are compared as integers.
cf_steps :: (map:CfMap,isotonic:bool,p:[]f64,y:[]f64,rows:[]int)->CfMap {
    m:=map;
    // One more group than a map holds, for the row that may yet pool.
    n:=rows.count;yes:[257]int;count:[257]int;knot:[257]f64;k:=0;
    for i:0..<n {
        r:=rows[i];
        // A new group starts at every row (isotonic) or every tenth of the rows (binned).
        start:=isotonic || i*CF_BINS/n!=(i-1)*CF_BINS/n || i==0;
        if start {knot[k]=p[r];yes[k]=0;count[k]=0;k+=1;}
        yes[k-1]+=cast(int)y[r];count[k-1]+=1;
        while isotonic && k>1 && yes[k-2]*count[k-1]>=yes[k-1]*count[k-2] {
            yes[k-2]+=yes[k-1];count[k-2]+=count[k-1];k-=1;
        }
        if k>CF_KNOTS_MAX {return CfMap.{status=CfStatus.capacity};}
    }
    for j:0..<k {m.knot[j]=knot[j];m.value[j]=cf_clamp(f64(yes[j])/f64(count[j]));}
    m.knots=k;
    return m;
}
cf_fit :: (kind:CfKind,model:u64,questions:u64,question:int,q:[]int,p:[]f64,y:[]f64,rows:[]int)->CfMap #export_c {
    n:=p.count;
    // A question outside [0, 8) has no rows, and is refused with the rest below 200.
    if n>CF_ROWS_MAX || q.count!=n || y.count!=n || cast(int)kind>cast(int)CfKind.isotonic {return CfMap.{status=CfStatus.shape};}
    if rows.count<n {return CfMap.{status=CfStatus.capacity};}
    if !cf_rows_ok(q,p,y) {return CfMap.{status=CfStatus.number};}
    m:=CfMap.{kind=kind,model=model,questions=questions,question=question,slope=1};
    own:=rows[0..<cf_rows(q,question,rows)];
    if own.count<CF_BINS*CF_MIN_ROWS {return CfMap.{status=CfStatus.shape};}
    if kind==CfKind.temperature {m.slope=cf_slope(p,y,own);}
    if kind==CfKind.binned || kind==CfKind.isotonic {
        cf_sort(p,own);m=cf_steps(m,kind==CfKind.isotonic,p,y,own);
    }
    return m;
}
```

- **Temperature** keeps the model's ranking and rescales its logits by one
  slope a, the one that minimizes log loss on the fit split. The log loss is
  convex in a, so halving on its derivative finds a to the last bit. On
  question 0 the slope is 0.5085, close to the 1/2 that undoes a scale of 2.
  A slope can't undo a shift, so on question 1 it only reaches 1.1061.
- **Binned** sorts the rows by probability, cuts them into ten groups of equal
  count, and maps each group to its rate of yes.
- **Isotonic** fits the rising step function closest to the labels: start
  with every row as its own group, and pool neighbors while a rate fails to
  rise. On model 1 it keeps 26 steps on question 0 and 28 on question 1. Rates
  are compared as integers, by cross-multiplying counts, so a tie is a tie.

A map needs at least 200 rows of its question (ten bins of 20), and holds at
most 256 steps. Values are clamped to [10⁻⁶, 1 − 10⁻⁶] so that log loss stays
finite.

## A map knows what it is for

<!-- source: psionic/lib/calibrate.coax apply -->
```jai
// A map applies only to the model and question set it was fitted for.
cf_map :: (m:CfMap,p:f64)->f64 {
    if m.kind==CfKind.none {return p;}
    if m.kind==CfKind.temperature {return cf_clamp(tb_sigmoid(m.slope*cf_logit(p)));}
    k:=0;
    for j:1..<m.knots {if m.knot[j]<=p {k=j;}}
    return m.value[k];
}
cf_apply :: (maps:[]CfMap,model:u64,questions:u64,q:[]int,p:[]f64,out:[]f64,overlap:bool)->CfStatus #export_c {
    n:=p.count;
    if n<1 || n>CF_ROWS_MAX || q.count!=n || maps.count<1 || maps.count>CF_QUESTIONS_MAX {return CfStatus.shape;}
    if out.count<n {return CfStatus.capacity;}
    if overlap {return CfStatus.alias;}
    for m,k:maps {
        if m.status!=CfStatus.ok || m.question!=k {return CfStatus.shape;}
        if m.model!=model || m.questions!=questions {return CfStatus.stale;}
    }
    for i:0..<n {
        if q[i]<0 || q[i]>=maps.count || !tb_in(p[i],0,1) {return CfStatus.number;}
        out[i]=cf_map(maps[q[i]],p[i]);
    }
    return CfStatus.ok;
}
```

`cf_apply` takes one map per question, in order, and the identity of the model
and question set the probabilities came from. A map fitted for another model
or another question set is refused as `stale`, before any output is written.
A failed map, or one in the wrong slot, is refused as `shape`. A map of kind
`none` passes probabilities through unchanged. It is how a question whose
fitted maps all failed the gate is served raw.

## Measure, then gate

<!-- source: psionic/lib/calibrate.coax measure -->
```jai
// Expected calibration error as chapter 12 measures it (top label, 15 equal
// bins (b/15, (b+1)/15]), Brier summed over both answers, log loss, and
// confident errors: a top label of at least 0.9 that is wrong.
cf_measure :: (p:[]f64,y:[]f64,rows:[]int)->CfReport #export_c {
    if p.count!=y.count || !tb_row_ok(rows,p.count) {return CfReport.{status=CfStatus.shape};}
    for r:rows {if !tb_in(p[r],0,1) || (y[r]!=0 && y[r]!=1) {return CfReport.{status=CfStatus.number};}}
    conf:[15]PsionicAccumulator;hit:[15]int;brier:PsionicAccumulator;loss:PsionicAccumulator;
    report:=CfReport.{rows=rows.count};
    for r:rows {
        // The top label is at least 1/2, so only bins 7 to 14 can hold it.
        v:=p[r];top:=0.5+abs(v-0.5);
        right:=(v>=0.5)==(y[r]==1);
        b:=7;while top>f64(b+1)/CF_ECE_BINS {b+=1;}
        conf[b]=psionic_ops_accumulate(conf[b],top);if right {hit[b]+=1;}
        if top>=0.9 && !right {report.confident_errors+=1;}
        e:=v-y[r];brier=psionic_ops_accumulate(brier,2*e*e);
        c:=cf_clamp(v);if y[r]==0 {c=1-c;}
        loss=psionic_ops_accumulate(loss,-log(c));
    }
    ece:PsionicAccumulator;
    for b:7..<CF_ECE_BINS {ece=psionic_ops_accumulate(ece,abs(psionic_ops_total(conf[b])-f64(hit[b])));}
    n:=f64(rows.count);
    report.ece=psionic_ops_total(ece)/n;report.brier=psionic_ops_total(brier)/n;report.log_loss=psionic_ops_total(loss)/n;
    return report;
}
// Reliability: ten equal-width bins of p, each with its count, mean
// probability and rate of yes. A bin under 20 rows reports a rate of -1.
cf_reliability :: (p:[]f64,y:[]f64,rows:[]int,count:[]int,mean:[]f64,rate:[]f64,overlap:bool)->CfStatus #export_c {
    if p.count!=y.count || !tb_row_ok(rows,p.count) {return CfStatus.shape;}
    if count.count<CF_BINS || mean.count<CF_BINS || rate.count<CF_BINS {return CfStatus.capacity;}
    if overlap {return CfStatus.alias;}
    for r:rows {if !tb_in(p[r],0,1) || (y[r]!=0 && y[r]!=1) {return CfStatus.number;}}
    s:[10]PsionicAccumulator;yes:[10]int;
    for b:0..<CF_BINS {count[b]=0;}
    for r:rows {
        b:=cast(int)(p[r]*CF_BINS);if b==CF_BINS {b=CF_BINS-1;}
        count[b]+=1;s[b]=psionic_ops_accumulate(s[b],p[r]);yes[b]+=cast(int)y[r];
    }
    for b:0..<CF_BINS {
        mean[b]=-1;rate[b]=-1;
        if count[b]>=CF_MIN_ROWS {mean[b]=psionic_ops_total(s[b])/f64(count[b]);rate[b]=f64(yes[b])/f64(count[b]);}
    }
    return CfStatus.ok;
}
// The frozen gate: a map is served only if it lowers both calibration error
// and log loss from the raw probabilities on the gate split.
cf_gate :: (raw:CfReport,mapped:CfReport)->bool #export_c {
    return raw.status==CfStatus.ok && mapped.status==CfStatus.ok && mapped.ece<raw.ece && mapped.log_loss<raw.log_loss;
}
```

The measures are chapter 12's:
- **Expected calibration error** puts the top label's probability in 15
  equal bins and averages the gap between confidence and accuracy.
- **Brier** sums the squared error over both answers.
- **Log loss** is clamped like the maps.
- **Confident errors** count a top label of at least 0.9 that is wrong.

Reliability uses ten bins of p. A bin with fewer than 20 rows reports no mean
and no rate, rather than a noisy one.

The gate is frozen before any map is fitted. On the gate split, a map must
lower both the calibration error and the log loss of the raw probabilities. If
several pass, the one with the lowest log loss is kept. Calibration error alone
can be lowered by a map that loses information, so log loss ranks first.

On question 0 all three pass. Binned has the lowest calibration error (0.0112),
but temperature has the lowest log loss (0.4506), and temperature is kept. On
question 1 binned is kept: temperature passes, barely, because a slope can't
remove the lean towards no.

## The test split, once

| Model 1, test split | Raw | Calibrated |
| --- | --- | --- |
| Question 0 (temperature): calibration error | 0.0980 | 0.0100 |
| Brier | 0.3180 | 0.2978 |
| Log loss | 0.5451 | 0.4622 |
| Confident errors | 149 | 43 |
| Question 1 (binned): calibration error | 0.0368 | 0.0133 |
| Brier | 0.3533 | 0.3052 |
| Log loss | 0.5312 | 0.4724 |
| Confident errors | 4 | 23 |

On question 1, confident errors rise from 4 to 23. That isn't a regression.
The raw model was underconfident, so it almost never said 0.9. Calibrated, it
says 0.9 when the rate of yes is 0.9, and about one in ten of those answers is
wrong. Counting confident errors alone would have rejected the better map.

The reliability rows show the same thing for question 0: raw, the bin with
mean probability 0.9747 is right 0.8877 of the time; calibrated, the top bin
has mean 0.9315 and rate 0.9186.

## A new model version

Model 2 has different errors (scale 1 and shift 0.9 on question 0, scale 1.5
on question 1). Through model 1's maps, `cf_apply` returns `stale` instead of
mapping its probabilities with the wrong correction. The same maps under a
different question set are refused the same way. Model 2 gets its own three
splits (seeds 256 to 258), and the same procedure chooses binned for
question 0 and temperature for question 1.

## Supported profile

`cpu-f64-calibrate-v1`:
- binary64, with chapter 3's compensated accumulator for every sum;
- yes-or-no questions, 1 to 8 per question set;
- 1 to 8,192 rows, probabilities within [0, 1], labels of exactly 0 or 1;
- at least 200 rows of a question to fit it;
- temperatures within [0.05, 20], up to 256 isotonic steps, ten bins;
- a map bound to one model identity and one question set.

Every exported procedure that writes caller storage takes an `overlap`
observation, which the native adapter computes from real addresses.

## How it is checked

- **Coax checks:** 67 checks in debug and release, counting the units it
  loads. They cover:
  - each map worked by hand, including ties in pooling and rows given out of
    order;
  - a slope at both ends of its range, and one where the derivative is
    exactly zero;
  - the largest isotonic map (256 steps) and the first one too large;
  - every refusal at its bound, and stale identities;
  - every measure by hand, including neighboring calibration bins;
  - the reliability floor of 20 rows, the gate and the simulator.
- **An independent oracle,** [calibrate_oracle.py](../tests/calibrate_oracle.py),
  written from the specification. It finds the slope by a different search,
  over the integer bit patterns of doubles, pools isotonic steps with exact
  fractions, and runs the whole experiment for both models with exactly
  rounded sums.
- **The native harness** [calibrate.c](../tests/calibrate.c) runs the same
  experiment through the span adapter, plain and under ASan/UBSan, in debug
  and release. Tolerances were frozen before comparison:
  - chosen kinds, gate verdicts, step counts and values, confident errors and
    reliability counts must be equal;
  - slopes, calibration errors, Brier, log loss and reliability means and
    rates within 10⁻¹².

  The largest difference was 2.8 × 10⁻¹⁷. The harness also checks a kind past
  isotonic, NaN, infinity and overlapping storage, which Coax can't express.
- **Mutation testing** of `calibrate.coax` kills all 1,264 mutants. The first
  run left 20 survivors, each fixed in the code or the checks:
  - checks that could never fail (a question index outside its range has no
    rows, and the 200-row floor already refuses it) were removed;
  - the top label is at least 1/2, so its bin search starts at bin 7;
  - clamping is written as a range test and a rounding, with no boundary to
    move;
  - checks were added for rows out of order, one-row calls, a derivative of
    exactly zero and neighboring bins.

The [verification record](../../docs/verification/psionic-ch156-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **Synthetic data.** Real Elephant probabilities and labels need a
  calibration set drawn from logged traffic, which chapter 158 records.
- **One draw per split.** No interval on the calibration error, and the gate
  compares point estimates. Chapter 180's gates can say "unverifiable".
- **Identity by number.** A map names a model and a question set by number. A
  content hash of the weights and the question text would be stronger.
- **Yes-or-no questions only.** Multi-way answers need a map per answer, or a
  vector temperature.
