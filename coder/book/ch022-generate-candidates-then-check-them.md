# 22. Generate candidates, then check them

> **Status: partly implemented.** Best-of-N selection with exact checks,
> budgets, pass@k and Wilson intervals runs on a simulated sampler. Real
> model candidates, sandboxed runs, the repair strategy and wall-clock cost
> remain; see [What remains](#what-remains).

A model can write several answers to one task, and a program can check
which of them is right. When the check is exact and cheap compared with
generating an answer, drawing more candidates and keeping one that passes
turns a weak sampler into a stronger system. This chapter builds that loop
and measures what it buys. It answers three separate questions for every
setting:
- **any right:** did any of the N candidates solve the task?
- **chosen right:** did the selection pick a right one?
- **chosen wrong:** did the selection accept a wrong one?

The pieces are:
- **a seeded sampler** that draws candidates for
  [Psionic chapter 242](../../psionic/book/ch242-environments-with-checkers.md)'s tasks, each candidate
  fixed by its seed, task and number;
- **checks with a cost:** a length screen, a weaker visible check, and
  Psionic chapter 242's exact checker;
- **a selection** that checks candidates cheapest first, stops at the first
  one that passes, never spends past its budget, and never chooses a
  candidate the check did not pass;
- **pass@k:** the unbiased estimator of the chance that k candidates hold a
  right one, and Wilson intervals on what the selection chose;
- **a record** of each run with a digest, which a replay from the same
  seeds must reproduce.

```shell
coax/bin/coax run psionic/examples/candidates.coax
coax/bin/coax test psionic/lib/harness/candidates.coax
coax/bin/coax test psionic/tests/candidates-checks.coax
```

## The profile

`cpu-int-candidates-v1`:
- Psionic chapter 242's task-sim-v1 tasks: sort, remove repeats, or running sums
  over 4 to 16 values from 0 to 9;
- up to 8 candidates per task, from sampler-sim-v1 on Psionic chapter 242's
  agent-sim-v1;
- check costs in units: 1 for the length screen, 1 plus half the input
  (rounded up) for the visible check, and 1 plus the input for the exact
  check;
- binary64 for pass@k and for Wilson intervals at z = 1.96;
- run digests under the tag `psionic.candidates.v1`, in
  [Psionic chapter 239](../../psionic/book/ch239-a-suite-is-three-digests.md)'s encoding.

## A sampler with seeds

<!-- source: psionic/lib/harness/candidates.coax sample -->
```jai
// sampler-sim-v1. Task r's chance that one candidate is right is 2 skill u,
// with u drawn on counter 2^40 + r, so tasks run from hopeless to easy and
// average skill. Candidate i is chapter 242's attempt i of rollout r at that
// chance: each candidate is fixed by (seed, r, i) alone.
cd_chance :: (s:CdSampler,r:int)->f64 {return 2*s.skill*tb_uniform(s.seed,CD_DIFFICULTY+cast(u64)r);}
cd_sample :: (s:CdSampler,t:EnTask,r:int,i:int)->EnWork {
    return en_attempt(EnAgent.{seed=s.seed,skill=cd_chance(s,r)},t,r,i);
}
```

The sampler is a stand-in for a model. It gives each task its own chance
that one candidate is right, from 0 up to twice the sampler's skill, so some
tasks are nearly hopeless and others easy. A wrong candidate is one of
Psionic chapter 242's three mistakes: one value bumped by one, the last value
dropped, or the input handed back unchanged.

Candidate i of task r depends on nothing but the seed, r and i. Two things
follow. Candidates are independent, which pass@k assumes. And drawing them
one at a time, only as the selection asks for them, gives the same
candidates as drawing all of them first, so a selection that stops early
also saves the generation it did not need.

The skill is at most 0.5, so the chance never passes 1. A sampler with a
higher skill is refused.

## Checks and their cost

<!-- source: psionic/lib/harness/candidates.coax check -->
```jai
// What one check costs, in units: the screen reads one number, the visible
// check half the input, the exact check all of it.
cd_cost :: (check:CdCheck,t:EnTask)->int {
    if check==CdCheck.length {return 1;}
    if check==CdCheck.visible {return 1+(t.work.size+1)/2;}
    return 1+t.work.size;
}
cd_passes :: (check:CdCheck,t:EnTask,w:EnWork)->bool {
    if check==CdCheck.exact {return en_check(EnChecker.exact,t,w);}
    if !en_check(EnChecker.length_only,t,w) {return false;}
    want:=en_solve(t);m:=ifx check==CdCheck.visible then (want.size+1)/2 else 0;
    for i:0..<m {if w.v[i]!=want.v[i] {return false;}}
    return true;
}
```

There are three checks, each with a cost:
- **length:** Psionic chapter 242's `length_only` checker. It reads one number, so
  it costs 1 unit. It cannot accept a right answer by itself: a candidate
  that passes it may still be wrong.
- **visible:** the length and the first half of the answer. This stands for
  the tests a repair step is allowed to see. It costs 1 plus half the input.
- **exact:** Psionic chapter 242's exact checker, which compares every value with
  the reference solution. It costs 1 plus the input, and it is the check
  that scores every strategy here.

Psionic chapter 242 would refuse the visible check as an environment's checker: a
near miss that changes the last value passes it. Here it is used on purpose,
to measure what selecting with a weaker check than the score costs.

## Selecting, cheapest first

<!-- source: psionic/lib/harness/candidates.coax select -->
```jai
// Candidates in order, each screened by length before the selection check
// runs. The first to pass is chosen, so ties go to the lower number. A check
// the budget cannot cover is not run: the selection stops there.
cd_select :: (t:EnTask,c:[]EnWork,check:CdCheck,budget:int)->CdPick #export_c {
    out:=CdPick.{pick=-1};
    if check==CdCheck.none {out.status=CdStatus.no_checker;return out;}
    if c.count<1 || c.count>CD_CANDIDATES_MAX || budget<0 {out.status=CdStatus.shape;return out;}
    k:=cd_cost(check,t);
    for w,i:c {
        out.examined=i+1;
        if check!=CdCheck.length {
            if out.cost+1>budget {out.status=CdStatus.budget;return out;}
            out.cost+=1;out.checks+=1;
            if !cd_passes(CdCheck.length,t,w) {continue;}
        }
        if out.cost+k>budget {out.status=CdStatus.budget;return out;}
        out.cost+=k;out.checks+=1;
        if cd_passes(check,t,w) {out.pick=i;return out;}
    }
    out.status=CdStatus.unsolved;return out;
}
```

The selection takes the candidates in order. Before the selection check runs
on a candidate, the length screen runs first, for 1 unit. A candidate of the
wrong length is dropped without paying for the larger check. This is
[Psionic chapter 244](../../psionic/book/ch244-search-a-policy-cheapest-first.md)'s idea in its smallest
form: measure everything cheaply, and spend fully only on what survives.

The first candidate that passes is chosen, so ties always go to the lower
number. The selection never spends past the budget: when the next check
would cost more than what is left, that check does not run, and the result
is `budget` with no candidate. It also never returns a candidate the check
did not pass. When every candidate fails, the result is `unsolved`. With no
checker at all, the selection refuses with `no_checker` before looking at
anything.

### By hand: the first held-out task

The held-out set is 60 tasks drawn from seed 212, a different seed from
Psionic chapter 242's. Its first task is sort [6 8 0 5 0], with answer [0 0 5 6 8].
The sampler is seed 2120 with skill 0.3. Its eight candidates are:

| Candidate | Work | Length | Visible | Exact |
| --- | --- | --- | --- | --- |
| 0 | [6 8 0 5 0], the input | yes | no | no |
| 1 | [0 0 5 6], last dropped | no | no | no |
| 2 | [6 8 0 5 0] | yes | no | no |
| 3 | [6 8 0 5 0] | yes | no | no |
| 4 | [0 0 5 6 8], right | yes | yes | yes |
| 5 | [6 8 0 5 0] | yes | no | no |
| 6 | [0 0 5 6] | no | no | no |
| 7 | [0 0 5 7 8], one value bumped | yes | yes | no |

The input has 5 values, so the screen costs 1, the visible check 1 + 3 = 4,
and the exact check 1 + 5 = 6. Selecting with the exact check and a budget
of 200:

| Candidate | Screen | Exact check | Spent so far |
| --- | --- | --- | --- |
| 0 | pass, 1 | fail, 6 | 7 |
| 1 | fail, 1 | not run | 8 |
| 2 | pass, 1 | fail, 6 | 15 |
| 3 | pass, 1 | fail, 6 | 22 |
| 4 | pass, 1 | pass, 6 | 29 |

Candidate 4 is chosen after 5 candidates, 9 checks and a cost of 29.
Candidates 5 to 7 are never generated. The five that were generated hold
5 + 4 + 5 + 5 + 5 = 24 values, the run's tokens.

With the visible check, each larger check costs 4 instead of 6: 5, 6, 11, 16
and 21, and candidate 4 is chosen again. Candidate 7 also passes the visible
check, though it is wrong. Here it is never reached. Without candidate 4, the
visible check would choose it.

With the exact check and a budget of 20, candidate 3's screen brings the
spend to 16. Its exact check would bring it to 22, past 20, so the check
does not run. The result is `budget` after 4 candidates, 6 checks, a cost of
16 and 19 tokens.

## pass@k, and why it is not 1 - (1 - c/n)^k

Drawing n candidates per task and counting c right ones gives more than one
number. The chance that k of them, drawn without replacement, hold a right
one is

pass@k = 1 - C(n-c, k) / C(n, k),

because C(n-c, k) of the C(n, k) ways to choose k are all wrong. Averaged
over tasks, this is an unbiased estimate of pass@k. The tempting
1 - (1 - c/n)^k draws with replacement and comes out too low.

Two examples by hand:
- **n = 5, c = 2, k = 2:** there are C(5, 2) = 10 pairs, and C(3, 2) = 3 of
  them are both wrong, so pass@2 = 1 - 3/10 = 0.7.
- **The first held-out task:** 1 of 8 candidates is right. pass@1 is 1/8,
  and pass@4 = 1 - C(7, 4)/C(8, 4) = 1 - 35/70 = 0.5. The formula with
  replacement gives 1 - (7/8)^4 = 1 - 2401/4096 ≈ 0.414.

<!-- source: psionic/lib/harness/candidates.coax estimate -->
```jai
// pass@k from n samples with c right: one minus the chance that k drawn
// without replacement are all wrong, C(n-c,k)/C(n,k), written as the
// product of (1 - k/i) for i from n-c+1 to n. A factor is 0 once n-c < k.
cd_pass_at :: (n:int,c:int,k:int)->CdValue #export_c {
    if n<1 || n>CD_SAMPLES_MAX || c<0 || c>n || k<1 || k>n {return CdValue.{status=CdStatus.shape};}
    q:=1.0;for i:n-c+1..<n+1 {q*=1-f64(k)/f64(i);}
    return CdValue.{value=1-q};
}
// The Wilson score interval for k of n at z = 1.96; at 0 and at n the
// bounds are exactly 0 and 1.
cd_wilson :: (k:int,n:int)->CdInterval #export_c {
    if n<1 || k<0 || k>n {return CdInterval.{status=CdStatus.shape};}
    m:=f64(n);p:=f64(k)/m;z2:=CD_Z*CD_Z;d:=1+z2/m;
    c:=(p+z2/(2*m))/d;h:=CD_Z*sqrt(p*(1-p)/m+z2/(4*m*m))/d;
    return CdInterval.{low=ifx k==0 then 0.0 else c-h,high=ifx k==n then 1.0 else c+h};
}
```

The code writes the ratio of binomials as a product,
C(n-c, k)/C(n, k) = (1 - k/(n-c+1)) ⋯ (1 - k/n). For n = 5, c = 2, k = 2
this is (1 - 2/4)(1 - 2/5) = 0.5 × 0.6 = 0.3. When n - c < k, one factor is
1 - k/k = 0, and pass@k is exactly 1.

A count of right choices among tasks gets a Wilson score interval. For 18 of
60: p = 0.3, z² = 3.8416, and the denominator is 1 + z²/60 = 1.064027. The
center is (0.3 + 3.8416/120)/1.064027 = 0.312036. The half-width is
1.96 × √(0.3 × 0.7/60 + 3.8416/14400)/1.064027 = 0.113055. The interval is
[0.1990, 0.4251]. At 0 and at n the bounds are set to exactly 0 and 1, since
the formula is off by rounding there.

## A run's record and its replay

<!-- source: psionic/lib/harness/candidates.coax run -->
```jai
cd_sampler_ok :: (s:CdSampler)->bool {return s.skill>=0 && s.skill<=CD_SKILL_MAX;}
// Draws n candidates for task r and selects among them. Only the candidates
// the selection examined count as generated: drawing them one at a time gives
// the same candidates, because each depends only on (seed, r, i). The digest
// starts from the task, the sampler and the settings, chains each examined
// candidate, and ends with the selection.
cd_run :: (s:CdSampler,t:EnTask,r:int,n:int,check:CdCheck,budget:int)->CdRun #export_c {
    out:=CdRun.{pick=CdPick.{status=CdStatus.shape,pick=-1}};
    if !cd_sampler_ok(s) || r<0 || n<1 || n>CD_CANDIDATES_MAX || budget<0 {return out;}
    c:[CD_CANDIDATES_MAX]EnWork;for i:0..<n {c[i]=cd_sample(s,t,r,i);}
    out.pick=cd_select(t,c[0..<n],check,budget);
    buf:[320]u8;at:=su_text(buf[..],0,"psionic.candidates.v1");buf[at]=cast(u8)t.kind;
    at=en_work(buf[..],at+1,t.work);at=su_u64(buf[..],at,s.seed);at=su_u64(buf[..],at,f64_bits(s.skill));
    at=su_u64(buf[..],at,cast(u64)r);at=su_u64(buf[..],at,cast(u64)n);buf[at]=cast(u8)check;
    at=su_u64(buf[..],at+1,cast(u64)budget);out.digest=su_hash(buf[0..<at]);
    for i:0..<out.pick.examined {
        out.tokens+=c[i].size;for j:0..<32 {buf[j]=out.digest[j];}
        out.digest=su_hash(buf[0..<en_work(buf[..],32,c[i])]);
    }
    for j:0..<32 {buf[j]=out.digest[j];}buf[32]=cast(u8)out.pick.status;
    at=su_u64(buf[..],33,cast(u64)(out.pick.pick+1));at=su_u64(buf[..],at,cast(u64)out.pick.checks);
    out.digest=su_hash(buf[0..<su_u64(buf[..],at,cast(u64)out.pick.cost)]);
    return out;
}
```

A run draws the candidates for one task and selects among them. Its digest
starts from the task, the sampler, the number of candidates, the check and
the budget. It then chains each candidate the selection examined, and ends
with the selection's result. Candidates that were never examined are not in
the record, since a run that stops early never generates them.

<!-- source: psionic/lib/harness/candidates.coax replay -->
```jai
cd_pick_same :: (a:CdPick,b:CdPick)->bool {
    return a.status==b.status && a.pick==b.pick && a.examined==b.examined && a.checks==b.checks && a.cost==b.cost;
}
cd_replay :: (s:CdSampler,t:EnTask,r:int,n:int,check:CdCheck,budget:int,claimed:CdRun)->bool #export_c {
    x:=cd_run(s,t,r,n,check,budget);
    return cd_pick_same(x.pick,claimed.pick) && x.tokens==claimed.tokens && su_same(x.digest,claimed.digest);
}
```

A replay runs again from the same seeds and accepts the record only when
every field and the digest come out the same. Bad settings are refused with
`shape` and an empty digest: a skill outside 0 to 0.5, a negative task
number, fewer than 1 or more than 8 candidates, or a negative budget.

## Strategies on the held-out set

<!-- source: psionic/lib/harness/candidates.coax tally -->
```jai
// How many of n samples for task r are right.
cd_right :: (s:CdSampler,t:EnTask,r:int,n:int)->int {
    c:=0;for i:0..<n {if cd_passes(CdCheck.exact,t,cd_sample(s,t,r,i)) {c+=1;}}
    return c;
}
// One strategy over a task set: task r is rollout r.
cd_tally :: (s:CdSampler,tasks:[]EnTask,n:int,check:CdCheck,budget:int)->CdTally #export_c {
    out:CdTally;
    for t,r:tasks {
        x:=cd_run(s,t,r,n,check,budget);p:=x.pick;
        if p.status==CdStatus.shape || p.status==CdStatus.no_checker {return CdTally.{status=p.status};}
        out.tasks+=1;out.checks+=p.checks;out.cost+=p.cost;out.tokens+=x.tokens;
        if cd_right(s,t,r,n)>0 {out.any+=1;}
        if p.status==CdStatus.budget {out.out+=1;}
        if p.status==CdStatus.ok {
            if cd_passes(CdCheck.exact,t,cd_sample(s,t,r,p.pick)) {out.chosen+=1;} else {out.wrong+=1;}
        }
    }
    return out;
}
// The mean of pass@k over a task set, n samples per task.
cd_pass_mean :: (s:CdSampler,tasks:[]EnTask,n:int,k:int)->CdValue #export_c {
    if !cd_sampler_ok(s) || tasks.count<1 || n>CD_CANDIDATES_MAX {return CdValue.{status=CdStatus.shape};}
    sum:=0.0;
    for t,r:tasks {
        v:=cd_pass_at(n,cd_right(s,t,r,n),k);if v.status!=CdStatus.ok {return v;}
        sum+=v.value;
    }
    return CdValue.{value=sum/f64(tasks.count)};
}
```

A tally runs one strategy over a task set, with task r as run r. It counts
the three answers from the introduction separately. "Any right" asks the
exact checker about all N candidates, whether or not the selection examined
them. "Chosen right" and "chosen wrong" ask it about the chosen candidate.

The chapter tunes nothing on the held-out set; it only measures it. Every
number below is frozen in
[candidates-checks.coax](../../psionic/tests/candidates-checks.coax). A change to the
sampler, the checks or the selection that moves any of them fails there.

What the run shows:
- **More candidates, more right answers.** With the exact check and enough
  budget, the chosen candidate is right whenever any candidate is: 18, 31,
  40 and 46 of 60 for 1, 2, 4 and 8 candidates.
- **pass@k agrees with best-of-N.** Estimated from all 8 candidates per
  task, pass@2 is 0.4524 and pass@4 is 0.6345, against 31/60 = 0.517 and
  40/60 = 0.667 measured from the first 2 and 4 candidates only. pass@8 is
  exactly 46/60, since with k = n the estimator is just whether any is right.
- **Beyond the interval.** Best of 8 chooses right on 46 of 60 tasks,
  [0.6456, 0.8556]; one candidate on 18, [0.1990, 0.4251]. The intervals do
  not overlap.
- **It costs more.** Best of 8 spends 2,010 check units and 2,121 tokens,
  against 576 and 579 for one candidate checked once: about 3.5 times the
  checking and 3.7 times the generation, for 2.6 times the right answers.
- **A budget trades both ways.** At 40 units per task, 19 tasks run out.
  Six of them had a right candidate past the budget, so 40 are chosen right
  instead of 46. The cost falls from 2,010 to 1,413.
- **A weaker check accepts wrong answers.** Selecting with the visible check
  costs 873 units, under half the exact check's. But it chooses a wrong
  candidate on 19 tasks, each one with the error in the second half of the
  answer, which the visible check does not read. That is the reason the
  score is a separate, exact check.

## Run it

The [example](../../psionic/examples/candidates.coax) works the hand checks first,
then the first held-out task: its candidates, the three selections and three
refusals (no checker, a sampler with skill 0.6, and pass@9 from 8 samples).
Then it runs every strategy on the held-out set and replays every best-of-8
run, and one changed record.

<!-- source: psionic/examples/candidates.coax example -->
```jai
main :: () {
    print("hand checks: pass@2 of 5 samples with 2 right %; pass@4 of 8 with 1 right %; Wilson 18 of 60 ",
        r4(cd_pass_at(5,2,2).value),r4(cd_pass_at(8,1,4).value));
    show_interval(cd_wilson(18,60));print("\n");

    // The held-out set and sampler, fixed before any run below.
    tasks:[60]EnTask;#assert en_tasks(212,tasks[..])==EnStatus.ok;
    s:=CdSampler.{seed=2120,skill=0.3};
    t:=tasks[0];
    print("the first held-out task: % ",KIND[cast(int)t.kind]);show_work(t.work);print(" -> ");show_work(en_solve(t));print("\n");
    for i:0..<8 {
        w:=cd_sample(s,t,0,i);print("  candidate % ",i);show_work(w);
        print(": length %, visible %, exact %\n",yes(cd_passes(CdCheck.length,t,w)),yes(cd_passes(CdCheck.visible,t,w)),yes(cd_passes(CdCheck.exact,t,w)));
    }
    for c:[2]CdCheck.[CdCheck.exact,CdCheck.visible] {print("  % check, budget 200: ",CHECK[cast(int)c]);show_run(cd_run(s,t,0,8,c,200));}
    print("  exact check, budget 20: ");show_run(cd_run(s,t,0,8,CdCheck.exact,20));
    print("  no checker: %\n",STATUS[cast(int)cd_run(s,t,0,8,CdCheck.none,200).pick.status]);
    print("  a sampler with skill 0.6: %\n",STATUS[cast(int)cd_run(CdSampler.{seed=2120,skill=0.6},t,0,8,CdCheck.exact,200).pick.status]);
    print("  pass@9 of 8 samples: %\n",STATUS[cast(int)cd_pass_at(8,1,9).status]);

    print("held-out: 60 tasks from seed 212, sampler seed 2120 with skill 0.3, 8 candidates each\n");
    print("  pass@1 %, pass@2 %, pass@4 %, pass@8 %\n",r4(cd_pass_mean(s,tasks[..],8,1).value),
        r4(cd_pass_mean(s,tasks[..],8,2).value),r4(cd_pass_mean(s,tasks[..],8,4).value),r4(cd_pass_mean(s,tasks[..],8,8).value));
    one:=cd_tally(s,tasks[..],1,CdCheck.exact,200);show_row("1 candidate:                ",one);
    show_row("best of 2, exact:           ",cd_tally(s,tasks[..],2,CdCheck.exact,200));
    show_row("best of 4, exact:           ",cd_tally(s,tasks[..],4,CdCheck.exact,200));
    eight:=cd_tally(s,tasks[..],8,CdCheck.exact,200);show_row("best of 8, exact:           ",eight);
    show_row("best of 8, exact, budget 40:",cd_tally(s,tasks[..],8,CdCheck.exact,40));
    show_row("best of 8, visible:         ",cd_tally(s,tasks[..],8,CdCheck.visible,200));
    a:=cd_wilson(one.chosen,60);b:=cd_wilson(eight.chosen,60);
    print("  best of 8 against 1 candidate: ");show_interval(b);print(" against ");show_interval(a);
    print(", %\n",ifx b.low>a.high then "better: the intervals do not overlap" else "not shown: the intervals overlap");

    replayed:=0;first:CdRun;
    for u,r:tasks {x:=cd_run(s,u,r,8,CdCheck.exact,200);if r==0 {first=x;}if cd_replay(s,u,r,8,CdCheck.exact,200,x) {replayed+=1;}}
    claimed:=first;claimed.pick.checks-=1;
    print("replay: % of 60 best-of-8 runs replay from their seeds; the first claimed with one check fewer: %\n",
        replayed,ifx cd_replay(s,t,0,8,CdCheck.exact,200,claimed) then "accepts" else "refuses");
}
```

```text
hand checks: pass@2 of 5 samples with 2 right 0.7; pass@4 of 8 with 1 right 0.5; Wilson 18 of 60 [0.199, 0.4251]
the first held-out task: sort [6 8 0 5 0] -> [0 0 5 6 8]
  candidate 0 [6 8 0 5 0]: length yes, visible no, exact no
  candidate 1 [0 0 5 6]: length no, visible no, exact no
  candidate 2 [6 8 0 5 0]: length yes, visible no, exact no
  candidate 3 [6 8 0 5 0]: length yes, visible no, exact no
  candidate 4 [0 0 5 6 8]: length yes, visible yes, exact yes
  candidate 5 [6 8 0 5 0]: length yes, visible no, exact no
  candidate 6 [0 0 5 6]: length no, visible no, exact no
  candidate 7 [0 0 5 7 8]: length yes, visible yes, exact no
  exact check, budget 200: chosen candidate 4, 5 examined, 9 checks, cost 29, 24 tokens, digest dcbbb14a5814c385
  visible check, budget 200: chosen candidate 4, 5 examined, 9 checks, cost 21, 24 tokens, digest d33d764ddb8893fb
  exact check, budget 20: refused: out of budget, 4 examined, 6 checks, cost 16, 19 tokens, digest 6ac30cf42c222f48
  no checker: refused: no checker
  a sampler with skill 0.6: refused: shape
  pass@9 of 8 samples: refused: shape
held-out: 60 tasks from seed 212, sampler seed 2120 with skill 0.3, 8 candidates each
  pass@1 0.2813, pass@2 0.4524, pass@4 0.6345, pass@8 0.7667
  1 candidate:                 any right 18, chosen right 18 [0.199, 0.4251], chosen wrong 0, out of budget 0, 106 checks, cost 576, 579 tokens
  best of 2, exact:            any right 31, chosen right 31 [0.3931, 0.6383], chosen wrong 0, out of budget 0, 178 checks, cost 959, 979 tokens
  best of 4, exact:            any right 40, chosen right 40 [0.5406, 0.7727], chosen wrong 0, out of budget 0, 269 checks, cost 1438, 1479 tokens
  best of 8, exact:            any right 46, chosen right 46 [0.6456, 0.8556], chosen wrong 0, out of budget 0, 377 checks, cost 2010, 2121 tokens
  best of 8, exact, budget 40: any right 46, chosen right 40 [0.5406, 0.7727], chosen wrong 0, out of budget 19, 295 checks, cost 1413, 1691 tokens
  best of 8, visible:          any right 46, chosen right 40 [0.5406, 0.7727], chosen wrong 19, out of budget 0, 264 checks, cost 873, 1437 tokens
  best of 8 against 1 candidate: [0.6456, 0.8556] against [0.199, 0.4251], better: the intervals do not overlap
replay: 60 of 60 best-of-8 runs replay from their seeds; the first claimed with one check fewer: refuses
```

The example takes about four seconds, most of it checking the units it
loads. It loads only Psionic chapter 242's `env.coax` and its own unit; the heavier
held-out checks live in a separate file it does not load.

## How it is checked

- **Coax checks** in [candidates.coax](../../psionic/lib/harness/candidates.coax),
  in debug and release, 125 counting the units it loads:
  - the sampler's chance and the first task's eight candidates;
  - each check and cost by hand, with odd and even answers;
  - every selection in the hand trace, the budget at 0, 8, 20, 28 and 29,
    and each refusal;
  - a `#forall` over 12 random tasks, samplers and candidate counts,
    against an oracle that scans for the first candidate each check passes.
    Without a budget, the selection must choose exactly that candidate, or
    report `unsolved` only when there is none. For every budget from 0 up,
    it must never spend past the budget, and must give the unlimited
    result once the budget covers it, and `budget` with no candidate
    before;
  - a `#forall` comparing pass@k with brute force: for every n up to 8,
    every c and every k, it lists every set of k positions and counts
    those that hold a right one;
  - pass@k and Wilson by hand, at their exact endpoints, and their refusals;
  - two pinned run digests; each input and each field of a record, which a
    replay must catch when changed; and refusals with an empty digest;
  - the first six held-out tasks: right candidates 1, 2, 2, 4, 0 and 6 of 8,
    three strategies' tallies, and the mean pass@k by hand: 15/48, 82/168,
    284/420 and 5/6.
- **Held-out checks** in
  [candidates-checks.coax](../../psionic/tests/candidates-checks.coax): the six
  strategies and pass@k on all 60 held-out tasks, a replay of every best-of-8
  run, and Wilson intervals for every k ≤ n ≤ 40, equal bit for bit to
  [Psionic chapter 246](../../psionic/book/ch246-judges-are-components.md)'s `jg_wilson`, which was
  written for a different chapter.
- **Mutation testing** of `candidates.coax` kills all 308 mutants. The
  first run left 4 survivors, fixed in the checks:
  - the brute-force pass@k counted bits only below n, so it could start at
    either mask or run past 2^n unnoticed; it now counts every bit, and is
    checked at k = 0;
  - the visible check's half was checked only on an odd answer; an even
    answer now tells (n + 1)/2 from (n + 2)/2;
  - the task's kind in the digest was only checked on a sort task, whose
    kind is 0; a second pinned digest is for a running-sums task.

## What remains

- **A real model.** The sampler is simulated. Drawing candidates from a
  model through [chapter1](ch001-the-simplest-loop.md)'s loop, with real
  token counts, remains.
- **Running candidates.** The tasks' answers are data checked by a pure
  function. Candidates that are programs, compiled and run in
  [chapter5](ch005-a-sandbox-and-approvals.md)'s sandbox, with
  compilation recorded apart from correctness, remain.
- **The repair strategy.** Generate, run the visible tests, read the
  failures and repair needs a model that reads its failures. The selection
  here never shows the exact check to anything, but no repair step exists
  yet to keep it from.
- **A learned verifier**, added only once its labels are trustworthy.
- **Wall-clock time and execution cost.** Cost here is check units and
  answer values, not seconds or money.
- **Psionic chapter 241's gate.** The comparison with one candidate uses whether
  Wilson intervals overlap, not Psionic chapter 241's verdicts with a noise floor.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
- [Psionic chapter 242](../../psionic/book/ch242-environments-with-checkers.md): the tasks, the
  exact checker and the simulated agent.
- [Psionic chapter 244](../../psionic/book/ch244-search-a-policy-cheapest-first.md): measuring
  cheaply first.
- Chen et al., *Evaluating Large Language Models Trained on Code* (2021):
  the unbiased pass@k estimator.
- Wilson, *Probable Inference, the Law of Succession, and Statistical
  Inference* (1927): the score interval.
