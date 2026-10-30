# 187. Is the loop improving?

Chapters 178 to 186 built the parts of an improvement loop:
- a fixed suite;
- a noise floor;
- gates;
- environments with checkers;
- a policy search;
- training rows that keep held-out groups apart.

Wired together, they give a loop that proposes changes, grades them and
adopts the best. Such a loop always looks busy. Whether it improves anything
is a separate question, and each component it adds costs money and
settings, whether or not it helps.

This chapter answers that question with four rules:
- **One number per cycle:** the cost per accepted result, measured on a
  fixed set. The set is named by its digest, so two runs on different sets
  are refused, not compared.
- **An ablation for every component:** each is removed in turn under the
  same seeds. It stays only if removing it raises the cost by more than
  twice the standard error.
- **A complexity ledger:** each component's settings and spend, next to
  what it buys.
- **Shared budgets:** the proposer, reflector, grader and student each draw
  on their own share of one budget. A cycle starts only when every share can
  pay for it.

A loop that does not improve is reported as such.

```shell
coax/bin/coax run psionic/examples/loop.coax
coax/bin/coax test psionic/lib/loop.coax
```

```jai
fixed set: 64 tasks, digest 755c6ead; grading: 24 items; 8 cycles, 8 replicate seeds
replicate 1, cost per accepted result by cycle: 2.3902 2.3902 2.3902 2.2326 2.2326 2.2326 2.2326 1.84 1.84
  adopted: - - #0 - - - #1 -
mean over 8 replicates by cycle: 2.2349 2.1637 2.0392 2.0014 1.9561 1.9394 1.9394 1.8384 1.8131
is the loop improving? first less last 0.4218 per accepted result, standard error 0.0682 -> improving
spent per run: proposer 64.0 of 64.0, reflector 32.0 of 32.0, grader 1536.0 of 1536.0, student 834.5 of 1152.0 (mean)
break-even: the loop's 1632.0 units are paid back after 3870 accepted results at the lower cost
ablations, same seeds:
  proposer: 1 setting, spends 64.0 a run; removing it costs 0.4218 +- 0.0682 per accepted result -> earns its place
  reflector: 1 setting, spends 32.0 a run; removing it costs 0.0658 +- 0.0495 per accepted result -> within noise, remove it
  grader: 2 settings, spends 1536.0 a run; removing it costs 1.3592 +- 0.5673 per accepted result -> earns its place
complexity ledger: 3 of 4 settings earn their place; 32.0 of 1632.0 units a run go to components that do not
barren world, proposals in [-0.6, 0): first less last 0.0 +- 0.0, 0 adopted -> flat; 1632.0 units a run bought nothing
grader's share cut to 1500.0: budget stop before cycle 8, the grader share short
comparing against runs on another fixed set: moved
```

The [example](../examples/loop.coax) runs in under a second.

## A world with a student in it

<!-- source: psionic/lib/loop.coax world -->
```jai
// loop-sim-v1. A set of tasks draws difficulties in [-2, 2]. The student
// solves a task of difficulty d with skill s at 0.5+z/(2(1+|z|)), z = s-d.
lp_tasks :: (seed:u64,out:[]f64)->LpStatus #export_c {
    if out.count<1 || out.count>LP_TASKS_MAX {return LpStatus.shape;}
    for i:0..<out.count {out[i]=4*tb_uniform(seed,cast(u64)i)-2;}
    return LpStatus.ok;
}
lp_chance :: (skill:f64,d:f64)->f64 {z:=skill-d;return 0.5+z/(2*(1+abs(z)));}
// Draws come from four streams of one seed. The student's draws on the fixed
// set depend on the task and the attempt but not the cycle, so a cycle that
// changes nothing measures exactly what the one before it did.
lp_draw :: (seed:u64,stream:int,index:int)->f64 {return tb_uniform(seed,cast(u64)(index*4+stream));}
// A draw u solves at chance p when u+p reaches 1.
lp_solves :: (u:f64,p:f64)->bool {return cast(int)(u+p)>0;}
```

The world is simulated, so what each proposal really does is known:
- **The student** is one number, its skill. A fixed set of 64 tasks and a
  grading set of 24 items each draw difficulties in [-2, 2].
- **A proposal** changes the skill by an effect drawn uniformly from
  [-0.6, 0.4). Most proposals hurt, which is the usual case for proposed
  changes.
- **The reflector** rewrites each proposal before it is graded. Here a
  rewrite moves the effect by at most 0.05 either way and is no better on
  average. That is deliberate: the ledger has to catch a component that
  costs money and does nothing.

The student's draws on the fixed set depend on the task and the attempt but
not on the cycle. A cycle that adopts nothing therefore measures exactly
what the cycle before it did. A change in the number comes from a change in
the student, not from fresh luck on the same tasks, which is
[chapter 179](ch179-how-far-a-metric-moves-by-itself.md)'s problem removed
at the source.

## One cycle

<!-- source: psionic/lib/loop.coax cycle -->
```jai
// The student answers every fixed task, retrying once when the check fails.
lp_measure :: (seed:u64,fixed:[]f64,skill:f64)->LpCycle {
    c:=LpCycle.{skill=skill,took=-1};
    for d,i:fixed {
        p:=lp_chance(skill,d);
        for a:0..<LP_ATTEMPTS {
            c.attempts+=1;
            if lp_solves(lp_draw(seed,0,i*LP_ATTEMPTS+a),p) {c.accepted+=1;break;}
        }
    }
    c.cost=ifx c.accepted>0 then LP_ANSWER*f64(c.attempts)/f64(c.accepted) else LP_BIG;
    return c;
}
// One cycle's improvement: proposals, rewritten when the reflector is on;
// each graded on the grading items against the current skill on the same
// draws, the first largest gain adopted when it reaches the threshold; with no
// grader the first proposal is adopted blindly.
LpStep :: struct {took:int;effect:f64;spent:LpBudget;}
lp_improve :: (seed:u64,grading:[]f64,world:LpWorld,l:LpLoop,cycle:int,skill:f64,spent:LpBudget)->LpStep {
    r:=LpStep.{took=-1,spent=spent};
    if !l.proposer {return r;}
    effect:[LP_CANDIDATES_MAX]f64;
    for j:0..<l.candidates {
        at:=cycle*LP_CANDIDATES_MAX+j;
        effect[j]=world.low+world.width*lp_draw(seed,1,at);r.spent.proposer+=LP_PROPOSE;
        if l.reflector {effect[j]+=LP_NUDGE*(lp_draw(seed,2,at)-0.5);r.spent.reflector+=LP_REWRITE;}
    }
    if !l.grader {r.took=0;r.effect=effect[0];return r;}
    best:=l.threshold-1;
    for j:0..<l.candidates {
        gain:=0;
        for k:0..<l.items {
            u:=lp_draw(seed,3,(cycle*LP_CANDIDATES_MAX+j)*LP_ITEMS_MAX+k);
            if lp_solves(u,lp_chance(skill+effect[j],grading[k])) {gain+=1;}
            if lp_solves(u,lp_chance(skill,grading[k])) {gain-=1;}
            r.spent.grader+=2*LP_GRADE;
        }
        if gain>best {best=gain;r.took=j;}
    }
    if r.took>=0 {r.effect=effect[r.took];}
    return r;
}
```

The student answers each fixed task and retries once when the check fails.
The cycle's cost per accepted result is its attempts divided by its
accepted answers. A student that solves nothing has no finite cost and is
given 10^300.

The grader scores each candidate on all 24 grading items, against the
current student, with the same draw for both. The candidate with the
largest gain is adopted if that gain reaches the threshold of 2; on a tie
the first one wins. With no grader, the first proposal is adopted unseen.

Prices are written down:

| Work | Price |
| --- | --- |
| A proposal | 2 |
| A rewrite | 1 |
| A graded answer | 1 |
| A student attempt | 1 |

Grading is the expensive part: 4 candidates × 24 items × 2 answers is 192
units a cycle.

## A run, on a budget

<!-- source: psionic/lib/loop.coax run -->
```jai
// Each component's worst case for one cycle; a cycle starts only when every
// share still covers it, and otherwise the run stops and names the component.
lp_need :: (l:LpLoop,fixed:int,improve:bool)->LpBudget {
    n:=LpBudget.{student=LP_ANSWER*f64(LP_ATTEMPTS*fixed)};
    if improve && l.proposer {
        n.proposer=LP_PROPOSE*f64(l.candidates);
        if l.reflector {n.reflector=LP_REWRITE*f64(l.candidates);}
        if l.grader {n.grader=2*LP_GRADE*f64(l.candidates*l.items);}
    }
    return n;
}
lp_short :: (budget:LpBudget,spent:LpBudget,need:LpBudget)->int {
    if spent.proposer+need.proposer>budget.proposer {return 0;}
    if spent.reflector+need.reflector>budget.reflector {return 1;}
    if spent.grader+need.grader>budget.grader {return 2;}
    if spent.student+need.student>budget.student {return 3;}
    return -1;
}
lp_loop_ok :: (l:LpLoop,grading:int)->bool {
    return l.cycles>=1 && l.cycles<=LP_CYCLES_MAX && l.candidates>=1 && l.candidates<=LP_CANDIDATES_MAX &&
        l.items>=1 && l.items<=grading && l.threshold>=1 && l.threshold<=l.items;
}
// Cycle 0 measures the student as it is; cycles 1 to l.cycles each improve,
// then measure. out[c] holds cycle c.
lp_run :: (seed:u64,fixed:[]f64,grading:[]f64,world:LpWorld,l:LpLoop,budget:LpBudget,out:[]LpCycle)->LpRun #export_c {
    r:=LpRun.{seed=seed,cycles=-1};
    if fixed.count<1 || fixed.count>LP_TASKS_MAX || grading.count>LP_ITEMS_MAX || !lp_loop_ok(l,grading.count) ||
        out.count<=l.cycles || !(world.width>0 && world.width<LP_BIG && abs(world.low)<LP_BIG) {r.status=LpStatus.shape;return r;}
    r.set=su_values(fixed);skill:=0.0;
    for c:0..<l.cycles+1 {
        short:=lp_short(budget,r.spent,lp_need(l,fixed.count,c>0));
        if short>=0 {r.status=LpStatus.budget;r.stopped=cast(LpComponent)short;return r;}
        step:=LpStep.{took=-1,spent=r.spent};
        if c>0 {step=lp_improve(seed,grading,world,l,c,skill,r.spent);}
        r.spent=step.spent;skill+=step.effect;
        out[c]=lp_measure(seed,fixed,skill);out[c].took=step.took;
        r.spent.student+=LP_ANSWER*f64(out[c].attempts);
        r.cycles=c;
    }
    r.first=out[0].cost;r.last=out[l.cycles].cost;
    return r;
}
```

Cycle 0 measures the student as it is. Each later cycle improves, then
measures. Before a cycle starts, every component's worst case for it is
checked against what is left of that component's share:
- the student's worst case is two attempts on each fixed task;
- the proposer's, reflector's and grader's worst cases are fixed per cycle.

When a share can't pay, the run stops and names the component. In the
example, a grader's share of 1,500 units pays for 7 cycles of 192, and the
run stops before cycle 8.

The shares in the example are exactly what 8 cycles need: 64, 32, 1,536
and 1,152. The student's share holds its worst case; on average it spends
834.5 of it.

## Judging the loop and its components

<!-- source: psionic/lib/loop.coax judge -->
```jai
// The mean and standard error of a[i]-b[i] over paired runs.
lp_paired :: (a:[]f64,b:[]f64)->LpStat {
    if a.count<2 || a.count>LP_RUNS_MAX || b.count!=a.count {return LpStat.{status=LpStatus.shape};}
    n:=f64(a.count);total:=0.0;
    for x,i:a {total+=x-b[i];}
    mean:=total/n;squares:=0.0;
    for x,i:a {d:=x-b[i]-mean;squares+=d*d;}
    return LpStat.{mean=mean,se=sqrt(squares/(n-1)/n)};
}
// A difference counts when it is more than twice its standard error.
lp_verdict :: (s:LpStat)->LpVerdict {
    if s.mean>2*s.se {return LpVerdict.improving;}
    if s.mean< -2*s.se {return LpVerdict.worse;}
    return LpVerdict.flat;
}
// Runs compare only on the same fixed set, seed and number of cycles.
lp_paired_runs :: (a:[]LpRun,b:[]LpRun)->bool {
    if a.count!=b.count {return false;}
    for x,i:a {
        y:=b[i];
        if x.status!=LpStatus.ok || y.status!=LpStatus.ok || !su_same(x.set,y.set) || x.seed!=y.seed || x.cycles!=y.cycles {return false;}
    }
    return true;
}
// Is the loop improving? The first cycle's cost per accepted result less the
// last's, over replicate runs.
lp_improvement :: (runs:[]LpRun)->LpStat #export_c {
    if !lp_paired_runs(runs,runs) {return LpStat.{status=LpStatus.moved};}
    first:[LP_RUNS_MAX]f64;last:[LP_RUNS_MAX]f64;
    for r,i:runs {first[i]=r.first;last[i]=r.last;}
    return lp_paired(first[0..<runs.count],last[0..<runs.count]);
}
// A component's ledger entry: what removing it does to the last cycle's cost
// per accepted result (ablated less full, so a positive gain means the
// component earns its place), its settings, and the mean it spent.
lp_entry :: (component:LpComponent,settings:int,full:[]LpRun,ablated:[]LpRun)->LpEntry #export_c {
    e:=LpEntry.{component=component,settings=settings};
    if component==LpComponent.student {e.status=LpStatus.shape;return e;}
    if !lp_paired_runs(full,ablated) {e.status=LpStatus.moved;return e;}
    a:[LP_RUNS_MAX]f64;b:[LP_RUNS_MAX]f64;
    for r,i:full {
        a[i]=ablated[i].last;b[i]=r.last;
        e.spend+=ifx component==LpComponent.proposer then r.spent.proposer else ifx component==LpComponent.reflector then r.spent.reflector else r.spent.grader;
    }
    e.gain=lp_paired(a[0..<full.count],b[0..<full.count]);e.status=e.gain.status;
    e.spend/=f64(full.count);e.verdict=lp_verdict(e.gain);
    return e;
}
```

Every configuration runs under the same 8 replicate seeds. Run `i` of one
configuration and run `i` of another share their draws, so their difference
is paired. A difference counts only when its mean is more than twice its
standard error. Within that band the verdict is `flat`.

The rule is used twice:
- **Is the loop improving?** The first cycle's cost per accepted result
  less the last's. The full loop goes from 2.2349 to 1.8131, a gain of
  0.4218 with a standard error of 0.0682, so it is improving.
- **Does a component earn its place?** The last cycle's cost with the
  component removed, less the full loop's:

  | Component | Removing it costs | Verdict |
  | --- | --- | --- |
  | Proposer | 0.4218 ± 0.0682 | earns its place |
  | Grader | 1.3592 ± 0.5673 | earns its place: a blind loop adopts mostly harmful changes |
  | Reflector | 0.0658 ± 0.0495 | within noise, remove it |

The ledger adds it up. Three of the four settings earn their place. The
reflector's one setting and its 32 units a run do not: it is about 2% of
the loop's spend, buying nothing that can be measured.

Before comparing, both functions check that every pair of runs finished,
used the same fixed set (by digest), the same seed and the same number of
cycles. Comparing against runs on another fixed set returns `moved`, not a
number.

## When a loop does not improve

The same loop in a barren world, where every proposal's effect is drawn
from [-0.6, 0), adopts nothing in 64 cycles. Its cost per accepted result
does not move, so the verdict is `flat`. It still spent 1,632 units a run
on proposing and grading. The report says so, instead of showing activity
as progress.

A loop that improves also has to pay for itself. The full loop spends 1,632
units a run and saves 0.4218 units per accepted result. It breaks even after
3,870 accepted results. Below that volume, the loop costs more than it
saves.

## Supported profile

`cpu-f64-loop-v1`:
- binary64, plain sums in a fixed order;
- up to 64 fixed tasks, 32 grading items, 8 candidates a cycle, 16 cycles
  and 32 replicate runs;
- the world `loop-sim-v1`, written down above;
- a difference counts at more than twice its paired standard error.

## How it is checked

- **Coax checks:** the chapter's 11 checks, with the 101 that `loop.coax` loads from chapters
  178 and 70, all 112 passing in debug and release. They cover:
  - set digests;
  - the solve rule, measurement, and a student who solves nothing;
  - the grader's choice: the first largest gain, and a gain that reaches the
    threshold without passing it;
  - full and ablated runs, pinned cycle by cycle;
  - every share's stop, at exactly its limit and one unit under;
  - each refusal of a loop that can't run;
  - paired statistics and their verdicts;
  - comparisons that refuse a different set, seed, cycle count or status.
- **An independent oracle,** [loop_oracle.py](../tests/loop_oracle.py),
  rewrites the world, the cycle, the budgets and the statistics, with
  `hashlib` for the set digest. It rebuilds the example's whole output and
  matches it line for line.
- **Mutation testing** of `loop.coax`: 2,285 of 2,285 mutants
  killed.

The [verification record](../../docs/verification/psionic-ch187-2026-10-30.md)
has the commands and results.

## What this does not do yet

- **Real components.** The proposer, reflector, grader and student are
  simulated. Chapter 184's prompt evolution, chapter 185's judges and
  chapter 174's students are what this ledger is for.
- **Chapter 180's gate.** Verdicts here use a plain two-standard-error rule
  over paired replicates. Chapter 180's gate would add the noise floor and
  the right to say `unverifiable`.
- **Moving budget between components.** Shares are fixed when the run
  starts. A share freed by an ablation is left unspent, not given to the
  student.
