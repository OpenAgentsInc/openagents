# 108. Choose the next example to learn from

Labels cost money or time. Active learning lets a model pick which unlabeled
examples to label next. This chapter builds that loop on a pool of points we
own and asks one question: at the same label budget, does choosing which
labels to buy beat buying them at random?

The model is [chapter 103](ch103-learning-with-arrays.md)'s small
classifier. Four rules choose the labels:
- **random:** any point not yet labeled, uniformly;
- **uncertainty:** the point the current model is least sure about;
- **diversity:** the point farthest from every labeled point;
- **mixture:** the sum of the uncertainty and diversity scores.

Every rule starts from the same 8 random labels and buys 4 more a round until
it holds 40. The answer comes from
[chapter 240](ch240-how-far-a-metric-moves-by-itself.md)'s noise floor and
[chapter 241](ch241-gates-that-can-say-unverifiable.md)'s decision gate, not
from one number. On this pool all three rules end ahead of random selection,
but none is clearly beyond the noise floor. The gate says so: unverifiable.

```shell
coax/bin/coax run psionic/examples/choose-next-example.coax
coax/bin/coax test psionic/lib/acquire.coax
```

```text
pool: 240 points, 98 labeled 1 by the checker, 15 in the pocket
held out: 400 points in 40 groups of 10, 20 in the pocket
budget: 8 random labels, then 4 a round to 40; seeds 1 to 16, and random on seeds 1 to 64 for the floor
random: held-out accuracy at 8, 16, 24, 32 and 40 labels, mean (sd) over 16 seeds:
  0.797 (0.104) 0.844 (0.061) 0.874 (0.054) 0.872 (0.065) 0.892 (0.047)
  pocket: 38 of 512 bought labels, held-out pocket accuracy 0.428
  cost: 92800 training steps, 0 predictions, 0 distances; seeds 1 to 4 sealed: true
uncertainty, seed 1, labels 9 to 16 (point=label): 156=0.0 96=1.0 59=1.0 51=1.0 44=0.0 195=0.0 228=0.0 105=0.0
uncertainty: held-out accuracy at 8, 16, 24, 32 and 40 labels, mean (sd) over 16 seeds:
  0.797 (0.104) 0.882 (0.092) 0.897 (0.073) 0.914 (0.04) 0.927 (0.03)
  pocket: 24 of 512 bought labels, held-out pocket accuracy 0.138
  cost: 92800 training steps, 27904 predictions, 0 distances; seeds 1 to 4 sealed: true
diversity: held-out accuracy at 8, 16, 24, 32 and 40 labels, mean (sd) over 16 seeds:
  0.797 (0.104) 0.885 (0.028) 0.907 (0.034) 0.913 (0.02) 0.923 (0.012)
  pocket: 17 of 512 bought labels, held-out pocket accuracy 0.038
  cost: 92800 training steps, 0 predictions, 2561280 distances; seeds 1 to 4 sealed: true
mixture: held-out accuracy at 8, 16, 24, 32 and 40 labels, mean (sd) over 16 seeds:
  0.797 (0.104) 0.893 (0.035) 0.92 (0.022) 0.93 (0.014) 0.925 (0.034)
  pocket: 35 of 512 bought labels, held-out pocket accuracy 0.241
  cost: 92800 training steps, 27904 predictions, 2561280 distances; seeds 1 to 4 sealed: true
uncertainty against random at 40 labels: gain 0.035 over a floor of 0.025, room 0.108; 33 of 1000 replicates at or under the floor: unverifiable (near the floor)
diversity against random at 40 labels: gain 0.031 over a floor of 0.025, room 0.108; 144 of 1000 replicates at or under the floor: unverifiable (near the floor)
mixture against random at 40 labels: gain 0.033 over a floor of 0.025, room 0.108; 29 of 1000 replicates at or under the floor: unverifiable (near the floor)
budget 42 in rounds of 4 after 8: budget
budget 40 from a pool of 32: budget
1 initial label: budget
a pool label of 0.5: pool
a pool point outside the square: pool
no held-out points: shape
```

The [example](../examples/choose-next-example.coax) makes 112 runs, and 16
of them again for the seal, in about 7 seconds in debug mode and 3 in
release mode.

## The pool and its checker

Every point is uniform in the square [−2, 2]², drawn with chapter 103's
SplitMix64 at two counters per point. The checker labels a point 1 when it
lies above the diagonal x₁ + x₂ = 0, except inside a small disk of radius 0.5
around (1.25, 1.25), the *pocket*, where the label is 0.

<!-- source: psionic/lib/acquire.coax world -->
```jai
// The owned pool: point i is uniform in the square [-2, 2]^2 at counters
// 2i and 2i + 1. The checker labels it; only the loop's oracle calls it.
aq_pocket :: (a:f64,b:f64)->bool {return (a-1.25)*(a-1.25)+(b-1.25)*(b-1.25)<0.25;}
aq_truth :: (a:f64,b:f64)->f64 {
    above:=a+b>0;
    if aq_pocket(a,b) {above=false;}
    return ifx above then 1.0 else 0.0;
}
aq_world :: (seed:u64,first:int,x:[]f64,y:[]f64)->AqStatus {
    if first<0 || x.count!=2*y.count {return AqStatus.shape;}
    for i:0..<y.count {
        c:=cast(u64)(2*(first+i));
        x[2*i]=4*tb_uniform(seed,c)-2;x[2*i+1]=4*tb_uniform(seed,c+1)-2;
        y[i]=aq_truth(x[2*i],x[2*i+1]);
    }
    return AqStatus.ok;
}
```

The pocket is the confidently wrong region. Its center is about 1.77 from the
diagonal, deep on the side labeled 1, so a model that has learned the diagonal
says 1 there with high confidence and is wrong. Uncertainty sampling only asks
about points the model is unsure of, so it has little reason to look there.

The pool is 240 points from seed 1, counters 0 to 479; 15 of them fall in the
pocket. The 400 held-out points come from counters starting at 2,000,000 and
are split into 40 groups of 10. They never enter the pool, the selector or the
choice of any setting.

The labels belong to the checker. The selector never receives a label; the
loop reads a pool label only at the moment it buys it.

## Four scores

Each rule gives every unlabeled point a score, and the highest score is
bought first.

<!-- source: psionic/lib/acquire.coax scores -->
```jai
// Higher is chosen first. p is the model's probability of class 1.
aq_uncertainty :: (p:f64)->f64 {return 1-abs(2*p-1);}
aq_distance :: (x:[]f64,i:int,j:int)->f64 {
    dx:=x[2*i]-x[2*j];dy:=x[2*i+1]-x[2*j+1];
    return sqrt(dx*dx+dy*dy);
}
// The labeled point nearest to point i, the earliest in `have` on a tie.
AqNear :: struct {at:int;distance:f64;}
aq_nearest :: (x:[]f64,i:int,have:[]int)->AqNear {
    r:=AqNear.{at=-1};
    for h:have {
        e:=aq_distance(x,i,h);
        if r.at<0 || e<r.distance {r=AqNear.{at=h,distance=e};}
    }
    return r;
}
// Random selection is a score too: a fresh uniform draw per point and round.
aq_score :: (rule:AqRule,p:f64,nearest:f64,i:int,seed:u64,round:int)->f64 {
    if rule==AqRule.uncertainty {return aq_uncertainty(p);}
    if rule==AqRule.diversity {return nearest;}
    if rule==AqRule.mixture {return aq_uncertainty(p)+nearest;}
    return tb_uniform(seed,AQ_RANDOM_COUNTER+cast(u64)(round*AQ_POOL_MAX+i));
}
```

- **Uncertainty** is 1 − |2p − 1|, where p is the model's probability of
  class 1. It is 1 at p = 0.5 and 0 at p = 0 or 1.
- **Diversity** is the distance to the nearest labeled point.
- **Mixture** adds the two, unweighted.
- **Random** draws a fresh uniform number for each point and round. Random
  selection is then the same argmax as the other rules, so the same code and
  the same checks cover it.

## Work the scores by hand

A tiny pool makes every number exact. Points 0 at (0, 0) and 1 at (4, 0) are
labeled. The candidates, with the model's probabilities, are:

| Point | Position | p | Uncertainty | Nearest labeled | Mixture |
| --- | --- | --- | --- | --- | --- |
| 2 | (0, 3) | 0.875 | 0.25 | 3 (point 0) | 3.25 |
| 3 | (2, 0) | 0.5 | 1 | 2 (point 0, tied with 1) | 3 |
| 4 | (4, 3) | 0.75 | 0.5 | 3 (point 1) | 3.5 |
| 5 | (2, 1.5) | 0.625 | 0.75 | 2.5 (point 0, tied with 1) | 3.25 |

Point 4 is 3 from point 1 and 5 from point 0, a 3-4-5 triangle. Point 5 is
2.5 from both labeled points.

Buy two labels with each rule. A pick joins the labeled set at once, so the
second pick measures distances from the first:
- **Uncertainty** buys 3 (score 1), then 5 (0.75). The model isn't retrained
  between the two picks, so the scores don't change.
- **Diversity:** points 2 and 4 tie at 3, and the lower index wins, so it buys
  2. Point 4 is 4 from point 2 and still 3 from point 1, so its score stays 3;
  point 5 is 2.5 from point 2 as well; point 3 stays at 2. It buys 4.
- **Mixture** buys 4 (3.5). Point 2 is 4 from point 4, so it keeps 3.25; point
  5 is 2.5 from point 4, so it also keeps 3.25. They tie, and it buys 2.

Two of the three rules hit a tie, so the tie rule matters: the lowest index
wins, always.

The diversity picks cost 4 × 2 distances for the first pick (4 candidates, 2
labeled points) and 3 × 3 for the second, 17 in all. The checks assert every
number in this section.

## Buying a batch

<!-- source: psionic/lib/acquire.coax pick -->
```jai
AqCost :: struct {predictions:int;distances:int;}
// Choose k points, one at a time: the highest score among the points not
// yet labeled, the lowest index on a tie. Each pick joins the labeled set
// at once, so diversity measures from it for the next pick. The selector
// sees the points, the model's probabilities and which points are labeled.
// It never sees a label.
aq_pick :: (rule:AqRule,x:[]f64,p:[]f64,have:[]int,k:int,seed:u64,round:int,picks:[]int)->AqCost {
    n:=x.count/AQ_D;cost:AqCost;
    chosen:[AQ_POOL_MAX]int;taken:[AQ_POOL_MAX]bool;
    for h,j:have {chosen[j]=h;taken[h]=true;}
    spatial:=rule==AqRule.diversity || rule==AqRule.mixture;
    if rule==AqRule.uncertainty || rule==AqRule.mixture {cost.predictions=n-have.count;}
    for q:0..<k {
        labeled:=have.count+q;best:=-1;top:=0.0;
        for i:0..<n {
            if taken[i] {continue;}
            near:=0.0;
            if spatial {near=aq_nearest(x,i,chosen[0..<labeled]).distance;cost.distances+=labeled;}
            s:=aq_score(rule,p[i],near,i,seed,round);
            if best<0 || s>top {best=i;top=s;}
        }
        taken[best]=true;chosen[labeled]=best;picks[q]=best;
    }
    return cost;
}
```

`aq_pick` sees the points, the model's probabilities and the indices already
labeled. Nothing in its arguments carries a label. `chosen` is the labeled
set plus this batch's picks so far, which is what the second diversity pick
above measures from.

The nearest labeled point is found by scanning the whole labeled set for
every candidate and every pick. That is the simplest correct choice, and its
cost is counted: (240 − L − q)(L + q) distances for pick q of a round that
starts with L labels.

## The loop

<!-- source: psionic/lib/acquire.coax admit -->
```jai
// Every refusal, before any label is bought.
aq_admit :: (c:AqConfig,px:[]f64,py:[]f64,hx:[]f64,hy:[]f64)->AqStatus {
    n:=py.count;
    if n<2 || n>AQ_POOL_MAX || px.count!=AQ_D*n || !aq_box(px) || !aq_labels(py) {return AqStatus.pool;}
    if hy.count<1 || hy.count>AQ_HELD_MAX || hx.count!=AQ_D*hy.count || !aq_box(hx) || !aq_labels(hy) {return AqStatus.shape;}
    if c.initial<2 || c.batch<1 || c.batch>AQ_BATCH_MAX || c.budget<c.initial {return AqStatus.budget;}
    if c.budget>AQ_BUDGET_MAX || c.budget>n || (c.budget-c.initial)%c.batch!=0 {return AqStatus.budget;}
    if c.epochs<1 || c.epochs>TB_EPOCHS_MAX {return AqStatus.shape;}
    return AqStatus.ok;
}
```

<!-- source: psionic/lib/acquire.coax run -->
```jai
// One run. Round 0 picks the initial labels at random, the same points
// for every rule with the same seed. Every round then buys the labels it
// picked, trains on every label bought so far, and scores the held-out
// points; the next round picks by the rule, until the budget is spent.
// Only the oracle line reads py, and only at points just picked.
aq_run :: (c:AqConfig,px:[]f64,py:[]f64,hx:[]f64,hy:[]f64)->AqRun {
    admitted:=aq_admit(c,px,py,hx,hy);
    if admitted!=AqStatus.ok {return AqRun.{status=admitted};}
    run:AqRun;p:[AQ_POOL_MAX]f64;params:[17]f64;
    rule:=AqRule.random;k:=c.initial;
    while true {
        cost:=aq_pick(rule,px,p[..],run.queries[0..<run.count],k,c.seed,run.rounds,run.queries[run.count..<run.count+k]);
        run.predictions+=cost.predictions;run.distances+=cost.distances;
        // The oracle: the checker's label for each point just picked, and no other.
        for q:run.count..<run.count+k {run.labels[q]=py[run.queries[q]];}
        run.count+=k;
        t:=aq_fit(c,px,run.queries[0..<run.count],run.labels[0..<run.count],params[..]);run.steps+=t.steps;
        run.accuracy[run.rounds]=aq_score_held(params[..],hx,hy,run.correct[..]);
        if run.count==c.budget {break;}
        run.rounds+=1;rule=c.rule;k=c.batch;
        if rule==AqRule.uncertainty || rule==AqRule.mixture {aq_probabilities(params[..],px,p[..]);}
    }
    return run;
}
```

Round 0 is always random, with the run's seed, so every rule starts from the
same 8 labels. The budget counts those 8. Each round:
1. picks `k` points (the initial 8, then 4 a round);
2. buys their labels from the checker, the only place `py` is read;
3. trains chapter 103's classifier from scratch on every label bought so far:
   2 inputs, 4 tanh units, a logistic output, 200 epochs of 8 labels a step at
   rate 2, initialized and shuffled from the run's seed;
4. scores each held-out point 1 when its predicted class is right.

The record keeps every query and its label in order, the held-out accuracy
after every round, the per-point held-out scores of the last model, and what
the run cost: training steps, the probabilities a rule read and the distances
it computed.

## Sealing the selector

A test of "the selector never saw a hidden label" has to change a hidden
label and see that nothing the selector did changes.

<!-- source: psionic/lib/acquire.coax sealed -->
```jai
aq_same_queries :: (a:AqRun,b:AqRun)->bool {
    if a.count!=b.count {return false;}
    for q:0..<a.count {if a.queries[q]!=b.queries[q] {return false;}}
    return true;
}
// The run again with every unqueried pool label and every held-out label
// flipped. If either reached the selector, the queries would change. The
// model is the same, so every held-out score must flip.
aq_sealed :: (c:AqConfig,px:[]f64,py:[]f64,hx:[]f64,hy:[]f64,run:AqRun)->bool {
    fy:[AQ_POOL_MAX]f64;fh:[AQ_HELD_MAX]f64;asked:[AQ_POOL_MAX]bool;
    for q:0..<run.count {asked[run.queries[q]]=true;}
    for v,i:py {fy[i]=ifx asked[i] then v else 1-v;}
    for v,i:hy {fh[i]=1-v;}
    again:=aq_run(c,px,fy[0..<py.count],hx,fh[0..<hy.count]);
    if !aq_same_queries(run,again) {return false;}
    for v,i:hy {if again.correct[i]!=1-run.correct[i] {return false;}}
    return true;
}
```

`aq_sealed` runs the same configuration again with every unbought pool label
and every held-out label flipped. If any of them reached the selector, some
query would change. The bought labels are the same, so the model is the same,
and every held-out score must flip from right to wrong or back. The example
seals seeds 1 to 4 of every rule. A check also flips two bought labels and
confirms that the queries do change, so the test can fail.

## Comparing with random selection

Held-out accuracy moves from seed to seed. Random selection's sd at 40 labels
is 0.047 over 16 seeds; seed 3 alone, which drew 10 points labeled 1 out of
40, reaches 0.76. A single run of each rule says little.

<!-- source: psionic/lib/acquire.coax compare -->
```jai
// A rule against random selection. Scores are seed-major: seed s's per-item
// scores fill [s*items..<(s+1)*items]. The rule ran on some seeds; random
// ran on those seeds and more, so that its runs fill at least two blocks
// the rule's size. The floor is chapter 240's over those blocks, the
// difference two blocks of that many seeds reach by chance. The rule is
// compared with random's first block, by chapter 241's decision gate,
// drawing whole held-out groups again.
AqCompare :: struct {floor:NsFloor;decision:VdDecision;}
aq_compare :: (random:[]f64,rule:[]f64,group:[]int,seed:u64,replicates:int)->AqCompare {
    items:=group.count;r:AqCompare;
    r.decision.status=VdStatus.shape;
    if items<1 || items>AQ_HELD_MAX || rule.count<items || rule.count%items!=0 || random.count%items!=0 {return r;}
    seeds:=rule.count/items;part:[512]int;a:[512]f64;b:[512]f64;
    r.floor=ns_floor(random,part[0..<items],0,random.count/items,seeds);
    if r.floor.status==NsStatus.shape {return r;}
    r.decision.status=VdStatus.number;
    if r.floor.status!=NsStatus.ok || ns_means(rule,items,0,seeds,b[0..<items],false)!=NsStatus.ok {return r;}
    // The floor already checked random's scores, so its means cannot refuse.
    means:=ns_means(random,items,0,seeds,a[0..<items],false);
    r.decision=vd_decision(a[0..<items],b[0..<items],group,part[0..<items],0,r.floor,1,seed,replicates);
    return r;
}
```

The comparison follows chapters 240 and 241:
- **The floor.** Random selection runs on seeds 1 to 64, in four blocks of 16.
  The floor is how far two block means of 16 seeds move apart by chance:
  1.96 √2 times their standard deviation, 0.025 here. Its blocks are the same
  size as the comparison, 16 seeds.
- **The gain.** Each rule's per-point held-out score, averaged over seeds 1
  to 16, against random selection's over the same seeds. The 40 held-out
  groups are drawn again with replacement 1,000 times.
- **The verdict.** Passed when at most 1 replicate in 40 falls at or under the
  floor, failed when at most 1 in 40 falls above it, and unverifiable
  otherwise. A baseline already within the floor of perfect accuracy is
  refused as having no headroom; here the room is 0.108.

## What the run shows

**Quality against labels.** From 16 labels on, all three rules are ahead of
random selection at every budget in the table, and their spread across seeds
is smaller. At 40 labels random selection reaches 0.892, uncertainty 0.927,
diversity 0.923 and mixture 0.925.

**The verdict.** The mean gains, 0.031 to 0.035, are above the floor of
0.025. But when the held-out groups are drawn again, 33, 144 and 29 of 1,000
replicates fall at or under the floor, and clear needs 25 or fewer. All three
rules are *unverifiable (near the floor)*. On this pool, at this budget and
with 16 seeds, the chapter can't say that choosing labels beats random
selection beyond the noise. It can't say that it doesn't, either.

**The pocket.** The pocket holds 15 of the 240 pool points, 6.25 percent. Over
16 seeds each rule bought 512 labels after the initial ones:

| Rule | Pocket labels bought | Held-out pocket accuracy |
| --- | --- | --- |
| random | 38 | 0.428 |
| uncertainty | 24 | 0.138 |
| diversity | 17 | 0.038 |
| mixture | 35 | 0.241 |

Uncertainty bought fewer pocket labels than random selection, as expected:
the model is confident there. Diversity bought fewer still. It looks only at
distance to labeled points, not at where the model might be wrong. The rules
that do best on average do worst in the pocket. Random selection, which buys
pocket labels roughly in proportion to the pocket's size, gets 0.428 of the
20 held-out pocket points right, three times uncertainty's 0.138. A report that gave only overall accuracy would hide this.

**Cost.** Every rule trains the same model on the same number of labels, so
all four use 92,800 training steps over 16 seeds. Uncertainty and mixture
also read 1,744 probabilities a run (the unlabeled points in each of 8
rounds), and diversity and mixture compute 160,080 distances a run. In this
program the distances cost more than the probabilities, and both are small
next to training.

## Refusals

`aq_admit` refuses a run before any label is bought:
- `pool` for a pool of fewer than 2 or more than 256 points, coordinates that
  aren't pairs in the square, or labels other than 0 and 1;
- `shape` for a held-out set of 0 or more than 512 points or with the same
  faults, and for epochs outside chapter 103's 1 to 512;
- `budget` for fewer than 2 initial labels, a batch outside 1 to 16, a budget
  below the initial labels, above 64 or above the pool size, or a budget that
  rounds of the batch size can't reach exactly.

The last six lines of the transcript are six such requests: a budget of 42
in rounds of 4 after 8, a budget of 40 from a pool of 32 points, 1 initial
label, a pool label of 0.5, a pool point at x = 3, and no held-out points. A
refused run returns its status with nothing bought, trained or scored.

## Supported profile

`cpu-f64-acquire-v1`:
- binary64, chapter 103's classifier and trainer;
- pools of 2 to 256 points with 2 coordinates in [−2, 2], labels 0 or 1;
- 1 to 512 held-out points, scored by predicted class;
- budgets of at most 64 labels counting at least 2 initial random ones, and
  batches of 1 to 16;
- the comparison: chapter 240's floor over at least two blocks of the
  compared number of seeds, and chapter 241's decision gate.

Every run is a function of its configuration and the pool, so the transcript
is the same in debug and release builds.

## How it is checked

- **Coax checks:** 11 checks in debug and release. They cover:
  - the scores, distances, nearest points and ties of the tiny pool above,
    and its picks for every rule;
  - the cost of choosing: 17 distances for two diversity picks, 30 for four;
  - every pick of a 6-pick batch on 24 random 12-point pools, for every rule,
    against a brute-force argmax that recomputes each candidate's score from
    its definition over the whole labeled set;
  - a 40-point run for every rule: each label against the checker, the
    initial picks against the four largest round-0 draws, the training steps,
    probabilities and distances against their formulas;
  - every round's model retrained outside the loop, its held-out accuracy and
    the next round's picks recomputed;
  - the seal: unbought and held-out labels flipped, and records with one
    query or one held-out score changed refused;
  - every refusal at its bound;
  - the comparison by hand: a floor of 1.96 × 0.025 = 0.049 from four seeds
    in blocks of two, a passed and a failed verdict, and its shape refusals.
- **The example** compares its transcript exactly in debug and release.
- **Mutation testing** of `acquire.coax` with `--root-only` kills all 367
  mutants. Early runs left survivors where a check couldn't tell two programs
  apart. The nearest-point search now reports which labeled point is nearest,
  the earliest on a tie, so a change to its comparison is visible. The
  initial random pick and every later pick share one call, and probability
  computation is its own checked function.

## What this does not do yet

- **One pool, one model.** The pool is 2-D with one straight boundary and one
  pocket. Other data could favor a rule clearly, or favor random selection.
- **No model of label cost.** Every label costs the same. Rules that query
  expensive examples less often aren't modeled.
- **Greedy batches.** Uncertainty doesn't update within a batch, so a batch of
  4 can buy four nearly identical points. Batch-aware rules aren't built.
- **No tuning set.** The settings were chosen once and fixed. A study that
  tuned a rule's settings would need its own tuning groups, apart from the
  held-out groups that decide the verdict.
- **More seeds would sharpen the answer.** With more seeds both the floor and
  the bootstrap tighten; this chapter stops at 16 to keep the example short.
