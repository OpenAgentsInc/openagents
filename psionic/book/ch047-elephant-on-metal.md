# 47. Elephant on Metal

[Chapter 44](ch044-elephant-on-cuda.md) answered chapter 14's question on an
RTX 4080. This chapter answers it on the GPU of a MacBook Pro, an Apple M5 Max
with 128 GB of unified memory, from the same Coax kernels and the same device
plan. The answer agrees with the CPU to six decimal places. The first build
was about nineteen times slower than the 4080; Apple's matrix units for the
projections and one prefix pass per request brought a decision to about
0.4 s and the Lab's five questions to about 2.4 s, and this chapter says how.

The reason to run Elephant here is practical. One RTX 4080 holds Elephant or
gpt-oss-20b, not both, so a router on the 4080 forces a model swap before
every answer. With Elephant on the Mac, gpt-oss-20b stays loaded on the 4080
and the desktop app's Lab asks one, then the other, with nothing swapped.

The plan's title promises Mammoth too. Only Elephant has run.

## The same kernels, lowered for Metal

`coax metal-compute FILE` prints a file's kernels as Metal Shading Language.
It admits exactly what `coax cuda` admits ([chapter 37](ch037-a-coax-compute-subset.md)),
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

Apple GPUs have no binary64. Chapter 4's profile adds reductions in f64, so
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

`psionic/platform/elephant-metal.c` performs chapter 44's plan. Unified
memory changes one rule: the CPU and GPU share the same bytes, so every weight
is resident (14.8 GiB copied once, in about 5 seconds), and a plan that would
stream a weight is refused rather than run. The workspace inputs (embedding
rows and the rotary table) are computed by Coax on the host and written
straight into the shared workspace.

## Run it

```shell
psionic/tests/run_elephant_metal.sh /path/to/elephant-directory 6
```

The acceptance test is chapter 44's, unchanged: the choice must be `yes` and
each decision statistic within 0.0005 of chapter 14's independent reference
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

The first build's warm decision of the 86-token sample took 2.7 to 2.9
seconds (the median of five warm runs, in two sessions), against 147 ms on
the 4080. 90% of the GPU time was the 86-token projections (`dw_project`), at
0.5 to 1 TFLOP/s: the 4080 does each 16x8x16 product in a tensor-core
instruction, while on Metal each lane gathered its operands with 16 to 32
shuffles, and the shuffles, not the arithmetic, set the rate.

## Apple's matrix units

Apple's matrix units are reached through tiles that a threadgroup shares, so
the compute subset first gained group kernels ([coax/README.md](../../coax/README.md)):
`for thread: 0..<T` inside a kernel runs T threads of one work item, which
share memory only through writes each thread owns and barriers between them,
on Metal and CUDA alike, with a tiled matrix product checked bit for bit
against its C oracle on both. On top of them, `coax/lib/compute/apple.coax`'s
`am_matmul_bf16` is one Metal 4 tensor operation (Metal Performance
Primitives `matmul2d`, bfloat weights times float activations, accumulated in
float) whose Coax meaning, compiled to C, is the oracle. Metal's plan
(`dp_plan_metal`) replaces every whole-prompt projection with one launch of a
tile kernel and no split or reduce:

<!-- source: psionic/lib/device/tile.coax tiles -->
```jai
// Work item i of a TM x TN tiling: token tile i % tiles(tokens), row tile
// i / tiles(tokens), so neighbouring threadgroups share one weight tile.
DT_THREADS :: 128;
dt_tiles :: (n:int,tile:int)->int {return (n+tile-1)/tile;}
dt_items :: (rows:int,tokens:int,tm:int,tn:int)->int {return dt_tiles(tokens,tm)*dt_tiles(rows,tn);}
dt_project_16x32 :: (item:int,rows:int,cols:int,tokens:int,w:[]u32,x:[]f32,out:[]f32) #export_c {
    t:=dt_tiles(tokens,16);
    am_matmul_bf16(128,16,32,rows,cols,tokens,item/t*32,item%t*16,w,x,out);
}
dt_project_32x32 :: (item:int,rows:int,cols:int,tokens:int,w:[]u32,x:[]f32,out:[]f32) #export_c {
    t:=dt_tiles(tokens,32);
    am_matmul_bf16(128,32,32,rows,cols,tokens,item/t*32,item%t*32,w,x,out);
}
dt_project_16x64 :: (item:int,rows:int,cols:int,tokens:int,w:[]u32,x:[]f32,out:[]f32) #export_c {
    t:=dt_tiles(tokens,16);
    am_matmul_bf16(128,16,64,rows,cols,tokens,item/t*64,item%t*16,w,x,out);
}
```

Each tile is 16 tokens by 64 rows (16 by 32 for the 32-row gate
projections), 128 threads. On the six 86-token shapes the projections went
from 0.2 to 13.2 ms to 0.05 to 0.76 ms each, up to 11.9 TFLOP/s against
29.4 TFLOP/s measured for the same tensor operation on large half-precision
shapes; their error is at most 7e-7 of `sum |w x|` (bound 2^-20, frozen
before measuring). The accumulation order is the hardware's, so a tile is not
bit-identical to the Coax meaning: the numerical profile is BF16 products
accumulated in f32 in an unspecified order, checked by that bound and by the
whole decision. A warm decision took 425 ms (median of six; the fastest
202 ms, the machine shared with other work), p(yes) 0.757192428 against the
CPU's 0.757196285, and `PSIONIC_DEVICE_QUALIFY=1` still passes: 19 of 40
launches identical, float-float reductions within 4.6e-14, the tiles differing
only as accumulation order allows. The tiles are now 56% of the GPU time,
then the delta rule (14%) and the gated norm (9%).

## One prefix per request

The Lab asks five questions about one message, so the message's prefix block
was decided five times. Chapter 12's fork runs it once: the prefix pass keeps
each linear layer's last three convolution inputs and delta-rule state and
each full layer's key and value rows in an 82 MiB fork buffer, and each
question's suffix starts from them (`dl_conv_state`, `dl_scan_state`) and
attends to every prefix key. The engine keeps the prefix while the next
request's tokens match it.

<!-- source: psionic/lib/device/plan.coax forkplan -->
```jai
// A forked decision on an opened placement: the prefix pass over tokens
// 0..prefix (one block, no readout), then one suffix pass per question over
// tokens start..start+tokens, whose logits are the last token's.
dp_plan_prefix :: (prefix:int,opened:DpPlan,stream:[]bool,ops:[]DpOp)->DpPlan #export_c {
    return dp_plan_fork(DpFork.{on=true,save=true},prefix,opened,stream,ops);
}
dp_plan_suffix :: (start:int,tokens:int,opened:DpPlan,stream:[]bool,ops:[]DpOp)->DpPlan #export_c {
    if start<1 {return DpPlan.{status=DpStatus.shape};}
    return dp_plan_fork(DpFork.{on=true,start=start},tokens,opened,stream,ops);
}
dp_plan_fork :: (fork:DpFork,tokens:int,opened:DpPlan,stream:[]bool,ops:[]DpOp)->DpPlan {
    m:=dp_model(opened.kind);
    if opened.status!=DpStatus.ok || stream.count!=dp_streamable(m) {return DpPlan.{status=DpStatus.capacity};}
    if tokens<1 || fork.start<0 || fork.start+tokens>DP_MAX_TOKENS {return DpPlan.{status=DpStatus.shape};}
    if dp_layout_of(m,tokens,fork.start+tokens).total>opened.workspace {return DpPlan.{status=DpStatus.capacity};}
    n:=0;for i:0..<stream.count {if stream[i] {n+=1;}}
    if n!=opened.streamed_tensors {return DpPlan.{status=DpStatus.capacity};}
    return dp_schedule_fork(tokens,0,fork,opened,stream,ops);
}
```

The forked decision is bit-identical to the whole decision: on the sample,
none of the 496,640 logits differ, on this Mac and on the 4080. The suffix
alone takes about 244 ms here.

## Serving it

```shell
psionic/platform/serve.sh --elephant-metal /path/to/elephant-directory --port 8793 --capacity 512
```

`psionic-serve --elephant-metal` is chapter 20's loopback `/v1/systemone`
door on this engine, one decision at a time, stoppable between layers like
`--elephant-gpu`. The Lab's five questions took 23 to 31 seconds on the
first build; with the tiles 4.1 to 5.4 s, and with forks 2.3 to 2.5 s,
whether or not the prefix was seen before, with answers unchanged. The 4080
went from 1.84 to 1.17 s with forks. A verified-file record ([chapter 19](../../docs/verification/psionic-ch15-verified-files-2026-10-29.md))
lets a restart skip the checkpoint hash, so the server is up in about 4 s. The [verification record](../../docs/verification/psionic-ch31-2026-10-29.md)
has the runs, the profile, the qualification and how the server stays up.

## Limits

One request shape, one machine, one arithmetic profile per kernel. The
decision's GPU time varies from 0.2 to 0.5 s with what else the machine is
doing. Questions run one after another, not batched (chapter 36). Mammoth has
not run.
