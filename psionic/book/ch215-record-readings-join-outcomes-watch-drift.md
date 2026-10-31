# 215. Record readings, join outcomes, watch drift

[Chapter 213](ch213-fit-our-own-calibration.md) fitted a calibration map and
[chapter 214](ch214-thresholds-from-written-costs.md) derived thresholds from
costs. Both were checked once, on data drawn before anything was served. Once
decisions are being made, the world can move: the same probability can stop
meaning what it meant.

This chapter keeps a record of every decision, joins each record to its
outcome when the outcome arrives, and watches calibration window by window. It
refits the map on a schedule. Each record also carries enough to evaluate a
different threshold from the log alone.

```shell
coax/bin/coax run psionic/examples/watch-drift.coax
psionic/check.sh --only drift
```

```jai
first map, 3000 calibration rows, gated against the raw probabilities:
  in use (raw) error 0.1025, log loss 0.555; temperature 0.0293, 0.4679; binned 0.0286, 0.476; isotonic 0.0269, 0.4723
  -> temperature
outcomes: 10817 joined, 1183 never arrived; 818 decisions explored
refit at decision 4000:
  in use (temperature) error 0.0407, log loss 0.4228; temperature 0.0357, 0.4194; binned 0.0322, 0.423; isotonic 0.0272, 0.421
  -> temperature
refit at decision 8000:
  in use (temperature) error 0.0758, log loss 0.5772; temperature 0.0246, 0.5574; binned 0.0278, 0.4364; isotonic 0.0409, 0.4405
  -> binned
window  map  joined  calibration error  draws reaching it (of 200)
0  1  901  0.0276  109  
1  1  909  0.0272  115  
2  1  887  0.0407  21  
3  1  900  0.0229  146  
4  2  905  0.0665  0  ALERT
5  2  912  0.0899  0  ALERT
6  2  904  0.0758  0  ALERT
7  2  913  0.0941  0  ALERT
8  3  905  0.0226  93  
9  3  904  0.0089  192  
10  3  886  0.0233  94  
11  3  891  0.0138  177  
record 4321: map 2, policy 1, raw 0.2429, calibrated 0.3568, threshold 0.25, eligible 3, action 1, selection probability 0.9, joined at 4355, label 0, cost 1.0
threshold 0.25: from the log 0.3746 ± 0.0131, true 0.3743
threshold 0.5: from the log 0.402 ± 0.0427, true 0.4082
threshold 0.05: unsupported
```

The [example](../examples/watch-drift.coax) runs in about three seconds.

## A record per decision, with no content

<!-- source: psionic/lib/drift.coax record -->
```jai
dr_decide :: (policy:DrPolicy,map:CfMap,version:int,id:int,raw:f64,u:f64)->DrRecord {
    c:=cf_map(map,raw);greedy:=0;
    if c>=policy.threshold {greedy=1;}
    r:=DrRecord.{id=id,model=map.model,questions=map.questions,map_version=version,policy_version=policy.version,
        raw=raw,calibrated=c,threshold=policy.threshold,eligible=3,action=greedy,propensity=1-policy.explore,joined_at=-1};
    // Near certainty only the greedy action is eligible, and it is chosen with probability 1.
    if !tb_in(c,policy.sure,1-policy.sure) {r.eligible=1<<greedy;r.propensity=1;return r;}
    if u<policy.explore {r.action=1-greedy;r.propensity=policy.explore;}
    return r;
}
// An outcome joins its record once: the label and the cost of the action taken.
dr_join :: (entries:[]DrRecord,logged:int,id:int,label:f64,at:int,policy:DrPolicy)->DrStatus #export_c {
    if id<0 || id>=logged || logged>entries.count {return DrStatus.shape;}
    if label!=0 && label!=1 {return DrStatus.number;}
    if entries[id].status==DrOutcome.observed {return DrStatus.duplicate;}
    a:=f64(entries[id].action);
    entries[id].status=DrOutcome.observed;entries[id].label=label;entries[id].joined_at=at;
    entries[id].cost=a*(1-label)*policy.wrong_yes+(1-a)*label*policy.wrong_no;
    return DrStatus.ok;
}
```

A record holds numbers only, never the request or the answer:

| Field | Why it is kept |
| --- | --- |
| Model, question set, map version | Which calibration produced the probability, so a stale map shows up in the log |
| Raw and calibrated probability | What the model said, and what the decision used |
| Threshold, policy version | The rule that turned the probability into an action |
| Eligible actions | Which actions the policy could have taken |
| Action, selection probability | What it did, and how likely it was to do that |
| Outcome status, arrival, label, cost | Pending until the outcome joins; then the label and what the action cost |

The policy acts yes from 0.25, the voice threshold of chapter 214. With
probability 0.1 it takes the other action instead, so the log contains outcomes
for actions the threshold would not have chosen. Within 0.1 of 0 or 1 it never
explores: only the greedy action is eligible, and its selection probability is
1.

An outcome joins its record once. `dr_join` refuses an unknown record, a
label other than 0 or 1, and a second outcome for the same record.

## The world, and when its outcomes arrive

The data are synthetic, so the drift is known exactly:
- Decision t has a true logit l uniform in [−3, 3].
- The model reports σ(2l). It is overconfident, as in chapter 213.
- Until decision 4,000 the label is yes with probability σ(l). From then on it
  is σ(l − 1.5): the same evidence means yes less often.
- An outcome arrives 1 to 500 decisions later. One in ten never arrives.

The first map is fitted on 3,000 calibration rows drawn before the stream,
with chapter 213's procedure: two windows to fit, one to gate. Temperature has
the lowest gate log loss (0.4679 against 0.555 raw) and is kept.

## How unusual is a window's error?

A calibration error of 0.04 means little on its own, because a perfectly
calibrated window of 900 rows still shows some error by chance. The watcher
asks how often chance alone would produce it:

<!-- source: psionic/lib/drift.coax watch -->
```jai
// How unusual is a window's calibration error? Draw its labels again from its
// own probabilities, 200 times, so that they are calibrated by construction,
// and count the draws whose error is at least the window's. Rows are 0 to
// count - 1.
dr_exceed :: (p:[]f64,y:[]f64,rows:[]int,seed:u64,window:int)->int {
    observed:=cf_measure(p,y,rows).ece;z:[1000]f64;count:=0;
    for b:0..<DR_DRAWS {
        base:=cast(u64)((window*DR_DRAWS+b)*DR_WINDOW_MAX);
        for j:0..<rows.count {z[j]=floor(tb_uniform(seed,base+cast(u64)j)+p[j]);}
        if cf_measure(p,z[0..<rows.count],rows).ece>=observed {count+=1;}
    }
    return count;
}
// The watcher reads only the log: each window's joined records, their
// calibration error, and an alert when fewer than 2 of 200 calibrated draws
// reach it (about a 1 percent chance for a calibrated window).
dr_watch :: (entries:[]DrRecord,window:int,seed:u64)->DrWatch #export_c {
    v:=DrWatch.{windows=dr_windows(window,entries.count)};
    if v.windows==0 {return DrWatch.{status=DrStatus.shape};}
    p:[1000]f64;y:[1000]f64;rows:[1000]int;
    for w:0..<v.windows {
        m:=dr_rows(entries,w*window,(w+1)*window,false,p[..],y[..],rows[..]);
        v.joined[w]=m;v.ece[w]=cf_measure(p[0..<m],y[0..<m],rows[0..<m]).ece;
        v.exceed[w]=dr_exceed(p[0..<m],y[0..<m],rows[0..<m],seed,w);v.alert[w]=v.exceed[w]<DR_ALERT;
    }
    return v;
}
// Refit from the raw probabilities of two older windows, gated on a newer
// one against the map in use: chapter 213's three kinds and its gate.
dr_refit :: (entries:[]DrRecord,current:CfMap,fit_from:int,gate_from:int,window:int)->DrRefit {
    fp:[2000]f64;fy:[2000]f64;fq:[2000]int;rows:[2000]int;gp:[1000]f64;gy:[1000]f64;gr:[1000]int;mapped:[1000]f64;
    n:=dr_rows(entries,fit_from,fit_from+DR_FIT*window,true,fp[..],fy[..],rows[..]);
    g:=dr_rows(entries,gate_from,gate_from+window,true,gp[..],gy[..],gr[..]);
    for j:0..<g {mapped[j]=cf_map(current,gp[j]);}
    now:=cf_measure(mapped[0..<g],gy[0..<g],gr[0..<g]);
    out:=DrRefit.{map=current,gate=DrGate.{kind=current.kind}};best:=now.log_loss;
    out.gate.ece[0]=now.ece;out.gate.loss[0]=now.log_loss;
    for kind:1..<4 {
        m:=cf_fit(cast(CfKind)kind,current.model,current.questions,0,fq[0..<n],fp[0..<n],fy[0..<n],rows[0..<n]);
        out.gate.ece[kind]=-1;out.gate.loss[kind]=-1;
        if m.status==CfStatus.ok {
            for j:0..<g {mapped[j]=cf_map(m,gp[j]);}
            r:=cf_measure(mapped[0..<g],gy[0..<g],gr[0..<g]);
            out.gate.ece[kind]=r.ece;out.gate.loss[kind]=r.log_loss;
            if cf_gate(now,r) && r.log_loss<best {out.map=m;out.gate.adopted=true;out.gate.kind=m.kind;best=r.log_loss;}
        }
    }
    return out;
}
```

For each window it draws the labels again, 200 times, from the window's own
probabilities. Those labels are calibrated by construction. It then counts the
draws whose error is at least the window's. If fewer than 2 of 200 do, the
window is flagged. A calibrated window is flagged about 1 time in 100.

There is no hand-chosen bound to tune. The comparison adapts to the window's
size and to where its probabilities lie.

The watcher reads only the log: a window is measured once its last outcome can
have arrived, half a window after its last decision.

| Window | Map | Joined | Calibration error | Draws reaching it |
| --- | --- | --- | --- | --- |
| 0–3 | 1 | 887–909 | 0.0229–0.0407 | 21–146 |
| 4–7 | 2 | 904–913 | 0.0665–0.0941 | 0: alert |
| 8–11 | 3 | 886–905 | 0.0089–0.0233 | 93–192 |

All four windows after the drift are flagged, and none before it.

Small windows are much less sensitive. In the checks, a shift of 3 in windows
of about 45 rows went unflagged at some seeds.

## Refits on a schedule

Every four windows the map is refitted. Each refit fits on two older windows
and gates on a newer one, against the map in use rather than the raw
probabilities. It reads raw probabilities, and only those records whose outcome
has already arrived:

<!-- source: psionic/lib/drift.coax run -->
```jai
// Time runs past the last decision until every outcome that will arrive
// has. Outcomes arrive at most half a window late. At each step, outcomes
// arriving now join; every four windows the map is refitted from windows
// whose outcomes are all in; then the next decision.
dr_run :: (seed:u64,policy:DrPolicy,first:CfMap,window:int,drift_at:int,shift:f64,entries:[]DrRecord)->DrRun #export_c {
    n:=entries.count;windows:=dr_windows(window,n);
    if windows==0 || first.status!=CfStatus.ok {return DrRun.{status=DrStatus.shape};}
    if !dr_inputs_ok(policy,shift) {return DrRun.{status=DrStatus.number};}
    run:=DrRun.{windows=windows};delay:=window/2;
    // Outcomes wait in a list per arrival time, as id + 1 (0 ends a list);
    // arrival 0 is never reached.
    head:[16500]int;next:[16000]int;label:[16000]f64;
    map:=first;version:=1;refit:=DR_REFIT*window;
    for t:0..<n+delay {
        id:=head[t];
        while id>0 {
            if dr_join(entries,n,id-1,label[id-1],t,policy)==DrStatus.ok {run.joined+=1;}
            id=next[id-1];
        }
        if t<n {
            if t==refit {
                r:=dr_refit(entries,map,t-DR_REFIT*window,t-2*window,window);
                run.refit[run.refits]=r.gate;run.refits+=1;refit+=DR_REFIT*window;
                if r.gate.adopted {map=r.map;version+=1;}
            }
            d:=dr_draw(seed,t,ifx t>=drift_at then shift else 0,delay);
            if d.arrival==0 {run.lost+=1;}
            label[t]=d.label;next[t]=head[d.arrival];head[d.arrival]=t+1;
            entries[t]=dr_decide(policy,map,version,t,d.raw,d.explore);
            if entries[t].propensity==policy.explore {run.explored+=1;}
            run.version[t/window]=version;
        }
    }
    return run;
}
```

| Refit | In use | Temperature | Binned | Isotonic | Kept |
| --- | --- | --- | --- | --- | --- |
| Decision 4,000 (data before the drift) | 0.0407 / 0.4228 | 0.0357 / **0.4194** | 0.0322 / 0.423 | 0.0272 / 0.421 | new temperature |
| Decision 8,000 (data after the drift) | 0.0758 / 0.5772 | 0.0246 / 0.5574 | 0.0278 / **0.4364** | 0.0409 / 0.4405 | binned |

Each cell is calibration error / log loss on the gate window.
- **At 4,000** the drift hasn't started. A temperature fitted on more data is a
  little better, and is kept as map 2.
- **At 8,000** all three kinds pass the gate. Temperature has the lowest
  calibration error, but binned has the lowest log loss, and log loss ranks
  first. A slope can't undo a shift, as chapter 213 found.

The alerts and the schedule are separate. Windows 4 to 7 were flagged from
decision 5,500 on, and the scheduled refit replaced the map at 8,000. Refitting
when an alert fires is not done here.

The gate needs both a lower calibration error and a lower log loss. One check
shows the cost of that. A map that answers the base rate for everything has no
calibration error at all, so a sharper map that would lower the log loss is
refused. [Chapter 239's](ch239-gates-that-can-say-unverifiable.md) gates rank the two.

## Other thresholds, from the log

Because each record carries its eligible actions and selection probability, the
log can answer a question it was never asked. What would another threshold
have cost?

<!-- source: psionic/lib/drift.coax evaluate -->
```jai
// Inverse propensity: a logged record counts when the new threshold would
// have taken the same action, weighted by one over that action's selection
// probability. A threshold that needs an action the log never made eligible
// is refused: the log can't say what it would cost.
dr_ips :: (entries:[]DrRecord,threshold:f64)->DrValue #export_c {
    if entries.count>DR_WINDOWS_MAX*DR_WINDOW_MAX {return DrValue.{status=DrStatus.shape};}
    if !tb_in(threshold,0,1) {return DrValue.{status=DrStatus.number};}
    s:PsionicAccumulator;used:=0;
    for r:entries {
        a:=0;if r.calibrated>=threshold {a=1;}
        if (r.eligible&(1<<a))==0 {return DrValue.{status=DrStatus.unsupported};}
        if r.status==DrOutcome.observed {
            used+=1;
            if r.action==a {s=psionic_ops_accumulate(s,r.cost/r.propensity);}
        }
    }
    if used<2 {return DrValue.{status=DrStatus.shape};}
    mean:=psionic_ops_total(s)/f64(used);v:PsionicAccumulator;
    for r:entries {
        if r.status==DrOutcome.observed {
            a:=0;if r.calibrated>=threshold {a=1;}
            e:=-mean;if r.action==a {e+=r.cost/r.propensity;}
            v=psionic_ops_accumulate(v,e*e);
        }
    }
    return DrValue.{used=used,value=mean,half=1.96*sqrt(psionic_ops_total(v)/f64(used-1)/f64(used))};
}
// The simulator knows every label, lost or not: the threshold's true mean cost.
dr_truth :: (seed:u64,drift_at:int,shift:f64,policy:DrPolicy,entries:[]DrRecord,threshold:f64)->f64 #export_c {
    s:PsionicAccumulator;
    for r,t:entries {
        y:=dr_label(seed,t,ifx t>=drift_at then shift else 0);
        a:=0.0;if r.calibrated>=threshold {a=1;}
        s=psionic_ops_accumulate(s,a*(1-y)*policy.wrong_yes+(1-a)*y*policy.wrong_no);
    }
    return psionic_ops_total(s)/f64(entries.count);
}
```

| Threshold | From the log (± 1.96 standard errors) | True cost |
| --- | --- | --- |
| 0.25 (the one used) | 0.3746 ± 0.0131 | 0.3743 |
| 0.5 | 0.4020 ± 0.0427 | 0.4082 |
| 0.05 | refused | 0.5503 |

The simulator knows every label, including the 1,183 that never arrived, so
the true cost is exact. Both estimates are within their intervals.

The interval for 0.5 is three times wider, because its actions differ from the
logged ones more often and those records count only when exploration happened
to agree. Threshold 0.05 is refused outright. It would act yes on probabilities
between 0.05 and 0.1, where the policy never explored, so no record says what
yes costs there.

## Supported profile

`cpu-f64-drift-v1`:
- binary64, with chapter 4's compensated accumulator for every sum;
- one yes-or-no question, two actions, and a policy with a threshold,
  exploration in [0.01, 0.25] and a no-exploration margin in [0, 0.25];
- 1 to 16 windows of 50 to 1,000 decisions;
- outcomes at most half a window late;
- a refit every four windows, on two windows to fit and one to gate;
- drift as a shift of the true logit within [−8, 8];
- an alert when fewer than 2 of 200 calibrated draws reach a window's error;
- inverse-propensity estimates with normal intervals.

The procedures that write caller storage write only the log. The native adapter
names a native array as a log, and there is no second span that could overlap.

## How it is checked

- **Coax checks:** 84 checks in debug and release, counting the units it
  loads. They cover:
  - the world at its counters;
  - decisions by hand at the threshold and both exploration margins;
  - joins and every refusal;
  - the draw count, and a window reached by exactly 1 and exactly 2 draws;
  - a window as calibrated as it can be, which is no alert;
  - each refit's gate recomputed, a three-way tie that keeps the first kind,
    the base-rate map that stays, and refused fits;
  - a short run recomputed record by record, and a run of small windows with
    two refits;
  - inverse propensity by hand, the true cost, and every bound.
- **An independent oracle,** [drift_oracle.py](../tests/drift_oracle.py),
  written from the specification. It fits maps with chapter 213's oracle and
  computes the run in closed form rather than by stepping time: every outcome
  that isn't lost joins at its arrival, and a refit sees exactly the records
  of its windows that weren't lost.
- **The native harness** [drift.c](../tests/drift.c) runs the whole example
  through the adapter, plain and under ASan/UBSan, in debug and release.
  Tolerances were frozen before comparison:
  - kinds, adoptions, versions, counts, actions, draw counts and alerts must
    be equal;
  - errors, log losses, estimates and true costs within 10⁻¹².

  The largest difference was 1.1 × 10⁻¹³. NaN and infinity are refused in
  labels, shifts, thresholds and policies.
- **Mutation testing** of `drift.coax` kills all 1,605 mutants. The first
  runs left survivors, each fixed in the code or the checks:
  - a sorted percentile, whose ties made `>` and `>=` the same, became a
    count of draws;
  - a lost outcome, compared with `<`, became a floor;
  - the refit's per-kind results were made visible;
  - refits follow a counter rather than a remainder;
  - lists of waiting outcomes need no initialization;
  - calibration rows stopped pretending to be decisions;
  - checks were added for a tie, a failed fit and the decision at the drift.

The [verification record](../../docs/verification/psionic-ch158-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **Synthetic drift,** one shift at a known time, and one run.
- **No judged sample.** Lost outcomes stay lost. A small sample labelled by
  hand or by Mammoth, with its own selection probability, would cover them.
- **Alerts don't trigger anything.** A refit on alert, or rolling back to the
  previous map, needs its own gate.
- **One question, two actions.** Chapter 214's escalate and ask actions, and
  the doubly robust estimates of chapter 223, would use the same log.
