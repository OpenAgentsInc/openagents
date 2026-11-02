# 211. Measure the harness

> **Status: partly implemented.** End-to-end task suites run through
> chapter 200's loop with a scripted stand-in model, one configuration
> change at a time, with seed blocks, a noise floor, chapter 241's verdicts
> and a receipt chain. Benchmarks of the harness's own operations, wall-clock
> time on a named host, real models and cache hits remain; see
> [What remains](#what-remains).

Every addition to the harness has to earn its place. An addition earns it
by doing better than the harness without it, on the same tasks, seeds and
model, by more than the runs move by themselves. This chapter builds the
bench that decides that:
- **a pinned suite:** [chapter 242](ch242-environments-with-checkers.md)'s
  tasks, fixed by [chapter 239](ch239-a-suite-is-three-digests.md)'s three
  digests, split into development tasks and held-out tasks;
- **runs through the harness:** each task goes through
  [chapter 200](ch200-the-simplest-loop.md)'s loop, under one
  configuration's budgets and tools, once per seed, with a scripted
  stand-in model;
- **what each run costs:** whether chapter 242's exact checker passes the
  answer, the steps, the tokens drawn, the tool calls, chapter 200's clock
  ticks and the stop that ended the turn;
- **a receipt chain:** each configuration's results on one partition are a
  receipt bound to the suite, harness and configuration digests, in a hash
  chain, and every run replays from its seeds;
- **verdicts:** one change at a time, on held-out tasks only, beyond
  [chapter 240](ch240-how-far-a-metric-moves-by-itself.md)'s noise floor
  from the baseline's seed blocks, with
  [chapter 241](ch241-gates-that-can-say-unverifiable.md)'s passed, failed
  or unverifiable.

```shell
coax/bin/coax run psionic/examples/measure-harness.coax
coax/bin/coax test psionic/lib/harness/measure.coax
```

## The profile

`cpu-harness-bench-v1`:
- chapter 200's `cpu-harness-loop-v1` with its scripted model, so the loop,
  its budgets, its calculator tool and its typed stops are the ones under
  test;
- the stand-in model agent-sim-tools-v1, over chapter 242's agent-sim-v1;
- up to 64 tasks, 16 runs and 32 steps, a window of at most 512 tokens;
- cost counted in tokens the model draws, and time in chapter 200's logical
  clock ticks: the positions each reply reads plus a fixed cost per tool
  call. No wall-clock time is measured;
- binary64 for means, floors and the bootstrap, as in chapters 240 and 241;
- digests in chapter 239's encoding, under tags that start with
  `psionic.harness-bench`.

## A task as a prompt

<!-- source: psionic/lib/harness/measure.coax prompt -->
```jai
// The task as a prompt: <|bos|> <|user_start|>, the kind's letter (s, d or
// p) and each value after a space, <|user_end|> <|assistant_start|>.
// Tokens below 256 are bytes.
mh_prompt :: (t:EnTask,ids:[]int)->int {
    ids[0]=HL_BOS;ids[1]=MH_USER;ids[2]=MH_KIND[cast(int)t.kind];at:=3;
    for i:0..<t.work.size {ids[at]=32;ids[at+1]=48+t.work.v[i];at+=2;}
    ids[at]=MH_USER_END;ids[at+1]=MH_ASSISTANT;return at+2;
}
```

A task is one of chapter 242's: sort, remove repeats, or running sums,
over values from 0 to 9. Its prompt names the kind with one letter and
lists the values. The running-sums task [1 2 3 4] is 13 tokens:
<|bos|> <|user_start|> `p 1 2 3 4` <|user_end|> <|assistant_start|>.

## The stand-in model

<!-- source: psionic/lib/harness/measure.coax standin -->
```jai
// agent-sim-tools-v1, the stand-in model. Offered the calculator on a
// running-sums task, it adds one value per call and then writes the sums;
// otherwise it writes chapter 242's attempt 0 for task r at seed seed + q.
// Its replies are fixed before the run, which a scripted model in chapter
// 200's loop needs: the calculator is exact, so the sums it will return are
// known. Returns the number of replies.
mh_script :: (h:MhHarness,c:MhConfig,t:EnTask,r:int,q:int,script:[]int,starts:[]int)->int {
    n:=0;at:=0;
    if c.tools && t.kind==EnKind.prefix {
        sum:=t.work.v[0];
        for i:1..<t.work.size {
            script[at]=HL_CALL;at=mh_number(sum,script,at+1);script[at]=43;
            at=mh_number(t.work.v[i],script,at+1);script[at]=HL_CLOSE;at+=1;
            sum+=t.work.v[i];n+=1;starts[n]=at;
        }
        at=mh_text(en_solve(t),script,at);
    } else {
        at=mh_text(en_attempt(EnAgent.{seed=h.seed+cast(u64)q,skill=h.skill},t,r,0),script,at);
    }
    script[at]=HL_END;n+=1;starts[n]=at+1;
    return n;
}
```

The model is a stand-in, so that the harness is what changes between runs
and every run is exact. Without the calculator it writes chapter 242's
attempt: right with probability equal to its skill, otherwise one value
bumped by one, the last value dropped, or the input handed back. Offered
the calculator on a running-sums task, it adds one value per call and then
writes the sums, which are then right.

Chapter 200's scripted model plays a fixed list of replies. That works here
because the calculator is exact: the stand-in knows in advance what each
call will return. The loop still runs each call, appends its output and
charges for it, and the run's record holds both.

## One run through the loop

<!-- source: psionic/lib/harness/measure.coax run -->
```jai
// Task r, run q: the prompt, the stand-in's replies, and chapter 200's loop
// under the configuration's budgets. Only an answer can be right, and only
// when chapter 242's exact checker passes it. The digest covers the harness,
// the configuration, the environment, r and q, the replies scripted, the
// turn, each step's seed and every token of the transcript.
mh_run :: (h:MhHarness,c:MhConfig,t:EnTask,r:int,q:int)->MhRun {tb:=NC_LEARNED;return mh_turn(tb[..],h,c,t,r,q);}
mh_turn :: (tb:[]u8,h:MhHarness,c:MhConfig,t:EnTask,r:int,q:int)->MhRun {
    out:MhRun;
    if !mh_harness_ok(h) || !mh_config_ok(c) || r<0 || q<0 || q>=h.runs {out.status=MhStatus.shape;return out;}
    script:[MH_SCRIPT_MAX]int;starts:[MH_STEPS_MAX+1]int;replies:=mh_script(h,c,t,r,q,script[..],starts[..]);
    ids:[MH_CONTEXT_MAX]int;p:=mh_prompt(t,ids[..]);steps:[MH_STEPS_MAX]HlStep;
    b:=HlBudget.{steps=c.steps,tokens=c.tokens,deadline=c.deadline,reply=c.reply,tool=h.tool};
    u:=hl_run(hl_scripted(tb,script[..],starts[..],h.context),b,h.seed+cast(u64)q,p,ids[..],steps[..]);
    out.stop=u.stop;out.steps=u.steps;out.tokens=u.tokens;out.tools=u.tools;out.ticks=u.ticks;
    if u.stop==HlStop.answer {a:=mh_answer(ids[u.reply..<u.at]);out.solved=a.ok && en_check(EnChecker.exact,t,a.work);}
    buf:[1536]u8;at:=su_text(buf[..],0,"psionic.harness-bench.run.v1");hd:=mh_harness_digest(h);cd:=mh_config_digest(c);
    ed:=en_digest(EnEnv.{task=t,checker=EnChecker.exact,attempts=1,reference=en_solve(t)});
    for i:0..<32 {buf[at+i]=hd[i];buf[at+32+i]=cd[i];buf[at+64+i]=ed[i];}
    at=su_u64(buf[..],at+96,cast(u64)r);at=su_u64(buf[..],at,cast(u64)q);at=su_u64(buf[..],at,cast(u64)replies);buf[at]=cast(u8)u.stop;
    at=su_u64(buf[..],at+1,cast(u64)u.steps);at=su_u64(buf[..],at,cast(u64)u.tokens);at=su_u64(buf[..],at,cast(u64)u.tools);
    at=su_u64(buf[..],at,cast(u64)u.ticks);at=su_flag(buf[..],at,out.solved);at=su_u64(buf[..],at,cast(u64)u.at);
    for k:0..<u.steps {at=su_u64(buf[..],at,steps[k].seed);}
    for i:0..<u.at {buf[at]=cast(u8)(ids[i]&255);buf[at+1]=cast(u8)(ids[i]>>8);at+=2;}
    out.digest=su_hash(buf[0..<at]);
    return out;
}
```

A run is one task under one configuration, at run q's seed. It renders the
prompt, scripts the stand-in, and hands both to chapter 200's `hl_run`
with the configuration's budgets. Only an answer can be right: a turn that
ends any other way scores 0. An answer is numbers of one to four digits
separated by single spaces, at most 16 of them, and chapter 242's exact
checker decides whether it is right.

### By hand: the hand suite

The hand suite has three held-out tasks: sort [3 1 2 1], running sums
[1 2 3 4], and remove repeats [2 2 5 2]. The harness has skill 0.5, seeds
7 and 8 (two runs, in blocks of one), a window of 128 tokens and 3 ticks a
tool call. Every prompt is 13 tokens.

The stand-in's attempts:

| Run | Sort [3 1 2 1] | Sums [1 2 3 4] | Repeats [2 2 5 2] |
| --- | --- | --- | --- |
| 0 (seed 7) | [1 1 2 4], 3 bumped | [1 3 7 10], 6 bumped | [2], last dropped |
| 1 (seed 8) | [1 1 2 3], right | [1 2 3 4], input | [2 2 5 2], input |

**Direct**, the loop with no calculator: one step per task. An answer of w
tokens draws w + 1, counting the <|assistant_end|> that ends it, and the
step reads positions up to 13 + w. [1 3 7 10] is 8 tokens (two for 10), so
it draws 9 and costs 21 ticks; [2] draws 2 and costs 14; the four-value
answers draw 8 and cost 20. One run of six is right: run 1's sort.

**Calculator**: the sums take four steps.

| Step | Saw | Reply | Drew | Read up to | Tool output |
| --- | --- | --- | --- | --- | --- |
| 0 | 13 | <\|python_start\|> `1+2` <\|python_end\|> | 5 | 17 | <\|output_start\|> `3` <\|output_end\|> |
| 1 | 21 | <\|python_start\|> `3+3` <\|python_end\|> | 5 | 25 | <\|output_start\|> `6` <\|output_end\|> |
| 2 | 29 | <\|python_start\|> `6+4` <\|python_end\|> | 5 | 33 | <\|output_start\|> `10` <\|output_end\|> |
| 3 | 38 | `1 3 6 10` | 9 | 46 | |

That is 5 + 5 + 5 + 9 = 24 tokens, 3 tool calls, and
17 + 25 + 33 + 46 + 3 × 3 = 130 ticks, and the answer is right. The sort
and the repeats are the same as direct, so three runs of six are right.

Tighter budgets end the same turn with chapter 200's typed stops:
- **3 steps:** the loop stops before the answer, at `steps`, after 15
  tokens and 17 + 25 + 33 + 9 = 84 ticks.
- **20 tokens:** step 3 may draw only 5, and ends at `tokens`; it reads up
  to position 42, for 126 ticks.
- **8 tokens a reply:** step 3 draws 8 without its end token, and ends at
  `length`, after 23 tokens and 129 ticks.
- **a deadline of 80 ticks:** before step 2, 48 ticks are spent and the
  transcript is 29 tokens, so a reply may draw only 80 - 48 - 29 + 1 = 4.
  The call is cut at `deadline`, after 14 tokens, 2 tool calls and 80
  ticks.
- **a window of 40 tokens:** step 3 has room for 2, and ends at `window`.

## A run's record and its replay

<!-- source: psionic/lib/harness/measure.coax replay -->
```jai
mh_run_replay :: (h:MhHarness,c:MhConfig,t:EnTask,r:int,q:int,claimed:MhRun)->bool {
    x:=mh_run(h,c,t,r,q);
    return x.status==claimed.status && x.stop==claimed.stop && x.solved==claimed.solved && x.steps==claimed.steps &&
        x.tokens==claimed.tokens && x.tools==claimed.tools && x.ticks==claimed.ticks && su_same(x.digest,claimed.digest);
}
```

A run's digest covers the harness, the configuration, the environment,
the task and run numbers, the number of scripted replies, the turn's stop
and counts, whether it was right, each step's seed in chapter 200's record,
and every token of the transcript, prompt, replies and tool outputs alike.
A replay runs it again from the same seeds and accepts the record only
when every field and the digest come out the same.

## A suite is three digests

<!-- source: psionic/lib/harness/measure.coax digests -->
```jai
mh_harness_digest :: (h:MhHarness)->[32]u8 {
    buf:[160]u8;at:=su_text(buf[..],0,"psionic.harness-bench.harness.v1");at=su_text(buf[..],at,"cpu-harness-loop-v1 agent-sim-tools-v1");
    at=su_u64(buf[..],at,cast(u64)h.context);at=su_u64(buf[..],at,cast(u64)h.tool);at=su_u64(buf[..],at,f64_bits(h.skill));
    at=su_u64(buf[..],at,h.seed);at=su_u64(buf[..],at,cast(u64)h.runs);at=su_u64(buf[..],at,cast(u64)h.block);
    return su_hash(buf[0..<at]);
}
mh_config_digest :: (c:MhConfig)->[32]u8 {
    buf:[96]u8;at:=su_text(buf[..],0,"psionic.harness-bench.config.v1");at=su_flag(buf[..],at,c.tools);
    at=su_u64(buf[..],at,cast(u64)c.steps);at=su_u64(buf[..],at,cast(u64)c.tokens);at=su_u64(buf[..],at,cast(u64)c.reply);
    return su_hash(buf[0..<su_u64(buf[..],at,cast(u64)c.deadline)]);
}
mh_gate_digest :: (g:MhGate)->[32]u8 {
    buf:[96]u8;at:=su_text(buf[..],0,"psionic.harness-bench.gate.v1");at=su_u64(buf[..],at,cast(u64)g.replicates);
    at=su_u64(buf[..],at,g.seed);at=su_u64(buf[..],at,f64_bits(g.cost_limit));
    return su_hash(buf[0..<su_u64(buf[..],at,f64_bits(g.time_limit))]);
}
// The suite's three digests, as in chapter 239: the items (every
// environment, with the split), the questions (every prompt) and the gate.
mh_suite :: (tasks:[]EnTask,split:int,g:MhGate)->MhSuite {
    n:=tasks.count;
    if n<1 || n>MH_TASKS_MAX || split<0 || split>n || g.replicates<2 || g.replicates>NS_REPLICATES_MAX {return MhSuite.{status=MhStatus.shape};}
    if !tb_in(g.cost_limit,0,NS_BIG) || !tb_in(g.time_limit,0,NS_BIG) {return MhSuite.{status=MhStatus.shape};}
    s:=MhSuite.{tasks=n,split=split,gate=g,gates=mh_gate_digest(g)};
    buf:[2400]u8;at:=su_text(buf[..],0,"psionic.harness-bench.items.v1");at=su_u64(buf[..],at,cast(u64)n);at=su_u64(buf[..],at,cast(u64)split);
    for t:tasks {d:=en_digest(EnEnv.{task=t,checker=EnChecker.exact,attempts=1,reference=en_solve(t)});for i:0..<32 {buf[at+i]=d[i];}at+=32;}
    s.items=su_hash(buf[0..<at]);
    ids:[64]int;at=su_text(buf[..],0,"psionic.harness-bench.questions.v1");at=su_u64(buf[..],at,cast(u64)n);
    for t:tasks {p:=mh_prompt(t,ids[..]);for i:0..<p {buf[at]=cast(u8)(ids[i]&255);buf[at+1]=cast(u8)(ids[i]>>8);at+=2;}}
    s.questions=su_hash(buf[0..<at]);
    return s;
}
// The suite was built from these tasks and its gate, and nothing changed since.
mh_suite_ok :: (s:MhSuite,tasks:[]EnTask)->bool {
    if s.status!=MhStatus.ok {return false;}
    x:=mh_suite(tasks,s.split,s.gate);
    return x.status==MhStatus.ok && su_same(x.items,s.items) && su_same(x.questions,s.questions) && su_same(x.gates,s.gates);
}
```

The suite's items digest covers each task as chapter 242's environment
(task, exact checker, one attempt, reference answer) and the split between
development and held-out tasks. Its questions digest covers every prompt,
and its gate digest the verdict's settings: bootstrap replicates and seed,
and limits on tokens and ticks per run. The harness digest covers what
stays fixed between configurations: chapter 200's window and tool cost,
the stand-in's skill, and the seed blocks. The configuration digest covers
what may change: the calculator and each of chapter 200's budgets.

## A receipt per configuration

<!-- source: psionic/lib/harness/measure.coax bench -->
```jai
mh_chain :: (d:[32]u8,run:[32]u8)->[32]u8 {buf:[64]u8;for i:0..<32 {buf[i]=d[i];buf[32+i]=run[i];}return su_hash(buf[..]);}
// Every task of one partition (0: development, 1: held out), once per run:
// scores[q*n+i] is 1 when run q got task i right. The receipt binds the
// suite, harness and configuration digests, the totals, the chain of run
// digests and the digest of the scores.
mh_bench :: (h:MhHarness,c:MhConfig,s:MhSuite,tasks:[]EnTask,partition:int,scores:[]f64)->MhReceipt {
    if !mh_harness_ok(h) || !mh_config_ok(c) || partition<0 || partition>1 {return MhReceipt.{status=MhStatus.shape};}
    if !mh_suite_ok(s,tasks) {return MhReceipt.{status=MhStatus.suite};}
    first:=ifx partition==0 then 0 else s.split;last:=ifx partition==0 then s.split else s.tasks;n:=last-first;
    if n<1 || scores.count!=h.runs*n {return MhReceipt.{status=MhStatus.shape};}
    out:=MhReceipt.{items=s.items,questions=s.questions,gates=s.gates,harness=mh_harness_digest(h),config=mh_config_digest(c),partition=partition};
    out.totals.tasks=n;tb:=NC_LEARNED;
    for q:0..<h.runs {
        for i:first..<last {
            x:=mh_turn(tb[..],h,c,tasks[i],i,q);m:=out.totals;
            m.runs+=1;m.steps+=x.steps;m.tokens+=x.tokens;m.tools+=x.tools;m.ticks+=x.ticks;m.stops[cast(int)x.stop]+=1;
            scores[q*n+i-first]=ifx x.solved then 1.0 else 0.0;
            if x.solved {m.solved+=1;}
            out.totals=m;out.runs=mh_chain(out.runs,x.digest);
        }
    }
    out.values=su_values(scores);
    return out;
}
```

The bench runs every task of one partition once per seed. Run q's score on
task i is 1 or 0, stored at q × n + i. The receipt keeps the totals, counts
each stop, chains every run's digest in order, and binds the scores by
their digest. Running the bench again replays every run and must give the
same receipt body.

For the hand suite, direct gets 1 of 6 runs right, with 43 tokens and 115
ticks in all; the calculator gets 3 of 6, with 74 tokens, 6 tool calls and
334 ticks.

<!-- source: psionic/lib/harness/measure.coax chain -->
```jai
mh_receipt_digest :: (r:MhReceipt)->[32]u8 {
    buf:[512]u8;at:=su_text(buf[..],0,"psionic.harness-bench.receipt.v1");at=su_u64(buf[..],at,cast(u64)r.sequence);
    for i:0..<32 {buf[at+i]=r.previous[i];buf[at+32+i]=r.items[i];buf[at+64+i]=r.questions[i];buf[at+96+i]=r.gates[i];buf[at+128+i]=r.harness[i];buf[at+160+i]=r.config[i];}
    at=su_u64(buf[..],at+192,cast(u64)r.partition);m:=r.totals;
    for v:[7]int.[m.tasks,m.runs,m.solved,m.steps,m.tokens,m.tools,m.ticks] {at=su_u64(buf[..],at,cast(u64)v);}
    for v:m.stops {at=su_u64(buf[..],at,cast(u64)v);}
    for i:0..<32 {buf[at+i]=r.runs[i];buf[at+32+i]=r.values[i];}
    return su_hash(buf[0..<at+64]);
}
// Append at count: the receipt takes the next sequence number and the
// previous receipt's digest. index is the new count.
mh_append :: (store:[]MhReceipt,count:int,r:MhReceipt)->MhCheck {
    if count<0 || count>store.count || r.status!=MhStatus.ok {return MhCheck.{status=MhStatus.shape};}
    if count==store.count {return MhCheck.{status=MhStatus.capacity};}
    e:=r;e.sequence=count;e.previous=ifx count==0 then su_zero() else store[count-1].digest;
    e.digest=mh_receipt_digest(e);store[count]=e;
    return MhCheck.{index=count+1};
}
// The first receipt whose sequence, link or digest does not hold.
mh_verify :: (store:[]MhReceipt,count:int)->MhCheck {
    if count<0 || count>store.count {return MhCheck.{status=MhStatus.shape};}
    for r,i:store[0..<count] {
        previous:=ifx i==0 then su_zero() else store[i-1].digest;
        if r.sequence!=i || !su_same(r.previous,previous) || !su_same(r.digest,mh_receipt_digest(r)) {return MhCheck.{status=MhStatus.broken,index=i};}
    }
    return MhCheck.{};
}
```

Receipts go into a hash chain as in chapter 239: each takes the next
sequence number and the previous receipt's digest. Verifying the chain
names the first receipt whose sequence, link or digest does not hold, so a
receipt that claims one more right run is found.

## Verdicts, one change at a time

<!-- source: psionic/lib/harness/measure.coax verdict -->
```jai
mh_changes :: (a:MhConfig,b:MhConfig)->int {
    n:=ifx a.tools!=b.tools then 1 else 0;
    for d:[4]int.[a.steps-b.steps,a.tokens-b.tokens,a.reply-b.reply,a.deadline-b.deadline] {if d!=0 {n+=1;}}
    return n;
}
// Receipt i (the baseline, ac) against receipt j (the candidate, bc), with
// the scores each one's values digest pinned. Refused, in this order: a bad
// index or harness, a broken chain, another suite, another harness, a configuration
// the receipt did not pin, development tasks, no change or more than one,
// scores that are not the pinned ones. Otherwise chapter 241's decision on
// the per-task means over runs, beyond chapter 240's floor from the
// baseline's seed blocks, then its evaluation of tokens and ticks per run.
mh_verdict :: (store:[]MhReceipt,count:int,i:int,j:int,s:MhSuite,h:MhHarness,ac:MhConfig,bc:MhConfig,sa:[]f64,sb:[]f64)->MhVerdict {
    if i<0 || j<0 || i>=count || j>=count || i==j {return MhVerdict.{status=MhStatus.shape};}
    if mh_verify(store,count).status!=MhStatus.ok {return MhVerdict.{status=MhStatus.broken};}
    a:=store[i];b:=store[j];
    if s.status!=MhStatus.ok || !su_same(s.gates,mh_gate_digest(s.gate)) {return MhVerdict.{status=MhStatus.suite};}
    for r:[2]MhReceipt.[a,b] {
        if !su_same(r.items,s.items) || !su_same(r.questions,s.questions) || !su_same(r.gates,s.gates) {return MhVerdict.{status=MhStatus.suite};}
    }
    if !mh_harness_ok(h) {return MhVerdict.{status=MhStatus.shape};}
    hd:=mh_harness_digest(h);
    if !su_same(a.harness,hd) || !su_same(b.harness,hd) {return MhVerdict.{status=MhStatus.harness};}
    if !su_same(a.config,mh_config_digest(ac)) || !su_same(b.config,mh_config_digest(bc)) {return MhVerdict.{status=MhStatus.unpinned};}
    if a.partition!=1 || b.partition!=1 {return MhVerdict.{status=MhStatus.development};}
    k:=mh_changes(ac,bc);
    if k==0 {return MhVerdict.{status=MhStatus.same};}
    if k>1 {return MhVerdict.{status=MhStatus.changes};}
    n:=a.totals.tasks;
    if sa.count!=h.runs*n || sb.count!=h.runs*n {return MhVerdict.{status=MhStatus.shape};}
    if !su_same(su_values(sa),a.values) || !su_same(su_values(sb),b.values) {return MhVerdict.{status=MhStatus.values};}
    ma:[MH_TASKS_MAX]f64;mb:[MH_TASKS_MAX]f64;task:[MH_TASKS_MAX]int;part:[MH_TASKS_MAX]int;
    for t:0..<n {task[t]=t;for q:0..<h.runs {ma[t]+=sa[q*n+t];mb[t]+=sb[q*n+t];}ma[t]/=f64(h.runs);mb[t]/=f64(h.runs);}
    v:=MhVerdict.{floor=ns_floor(sa,part[0..<n],0,h.runs,h.block)};
    v.decision=vd_decision(ma[0..<n],mb[0..<n],task[0..<n],part[0..<n],0,v.floor,1.0,s.gate.seed,s.gate.replicates);
    v.cost=f64(b.totals.tokens)/f64(b.totals.runs);v.time=f64(b.totals.ticks)/f64(b.totals.runs);
    v.result=vd_evaluate(v.decision.result,v.cost,v.time,s.gate.cost_limit,s.gate.time_limit);
    return v;
}
```

A verdict compares a baseline receipt with a candidate receipt from one
verified chain. It refuses, in this order:
- an index outside the chain, or the same receipt twice: `shape`;
- a chain that does not verify: `broken`;
- either receipt from a suite other than the one given, or a suite whose
  gate changed after it was pinned: `suite`;
- a harness out of range (`shape`), or either receipt from another
  harness, including other seeds: `harness`;
- a configuration other than the one the receipt pinned: `unpinned`;
- development tasks: a gain on the tasks the change was built from does
  not count, so only held-out receipts compare: `development`;
- two configurations that are the same (`same`), or that differ in more
  than one setting (`changes`), since then the verdict could not say which
  change did it;
- scores that are not the ones the receipts pinned: `values`.

Otherwise each held-out task's score is its mean over the runs. Chapter
240's floor comes from the baseline's runs in blocks. Chapter 241 decides on
the per-task gains, redrawing whole tasks, and then lets cost and time
block a pass. Its verdict is passed, failed or unverifiable, with the
reason. Three tasks are too few for chapter 241, so the hand suite's
calculator against direct is unverifiable (`few_tasks`), not refused.

## Run it

The suite is 40 tasks from chapter 242's generator at seed 211: the first
10 for development, 30 held out (11 sort, 11 remove repeats, 8 running
sums). The harness has skill 0.6, seeds 2110 to 2117 in four blocks of
two, a window of 512 and 4 ticks a call. The gate draws 1,000 replicates at
seed 2111 and limits a run to 60 tokens and 3,000 ticks. There are three
configurations:
- **direct:** no calculator; 8 steps, 96 tokens, 48 a reply, a deadline
  of 100,000 ticks;
- **calculator:** the same with the calculator;
- **calculator, 16 steps:** the calculator with 16 steps.

All of this is fixed before any run. The
[example](../examples/measure-harness.coax) works the hand suite first, then
prints the digests, runs every configuration, gives the verdicts and the
refusals, and replays the receipts.

<!-- source: psionic/examples/measure-harness.coax example -->
```jai
main :: () {
    // The hand suite: three held-out tasks, two runs, two configurations.
    hand:=mh_hand();
    print("the hand suite, seeds 7 and 8, skill 0.5:\n");
    for c,k:[2]MhConfig.[MH_DIRECT,MH_CALC] {
        for q:0..<2 {for t,r:hand {
            print("  % run % % ",ifx k==0 then "direct    " else "calculator",q,KIND[cast(int)t.kind]);show_work(t.work);print(": ");
            show_run(mh_run(MH_H,c,t,r,q));
        }}
    }
    print("  the calculator's sums under tighter budgets (3 steps, 20 tokens, 8 a reply, deadline 80):\n");
    budgets:=[4]MhConfig.[MH_CALC,MH_CALC,MH_CALC,MH_CALC];budgets[0].steps=3;budgets[1].tokens=20;budgets[2].reply=8;budgets[3].deadline=80;
    for c:budgets {print("    ");show_run(mh_run(MH_H,c,hand[1],1,0));}
    small:=MH_H;small.context=40;print("    window 40: ");show_run(mh_run(small,MH_CALC,hand[1],1,0));

    // The suite, harness and configurations, pinned before any run below.
    tasks:[40]EnTask;#assert en_tasks(211,tasks[..])==EnStatus.ok;
    gate:=MhGate.{replicates=1000,seed=2111,cost_limit=60,time_limit=3000};
    suite:=mh_suite(tasks[..],10,gate);
    h:=MhHarness.{context=512,tool=4,skill=0.6,seed=2110,runs=8,block=2};
    direct:=MhConfig.{tools=false,steps=8,tokens=96,reply=48,deadline=100000};
    calc:=direct;calc.tools=true;
    long:=calc;long.steps=16;
    kinds:[3]int;for t:tasks[10..<40] {kinds[cast(int)t.kind]+=1;}
    print("suite: 40 tasks from seed 211, 10 development and 30 held out (% sort, % dedupe, % prefix); items ",kinds[0],kinds[1],kinds[2]);
    hex8(suite.items);print(", questions ");hex8(suite.questions);print(", gate ");hex8(suite.gates);
    print("\nharness: chapter 200's loop, window 512, 4 ticks a call, skill 0.6, seeds 2110 to 2117 in 4 blocks of 2; digest ");hex8(mh_harness_digest(h));print("\n");

    // Every configuration on the held-out tasks, into one receipt chain.
    store:[8]MhReceipt;count:=0;
    sd:[240]f64;sc:[240]f64;sl:[240]f64;dev:[80]f64;
    count=mh_append(store[..],count,mh_bench(h,direct,suite,tasks[..],1,sd[..])).index;
    count=mh_append(store[..],count,mh_bench(h,calc,suite,tasks[..],1,sc[..])).index;
    count=mh_append(store[..],count,mh_bench(h,long,suite,tasks[..],1,sl[..])).index;
    count=mh_append(store[..],count,mh_bench(h,calc,suite,tasks[..],0,dev[..])).index;
    print("held out, 30 tasks x 8 runs:\n");
    show_totals("direct, 8 steps         ",store[0]);
    show_totals("calculator, 8 steps     ",store[1]);
    show_totals("calculator, 16 steps    ",store[2]);
    print("development, 10 tasks x 8 runs:\n");show_totals("calculator, 8 steps     ",store[3]);

    // Verdicts: one change at a time, on held-out tasks only.
    print("verdicts:\n");
    show_verdict("calculator against direct",mh_verdict(store[..],count,0,1,suite,h,direct,calc,sd[..],sc[..]));
    show_verdict("16 steps against 8, with the calculator",mh_verdict(store[..],count,1,2,suite,h,calc,long,sc[..],sl[..]));
    show_verdict("16 steps and the calculator against direct",mh_verdict(store[..],count,0,2,suite,h,direct,long,sd[..],sl[..]));
    show_verdict("on the development tasks",mh_verdict(store[..],count,0,3,suite,h,direct,calc,sd[..],dev[..]));
    other:[40]EnTask;#assert en_tasks(212,other[..])==EnStatus.ok;
    show_verdict("against another suite",mh_verdict(store[..],count,0,1,mh_suite(other[..],10,gate),h,direct,calc,sd[..],sc[..]));
    seeds:=h;seeds.seed=2120;show_verdict("under other seeds",mh_verdict(store[..],count,0,1,suite,seeds,direct,calc,sd[..],sc[..]));
    loose:=calc;loose.tokens=128;show_verdict("with a configuration the receipt did not pin",mh_verdict(store[..],count,0,1,suite,h,direct,loose,sd[..],sc[..]));
    swapped:=sc;swapped[0]=1-swapped[0];show_verdict("with one score changed",mh_verdict(store[..],count,0,1,suite,h,direct,calc,sd[..],swapped[..]));

    // Replays: every held-out receipt from its seeds, which reruns every
    // run and compares the chain of run digests; then the receipt chain.
    scratch:[240]f64;bodies:=0;
    for c,k:[3]MhConfig.[direct,calc,long] {if mh_bench_replay(h,c,suite,tasks[..],1,store[k],scratch[..]) {bodies+=1;}}
    print("replay: % of 3 held-out receipts (720 runs) reproduce from their seeds; chain of % receipts: %\n",bodies,count,
        STATUS[cast(int)mh_verify(store[..],count).status]);
    edited:=store;edited[1].totals.solved+=1;c:=mh_verify(edited[..],count);
    print("  receipt 1 claimed with one more right run: % at receipt %\n",STATUS[cast(int)c.status],c.index);
}
```

```text
the hand suite, seeds 7 and 8, skill 0.5:
  direct     run 0 sort [3 1 2 1]: stop answer, wrong, 1 steps, 8 tokens, 0 tool calls, 20 ticks
  direct     run 0 prefix [1 2 3 4]: stop answer, wrong, 1 steps, 9 tokens, 0 tool calls, 21 ticks
  direct     run 0 dedupe [2 2 5 2]: stop answer, wrong, 1 steps, 2 tokens, 0 tool calls, 14 ticks
  direct     run 1 sort [3 1 2 1]: stop answer, right, 1 steps, 8 tokens, 0 tool calls, 20 ticks
  direct     run 1 prefix [1 2 3 4]: stop answer, wrong, 1 steps, 8 tokens, 0 tool calls, 20 ticks
  direct     run 1 dedupe [2 2 5 2]: stop answer, wrong, 1 steps, 8 tokens, 0 tool calls, 20 ticks
  calculator run 0 sort [3 1 2 1]: stop answer, wrong, 1 steps, 8 tokens, 0 tool calls, 20 ticks
  calculator run 0 prefix [1 2 3 4]: stop answer, right, 4 steps, 24 tokens, 3 tool calls, 130 ticks
  calculator run 0 dedupe [2 2 5 2]: stop answer, wrong, 1 steps, 2 tokens, 0 tool calls, 14 ticks
  calculator run 1 sort [3 1 2 1]: stop answer, right, 1 steps, 8 tokens, 0 tool calls, 20 ticks
  calculator run 1 prefix [1 2 3 4]: stop answer, right, 4 steps, 24 tokens, 3 tool calls, 130 ticks
  calculator run 1 dedupe [2 2 5 2]: stop answer, wrong, 1 steps, 8 tokens, 0 tool calls, 20 ticks
  the calculator's sums under tighter budgets (3 steps, 20 tokens, 8 a reply, deadline 80):
    stop steps, wrong, 3 steps, 15 tokens, 3 tool calls, 84 ticks
    stop tokens, wrong, 4 steps, 20 tokens, 3 tool calls, 126 ticks
    stop length, wrong, 4 steps, 23 tokens, 3 tool calls, 129 ticks
    stop deadline, wrong, 3 steps, 14 tokens, 2 tool calls, 80 ticks
    window 40: stop window, wrong, 4 steps, 17 tokens, 3 tool calls, 123 ticks
suite: 40 tasks from seed 211, 10 development and 30 held out (11 sort, 11 dedupe, 8 prefix); items 7495cedf, questions c2df9da7, gate e60834fa
harness: chapter 200's loop, window 512, 4 ticks a call, skill 0.6, seeds 2110 to 2117 in 4 blocks of 2; digest b36e9532
held out, 30 tasks x 8 runs:
  direct, 8 steps         : 146 of 240 runs right (0.6083), per run 1.0 steps, 19.24 tokens, 0.0 tool calls, 43.37 ticks
    stops: answer 240; config 19d36c35, runs 8d9a870e, receipt 36f0e5ed
  calculator, 8 steps     : 133 of 240 runs right (0.5542), per run 2.77 steps, 25.12 tokens, 1.93 tool calls, 164.09 ticks
    stops: answer 200 steps 40; config 209af09b, runs dda4f9e4, receipt 4eab1e39
  calculator, 16 steps    : 157 of 240 runs right (0.6542), per run 3.4 steps, 32.42 tokens, 2.4 tool calls, 251.12 ticks
    stops: answer 224 tokens 16; config 5f691bc3, runs 5863dd93, receipt 7cbb92c1
development, 10 tasks x 8 runs:
  calculator, 8 steps     : 51 of 80 runs right (0.6375), per run 3.8 steps, 29.08 tokens, 3.0 tool calls, 215.48 ticks
    stops: answer 64 steps 16; config 209af09b, runs c1bd70fd, receipt 205ff3f1
verdicts:
  calculator against direct: floor 0.2572 from 4 blocks, gain -0.0542 (se 0.0463, 1000 of 1000 replicates at or below the floor), headroom 0.3917; cost 25.12 tokens, time 164.09 ticks: failed (within the floor)
  16 steps against 8, with the calculator: floor 0.2212 from 4 blocks, gain 0.1 (se 0.0569, 969 of 1000 replicates at or below the floor), headroom 0.4458; cost 32.42 tokens, time 251.12 ticks: unverifiable (near the floor)
  16 steps and the calculator against direct: refused: more than one change
  on the development tasks: refused: development tasks
  against another suite: refused: another suite
  under other seeds: refused: another harness
  with a configuration the receipt did not pin: refused: configuration not pinned
  with one score changed: refused: scores not pinned
replay: 3 of 3 held-out receipts (720 runs) reproduce from their seeds; chain of 4 receipts: ok
  receipt 1 claimed with one more right run: broken chain at receipt 1
```

What the run shows:
- **The calculator alone loses.** It makes every short running-sums task
  right, but a task of more than 8 values needs more than 8 steps, so 40
  runs stop at `steps` with no answer. Without the calculator those tasks
  were right about 60 percent of the time. Its gain is -0.054, and every
  replicate lands at or below the floor of 0.257: failed, within the floor.
  It also costs 25 tokens and 164 ticks a run against 19 and 43.
- **16 steps help, but not clearly.** With 16 steps the step stops are gone;
  16 runs now stop at `tokens` instead. The gain over the calculator with 8
  steps is 0.1, but 969 of 1,000 replicates are at or below the floor:
  unverifiable, near the floor. Four blocks of two runs make a wide floor.
  More seed blocks would narrow it; this chapter reports what these blocks
  show.
- **Both changes at once are refused.** The calculator with 16 steps
  against direct differs in two settings, so it gets no verdict, though its
  rate, 0.654, is above direct's 0.608.
- **The other refusals:** development tasks, another suite (seed 212),
  other seeds, a configuration the receipt did not pin, and one changed
  score each get their own status.
- **Replay:** every held-out receipt replays from its seeds, which reruns
  all 720 runs, and the chain verifies. A receipt claimed with one more
  right run breaks the chain at that receipt.

By the draft's rule, an addition with no measured gain is removed. Here the
calculator with 8 steps fails and is not kept; the step budget it needs is
unverifiable on this suite.

## Checks

The unit's checks:
- **the hand suite**, run by run and in total: every attempt, step, token,
  tool call and tick above, and each budget's typed stop;
- **prompts, numbers and answers** as tokens, with every answer the parser
  refuses;
- **ranges:** each harness and configuration setting at its edge and one
  past it;
- **records:** each field of a run, a receipt and the chain, changed alone,
  is caught by its replay or its digest;
- **the suite's digests:** each one moves with what it covers and only that;
- **verdict refusals** in order, and refusals on development tasks;
- **verdict rules** (`#forall`, 12 seeds): on 12 held-out tasks with chosen
  scores in 4 runs, the verdict equals chapter 240's floor and chapter 241's
  decision and evaluation computed directly from per-task means worked out
  separately; a pass always means a gain clear of the floor and both limits
  held; a run over a limit always fails; and changing one score is always
  refused.

The hand suite and the checks' helpers live in
[harness-measure-checks.coax](../tests/harness-measure-checks.coax).
`coax mutate psionic/lib/harness/measure.coax --root-only` kills all 590
mutants. Earlier runs left survivors that sharpened the checks: the chain's
sequence and link checks were only reached together with the digest, so
resealed forgeries now test each alone; a harness out of range now has its
own refusal; and pinned digests fix the run, receipt and suite encodings.

## What remains

- **Benchmarks of the harness's own operations:** opening a session,
  folding a long conversation with
  [chapter 206](ch206-condense-the-history.md), reconnecting a stream and
  continuing an agent, each with P50 and P95 wall-clock time on a named
  host. Nothing here measures seconds.
- **A real model.** The stand-in is scripted. Running the suite with a
  model behind chapter 200's loop, with real token counts, remains.
- **Cache hits and money.** Cost is tokens drawn and time is logical clock
  ticks, with no cache and no price.
- **More additions under test.** Prompt assembly from
  [chapter 202](ch202-keep-the-prefix-stable.md), condensation from
  chapter 206 and the protocols of
  [chapter 210](ch210-speak-the-protocols.md) are not yet configurations
  of this bench; only the calculator and chapter 200's budgets are.
- **Chapters 251 and 252** add system-level runs (an ablation of Router
  and Workers, a Router batching stress test and fault injection for the
  watchdog) that will report through this chapter.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
- [Chapter 200](ch200-the-simplest-loop.md): the loop, its budgets and its
  typed stops.
- [Chapter 239](ch239-a-suite-is-three-digests.md),
  [chapter 240](ch240-how-far-a-metric-moves-by-itself.md) and
  [chapter 241](ch241-gates-that-can-say-unverifiable.md): suites, noise
  floors and verdicts.
- [Chapter 242](ch242-environments-with-checkers.md): the tasks, the exact
  checker and the simulated agent.
