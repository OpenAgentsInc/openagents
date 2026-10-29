# 157. Thresholds from written costs

[Chapter 12](ch012-are-the-probabilities-any-good.md) measured whether
Elephant's probabilities are any good. A probability still isn't a decision.
Whether 0.7 is enough to act on depends on what a wrong action costs, what
waiting costs and what the alternatives cost. Hand-tuned thresholds hide those
costs. This chapter writes the costs down and derives the thresholds from them.

The project is a cascade. Elephant answers first, and for each request the
consumer can:
- act on Elephant's answer, yes or no;
- escalate to Mammoth, which is slower and costs more but knows more;
- ask a person.

Two consumers read the same probabilities and, because their costs differ,
rationally choose differently.

```shell
coax/bin/coax run psionic/examples/cost-thresholds.coax
psionic/check.sh --only threshold
```

```jai
by hand, background classification at p = 0.5: no 0.5, yes 2.5, ask 0.5, escalate 0.3112
validation, 2000 items: Mammoth's evidence points the right way on 0.9005 of them
threshold records, version 1, rho 0.9005:
  voice interruption: no below 0.25, yes from there
  background classification: no below 0.1219, escalate below 0.9851, yes from there
confirmation, 4000 items; the records match expected costs on 8000 of 8000 decisions
voice interruption: cost per decision, with a 95 percent interval
  always Elephant: 0.6548 ± 0.0271; errors 705 of 4000 answered (0.1763), escalated 0, asked 0, 0.15 s
  always Mammoth: 2.553 ± 0.0179; errors 292 of 4000 answered (0.073), escalated 0, asked 0, 1.2 s
  fixed rule: 1.3034 ± 0.0405; errors 348 of 4000 answered (0.087), escalated 1384, asked 0, 0.57 s
  record: 0.5815 ± 0.0201; errors 834 of 4000 answered (0.2085), escalated 0, asked 0, 0.15 s
  record minus always Elephant: -0.0732 ± 0.0231
background classification: cost per decision, with a 95 percent interval
  always Elephant: 0.5259 ± 0.0438; errors 705 of 4000 answered (0.1763), escalated 0, asked 0, 0.15 s
  always Mammoth: 0.2362 ± 0.03; errors 292 of 4000 answered (0.073), escalated 0, asked 0, 1.2 s
  fixed rule: 0.2755 ± 0.0328; errors 348 of 4000 answered (0.087), escalated 1384, asked 0, 0.57 s
  record: 0.1587 ± 0.0174; errors 197 of 3541 answered (0.0556), escalated 2999, asked 459, 35.47 s
  record minus always Mammoth: -0.0775 ± 0.0236
the same probabilities, two consumers: they act differently on 2999 of 4000 items
```

The [example](../examples/cost-thresholds.coax) runs in about two seconds.

## Two tables of consequences

Each consumer writes one table. All costs are in one unit, call it dollars:

| | Voice interruption | Background classification |
| --- | --- | --- |
| Question | Did the person start talking, so the assistant should stop? | Does this item belong in the class? |
| Wrong yes | 1 (stops for nothing) | 5 (a wrong label acted on later) |
| Wrong no | 3 (talks over the person) | 1 (a missed item) |
| Ask a person | 0.8, and 1 s | 0.2, and 300 s in a review queue |
| Each second of waiting | 2 | 0.001 |
| Elephant | 0.0005, 0.15 s | the same |
| Mammoth | 0.01, 1.2 s | the same |

The voice table makes waiting expensive: a second of silence in a conversation
costs more than most mistakes. The background table barely charges for time
but makes a wrong yes costly.

## The expected cost of answering

If Elephant's probability of yes is p and it is calibrated, answering no is
wrong with probability p and answering yes with probability 1 − p. Asking
costs the same whatever the answer:

```jai
no   p · wrong_no
yes  (1 − p) · wrong_yes
ask  ask + per_second · person_seconds
```

<!-- source: psionic/lib/threshold.coax answer -->
```jai
// The expected cost of answering at probability p of yes: no is wrong with
// probability p, yes with 1 - p, and asking costs the same either way.
// The cheapest wins; a tie keeps the earlier of no, yes, ask.
th_answer :: (c:ThCosts,p:f64)->ThChoice {
    best:=ThChoice.{action=ThAction.no,cost=p*c.wrong_no};
    yes:=(1-p)*c.wrong_yes;if yes<best.cost {best=ThChoice.{action=ThAction.yes,cost=yes};}
    ask:=c.ask+c.per_second*c.person_seconds;if ask<best.cost {best=ThChoice.{action=ThAction.ask,cost=ask};}
    return best;
}
```

For the voice table, no costs 3p and yes costs 1 − p. They are equal at p =
0.25, so the assistant should stop talking from 0.25 on, not from 0.5.
Talking over someone costs three times as much as stopping for nothing.
Answering never costs more than 0.75, so asking (0.8 + 2 × 1 = 2.8) never
wins.

## The expected cost of escalating

Mammoth's evidence points the right way with probability ρ. Bayes' rule says
where it moves p: up to pρ / (pρ + (1 − p)(1 − ρ)) or down to p(1 − ρ) / (p(1 −
ρ) + (1 − p)ρ). The consumer then answers at Mammoth's probability, by the same
costs. So escalating costs Mammoth's price, plus the expected cost of the
answer after each reading, weighted by how likely that reading is:

<!-- source: psionic/lib/threshold.coax escalate -->
```jai
// Mammoth's evidence points the right way with probability rho. Its answer
// moves p by Bayes' rule, and the consumer then answers at Mammoth's p.
th_posterior :: (p:f64,rho:f64,up:bool)->f64 {
    if up {return p*rho/(p*rho+(1-p)*(1-rho));}
    return p*(1-rho)/(p*(1-rho)+(1-p)*rho);
}
// The expected cost of escalating: Mammoth's call and wait, then the answer
// at each of its two readings, weighted by how likely that reading is.
th_escalate :: (c:ThCosts,p:f64,rho:f64)->f64 {
    up:=p*rho+(1-p)*(1-rho);
    after:=up*th_answer(c,th_posterior(p,rho,true)).cost+(1-up)*th_answer(c,th_posterior(p,rho,false)).cost;
    return c.mammoth_dollars+c.per_second*c.mammoth_seconds+after;
}
// Escalate only when it is strictly cheaper than the best answer now.
th_best :: (c:ThCosts,p:f64,rho:f64)->ThChoice #export_c {
    now:=th_answer(c,p);e:=th_escalate(c,p,rho);
    if e<now.cost {return ThChoice.{action=ThAction.escalate,cost=e};}
    return now;
}
```

By hand, for the background table at p = 0.5 with ρ = 0.9:
- Answering no costs 0.5 × 1 = 0.5, yes costs 0.5 × 5 = 2.5, and asking
  costs 0.2 + 0.001 × 300 = 0.5.
- Mammoth reads up half the time, to 0.9. There, yes costs 0.1 × 5 = 0.5, the
  same as asking, and the tie keeps yes.
- Mammoth reads down half the time, to 0.1. There, no costs 0.1.
- Escalating therefore costs 0.01 + 0.001 × 1.2 + (0.5 × 0.5 + 0.5 × 0.1) =
  0.3112, less than any answer now.

For the voice table, Mammoth's 1.2 s alone cost 2.4, more than any answer.
The voice consumer never escalates.

## A threshold record

The cheapest action changes at a few probabilities. Those probabilities are
the thresholds, and they are read off the expected costs rather than chosen:

<!-- source: psionic/lib/threshold.coax record -->
```jai
// The bands, read off th_best. At p = 0 answering no costs nothing, so the
// first band is always no (the zero action). Probabilities k/4096 are scanned;
// at each change, halving between the last probability with the old action and
// the first with the new one, until the two are adjacent numbers, finds the
// first probability where the old action stops.
th_record :: (c:ThCosts,rho:f64)->ThRecord #export_c {
    if !th_costs_ok(c) || !(rho>0.5 && rho<1) {return ThRecord.{status=ThStatus.shape};}
    r:=ThRecord.{version=TH_VERSION,rho=rho,bands=1};last:=0.0;
    for k:1..<TH_GRID+1 {
        p:=f64(k)/TH_GRID;a:=th_best(c,p,rho).action;old:=r.action[r.bands-1];
        if a!=old {
            if r.bands==TH_BANDS_MAX {return ThRecord.{status=ThStatus.capacity};}
            lo:=last;hi:=p;
            while true {
                mid:=(lo+hi)/2;
                if mid==lo || mid==hi {break;}
                if th_best(c,mid,rho).action==old {lo=mid;} else {hi=mid;}
            }
            r.edge[r.bands]=hi;r.action[r.bands]=a;r.bands+=1;
        }
        last=p;
    }
    return r;
}
// The action of the last band whose edge is at most p.
th_decide :: (r:ThRecord,p:f64)->ThAction #export_c {
    k:=0;
    for b:1..<r.bands {if r.edge[b]<=p {k=b;}}
    return r.action[k];
}
```

A record carries a version, the ρ it was derived with, and up to four bands.
The scan steps 1/4096 at a time. Each change is then located exactly, as the
first double at which the old action stops being cheapest. A band narrower than
one step could be missed, so the example also checks the record against the
expected costs at every confirmation probability. A table that needs five bands
(escalating on both sides of a band of asking) is refused as `capacity`.

The background record escalates from 0.1219 to 0.9851. Asking never appears
as a band, although at p = 0.6 it is cheaper than answering: escalating is
cheaper still. A person is asked after Mammoth, when Mammoth's probability lands
where asking beats both answers.

## Validation first, confirmation once

The data are synthetic and calibrated by construction:
- Elephant's probability comes from a logit uniform in [−4, 4].
- The label is yes with exactly that probability.
- Mammoth's evidence is right with probability 0.9, and Mammoth reports
  Bayes' rule.

<!-- source: psionic/lib/threshold.coax simulate -->
```jai
// threshold-sim-v1. Item i: Elephant's probability from a logit uniform in
// [-4, 4] at counter 3i; the label, yes with that probability, at 3i+1;
// Mammoth's evidence, right with probability rho, at 3i+2. A draw u gives
// "yes" when u + p reaches 1, which happens with probability p, and Mammoth
// is wrong when u + (1 - rho) reaches 1. Mammoth reports Bayes' rule.
th_simulate :: (seed:u64,rho:f64,elephant:[]f64,mammoth:[]f64,label:[]f64,overlap:bool)->ThStatus #export_c {
    n:=elephant.count;
    if n<1 || n>TH_ITEMS_MAX || !(rho>0.5 && rho<1) {return ThStatus.shape;}
    if mammoth.count<n || label.count<n {return ThStatus.capacity;}
    if overlap {return ThStatus.alias;}
    for i:0..<n {
        c:=cast(u64)(3*i);p:=tb_sigmoid(8*tb_uniform(seed,c)-4);
        y:=floor(tb_uniform(seed,c+1)+p);wrong:=floor(tb_uniform(seed,c+2)+(1-rho));
        elephant[i]=p;label[i]=y;mammoth[i]=th_posterior(p,rho,y!=wrong);
    }
    return ThStatus.ok;
}
// Mammoth's evidence pointed up when its probability exceeds Elephant's;
// rho is estimated as the share of items where that direction was right.
th_rho :: (elephant:[]f64,mammoth:[]f64,label:[]f64)->ThValue #export_c {
    n:=elephant.count;
    if n<1 || n>TH_ITEMS_MAX || mammoth.count!=n || label.count!=n {return ThValue.{status=ThStatus.shape};}
    if !th_unit(elephant) || !th_unit(mammoth) {return ThValue.{status=ThStatus.number};}
    for y:label {if y!=0 && y!=1 {return ThValue.{status=ThStatus.number};}}
    right:=0;
    for i:0..<n {if (mammoth[i]>elephant[i])==(label[i]==1) {right+=1;}}
    return ThValue.{value=f64(right)/f64(n)};
}
```

The consumer doesn't know ρ. It measures it on 2,000 validation items as the
share where Mammoth moved p in the direction of the label, and gets 0.9005. The
records are derived with that ρ and frozen. Only then are the 4,000
confirmation items drawn and scored, once:

<!-- source: psionic/lib/threshold.coax run -->
```jai
// The cost of every decision under one policy, and its mean with a 95 percent
// normal interval. Elephant, Mammoth and the fixed rule answer by the most
// likely label (yes from 0.5); the fixed rule escalates when 0.2 < p < 0.8.
// The record decides by its bands, and after escalating answers by costs.
ThItem :: struct {cost:f64;seconds:f64;wrong:bool;asked:bool;escalated:bool;}
th_item :: (c:ThCosts,policy:ThPolicy,r:ThRecord,p:f64,q:f64,y:f64)->ThItem {
    t:=ThItem.{seconds=c.elephant_seconds};dollars:=c.elephant_dollars;a:=ThAction.no;
    if p>=0.5 {a=ThAction.yes;}
    if policy==ThPolicy.mammoth {t.seconds=0;dollars=0;t.escalated=true;}
    if policy==ThPolicy.fixed && p>0.2 && p<0.8 {t.escalated=true;}
    if policy==ThPolicy.record {a=th_decide(r,p);t.escalated=a==ThAction.escalate;}
    if t.escalated {
        t.seconds+=c.mammoth_seconds;dollars+=c.mammoth_dollars;
        a=ThAction.no;if q>=0.5 {a=ThAction.yes;}
        if policy==ThPolicy.record {a=th_answer(c,q).action;}
    }
    if a==ThAction.ask {t.seconds+=c.person_seconds;dollars+=c.ask;t.asked=true;}
    if a==ThAction.yes && y==0 {dollars+=c.wrong_yes;t.wrong=true;}
    if a==ThAction.no && y==1 {dollars+=c.wrong_no;t.wrong=true;}
    t.cost=dollars+c.per_second*t.seconds;
    return t;
}
th_run :: (c:ThCosts,policy:ThPolicy,r:ThRecord,elephant:[]f64,mammoth:[]f64,label:[]f64,cost:[]f64,overlap:bool)->ThReport #export_c {
    n:=elephant.count;
    if n<2 || n>TH_ITEMS_MAX || mammoth.count!=n || label.count!=n || !th_costs_ok(c) {return ThReport.{status=ThStatus.shape};}
    if policy==ThPolicy.record && (r.status!=ThStatus.ok || r.version!=TH_VERSION) {return ThReport.{status=ThStatus.shape};}
    if cost.count<n {return ThReport.{status=ThStatus.capacity};}
    if overlap {return ThReport.{status=ThStatus.alias};}
    if !th_unit(elephant) || !th_unit(mammoth) {return ThReport.{status=ThStatus.number};}
    for y:label {if y!=0 && y!=1 {return ThReport.{status=ThStatus.number};}}
    report:ThReport;seconds:PsionicAccumulator;
    for i:0..<n {
        t:=th_item(c,policy,r,elephant[i],mammoth[i],label[i]);cost[i]=t.cost;seconds=psionic_ops_accumulate(seconds,t.seconds);
        if t.wrong {report.errors+=1;}
        if t.asked {report.asked+=1;}
        // Always-Mammoth calls Mammoth directly; it is not counted as escalating.
        if t.escalated && policy!=ThPolicy.mammoth {report.escalated+=1;}
    }
    v:=th_interval(cost[0..<n]);
    report.mean=v.value;report.half=v.half;report.seconds=psionic_ops_total(seconds)/f64(n);
    return report;
}
// Mean and 1.96 standard errors, with the n - 1 sample variance.
th_interval :: (x:[]f64)->ThValue {
    s:PsionicAccumulator;for v:x {s=psionic_ops_accumulate(s,v);}
    mean:=psionic_ops_total(s)/f64(x.count);
    d:PsionicAccumulator;for v:x {e:=v-mean;d=psionic_ops_accumulate(d,e*e);}
    return ThValue.{value=mean,half=1.96*sqrt(psionic_ops_total(d)/f64(x.count-1)/f64(x.count))};
}
// A paired comparison: the mean of a - b per item, with its interval.
th_paired :: (a:[]f64,b:[]f64,scratch:[]f64,overlap:bool)->ThValue #export_c {
    n:=a.count;
    if n<2 || n>TH_ITEMS_MAX || b.count!=n {return ThValue.{status=ThStatus.shape};}
    if scratch.count<n {return ThValue.{status=ThStatus.capacity};}
    if overlap {return ThValue.{status=ThStatus.alias};}
    for i:0..<n {
        if !tb_in(a[i],-1e12,1e12) || !tb_in(b[i],-1e12,1e12) {return ThValue.{status=ThStatus.number};}
        scratch[i]=a[i]-b[i];
    }
    return th_interval(scratch[0..<n]);
}
```

Every policy pays for its own calls, waits and mistakes. Always Elephant, always
Mammoth and the fixed rule answer by the more likely label. The fixed rule
escalates when 0.2 < p < 0.8. The record decides by its bands, and after
escalating it answers by costs.

| Voice interruption | Cost per decision | Errors (rate) | Escalated | Seconds |
| --- | --- | --- | --- | --- |
| Always Elephant | 0.6548 ± 0.0271 | 705 (0.176) | 0 | 0.15 |
| Always Mammoth | 2.5530 ± 0.0179 | 292 (0.073) | — | 1.2 |
| Fixed rule | 1.3034 ± 0.0405 | 348 (0.087) | 1,384 | 0.57 |
| Record | 0.5815 ± 0.0201 | 834 (0.209) | 0 | 0.15 |

| Background classification | Cost per decision | Errors of answered (rate) | Escalated, asked | Seconds |
| --- | --- | --- | --- | --- |
| Always Elephant | 0.5259 ± 0.0438 | 705 of 4,000 (0.176) | 0, 0 | 0.15 |
| Always Mammoth | 0.2362 ± 0.0300 | 292 of 4,000 (0.073) | —, 0 | 1.2 |
| Fixed rule | 0.2755 ± 0.0328 | 348 of 4,000 (0.087) | 1,384, 0 | 0.57 |
| Record | 0.1587 ± 0.0174 | 197 of 3,541 (0.056) | 2,999, 459 | 35.47 |

The intervals are 1.96 standard errors of the mean cost. Each record is also
compared, item by item, with the best other policy for its consumer: −0.0732 ±
0.0231 against always Elephant for voice, and −0.0775 ± 0.0236 against always
Mammoth for background. Neither interval includes zero.

The voice record makes more mistakes than always Elephant (834 against 705)
and is still cheaper. It trades talking over a person for stopping when nobody
spoke, at a third of the cost. A lower error rate was never the goal. The
background record answers fewer items (3,541 of 4,000), with the lowest error
rate of any policy. It waits for Mammoth and sometimes for a person, which
costs that consumer almost nothing. On 2,999 of the 4,000 items the two
consumers act differently on the same probability.

## Supported profile

`cpu-f64-threshold-v1`:
- binary64, with chapter 3's compensated accumulator for every sum;
- one yes-or-no question with four actions (no, yes, ask, escalate);
- costs within [0, 10⁶] and ρ in (0.5, 1);
- 1 to 8,192 items, with 2 to 8,192 for scoring, probabilities within
  [0, 1] and labels of exactly 0 or 1;
- records of version 1, at most four bands, with each edge located exactly
  after a scan in steps of 1/4096;
- intervals by the normal approximation.

`th_run` refuses a record that failed or has another version. Every exported
procedure that writes caller storage takes an `overlap` observation, which the
native adapter computes from real addresses.

## How it is checked

- **Coax checks:** 69 checks in debug and release, counting the units it
  loads. They cover:
  - the written tables;
  - every cost bound;
  - answering, Bayes' rule and escalation on a table of binary fractions,
    worked by hand with exact results, including ties;
  - records for both consumers, whose edges must be the first number with the
    new action (the number below has the old one);
  - a band narrower than the scan's first two steps, a first edge below
    1/4096, a four-band record, and the five-band refusal;
  - the simulator at its counters, with label and Mammoth frequencies within
    four standard deviations over 8,192 items;
  - every policy's cost per item by hand, and a run on a hand-made record;
  - intervals, paired differences and every refusal at its bound.
- **An independent oracle,** [threshold_oracle.py](../tests/threshold_oracle.py),
  written from the specification. It finds each edge by a different search,
  over the integer bit patterns of doubles, and runs the whole experiment with
  exactly rounded sums.
- **The native harness** [threshold.c](../tests/threshold.c) runs the same
  experiment through the span adapter, plain and under ASan/UBSan, in debug
  and release. Tolerances were frozen before comparison:
  - ρ, every edge, action, decision and tally must be equal;
  - means, intervals and seconds within 10⁻¹².

  All were equal. The harness also checks that NaN, infinity and overlapping
  storage are refused.
- **Mutation testing** of `threshold.coax` kills all 1,217 mutants. The
  first runs left survivors, each fixed in the code or the checks:
  - a first band that is always no, at p = 0 answering no costs nothing, so
    its assignment was dropped;
  - a fixed number of halvings, replaced by halving until the bounds are
    adjacent;
  - a variance written so that `+` and `−` gave the same sum;
  - missing checks for the scan's first and last steps and for exact counts.

The [verification record](../../docs/verification/psionic-ch157-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **Synthetic, calibrated probabilities.** Elephant's real probabilities need
  [chapter 156](PLAN.md)'s calibration before costs can be trusted with them,
  and Mammoth's real evidence is not a single right-or-wrong reading.
- **One validation draw** and normal intervals. No interval on ρ carries into
  the record.
- **No live traffic.** Chapter 158 records each decision, its band and its
  later outcome, so the costs can be checked against what happened.
- **One question with two answers.** Multi-way choices and scores need a cost
  per pair of answers.
