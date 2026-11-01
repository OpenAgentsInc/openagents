# 131. Train the same model eager and compiled

> **Status: partly implemented.** Chapter 117's block trains under chapter
> 118's trainer three ways on the CPU (eager, compiled, and compiled with
> recomputation) to the same losses, weights and AdamW moments, bit for bit.
> Each path's memory and launches are counted, step times are compared with
> chapter 34's statistics in a JSON receipt, and a comparison of runs that
> trained different things is refused. Bounded dynamic batch sizes through
> chapter 39's guarded specialization, fusion of forward and backward
> operations, the optimizer compiled into the program, and a compiled GPU
> path are not here yet.

Compilation must change speed and memory, not what the model learns. The
way to show that is to train one model twice, once each way, under the same
trainer, data order and random-number policy, and compare everything the
runs produce.

[Chapter 106](ch106-differentiate-any-tensor-program.md)'s tape is the
eager path. It is planned, but it stays an interpreter: each step looks up
its node, rebuilds the kernel's metadata and finds its operands through the
program. This chapter goes further, as a define-by-run framework does: the
eager path records the tape again for every training step.
[Chapter 114](ch114-compile-the-backward-pass.md) compiles the same tape
once into a straight-line program with every address resolved. Its chooser
can also recompute cheap values in the backward pass instead of saving them,
which trades launches for memory.

This chapter puts all three under
[chapter 118](ch118-one-trainer-for-every-model.md)'s trainer:

- **Three paths, one trainer.** Each path computes one micro-batch's loss
  and gradient. The trainer makes every AdamW update, so the paths can
  differ only in how the gradient is computed.
- **Bit-for-bit agreement.** Losses, weights and both AdamW moments must be
  the same bits on every path, and the same bits as chapter 118's own loop,
  which is written independently and records its tape only once.
- **A footprint for each path.** The values arena (f32), the gradient arena
  (f64), the flags workspace and the kernel launches a step.
- **A comparison with chapter 34's statistics.** Step times become a mean
  and standard deviation, computed by two-pass and by Welford's method,
  which must agree. A winner is named only beyond two standard errors.
- **The compile cost.** The report gives the number of steps after which a
  compile pays for itself, or says that it never does.
- **Refusals.** Two runs whose programs, configurations, seeds, shapes or
  step counts differ are not compared.

The profile is `train-compare-v1`: times as positive integer nanoseconds,
5 to 1,024 warm steps a side (chapter 34's `cm-warm-v1`), binary64
statistics; memory as the bytes of the values arena (4 bytes a float), the
gradient arena (8 bytes a double) and the flags workspace (1 byte an item).
The paths and footprints are in
[trainer-compiled.coax](../lib/trainer-compiled.coax). The comparison is in
[train-compare.coax](../lib/train-compare.coax), which decides admission and
the verdict, loads only chapter 34's comparator and is marked `#authority`.
The [example](../examples/eager-and-compiled.coax) prints the output below, and
the bench ([trainer-compiled-bench.coax](../tests/trainer-compiled-bench.coax))
times real steps on this machine.

```shell
coax/bin/coax run psionic/examples/eager-and-compiled.coax
coax/bin/coax test psionic/lib/train-compare.coax
coax/bin/coax test psionic/lib/trainer-compiled.coax
COAX_CC=gcc psionic/tests/check_trainer_compiled.sh
```

## The record of a run and a comparison

<!-- source: psionic/lib/train-compare.coax types -->
```jai
TcStatus :: enum u8 {ok;program;config;seed;shape;steps;few;}
TcVerdict :: enum u8 {refused;faster;slower;same;noisy;}
// What a run trained: digests of the program with its gradient mode and of
// the trainer's configuration, the initialization seed, the trained and
// input floats, and the steps.
TcIdentity :: struct {program:u64;config:u64;seed:u64;floats:int;inputs:int;steps:int;}
// What one step of a path holds and launches: values (f32) and gradients
// (f64) in floats, the flags workspace in bytes, kernel launches (forward,
// recompute and backward), and the three as bytes.
TcFootprint :: struct {values:int;grads:int;flags:int;launches:int;bytes:int;}
// One side of a comparison: its run's identity and footprint and its
// one-off cost before the first step, in nanoseconds.
TcSide :: struct {identity:TcIdentity;footprint:TcFootprint;compile:int;}
// Side b against side a: both sides' statistics by two passes (a, b) and by
// Welford's update (wa, wb), a's mean over b's (above 1 when b is faster),
// the standard error of the difference, the verdict, b's bytes over a's,
// b's launches less a's, and the steps after which b's extra one-off cost
// is paid back (-1: never, or no winner).
TcReport :: struct {
    status:TcStatus;a:CmStats;b:CmStats;wa:CmStats;wb:CmStats;ratio:f64;se:f64;verdict:TcVerdict;
    memory:f64;launches:int;break_even:int;
}
```

A comparison holds two sides, each a run's identity, footprint and
one-off cost, and gives back both sides' statistics by both methods and a
verdict.

A run's identity is what it trained. Two runs with the same identity
started from the same values, saw the same data and dropout masks, and
were stepped by the same optimizer the same number of times. The program
digest covers chapter 38's nodes, the gradient mode and each model entry's
layout and initialization. A dropout rate lives in its mask entry, not in
the program, so the entries have to be in the digest too.

<!-- source: psionic/lib/trainer-compiled.coax identity -->
```jai
// The program's nodes, which inputs train, and each entry's layout and
// initialization (a dropout rate lives in its mask entry, not the program).
tc_digest :: (m:MdlModel)->u64 {
    p:=m.program;mode:=mdl_mode(m);h:u64=14695981039346656037;h=tc_mix(h,cast(u64)p.count);h=tc_mix(h,cast(u64)p.output);
    for i:0..<p.count {
        n:=p.nodes[i];h=tc_mix(h,cast(u64)n.op);h=tc_mix(h,cast(u64)(n.a+1));h=tc_mix(h,cast(u64)(n.b+1));
        for d:0..<4 {h=tc_mix(h,cast(u64)n.shape.dims[d]);}
        h=tc_mix(h,cast(u64)n.input_base);h=tc_mix(h,cast(u64)n.axis);h=tc_mix(h,cast(u64)(ifx mode.grad[i] then 1 else 0));
    }
    for i:0..<m.count {e:=m.entries[i];h=tc_mix(h,cast(u64)e.kind);h=tc_mix(h,cast(u64)e.base);h=tc_mix(h,cast(u64)e.drop);h=tc_mix(h,cast(u64)f32_bits(e.value));}
    return h;
}
tc_config_digest :: (c:TrConfig)->u64 {
    h:u64=14695981039346656037;h=tc_mix(h,cast(u64)c.optimizer);
    for x:[6]f64.[c.rate,c.beta1,c.beta2,c.eps,c.decay,c.clip] {h=tc_mix(h,f64_bits(x));}
    return h;
}
tc_identity :: (m:MdlModel,seed:u64,c:TrConfig)->TcIdentity {
    return TcIdentity.{program=tc_digest(m),config=tc_config_digest(c),seed=seed,floats=tm_module_floats(m),inputs=m.top,steps=tr_steps(c)};
}
```

The trainer's configuration is AdamW at rate 0.01 with chapter 124's usual
betas. The block sees one example a step, so only the dropout mask changes
from step to step. It is drawn by chapter 117's counter generator from the
step number, which is the random-number policy every path shares.

<!-- source: psionic/lib/trainer-compiled.coax config -->
```jai
// AdamW at rate, chapter 124's usual betas, no decay or clipping, one
// example a step: the block sees the same batch each step and only its
// dropout mask changes. Parameters are binary64 in the trainer and are
// rounded to f32 into the model's input buffer before each step.
tc_config :: (steps:int,rate:f64)->TrConfig {
    return TrConfig.{optimizer=TrOptimizer.adamw,rate=rate,beta1=0.9,beta2=0.999,eps=1e-8,bound=AW_F32_MAX,examples=1,epochs=steps};
}
```

## Three paths

<!-- source: psionic/lib/trainer-compiled.coax kinds -->
```jai
TcPath :: enum u8 {eager;compiled;recompute;}
```

The eager path records chapter 106's tape and then runs it, every step. The
compiled paths run chapter 114's program, built once with no update
compiled in, and publish the output and gradients the same way chapter
118's tape step does. The recomputing path asks chapter 114's chooser for
a values arena of at most 127 floats.

<!-- source: psionic/lib/trainer-compiled.coax paths -->
```jai
// The compiled paths' programs: chapter 114's, gradients only (the trainer
// updates), plain or recomputing to meet TC_GOAL floats. Eager has none.
tc_program :: (path:TcPath,m:MdlModel)->CbProgram {
    if path==TcPath.compiled {return cb_compile(m.program,mdl_mode(m),OrRegistry.{},CbChoice.{});}
    if path==TcPath.recompute {return cb_choose(m.program,mdl_mode(m),OrRegistry.{},TC_GOAL,false);}
    return CbProgram.{};
}
// One micro-batch through the compiled program: the same refresh, inputs
// and gradient publication as chapter 118's tape step (tm_module_step).
tc_compiled_step :: (m:MdlModel,c:CbProgram,step:int,p:[]f64,inputs:[]f32,g:[]f64)->f64 {
    tm_module_move(m,p,inputs,true);#assert mdl_refresh(m,cast(u64)(step+TC_DROPOUT),inputs)==MdlStatus.ok;
    values:[1024]f32;grads:[1024]f64;flags:[64]u8;out:[1]f32;g32:[64]f32;mode:=mdl_mode(m);
    #assert cb_run(c,0,inputs,values[..],grads[..],flags[..])==CbStatus.ok && cb_output(c,inputs,values[..],out[..])==CbStatus.ok;
    at:=0;
    for i:0..<m.count {e:=m.entries[i];if !mode.grad[e.node] {continue;}
        #assert cb_gradient(c,e.node,grads[..],g32[0..<e.size])==CbStatus.ok;for j:0..<e.size {g[at+j]=f64(g32[j]);}at+=e.size;}
    return f64(out[0]);
}
// One micro-batch on a path. Eager records chapter 106's tape again first.
tc_step :: (path:TcPath,m:MdlModel,c:CbProgram,step:int,p:[]f64,inputs:[]f32,g:[]f64)->f64 {
    if path==TcPath.eager {return tm_module_step(m,ad_tape(m.program,mdl_mode(m),OrRegistry.{}),step,p,inputs,g);}
    return tc_compiled_step(m,c,step,p,inputs,g);
}
```

The training loop is chapter 118's: ask the trainer for the next batch, add
the micro-batch's loss and gradient, apply the step. Only the call to
`tc_step` differs between paths. `tc_whole` is the other way to compile a
step: chapter 114's program with its SGD update compiled in, run with no
trainer at all. A check shows that it reaches the trainer's SGD bits.

<!-- source: psionic/lib/trainer-compiled.coax train -->
```jai
// Initialize at seed, then train every step of c on one path under the
// trainer, recording each step's loss. Inputs end holding the trained values.
tc_train :: (path:TcPath,m:MdlModel,seed:u64,c:TrConfig,prog:CbProgram,inputs:[]f32,p:[]f64,pm:[]f64,pv:[]f64,losses:[]f64)->TrDrive {
    n:=tm_module_floats(m);
    if inputs.count<m.top || p.count!=n || (path!=TcPath.eager && prog.status!=CbStatus.ok) {return TrDrive.{status=TrStatus.shape};}
    #assert mdl_init(m,seed,inputs)==MdlStatus.ok;
    for j:0..<16 {inputs[j]=f32(f64((j*5)%16-8)/8);inputs[16+j]=f32(f64((((j/4)*4+(j+1)%4)*5)%16-8)/16);}
    tm_module_move(m,p,inputs,false);none:[1]PkDoc;nothing:[1]u8;order:[1]int;
    b:=tr_begin(c,n,none[..],nothing[..],order[..]);out:=TrDrive.{status=b.status,run=b.run};
    g:[128]f64;total:[128]f64;stage:[384]f64;
    while out.status==TrStatus.ok {
        x:=tr_next(c,out.run,none[..],nothing[..],nothing[..]);
        if x.status!=TrStatus.ok {out.status=x.status;continue;}
        s:=tr_add(x,TrSum.{},tc_step(path,m,prog,out.run.revision,p,inputs,g[0..<n]),g[0..<n],total[0..<n]);
        a:=tr_apply(c,out.run,x,s,p,pm,pv,total[0..<n],stage[0..<3*n],order[..]);out.status=a.status;
        if a.status==TrStatus.ok {losses[out.steps]=a.loss;out.run=a.run;out.steps+=1;}
    }
    tm_module_move(m,p,inputs,true);
    return out;
}
// The whole step compiled: forward, backward and chapter 114's SGD update
// in one program, no trainer. For the check against the trainer's SGD.
tc_whole :: (m:MdlModel,rate:f32,steps:int,inputs:[]f32,losses:[]f64)->CbStatus {
    c:=cb_compile(m.program,mdl_mode(m),OrRegistry.{},CbChoice.{update=true});tm_module_inputs(m,inputs);
    values:[1024]f32;grads:[1024]f64;flags:[64]u8;out:[1]f32;
    for step:0..<steps {
        #assert mdl_refresh(m,cast(u64)(step+TC_DROPOUT),inputs)==MdlStatus.ok;
        s:=cb_run(c,rate,inputs,values[..],grads[..],flags[..]);if s!=CbStatus.ok {return s;}
        #assert cb_output(c,inputs,values[..],out[..])==CbStatus.ok;losses[step]=f64(out[0]);
    }
    return CbStatus.ok;
}
```

## Memory and launches, by hand

<!-- source: psionic/lib/trainer-compiled.coax footprint -->
```jai
// Eager's arenas are the tape's; its workspace is the largest node a step
// runs, and each tape step is one launch. A compiled program states its own.
tc_footprint :: (path:TcPath,m:MdlModel,c:CbProgram)->TcFootprint {
    if path!=TcPath.eager {return tc_bytes(TcFootprint.{values=c.values.top,grads=c.grads.top,flags=c.flag_items,launches=c.launches});}
    t:=ad_tape(m.program,mdl_mode(m),OrRegistry.{});f:=TcFootprint.{values=t.values.top,grads=t.grads.top,launches=t.count};
    for s:0..<t.count {f.flags=cb_larger(f.flags,t.plan.values[t.steps[s].node].storage);}
    return tc_bytes(f);
}
```

Chapter 117's block has 27 nodes. Its tape has 30 steps: 16 forward and 14
backward rules, so eager makes 30 launches a step. Chapter 114's program
runs the same 30 commands and rules (its clears and its seed are not
launches). The largest node holds 32 floats, so each path needs 32 bytes of
flags.

The arenas give the bytes:

| Path | Values (f32) | Gradients (f64) | Flags | Bytes | Launches |
| --- | --- | --- | --- | --- | --- |
| eager | 144 × 4 = 576 | 148 × 8 = 1,184 | 32 | 1,792 | 30 |
| compiled | 144 × 4 = 576 | 140 × 8 = 1,120 | 32 | 1,728 | 30 |
| recomputing | 113 × 4 = 452 | 140 × 8 = 1,120 | 32 | 1,604 | 34 |

The compiled program places gradient buffers by its own instruction
lifetimes rather than the tape's steps, which saves 8 doubles. Recomputing
four cheap values (chapter 114's nodes 4, 6, 11 and 13) saves 31 floats and
costs 4 more launches. Compiled uses 1,728 / 1,792 = 0.964 of eager's bytes;
recomputing uses 1,604 / 1,792 = 0.895.

## When compiling wins

<!-- source: psionic/lib/train-compare.coax compare -->
```jai
// FNV-1a over 64-bit words, for the identity's digests.
tc_mix :: (h:u64,x:u64)->u64 {return mul_wrap(h^x,1099511628211);}
tc_bytes :: (f:TcFootprint)->TcFootprint {r:=f;r.bytes=4*f.values+8*f.grads+f.flags;return r;}
tc_same :: (a:TcIdentity,b:TcIdentity)->TcStatus {
    if a.program!=b.program {return TcStatus.program;}
    if a.config!=b.config {return TcStatus.config;}
    if a.seed!=b.seed {return TcStatus.seed;}
    if a.floats!=b.floats || a.inputs!=b.inputs {return TcStatus.shape;}
    if a.steps!=b.steps {return TcStatus.steps;}
    return TcStatus.ok;
}
// Two-pass and Welford agree to a part in a billion of the mean.
tc_agree :: (x:CmStats,y:CmStats)->bool {
    return abs(x.mean-y.mean)<=1e-9*x.mean && abs(x.sd-y.sd)<=1e-9*x.mean;
}
// Steps n at which b's extra one-off cost is paid back: the least n >= 1
// with n * mean_a >= extra + n * mean_b. Never (-1) unless b's step is faster.
tc_break_even :: (extra:int,mean_a:f64,mean_b:f64)->int {
    if mean_b>=mean_a {return -1;}
    if extra<=0 {return 1;}
    q:=f64(extra)/(mean_a-mean_b);n:=cast(int)q;
    if f64(n)<q {n+=1;}
    return n;
}
tc_report :: (a:TcSide,ta:[]int,b:TcSide,tb:[]int)->TcReport {
    r:=TcReport.{status=tc_same(a.identity,b.identity),break_even=-1};
    if r.status!=TcStatus.ok {return r;}
    r.a=cm_two_pass(ta);r.b=cm_two_pass(tb);r.wa=cm_welford(ta);r.wb=cm_welford(tb);
    if r.a.status!=CmStatus.ok || r.b.status!=CmStatus.ok {r.status=TcStatus.few;return r;}
    r.ratio=r.a.mean/r.b.mean;r.se=sqrt(r.a.sd*r.a.sd/f64(r.a.n)+r.b.sd*r.b.sd/f64(r.b.n));
    r.memory=f64(b.footprint.bytes)/f64(a.footprint.bytes);r.launches=b.footprint.launches-a.footprint.launches;
    // Chapter 34's rule: noisy when a side's sd is over a tenth of its mean,
    // otherwise a winner only beyond two standard errors.
    r.verdict=TcVerdict.same;
    if r.a.sd*10>r.a.mean || r.b.sd*10>r.b.mean {r.verdict=TcVerdict.noisy;return r;}
    if r.b.mean<r.a.mean-2*r.se {r.verdict=TcVerdict.faster;r.break_even=tc_break_even(b.compile-a.compile,r.a.mean,r.b.mean);}
    if r.b.mean>r.a.mean+2*r.se {r.verdict=TcVerdict.slower;}
    return r;
}
```

Work one comparison by hand. Five warm eager steps take 998,000 to
1,002,000 nanoseconds in steps of 1,000; five compiled steps take 698,000
to 702,000. The compile costs 2,400,000 nanoseconds, once.

- Means: 1,000,000 and 700,000 ns.
- Each side's deviations from its mean are −2,000, −1,000, 0, 1,000 and
  2,000. Their squares sum to 10,000,000, so the sample variance is
  10,000,000 / 4 = 2,500,000 and the standard deviation is
  √2,500,000 ≈ 1,581.139 ns.
- Neither side is noisy: 1,581 × 10 is far below either mean.
- Standard error of the difference: √(2,500,000 / 5 + 2,500,000 / 5) =
  √1,000,000 = 1,000 ns. The difference, 300,000 ns, is far beyond two
  standard errors, so compiled is faster: 1,000,000 / 700,000 ≈ 1.429
  times eager's speed.
- Break-even: after n steps eager has spent 1,000,000 n ns and compiled
  2,400,000 + 700,000 n. They are equal when 300,000 n = 2,400,000, at
  n = 8. From the ninth step on, compiling is ahead.

So compiling wins when its step is faster by more than the noise and the
run is longer than the break-even count. It does not win when:

- **The run is short.** A 5-step run of this model would spend 2,400,000
  ns compiling to save 1,500,000 ns.
- **The step is not faster.** If a compiled step took 1,100,000 ns, it
  would be slower, and the compile would never pay back (break-even −1).
  This happens when the step's time is all in arithmetic both paths share,
  so the interpreter's overhead is a small part.
- **Memory is the limit, not time.** Recomputing makes a step slower
  (more launches) to fit a smaller values arena; it wins only when the
  smaller arena is what lets the run fit at all.
- **The program keeps changing.** A shape or a model that changes every
  step would need a compile every step. Bounding those changes is chapter
  39's guarded specialization, which this chapter does not use yet.

## The run

The example trains 6 steps on each path, prints each path's footprint and
the losses, and compares every path's losses, weights and moments with
eager's, bit for bit. Then it compares the fixed step times worked above,
so the transcript is exact, and shows the refusals.

<!-- source: psionic/examples/eager-and-compiled.coax example -->
```jai
// Chapter 131: chapter 117's block trained for 6 steps under chapter 118's
// trainer (AdamW), eager, compiled and recomputing; then the comparison of
// a fixed set of step times (the bench measures real ones); then refusals.
STEPS :: 6;
main :: () {
    m:=mdl_test_block(128);c:=tc_config(STEPS,0.01);
    print("chapter 117's block: % trained floats, % input floats, AdamW at rate %\n",tm_module_floats(m),m.top,c.rate);
    losses:[3][8]f64;p:[3][84]f64;pm:[3][84]f64;pv:[3][84]f64;fp:[3]TcFootprint;
    for k:0..<3 {
        path:=cast(TcPath)k;prog:=tc_program(path,m);inputs:[256]f32;fp[k]=tc_footprint(path,m,prog);
        d:=tc_train(path,m,7,c,prog,inputs[..],p[k][..],pm[k][..],pv[k][..],losses[k][..]);
        f:=fp[k];print("  %: %, % steps; values % floats, gradients % doubles, flags % bytes = % bytes; % launches a step\n",
            tc_path_name(path),d.status,d.steps,f.values,f.grads,f.flags,f.bytes,f.launches);
    }
    for s:0..<STEPS {print("  step %: loss %\n",s,r6(losses[0][s]));}
    for k:1..<3 {
        print("  % against eager, bit for bit: losses %, weights %, first moments %, second moments %\n",tc_path_name(cast(TcPath)k),
            tr_same(losses[k][0..<STEPS],losses[0][0..<STEPS]),tr_same(p[k][..],p[0][..]),tr_same(pm[k][..],pm[0][..]),tr_same(pv[k][..],pv[0][..]));
    }

    // Fixed step times in nanoseconds, so this transcript is exact.
    id:=tc_identity(m,7,c);
    eager:=[5]int.[998000,999000,1000000,1001000,1002000];compiled:=[5]int.[698000,699000,700000,701000,702000];
    a:=TcSide.{identity=id,footprint=fp[0]};b:=TcSide.{identity=id,footprint=fp[1],compile=2400000};
    r:=tc_report(a,eager[..],b,compiled[..]);
    print("compiled against eager, 5 warm steps a side: %\n",tc_status_name(r.status));
    print("  eager: mean % ns, sd % ns; compiled: mean % ns, sd % ns\n",r.a.mean,r3(r.a.sd),r.b.mean,r3(r.b.sd));
    print("  verdict: %, % times eager's speed, standard error % ns\n",tc_verdict_name(r.verdict),r3(r.ratio),r3(r.se));
    print("  memory: % of eager's bytes, % launches more; a 2400000 ns compile pays back after % steps\n",r3(r.memory),r.launches,r.break_even);
    slow:=[5]int.[1098000,1099000,1100000,1101000,1102000];s:=tc_report(a,eager[..],b,slow[..]);
    print("  a compiled step of 1100000 ns: %, break-even %\n",tc_verdict_name(s.verdict),s.break_even);

    // Refusals: each side trained something else.
    print("refusals:\n");
    other:=b;other.identity=tc_identity(m,8,c);print("  seed 8 against seed 7: %\n",tc_status_name(tc_report(a,eager[..],other,compiled[..]).status));
    other.identity=tc_identity(mdl_test_block(64),7,c);print("  dropout 64 against dropout 128: %\n",tc_status_name(tc_report(a,eager[..],other,compiled[..]).status));
    other.identity=tc_identity(mdl_freeze(m,"norm",true),7,c);print("  norm frozen against norm trained: %\n",tc_status_name(tc_report(a,eager[..],other,compiled[..]).status));
    other.identity=tc_identity(m,7,tc_config(STEPS,0.02));print("  rate 0.02 against 0.01: %\n",tc_status_name(tc_report(a,eager[..],other,compiled[..]).status));
    other.identity=tc_identity(m,7,tc_config(STEPS+1,0.01));print("  7 steps against 6: %\n",tc_status_name(tc_report(a,eager[..],other,compiled[..]).status));
    print("  4 timed steps: %\n",tc_status_name(tc_report(a,eager[0..<4],b,compiled[..]).status));
}
```

```text
chapter 117's block: 84 trained floats, 153 input floats, AdamW at rate 0.01
  eager: done, 6 steps; values 144 floats, gradients 148 doubles, flags 32 bytes = 1792 bytes; 30 launches a step
  compiled: done, 6 steps; values 144 floats, gradients 140 doubles, flags 32 bytes = 1728 bytes; 30 launches a step
  recomputing: done, 6 steps; values 113 floats, gradients 140 doubles, flags 32 bytes = 1604 bytes; 34 launches a step
  step 0: loss 9.820698
  step 1: loss 9.182202
  step 2: loss 9.313958
  step 3: loss 8.655045
  step 4: loss 7.852382
  step 5: loss 8.439621
  compiled against eager, bit for bit: losses true, weights true, first moments true, second moments true
  recomputing against eager, bit for bit: losses true, weights true, first moments true, second moments true
compiled against eager, 5 warm steps a side: ok
  eager: mean 1000000.0 ns, sd 1581.139 ns; compiled: mean 700000.0 ns, sd 1581.139 ns
  verdict: faster, 1.429 times eager's speed, standard error 1000.0 ns
  memory: 0.964 of eager's bytes, 0 launches more; a 2400000 ns compile pays back after 8 steps
  a compiled step of 1100000 ns: slower, break-even -1
refusals:
  seed 8 against seed 7: the seeds differ
  dropout 64 against dropout 128: the programs differ
  norm frozen against norm trained: the programs differ
  rate 0.02 against 0.01: the configurations differ
  7 steps against 6: the step counts differ
  4 timed steps: fewer than 5 positive warm steps a side (or more than 1024)
```

Every comparison of two runs that trained different things is refused
before any statistic is computed: a different initialization seed, a
different dropout rate (the program is the same, but its mask entry is
not), a frozen layer (a different gradient mode), a different learning
rate, or a different number of steps. Fewer than five timed steps on a
side is refused too, by chapter 34's admission.

## Measured on this machine

`psionic/tests/check_trainer_compiled.sh` runs the checks and the example in
debug and release, then builds the bench in release with this host's C
compiler and runs it. The bench gives each path five timed compiles and
then 10 untimed and 400 timed trainer steps, each from the micro-batch to the
trainer's committed update, read on the monotonic clock. It prints the
receipt as JSON and exits nonzero unless the three paths agree bit for bit
and both comparisons are admitted, with two-pass and Welford statistics
agreeing on every side. Times are a receipt, never a transcript. One run on 2026-11-01, under other load on the machine:

```text
{
  "schema": "psionic.train-compare.v1",
  "model": "chapter 117 block, dropout 128/256, seed 7, 84 trained floats",
  "trainer": "AdamW rate 0.01, betas 0.9/0.999, eps 1e-8",
  "warm_steps": 10, "timed_steps": 400,
  "bits_identical": true,
  "paths": {
    "eager": {"compile_ns": 0, "step_mean_ns": 1339535, "step_sd_ns": 237431, "values_floats": 144, "grads_floats": 148, "flags_bytes": 32, "bytes": 1792, "launches": 30},
    "compiled": {"compile_ns": 282354, "step_mean_ns": 118633, "step_sd_ns": 10836, "values_floats": 144, "grads_floats": 140, "flags_bytes": 32, "bytes": 1728, "launches": 30},
    "recomputing": {"compile_ns": 4155040, "step_mean_ns": 122679, "step_sd_ns": 9236, "values_floats": 113, "grads_floats": 140, "flags_bytes": 32, "bytes": 1604, "launches": 34}
  },
  "compiled_vs_eager": {"status": "ok", "verdict": "noisy", "speedup": 11.291357716709404, "memory": 0.9642857142857143, "break_even_steps": -1},
  "recomputing_vs_eager": {"status": "ok", "verdict": "noisy", "speedup": 10.919007503592601, "memory": 0.8950892857142857, "break_even_steps": -1}
}
```

The three paths agreed bit for bit over all 410 steps, and both comparisons
were admitted: same program, configuration, seed, shape and step count.
But neither names a winner. The machine was running about 60 other
processes at once, and preemption made the eager step's standard deviation
18 percent of its mean, over chapter 34's limit of a tenth. So the verdict
is `noisy` and the report gives no break-even count. That is the rule
working: a mean of 1,339,535 ns against 118,633 ns looks like a factor of
about 11, but a run this disturbed does not support a claim. Four other
runs that day gave the same verdict, with eager means of 1.3 to 2.7 ms and
compiled means of 0.13 to 0.34 ms.

The means still show where the time goes. Most of an eager step is
recording the tape: chapter 106's `ad_tape` plans the arenas and orders the
rules again on every step, and the compiled path does that work once. Had
the verdict been `faster`, the break-even arithmetic from above would give
282,354 / (1,339,535 − 118,633) ≈ 0.23, so one step, for the compiled
path; and 4,155,040 / (1,339,535 − 122,679) ≈ 3.4, so four steps, for the
recomputing path, whose chooser compiles the program many times while it
searches. A measured verdict needs a quiet machine; the bench is the same
command there.

## Checks

The checks in [train-compare.coax](../lib/train-compare.coax) and
[trainer-compiled.coax](../lib/trainer-compiled.coax):

- **The hand calculation above** (in train-compare.coax, with zero
  surviving mutants): means, standard deviations, standard
  error, verdict, bytes ratio and break-even, plus the edges of the
  two-standard-error rule, a noisy side, a slower side and a compile that
  never pays back.
- **Refusals:** each identity field changed alone, on either side; fewer
  than 5 or more than 1,024 steps, or a step that is not positive; the
  tolerance within which two-pass and Welford statistics agree; and, in
  trainer-compiled.coax, identities that change with the seed, a frozen
  layer, the dropout rate, the step count and each configuration field.
- **Footprints:** each path's arenas, workspace and launches, as in the
  table.
- **The independent check:** two AdamW steps on each path against chapter
  118's own loop (`tm_module_drive`, its tape recorded once): losses,
  weights, first and second moments and the whole input buffer, bit for
  bit; another seed gives another loss; a missing program or short buffer
  is refused before a step is taken.
- **The whole step compiled:** chapter 114's program with its update on,
  against the trainer's SGD at chapter 117's f32 rate, bit for bit.

## What remains

- **Bounded dynamic batch sizes.** The block trains one fixed batch.
  Admitting a range of batch sizes through chapter 39's guarded
  specialization, and refusing one outside it rather than recompiling
  without limit, is not here.
- **Fusion.** Each forward command and backward rule is still its own
  launch.
- **The optimizer in the program.** `tc_whole` compiles chapter 114's SGD
  into the step; AdamW, clipping and the schedule still run in the trainer.
- **A compiled GPU path.** Chapters 129 and 130 train and profile on the
  GPU with hand-placed kernels; a compiled GPU program to compare against
  them is not here.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
- [Chapter 34](ch034-benchmark-against-llama-cpp-and-cublas.md): the
  comparison protocol and its statistics
