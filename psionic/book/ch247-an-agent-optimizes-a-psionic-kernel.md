# 247. An agent optimizes a Psionic kernel

[Chapter 37](ch037-a-coax-compute-subset.md) gave Coax a small language for
GPU code, so that the arithmetic of every kernel stays in Coax. Writing fast
kernels in it is slow work, and the obvious idea is to hand that work to an
agent. The risk is just as obvious: an agent that is rewarded for speed finds
whatever the benchmark fails to check. This chapter runs the experiment with
the checks fixed first:
- **One kernel, one workload, one contract.** The kernel is chapter 44's
  offset RMS norm on Elephant's 86 × 4,096 prefill, on an NVIDIA L4. Its
  numerical behaviour is declared before the search starts.
- **A conventional tuner goes first.** It sweeps a family of kernels with
  knobs for tile size, layout, vector width and one fusion choice, across six
  block sizes.
- **Then an agent proposes new kernels.** It gets the operation, the
  contract, the tuner's best result, compilation, correctness results and
  per-pass timings, and a budget of eight evaluations.
- **The evaluator is outside the agent's reach.** The cases, the contract,
  the host and the ledger are files the agent may not write. Nine of the ten
  cases are held out from it.
- **Everything is kept.** Every evaluation, failures included, is written to
  a ledger with its source digest, and the book example draws its verdicts
  from that ledger.

```shell
coax/bin/coax run psionic/examples/kernel-search.coax
coax/bin/coax test psionic/lib/kernel-search.coax
coax/bin/coax test psionic/lib/device/norm-search.coax
python3 -I psionic/tests/kernel_search_oracle.py
psionic/tests/kernel_search_eval.sh psionic/kernels/norm/agent/a8.coax a8 cpu
```

```jai
workload: Elephant's offset RMS norm over chapter 14's 86-token sample, 86 x 4096, epsilon 1e-6
contract: squares added in f64 in any order, the rest in f64, one rounding to f32;
  each output must be one of the two f32 values next to the f64 norm
held out from the agent: a decode row, Mammoth's width 5120, width 1000, width 4097, width 1, width 17, extreme values, zero rows, one huge value among tiny ones

on this CPU, now, every case:
  chapter 44 (n30): 6 of 10 cases; wrong on width 1000, width 4097, width 1, width 17
  the tuner's choice (nt 3240): 10 of 10 cases
  the agent's winner (a8 64160): 10 of 10 cases

on the L4 (NVIDIA L4 compute 8.9, 58 SMs, driver 13020), from the ledger of 15 evaluations:
  chapter 44, evaluation 15, on its declared widths: best: knob 0, block 32: 124.74 us (124.7 to 124.81 over 11 trials); pass 0 32.19 us, pass 1 92.57 us at block 32
  the tuner, evaluation 14: 49 knobs x 6 blocks = 294 configurations, 49 knobs passing every case on both
    best: knob 3240, block 32: 72.29 us (72.25 to 72.34 over 11 trials) (32 contiguous chunks, 4 outputs an item), 1.73 x chapter 44; pass 0 18.0 us, pass 1 54.48 us at block 32
    slowest: knob 0, block 128: 2632.99 us (2630.12 to 2635.86 over 11 trials) (one item a row, both passes fused)
  the agent: 8 budgeted evaluations, failures included
    a1, evaluation 3: 9 knobs, stopped at block 1024: CUDA error 701 (too many resources requested for launch)
    a2, evaluation 4: 9 knobs, 9 passing; best: knob 128128, block 64: 39.12 us (39.06 to 39.18 over 11 trials), faster than the tuner
    a3, evaluation 5: 11 knobs, 11 passing; best: knob 1281281, block 32: 43.56 us (43.52 to 43.64 over 11 trials), faster than the tuner
    a4, evaluation 6: 8 knobs, 8 passing; best: knob 64128, block 32: 35.55 us (35.49 to 35.61 over 11 trials), faster than the tuner
    a5, evaluation 7: 12 knobs, 12 passing; best: knob 641281, block 32: 27.91 us (27.87 to 27.98 over 11 trials), faster than the tuner
    a6, evaluation 8: 8 knobs, 8 passing; best: knob 64128, block 32: 27.85 us (27.81 to 27.91 over 11 trials), faster than the tuner
    a7, evaluation 9: 10 knobs, 10 passing; best: knob 64160, block 32: 26.36 us (26.3 to 26.38 over 11 trials), faster than the tuner
    a8, evaluation 10: 8 knobs, 8 passing; best: knob 64160, block 32: 25.29 us (25.25 to 25.31 over 11 trials), faster than the tuner
  winner: a8, re-measured in evaluation 13, beside evaluations 14 and 15:
    same: knob 64160, block 32: 25.33 us (25.29 to 25.35 over 11 trials), 2.85 x the tuner, 4.92 x chapter 44; pass 0 9.26 us, pass 1 16.16 us at block 32
    167 GB/s effective for the 4.23 MB a call must move (x read twice, out written)
    source psionic/kernels/norm/agent/a8.coax
    sha256 6dc4061dbb45b66137c43f1c0dee6acf7e87b987f31c039882b56b7be0142a80, at commit aca6a4ccfd
  evaluator time, admission to last trial: 415 s in all, 122 s of it the agent's 8
```

In short:
- **The tuner's best is 1.73 times faster than chapter 44.** It found 32
  contiguous chunks a row, with 4 outputs an item, at block size 32: 72.3 µs
  against 124.7 µs. It also generalizes: all 49 of its knobs pass the
  held-out widths, which chapter 44's kernels do not.
- **The agent's best is 2.85 times faster than the tuner.** Its eighth
  candidate takes 25.3 µs, 4.9 times faster than chapter 44, and passes every
  held-out case, bit for bit the same on the CPU and the L4. Seven of its
  eight candidates were correct and faster than the tuner. The first failed
  at launch.
- **Most of the gain came from one observation.** The agent saw that pass 1
  was spending its time converting f64 results to f32. It replaced the
  conversion with about fifteen integer operations that give the same
  bits.
- **The winner still moves 167 GB/s,** about half of the L4's memory
  bandwidth. Both passes have too few work items to fill the card, and the
  subset has no shared memory or barriers to do better.

## The cases and the contract

<!-- source: psionic/lib/kernel-search.coax cases -->
```jai
// Case 0 is the declared workload: Elephant's offset RMS norm over chapter
// 14's 86-token sample, 4,096 wide. The rest are held out from the agent:
// a decode row, Mammoth's width, widths no power of two divides, a width of
// 1, extreme values, zero rows, and one huge value among tiny ones.
KS_CASES :: 10;
KS_EPSILON :: 1e-6;
KsShape :: struct {rows:int;width:int;}
ks_shape :: (c:int)->KsShape #export_c {
    s:=[20]int.[86,4096, 1,4096, 7,5120, 3,1000, 5,4097, 2,1, 1,17, 4,4096, 2,4096, 3,4096];
    if c<0 || c>=KS_CASES {return KsShape.{};}
    return KsShape.{rows=s[2*c],width=s[2*c+1]};
}
// A value from a 32-bit xorshift: about -4 to 4 with 20 fraction bits.
ks_random :: (state:u32)->u32 {
    s:=state;s^=s<<13;s^=s>>17;s^=s<<5;return s;
}
ks_fill :: (c:int,x:[]f32,scale:[]f32) #export_c {
    sh:=ks_shape(c);n:=sh.rows*sh.width;state:=mul_wrap(cast(u32)(c+1),cast(u32)2654435761);
    extremes:=[8]f32.[3.4028235e38,-3.4028235e38,f32_from_bits(1),0,1,-1e-20,65504,-f32_from_bits(0x7fffff)];
    for i:0..<n {
        state=ks_random(state);
        x[i]=f32(cast(int)(state>>8)-(1<<23))/f32(1<<21);
        if c==7 {x[i]=extremes[i%8];}
        if c==8 {x[i]=0;}
        if c==9 {x[i]=ifx i%sh.width==(i/sh.width)*97 then 1e30 else 1e-30;}
    }
    for i:0..<sh.width {state=ks_random(state);scale[i]=f32(cast(int)(state>>8)-(1<<23))/f32(1<<24);}
}
```

The cases are generated, not stored, so the evaluator can rebuild them
anywhere. Case 0 is the only shape the agent was told about and the only one
that is timed. The other nine catch what a fast kernel tends to break:
- widths that no chunk count divides;
- a width of 1, where most chunks are empty;
- values at the edges of f32: its largest value, subnormals, a huge value
  among tiny ones, and rows of zeros.

<!-- source: psionic/lib/kernel-search.coax contract -->
```jai
// The numerical contract: each output is one of the two f32 values next to
// the f64 norm (squares added in f64 in column order), so the only freedom
// is the order of the f64 additions. An output still holding the host's
// prefill, the largest f32, was never written: no norm comes near it.
KsCheck :: struct {wrong:int;unwritten:int;first:int;}
KS_UNWRITTEN :: 0x7f7fffff;
ks_ulp :: (r:f32)->f64 {
    a:=abs(r);return f64(f32_from_bits(f32_bits(a)+1))-f64(a);
}
// Written so that a NaN, which compares false, is not within.
ks_within :: (got:f32,want:f64)->bool {return abs(f64(got)-want)<=ks_ulp(f32(want));}
ks_check :: (c:int,x:[]f32,scale:[]f32,out:[]f32)->KsCheck #export_c {
    sh:=ks_shape(c);k:KsCheck;k.first=-1;
    for r:0..<sh.rows {
        at:=r*sh.width;sum:f64=0;
        for i:0..<sh.width {v:=f64(x[at+i]);sum+=v*v;}
        d:=sqrt(sum/f64(sh.width)+f64(KS_EPSILON));
        for i:0..<sh.width {
            want:=f64(x[at+i])/d*(1+f64(scale[i]));got:=out[at+i];
            bad:=false;
            if f32_bits(got)==KS_UNWRITTEN {k.unwritten+=1;bad=true;}
            else if !ks_within(got,want) {k.wrong+=1;bad=true;}
            if bad && k.first<0 {k.first=at+i;}
        }
    }
    return k;
}
```

The contract leaves one freedom: the order of the f64 additions. Any order
changes the f64 result by a few units in its last place. After one rounding,
the output is still one of the two f32 values next to the reference. Adding
the squares in f32 is caught: the library's own test shows it missing the
contract on a decode row.

The host adds three checks the contract cannot express:
- **Order.** On the CPU it runs every item in ascending and then in
  descending order. The two results must have the same bits.
- **Device against CPU.** The L4's result must have the same bits as the
  same Coax compiled for the CPU.
- **Repeats.** The L4 runs the kernel twice and must give the same bits.

Bounds checks stay on, so an out-of-range index stops the run as a trap. A
kernel that cannot launch at a listed block size fails as a whole.

## The family the tuner searches

<!-- source: psionic/lib/device/norm-search.coax knobs -->
```jai
// A knob is chunks*100 + vector*10 + strided. Chunks is how many items add
// one row's squares (0: one item does the whole row, both passes fused);
// strided items read columns j, j+chunks, ... instead of one contiguous
// run; vector is how many outputs one pass-1 item writes.
nt_knobs :: (item:int,knobs:[]int) #export_c {
    if item!=0 {return;}
    knobs[0]=0;n:=1;
    // Chunks 1 to 128 and vectors 1 to 4, in powers of two. (Constant arrays
    // are not yet emitted for the device, so these are loops.)
    for c:0..<8 {for v:0..<3 {for s:0..<2 {knobs[n]=(1<<c)*100+(1<<v)*10+s;n+=1;}}}
    knobs[n]=-1;
}
nt_plan :: (item:int,rows:int,width:int,knob:int,plan:[]int) #export_c {
    if item!=0 {return;}
    c:=knob/100;v:=(knob/10)%10;
    if c==0 {plan[0]=rows;plan[1]=0;plan[2]=rows;return;}
    plan[0]=rows*c;plan[1]=rows*((width+v-1)/v);plan[2]=rows*c;
}
```

<!-- source: psionic/lib/device/norm-search.coax passes -->
```jai
nt_divisor :: (squares:f64,width:int,epsilon:f32)->f64 {return sqrt(squares/f64(width)+f64(epsilon));}
nt_write :: (at:int,i:int,divisor:f64,x:[]f32,scale:[]f32,out:[]f32) {
    out[at+i]=f32(f64(x[at+i])/divisor*(1+f64(scale[i])));
}
// Pass 0: item r*chunks+j adds the squares of its share of row r. Fused,
// item r adds the whole row and writes it.
nt_pass0 :: (item:int,rows:int,width:int,knob:int,epsilon:f32,x:[]f32,scale:[]f32,work:[]f64,out:[]f32) #export_c {
    c:=knob/100;s:=knob%10;sum:f64=0;
    if c==0 {
        at:=item*width;
        for i:0..<width {v:=f64(x[at+i]);sum+=v*v;}
        divisor:=nt_divisor(sum,width,epsilon);
        for i:0..<width {nt_write(at,i,divisor,x,scale,out);}
        work[item]=sum;return;
    }
    at:=(item/c)*width;j:=item%c;
    if s==1 {
        for k:0..<(width-j+c-1)/c {v:=f64(x[at+j+k*c]);sum+=v*v;}
    } else {
        // A test per column here costs 3 us on the L4; the run is clamped once.
        // (hi>width and hi>=width both leave hi at width: an equivalent mutant.)
        per:=(width+c-1)/c;lo:=j*per;hi:=lo+per;
        if hi>width {hi=width;}
        for i:lo..<hi {v:=f64(x[at+i]);sum+=v*v;}
    }
    work[item]=sum;
}
// Pass 1: item r*ceil(width/vector)+k adds row r's partial sums in order and
// writes outputs k*vector up to the row's end.
nt_pass1 :: (item:int,rows:int,width:int,knob:int,epsilon:f32,x:[]f32,scale:[]f32,work:[]f64,out:[]f32) #export_c {
    c:=knob/100;v:=(knob/10)%10;m:=(width+v-1)/v;r:=item/m;k:=item%m;sum:f64=0;
    for j:0..<c {sum+=work[r*c+j];}
    divisor:=nt_divisor(sum,width,epsilon);
    for i:k*v..<k*v+v {if i<width {nt_write(r*width,i,divisor,x,scale,out);}}
}
```

Every implementation, the agent's included, exports the same four kernels
under its own prefix: its knobs, its plan, and two passes. `coax cuda`
compiles every `#export_c` procedure as a kernel, so the plan is a kernel
too. The host runs it on the CPU and reads back the work items of each pass
and the scratch it needs.

Chapter 44's kernels go behind the same contract:

<!-- source: psionic/lib/device/norm-search.coax baseline -->
```jai
// Chapter 44's kernels: 16 items per row, each adding a contiguous sixteenth
// of width/16 columns, then one item per output. Its declared shapes have a
// width that is a multiple of 16.
n30_knobs :: (item:int,knobs:[]int) #export_c {if item==0 {knobs[0]=0;knobs[1]=-1;}}
n30_plan :: (item:int,rows:int,width:int,knob:int,plan:[]int) #export_c {
    if item!=0 {return;}
    plan[0]=rows*DL_CHUNKS;plan[1]=rows*width;plan[2]=rows*DL_CHUNKS;
}
n30_pass0 :: (item:int,rows:int,width:int,knob:int,epsilon:f32,x:[]f32,scale:[]f32,work:[]f64,out:[]f32) #export_c {
    dl_squares(item,width,x,work);
}
n30_pass1 :: (item:int,rows:int,width:int,knob:int,epsilon:f32,x:[]f32,scale:[]f32,work:[]f64,out:[]f32) #export_c {
    dl_scale(item,width,epsilon,x,scale,work,out);
}
```

They were written for Elephant's widths, and on a width that 16 does not
divide they never add the last columns. The example therefore holds them
only to their declared widths. That is a finding about chapter 44's kernels,
not a fault in Elephant, whose widths are 4,096 and 5,120.

## Timings and the tuner's choice

<!-- source: psionic/lib/kernel-search.coax timing -->
```jai
// One configuration's trials (seconds per call of both passes). It is
// faster than another only when its slowest trial beats the other's fastest.
KS_TRIALS_MAX :: 64;
KsTiming :: struct {median:f64;low:f64;high:f64;count:int;}
ks_timing :: (trials:[]f64)->KsTiming {
    t:KsTiming;n:=trials.count;
    if n<1 || n>KS_TRIALS_MAX {return t;}
    s:[KS_TRIALS_MAX]f64;
    for v,i:trials {
        j:=i;
        while j>0 && s[j-1]>v {s[j]=s[j-1];j-=1;}
        s[j]=v;
    }
    t.median=ifx n%2==1 then s[n/2] else (s[n/2-1]+s[n/2])/2;
    t.low=s[0];t.high=s[n-1];t.count=n;
    return t;
}
ks_faster :: (a:KsTiming,b:KsTiming)->bool {return a.count>0 && b.count>0 && a.high<b.low;}
```

<!-- source: psionic/lib/kernel-search.coax choose -->
```jai
// The tuner's choice among measured configurations: the lowest median among
// those that passed every case. -1 when none passed.
KsConfig :: struct {knob:int;block:int;passed:bool;timing:KsTiming;}
ks_best :: (configs:[]KsConfig)->int {
    best:=-1;
    for c,i:configs {
        if !c.passed || c.timing.count==0 {continue;}
        if best<0 || c.timing.median<configs[best].timing.median {best=i;}
    }
    return best;
}
```

Each configuration is timed in 11 trials of 50 calls, between CUDA events.
A trial's spread is a few hundredths of a microsecond. "Faster" means no
overlap: the slower trial of one must beat the faster trial of the other.

The spread within a run says little about the spread between runs, and the
ledger shows it. A rewrite of the tuner's pass 0 that made the tests stronger
also put a bounds test on every column, and the tuner's best went from
72.3 to 75.4 µs. That is a hundred times the spread within the run, and it
was a real slowdown, not noise. Pass 0 had gone from 18.0 to 21.2 µs. The
loop was restored, and the next run measured 72.29 µs against the first
run's 72.32. Comparing one session with another is how such a change gets
found. For the same reason the winner was measured again beside the final
runs of the tuner and of chapter 44.

## The agent

The agent was a Claude subagent working in the same checkout. Its brief
gave it:
- the operation, the contract and the workload;
- the interface, and chapter 37's rules for the subset;
- the results of chapter 44 and of the tuner, with times per pass;
- a free CPU-only mode of the evaluator, and a budget of eight full
  evaluations, failures included.

It could write only in `psionic/kernels/norm/agent/`. It was told not to
read the evaluator, the host, or the ledger, so it never saw the held-out
cases. Nothing enforced that rule during the run. Afterwards the checkout
showed changes only in its own directory, and each evaluated source still
matched the digest in the ledger.

Its eight candidates are in [`psionic/kernels/norm/agent/`](../kernels/norm/agent):

| | Idea | Best on the L4 |
| --- | --- | --- |
| [a1](../kernels/norm/agent/a1.coax) | strided pass 0 with four f64 accumulators; pass 1 computes the reciprocal once and writes every m-th column | failed: too many registers to launch at block 1,024 |
| [a2](../kernels/norm/agent/a2.coax) | a1 with one accumulator | 39.1 µs |
| [a3](../kernels/norm/agent/a3.coax) | pass-1 items covering several rows, to reuse 1 + scale | 43.6 µs |
| [a4](../kernels/norm/agent/a4.coax) | running indices, two accumulators | 35.6 µs |
| [a5](../kernels/norm/agent/a5.coax) | 1 + scale stored once in scratch; f64 to f32 rounded with integer bit operations | 27.9 µs |
| [a6](../kernels/norm/agent/a6.coax) | four loads a step in both passes | 27.9 µs |
| [a7](../kernels/norm/agent/a7.coax) | a5's pass 0 with a6's pass 1, knobs between powers of two | 26.4 µs |
| [a8](../kernels/norm/agent/a8.coax) | a7 with contiguous chunks in pass 0 | 25.3 µs |

The largest single gain was a5's. On the L4 the conversion from f64 to f32
is slow, and doing it by hand cut pass 1 from 22.4 to 17.7 µs. The
hand-written version takes the exponent and the top 23 bits of the
mantissa, then rounds to nearest even. It falls back to the cast for
subnormals and overflow. The agent's tests check it against the cast on
up to 3,000 bit patterns, exact ties and both ends of the f32 range, and the
evaluator's contract and bit-for-bit checks cover the rest. The same trick
for widening was slower, and the agent kept the cast there.

The agent also measured the shape of the problem. More pass-0 items mean
more partial sums for each pass-1 item to add, and the best balance was
about 64 partials and 128 to 160 pass-1 items a row. Block size 32 won every
time, because neither pass has enough items to fill 58 multiprocessors.

## The ledger and the example

<!-- source: psionic/examples/kernel-search.coax live -->
```jai
// One implementation over every case on this CPU: pass 0's items, then pass
// 1's, in order, then chapter 247's contract.
Impl :: enum u8 {ch30;tuned;winner;}
Live :: struct {passed:int;failed:[10]bool;}
live :: (impl:Impl,knob:int,x:[]f32,scale:[]f32,out:[]f32,work:[]f64)->Live {
    l:Live;
    for c:0..<KS_CASES {
        sh:=ks_shape(c);n:=sh.rows*sh.width;
        ks_fill(c,x[0..<n],scale[0..<sh.width]);
        for i:0..<n {out[i]=f32_from_bits(KS_UNWRITTEN);}
        plan:[3]int;
        if impl==Impl.ch30 {n30_plan(0,sh.rows,sh.width,knob,plan[..]);}
        else if impl==Impl.tuned {nt_plan(0,sh.rows,sh.width,knob,plan[..]);}
        else {a8_plan(0,sh.rows,sh.width,knob,plan[..]);}
        xv:=x[0..<n];sv:=scale[0..<sh.width];ov:=out[0..<n];wv:=work[0..<plan[2]];
        for p:0..<2 {
            for i:0..<plan[p] {
                if impl==Impl.ch30 {
                    if p==0 {n30_pass0(i,sh.rows,sh.width,knob,KS_EPSILON,xv,sv,wv,ov);} else {n30_pass1(i,sh.rows,sh.width,knob,KS_EPSILON,xv,sv,wv,ov);}
                } else if impl==Impl.tuned {
                    if p==0 {nt_pass0(i,sh.rows,sh.width,knob,KS_EPSILON,xv,sv,wv,ov);} else {nt_pass1(i,sh.rows,sh.width,knob,KS_EPSILON,xv,sv,wv,ov);}
                } else {
                    if p==0 {a8_pass0(i,sh.rows,sh.width,knob,KS_EPSILON,xv,sv,wv,ov);} else {a8_pass1(i,sh.rows,sh.width,knob,KS_EPSILON,xv,sv,wv,ov);}
                }
            }
        }
        k:=ks_check(c,xv,sv,ov);
        if k.wrong==0 && k.unwritten==0 {l.passed+=1;} else {l.failed[c]=true;}
    }
    return l;
}
```

<!-- source: psionic/examples/kernel-search.coax ledger -->
```jai
// A knob passed when every case was ok on the CPU and on the L4. Chapter
// 44's kernels are held only to their own declared widths, multiples of 16.
passed :: (kk:int,declared:bool)->bool {
    for c:0..<KS_CASES {
        if declared && ks_shape(c).width%16!=0 {continue;}
        if KK_CPU[kk*KS_CASES+c]!=1 || KK_DEVICE[kk*KS_CASES+c]!=1 {return false;}
    }
    return true;
}
timing :: (kk:int,b:int)->KsTiming {
    t:[TRIALS]f64;at:=(kk*BLOCKS.count+b)*TRIALS;
    if KK_TRIALS[at]==0 {return KsTiming.{};}
    for j:0..<TRIALS {t[j]=KK_TRIALS[at+j];}
    return ks_timing(t[..]);
}
// Every configuration an evaluation measured, as the tuner sees them.
Measured :: struct {configs:[KS_TRIALS_MAX*6]KsConfig;kk:[KS_TRIALS_MAX*6]int;count:int;knobs:int;passing:int;}
measured :: (run:int,declared:bool)->Measured {
    m:Measured;
    for kk:0..<KK_COUNT {
        if KK_RUN[kk]!=run {continue;}
        m.knobs+=1;ok:=passed(kk,declared);
        if ok {m.passing+=1;}
        for b:0..<BLOCKS.count {
            t:=timing(kk,b);
            if t.count==0 {continue;}
            m.configs[m.count]=KsConfig.{knob=KK_KNOB[kk],block=BLOCKS[b],passed=ok,timing=t};m.kk[m.count]=kk;m.count+=1;
        }
    }
    return m;
}
AGENT_DIR :: "psionic/kernels/norm/agent/";
agent_file :: (s:string)->bool {
    if s.count<AGENT_DIR.count {return false;}
    for i:0..<AGENT_DIR.count {if byte_at(s,i)!=byte_at(AGENT_DIR,i) {return false;}}
    return true;
}
latest :: (prefix:string)->int {
    last:=-1;
    for r:0..<KR_COUNT {if KR_PREFIX[r]==prefix {last=r;}}
    return last;
}
```

[`kernel_search_eval.sh`](../tests/kernel_search_eval.sh) is the only way in.
It checks admission with `coax cuda` and runs the CPU gates. It then ships
the generated CUDA and C to the L4, builds there, and runs the device gates
and the timings. Each evaluation is written in full to
[`psionic/kernels/norm/ledger/`](../kernels/norm/ledger). Its header holds:
- the source and its SHA-256;
- the time in Chicago;
- the commit;
- from evaluation 11 on, a digest of the evaluator's own files;
- the GPU and its driver, in the device lines.

[`kernel_search_fixture.py`](../tests/kernel_search_fixture.py) renders the
ledger as [Coax data](../fixtures/kernel-search-l4.coax), and its `--check`
mode confirms the rendering. The [example](../examples/kernel-search.coax)
makes every decision in Coax:
- which configurations passed;
- the medians;
- each candidate's best;
- whether it is faster than the tuner;
- the winner.

It also reruns three kernels on the CPU over all ten cases, so the
correctness half of the verdict can be repeated on a machine without a GPU.

<!-- source: psionic/examples/kernel-search.coax example -->
```jai
main :: () {
    print("workload: Elephant's offset RMS norm over chapter 14's 86-token sample, 86 x 4096, epsilon 1e-6\n");
    print("contract: squares added in f64 in any order, the rest in f64, one rounding to f32;\n");
    print("  each output must be one of the two f32 values next to the f64 norm\n");
    print("held out from the agent:");
    for c:1..<KS_CASES {print("%",ifx c==1 then " " else ", ");print("%",CASE_NAMES[c]);}
    print("\n\non this CPU, now, every case:\n");
    x:[352256]f32;scale:[5120]f32;out:[352256]f32;work:[88064]f64;
    impls:=[3]Impl.[Impl.ch30,Impl.tuned,Impl.winner];knobs:=[3]int.[0,3240,64160];
    names:=[3]string.["chapter 44 (n30)","the tuner's choice (nt 3240)","the agent's winner (a8 64160)"];
    for i:0..<3 {
        l:=live(impls[i],knobs[i],x[..],scale[..],out[..],work[..]);
        print("  %: % of % cases",names[i],l.passed,KS_CASES);
        first:=true;
        for c:0..<KS_CASES {if l.failed[c] {print("%",ifx first then "; wrong on " else ", ");print("%",CASE_NAMES[c]);first=false;}}
        print("\n");
    }

    base:=latest("n30");tuner:=latest("nt");
    print("\non the L4 (%), from the ledger of % evaluations:\n",KR_GPU[base],KR_COUNT);
    b:=measured(base,true);bi:=ks_best(b.configs[0..<b.count]);
    print("  chapter 44, evaluation %, on its declared widths: ",base+1);show("best",b.configs[bi]);pass_split(b,bi);
    t:=measured(tuner,false);ti:=ks_best(t.configs[0..<t.count]);best:=t.configs[ti];
    print("  the tuner, evaluation %: % knobs x % blocks = % configurations, % knobs passing every case on both\n",
        tuner+1,t.knobs,BLOCKS.count,t.count,t.passing);
    print("    ");show("best",best);describe(best.knob);print(", % x chapter 44",r2(b.configs[bi].timing.median/best.timing.median));pass_split(t,ti);
    slow:=0;
    for c,i:t.configs {if c.timing.median>t.configs[slow].timing.median {slow=i;}}
    print("    ");show("slowest",t.configs[slow]);describe(t.configs[slow].knob);print("\n");

    budgeted:=0;
    for r:0..<KR_COUNT {if agent_file(KR_FILE[r]) && !KR_CONFIRM[r] {budgeted+=1;}}
    print("  the agent: % budgeted evaluations, failures included\n",budgeted);
    winner:=-1;wbest:KsConfig;
    for r:0..<KR_COUNT {
        if !agent_file(KR_FILE[r]) || KR_CONFIRM[r] {continue;}
        m:=measured(r,false);print("    %, evaluation %: % knobs",KR_PREFIX[r],r+1,m.knobs);
        if KR_NOTE[r].count>0 {
            last:=0;
            for c:m.configs[0..<m.count] {if c.knob==m.configs[0].knob {last+=1;}}
            print(", stopped at block %: %\n",BLOCKS[last],KR_NOTE[r]);continue;
        }
        i:=ks_best(m.configs[0..<m.count]);c:=m.configs[i];
        print(", % passing; ",m.passing);show("best",c);
        print(", % than the tuner\n",ifx ks_faster(c.timing,best.timing) then "faster" else "not faster");
        if ks_faster(c.timing,best.timing) && (winner<0 || c.timing.median<wbest.timing.median) {winner=r;wbest=c;}
    }
    confirm:=latest(KR_PREFIX[winner]);
    m:=measured(confirm,false);ci:=-1;
    for c,i:m.configs[0..<m.count] {if c.knob==wbest.knob && c.block==wbest.block {ci=i;}}
    c:=m.configs[ci];
    print("  winner: %, re-measured in evaluation %, beside evaluations % and %:\n",KR_PREFIX[winner],confirm+1,tuner+1,base+1);
    print("    ");show("same",c);print(", % x the tuner, % x chapter 44",r2(best.timing.median/c.timing.median),r2(b.configs[bi].timing.median/c.timing.median));
    pass_split(m,ci);
    print("    % GB/s effective for the 4.23 MB a call must move (x read twice, out written)\n",
        cast(int)floor(3*352256*4/c.timing.median/1e9+0.5));
    print("    source %\n    sha256 %, at commit %\n",KR_FILE[confirm],KR_SHA[confirm],KR_COMMIT[confirm]);
    total:=0;agent:=0;
    for r:0..<KR_COUNT {
        total+=KR_SECONDS[r];
        if agent_file(KR_FILE[r]) && !KR_CONFIRM[r] {agent+=KR_SECONDS[r];}
    }
    print("  evaluator time, admission to last trial: % s in all, % s of it the agent's %\n",total,agent,budgeted);
}
```

## What the search cost

- **The L4:** 415 s of evaluator time across all 15 evaluations, 122 s of
  it the agent's. The VM is a `g2-standard-4` spot instance at about $0.42 an
  hour, so the L4 time cost about five cents.
- **The agent:** two sessions, 289,648 tokens, 17 minutes. The first session
  ran into a bug in the evaluator: its budget count failed while the ledger
  held no agent entries. Four calls ended silently and wrote nothing, so they
  used none of the budget. The bug was fixed, and the second session spent
  all eight evaluations.
- **The baseline and the tuner:** run three times each. Once before the
  agent, once after a change to the tuner's source, and once at the end.
  The first two pairs stay in the ledger. Their source digests no longer
  match, and the example reads only the latest.

## How it is checked

- **13 Coax checks:**
  - the cases, the generator's values and its planted extremes;
  - the contract at exactly one f32 step on each side, unwritten outputs,
    and squares added in f32;
  - the timing statistics, the no-overlap rule and the tuner's choice;
  - every tuner knob on a width no knob divides;
  - each plan, partial sums by hand, vector outputs at a row's end, and
    chapter 44's kernels inside and outside their widths.
- **[`kernel_search_oracle.py`](../tests/kernel_search_oracle.py)** rebuilds
  the example's output line for line. It regenerates the ten cases with its
  own xorshift. It reruns the arithmetic of chapter 44's kernels, the tuner's
  choice and the agent's winner in binary64 with one rounding, and applies
  the contract itself. It reads the raw ledger directly, not through the
  fixture.
- **Mutation:** the evaluator's 202 mutants and the kernel family's 704, all killed but one equivalent: the clamp in the tuner's contiguous pass 0, where hi > width and hi ≥ width both leave hi at width.

## Limits

- **One kernel, one shape, one card.** The result says nothing yet about
  the gated feed-forward block, about other shapes, or about other GPUs.
- **No profiler beyond CUDA events.** The agent saw the time of each pass,
  not occupancy or memory traffic, and could not see register counts. That
  is how a1 failed.
- **A shared card.** The L4 kept Elephant resident, idle, through every
  run. All three implementations were timed under the same conditions.
- **The read boundary was a rule, not a sandbox.** The held-out cases are in
  the same checkout as the agent. A later run should give the agent a copy
  without the evaluator.
- **The subset itself caps the result.** Without shared memory or barriers,
  a row's sum needs two passes and a trip through global memory. The
  compiler's own [`compute.coax`](../../coax/lib/compiler/compute.coax) now
  describes warp and group kernels, but this evaluator launches only plain
  item kernels. Constant arrays are not yet emitted for the device either:
  the tuner's knob list had to become loops.
