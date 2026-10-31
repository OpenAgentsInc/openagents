# 241. Fit settings on recorded answers

Before anyone rewrites a prompt or trains a model, there is a cheaper thing to
try. Most decision layers have settings: a threshold, a cut point, a weight.
They are usually written by hand once and never revisited. The answers the
model already gave are recorded, and so are the outcomes. Fitting the settings
to those records takes no model calls at all.

The [notes on self-improvement](../docs/self-improvement.md) record why this
chapter comes first. Ten earlier attempts at prompt optimization produced no
measured win. Fitting settings on recorded answers raised stall recall from
35% to 53% without changing a single policy.

This chapter fits a stall detector's three settings:
- **candidates** at the midpoints where a decision changes;
- a **coordinate search** priced by written costs;
- a move kept only when it is **strictly cheaper** on the recorded answers,
  and, as an option, on grouped cross-validation as well.

It then reads a locked held-out set once and reports through
[chapter 239](ch239-gates-that-can-say-unverifiable.md)'s gates.

```shell
coax/bin/coax run psionic/examples/stall-settings.coax
psionic/check.sh --only settings
```

```jai
recorded answers: 40 runs, 400 steps, 125 stalled; a missed stall costs 4, a false alarm 1
  hand-written: weight 0.0, threshold 0.5, quiet 700.0 s: cost 351, caught 39 of 125, 7 false alarms
fit with cross-validation over 5 folds of runs:
  pass 0, weight 0.0 -> 0.6139: cost 351 -> 124 on the recorded answers, 128 on the held-out folds: accepted
  pass 0, threshold 0.5: no cheaper value
  pass 0, quiet 700.0 -> 586.1724: cost 124 -> 115 on the recorded answers, 119 on the held-out folds: accepted
  pass 1, weight 0.6139: no cheaper value
  pass 1, threshold 0.5: no cheaper value
  pass 1, quiet 586.1724: no cheaper value
  fitted: weight 0.6139, threshold 0.5, quiet 586.1724 s: cost 115, caught 107 of 125, 43 false alarms
  fit only: weight 0.6139, threshold 0.5, quiet 586.1724 s: cost 115, caught 107 of 125, 43 false alarms
the held-out 80 runs, read once: 800 steps, 253 stalled
  hand-written: weight 0.0, threshold 0.5, quiet 700.0 s: cost 708, caught 78 of 253, 8 false alarms
  fitted: weight 0.6139, threshold 0.5, quiet 586.1724 s: cost 162, caught 234 of 253, 86 false alarms
  decision gate, fitted against hand-written: gain 0.6825 per step, floor 0.0414 -> passed (beyond the floor)
  cross-validated over the 16 fit sets: cost per step 0.2201, floor 0.0414
  fit only over the 16 fit sets: cost per step 0.2109, floor 0.0599
  fit only against cross-validated, by fit set: gain 0.0091 per step, 34 of 1000 replicates at or below no gain -> unverifiable
```

The [example](../examples/stall-settings.coax) fits 34 times and runs in
about two seconds.

## Recorded answers

An agent works through runs of ten steps. At each step a model has already
answered how likely it is that the agent is stuck. Two more facts are
recorded: how many times the agent repeated its last call, and how many
seconds passed with no output. Whether the step really was stalled is known
afterwards.

<!-- source: psionic/lib/settings.coax simulate -->
```jai
// stall-sim-v1. Run g draws how prone it is to stall; step s of the run is
// stalled or not, and records the model's probability that it is stalled, how
// many times the agent repeated its last call, and how many seconds passed
// without output. The probability is honest but timid: a stalled step reads
// 0.2 to 0.7, a working one 0 to 0.55.
st_simulate :: (seed:u64,runs:int,out:[]StRow)->StStatus #export_c {
    if runs<1 || runs>ST_ROWS_MAX/ST_STEPS {return StStatus.shape;}
    if out.count<runs*ST_STEPS {return StStatus.capacity;}
    for g:0..<runs {
        prone:=tb_uniform(seed,cast(u64)g);
        for s:0..<ST_STEPS {
            b:=cast(u64)(1000*(g+1)+8*s);
            stalled:=cast(int)(tb_uniform(seed,b)+0.1+0.4*prone)>0;
            a1:=tb_uniform(seed,b+1);a2:=tb_uniform(seed,b+2);a3:=tb_uniform(seed,b+3);
            r:=StRow.{group=g,stalled=stalled};
            if stalled {
                r.p=0.2+0.5*(a1+a2)/2;r.repeats=cast(int)(9*a3);r.quiet=30+700*tb_uniform(seed,b+4);
            } else {
                r.p=0.55*(a1+a2)/2;r.repeats=cast(int)(4*a3*a3);r.quiet=700*tb_uniform(seed,b+4)*tb_uniform(seed,b+5);
            }
            out[g*ST_STEPS+s]=r;
        }
    }
    return StStatus.ok;
}
```

The simulator gives the model a known flaw: it is honest but timid. A stalled
step gets a probability between 0.2 and 0.7, so a threshold of 0.5 misses most
of them. Repeated calls and long silences carry what the probability leaves
out.

## The rule and its price

<!-- source: psionic/lib/settings.coax rule -->
```jai
// The detector: flag a step when the probability, raised by the weight for
// each repeated call, reaches the threshold, or when it has been quiet for at
// least the cut point in seconds.
st_score_of :: (s:StSettings,row:StRow)->f64 {return row.p+s.weight*f64(row.repeats)/ST_REPEATS;}
st_flag :: (s:StSettings,row:StRow)->bool {return st_score_of(s,row)>=s.threshold || row.quiet>=s.quiet;}
// Written costs: a missed stall and a false alarm, counted then priced.
st_score :: (rows:[]StRow,s:StSettings,costs:StCosts)->StScore #export_c {
    r:=StScore.{rows=rows.count};
    for row:rows {
        f:=st_flag(s,row);
        if row.stalled {r.stalls+=1;}
        if f {r.flagged+=1;}
        if row.stalled && f {r.caught+=1;}
        if row.stalled && !f {r.missed+=1;}
        if !row.stalled && f {r.alarms+=1;}
    }
    r.cost=costs.miss*f64(r.missed)+costs.alarm*f64(r.alarms);
    return r;
}
```

The costs are written before anything is fitted: a missed stall costs 4, a
false alarm costs 1. Counts are kept as integers and priced at the end, so the
sum is the same in any order.

The hand-written settings ignore repeats (weight 0), flag at 0.5, and wait 700
seconds. On the 400 recorded steps they catch 39 of 125 stalls, for a cost
of 351.

## Candidates

<!-- source: psionic/lib/settings.coax candidates -->
```jai
// Where a setting can change a decision. The threshold changes one where a
// step's score sits, the cut point where its quiet seconds sit, and the
// weight where a step that is below the threshold and repeated reaches it.
// Between two such points every decision is the same, so the candidates are
// the midpoints, plus one value past each end.
st_candidates :: (rows:[]StRow,s:StSettings,which:int,out:[]f64)->int {
    points:[ST_ROWS_MAX]f64;n:=0;
    for row:rows {
        if which==0 {
            // Below the threshold and able to reach it under the largest weight.
            gap:=(s.threshold-row.p)*ST_REPEATS;
            if row.p<s.threshold && gap<ST_WEIGHT_MAX*f64(row.repeats) {points[n]=gap/f64(row.repeats);n+=1;}
        } else {points[n]=ifx which==1 then st_score_of(s,row) else row.quiet;n+=1;}
    }
    m:=st_distinct(points[..],n);
    out[0]=ifx which==0 then 0 else points[0];k:=1;
    for i:1..<m {out[k]=(points[i-1]+points[i])/2;k+=1;}
    out[k]=ifx which==0 then ST_WEIGHT_MAX else points[m-1]+1;
    return k+1;
}
```

A setting can only change a decision at a few points:
- the **threshold**, where a step's score sits;
- the **cut point**, where a step's quiet seconds sit;
- the **weight**, where a step below the threshold just reaches it.

Between two such points every decision is the same. So the midpoints, plus
one value past each end, cover every distinct outcome, and nothing finer is
worth trying. The weight is held between 0 and 4.

<!-- source: psionic/lib/settings.coax best -->
```jai
// The candidate with the lowest written cost, the first on ties; it replaces
// the current value only when strictly cheaper.
st_best :: (rows:[]StRow,s:StSettings,which:int,costs:StCosts)->StBest {
    out:[ST_ROWS_MAX+2]f64;k:=st_candidates(rows,s,which,out[..]);
    r:=StBest.{value=st_get(s,which),current=st_score(rows,s,costs).cost};r.cost=r.current;
    for x:out[0..<k] {
        c:=st_score(rows,st_with(s,which,x),costs).cost;
        if c<r.cost {r.value=x;r.cost=c;}
    }
    return r;
}
```

## The search

<!-- source: psionic/lib/settings.coax fit -->
```jai
// Coordinate search over the weight, the threshold and the cut point, in that
// order, until a pass changes nothing. A move is kept only when it is
// strictly cheaper on the recorded answers and, with cross-validation, when
// refitting that one setting without each fold of runs and pricing it on the
// fold is strictly cheaper, in total, than leaving it alone.
st_fit :: (rows:[]StRow,start:StSettings,costs:StCosts,folds:int,cv:bool,passes:int,trail:[]StStep)->StFit #export_c {
    status:=st_rows_ok(rows);if status!=StStatus.ok {return StFit.{status=status};}
    if !st_settings_ok(start) || !(costs.miss>=0 && costs.miss<=ST_BIG) || !(costs.alarm>=0 && costs.alarm<=ST_BIG) {return StFit.{status=StStatus.number};}
    if folds<2 || folds>ST_FOLDS_MAX || passes<1 || passes>ST_PASSES_MAX {return StFit.{status=StStatus.shape};}
    if trail.count<3*passes {return StFit.{status=StStatus.capacity};}
    // Every fold holds out at least one run, so every fold also keeps one.
    seen:[16]bool;for row:rows {seen[row.group%folds]=true;}
    for f:0..<folds {if !seen[f] {return StFit.{status=StStatus.shape};}}
    r:=StFit.{settings=start};
    train:[ST_ROWS_MAX]StRow;
    for pass:0..<passes {
        changed:=false;
        for which:0..<3 {
            b:=st_best(rows,r.settings,which,costs);
            step:=StStep.{pass=pass,which=which,from=st_get(r.settings,which),to=b.value,before=b.current,after=b.cost};
            if b.cost>=b.current {step.outcome=StOutcome.no_gain;}
            else if cv {
                held:=0.0;
                for f:0..<folds {
                    a:=0;
                    for row:rows {if row.group%folds!=f {train[a]=row;a+=1;}}
                    // The fold's own counts are the whole set's less the training runs'.
                    x:=st_with(r.settings,which,st_best(train[0..<a],r.settings,which,costs).value);
                    whole:=st_score(rows,x,costs);part:=st_score(train[0..<a],x,costs);
                    held+=costs.miss*f64(whole.missed-part.missed)+costs.alarm*f64(whole.alarms-part.alarms);
                }
                step.held=held;
                step.outcome=ifx held<b.current then StOutcome.accepted else StOutcome.no_held_gain;
            } else {step.outcome=StOutcome.accepted;}
            if step.outcome==StOutcome.accepted {r.settings=st_with(r.settings,which,b.value);r.accepted+=1;changed=true;}
            trail[r.steps]=step;r.steps+=1;
        }
        if !changed {break;}
    }
    return r;
}
```

Each pass tries the weight, then the threshold, then the cut point, holding
the other two. The search ends when a pass changes nothing.

With cross-validation, the runs are dealt into 5 folds by run, never by step:
steps of one run are alike, and splitting them would let a fold see its own
run. For each fold, the move is fitted again without that fold and priced on
it. The move is kept only if the folds' total is strictly below the cost of
leaving the setting alone. Pricing a fold uses the whole set's counts less
the training runs' counts, so no step is priced twice.

On the recorded answers the weight moves first, from 0 to 0.61. That one
change takes the cost from 351 to 124. The threshold then has no cheaper
value, and the cut point drops from 700 to 586 seconds. The held-out folds
agree with both moves, 128 against 351 and 119 against 124.

## Reading the held-out runs once

All 32 sets of settings are written down before the 80 held-out runs are
read: the main fit and 15 more fit sets, each fitted both ways. The held-out
runs are then read once.

| Settings | Cost per step | Stalls caught | False alarms |
| --- | --- | --- | --- |
| hand-written | 0.885 | 78 of 253 (31%) | 8 |
| fitted | 0.2025 | 234 of 253 (92%) | 86 |

The fitted detector raises ten times as many false alarms. The written costs
say that trade is worth it: a missed stall costs four alarms.

Chapter 239's decision gate takes its floor from the 16 fit sets, which is how
far the fitted result moves when the recorded answers do. The gain is 0.6825
per step against a floor of 0.0414, and no task replicate falls back to the
floor. It **passes**.

## Does cross-validation earn its keep?

On the main fit set both rules land on the same settings. Across all 16 fit
sets they differ:

| Rule | Mean cost per step | Floor | Cheaper on |
| --- | --- | --- | --- |
| cross-validated | 0.2201 | 0.0414 | 1 fit set |
| fit only | 0.2109 | 0.0599 | 9 fit sets |

They tie on the other 6. Cross-validation refuses some moves that would have
paid off. The settings it keeps move less from one fit set to the next, which
shows as a lower floor.

Is fit-only cheaper? A bootstrap over the 16 fit sets puts 34 of 1,000
replicates at or below no gain. That is more than the 25 that chapter 239
allows for a clear result. It is **unverifiable**.

The rule that worked before is not shown to help here, and it is not shown to
hurt. The chapter keeps it as an option, not a default, and reports both.
With three settings and 400 steps there is little room to overfit. A layer
with many more settings, or fewer runs, is where cross-validation has
something to refuse.

## Supported profile

`cpu-f64-settings-v1`:
- binary64; costs counted as integers and priced at the end;
- the stall-sim-v1 simulator on chapter 103's SplitMix64 counters;
- one rule of three settings: a weight from 0 to 4 per eight repeats, a
  threshold and a cut point within ±10³⁰⁰;
- up to 1,024 recorded steps, 2 to 16 folds of runs, and up to 16 passes.

The search reads its rows and writes its trail of steps, which have different
types. The native adapter only names native arrays as rows and as a trail.

## How it is checked

- **Coax checks:** 63 checks in debug and release, counting the units it
  loads. They cover:
  - the simulator at its counters, against a second formula for each field;
  - the rule and its costs by hand, including a step exactly at the threshold
    and at the cut point;
  - candidates by hand: duplicates, a step at the threshold, and a step that
    the largest weight only just reaches;
  - the cheapest candidate, the first on ties, and no move unless strictly
    cheaper;
  - a fit on six hand-worked steps, whose refused weight is worked out in a
    comment;
  - fits on four simulated runs with 2 and 3 folds, pinned to the oracle,
    including a fold total exactly equal to the current cost;
  - every refusal and bound.
- **An independent oracle,** [settings_oracle.py](../tests/settings_oracle.py),
  writes the simulator, rule, candidates and search again. It takes the floor
  and gate from chapters 238 and 239's oracles.
- **The native harness** [settings.c](../tests/settings.c) runs the whole
  experiment through the adapter, plain and under ASan/UBSan, in debug and
  release. Rows, settings, counts, costs, outcomes, verdicts and replicate
  counts must be equal. Floors and gains must agree within 10⁻¹². The largest
  difference was 0.
- **Mutation testing** of `settings.coax` kills all 1,260 mutants. The first
  run left 40 survivors, fixed in the code or the checks:
  - With two folds, swapping the training and held-out runs gives the same
    total, so the checks pin a three-fold fit.
  - A stall drawn by comparing two continuous numbers can't be tested at
    equality. It is now drawn by truncating a sum.
  - Guards made redundant by later ones were removed.
  - Each held-out fold is now priced from counts, with no buffer of its own.
  - The check helpers are tested on mismatches.

The [verification record](../../docs/verification/psionic-ch182-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **One rule.** The settings, the order of the search and the candidates
  belong to this detector. Another layer needs its own rule written down.
- **Coordinate search only.** One setting moves at a time, so a pair of moves
  that only pays together is never found.
- **Simulated records.** Chapter 215's decision log and chapter 245's outcome
  rows are where real recorded answers come from.
- **No search over rules.** Choosing between whole policies, cheapest
  measurement first, is chapter 242.
