# 181. Environments with checkers

To learn from its own attempts (chapter 85), a model needs tasks whose
answers a program can check. Such a checker is easy to get wrong. One that
passes anything rewards nonsense, and one that checks only an answer's
length rewards a near miss. If the reference solution itself fails the
checker, every agent looks worse than it is. Once a training run has
started, these mistakes are hard to see, so they are caught before an
environment is admitted.

This chapter makes an environment one record:
- **a task**, drawn from a seed;
- **a checker** that judges work against the task;
- **a budget** of attempts;
- **a reference solution**;
- **a digest** over all of it.

An environment is admitted only after its checker passes three tests. A
**rollout** is an agent's attempts until the checker passes or the budget
runs out. It carries a digest that chains every attempt, so a **replay**
either reproduces it exactly or shows the record was changed.

```shell
coax/bin/coax run psionic/examples/checked-environments.coax
coax/bin/coax test psionic/lib/env.coax
```

```jai
40 environments (16 sort, 9 dedupe, 15 prefix sums), exact checkers, 3 attempts each: 40 admitted
  the first: sort [6 9 4 4 2 3 9 3] -> [2 3 3 4 4 6 9 9], digest 2432ec011b437478
the same task with a weak checker or a wrong reference:
  anything checker: refused: empty work passes
  length only checker: refused: a near miss passes
  exact checker, reference [3 3 3 4 4 6 9 9]: refused: the reference fails
agent with skill 0.35: solved 32 of 40, 18 on the first attempt, 78 attempts in all; 40 of 40 replay exactly
agent with skill 0.75: solved 39 of 40, 29 on the first attempt, 53 attempts in all; 40 of 40 replay exactly
  its first rollout: solved false, 3 attempts, digest db5a0f2fd029bd9d
  claimed in 1 attempt instead: replay refuses
  replayed against a changed environment: refuses
```

The [example](../examples/checked-environments.coax) runs in about a second.

## Tasks

<!-- source: psionic/lib/env.coax task -->
```jai
// task-sim-v1. Task i draws its kind, its size from 4 to 16, and its values
// from 0 to 9 on counters 64i, 64i+1 and 64i+2 onwards.
en_tasks :: (seed:u64,out:[]EnTask)->EnStatus #export_c {
    if out.count>300 {return EnStatus.shape;}
    for i:0..<out.count {
        b:=cast(u64)(64*i);
        out[i].kind=cast(EnKind)cast(int)(3*tb_uniform(seed,b));
        out[i].work.size=4+cast(int)(13*tb_uniform(seed,b+1));
        for j:0..<out[i].work.size {out[i].work.v[j]=cast(int)(10*tb_uniform(seed,b+2+cast(u64)j));}
    }
    return EnStatus.ok;
}
```

There are three kinds of task, each over 4 to 16 values from 0 to 9:
sorting, removing repeats while keeping the first of each, and running sums.
They are small on purpose: the subject is the record around a task, not the
task.

<!-- source: psionic/lib/env.coax solve -->
```jai
// The reference solutions: sort by counting, keep first occurrences, and
// running sums.
en_solve :: (t:EnTask)->EnWork {
    w:EnWork;n:=t.work.size;
    if t.kind==EnKind.sort {
        count:[10]int;for i:0..<n {count[t.work.v[i]]+=1;}
        for x:0..<10 {for c:0..<count[x] {w.v[w.size]=x;w.size+=1;}}
    } else if t.kind==EnKind.dedupe {
        seen:[10]bool;
        for i:0..<n {x:=t.work.v[i];if !seen[x] {seen[x]=true;w.v[w.size]=x;w.size+=1;}}
    } else {
        sum:=0;for i:0..<n {sum+=t.work.v[i];w.v[i]=sum;}w.size=n;
    }
    return w;
}
```

## Checkers, and what earns admission

<!-- source: psionic/lib/env.coax check -->
```jai
en_same :: (a:EnWork,b:EnWork)->bool {
    if a.size!=b.size {return false;}
    for i:0..<a.size {if a.v[i]!=b.v[i] {return false;}}
    return true;
}
en_check :: (checker:EnChecker,t:EnTask,work:EnWork)->bool {
    if checker==EnChecker.anything {return true;}
    want:=en_solve(t);
    if checker==EnChecker.length_only {return work.size==want.size;}
    return en_same(work,want);
}
```

There are three checkers. Only `exact` is fit to use; the other two are the
mistakes admission has to catch.

<!-- source: psionic/lib/env.coax admit -->
```jai
en_near :: (w:EnWork)->EnWork {near:=w;near.v[near.size-1]+=1;return near;}
// A checker earns admission three ways: it fails on no work, it passes the
// reference, and it fails on the reference with its last value off by one.
en_admit :: (e:EnEnv)->EnAdmit #export_c {
    if en_check(e.checker,e.task,EnWork.{}) {return EnAdmit.empty_passes;}
    if !en_check(e.checker,e.task,e.reference) {return EnAdmit.reference_fails;}
    if en_check(e.checker,e.task,en_near(e.reference)) {return EnAdmit.near_miss_passes;}
    return EnAdmit.admitted;
}
```

A checker is admitted only when it passes all three tests:
- **it fails on empty work**, or it rewards doing nothing;
- **it passes the reference**, or the environment can't be solved even by
  its author;
- **it fails on a near miss,** the reference with its last value off by one.
  A checker that passes this can't tell right from almost right.

In the example, all 40 exact environments are admitted. On the first task,
the `anything` checker is refused because empty work passes, `length only`
because a near miss passes, and a wrong reference because it fails its own
checker.

## The record and its digest

<!-- source: psionic/lib/env.coax digest -->
```jai
en_work :: (buf:[]u8,at:int,w:EnWork)->int {
    p:=su_u64(buf,at,cast(u64)w.size);
    for i:0..<w.size {p=su_u64(buf,p,cast(u64)w.v[i]);}
    return p;
}
en_digest :: (e:EnEnv)->[32]u8 #export_c {
    buf:[320]u8;at:=su_text(buf[..],0,"psionic.env.v1");buf[at]=cast(u8)e.task.kind;
    at=en_work(buf[..],at+1,e.task.work);buf[at]=cast(u8)e.checker;
    at=su_u64(buf[..],at+1,cast(u64)e.attempts);at=en_work(buf[..],at,e.reference);
    return su_hash(buf[0..<at]);
}
```

The digest uses [chapter 178](ch178-a-suite-is-three-digests.md)'s
encoding: a length-prefixed tag and little-endian 64-bit fields. It covers
the task's kind and values, the checker, the budget and the reference. It
does not cover the slots past a work's size, which aren't part of the
record. Changing any field that is part of it changes the digest.

## Rollouts

<!-- source: psionic/lib/env.coax agent -->
```jai
// agent-sim-v1. On attempt a of rollout r the agent is right with
// probability skill; otherwise it bumps one value, drops the last, or hands
// back its input, each a third of the time.
en_attempt :: (g:EnAgent,t:EnTask,r:int,a:int)->EnWork {
    b:=cast(u64)((r*8+a)*4);w:=en_solve(t);
    if cast(int)(tb_uniform(g.seed,b)+g.skill)>0 {return w;}
    mistake:=cast(int)(3*tb_uniform(g.seed,b+1));
    if mistake==0 {w.v[cast(int)(f64(w.size)*tb_uniform(g.seed,b+2))]+=1;return w;}
    if mistake==1 {w.size-=1;return w;}
    return t.work;
}
```

Until chapters 16 to 20 serve a model for this, the agent is simulated. On
each attempt it is right with a probability called its skill. Otherwise it
makes one of three mistakes:
- adds one to a value;
- drops the last value;
- hands back its input unchanged.

Every draw comes from a counter for that rollout and attempt, so the same
seed gives the same attempts.

<!-- source: psionic/lib/env.coax rollout -->
```jai
en_rollout_ok :: (e:EnEnv,g:EnAgent,r:int)->bool {
    return e.attempts>=1 && e.attempts<=EN_ATTEMPTS_MAX && g.skill>=0 && g.skill<=1 && r>=0;
}
// The digest starts from the environment, the agent and the rollout number,
// then chains each attempt's work and verdict.
en_rollout :: (e:EnEnv,g:EnAgent,r:int)->EnRollout #export_c {
    out:EnRollout;
    if !en_rollout_ok(e,g,r) {out.status=EnStatus.shape;return out;}
    buf:[320]u8;at:=su_text(buf[..],0,"psionic.rollout.v1");d:=en_digest(e);
    for i:0..<32 {buf[at+i]=d[i];}
    at=su_u64(buf[..],at+32,g.seed);at=su_u64(buf[..],at,f64_bits(g.skill));at=su_u64(buf[..],at,cast(u64)r);
    out.digest=su_hash(buf[0..<at]);
    for a:0..<e.attempts {
        w:=en_attempt(g,e.task,r,a);ok:=en_check(e.checker,e.task,w);
        for i:0..<32 {buf[i]=out.digest[i];}
        out.digest=su_hash(buf[0..<su_flag(buf[..],en_work(buf[..],32,w),ok)]);
        out.attempts=a+1;
        if ok {out.solved=true;return out;}
    }
    return out;
}
```

The digest starts from the environment's digest, the agent's seed and skill
and the rollout's number. It then chains each attempt's work and the
checker's verdict on it.

| Agent | Solved | First attempt | Attempts used |
| --- | ---: | ---: | ---: |
| skill 0.35 | 32 of 40 | 18 | 78 |
| skill 0.75 | 39 of 40 | 29 | 53 |

## Replay

<!-- source: psionic/lib/env.coax replay -->
```jai
// A replay runs the rollout again from the record and accepts it only when
// every field comes out the same.
en_replay :: (e:EnEnv,g:EnAgent,r:int,claimed:EnRollout)->bool #export_c {
    x:=en_rollout(e,g,r);
    return x.status==claimed.status && x.solved==claimed.solved && x.attempts==claimed.attempts && su_same(x.digest,claimed.digest);
}
```

A rollout is accepted as training data only when a replay reproduces it.
All 80 rollouts in the example replay exactly. The first rollout used 3
attempts and failed; a record claiming 1 attempt is refused. Replaying the
true record against an environment whose reference was changed is refused
too, because the environment's digest heads the chain.

## Supported profile

`cpu-int-env-v1`:
- integer work of up to 16 values from 0 to 9;
- task-sim-v1 for up to 300 tasks;
- agent-sim-v1 on chapter 70's SplitMix64 counters, with skill from 0 to 1;
- budgets of 1 to 8 attempts;
- SHA-256 digests in chapter 178's encoding.

## How it is checked

- **Coax checks:** 117 checks in debug and release, counting the units
  it loads. They cover:
  - tasks and solutions against the oracle and by hand;
  - every checker at its edges;
  - each way admission refuses;
  - the digest against the oracle and against a change to each field;
  - the agent's attempts and four rollouts against the oracle, and every
    bound;
  - replay against each kind of tampering;
  - the 40-environment experiment for both agents, in quarters to stay
    inside the compile-time step limit.
- **An independent oracle,** [env_oracle.py](../tests/env_oracle.py), writes
  the tasks, checkers, admission, agent, rollouts and digests again with
  `hashlib`. The example's output equals its figures.
- **Mutation testing** of `env.coax`: all 2,229 mutants are killed.

There is no native harness yet. The
[verification record](../../docs/verification/psionic-ch181-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **A simulated agent.** Rollouts from a Psionic-served model come after
  chapters 16 to 20. The record, admission and replay stay the same; only
  the source of attempts changes.
- **Toy tasks.** Code tasks with test-suite checkers, where the near miss is
  a mutant of the reference (as in this book's own mutation testing), are
  the next environments.
- **No learning.** Chapter 85 trains on these rollouts.
