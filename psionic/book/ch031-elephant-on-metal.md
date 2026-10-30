# 31. Elephant on Metal

[Chapter 30](ch030-elephant-on-cuda.md) answered chapter 10's question on an
RTX 4080. This chapter answers it on the GPU of a MacBook Pro, an Apple M5 Max
with 128 GB of unified memory, from the same Coax kernels and the same device
plan. The answer agrees with the CPU to six decimal places. It is also about
nineteen times slower than the 4080, and this chapter says why.

The reason to run Elephant here is practical. One RTX 4080 holds Elephant or
gpt-oss-20b, not both, so a router on the 4080 forces a model swap before
every answer. With Elephant on the Mac, gpt-oss-20b stays loaded on the 4080
and the desktop app's Lab asks one, then the other, with nothing swapped.

The plan's title promises Mammoth too. Only Elephant has run.

## The same kernels, lowered for Metal

`coax metal-compute FILE` prints a file's kernels as Metal Shading Language.
It admits exactly what `coax cuda` admits ([chapter 26](ch026-a-coax-compute-subset.md)),
so `psionic/lib/device/project.coax`, `layer.coax` and `warp.coax` compile
unchanged. Each kernel takes one argument block (the item count, then each
parameter in an 8-byte slot, views as a device address and a count) and two
error words. A lane loop runs one 32-lane simdgroup per work item, and the
tensor-core primitive `cu_mma_bf16_16x8x16` keeps its PTX fragment layout:
each lane gathers the rows and columns it needs from the lanes that hold
them with `simd_shuffle` and adds the 16 exact products in order.

Two things differ from CUDA, and both are stated rather than hidden.

Metal has no trap instruction. A failed integer, bounds or slice check counts
itself in the thread's error words, takes a harmless value (element 0 for an
index) and lets the launch finish; the kernel adds its words to the launch's
once, at the end, and the host reports `PM_CHECK` and throws the launch's
results away. The words stay in registers until then because an atomic inside
every check made each later `simd_shuffle` wait: the first build ran the
86-token projections eleven times slower.

Apple GPUs have no binary64. Chapter 3's profile adds reductions in f64, so
on Metal an `f64` is a pair of f32 values whose sum is the number (profile
APPLE-FF-v1): sums are compensated (two-sum), products use `fma` for the
exact low half, and `exp`, `log` and `sqrt` are written for pairs. An `f64`
bit pattern, `sin`, `cos`, `tan` or `pow` of an `f64`, or an `f64` converted
to an integer is refused (error E0702) instead of being approximated. The
conformance kernels are ordinary Coax; compiled to C they use binary64, which
is the oracle:

<!-- source: psionic/tests/metal-ff.coax arith -->
```jai
// Sums, products and quotients of widened f32 values: a b + c, (a + b) - c
// and a / b, in that order per item.
ff_arith :: (item:int,a:[]f32,b:[]f32,c:[]f32,out:[]f64) #export_c {
    x:=f64(a[item]);y:=f64(b[item]);z:=f64(c[item]);
    out[3*item]=x*y+z;out[3*item+1]=(x+y)-z;out[3*item+2]=x/y;
}
```

The bounds were frozen before measuring: 2^-44 of the operands' combined
magnitude for arithmetic, 2^-40 relative for `exp`, `log` and `sqrt`. On 4
million random triples the arithmetic is within 3.6e-15. `exp`, `log` and
`sqrt` were checked on every one of the 2^32 f32 inputs; the first exhaustive
run found two real faults (an overflow threshold below `log FLT_MAX`, and lost
digits in `log` just below 1, now computed through `atanh` of
`(m - 1)/(m + 1)`) and one wrong frozen range: a pair's low half below 2^-126
is flushed, so `exp` keeps 40 bits only above 2^-86, not 2^-102. The bound
did not move; the range was corrected and the record says so.

```shell
psionic/tests/run_metal_kernels.sh
```

## The host and the engine

`psionic/platform/metal-host.m` opens the device, compiles the generated
source with fast math off (`MTLMathModeSafe`, precise functions), allocates
shared buffers, encodes launches into one serial compute encoder and waits.
It decides nothing, and it has no CPU fallback: without a Metal device every
call is `PM_ABSENT`.

`psionic/platform/elephant-metal.c` performs chapter 30's plan. Unified
memory changes one rule: the CPU and GPU share the same bytes, so every weight
is resident (14.8 GiB copied once, in about 5 seconds), and a plan that would
stream a weight is refused rather than run. The workspace inputs (embedding
rows and the rotary table) are computed by Coax on the host and written
straight into the shared workspace.

## Run it

```shell
psionic/tests/run_elephant_metal.sh /path/to/elephant-directory 6
```

The acceptance test is chapter 30's, unchanged: the choice must be `yes` and
each decision statistic within 0.0005 of chapter 10's independent reference
and of its recorded CPU result. The Mac returned `yes` with p(yes)
0.757200844 against the CPU's 0.757196285, a difference of 4.6e-6.

With `PSIONIC_DEVICE_QUALIFY=1` the harness repeats every launch of layers 0
and 3 and the readout on the CPU from the GPU's own inputs. 49 of 70 launches
were identical. The float-float reductions differ from binary64 by at most
5.3e-14 relative. The tensor-core projections differ as they do on the 4080,
because the Coax meaning adds each 16-product group in order and the GPU's
partial sums round differently; some of the differing outputs are partial
sums close to zero, so their relative difference is large, and the whole
decision still meets the frozen rule.

A warm decision of the 86-token sample takes 2.7 to 2.9 seconds (the median
of five warm runs, in two sessions), against 147 ms on the 4080. 90% of the
GPU time is the 86-token projections (`dw_project`), at 0.5 to 1 TFLOP/s. The 4080 does each 16x8x16 product in a tensor-core instruction;
on Metal each lane gathers its operands with 16 to 32 shuffles, and the
shuffles, not the arithmetic, set the rate. Keeping the gathered rows of the
weight fragment between the products that share it made the projections
about 1.4 times faster. Apple's own matrix units would need tiles shared
through threadgroup memory, which the compute subset does not have yet.

## Serving it

```shell
psionic/platform/serve.sh --elephant-metal /path/to/elephant-directory --port 8793 --capacity 512
```

`psionic-serve --elephant-metal` is chapter 16's loopback `/v1/systemone`
door on this engine, one decision at a time, stoppable between layers like
`--elephant-gpu`. The Lab's five questions take 23 to 31 seconds on the Mac,
against about 1.8 seconds on the 4080, and return the 4080's distributions to
seven decimal places. The [verification record](../../docs/verification/psionic-ch31-2026-10-29.md)
has the runs, the profile, the qualification and how the server stays up.

## Limits

One request, one machine, one arithmetic profile. The projections are
shuffle-bound; until the compute subset has threadgroup memory and barriers,
the Mac stays an order of magnitude behind the 4080. Opening the session still
hashes the 18.8 GB checkpoint on the CPU, about two to four minutes. There is
no batching or prefix reuse. Mammoth has not run.
