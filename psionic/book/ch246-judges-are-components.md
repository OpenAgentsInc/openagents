# 246. Judges are components

> **Status: partly implemented.** A judge as a versioned component named by
> one identity digest; its agreement with labelled outcomes, Cohen's kappa
> with an interval and calibration error; its noise between repeated runs;
> position and length probes; a gate that says passed, failed or
> unverifiable; a refusal when the grader shares an identity or weights with
> the student or the reflector; and chapter 247 refusing a judge's grade as a
> training label. The judges here are stand-ins with written-down behavior.
> Still to come: Elephant and Mammoth themselves as judges on recorded
> answers, and real outcomes joined by chapter 217.

A search loop often needs a model to grade or route candidates.
[Chapter 244](ch244-search-a-policy-cheapest-first.md) could measure every
policy with a check that runs. Many tasks have no such check, and the
tempting shortcut is to ask a model whether the answer is right. That model
is then part of the measurement. If it prefers long answers, the search
learns to write long answers. If it grades its own model's work, the search
learns to please it.

This chapter treats a judge like any other component:
- **an identity:** one digest over the model's weights, the rubric and the
  settings, so a one-character edit to the rubric makes a different judge;
- **a measurement before use:** agreement with outcomes that a check or a
  diff labelled, Cohen's kappa with an interval, calibration error, and cost
  per judgment;
- **its own noise and biases:** how often two runs on the same item
  disagree, whether the order of a pair changes the winner, and whether it
  favors the longer answer;
- **a gate:** chapter 241's three verdicts, with a reason;
- **separation:** a grader never shares an identity or weights with the
  student it grades or the reflector that rewrites the student's prompt;
- **no labels:** a judge's grade offered as a training row is refused by
  [chapter 247](ch247-outcomes-become-training-rows.md) as an opinion.

```shell
coax/bin/coax run psionic/examples/judges.coax
coax/bin/coax test psionic/lib/judge.coax
```

## Kappa by hand

Agreement alone flatters a judge. If 60 percent of answers fail, a judge that
always says no agrees 60 percent of the time. Cohen's kappa subtracts the
agreement that the two sets of answers would reach by chance, given how often
each says yes.

Take 20 items with labels from checks. The judge says yes on 9, the checks
on 8:

| | check yes | check no |
| --- | ---: | ---: |
| judge yes | 7 | 2 |
| judge no | 1 | 10 |

- Observed agreement: (7 + 10) / 20 = 0.85.
- Chance agreement: (9/20)(8/20) + (11/20)(12/20) = (72 + 132) / 400 = 0.51.
- Kappa: (0.85 − 0.51) / (1 − 0.51) = 0.34 / 0.49 = 0.6939.

Twenty items can't pin that down. The Wilson interval for 17 of 20 at
z = 1.96, with z² = 3.8416:
- center: (0.85 + 3.8416/40) / (1 + 3.8416/20) = 0.94604 / 1.19208 = 0.79361;
- half-width: 1.96 × √(0.85 × 0.15 / 20 + 3.8416/1600) / 1.19208
  = 1.96 × √0.0087760 / 1.19208 = 0.15402;
- agreement: [0.6396, 0.9476].

Mapping both ends through the kappa formula, with chance agreement held at
0.51, gives kappa in [0.2644, 0.8931]. The judge looks good, and the
interval says the run can't tell whether it is good or poor. A gate
requiring kappa of at least 0.6 calls it unverifiable, not passed.

The judge's probabilities on these items are 0.8 for yes and 0.3 for no.
Their top labels are 0.8 (9 items, 7 right) and 0.7 (11 items, 10 right),
so chapter 215's calibration error is
(|7.2 − 7| + |7.7 − 10|) / 20 = 0.125, and the Brier score is
2 × (7 × 0.04 + 10 × 0.09 + 2 × 0.64 + 0.49) / 20 = 0.295. The unit's
checks assert each of these numbers.

## The run

The [example](../examples/judges.coax) measures five judges on 120 held-out
outcomes and 60 pairs of answers:

```text
held out: 120 outcomes at times 100 to 219; 108 labelled by a check or a diff (46 passed), 12 by the agent's own summary (not truth)
pairs: 60, the longer answer better in 33
judge 0: Elephant, rubric v1, temperature 0.3, samples 1; identity 4b03c35b; cost 1.0 per judgment, 2.0 per pair
  agreement 105/108 = 0.9722 [0.9215, 0.9905], chance 0.5123, kappa 0.943 [0.839, 0.9805]; calibration error 0.097, Brier 0.0744
  runs differ on 9/120; order changes the winner of 9/60 pairs; the first shown won both ways in 3, the second in 6; picks the longer in 35, truth 33
  -> passed (agrees)
judge 1: Elephant, rubric v2, asks for thoroughness, temperature 0.3, samples 1; identity 492b0812; cost 1.0 per judgment, 2.0 per pair
  agreement 97/108 = 0.8981 [0.8268, 0.9422], chance 0.5096, kappa 0.7923 [0.6468, 0.8821]; calibration error 0.0397, Brier 0.1362
  runs differ on 7/120; order changes the winner of 5/60 pairs; the first shown won both ways in 0, the second in 5; picks the longer in 46, truth 33
  -> failed (length)
judge 2: Elephant, A or B, temperature 0.3, samples 1; identity ccdbfc71; cost 1.0 per judgment, 1.0 per pair
  agreement 98/108 = 0.9074 [0.8379, 0.9489], chance 0.511, kappa 0.8107 [0.6685, 0.8956]; calibration error 0.095, Brier 0.0953
  runs differ on 9/120; order changes the winner of 26/60 pairs; the first shown won both ways in 26, the second in 0; picks the longer in 32, truth 33
  -> failed (position)
judge 3: Elephant, terse rubric, temperature 1.0, samples 1; identity 9dd42dc7; cost 1.0 per judgment, 2.0 per pair
  agreement 74/108 = 0.6852 [0.5925, 0.7651], chance 0.5027, kappa 0.3669 [0.1805, 0.5277]; calibration error 0.1611, Brier 0.4042
  runs differ on 45/120; order changes the winner of 32/60 pairs; the first shown won both ways in 17, the second in 15; picks the longer in 33, truth 33
  -> unverifiable (noisy)
judge 4: Mammoth, rubric v1, temperature 0.3, samples 3; identity 8986941a; cost 18.0 per judgment, 36.0 per pair
  agreement 107/108 = 0.9907 [0.9494, 0.9984], chance 0.5123, kappa 0.981 [0.8962, 0.9966]; calibration error 0.0816, Brier 0.0547
  runs differ on 2/120; order changes the winner of 6/60 pairs; the first shown won both ways in 1, the second in 5; picks the longer in 32, truth 33
  -> passed (agrees)
judge 0 with one character of its rubric changed: identity 4b03c35b -> e4255283
grader judge 0 for an Elephant student and reflector: same_weights
grader judge 1 for an Elephant student and reflector: same_weights
grader judge 4 for an Elephant student and reflector: separate
grader judge 0 grading itself: same_identity
a passing grade from judge 0: unverifiable (shared)
a passing grade from judge 1: unverifiable (shared)
a passing grade from judge 2: unverifiable (shared)
a passing grade from judge 3: unverifiable (shared)
a passing grade from judge 4: passed (graded)
judge 4's grade of item 0 offered as a training row: opinion
judge output 1.5 on item 17: number, gate unverifiable (unmeasured)
```

**The judges are stand-ins.** Chapters 14 and 15 run Elephant and Mammoth
from their original weights, but slowly on the CPU, and no recorded set of
their judgments exists yet. Each judge here has the identity of the real
component: the model, the weights' digest (SHA-256 of the pinned origin and
revision from chapter 14's pins), the rubric and the settings. Its answers
come from a written-down behavior, described under
[The stand-in judge](#the-stand-in-judge). Every function that measures or
gates a judge reads only its probabilities, so recorded answers from the real
models can replace the stand-in's without changing them.

Read the run judge by judge:
- **Judge 0** agrees on 105 of 108, and kappa's interval starts at 0.839,
  above 0.6. Its biases and noise are inside the limits. It passes.
- **Judge 1** is judge 0 with one rubric line changed to ask for thoroughness.
  Its agreement is still high, kappa 0.79. But it picks the longer answer in
  46 of 60 pairs, where the longer answer is better in 33. A search graded by
  it would learn to write longer answers, so it fails on length, whatever its
  agreement.
- **Judge 2** asks "A or B?". The answer shown first wins both ways in 26
  pairs, the second never. It fails on position.
- **Judge 3** runs at temperature 1.0 with a terse rubric. Two runs on the
  same items disagree on 45 of 120. Kappa's interval ends below 0.6 as well,
  but the noise check comes first: more samples a judgment might fix it, so
  the verdict is unverifiable, not failed.
- **Judge 4** is Mammoth, averaging 3 calls a judgment. It agrees on 107 of
  108 and costs 18 times as much as judge 0. The cost sits beside the
  agreement, so the choice between them is visible.

Only judge 4 can grade this Elephant student, because judges 0 to 3 run on
the student's own weights.

## A judge is a component

<!-- source: psionic/lib/judge.coax identity -->
```jai
jg_component_ok :: (c:JgComponent)->bool {
    return c.rubric.count>=1 && c.rubric.count<=JG_RUBRIC_MAX &&
        c.temperature>=0 && c.temperature<=JG_TEMPERATURE_MAX && c.samples>=1 && c.samples<=JG_SAMPLES_MAX &&
        tb_in(c.price,0,JG_PRICE_MAX);
}
// Chapter 239's encoding under its own tag: the model, the weights' digest,
// the rubric's bytes and the settings. A changed byte anywhere is a new judge.
jg_identity :: (c:JgComponent)->[32]u8 #export_c {
    if !jg_component_ok(c) {return su_zero();}
    buf:[4224]u8;at:=su_text(buf[..],0,"psionic.component.v1");
    buf[at]=cast(u8)c.model;at+=1;
    for i:0..<32 {buf[at+i]=c.weights[i];}
    at=su_text(buf[..],at+32,c.rubric);
    at=su_u64(buf[..],at,cast(u64)c.temperature);at=su_u64(buf[..],at,cast(u64)c.samples);at=su_flag(buf[..],at,c.swap);
    return su_hash(buf[0..<at]);
}
// Calls per judgment times the price of one; a pair judged both ways is two.
jg_cost :: (c:JgComponent,pair:bool)->f64 #export_c {
    return c.price*f64(c.samples)*(ifx pair && c.swap then 2.0 else 1.0);
}
// The grader grades someone else's work: a different identity and different
// weights from the student and the reflector.
jg_separate :: (grader:JgComponent,student:JgComponent,reflector:JgComponent)->JgSeparation #export_c {
    if !jg_component_ok(grader) || !jg_component_ok(student) || !jg_component_ok(reflector) {return JgSeparation.shape;}
    g:=jg_identity(grader);
    if su_same(g,jg_identity(student)) || su_same(g,jg_identity(reflector)) {return JgSeparation.same_identity;}
    if su_same(grader.weights,student.weights) || su_same(grader.weights,reflector.weights) {return JgSeparation.same_weights;}
    return JgSeparation.separate;
}
```

The identity covers what changes a judgment: the model, its weights, the
rubric, the temperature, how many calls are averaged and whether pairs are
judged both ways. It leaves out the price, which changes what a judgment
costs, not what it says. The example changes one character of judge 0's
rubric, a period to an exclamation mark, and the identity changes from
`4b03c35b` to `e4255283`. A measurement recorded for one identity says
nothing about the other.

Separation is checked twice. The same identity as the student or the
reflector is `same_identity`. The same weights with a different rubric is
`same_weights`: a model told to grade is still grading its own kind of
answer. The student and the reflector may share weights; only the grader is
held apart.

## Truth comes from checks

<!-- source: psionic/lib/judge.coax agreement -->
```jai
// Only a label from a check or a diff is truth (chapter 247's rule).
jg_counted :: (o:TrOutcome)->bool {return o.from==TrLabel.check || o.from==TrLabel.diff;}
// The Wilson score interval for k agreements in n at z = 1.96.
jg_wilson :: (k:int,n:int)->[2]f64 {
    m:=f64(n);p:=f64(k)/m;z2:=JG_Z*JG_Z;d:=1+z2/m;
    c:=(p+z2/(2*m))/d;h:=JG_Z*sqrt(p*(1-p)/m+z2/(4*m*m))/d;
    // At 0 and at n the bounds are exactly 0 and 1; rounding is kept out.
    return [2]f64.[ifx k==0 then 0.0 else c-h,ifx k==n then 1.0 else c+h];
}
// The judge's probability of yes p against labelled outcomes; yes is p >= 0.5.
// Kappa's interval maps agreement's interval through the same formula, with
// chance agreement held at its measured value.
jg_agreement :: (p:[]f64,truth:[]TrOutcome)->JgAgreement #export_c {
    n:=p.count;
    if n<1 || n>JG_ITEMS_MAX || truth.count!=n {return JgAgreement.{status=JgStatus.shape};}
    for i:0..<n {
        if !tb_in(p[i],0,1) || truth[i].label<0 || truth[i].label>1 {return JgAgreement.{status=JgStatus.number};}
    }
    a:JgAgreement;y:[JG_ITEMS_MAX]f64;rows:[JG_ITEMS_MAX]int;
    for i:0..<n {
        if !jg_counted(truth[i]) {a.skipped+=1;continue;}
        yes:=p[i]>=0.5;t:=truth[i].label==1;
        y[i]=f64(truth[i].label);rows[a.items]=i;a.items+=1;
        if yes {a.judge_yes+=1;}
        if t {a.truth_yes+=1;}
        if yes==t {a.agree+=1;}
    }
    if a.items==0 {return a;}
    m:=f64(a.items);w:=jg_wilson(a.agree,a.items);
    a.observed=f64(a.agree)/m;a.low=w[0];a.high=w[1];
    a.chance=(f64(a.judge_yes)*f64(a.truth_yes)+f64(a.items-a.judge_yes)*f64(a.items-a.truth_yes))/(m*m);
    if a.chance<1 {
        a.kappa=(a.observed-a.chance)/(1-a.chance);
        a.kappa_low=(a.low-a.chance)/(1-a.chance);a.kappa_high=(a.high-a.chance)/(1-a.chance);
    }
    r:=cf_measure(p,y[0..<n],rows[0..<a.items]);a.ece=r.ece;a.brier=r.brier;
    return a;
}
```

A judge is measured only against outcomes that chapter 247 would admit as
labels: those from a check, such as a test that passed, or a diff, such as a
change that was merged. In the example, 12 of the 120 outcomes were labelled
by the agent's own summary. They are counted as skipped, not as truth, so
every judge is measured on 108.

The interval on kappa holds chance agreement fixed at its measured value.
This is an approximation: chance agreement also varies from sample to sample.
It is stated in the profile, and it keeps the interval something a reader can
compute by hand, as in the section above.

## Noise and two biases

<!-- source: psionic/lib/judge.coax probe -->
```jai
jg_unit :: (v:[]f64)->bool {for x:v {if !tb_in(x,0,1) {return false;}}return true;}
jg_side :: (v:[]int)->bool {for x:v {if x!=0 && x!=1 {return false;}}return true;}
// first and second: two runs of the same judge on the same items. q: the
// probability that the answer shown first is better, shown (a, b); r: the
// same, shown (b, a). longer and better name a (0) or b (1).
jg_probe :: (first:[]f64,second:[]f64,q:[]f64,r:[]f64,longer:[]int,better:[]int)->JgProbe #export_c {
    n:=first.count;m:=q.count;
    if n<1 || n>JG_ITEMS_MAX || second.count!=n || m<1 || m>JG_ITEMS_MAX || r.count!=m || longer.count!=m || better.count!=m {
        return JgProbe.{status=JgStatus.shape};
    }
    if !jg_unit(first) || !jg_unit(second) || !jg_unit(q) || !jg_unit(r) || !jg_side(longer) || !jg_side(better) {
        return JgProbe.{status=JgStatus.number};
    }
    p:=JgProbe.{items=n,pairs=m};
    for i:0..<n {if (first[i]>=0.5)!=(second[i]>=0.5) {p.flips+=1;}}
    for k:0..<m {
        // Shown first, a wins at q >= 0.5; shown second, a wins when b loses.
        if (q[k]>=0.5)!=(r[k]<0.5) {p.inconsistent+=1;}
        if q[k]>=0.5 && r[k]>=0.5 {p.first+=1;}
        if q[k]<0.5 && r[k]<0.5 {p.second+=1;}
        pick:=ifx q[k]+(1-r[k])>=1 then 0 else 1;
        if pick==longer[k] {p.judge_longer+=1;}
        if better[k]==longer[k] {p.truth_longer+=1;}
    }
    p.flip_rate=f64(p.flips)/f64(n);p.position=f64(p.first-p.second)/f64(m);
    p.length_bias=f64(p.judge_longer-p.truth_longer)/f64(m);
    return p;
}
```

Three measurements need no labels at all, only more calls:
- **Noise.** The judge runs twice on each item. The flip rate is the share of
  items where the two runs give different answers. It is the judge's own
  noise floor: a difference between two candidates smaller than this is not
  something the judge can see.
- **Position.** Each pair is shown both ways. When the winner changes with
  the order, the pair is inconsistent. Noise alone makes some pairs
  inconsistent, evenly in both directions. A bias shows as a lean: the answer
  shown first winning both ways more often than the answer shown second. The
  lean is (first − second) / pairs.
- **Length.** From both orders, the judge's pick is the answer with the
  higher average. The length bias is how much more often it picks the longer
  answer than the labels say the longer answer is better.

On a small fixture from the checks: q = (0.9, 0.7, 0.3, 0.6) and
r = (0.2, 0.6, 0.8, 0.45). Pair 1 has q = 0.7 and r = 0.6, so the answer shown
first wins both ways; the other three are consistent. The lean is
(1 − 0) / 4 = 0.25. The picks are a (0.9 + 0.8 = 1.7), a (1.1), b (0.5) and
a (1.15). With the longer answers a, a, b, b, the judge picks the longer in
3 pairs; with the better answers a, b, b, a, the longer is better in 2. The
length bias is (3 − 2) / 4 = 0.25.

## The gate

<!-- source: psionic/lib/judge.coax gate -->
```jai
jg :: (v:VdVerdict,why:JgReason)->JgResult {return JgResult.{verdict=v,reason=why};}
jg_policy_ok :: (y:JgPolicy)->bool {
    return y.items_min>=1 && tb_in(y.kappa_min,-1,1) && tb_in(y.flip_max,0,1) && tb_in(y.position_max,0,1) && tb_in(y.length_max,0,1);
}
// Enough labelled items in two classes first; then a bias fails the judge,
// noise between runs leaves it unverifiable, and kappa's interval decides.
jg_gate :: (a:JgAgreement,p:JgProbe,y:JgPolicy)->JgResult #export_c {
    if a.status!=JgStatus.ok || p.status!=JgStatus.ok || !jg_policy_ok(y) {return jg(VdVerdict.unverifiable,JgReason.unmeasured);}
    if a.items<y.items_min {return jg(VdVerdict.unverifiable,JgReason.few_items);}
    if a.chance>=1 {return jg(VdVerdict.unverifiable,JgReason.one_class);}
    if abs(p.position)>y.position_max {return jg(VdVerdict.failed,JgReason.position);}
    if abs(p.length_bias)>y.length_max {return jg(VdVerdict.failed,JgReason.length);}
    if p.flip_rate>y.flip_max {return jg(VdVerdict.unverifiable,JgReason.noisy);}
    if a.kappa_low>=y.kappa_min {return jg(VdVerdict.passed,JgReason.agrees);}
    if a.kappa_high<y.kappa_min {return jg(VdVerdict.failed,JgReason.disagrees);}
    return jg(VdVerdict.unverifiable,JgReason.unclear);
}
// A grade counts only from a separate judge that passed its gate.
jg_grade :: (s:JgSeparation,judge:JgResult,grade:VdResult)->JgResult #export_c {
    if s!=JgSeparation.separate {return jg(VdVerdict.unverifiable,JgReason.shared);}
    if judge.verdict!=VdVerdict.passed {return jg(VdVerdict.unverifiable,JgReason.unproven);}
    return jg(grade.verdict,JgReason.graded);
}
// A grade offered as a row carries its source, so chapter 247 refuses it.
jg_opinion :: (group:int,time:int,decision:int,p:f64)->TrOutcome #export_c {
    return TrOutcome.{group=group,time=time,decision=decision,label=ifx p>=0.5 then 1 else 0,from=TrLabel.judge};
}
```

The limits are written down before any judge is measured:

| Limit | Value |
| --- | ---: |
| labelled items, at least | 30 |
| kappa's lower bound, at least | 0.6 |
| flip rate between runs, at most | 0.1 |
| position lean, at most (either way) | 0.1 |
| length bias, at most (either way) | 0.15 |

The order matters. Too few labelled items, or labels all of one class, leave
nothing to measure. A bias fails the judge even when it agrees, because a
search optimizes against the bias and agreement on held-out items won't show
it. Noise leaves it unverifiable, since averaging more calls can reduce noise.
Only then does kappa's interval decide, exactly as chapter 241's decision gate
does with a noise floor: clearly above the line passes, clearly below fails,
and straddling the line is unverifiable.

A grade from a judge counts only when two things hold: the judge is separate
from the student and the reflector, and it passed its own gate. Otherwise the
grade is unverifiable, with `shared` or `unproven` as the reason. A failed
judge's grade is unverifiable too: its verdict on the candidate says nothing
either way.

## Opinions are not labels

`jg_opinion` turns a judge's grade into the record chapter 247 admits rows
from, with its label source set to `judge`. Chapter 247 now refuses that
source as `opinion`, as it refuses an agent's own summary. A judge can rank
candidates inside a search. Its grades never become training rows, because a
model trained on them learns the judge's mistakes as if they were facts.

## The stand-in judge

<!-- source: psionic/lib/judge.coax standin -->
```jai
// stand-in-judge-v1. Not a model: a judge whose behavior is written down.
// skill pulls toward the truth, length toward longer answers, position
// toward the answer shown first, and temperature spreads each call; the
// draws are keyed by the judge's identity, so a new rubric draws anew.
JgBehavior :: struct {skill:f64;length:f64;position:f64;}
JG_PAIR_BASE :: 4096;
jg_seed :: (d:[32]u8)->u64 {s:u64=0;for i:0..<8 {s|=cast(u64)d[i]<<cast(u64)(8*i);}return s;}
jg_call :: (c:JgComponent,seed:u64,z:f64,slot:int,run:int)->f64 {
    total:=0.0;
    for k:0..<c.samples {
        u:=tb_uniform(seed,cast(u64)((slot*JG_SAMPLES_MAX+k)*2+run));
        total+=tb_sigmoid(z+f64(c.temperature)/1000*(8*u-4));
    }
    return total/f64(c.samples);
}
// One item: truth 0 or 1, how clearly the answer shows it in [0, 1], and
// length in [-1, 1].
jg_point :: (c:JgComponent,seed:u64,b:JgBehavior,truth:int,clarity:f64,length:f64,item:int,run:int)->f64 {
    return jg_call(c,seed,b.skill*f64(2*truth-1)*clarity+b.length*length,item,run);
}
// One pair as shown: quality and length of the first less the second's.
jg_pair :: (c:JgComponent,seed:u64,b:JgBehavior,quality:f64,length:f64,pair:int,order:int)->f64 {
    return jg_call(c,seed,b.skill*quality+b.length*length+b.position,JG_PAIR_BASE+pair,order);
}
```

The stand-in answers with a logistic of a score. Skill pulls the score toward
the truth, more strongly on items whose answer shows it clearly. A length
weight pulls it toward longer answers and a position weight toward the
answer shown first. Temperature adds a uniform spread to each call, and the
draws are keyed by the first 8 bytes of the judge's identity, so editing the
rubric changes the draws. The five judges' weights, written down:

| Judge | Skill | Length | Position | Temperature | Samples |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0, Elephant, rubric v1 | 6 | 0 | 0 | 0.3 | 1 |
| 1, Elephant, rubric v2 | 6 | 3 | 0 | 0.3 | 1 |
| 2, Elephant, A or B | 6 | 0 | 1 | 0.3 | 1 |
| 3, Elephant, terse | 3 | 0 | 0 | 1.0 | 1 |
| 4, Mammoth, rubric v1 | 8 | 0 | 0 | 0.3 | 3 |

The world is also written down. Of the 120 outcomes, 9 in 20 pass, by a
counter-based draw. Every tenth is labelled by a summary, and the rest by a
check (even) or a diff (odd). In each pair, which answer is better and which
is longer are drawn independently, so length says nothing about quality.

## A broken input

The example's last line hands `jg_agreement` judgments with a probability
of 1.5 on item 17. It refuses them with status `number`, and the gate,
handed the refused measure, says unverifiable (`unmeasured`) instead of
passing a judge on numbers it never checked:

```text
judge output 1.5 on item 17: number, gate unverifiable (unmeasured)
```

The checks refuse each malformed input the same way: probabilities outside
[0, 1] or not a number, labels other than 0 and 1, sides other than a or b,
mismatched lengths, more than 4,096 items or pairs, an empty rubric or one
over 4,096 bytes, settings out of range, and a policy whose limits aren't
numbers in range.

## Supported profile

`judge-v1`:
- binary64;
- Wilson intervals at z = 1.96, with kappa's interval holding chance
  agreement at its measured value;
- up to 4,096 items and 4,096 pairs a measurement;
- identities under the tag `psionic.component.v1` in chapter 239's encoding,
  with rubrics up to 4,096 bytes, temperature up to 2.0 in thousandths and up
  to 16 samples a judgment;
- chapter 215's calibration error and Brier score;
- labels admitted by chapter 247's rule (check or diff);
- stand-in-judge-v1 on chapter 103's counter-based draws for the example.

## How it is checked

- **Coax checks:** the chapter's 15 checks, among the 235 that run in debug
  and release with the units `judge.coax` loads. They cover:
  - the identity against a digest computed with Python's `hashlib`, a change
    in each field changing it, and the price not changing it;
  - judge 0's weights digest and identity in the example, also from
    `hashlib`;
  - the 20-item kappa, Wilson interval, calibration error and Brier score
    above, by hand;
  - a `#forall` over every 2×2 table with up to 4 items in each cell,
    comparing count, agreement and kappa with an independent textbook
    recount ([judge-checks.coax](../tests/judge-checks.coax)) and checking
    that each interval holds its value;
  - the probe on the fixture above, and ties at exactly 0.5;
  - every gate reason at its boundary, and a `#forall` that the gate passes
    only an agreeing, unbiased, quiet judge and fails only on a bias or a
    clear disagreement;
  - separation, grades and the opinion refusal;
  - the stand-in's formula by hand, and the example's counts.
- **Mutation testing** of `judge.coax`: 429 of 429 mutants in the file
  killed. The first run left 22 survivors: a cost check whose numbers gave the
  same answer for a sum and a product, missing ties at exactly 0.5, and
  comparisons in the world's draws where a tie has probability zero. The
  world now draws which answer is better and which is longer directly, and
  the checks cover the ties.
- **Chapter 247's unit** gains the `judge` label source and the `opinion`
  refusal, with its own check; a mutation run over its own file kills 230 of
  230.

These checks ran with the cached `coax` compiler.

## What this does not do yet

- **Elephant and Mammoth as judges.** The judges are stand-ins. Running the
  real models needs recorded judgments from chapters 14 and 15, or the GPU
  path; the measuring and gating code takes them unchanged.
- **Real outcomes.** The outcomes are synthetic. Chapter 217's joins are
  where held-out labelled outcomes come from.
- **Routers.** A router choosing a model per task is chapter 225's; it is not
  measured here as a judge.
- **A kappa interval that varies chance agreement.** A bootstrap over items
  would; this chapter keeps the hand-computable approximation.

## Sources

- [Chapter plan](PLAN.md), Part XXIV.
- [Measure, then improve](../docs/self-improvement.md).
- Cohen, "A coefficient of agreement for nominal scales", 1960.
- Wilson, "Probable inference, the law of succession, and statistical
  inference", 1927.
