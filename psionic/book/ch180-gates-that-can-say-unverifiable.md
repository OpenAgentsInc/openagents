# 180. Gates that can say unverifiable

[Chapter 179](ch179-how-far-a-metric-moves-by-itself.md) measured how far a
metric moves by itself. This chapter turns those measurements into verdicts.
A verdict is one of three things, always with its reason:
- **passed:** the evidence clearly supports the claim;
- **failed:** the evidence clearly goes against it;
- **unverifiable:** the evidence can't decide, or the suite couldn't have shown
  a gain.

There are three gates:
- the **decision gate** passes a gain only beyond the noise floor;
- the **probability gate** ranks log loss and confident errors above
  calibration error and accuracy;
- the **evaluation gate** lets cost and time block a result, but never earn
  one.

Chapters 12, 156, 157 and 179 report through them.

```shell
coax/bin/coax run psionic/examples/verdicts.coax
psionic/check.sh --only verdict
```

```jai
chapter 179's suite, candidates with 4 seeds against the baseline's first 4:
  -0.5 logits, open tasks: gain -0.043, floor 0.0893, 1000 of 1000 at or below it -> failed (within the floor)
  -0.5 logits, saturated tasks: gain -0.0219, floor 0.0184, 1000 of 1000 at or below it -> unverifiable (no headroom)
  0.5 logits, open tasks: gain 0.0555, floor 0.0893, 822 of 1000 at or below it -> unverifiable (near the floor)
  0.5 logits, saturated tasks: gain 0.0031, floor 0.0184, 1000 of 1000 at or below it -> unverifiable (no headroom)
  1.5 logits, open tasks: gain 0.1359, floor 0.0893, 58 of 1000 at or below it -> unverifiable (near the floor)
  1.5 logits, saturated tasks: gain 0.0031, floor 0.0184, 996 of 1000 at or below it -> unverifiable (no headroom)
  2.5 logits, open tasks: gain 0.3297, floor 0.0893, 0 of 1000 at or below it -> passed (beyond the floor)
  2.5 logits, saturated tasks: gain 0.0063, floor 0.0184, 996 of 1000 at or below it -> unverifiable (no headroom)
chapter 12, Elephant's accuracy, any candidate: unverifiable (no headroom)
chapter 156, test split, raw -> calibrated:
  question 0 (temperature): log loss 0.5451 -> 0.4622, confident errors 149 -> 43, calibration error 0.098 -> 0.01, right 1574 -> 1574 of 2000
    replicates of 1000 at or below 0: candidate ahead on log loss 0, baseline ahead 1000, baseline ahead on confident errors 1000 -> passed (lower log loss)
  question 1 (binned): log loss 0.5312 -> 0.4724, confident errors 4 -> 23, calibration error 0.0368 -> 0.0133, right 1493 -> 1579 of 2000
    replicates of 1000 at or below 0: candidate ahead on log loss 0, baseline ahead 1000, baseline ahead on confident errors 0 -> failed (more confident errors)
chapter 157, the record against always Elephant (voice) and always Mammoth (background):
  voice interruption: cost 0.6548 -> 0.5815, gain 0.0733, floor 0.0, 0 of 1000 at or below it -> passed (beyond the floor)
    0.15 s per decision, limit 0.5 s -> passed (beyond the floor)
  background classification: cost 0.2362 -> 0.1587, gain 0.0775, floor 0.0003, 0 of 1000 at or below it -> passed (beyond the floor)
    35.47 s per decision, limit 30.0 s -> failed (over time)
```

The [example](../examples/verdicts.coax) runs in about two seconds.

## What clear means

Every gate asks the same question of a bootstrap: how many replicates fall on
the wrong side of a line?

<!-- source: psionic/lib/verdict.coax side -->
```jai
vd :: (verdict:VdVerdict,reason:VdReason)->VdResult {return VdResult.{verdict=verdict,reason=reason};}
// 1 when at most 1 in 40 replicates fall at or below the line, -1 when at
// most 1 in 40 fall above it, 0 otherwise.
vd_side :: (below:int,replicates:int)->int {
    if below*VD_CLEAR<=replicates {return 1;}
    if (replicates-below)*VD_CLEAR<=replicates {return -1;}
    return 0;
}
```

Clear means at most 1 replicate in 40 on the other side, so 25 of 1,000. That
is the usual 95 percent two-sided rule, read one side at a time.

## The decision gate

<!-- source: psionic/lib/verdict.coax decision -->
```jai
// A candidate's per-item scores b against a baseline's a (higher is better),
// on one partition, with the floor measured for the baseline's blocks.
vd_decision :: (a:[]f64,b:[]f64,task:[]int,part:[]int,which:int,seeds:NsFloor,ceiling:f64,seed:u64,replicates:int)->VdDecision #export_c {
    measured:=seeds.status==NsStatus.ok && seeds.blocks>=2;
    noise:=ifx measured then seeds.floor else 0.0;
    d:=VdDecision.{room=ns_headroom(a,part,which,ceiling,noise),gain=ns_boot(a,b,task,part,which,noise,seed,replicates)};
    if d.room.status==NsStatus.shape || d.gain.status==NsStatus.shape {return VdDecision.{status=VdStatus.shape};}
    if d.room.status!=NsStatus.ok || d.gain.status!=NsStatus.ok {return VdDecision.{status=VdStatus.number};}
    if d.room.refused {d.result=vd(VdVerdict.unverifiable,VdReason.no_headroom);return d;}
    if !measured {d.result=vd(VdVerdict.unverifiable,VdReason.no_floor);return d;}
    if d.gain.tasks<VD_MIN_TASKS {d.result=vd(VdVerdict.unverifiable,VdReason.few_tasks);return d;}
    side:=vd_side(d.gain.below,replicates);
    if side==1 {d.result=vd(VdVerdict.passed,VdReason.beyond_floor);}
    else if side==-1 {d.result=vd(VdVerdict.failed,VdReason.within_floor);}
    else {d.result=vd(VdVerdict.unverifiable,VdReason.near_floor);}
    return d;
}
```

The order of the checks matters:
1. **Headroom.** A partition within the floor of its ceiling is unverifiable,
   whatever the candidate does.
2. **A floor.** A floor needs at least two blocks of seeds. Without one, the
   gain can't be judged.
3. **Ten tasks.** Fewer, and the task bootstrap has too little to draw from.
4. **The gain.** Clearly above the floor passes. Clearly at or below it fails.
   Anything between is unverifiable.

Chapter 179's candidates, each with 4 seeds:

| Candidate | Open tasks: gain, replicates ≤ floor 0.0893 | Verdict | Saturated tasks |
| --- | --- | --- | --- |
| 0.5 logits worse | −0.043, 1,000 | failed: within the floor | unverifiable: no headroom |
| 0.5 logits better | 0.0555, 822 | unverifiable: near the floor | unverifiable: no headroom |
| 1.5 logits better | 0.1359, 58 | unverifiable: near the floor | unverifiable: no headroom |
| 2.5 logits better | 0.3297, 0 | passed: beyond the floor | unverifiable: no headroom |

The candidate that is 1.5 logits better is better, and its measured gain is
above the floor. Still, 58 of 1,000 task replicates fall back to the floor, so
the gate says unverifiable. More seeds or more tasks would settle it.

On the saturated tasks, even the candidate that is 2.5 logits better is
unverifiable. The gate never looks at its gain.

**Chapter 12:** Elephant's accuracy on the held-out suite is unverifiable for
any candidate, because there is no headroom. The gate reaches that before it
asks for a floor, which this suite doesn't have either.

## The probability gate

<!-- source: psionic/lib/verdict.coax probability -->
```jai
// Yes-or-no probabilities of a baseline a and a candidate b on the same
// labelled items. Log loss decides; more confident errors block a pass;
// calibration error and accuracy are reported and decide nothing.
vd_probability :: (a:[]f64,b:[]f64,y:[]f64,task:[]int,seed:u64,replicates:int)->VdProbability #export_c {
    n:=y.count;
    if n<1 || n>NS_ITEMS_MAX || a.count!=n || b.count!=n || task.count!=n {return VdProbability.{status=VdStatus.shape};}
    if replicates<2 || replicates>NS_REPLICATES_MAX {return VdProbability.{status=VdStatus.shape};}
    for t:task {if t<0 || t>=NS_TASKS_MAX {return VdProbability.{status=VdStatus.shape};}}
    for i:0..<n {
        if !tb_in(a[i],0,1) || !tb_in(b[i],0,1) || (y[i]!=0 && y[i]!=1) {return VdProbability.{status=VdStatus.number};}
    }
    // Per item, higher is better: negated log loss and negated confident error.
    loss:[2][NS_ITEMS_MAX]f64;wrong:[2][NS_ITEMS_MAX]f64;part:[NS_ITEMS_MAX]int;rows:[NS_ITEMS_MAX]int;
    r:=VdProbability.{items=n};
    for k:0..<2 {
        p:=ifx k==0 then a else b;total:PsionicAccumulator;
        for i:0..<n {
            c:=cf_clamp(p[i]);if y[i]==0 {c=1-c;}
            loss[k][i]=log(c);total=psionic_ops_accumulate(total,-loss[k][i]);
            right:=(p[i]>=0.5)==(y[i]==1);top:=0.5+abs(p[i]-0.5);
            if right {r.correct[k]+=1;}
            if top>=0.9 && !right {wrong[k][i]=-1;r.errors[k]+=1;}
            rows[i]=i;
        }
        r.loss[k]=psionic_ops_total(total)/f64(n);r.ece[k]=cf_measure(p,y,rows[0..<n]).ece;
    }
    r.lower=ns_boot(loss[0][0..<n],loss[1][0..<n],task,part[0..<n],0,0,seed,replicates);
    r.higher=ns_boot(loss[1][0..<n],loss[0][0..<n],task,part[0..<n],0,0,seed,replicates);
    r.confident=ns_boot(wrong[1][0..<n],wrong[0][0..<n],task,part[0..<n],0,0,seed,replicates);
    if r.lower.tasks<VD_MIN_TASKS {r.result=vd(VdVerdict.unverifiable,VdReason.few_tasks);}
    else if vd_side(r.higher.below,replicates)==1 {r.result=vd(VdVerdict.failed,VdReason.loss_higher);}
    else if vd_side(r.confident.below,replicates)==1 {r.result=vd(VdVerdict.failed,VdReason.more_confident_errors);}
    else if vd_side(r.lower.below,replicates)==1 {r.result=vd(VdVerdict.passed,VdReason.loss_lower);}
    else {r.result=vd(VdVerdict.unverifiable,VdReason.loss_unclear);}
    return r;
}
```

Log loss decides the direction. More confident errors then block a pass. Each
is judged by a task bootstrap. Calibration error and accuracy are reported,
and decide nothing:
- A map that answers the base rate has no calibration error at all, as chapter
  158 noted. Its log loss is higher, so the gate fails it.
- Accuracy ignores how sure a wrong answer was.

**Chapter 156** chose a map per question by its own gate: lower calibration
error and lower log loss than the raw probabilities. On the test split, this
gate disagrees about question 1:

| Question | Map | Log loss | Confident errors | Calibration error | Right of 2,000 | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | temperature | 0.5451 → 0.4622 | 149 → 43 | 0.098 → 0.01 | 1,574 → 1,574 | passed: lower log loss |
| 1 | binned | 0.5312 → 0.4724 | 4 → 23 | 0.0368 → 0.0133 | 1,493 → 1,579 | failed: more confident errors |

The binned map improves three of the four measures, and 86 more answers are
right. But 23 answers are now wrong at 0.9 or more, up from 4, and no task
replicate has the baseline behind on confident errors. A confident wrong
answer is the one a consumer acts on without checking, so this gate refuses
the map that chapter 156 served.

## The evaluation gate

<!-- source: psionic/lib/verdict.coax evaluate -->
```jai
// Cost and seconds per decision against limits written before the run. Over
// a limit fails; unmeasured makes a pass unverifiable; a failure stays.
vd_measured :: (v:f64)->bool {return tb_in(v,0,NS_BIG);}
vd_evaluate :: (quality:VdResult,cost:f64,seconds:f64,cost_limit:f64,seconds_limit:f64)->VdResult #export_c {
    if quality.verdict==VdVerdict.failed {return quality;}
    if vd_measured(cost) && vd_measured(cost_limit) && cost>cost_limit {return vd(VdVerdict.failed,VdReason.over_cost);}
    if vd_measured(seconds) && vd_measured(seconds_limit) && seconds>seconds_limit {return vd(VdVerdict.failed,VdReason.over_time);}
    if !vd_measured(cost) || !vd_measured(seconds) || !vd_measured(cost_limit) || !vd_measured(seconds_limit) {
        return vd(VdVerdict.unverifiable,VdReason.unmeasured);
    }
    return quality;
}
```

Limits are written before the run. A result over a limit fails. A cost or time
that wasn't measured makes a pass unverifiable. Nothing here can turn an
unverifiable result into a pass, or a failure into anything else.

**Chapter 157** compared a threshold record with the best fixed policy. The
floor comes from 16 validation draws, the original and 15 more, each producing
its own record:

| Consumer | Cost per decision | Gain | Floor | Decision gate | Seconds per decision | Limit | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Voice interruption, against always Elephant | 0.6548 → 0.5815 | 0.0733 | 0.0 | passed | 0.15 | 0.5 | passed |
| Background classification, against always Mammoth | 0.2362 → 0.1587 | 0.0775 | 0.0003 | passed | 35.47 | 30 | failed: over time |

The voice record's floor is 0, because its thresholds don't depend on
Mammoth's measured accuracy: it never escalates, so every validation draw
writes the same record.

The background record is cheaper, but it gets there by asking a person on 459
items, each for 300 seconds. Its cost model priced that time at 0.001 per
second. A written limit of 30 seconds per decision is a separate promise,
and the evaluation gate holds it. The cost model alone would not have.

## Supported profile

`cpu-f64-verdict-v1`:
- binary64, with chapter 3's compensated accumulator for every sum;
- chapter 179's floor, headroom and task bootstrap, chapter 156's measures;
- clear meaning at most 1 replicate in 40 on the other side, and at least 10
  tasks;
- yes-or-no probabilities in the probability gate, confident meaning a top
  label of at least 0.9;
- costs and seconds per decision against limits up to 10³⁰⁰.

The gates read spans and write none. The native adapter only names a native
array as scores.

## How it is checked

- **Coax checks:** 83 checks in debug and release, counting the units it
  loads. They cover:
  - clear at exactly 1 in 40, on both sides and with odd replicate counts;
  - every decision verdict and reason by hand, including headroom exactly at
    the floor, one block, a refused floor and nine tasks;
  - every probability verdict by hand: lower and higher log loss, a sharper
    map that is confidently wrong once per task, and the base rate;
  - the probability gate's measures against chapter 156's `cf_measure`;
  - every evaluation case: over cost, over time, unmeasured, a failure that
    stays, and an unverifiable result that stays;
  - every bound and refusal.
- **An independent oracle,** [verdict_oracle.py](../tests/verdict_oracle.py),
  writes the gates again. It takes its runs, maps and thresholds from chapters
  179, 156 and 157's oracles, each independent of the library.
- **The native harness** [verdict.c](../tests/verdict.c) runs the whole example
  through the adapter, plain and under ASan/UBSan, in debug and release.
  Tolerances were frozen before comparison:
  - verdicts, reasons, task counts, replicate counts, confident errors and
    right answers must be equal;
  - gains, floors, log losses, calibration errors, costs and seconds within
    10⁻¹².

  Every value was identical.
- **Mutation testing** of `verdict.coax` kills all 1,706 mutants. The first
  run left nine survivors, each fixed in the code or the checks:
  - verdicts and reasons start with "unverifiable" and "none", so a result
    nobody set is never a pass;
  - a check helper is tested on mismatches;
  - checks were added for a single item, 10,000 replicates, a bad first item,
    a probability of exactly 0.5 and a top label of exactly 0.9.

The [verification record](../../docs/verification/psionic-ch180-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **No receipts.** Chapter 178 pins the items, questions and gate a verdict
  used.
- **Fixed rules.** The 1-in-40 line, 10 tasks and 0.9 for confident are
  constants. A suite that needs others is a different profile.
- **Yes-or-no probabilities only.** Choices and scores, as in chapter 12, need
  multi-class log loss.
- **One comparison at a time.** Many candidates against one baseline need a
  correction for multiple comparisons.
