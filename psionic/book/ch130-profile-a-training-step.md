# 130. Profile a training step

> **Status: partly implemented.** Chapter 129's training step is profiled
> on one CPU thread and on the RTX 4080: every phase timed, every kernel's
> FLOPs and bytes counted exactly, each kernel placed on a measured
> binary64 roofline, and the bottleneck named with its share of the step
> and its gap to the roof, all in a JSON receipt. The fix the profile
> points to (splitting one row's work across threads) and its
> before-and-after measurement, launch overhead apart from kernel time,
> and per-buffer allocation lifetimes remain.

You cannot speed up a training step until you know where its time goes.
[Chapter 129](ch129-train-a-whole-model-on-one-gpu.md) trains chapter
115's decoder on the GPU, and its own receipt says the device step takes
about 33 ms against 1.3 ms on the CPU. This chapter takes that step apart:
it times each phase, counts the work each kernel does, compares each time
with the least time the hardware allows, and names the phase to fix.

## The step, phase by phase

Chapter 129's step is nine phases, in launch order. The trainer names the
batch's rows before the step's clock starts.

| Phase | Kind | What runs |
| --- | --- | --- |
| upload | transfer | the step's rows, host to device |
| rows | kernel | `gk_rows`: each row's forward and backward pass |
| reduce | kernel | `gk_mean` then `gk_squares` |
| losses | transfer | the row losses and the squared norm, device to host |
| decide | host | `gt_decide` |
| optimizer | kernel | `gk_clip` (only past the clip), `gk_adamw`, `gk_admit` |
| flags | transfer | the two admission flags, device to host |
| admit | host | `gt_commit` |
| commit | kernel | `gk_commit` |

A tenth series, **forward**, is timed apart from the step: after each
step the harness launches `tp_forward`, the forward half of `gk_rows`, on
the same rows. Backward time is then the rows phase less the forward
series. The kernel writes only the forward values, with `gk_rows`' bits.
Two more kernels measure the roofline of whatever runs them, in binary64,
the format chapter 129 trains in:

<!-- source: psionic/lib/transformer/profile-kernels.coax kernels -->
```jai
// The forward half of gk_rows: chapter 113's td_advance at every position of
// row `item`, into that row's slice of work. Nothing else is written.
tp_forward :: (item:int,sh:[]int,p:[]f64,rows:[]u8,rope:[]f64,work:[]f64) #export_c {
    n:=sh[7];nw:=gk_work(sh);w:=work[item*nw..<item*nw+nw];
    for t:0..<n {gk_advance(sh,p,w,rope,cast(int)rows[item*n+t],t);}
}
// a = b + s * c, one element per item: 2 FLOPs, 16 bytes read, 8 written.
tp_triad :: (item:int,s:f64,a:[]f64,b:[]f64,c:[]f64) #export_c {
    a[item]=b[item]+s*c[item];
}
// Eight independent chains of v = v * m + k, rounds times, in registers:
// 16 FLOPs a round, 16 more to start and fold the chains, 8 bytes read and
// 8 written per item.
tp_madd :: (item:int,rounds:int,m:f64,k:f64,x:[]f64) #export_c {
    v:[8]f64;for c:0..<8 {v[c]=x[item]+f64(c);}
    for r:0..<rounds {for c:0..<8 {v[c]=v[c]*m+k;}}
    s:=0.0;for c:0..<8 {s+=v[c];}x[item]=s;
}
```

On the GPU each kernel phase ends with a device sync, so the clock sees
the kernel's whole run. Each phase is timed with chapter 18's monotonic
clock around its own work only; the step's wall time is a separate pair of
clock reads around all nine. What is left over (argument setup, clock
reads) is the step's **unaccounted** time.

## Counting the work

A FLOP here is one binary `+`, `-`, `*` or `/` on an `f64` (a compound
`+=` is one), or one `sqrt`, `exp` or `log`. Comparisons, casts, a sign
flip and index arithmetic are not counted. Every count follows the
kernel's code statement by statement.

Start with the pieces `gk_rows` calls. `gk_norm` over `n` values squares
and adds each (2n), takes `1/sqrt(s/n + eps)` (4), and scales each value
twice (2n): **4n + 4**. Its backward does 6 operations a value in the
first loop and 9 in the second: **15n**. A matrix-vector product of `r`
rows and `c` columns is **2rc**; its backward accumulates both the weight
gradient and the input gradient, **4rc**. RoPE rotates each pair with 6
operations: **3** a value.

One position of the forward pass (`gk_advance`) at shape `V, d, H, e, f`
(vocabulary, width, heads, head width, hidden) with `q = He` query and
`k` key values spends, per layer: two norms and two residual adds
(10d + 8), the four projections (4qd + 4kd), RoPE (3q + 3k), the FFN
(5fd: 2fd up, 3 per weight down), and attention: for each head and each
earlier position `j`, a dot product and scale (2e + 1), the softmax's
`exp`, sum and divide (4) and the weighted sum (2e), so `4e + 5` per pair.
The final norm, the logits and the scale are 4d + 6 + 2Vd. Summed over a
row of `n` positions, with `T = n(n+1)/2` pairs per head:

<!-- source: psionic/lib/transformer/train-profile.coax flops -->
```jai
// FLOPs of one training row in gk_rows, split into its forward loop and the
// rest. Counted: binary +, -, *, / on f64 (a compound += is one), sqrt, exp
// and log. Not counted: comparisons, casts, a sign flip, index arithmetic.
// T = n(n+1)/2 is the number of (position, earlier position) pairs one
// attention head sees over a row.
tp_forward_flops :: (sh:[]int)->int {
    v:=sh[0];d:=sh[1];h:=sh[2];e:=sh[4];f:=sh[5];n:=sh[7];q:=h*e;k:=sh[3]*e;t:=n*(n+1)/2;
    layer:=n*(10*d+8+4*q*d+4*k*d+3*q+3*k+5*f*d)+h*(4*e+5)*t;
    return n*(4*d+6+2*v*d)+sh[6]*layer;
}
tp_backward_flops :: (sh:[]int)->int {
    v:=sh[0];d:=sh[1];h:=sh[2];e:=sh[4];f:=sh[5];n:=sh[7];q:=h*e;k:=sh[3]*e;t:=n*(n+1)/2;
    layer:=n*(32*d+9*f*d+2*f+8*q*d+8*k*d+3*q+3*k)+h*(8*e+5)*t;
    return 3+n*d+(n-1)*(7*v+6+4*v*d+15*d)+sh[6]*layer;
}
```

By hand, on chapter 113's tiny shape (V 5, d 4, H 2, one KV head, e 2,
f 3, two layers, n 4; so q 4, k 2, T 10):

- Outside the layers: 4 × (4·4 + 6 + 2·5·4) = 4 × 62 = **248**.
- One layer: 4 × (40 + 8 + 64 + 32 + 12 + 6 + 60) = 4 × 222 = 888, plus
  attention 2 × (4·2 + 5) × 10 = 260: **1148**.
- Forward: 248 + 2 × 1148 = **2544**.
- Backward: 3 + 16 + 3 × (35 + 6 + 80 + 60) = 562 outside the layers, and
  per layer 4 × 452 + 2 × 21 × 10 = 2228: 562 + 2 × 2228 = **5018**.

Chapter 115's decoder (V 16, d 16, H 2, one KV head, e 8, f 16, two
layers, n 32) costs 292,352 FLOPs forward and 572,573 backward a row.

Bytes are compulsory traffic: every value that exists before a launch is
read once, and every value it writes is written once. That is the least
any implementation of the same kernel moves. For four rows the rows
kernel reads the shape (64 bytes), the 3152 parameters (25,216), the
tokens (128) and the RoPE table (2048), and writes each row's work and
work-gradient slices (2 × 13,984 values), its 3152 gradients and its loss:
64 + 25,216 + 128 + 2048 + 4 × (223,744 + 25,216 + 8) = **1,023,328**.
AdamW reads the parameter, gradient and two moments and writes three
staged values: 56 bytes and 20 FLOPs a parameter.

<!-- source: psionic/lib/transformer/train-profile.coax work -->
```jai
// Each kernel's work for one launch over micro rows. Bytes are compulsory
// traffic: every value that exists before the launch is read once and every
// value it writes is written once (a row's whole work slice counts as
// written). Parameters, moments and gradients are 8-byte values; tokens are
// bytes; the shape view is 8 integers.
tp_rows :: (sh:[]int,micro:int)->TpWork {
    np:=gk_parameters(sh);nw:=gk_work(sh);n:=sh[7];
    read:=64+8*np+micro*n+8*n*sh[4];write:=micro*(16*nw+8*np+8);
    return TpWork.{flops=micro*(tp_forward_flops(sh)+tp_backward_flops(sh)),bytes=read+write};
}
tp_forward_work :: (sh:[]int,micro:int)->TpWork {
    n:=sh[7];return TpWork.{flops=micro*tp_forward_flops(sh),bytes=64+8*gk_parameters(sh)+micro*n+8*n*sh[4]+micro*8*gk_work(sh)};
}
// gk_mean then gk_squares.
tp_reduce :: (np:int,micro:int)->TpWork {return TpWork.{flops=2*micro*np+2*np,bytes=8*micro*np+8*np+8*np+8};}
tp_clip :: (np:int)->TpWork {return TpWork.{flops=2*np,bytes=16*np};}
// gk_adamw (20 FLOPs a parameter) then gk_admit (comparisons only).
tp_optimizer :: (np:int)->TpWork {return TpWork.{flops=20*np,bytes=56*np+24*np+16};}
tp_commit :: (np:int)->TpWork {return TpWork.{bytes=48*np};}
// The work of phase p over a whole profile: runs steps, clips of them clipped.
tp_work :: (sh:[]int,micro:int,runs:int,clips:int,p:TpPhase)->TpWork {
    np:=gk_parameters(sh);w:=TpWork.{};
    if p==TpPhase.upload {w.bytes=micro*sh[7];}
    if p==TpPhase.rows {w=tp_rows(sh,micro);}
    if p==TpPhase.reduce {w=tp_reduce(np,micro);}
    if p==TpPhase.losses {w.bytes=8*micro+8;}
    if p==TpPhase.optimizer {w=tp_optimizer(np);}
    if p==TpPhase.flags {w.bytes=24;}
    if p==TpPhase.commit {w=tp_commit(np);}
    if p==TpPhase.forward {w=tp_forward_work(sh,micro);}
    w.flops*=runs;w.bytes*=runs;
    if p==TpPhase.optimizer {c:=tp_clip(np);w.flops+=clips*c.flops;w.bytes+=clips*c.bytes;}
    return w;
}
```

## The least time

A kernel phase cannot finish faster than its FLOPs at the arithmetic roof
or its bytes at the bandwidth roof, whichever takes longer. With rates in
chapter 33's units (MFLOP/s, MB/s), one MFLOP/s is one FLOP a microsecond,
so the time in nanoseconds is `flops × 1000 / rate`:

<!-- source: psionic/lib/transformer/train-profile.coax least -->
```jai
// The least time work can take on a roofline: its FLOPs at the arithmetic
// roof or its bytes at the bandwidth roof, whichever is longer, in
// nanoseconds (a rate of 1 MFLOP/s is one FLOP a microsecond).
TpLeast :: struct {ns:int;bound:RfBound;}
tp_least :: (h:RfHost,w:TpWork)->TpLeast {
    if h.mflops<=0 || h.mbytes<=0 {return TpLeast.{};}
    compute:=w.flops*1000/h.mflops;memory:=w.bytes*1000/h.mbytes;
    if compute>=memory && w.flops>0 {return TpLeast.{ns=compute,bound=RfBound.compute};}
    return TpLeast.{ns=memory,bound=RfBound.memory};
}
```

Wherever both roofs give whole nanoseconds this is chapter 33's
`rf_place`, and the checks compare the two. The rows phase for four rows
is 4 × (292,352 + 572,573) = 3,459,700 FLOPs. In one run on this host
the RTX 4080's roof measured 354,433 MFLOP/s and 608,204 MB/s. At that
rate the FLOPs take 3,459,700 × 1000 / 354,433 = **9761 ns**; the
1,023,328 bytes take 1682 ns, so the phase is bound by arithmetic. The
same run measured the rows phase at 30.93 ms: **3169 times** its least
time.

## The report

`tp_report` takes the shape, the rows per step, how many steps clipped,
the roof, and the ten series of per-step times. It refuses before it
reports: a shape or series that does not fit, a roof with no positive
rates, fewer than 5 or more than 1024 steps, or a time that is not
positive (chapter 34's `cm_two_pass` makes these last three calls). Then
it checks that **every step's nine phases fit inside that step and leave
at most 5 percent of it unaccounted**. A phase timer that overlaps the
next phase, or a step clock that started early, is refused rather than
reported.

<!-- source: psionic/lib/transformer/train-profile.coax report -->
```jai
// ns holds the ten series one after another, runs values each; step holds
// each measured step's wall time. clips of the runs launched gk_clip.
tp_report :: (sh:[]int,micro:int,clips:int,h:RfHost,ns:[]int,step:[]int)->TpReport #export_c {
    runs:=step.count;r:=TpReport.{status=TpStatus.shape,at=-1,runs=runs};
    if sh.count!=8 || micro<1 || ns.count!=TP_PHASES*runs || clips<0 || clips>runs {return r;}
    for x:sh {if x<1 {return r;}}
    if h.mflops<=0 || h.mbytes<=0 {r.status=TpStatus.host;return r;}
    r.step=cm_two_pass(step);
    if r.step.status!=CmStatus.ok {r.status=tp_refused(r.step.status);return r;}
    for p:0..<TP_PHASES {
        s:=cm_two_pass(ns[p*runs..<p*runs+runs]);
        if s.status!=CmStatus.ok {r.status=tp_refused(s.status);r.at=p;return r;}
        r.lines[p].stats=s;
    }
    // Every step: its phases fit inside it, leaving at most TP_SLACK thousandths.
    for k:0..<runs {
        left:=step[k];for p:0..<TP_STEP_PHASES {left-=ns[p*runs+k];}
        if left<0 || left*1000>step[k]*TP_SLACK {r.status=TpStatus.sum;r.at=k;return r;}
        if left>r.slack {r.slack=left;r.worst=k;}
    }
    r.status=TpStatus.ok;r.unaccounted=r.step.mean;r.idle=r.step.mean;
    for p:0..<TP_PHASES {
        ph:=cast(TpPhase)p;line:=r.lines[p];w:=tp_work(sh,micro,runs,clips,ph);
        line.flops=f64(w.flops)/f64(runs);line.bytes=f64(w.bytes)/f64(runs);line.share=line.stats.mean/r.step.mean;
        if tp_kind(ph)==TpKind.kernel {least:=tp_least(h,w);line.least=f64(least.ns)/f64(runs);line.bound=least.bound;line.gap=line.stats.mean/line.least;}
        r.lines[p]=line;
        if p<TP_STEP_PHASES {
            r.unaccounted-=line.stats.mean;
            if tp_kind(ph)==TpKind.kernel {r.idle-=line.stats.mean;}
            if line.stats.mean>r.lines[cast(int)r.bottleneck].stats.mean {r.bottleneck=ph;}
        }
    }
    r.launches=6+f64(clips)/f64(runs);
    b:=r.lines[cast(int)r.bottleneck];r.share=b.share;r.gap=b.gap;
    r.backward=r.lines[1].stats.mean-r.lines[9].stats.mean;
    return r;
}
```

Each line carries chapter 34's mean and sample standard deviation, the
phase's work per step, its least time, its share of the mean step, and its
gap (mean over least time; transfers and host work have no roof here).
The bottleneck is the step phase with the largest mean; the forward
series, timed apart, never is. **Idle** is the mean step less the kernel
phases: time the device runs nothing.

## Run it

The example, [`psionic/examples/train-profile.coax`](../examples/train-profile.coax),
prints the work of a four-row step on chapter 115's decoder, then reports
five synthetic steps built around the phase means and roof of the run
measured above. The times are fixed numbers, so the transcript is exact,
and the bottleneck is known before the report names it.

<!-- source: psionic/examples/train-profile.coax example -->
```jai
// Chapter 130: the exact work of chapter 129's step on chapter 115's decoder
// with four rows, then a profile of five steps built around the GPU
// receipt's phase means, placed on the RTX 4080's measured binary64 roof.
// The times here are fixed numbers so the transcript is exact; the device
// harness (psionic/tests/check_train_profile.sh) measures real ones.
// x as a percentage with two decimals.
percent :: (x:f64) {h:=cast(int)(x*10000+0.5);print("%.",h/100);if h%100<10 {print("0");}print("% percent",h%100);}
main :: () {
    sh:=[8]int.[16,16,2,1,8,16,2,32];
    print("one row of chapter 115's decoder: % FLOPs forward, % backward\n",tp_forward_flops(sh[..]),tp_backward_flops(sh[..]));
    print("a step of 4 rows, per phase: FLOPs, bytes, kind\n");
    for p:0..<TP_PHASES {ph:=cast(TpPhase)p;w:=tp_work(sh[..],4,1,0,ph);print("  %: %, %, %\n",tp_phase_name(ph),w.flops,w.bytes,tp_kind_name(tp_kind(ph)));}
    c:=tp_clip(gk_parameters(sh[..]));print("  clip, when launched: %, %\n",c.flops,c.bytes);

    gpu:=RfHost.{mflops=354433,mbytes=608204};
    base:=[10]int.[12000,30932000,212000,20000,800,873000,7000,200,7500,10883000];
    wobble:=[10]int.[2000,127000,2500,1600,250,950,1700,40,1200,223000];
    ns:[50]int;step:[5]int;tq_profile(base[..],wobble[..],300,ns[..],step[..]);
    r:=tp_report(sh[..],4,0,gpu,ns[..],step[..]);
    print("five steps on a roof of % MFLOP/s and % MB/s: %\n",gpu.mflops,gpu.mbytes,tp_status_name(r.status));
    print("  step: mean % ns, sd % ns\n",cast(int)r.step.mean,cast(int)r.step.sd);
    for p:0..<TP_PHASES {l:=r.lines[p];ph:=cast(TpPhase)p;
        print("  %: mean % ns, ",tp_phase_name(ph),cast(int)l.stats.mean);percent(l.share);
        if l.least>0 {print(", least % ns (%-bound), % times that",cast(int)l.least,rf_bound_name(l.bound),cast(int)(l.gap+0.5));}
        print("\n");}
    print("  the phases leave % ns of the mean step unaccounted, at most % ns in one step\n",cast(int)r.unaccounted,r.slack);
    print("  no kernel runs for % ns a step; % launches a step; backward is rows less forward: % ns\n",cast(int)r.idle,cast(int)r.launches,cast(int)r.backward);
    print("bottleneck: %, ",tp_phase_name(r.bottleneck));percent(r.share);print(" of the step, % times its least time\n",cast(int)(r.gap+0.5));
}
```

```shell
coax run psionic/examples/train-profile.coax
```

```text
one row of chapter 115's decoder: 292352 FLOPs forward, 572573 backward
a step of 4 rows, per phase: FLOPs, bytes, kind
  upload: 0, 128, transfer
  rows: 3459700, 1023328, kernel
  reduce: 31520, 151304, kernel
  losses: 0, 40, transfer
  decide: 0, 0, host
  optimizer: 63040, 252176, kernel
  flags: 0, 24, transfer
  admit: 0, 0, host
  commit: 0, 151296, kernel
  forward: 1169408, 474944, kernel
  clip, when launched: 6304, 50432
five steps on a roof of 354433 MFLOP/s and 608204 MB/s: ok
  step: mean 32064800 ns, sd 216995 ns
  upload: mean 12000 ns, 0.04 percent
  rows: mean 30932000 ns, 96.47 percent, least 9761 ns (compute-bound), 3169 times that
  reduce: mean 212000 ns, 0.66 percent, least 248 ns (memory-bound), 853 times that
  losses: mean 20000 ns, 0.06 percent
  decide: mean 800 ns, 0.00 percent
  optimizer: mean 873000 ns, 2.72 percent, least 414 ns (memory-bound), 2106 times that
  flags: mean 7000 ns, 0.02 percent
  admit: mean 200 ns, 0.00 percent
  commit: mean 7500 ns, 0.02 percent, least 248 ns (memory-bound), 30 times that
  forward: mean 10883000 ns, 33.94 percent, least 3299 ns (compute-bound), 3299 times that
  the phases leave 300 ns of the mean step unaccounted, at most 300 ns in one step
  no kernel runs for 40300 ns a step; 6 launches a step; backward is rows less forward: 20049000 ns
bottleneck: rows, 96.47 percent of the step, 3169 times its least time
```

## What it refuses

The second example
([`psionic/examples/train-profile-refuse.coax`](../examples/train-profile-refuse.coax))
breaks the same profile one way at a time: a rows timer that ran into the
next phase, a step clock that started 2 ms early, too few steps, a phase
the clock saw as 0 ns, a roof that was never measured, and more clipped
steps than steps.

<!-- source: psionic/examples/train-profile-refuse.coax refuse -->
```jai
// Chapter 130: profiles the report refuses instead of naming a bottleneck.
main :: () {
    sh:=[8]int.[16,16,2,1,8,16,2,32];gpu:=RfHost.{mflops=354433,mbytes=608204};
    base:=[10]int.[12000,30932000,212000,20000,800,873000,7000,200,7500,10883000];
    wobble:=[10]int.[2000,127000,2500,1600,250,950,1700,40,1200,223000];
    ns:[50]int;step:[5]int;tq_profile(base[..],wobble[..],300,ns[..],step[..]);
    print("the profile as measured: %\n",tp_status_name(tp_report(sh[..],4,0,gpu,ns[..],step[..]).status));

    // A rows timer that also caught the next phase: in step 2 the phases
    // add up to more than the step.
    x:=ns;x[1*5+2]+=212300;r:=tp_report(sh[..],4,0,gpu,x[..],step[..]);
    print("rows timed past its end in step %: %\n",r.at,tp_status_name(r.status));
    // A step whose clock started 2 ms early: 6 percent of it is unaccounted.
    s:=step;s[4]+=2000000;r=tp_report(sh[..],4,0,gpu,ns[..],s[..]);
    print("step % started 2 ms early: %\n",r.at,tp_status_name(r.status));
    // Four steps, and a host phase the clock saw as 0 ns.
    r=tp_report(sh[..],4,0,gpu,ns[0..<40],step[0..<4]);print("four steps: %\n",tp_status_name(r.status));
    x=ns;x[4*5+3]=0;r=tp_report(sh[..],4,0,gpu,x[..],step[..]);print("% took 0 ns in one step: %\n",tp_phase_name(cast(TpPhase)r.at),tp_status_name(r.status));
    // A roof that was never measured, and a clip count above the step count.
    r=tp_report(sh[..],4,0,RfHost.{},ns[..],step[..]);print("no roofline: %\n",tp_status_name(r.status));
    r=tp_report(sh[..],4,6,gpu,ns[..],step[..]);print("6 clipped of 5 steps: %\n",tp_status_name(r.status));
}
```

```shell
coax run psionic/examples/train-profile-refuse.coax
```

```text
the profile as measured: ok
rows timed past its end in step 2: refused: the phases do not account for the step
step 4 started 2 ms early: refused: the phases do not account for the step
four steps: refused: fewer than 5 measured steps
decide took 0 ns in one step: refused: a time that is not positive
no roofline: refused: the roofline has no positive rates
6 clipped of 5 steps: refused: the shape, micro-batch, clip count or series lengths do not fit
```

Both examples exit with status 0; the refusals are values the report
returns, and the transcripts are compared in debug and release.

## On the hardware

`psionic/tests/check_train_profile.sh` runs the checks and the examples,
then builds `psionic/tests/train_profile_device.c` against the Coax above
compiled to C. That file only allocates, copies, launches and reads the
clock. For each target, one CPU thread and then the GPU, it measures the
roof with `tp_triad` and `tp_madd` (best of seven runs after one warm-up),
runs 3 unprofiled steps, then 20 profiled steps from chapter 129's start,
and compares the final parameters with chapter 118's CPU run within
chapter 129's frozen tolerance, so the profiling cannot have changed the
numbers. It writes a receipt; without a device the GPU half prints
`SKIP`.

```shell
COAX_CC=gcc psionic/tests/check_train_profile.sh 20 4 RECEIPT.json
```

The receipts are in `psionic/fixtures/train-profile/`: four rows a step
(`coderos-4080-4rows.json`) and 64 (`coderos-4080-64rows.json`). Each
records the host, date, compiler, Coax build, NVRTC compile time, the load
average before and after, both roofs and every phase. They are
measurements, not transcripts: this host is shared and its load average
was between 30 and 50 during these runs, so the CPU numbers in particular
move from run to run.

| Receipt | Target | Roof (MFLOP/s, MB/s) | Step, mean (sd) | Rows: share, gap to roof | Forward / rows | Optimizer | Most unaccounted in one step | Load average before |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 4 rows | CPU, one thread | 3,862, 3,044 | 1.67 ms (0.07) | 94.2%, 1.8× | 0.41 | 0.051 ms | 620 ns | 40.36 |
| 4 rows | RTX 4080 | 349,724, 609,377 | 34.53 ms (3.76) | 93.6%, 3266.8× | 0.39 | 1.450 ms | 1256 ns | 40.36 |
| 64 rows | CPU, one thread | 5,188, 7,258 | 26.29 ms (6.82) | 97.6%, 2.4× | 0.43 | 0.042 ms | 1585 ns | 38.28 |
| 64 rows | RTX 4080 | 354,348, 611,221 | 55.22 ms (1.22) | 97.7%, 345.5× | 0.31 | 0.877 ms | 2662 ns | 38.28 |

What the profile shows:

- **The rows kernel is the bottleneck on both targets**, with 93–98
  percent of the step. Nothing else is worth fixing first.
- **On the CPU the step is near its roof.** The rows phase runs within
  about 2 times its least time on one thread's measured roof.
- **On the GPU it is thousands of times off its roof.** With four rows
  the kernel runs four threads on a device built for tens of thousands;
  its gap to the roof is over 3000. With 64 rows the gap falls to about
  345, and the step takes 55 ms for 16 times the work.
- **Backward costs about twice forward**, as the FLOP counts predict
  (572,573 against 292,352 a row, so forward is 0.34 of the row): the
  forward series is 0.31 to 0.43 of the rows phase.
- **The optimizer on the GPU is one thread.** `gk_admit` scans all 9456
  staged values on a single work item, about 0.9 ms a step whatever the
  row count (the four-row receipt's 1.45 ms has a standard deviation of
  1.17 ms: a noisy run). `gk_squares` does the same for the norm in the
  reduce phase. Together they are 2 to 6 percent of the step: the next fix
  after the rows kernel.
- **Transfers and host decisions are small.** The three transfers and two
  host phases together take under 0.1 ms of a 35 ms GPU step, and no
  kernel runs for about the same time (`kernel_idle_ns`).
- **A gap below 1 means the roof was measured low.** In the four-row CPU
  receipt the reduce, optimizer and commit phases ran faster than their
  least time. The triad that sets the CPU's bandwidth roof streams about
  100 MB through main memory, and under a load average of 40 it reached
  only 3 GB/s; those phases touch under 0.3 MB, which stays in cache. The
  roof is a measurement on a shared host, and the receipt records the load
  that went with it.
- **Compile time is kept apart.** NVRTC compiles the kernels once before
  the first step (4.7 s cold in this chapter's first run, 0.05–0.11 s in
  the recorded ones with the driver's cache warm); it is in the receipt,
  never in a step.

## How it is checked

- `psionic/lib/transformer/train-profile.coax`: the closed-form FLOP
  counts against a statement-by-statement walk of `gk_rows`' loops
  (`psionic/tests/train-profile-checks.coax`) on five shapes, and by hand
  on the tiny shape; each kernel's FLOPs and bytes by hand at chapter
  115's shape; the least time against `rf_place` and at every edge;
  synthetic profiles whose bottleneck is known (rows, a transfer, the
  optimizer, ties, the last phase), with every line's statistics equal to
  `cm_two_pass` and within twelve digits of `cm_welford`; the 5 percent
  bound at its exact edge and one nanosecond past it, in both directions;
  every refusal. Mutation testing kills all 368 mutants.
- `psionic/lib/transformer/profile-kernels.coax`: the forward kernel
  writes exactly `gk_rows`' forward values and nothing outside its row;
  the roof kernels by hand. Mutation testing kills all 26 mutants.
- The examples' transcripts, compared exactly in debug and release.
- On the hardware: the harness above, which fails if a profile is refused
  or a profiled run leaves chapter 129's tolerance.

## What is not here yet

- **The fix and its measurement.** The profile names the rows kernel: one
  thread computes a whole row. Splitting a row's matrix products,
  attention and norms across the lanes of a warp (chapter 37's warp
  kernels) and measuring the step again over at least five steps, with
  results unchanged within chapter 129's tolerance, is the next step.
  Spreading `gk_admit` and `gk_squares` over many work items comes after.
- **Launch overhead apart from kernel time.** Each kernel phase here is
  timed on the host clock around launch and sync; device events would
  split the launch's own cost from the kernel's run.
- **Allocation lifetimes.** Every buffer lives for the whole run, so peak
  device memory is chapter 129's plan (1,174,648 bytes for four rows,
  16,114,648 for 64); per-buffer lifetimes within a step are not traced.

## Public API

- `psionic/lib/transformer/train-profile.coax`: `tp_report` (C entry
  point), `tp_forward_flops`, `tp_backward_flops`, `tp_rows`,
  `tp_forward_work`, `tp_reduce`, `tp_clip`, `tp_optimizer`, `tp_commit`,
  `tp_work`, `tp_kind`, `tp_least`, `TpReport`, `TpLine`, `TpPhase`,
  `TpStatus`, `TP_SLACK`.
- `psionic/lib/transformer/profile-kernels.coax`: `tp_forward`,
  `tp_triad`, `tp_madd` (kernels).
- Receipt schema `psionic.train-profile.v1`, written by
  `psionic/tests/train_profile_device.c`.

## Dependencies

- [Chapter 129](ch129-train-a-whole-model-on-one-gpu.md): the GPU training
  step to profile.
- [Chapter 18](ch018-time-a-decision.md): monotonic phase timing.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): exact work
  accounting and the roofline.
- [Chapter 34](ch034-benchmark-against-llama-cpp-and-cublas.md): the
  statistics of repeated warm runs.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
