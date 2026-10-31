# 238. How far a metric moves by itself

[Chapter 16](ch016-are-the-probabilities-any-good.md) measured Elephant once
and got 12 of 12. Chapters
[213](ch213-fit-our-own-calibration.md) to
[215](ch215-record-readings-join-outcomes-watch-drift.md) compared maps on one
draw of their data. A difference between two such numbers is a gain only if it
is larger than the difference that shows up when nothing has changed.

This chapter measures that difference in two ways, and refuses a partition
where no gain could show:
- **Seed blocks.** Run the same configuration many times, group the runs into
  blocks, and see how far equal blocks differ.
- **A paired bootstrap over tasks.** Draw the suite's tasks again, with
  replacement, and see how far the gain moves.
- **Headroom.** A partition whose baseline is already within the floor of its
  ceiling is refused.

```shell
coax/bin/coax run psionic/examples/noise-floor.coax
psionic/check.sh --only noise
```

```jai
open tasks (32): block accuracy 0.4891 to 0.5938, sd 0.0322, floor 0.0893; baseline 0.5334, room 0.4666: open
saturated tasks (8): block accuracy 0.9719 to 0.9938, sd 0.0066, floor 0.0184; baseline 0.9879, room 0.0121: refused
the baseline against itself, 20 times: gains up to 0.1242, root mean square 0.0516, 3 beyond the floor
  standard error from items 0.0186: 6 of 20 beyond 1.96 of them
  standard error from tasks 0.0388: 5 of 20 beyond 1.96 of them
candidate 0.5 logits better: gain 0.0555, floor 0.0893
  items: standard error 0.018, 1 of 1000 at or below 0, 973 at or below the floor
  tasks: standard error 0.0351, 54 of 1000 at or below 0, 822 at or below the floor
candidate 1.5 logits better: gain 0.1359, floor 0.0893
  items: standard error 0.0173, 0 of 1000 at or below 0, 6 at or below the floor
  tasks: standard error 0.0299, 0 of 1000 at or below 0, 58 at or below the floor
Elephant, chapter 16's suite, accuracy:
  choice: 5 items, 0 below the ceiling, room 0.0, refused
  noul: 4 items, 0 below the ceiling, room 0.0, refused
  score: 3 items, 0 below the ceiling, room 0.0, refused
  all: 12 items, 0 below the ceiling, room 0.0, refused
Elephant, Brier score (negated, ceiling 0):
  choice: 5 items, 5 below the ceiling, room 0.0000534, open
  noul: 4 items, 4 below the ceiling, room 0.0000653, open
  score: 3 items, 3 below the ceiling, room 0.0000499, open
  all: 12 items, 12 below the ceiling, room 0.0000565, open
```

The [example](../examples/noise-floor.coax) runs in under a second.

## A suite with seeds

The suite is synthetic, so we know what is true:
- 40 tasks of 10 items each.
- Each task has a difficulty, and each item varies a little around it.
- Tasks 32 to 39 are 5 logits easier than the rest. The baseline almost always
  gets them right.

<!-- source: psionic/lib/noise.coax world -->
```jai
// SplitMix64 at a counter, as in chapter 213. Item i belongs to task i/10;
// tasks from 32 on are 5 logits easier, so the baseline saturates there.
ns_task :: (item:int)->int {return item/NS_PER_TASK;}
ns_part :: (item:int)->int {return ifx ns_task(item)>=NS_SATURATED then 1 else 0;}
ns_difficulty :: (seed:u64,item:int)->f64 {
    t:=ns_task(item);d:=3*tb_uniform(seed,cast(u64)t)-1.5+tb_uniform(seed,cast(u64)(1000+item))-0.5;
    if t>=NS_SATURATED {d-=5;}
    return d;
}
// Run r: a shift shared by every task, an effect for each task, and a draw
// for each item. Item i is right with probability σ(skill − difficulty + effects).
ns_run :: (seed:u64,run:int,skill:f64,out:[]f64)->NsStatus #export_c {
    if out.count!=NS_ITEMS || run<0 || run>=NS_RUNS {return NsStatus.shape;}
    if !tb_in(skill,-8,8) {return NsStatus.number;}
    base:=cast(u64)(100000*(run+1));shift:=NS_SHIFT*(2*tb_uniform(seed,base+999)-1);
    for i:0..<NS_ITEMS {
        g:=shift+NS_SPREAD*(2*tb_uniform(seed,base+cast(u64)ns_task(i))-1);
        out[i]=floor(tb_uniform(seed,base+cast(u64)(1000+i))+tb_sigmoid(skill-ns_difficulty(seed,i)+g));
    }
    return NsStatus.ok;
}
```

A run is one training or search seed. It moves the metric in two ways:
- a shift of up to 0.3 logits shared by every task;
- an effect of up to 3 logits on each task separately.

A seed is good at some tasks and bad at others. Real seeds behave like this,
and it is why the items of one task don't count as independent evidence.

## Seed blocks and the floor

The baseline runs 64 times, in 16 blocks of 4 seeds. A candidate is compared
using 4 seeds too, so a block is the unit that varies.

<!-- source: psionic/lib/noise.coax floor -->
```jai
// Runs of one configuration in blocks of equal size: each block's mean over
// the partition, their standard deviation, and the floor, the difference
// two blocks stay under 95 times in 100 if block means are normal.
ns_floor :: (scores:[]f64,part:[]int,which:int,runs:int,block:int)->NsFloor #export_c {
    n:=part.count;picked:=ns_parts_ok(part,which);
    if picked==0 || block<1 || runs%block!=0 || scores.count!=runs*n {return NsFloor.{status=NsStatus.shape};}
    r:=NsFloor.{blocks=runs/block};
    if r.blocks<2 || r.blocks>NS_BLOCKS_MAX {return NsFloor.{status=NsStatus.shape};}
    if !ns_finite(scores) {return NsFloor.{status=NsStatus.number};}
    all:PsionicAccumulator;
    for k:0..<r.blocks {
        a:PsionicAccumulator;
        for q:k*block..<(k+1)*block {for i:0..<n {if part[i]==which {a=psionic_ops_accumulate(a,scores[q*n+i]);}}}
        r.mean[k]=psionic_ops_total(a)/f64(block*picked);all=psionic_ops_accumulate(all,r.mean[k]);
    }
    mu:=psionic_ops_total(all)/f64(r.blocks);v:PsionicAccumulator;
    for k:0..<r.blocks {d:=r.mean[k]-mu;v=psionic_ops_accumulate(v,d*d);}
    r.sd=sqrt(psionic_ops_total(v)/f64(r.blocks-1));r.floor=NS_Z*sqrt(2.0)*r.sd;
    return r;
}
```

| Partition | Block accuracy | Standard deviation | Floor |
| --- | --- | --- | --- |
| Open tasks (32) | 0.4891 to 0.5938 | 0.0322 | 0.0893 |
| Saturated tasks (8) | 0.9719 to 0.9938 | 0.0066 | 0.0184 |

Two blocks of the same baseline differ by more than 0.0893 about 5 times in
100, if block means are normally distributed. A smaller gain could be seeds
alone.

The floor is an estimate too. Twenty more comparisons of the baseline with
itself had a root mean square gain of 0.0516. That implies a block standard
deviation of 0.0365 rather than 0.0322, and a floor near 0.10. Three of the
twenty passed 0.0893. Sixteen blocks are enough to place the floor, not to pin
it.

## Draw tasks, not items

Seeds are one source of noise. The other is which items were written. The
bootstrap measures it by drawing the suite again, keeping each item's pair of
scores together.

<!-- source: psionic/lib/noise.coax bootstrap -->
```jai
// The paired gain of b over a on a partition, and its spread when whole tasks
// are drawn again with replacement; below counts replicates at or under floor.
ns_boot :: (a:[]f64,b:[]f64,task:[]int,part:[]int,which:int,noise:f64,seed:u64,replicates:int)->NsBoot #export_c {
    n:=part.count;
    if ns_parts_ok(part,which)==0 || a.count!=n || b.count!=n || task.count!=n {return NsBoot.{status=NsStatus.shape};}
    if replicates<2 || replicates>NS_REPLICATES_MAX {return NsBoot.{status=NsStatus.shape};}
    for t:task {if t<0 || t>=NS_TASKS_MAX {return NsBoot.{status=NsStatus.shape};}}
    if !ns_finite(a) || !ns_finite(b) || !tb_in(noise,-NS_BIG,NS_BIG) {return NsBoot.{status=NsStatus.number};}
    sums:[NS_TASKS_MAX]PsionicAccumulator;counts:[NS_TASKS_MAX]int;
    for i:0..<n {if part[i]==which {sums[task[i]]=psionic_ops_accumulate(sums[task[i]],b[i]-a[i]);counts[task[i]]+=1;}}
    // The tasks present, in id order, with each task's summed difference.
    present:[NS_TASKS_MAX]int;gain:[NS_TASKS_MAX]f64;r:NsBoot;all:PsionicAccumulator;
    for t:0..<NS_TASKS_MAX {
        if counts[t]>0 {
            present[r.tasks]=t;gain[t]=psionic_ops_total(sums[t]);r.tasks+=1;r.items+=counts[t];
            all=psionic_ops_accumulate(all,gain[t]);
        }
    }
    r.gain=psionic_ops_total(all)/f64(r.items);
    reps:[NS_REPLICATES_MAX]f64;m:PsionicAccumulator;
    for k:0..<replicates {
        s:PsionicAccumulator;c:=0;
        for j:0..<r.tasks {
            t:=present[cast(int)(tb_uniform(seed,cast(u64)(k*r.tasks+j))*f64(r.tasks))];
            s=psionic_ops_accumulate(s,gain[t]);c+=counts[t];
        }
        reps[k]=psionic_ops_total(s)/f64(c);m=psionic_ops_accumulate(m,reps[k]);
        if reps[k]<=noise {r.below+=1;}
    }
    mu:=psionic_ops_total(m)/f64(replicates);v:PsionicAccumulator;
    for k:0..<replicates {d:=reps[k]-mu;v=psionic_ops_accumulate(v,d*d);}
    r.se=sqrt(psionic_ops_total(v)/f64(replicates-1));
    return r;
}
```

It draws whole tasks. Drawing items as if they were independent misses that
the items of a task rise and fall together.

The twenty self-comparisons show the difference:

| Spread of the gain | Value | Self-comparisons beyond 1.96 of it |
| --- | --- | --- |
| Measured across the twenty | 0.0516 | |
| Bootstrap over items | 0.0186 | 6 of 20 |
| Bootstrap over tasks | 0.0388 | 5 of 20 |

About 1 in 20 should land beyond 1.96 standard errors:
- Resampling items understates the spread by almost three times.
- Resampling tasks gets most of it.

Neither sees the shift a seed applies to every task, because a bootstrap reuses
the same runs. That is the floor's job.

## Two candidates

Each candidate has 4 seeds and is compared with the baseline's first block, on
the open tasks.

| Candidate | Gain | Items: standard error, replicates ≤ 0, ≤ floor | Tasks: standard error, replicates ≤ 0, ≤ floor |
| --- | --- | --- | --- |
| 0.5 logits better | 0.0555 | 0.018, 1, 973 | 0.0351, 54, 822 |
| 1.5 logits better | 0.1359 | 0.0173, 0, 6 | 0.0299, 0, 58 |

The candidate that is 0.5 logits better shows the trap:
- The item bootstrap puts 1 replicate of 1,000 at or below 0, so the gain looks
  certain.
- The task bootstrap puts 54 there.
- The gain is below the floor of 0.0893. Seeds alone move the metric that far.

The candidate that is 1.5 logits better clears the floor. Even so, 58 of 1,000
task replicates fall back to it. Turning these numbers into a verdict is
[chapter 239's](ch239-gates-that-can-say-unverifiable.md) job; this chapter only measures.

## Headroom

<!-- source: psionic/lib/noise.coax headroom -->
```jai
// A partition is refused when its baseline is within the floor of the
// ceiling: no gain larger than the floor could fit. With every item at the
// ceiling the room is exactly 0.
ns_headroom :: (score:[]f64,part:[]int,which:int,ceiling:f64,noise:f64)->NsRoom #export_c {
    r:=NsRoom.{items=ns_parts_ok(part,which)};
    if r.items==0 || score.count!=part.count {return NsRoom.{status=NsStatus.shape};}
    if !tb_in(ceiling,-NS_BIG,NS_BIG) || !tb_in(noise,0,NS_BIG) || !ns_finite(score) {return NsRoom.{status=NsStatus.number};}
    a:PsionicAccumulator;
    for v,i:score {
        if v>ceiling {return NsRoom.{status=NsStatus.number};}
        if part[i]==which {a=psionic_ops_accumulate(a,v);if v<ceiling {r.open+=1;}}
    }
    if r.open>0 {r.room=ceiling-psionic_ops_total(a)/f64(r.items);}
    r.refused=r.room<=noise;
    return r;
}
```

The saturated tasks are refused. The baseline is right on 0.9879 of them, so
the room left is 0.0121, smaller than their floor of 0.0184. Half of their 80
items are right in every one of the 64 runs. Even a perfect candidate could not
gain more than the seeds already move.

A loss enters negated, with ceiling 0, so that higher is always better.

## Elephant on chapter 16's suite

The same checks on Elephant's recorded run of chapter 16's twelve items:

| Kind | Items | Accuracy: below the ceiling | Brier score: room left |
| --- | --- | --- | --- |
| Choice | 5 | 0, refused | 0.0000534 |
| Noul | 4 | 0, refused | 0.0000653 |
| Score | 3 | 0, refused | 0.0000499 |
| All | 12 | 0, refused | 0.0000565 |

Accuracy is refused for every kind, and for the whole suite. Every item is
already right, so no candidate can gain.

The Brier score is open, with at most 0.0000565 to gain. Elephant reads the
option letters' logits with no random draw, so these use a floor of 0. A
bootstrap is no help either: when every item is right, every replicate is the
same.

So no optimization can claim a gain on this suite. It first needs items that
Elephant gets wrong.

## Supported profile

`cpu-f64-noise-v1`:
- binary64, with chapter 4's compensated accumulator for every sum;
- scores where higher is better, with losses entered negated;
- up to 4,096 items in up to 1,024 tasks, and a partition given per item;
- 2 to 16 blocks of equal size, and a floor of 1.96 × √2 standard deviations;
- 2 to 10,000 bootstrap replicates, with whole tasks drawn by SplitMix64 at a
  counter;
- refusal when the room left is at most the floor.

`ns_means` reads one span and writes another, and the native adapter reports
whether they overlap. A refusal writes nothing.

## How it is checked

- **Coax checks:** 64 checks in debug and release, counting the units it
  loads. They cover:
  - the world at its counters, and the first saturated item;
  - item means, floors over two and four blocks, a bootstrap and headroom by
    hand;
  - a replicate exactly at the floor, which counts;
  - every item at its ceiling, which leaves no room for any floor;
  - Brier scores and first-maximum ties;
  - a small suite end to end;
  - every bound and refusal.
- **An independent oracle,** [noise_oracle.py](../tests/noise_oracle.py),
  written from the specification. It reads Elephant's probabilities from
  [chapter 16's recorded run](../fixtures/quality/heldout-v1-elephant-cpu.txt)
  rather than from a copy.
- **The native harness** [noise.c](../tests/noise.c) runs the whole example
  through the adapter, plain and under ASan/UBSan, in debug and release.
  Tolerances were frozen before comparison:
  - run sums, task and item counts, replicates at or below each floor, open
    items and refusals must be equal;
  - means, floors, gains, standard errors, rooms and Brier scores within 10⁻¹².

  Every value was identical.
- **Mutation testing** of `noise.coax` kills all 1,193 mutants. The first run
  left five survivors, each fixed in the checks:
  - a tolerance helper whose `<` and `<=` were the same, replaced by exact
    comparisons;
  - an output buffer that already held the value a mutant skipped writing;
  - a suite of one item;
  - a NaN in the first run of a later range.

The [verification record](../../docs/verification/psionic-ch179-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **No Elephant seed blocks.** Repeated Elephant runs would vary the option
  order or the wording. At over an hour per decision on the CPU reference,
  that waits for a faster backend.
- **The floor assumes normal block means.** Its own uncertainty, which the
  self-comparisons showed, is not reported.
- **Standard errors and counts, not intervals.** Percentile intervals, and
  verdicts that say passed, failed or unverifiable, belong to chapter 239.
- **No suite digests here.** [Chapter 237](ch237-a-suite-is-three-digests.md) pins which items, questions and gate a
  comparison used.
