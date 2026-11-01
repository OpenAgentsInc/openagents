# 34. Benchmark against llama.cpp and cuBLAS

> **Status: partly implemented.** The comparison protocol (one warm-up run,
> at least five timed warm runs, mean and sample standard deviation by two
> independent methods, and a verdict that refuses unless both sides did the
> same work on the same machine with both builds recorded) runs on the CPU,
> and an opt-in harness applies it to Psionic's CUDA projection kernels
> against cuBLAS on nine Elephant shapes, recorded on one named host. The
> llama.cpp side is not built: no packaged llama.cpp build is approved yet,
> and Psionic has no engine that runs a whole GGUF file on the GPU to put
> beside it.

A speed number means little without a comparator that runs the same work on
the same machine. A comparison also means little when the two sides did
different work: a faster time from a run that read f32 weights is not a
result about a kernel that reads BF16 weights. This chapter writes the
comparison down as a protocol in Coax, checks its statistics against hand
calculations and an exact integer formula, and runs it on Psionic's own
matrix kernels against cuBLAS. Neither comparator becomes a dependency.

The library is [`comparator.coax`](../lib/comparator.coax), an `#authority`
file because its verdict decides which claims about speed the book may make.
The measuring harness is
[`cublas_bench.c`](../tests/cublas_bench.c), run by
[`bench_cublas.sh`](../tests/bench_cublas.sh); it loads cuBLAS with `dlopen`
at run time, and no Psionic library or server links or loads it.

## The protocol

The profile is `cm-warm-v1`:

- Each side runs the work once untimed. This warm-up loads kernels, settles
  the library's algorithm choice and raises the device clocks. Its time is
  never recorded.
- Each side then runs at least 5 and at most 1,024 timed warm runs. A run
  time is a positive integer number of nanoseconds.
- Each side's runs become a mean and a sample standard deviation, the one
  with `n - 1` in the denominator, as llama.cpp's `llama-bench` reports.
- A comparison names the work on both sides: an identity word for the weight
  file (or for generated weights), the weight, activation and accumulation
  formats, and the shape (rows by columns of weights, times tokens). It also
  names the engine, its exact build and the machine on each side.
- The verdict refuses, with a typed reason, when a build, engine or machine
  is missing, the machines differ, the files differ, any format differs or is
  unknown, the shapes differ, a side skipped its warm-up, or a side's timings
  were themselves refused.
- If either side's standard deviation is more than a tenth of its mean, the
  verdict is "too noisy to rank".
- Otherwise side a is faster when its mean is lower than b's by more than two
  standard errors of the difference, slower when it is higher by more than
  that, and the same within two standard errors when neither holds.

The standard error of the difference of two means is

```text
se = sqrt(sd_a^2 / n_a + sd_b^2 / n_b)
```

and the speed ratio is b's mean over a's, above 1 when a is faster.

## Work it by hand

Take five runs of side a: 100, 104, 96, 102 and 98 ns.

```text
sum            = 100 + 104 + 96 + 102 + 98 = 500
mean           = 500 / 5 = 100
deviations     = 0, 4, -4, 2, -2
squares        = 0, 16, 16, 4, 4, summing to 40
variance       = 40 / (5 - 1) = 10
sd             = sqrt(10) = 3.16227766...
```

Side b ran 110, 112, 108, 114 and 106 ns: the same deviations around 110, so
the same standard deviation. Then

```text
se    = sqrt(10 / 5 + 10 / 5) = sqrt(4) = 2
diff  = 100 - 110 = -10, below -2 * se = -4   -> a is faster
ratio = 110 / 100 = 1.1
```

A side c with runs 103, 105, 101, 107 and 99 ns has mean 103 and the same
spread. The difference is -3, inside plus or minus 4, so a and c are the same
within two standard errors, even though a's mean is lower. A side d with runs
100, 100, 100, 100 and 150 ns has mean 110, deviations -10 four times and 40
once, squares summing to 2,000, variance 500 and a standard deviation of
22.36 ns. That is more than a tenth of 110, so no ranking is made.

## Two ways to the same statistics

The two methods share their admission rule: 5 to 1,024 runs, each positive.

<!-- source: psionic/lib/comparator.coax admit -->
```jai
// The checks both methods share: 5 to 1,024 runs, each a positive time.
cm_admit :: (x:[]int)->CmStats {
    st:=CmStats.{n=x.count};
    if x.count<CM_RUNS_MIN {st.status=CmStatus.few;return st;}
    if x.count>CM_RUNS_MAX {st.status=CmStatus.capacity;return st;}
    for v:x {if v<=0 {st.status=CmStatus.sample;return st;}}
    return st;
}
```

The first method makes two passes: one for the mean, one for the squared
distances from it.

<!-- source: psionic/lib/comparator.coax two-pass -->
```jai
// First pass: the mean. Second pass: squared distances from it.
cm_two_pass :: (x:[]int)->CmStats #export_c {
    st:=cm_admit(x);
    if st.status!=CmStatus.ok {return st;}
    sum:=0.0;
    for v:x {sum+=cast(f64)v;}
    st.mean=sum/cast(f64)x.count;
    sq:=0.0;
    for v:x {d:=cast(f64)v-st.mean;sq+=d*d;}
    st.sd=sqrt(sq/cast(f64)(x.count-1));
    return st;
}
```

The second is Welford's update, which reads each run once. After k runs it
holds their mean and `m2`, their summed squared distances from that mean.

<!-- source: psionic/lib/comparator.coax welford -->
```jai
// One pass: after k runs, mean is their mean and m2 their summed squared
// distances from it. Run k+1 moves the mean by d / (k + 1) and adds
// d * (x - new mean) to m2, where d = x - old mean.
cm_welford :: (x:[]int)->CmStats #export_c {
    st:=cm_admit(x);
    if st.status!=CmStatus.ok {return st;}
    mean:=0.0;m2:=0.0;k:=0;
    for v:x {
        k+=1;f:=cast(f64)v;
        d:=f-mean;
        mean+=d/cast(f64)k;
        m2+=d*(f-mean);
    }
    st.mean=mean;
    st.sd=sqrt(m2/cast(f64)(k-1));
    return st;
}
```

Trace Welford on the runs 10, 12, 14, 16 and 18:

| Run | d = x - old mean | New mean | Added to m2 | m2 |
| --- | --- | --- | --- | --- |
| 10 | 10 | 10 | 10 × 0 = 0 | 0 |
| 12 | 2 | 11 | 2 × 1 = 2 | 2 |
| 14 | 3 | 12 | 3 × 2 = 6 | 8 |
| 16 | 4 | 13 | 4 × 3 = 12 | 20 |
| 18 | 5 | 14 | 5 × 4 = 20 | 40 |

So m2 is 40 and the variance is 40 / 4 = 10, the same as the two-pass sum of
squares 16 + 4 + 0 + 4 + 16. Every intermediate here is an exact binary64
value, so the checks compare both results with `sqrt(10.0)` for equality.

Neither method uses the one-pass textbook formula
`(sum x^2 - (sum x)^2 / n) / (n - 1)`. Runs near one second have squares near
10^18, where neighbouring binary64 values are hundreds apart, and the
subtraction loses the spread entirely. A check takes four runs of
1,000,000,004 to 1,000,000,016 ns, whose variance is 30, and shows that the
textbook formula misses it while both methods here return exact results for
five such runs.

## The verdict

<!-- source: psionic/lib/comparator.coax compare -->
```jai
cm_same_work :: (a:CmWork,b:CmWork)->CmReason {
    if a.file!=b.file {return CmReason.file;}
    known:=a.weights!=CmFormat.unknown && a.activations!=CmFormat.unknown && a.accumulate!=CmFormat.unknown;
    if !known || a.weights!=b.weights || a.activations!=b.activations || a.accumulate!=b.accumulate {return CmReason.format;}
    if a.rows!=b.rows || a.cols!=b.cols || a.tokens!=b.tokens {return CmReason.shape;}
    return CmReason.none;
}
// Side a against side b. The mean of each side is in nanoseconds, so lower
// is faster.
cm_compare :: (a:CmSide,sa:CmStats,b:CmSide,sb:CmStats)->CmCall #export_c {
    call:=CmCall.{verdict=CmVerdict.refused};
    if a.engine==0 || a.build==0 || a.machine==0 || b.engine==0 || b.build==0 || b.machine==0 {call.reason=CmReason.unbound;return call;}
    if a.machine!=b.machine {call.reason=CmReason.machine;return call;}
    call.reason=cm_same_work(a.work,b.work);
    if call.reason!=CmReason.none {return call;}
    if a.warmup<1 || b.warmup<1 {call.reason=CmReason.warmup;return call;}
    if sa.status!=CmStatus.ok || sb.status!=CmStatus.ok {call.reason=CmReason.stats;return call;}
    call.ratio=sb.mean/sa.mean;
    call.se=sqrt(sa.sd*sa.sd/cast(f64)sa.n+sb.sd*sb.sd/cast(f64)sb.n);
    if sa.sd*10>sa.mean || sb.sd*10>sb.mean {call.verdict=CmVerdict.noisy;return call;}
    diff:=sa.mean-sb.mean;
    call.verdict=CmVerdict.same;
    if diff< -2*call.se {call.verdict=CmVerdict.faster;}
    if diff>2*call.se {call.verdict=CmVerdict.slower;}
    return call;
}
```

The refusals come first, so a refused comparison never carries a ratio. The
first difference found is the one reported: machine, then file, then format,
then shape.

## Run the worked example

The example, [`comparator-protocol.coax`](../examples/comparator-protocol.coax),
runs the four sides worked above through both methods and the verdict. Its runs are hand-picked values, not measurements.

<!-- source: psionic/examples/comparator-protocol.coax protocol -->
```jai
main :: () {
    a:=[5]int.[100,104,96,102,98];
    b:=[5]int.[110,112,108,114,106];
    c:=[5]int.[103,105,101,107,99];
    d:=[5]int.[100,100,100,100,150];
    show("a",a[..]);
    show("b",b[..]);
    show("c",c[..]);
    show("d",d[..]);
    w:=CmWork.{file=11,weights=CmFormat.bf16,activations=CmFormat.bf16,accumulate=CmFormat.f32,rows=4096,cols=4096,tokens=86};
    ours:=CmSide.{work=w,engine=1,build=2,machine=3,warmup=1};
    theirs:=ours;theirs.engine=4;theirs.build=5;
    judge("a against b",ours,cm_two_pass(a[..]),theirs,cm_two_pass(b[..]));
    judge("a against c",ours,cm_two_pass(a[..]),theirs,cm_two_pass(c[..]));
    judge("a against d",ours,cm_two_pass(a[..]),theirs,cm_two_pass(d[..]));
}
```

```shell
coax/bin/coax run psionic/examples/comparator-protocol.coax
```

```text
a: 5 runs, mean 100.0 ns, sd 3.1622776601683795 ns (two passes); mean 100.0 ns, sd 3.1622776601683795 ns (Welford)
b: 5 runs, mean 110.0 ns, sd 3.1622776601683795 ns (two passes); mean 110.0 ns, sd 3.1622776601683795 ns (Welford)
c: 5 runs, mean 103.0 ns, sd 3.1622776601683795 ns (two passes); mean 103.0 ns, sd 3.1622776601683795 ns (Welford)
d: 5 runs, mean 110.0 ns, sd 22.360679774997898 ns (two passes); mean 110.0 ns, sd 22.360679774997898 ns (Welford)
a against b: faster, speed ratio 1.1, standard error 2.0 ns
a against c: the same within two standard errors, speed ratio 1.03, standard error 2.0 ns
a against d: too noisy to rank, speed ratio 1.1, standard error 10.09950493836208 ns
```

For d, the standard error is `sqrt(10 / 5 + 500 / 5) = sqrt(102) = 10.0995`.

## Refuse an unfair comparison

[`comparator-refuse.coax`](../examples/comparator-refuse.coax) changes one
thing at a time from a fair comparison.

<!-- source: psionic/examples/comparator-refuse.coax refuse -->
```jai
main :: () {
    runs:=[5]int.[100,104,96,102,98];
    four:=cm_two_pass(runs[0..<4]);
    print("four warm runs: %\n",cm_status_name(four.status));
    zero:=runs;zero[2]=0;
    print("a run of 0 ns: %\n",cm_status_name(cm_welford(zero[..]).status));
    s:=cm_two_pass(runs[..]);
    w:=CmWork.{file=11,weights=CmFormat.bf16,activations=CmFormat.bf16,accumulate=CmFormat.f32,rows=4096,cols=4096,tokens=86};
    ours:=CmSide.{work=w,engine=1,build=2,machine=3,warmup=1};
    theirs:=ours;theirs.engine=4;theirs.build=5;
    x:=theirs;x.build=0;judge("the comparator's build not recorded",ours,s,x,s);
    x=theirs;x.machine=6;judge("another machine",ours,s,x,s);
    x=theirs;x.work.file=12;judge("another weight file",ours,s,x,s);
    x=theirs;x.work.weights=CmFormat.f32;x.work.activations=CmFormat.f32;judge("f32 SGEMM against BF16 weights",ours,s,x,s);
    x=theirs;x.work.tokens=1;judge("one token against 86",ours,s,x,s);
    x=theirs;x.warmup=0;judge("no warm-up run",ours,s,x,s);
    judge("four warm runs",ours,s,theirs,four);
}
```

```shell
coax/bin/coax run psionic/examples/comparator-refuse.coax
```

```text
four warm runs: fewer than 5 warm runs
a run of 0 ns: a run time that is not positive
the comparator's build not recorded: refused (an engine, build or machine is not recorded)
another machine: refused (different machines)
another weight file: refused (different weight files)
f32 SGEMM against BF16 weights: refused (different number formats)
one token against 86: refused (different shapes)
no warm-up run: refused (no warm-up run)
four warm runs: refused (the timings were refused)
```

The program exits with status 0: each refusal is a typed result it prints,
not an error.

## Measure Psionic's kernels against cuBLAS

The harness applies the protocol to chapter 44's projection kernels, the
Coax code in [`device/project.coax`](../lib/device/project.coax) compiled by
`coax cuda`. It links the C that `coax c --library` emits for
`comparator.coax`, so the statistics and verdict in the receipt are the ones
checked above.

For each of nine Elephant shapes, it generates seeded BF16 weights and
activations and runs three engines on them:

- Psionic: `dv_matvec` (one token) or `dv_project` (86 tokens), then
  `dv_reduce`, reading the BF16 weight buffer.
- cuBLAS `GemmEx`: the same BF16 weight buffer, BF16 activations and f32
  accumulation.
- cuBLAS `Sgemm`: the same weights widened to f32, with f32 activations.

Both Psionic and `GemmEx` see the same activation values, rounded to BF16;
Psionic reads them from f32 storage, which costs it twice the activation
bytes. The weight identity word is an FNV-1a hash of the BF16 weight bytes.
Psionic's build is the git blob ids of every file the measured program is
built from, and cuBLAS's is the resolved library file. The machine is the
host name and the device name.

A run is 20 back-to-back products between two CUDA events, recorded as
nanoseconds per product. Each engine does one untimed warm-up run, then seven
timed runs.

```shell
psionic/tests/bench_cublas.sh --record
```

The [receipt for coderos-4080](../fixtures/comparator/coderos-4080.json), an
RTX 4080 with driver 595.71.05 and cuBLAS 13.3.0.5, recorded 2026-11-01,
gives these means and sample standard deviations in microseconds:

| Shape (rows × cols × tokens) | Psionic | cuBLAS BF16 | Verdict | Speed ratio | cuBLAS f32 |
| --- | --- | --- | --- | --- | --- |
| 248,320 × 4,096 × 1 | 3,376.3 ± 35.4 | 3,044.0 ± 23.9 | slower | 0.90 | refused |
| 12,288 × 4,096 × 1 | 168.8 ± 5.6 | 155.9 ± 3.8 | slower | 0.92 | refused |
| 4,096 × 12,288 × 1 | 185.4 ± 5.4 | 156.1 ± 0.6 | slower | 0.84 | refused |
| 4,096 × 4,096 × 1 | 20.6 ± 0.1 | 15.6 ± 0.1 | slower | 0.76 | refused |
| 8,192 × 4,096 × 86 | 894.2 ± 26.6 | 101.7 ± 0.2 | slower | 0.11 | refused |
| 12,288 × 4,096 × 86 | 1,288.5 ± 14.3 | 200.8 ± 0.2 | slower | 0.16 | refused |
| 4,096 × 12,288 × 86 | 1,266.5 ± 4.5 | 185.3 ± 0.3 | slower | 0.15 | refused |
| 4,096 × 4,096 × 86 | 476.4 ± 20.3 | 52.5 ± 0.3 | slower | 0.11 | refused |
| 1,024 × 4,096 × 86 | 179.3 ± 0.4 | 17.5 ± 0.1 | slower | 0.10 | refused |

Every comparison with cuBLAS's f32 SGEMM is refused for its different number
formats. The receipt still keeps those runs, but no ratio against them is a
result about BF16 kernels. Psionic's outputs agree with `GemmEx`'s to within
10^-6 of the sum of absolute products on sampled outputs.

What the numbers say: the one-token products, which read every weight once
and are limited by memory bandwidth, reach 76% to 92% of cuBLAS's speed. The
86-token products reach 10% to 16%: cuBLAS uses the tensor cores and
Psionic's projection does not yet. That gap is the target for the
cooperating group kernels of [chapter 68](ch068-coax-kernels-cooperate.md).

The machine is shared with other agents. The receipt records a load average
of 33 to 34 on 28 hardware threads, and the GPU clocks before and after. GPU
timings come from CUDA events and are less sensitive to CPU load than CPU
timings, but another process on the GPU would still show. An earlier
unrecorded run the same day gave the same verdicts on seven shapes and "too
noisy to rank" on two, each with one slow run among seven. That is why the
protocol keeps a noise rule, and why no measured number appears in an exact
transcript.

## Check it

```shell
coax/bin/coax test psionic/lib/comparator.coax
coax/bin/coax mutate psionic/lib/comparator.coax --root-only
coax/bin/coax run psionic/examples/comparator-protocol.coax
coax/bin/coax run psionic/examples/comparator-refuse.coax
psionic/tests/bench_cublas.sh
```

The checks' expected values are the hand calculations above, and a
`#forall` over 420 seeded run sets compares both methods with an exact
integer formula: with `S` the sum of `n` runs, the variance is
`sum (n x - S)^2 / (n^2 (n - 1))`, every term an integer. Its helpers live in
[`comparator-checks.coax`](../tests/comparator-checks.coax). `coax mutate`
leaves no survivors in `comparator.coax` (127 mutants).

## What is not built yet

- **llama.cpp on the same file.** The comparison the chapter is named for
  needs two things that do not exist yet: an owner-approved packaged
  llama.cpp build (nixpkgs has `llama-cpp` version 9190; a CUDA build needs
  the unfree CUDA packages and a source build, recorded in `NEEDS_OWNER.md`),
  and a Psionic engine that runs a whole GGUF file on the GPU. Chapters 49
  to 51 read and decode GGUF blocks on the CPU only.
- **The llama-bench shapes.** Prompt processing and generation at fixed
  lengths, depth with a prefilled cache, decision-shaped runs with a shared
  prefix and several suffixes, and long prompts, each with matched threads,
  offload, attention mode, cache type and batch sizes.
- **A quiet machine.** Every figure above was taken under load from other
  agents.
- **The reference bar.** The survey's figure of about 0.2 s for a
  1,000-token decision on a 9B hybrid on this GPU class, from the pre-reset
  Rust Psionic and llama.cpp b11538, is an external figure. Nothing here
  reproduces it, and the earlier claim that Psionic beat llama.cpp stays
  unverified.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md), sections on the
  benchmark method and fair comparison.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md) for the
  receipt pattern and [chapter 44](ch044-elephant-on-cuda.md) for the
  projection kernels.
- B. P. Welford, "Note on a method for calculating corrected sums of squares
  and products", *Technometrics* 4(3), 1962.
