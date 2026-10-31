# 242. Search a policy, cheapest first

[Chapter 241](ch241-fit-settings-on-recorded-answers.md) tuned the settings of
one rule without calling a model. The next choice is bigger: which policy to
run. A policy here is four choices:
- which model answers;
- how many times to retry;
- how much context to send;
- whether to check an answer before accepting it.

Every candidate has to be measured with real calls, and real calls cost money.
Measuring every policy fully is the honest way to choose, and also the most
expensive.

This chapter finds the same winner for a quarter of that spend:
- **policies as manifests:** each has a SHA-256 digest, under
  [chapter 237](ch237-a-suite-is-three-digests.md)'s encoding;
- **operators named for what they do:** escalate, retry, widen and verify
  grow every policy from the simplest one;
- **successive halving:** all of them are measured cheaply, the best are kept,
  plus any policy that alone is best on some task, and the survivors are
  measured more fully;
- **the held-out read last:** the winner is written down before held-out
  tasks are read, and reported through
  [chapter 239](ch239-gates-that-can-say-unverifiable.md)'s gate.

```shell
coax/bin/coax run psionic/examples/policy-search.coax
psionic/check.sh --only search
```

```jai
32 policies, breadth first from the simplest (Elephant, 0 retries, short context, digest 1e94bdd0)
  round 1: 32 policies on 12 tasks x 1 trials, best #19; kept 16, lone bests 0; spent 2395.5 so far
  round 2: 16 policies on 30 tasks x 2 trials, best #30; kept 8, lone bests 1; spent 6047.0 so far
  round 3: 9 policies on 60 tasks x 4 trials, best #30; kept 0, lone bests 0; spent 11479.5 so far
winner #30: Elephant, 3 retries, long context, checked, digest 6feb8050
  from the simplest: retry retry retry widen verify
measuring every policy in full would spend 45490.0 (the search spent 11479.5) and pick #30
  another policy scores higher on 55 of 60 tasks; the best per task averages 0.7206, the winner 0.6117
held-out 60 tasks, 4 trials each: score per task 0.3725 for the simplest, 0.5558 for the winner; floor 0.0834
  decision gate: gain 0.1833 per task, 7 of 1000 replicates at or below no gain -> passed (beyond the floor)
```

The [example](../examples/policy-search.coax) runs in about four seconds.

## A policy is a manifest

<!-- source: psionic/lib/search.coax manifest -->
```jai
// A policy's digest: chapter 237's encoding under its own tag.
sr_digest :: (p:SrPolicy)->[32]u8 #export_c {
    buf:[64]u8;at:=su_text(buf[..],0,"psionic.policy.v1");buf[at]=p.model;
    at=su_u64(buf[..],at+1,cast(u64)p.retries);buf[at]=p.context;at=su_flag(buf[..],at+1,p.verify);
    return su_hash(buf[0..<at]);
}
// Each operator changes one field and says so; at its limit it changes nothing.
sr_apply :: (p:SrPolicy,op:SrOp)->SrApplied {
    r:=SrApplied.{policy=p};
    if op==SrOp.escalate && p.model==0 {r.policy.model=1;r.changed=true;}
    if op==SrOp.retry && p.retries<SR_RETRIES_MAX {r.policy.retries+=1;r.changed=true;}
    if op==SrOp.widen && p.context==0 {r.policy.context=1;r.changed=true;}
    if op==SrOp.verify && !p.verify {r.policy.verify=true;r.changed=true;}
    return r;
}
// Breadth first from the simplest policy. A policy grows only by its own
// operator or a later one, so each is reached by one path (escalate, then
// retries, then widen, then verify) and none twice; each is recorded with
// its digest, the policy it came from and the operator used.
sr_expand :: (out:[]SrEntry)->SrExpand #export_c {
    n:=1;
    if out.count<32 {return SrExpand.{status=SrStatus.capacity};}
    out[0]=SrEntry.{policy=SR_SIMPLEST,digest=sr_digest(SR_SIMPLEST),parent=-1};
    k:=0;
    while k<n {
        for o:cast(int)out[k].op..<4 {
            op:=cast(SrOp)o;a:=sr_apply(out[k].policy,op);
            if a.changed {out[n]=SrEntry.{policy=a.policy,digest=sr_digest(a.policy),parent=k,op=op};n+=1;}
        }
        k+=1;
    }
    return SrExpand.{count=n};
}
```

A policy is written down as four fields and named by its digest. Two policies
with the same fields have the same digest, wherever they were built. The
digest also goes into a receipt, so a result can be traced to the exact
policy that earned it.

Each operator changes one field by one step. At its limit it changes nothing
and says so: Mammoth can't escalate further, and three retries is the most.

Starting from the simplest policy, the search expands breadth first. Each
policy grows only by its own operator or a later one. So every one of the
2 × 4 × 2 × 2 = 32 policies is reached by exactly one path, and no
duplicates need to be checked for. A policy's lineage reads as a recipe. The
winner, #30, is the simplest policy after retry, retry, retry, widen, verify.

## A world with known answers

<!-- source: psionic/lib/search.coax world -->
```jai
// world-sim-v1. Task i draws a difficulty in [-2, 2], whether it needs long
// context (3 in 10) and whether its answer can be checked (4 in 10).
sr_tasks :: (seed:u64,out:[]SrTask)->SrStatus #export_c {
    if out.count<1 || out.count>SR_TASKS_MAX {return SrStatus.shape;}
    for i:0..<out.count {
        b:=cast(u64)(3*i);
        out[i]=SrTask.{difficulty=4*tb_uniform(seed,b)-2,needs_context=cast(int)(tb_uniform(seed,b+1)+0.3)>0,
            checkable=cast(int)(tb_uniform(seed,b+2)+0.4)>0};
    }
    return SrStatus.ok;
}
sr_chance :: (p:SrPolicy,task:SrTask)->f64 {
    z:=SR_SKILL[cast(int)p.model]-task.difficulty;
    if task.needs_context {z+=ifx p.context==1 then 1.5 else -1.0;}
    return 0.5+z/(2*(1+abs(z)));
}
// Without a check that can run, the agent cannot tell a wrong answer from a
// right one, so it never retries.
sr_trial :: (seed:u64,p:SrPolicy,task:SrTask,index:int,trial:int)->SrTrial {
    q:=sr_chance(p,task);
    attempts:=ifx p.verify && task.checkable then 1+p.retries else 1;
    r:=SrTrial.{attempts=attempts};
    for a:0..<attempts {
        r.cost+=SR_PRICE[cast(int)p.model]*(ifx p.context==1 then SR_LONG else 1.0)+(ifx p.verify then SR_CHECK else 0.0);
        if cast(int)(tb_uniform(seed,cast(u64)((index*16+trial)*4+a))+q)>0 {r.solved=true;r.attempts=a+1;return r;}
    }
    return r;
}
```

The world is simulated, so its answers are known. Its prices and skills are
written down first:
- **Mammoth** is 1.5 more skilled than Elephant and costs 6 times as much a call.
- **Long context** doubles the price. It adds 1.5 where a task needs it, and
  short context loses 1.0 there.
- **A check** costs 0.5 an attempt. A retry happens only when a check exists
  and can run on the task.

Of the 60 development tasks, 15 need long context and 29 can be checked. A
task's score is its success rate less 0.04 per unit of mean cost.

## Measuring a policy

<!-- source: psionic/lib/search.coax measure -->
```jai
// A tier's measurement: each task tried trials times; its cell holds the
// success rate, the mean cost and the score. The spend is every unit paid;
// the score is the cells' scores summed in task order.
sr_measure :: (seed:u64,p:SrPolicy,tasks:[]SrTask,trials:int,out:[]SrCell)->SrMeasure #export_c {
    status:=sr_tasks_ok(tasks);if status!=SrStatus.ok {return SrMeasure.{status=status};}
    if trials<1 || trials>SR_TRIALS_MAX || !sr_policy_ok(p) {return SrMeasure.{status=SrStatus.shape};}
    if out.count<tasks.count {return SrMeasure.{status=SrStatus.capacity};}
    r:=SrMeasure.{};
    for task,i:tasks {
        solved:=0;total:=0.0;
        for t:0..<trials {x:=sr_trial(seed,p,task,i,t);if x.solved {solved+=1;}total+=x.cost;r.spend+=x.cost;}
        success:=f64(solved)/f64(trials);cost:=total/f64(trials);
        out[i]=SrCell.{success=success,cost=cost,score=success-SR_LAMBDA*cost};
        r.score+=out[i].score;
    }
    return r;
}
```

A measurement is a tier: the first `tasks` tasks, each tried `trials` times
under the tier's seed. Every unit paid is added to the spend. The cheapest
tier tells good policies from bad ones but not near rivals apart. Only the
last tier can do that, so only a few policies reach it.

## Ranking, and a task's lone best

<!-- source: psionic/lib/search.coax rank -->
```jai
// Positions by total, highest first, the earlier position first on ties.
sr_rank :: (total:[]f64,order:[]int) {
    for j:0..<total.count {
        q:=j;
        while q>0 && total[j]>total[order[q-1]] {order[q]=order[q-1];q-=1;}
        order[q]=j;
    }
}
// The position that alone holds the best score on task i of m, or -1 when
// the best is shared.
sr_lone_best :: (cells:[]SrCell,count:int,m:int,i:int)->int {
    for j:0..<count {
        alone:=true;
        for q:0..<count {if q!=j && cells[q*m+i].score>=cells[j*m+i].score {alone=false;}}
        if alone {return j;}
    }
    return -1;
}
// Keeps every position that alone holds the best score on one of m tasks;
// returns how many were not kept already.
sr_frontier :: (cells:[]SrCell,count:int,m:int,keep:[]bool)->int {
    added:=0;
    for i:0..<m {
        j:=sr_lone_best(cells,count,m,i);
        if j>=0 && !keep[j] {keep[j]=true;added+=1;}
    }
    return added;
}
```

Ties are ranked by position, and positions follow the expansion order. So
between equal policies the simpler one, found first, goes first.

A policy can rank low overall and still be the only best on one task. That
policy is worth measuring again: it is the per-task frontier, and dropping it
would lose the one thing it does better. A best shared by two policies keeps
nobody: either of them would do.

## Successive halving

<!-- source: psionic/lib/search.coax halve -->
```jai
// Each tier measures every policy still alive on its first tasks. Policies
// are ranked by their total score, the lower number first on ties. All but
// the last tier keep the best keeps[k], and any other policy that alone holds
// the best score on some task. The last tier's first is the winner.
sr_search :: (entries:[]SrEntry,tasks:[]SrTask,tiers:[]SrTier,keeps:[]int,rounds:[]SrRound)->SrSearch #export_c {
    n:=entries.count;
    if n<1 || n>SR_POLICIES_MAX || tiers.count<1 || tiers.count>SR_TIERS_MAX || keeps.count!=tiers.count-1 {return SrSearch.{status=SrStatus.shape};}
    status:=sr_tasks_ok(tasks);if status!=SrStatus.ok {return SrSearch.{status=status};}
    for e:entries {if !sr_policy_ok(e.policy) {return SrSearch.{status=SrStatus.shape};}}
    for t:tiers {if t.tasks<1 || t.tasks>tasks.count || t.trials<1 || t.trials>SR_TRIALS_MAX {return SrSearch.{status=SrStatus.shape};}}
    for k:keeps {if k<1 {return SrSearch.{status=SrStatus.shape};}}
    if rounds.count<tiers.count {return SrSearch.{status=SrStatus.capacity};}
    alive:[SR_POLICIES_MAX]int;count:=n;for i:1..<n {alive[i]=i;}   // alive[0] is 0 already
    cells:[SR_POLICIES_MAX*SR_TASKS_MAX]SrCell;total:[SR_POLICIES_MAX]f64;
    r:=SrSearch.{};
    for tier,k:tiers {
        m:=tier.tasks;
        for j:0..<count {
            e:=alive[j];x:=sr_measure(tier.seed,entries[e].policy,tasks[0..<m],tier.trials,cells[j*m..<j*m+m]);
            r.spend+=x.spend;total[j]=x.score;
        }
        order:[SR_POLICIES_MAX]int;sr_rank(total[0..<count],order[..]);
        round:=SrRound.{measured=count,best=alive[order[0]],spend=r.spend};
        r.winner=round.best;
        if k<keeps.count {
            keep:[SR_POLICIES_MAX]bool;
            for j:0..<count {if j<keeps[k] {keep[order[j]]=true;round.kept+=1;}}
            round.frontier=sr_frontier(cells[..],count,m,keep[..]);
            next:=0;for j:0..<count {if keep[j] {alive[next]=alive[j];next+=1;}}
            count=next;
        }
        rounds[k]=round;r.rounds+=1;
    }
    return r;
}
```

The example runs three tiers:

| Round | Policies | Tasks × trials | Best | Kept | Lone bests | Spend so far |
| --- | ---: | --- | --- | ---: | ---: | ---: |
| 1 | 32 | 12 × 1 | #19 | 16 | 0 | 2,395.5 |
| 2 | 16 | 30 × 2 | #30 | 8 | 1 | 6,047 |
| 3 | 9 | 60 × 4 | #30 | | | 11,479.5 |

Round 1 ranks #19 first: Elephant, one retry, long context, checked. On 12
tasks tried once, that is noise as much as skill. Round 2 corrects it. Round 2
also keeps #25 (Elephant, three retries, short context, checked) although it
ranks outside the top 8, because it alone is best on one task. In round 3 it
finishes third.

The winner is #30: Elephant, three retries, long context, checked. It scores
0.6117 a task, with a 75% success rate at a mean cost of 3.458. The simplest
policy scores 0.46. Mammoth on its own scores 0.4725: it succeeds more often
(71% against 50%), but costs six times as much. Mammoth with every option
scores only 0.3083.

To check the search, the example also measures all 32 policies on the full
tier. That would have cost 45,490, against the search's 11,479.5, and it picks
the same winner.

## What the per-task best does and does not say

Against that full measurement, another policy scores higher than the winner
on 55 of the 60 tasks. The best score on each task averages 0.7206, against
the winner's 0.6117.

That gap looks large, but it overstates what is on the table. Each task's
best is the largest of 32 noisy estimates from four trials each, so it is
biased upward. And to collect it, the agent would have to know in advance
which policy suits which task. Learning that is a router's job, not a
search's: see [chapter 223](ch223-learn-which-model-to-call.md). The gap
is an upper bound on what routing could add, not a result.

## Reading the held-out tasks once

The winner and its digest are written down. Only then are 60 new tasks drawn
and read once:
- **the comparison:** the simplest policy and the winner, four trials a task;
- **the floor:** the winner measured 16 times under different seeds gives
  [chapter 238](ch238-how-far-a-metric-moves-by-itself.md)'s floor, the
  amount the mean moves by itself.

| Policy | Score per task |
| --- | ---: |
| simplest | 0.3725 |
| winner (#30) | 0.5558 |

The gain is 0.1833 a task against a floor of 0.0834. Of 1,000 bootstrap
replicates over tasks, 7 are at or below no gain. The gate says **passed
(beyond the floor)**. On the development tasks the gain was 0.152 (0.6117
against 0.46). The held-out gain is no smaller, so nothing here suggests the
search only fitted the tasks it saw.

## Supported profile

`cpu-f64-search-v1`:
- binary64, with plain sums in a fixed order;
- up to 64 policies, 256 tasks, 4 tiers and 16 trials a task;
- world-sim-v1 on chapter 103's SplitMix64 counters;
- policy digests under the tag `psionic.policy.v1`, in chapter 237's encoding.

The search reads policies, tasks and tiers and writes rounds; a measurement
reads tasks and writes cells. Inputs and outputs have different types, so the
native adapter only names native arrays as each kind of span.

## How it is checked

- **Coax checks:** 111 checks in debug and release, counting the units
  it loads. They cover:
  - digests against the oracle, and distinct for distinct policies;
  - each operator, including at its limit;
  - the full expansion: every lineage, every parent earlier than its child,
    and every operator at or after its parent's;
  - the world and the chance formula by hand, and retries only with a check
    that can run;
  - measured cells, spends and summed scores against the oracle;
  - ranking with ties, and the lone best with a shared best;
  - a two-tier search with 4, 1 and 32 kept, against the oracle;
  - every refusal and bound.
- **An independent oracle,** [search_oracle.py](../tests/search_oracle.py),
  writes the manifests, operators, world, tiers, halving and lone-best rule
  again, and takes the floor and gate from chapters 238 and 239's oracles.
- **The native harness** [search.c](../tests/search.c) runs the whole
  experiment through the adapter, plain and under ASan/UBSan, in debug and
  release. Policies, digests, tasks, rounds, spends, winners, counts and
  verdicts must be equal. Means, the floor and the gain must agree within
  10⁻¹².
- **Mutation testing** of `search.coax` kills all 2,295 mutants. The
  first run left 23 survivors, fixed in the code or the checks:
  - **Duplicates:** a scan for an existing digest could skip the simplest
    policy without changing anything, since no operator leads back to it. The
    expansion now grows each policy only by its own operator or a later one,
    so no scan is needed.
  - **Ranking, the lone best and the frontier** moved into their own
    functions, tested directly on ties, shared bests and a lone best at the
    first position.
  - **A copy of the scores** could skip the first task unnoticed. Each
    policy's cells are now measured straight into the search's table.
  - **A constant in the totals** shifted every policy alike and changed no
    rank. The totals now come from the measurement's summed score, which the
    checks pin.
  - **Bounds:** one task, 256 tasks, an output exactly as long as the input,
    and the largest number of trials are now accepted in the checks.

The [verification record](../../docs/verification/psionic-ch183-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **A simulated world.** Real tasks come from chapter 237's suites, and real
  measurements spend real calls.
- **Four fields.** Prompts, tools or sampling settings would each need their
  own operators.
- **Fixed tiers.** The tiers and the number kept are written down in advance.
  A budget that adapts to how close the rivals are is not here.
- **One policy for every task.** Choosing a policy per task is chapter 223's
  router.
